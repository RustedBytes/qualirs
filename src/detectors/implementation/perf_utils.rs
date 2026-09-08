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

mod collection_usage;
pub(super) use collection_usage::{movable_clones, sort_selections};
