use std::collections::HashSet;

use syn::punctuated::Punctuated;
use syn::visit::Visit;

pub(super) fn path_to_string(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

pub(super) fn expr_path_tail(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        syn::Expr::Field(field) => match &field.member {
            syn::Member::Named(name) => Some(name.to_string()),
            syn::Member::Unnamed(_) => None,
        },
        syn::Expr::Reference(reference) => expr_path_tail(&reference.expr),
        syn::Expr::Paren(paren) => expr_path_tail(&paren.expr),
        _ => None,
    }
}

pub(super) fn pat_ident(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(ident) => Some(ident.ident.to_string()),
        syn::Pat::Type(pat_type) => pat_ident(&pat_type.pat),
        syn::Pat::Reference(reference) => pat_ident(&reference.pat),
        _ => None,
    }
}

pub(super) fn collect_pat_idents(pat: &syn::Pat, out: &mut HashSet<String>) {
    match pat {
        syn::Pat::Ident(ident) => {
            out.insert(ident.ident.to_string());
        }
        syn::Pat::Reference(reference) => collect_pat_idents(&reference.pat, out),
        syn::Pat::Slice(slice) => {
            for elem in &slice.elems {
                collect_pat_idents(elem, out);
            }
        }
        syn::Pat::Struct(pat_struct) => {
            for field in &pat_struct.fields {
                collect_pat_idents(&field.pat, out);
            }
        }
        syn::Pat::Tuple(tuple) => {
            for elem in &tuple.elems {
                collect_pat_idents(elem, out);
            }
        }
        syn::Pat::TupleStruct(tuple) => {
            for elem in &tuple.elems {
                collect_pat_idents(elem, out);
            }
        }
        syn::Pat::Type(pat_type) => collect_pat_idents(&pat_type.pat, out),
        _ => {}
    }
}

pub(super) fn int_lit_value(expr: &syn::Expr) -> Option<u128> {
    match expr {
        syn::Expr::Lit(lit) => match &lit.lit {
            syn::Lit::Int(int) => int.base10_parse().ok(),
            _ => None,
        },
        syn::Expr::Paren(paren) => int_lit_value(&paren.expr),
        _ => None,
    }
}

pub(super) fn macro_first_expr_ident(mac: &syn::Macro) -> Option<String> {
    let args = mac
        .parse_body_with(Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated)
        .ok()?;
    expr_path_tail(args.first()?)
}

pub(super) fn macro_mentions_any_ident(mac: &syn::Macro, targets: &HashSet<String>) -> bool {
    if targets.is_empty() {
        return false;
    }

    let tokens = mac.tokens.to_string();
    targets
        .iter()
        .any(|target| macro_tokens_mention_ident(&tokens, target))
}

pub(super) fn type_path_tail(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        syn::Type::Reference(reference) => type_path_tail(&reference.elem),
        syn::Type::Paren(paren) => type_path_tail(&paren.elem),
        syn::Type::Group(group) => type_path_tail(&group.elem),
        _ => None,
    }
}

pub(super) fn type_contains_dyn(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::TraitObject(_) => true,
        syn::Type::Reference(reference) => type_contains_dyn(&reference.elem),
        syn::Type::Paren(paren) => type_contains_dyn(&paren.elem),
        syn::Type::Group(group) => type_contains_dyn(&group.elem),
        syn::Type::Path(path) => path.path.segments.iter().any(|segment| {
            let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
                return false;
            };
            args.args.iter().any(|arg| match arg {
                syn::GenericArgument::Type(ty) => type_contains_dyn(ty),
                _ => false,
            })
        }),
        _ => false,
    }
}

pub(super) fn stmt_contains_ident(stmt: &syn::Stmt, ident: &str) -> bool {
    let mut visitor = IdentUseVisitor {
        targets: HashSet::from([ident.to_string()]),
        found: false,
    };
    visitor.visit_stmt(stmt);
    visitor.found
}

