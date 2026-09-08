use super::{expr_contains_any_ident, int_lit_value, pat_ident, stmt_contains_ident};
use crate::analysis::evidence::{self, Kind};
use std::collections::HashSet;
use syn::{spanned::Spanned, visit::Visit};

/// Only direct adjacent sort/selection pairs on an owned local Vec qualify.
/// Earlier aliases and later uses of the collection invalidate the conclusion.
pub(in crate::detectors::implementation) fn sort_selections(
    file: &syn::File,
    indexed: bool,
) -> Vec<usize> {
    let mut candidates = SortCandidates {
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
            selection_method(c, name, indexed).then_some(expr)
        }
        syn::Expr::Index(i) if indexed && ident(&i.expr).as_deref() == Some(name) => {
            selection_index(&i.index, name).then_some(expr)
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

pub(in crate::detectors::implementation) fn movable_clones(file: &syn::File) -> Vec<usize> {
    let mut candidates = CloneCandidates(HashSet::new());
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

struct SortCandidates {
    keys: HashSet<(usize, usize, usize, usize)>,
    indexed: bool,
}
impl<'a> Visit<'a> for SortCandidates {
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

struct CloneCandidates(HashSet<(usize, usize, usize, usize)>);
impl<'a> Visit<'a> for CloneCandidates {
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

fn selection_method(c: &syn::ExprMethodCall, name: &str, indexed: bool) -> bool {
    if indexed {
        c.method == "get" && c.args.len() == 1 && selection_index(&c.args[0], name)
    } else {
        matches!(c.method.to_string().as_str(), "first" | "last") && c.args.is_empty()
    }
}

fn selection_index(expr: &syn::Expr, name: &str) -> bool {
    int_lit_value(expr) != Some(0) && simple_index(expr, name)
}