pub(super) fn expr_contains_any_ident(expr: &syn::Expr, targets: &HashSet<String>) -> bool {
    if targets.is_empty() {
        return false;
    }

    let mut visitor = IdentUseVisitor {
        targets: targets.clone(),
        found: false,
    };
    visitor.visit_expr(expr);
    visitor.found
}

struct IdentUseVisitor {
    targets: HashSet<String>,
    found: bool,
}

impl<'ast> Visit<'ast> for IdentUseVisitor {
    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if self.found {
            return;
        }

        if let Some(ident) = node.path.get_ident()
            && self.targets.contains(&ident.to_string())
        {
            self.found = true;
            return;
        }

        syn::visit::visit_expr_path(self, node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if self.found {
            return;
        }

        let tokens = node.tokens.to_string();
        if self
            .targets
            .iter()
            .any(|target| macro_tokens_mention_ident(&tokens, target))
        {
            self.found = true;
            return;
        }

        syn::visit::visit_macro(self, node);
    }
}

fn macro_tokens_mention_ident(tokens: &str, ident: &str) -> bool {
    tokens
        .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .any(|part| part == ident)
}

/// Only direct adjacent sort/selection pairs on an owned local Vec qualify.
/// Earlier aliases and later uses of the collection invalidate the conclusion.
pub(super) fn sort_selections(file: &syn::File, indexed: bool) -> Vec<usize> {
    use crate::analysis::evidence::{self, Kind};
    use syn::spanned::Spanned;
    struct Candidates {
        keys: HashSet<(usize, usize, usize, usize)>,
        indexed: bool,
    }
    impl<'a> Visit<'a> for Candidates {
        fn visit_block(&mut self, block: &'a syn::Block) {
            for (i, pair) in block.stmts.windows(2).enumerate() {
                let syn::Stmt::Expr(syn::Expr::MethodCall(sort), Some(_)) = &pair[0] else {
                    continue;
                };
                if !matches!(sort.method.to_string().as_str(), "sort" | "sort_unstable") {
                    continue;
                }
                let Some(name) = evidence::ident(&sort.receiver) else {
                    continue;
                };
                if block.stmts[..i]
                    .iter()
                    .any(|s| mentions_except_declaration(s, &name))
                    || block.stmts[i + 2..]
                        .iter()
                        .any(|s| stmt_contains_ident(s, &name))
                {
                    continue;
                }
                let Some(expr) = statement_value(&pair[1]) else {
                    continue;
                };
                if let Some(selection) = selection(expr, &name, self.indexed) {
                    self.keys.insert(evidence::span_key(selection.span()));
                }
            }
            syn::visit::visit_block(self, block);
        }
    }
    let mut candidates = Candidates {
        keys: HashSet::new(),
        indexed,
    };
    candidates.visit_file(file);
    let mut lines = Vec::new();
    evidence::inspect(file, |expr, ctx| {
        if !candidates.keys.contains(&evidence::span_key(expr.span())) {
            return;
        }
        let receiver = match expr {
            syn::Expr::MethodCall(c) => &*c.receiver,
            syn::Expr::Index(i) => &*i.expr,
            _ => return,
        };
        if ctx.expr(receiver) == Kind::Vec
            && ctx.loop_depth == 0
            && ctx.is_current_binding(receiver)
        {
            lines.push(expr.span().start().line);
        }
    });
    lines
}

fn selection<'a>(expr: &'a syn::Expr, name: &str, indexed: bool) -> Option<&'a syn::Expr> {
    use crate::analysis::evidence::ident;
    match expr {
        syn::Expr::Return(r) => selection(r.expr.as_ref()?, name, indexed),
        syn::Expr::Paren(p) => selection(&p.expr, name, indexed),
        syn::Expr::MethodCall(c)
            if matches!(c.method.to_string().as_str(), "copied" | "cloned")
                && c.args.is_empty() =>
        {
            selection(&c.receiver, name, indexed)
        }
        syn::Expr::MethodCall(c) if ident(&c.receiver).as_deref() == Some(name) => {
            if !indexed
                && matches!(c.method.to_string().as_str(), "first" | "last")
                && c.args.is_empty()
                || indexed
                    && c.method == "get"
                    && c.args.len() == 1
                    && int_lit_value(&c.args[0]) != Some(0)
                    && simple_index(&c.args[0], name)
            {
                Some(expr)
            } else {
                None
            }
        }
        syn::Expr::Index(i)
            if indexed
                && ident(&i.expr).as_deref() == Some(name)
                && int_lit_value(&i.index) != Some(0)
                && simple_index(&i.index, name) =>
        {
            Some(expr)
        }
        _ => None,
    }
}

fn simple_index(expr: &syn::Expr, name: &str) -> bool {
    match expr {
        syn::Expr::Lit(_) | syn::Expr::Path(_) => true,
        syn::Expr::Paren(p) => simple_index(&p.expr, name),
        syn::Expr::Binary(b) => simple_index(&b.left, name) && simple_index(&b.right, name),
        syn::Expr::MethodCall(c) => {
            c.method == "len"
                && c.args.is_empty()
                && crate::analysis::evidence::ident(&c.receiver).as_deref() == Some(name)
        }
        _ => false,
    }
}

fn statement_value(stmt: &syn::Stmt) -> Option<&syn::Expr> {
    match stmt {
        syn::Stmt::Expr(e, _) => Some(e),
        syn::Stmt::Local(l) => Some(&l.init.as_ref()?.expr),
        _ => None,
    }
}

fn mentions_except_declaration(stmt: &syn::Stmt, name: &str) -> bool {
    if let syn::Stmt::Local(l) = stmt
        && pat_ident(&l.pat).as_deref() == Some(name)
    {
        return l
            .init
            .as_ref()
            .is_some_and(|i| expr_contains_any_ident(&i.expr, &HashSet::from([name.to_string()])));
    }
    stmt_contains_ident(stmt, name)
}

pub(super) fn movable_clones(file: &syn::File) -> Vec<usize> {
    use crate::analysis::evidence::{self, Kind};
    use syn::spanned::Spanned;
    struct Candidates(HashSet<(usize, usize, usize, usize)>);
    impl<'a> Visit<'a> for Candidates {
        fn visit_block(&mut self, block: &'a syn::Block) {
            for (i, stmt) in block.stmts.iter().enumerate() {
                let syn::Stmt::Expr(syn::Expr::MethodCall(c), _) = stmt else {
                    continue;
                };
                if c.method != "push" || c.args.len() != 1 {
                    continue;
                }
                let syn::Expr::MethodCall(clone) = &c.args[0] else {
                    continue;
                };
                if clone.method != "clone" || !clone.args.is_empty() {
                    continue;
                }
                let Some(name) = evidence::ident(&clone.receiver) else {
                    continue;
                };
                if expr_contains_any_ident(&c.receiver, &HashSet::from([name.clone()]))
                    || block.stmts[..i]
                        .iter()
                        .any(|s| mentions_except_declaration(s, &name))
                    || block.stmts[i + 1..]
                        .iter()
                        .any(|s| stmt_contains_ident(s, &name))
                {
                    continue;
                }
                self.0.insert(evidence::span_key(c.span()));
            }
            syn::visit::visit_block(self, block);
        }
    }
    let mut candidates = Candidates(HashSet::new());
    candidates.visit_file(file);
    let mut lines = Vec::new();
    evidence::inspect(file, |expr, ctx| {
        if !candidates.0.contains(&evidence::span_key(expr.span())) || ctx.loop_depth > 0 {
            return;
        }
        let syn::Expr::MethodCall(c) = expr else {
            return;
        };
        let syn::Expr::MethodCall(clone) = &c.args[0] else {
            return;
        };
        // A nested branch can borrow a value later used by its parent. Require
        // the source declaration and use to share the current lexical block.
        if *ctx.expr(&c.receiver).value() == Kind::Vec
            && matches!(ctx.expr(&clone.receiver), Kind::String | Kind::Vec)
            && ctx.is_current_binding(&clone.receiver)
        {
            lines.push(clone.method.span().start().line);
        }
    });
    lines
}
