use std::collections::HashSet;

use syn::visit::{Visit, visit_block};

use crate::analysis::detector::Detector;
use crate::detectors::implementation::perf_utils::{
    expr_path_tail, is_obvious_copy_type, pat_ident, stmt_contains_ident, type_is_reference,
};
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects `push(value.clone())` or `insert(value.clone())` when the source is not used again.
pub struct CloneBeforeMoveIntoCollectionDetector;

impl Detector for CloneBeforeMoveIntoCollectionDetector {
    fn name(&self) -> &str {
        "Clone Before Move Into Collection"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Fn(item_fn) = item {
                let mut visitor = CloneMoveVisitor {
                    owned_vars: collect_owned_params(&item_fn.sig),
                    block_depth: 0,
                    findings: Vec::new(),
                };
                visitor.visit_block(&item_fn.block);

                for (line, name) in visitor.findings {
                    smells.push(Smell::new(
                        SmellCategory::Performance,
                        "Clone Before Move Into Collection",
                        Severity::Info,
                                                crate::domain::smell::FindingConfidence::High,
                        SourceLocation::new(file.path.clone(), line, line, None),
                        format!("`{name}` is cloned into a collection and is not used afterwards"),
                        "Move the value into the collection directly when the original binding is no longer needed.",
                    ));
                }
            }
        }

        smells
    }
}

struct CloneMoveVisitor {
    owned_vars: HashSet<String>,
    block_depth: usize,
    findings: Vec<(usize, String)>,
}

impl<'ast> Visit<'ast> for CloneMoveVisitor {
    fn visit_block(&mut self, node: &'ast syn::Block) {
        let outer_owned_vars = if self.block_depth == 0 {
            None
        } else {
            Some(std::mem::take(&mut self.owned_vars))
        };

        for (index, stmt) in node.stmts.iter().enumerate() {
            self.record_owned_local(stmt);

            if let Some((line, name)) = cloned_value_moved_into_collection(stmt)
                && self.owned_vars.contains(&name)
                && !node
                    .stmts
                    .iter()
                    .skip(index + 1)
                    .any(|later| stmt_contains_ident(later, &name))
            {
                self.findings.push((line, name));
            }
        }

        self.block_depth += 1;
        visit_block(self, node);
        self.block_depth -= 1;

        if let Some(outer_owned_vars) = outer_owned_vars {
            self.owned_vars = outer_owned_vars;
        }
    }
}

impl CloneMoveVisitor {
    fn record_owned_local(&mut self, stmt: &syn::Stmt) {
        let syn::Stmt::Local(local) = stmt else {
            return;
        };

        let Some(name) = pat_ident(&local.pat) else {
            return;
        };

        if local_declares_owned_binding(local) {
            self.owned_vars.insert(name);
        }
    }
}

fn local_declares_owned_binding(local: &syn::Local) -> bool {
    if let syn::Pat::Type(pat_type) = &local.pat {
        return !type_is_reference(&pat_type.ty) && !is_obvious_copy_type(&pat_type.ty);
    }

    matches!(&local.pat, syn::Pat::Ident(_))
        && local
            .init
            .as_ref()
            .is_some_and(|init| init_is_owned(&init.expr))
}

fn collect_owned_params(sig: &syn::Signature) -> HashSet<String> {
    sig.inputs
        .iter()
        .filter_map(|input| match input {
            syn::FnArg::Typed(pat_type)
                if !type_is_reference(&pat_type.ty) && !is_obvious_copy_type(&pat_type.ty) =>
            {
                pat_ident(&pat_type.pat)
            }
            _ => None,
        })
        .collect()
}

fn cloned_value_moved_into_collection(stmt: &syn::Stmt) -> Option<(usize, String)> {
    let expr = match stmt {
        syn::Stmt::Expr(expr, _) => expr,
        _ => return None,
    };

    let syn::Expr::MethodCall(call) = expr else {
        return None;
    };

    if !matches!(call.method.to_string().as_str(), "push" | "insert") {
        return None;
    }

    call.args.iter().find_map(cloned_simple_ident)
}

fn cloned_simple_ident(expr: &syn::Expr) -> Option<(usize, String)> {
    let syn::Expr::MethodCall(call) = expr else {
        return None;
    };
    if call.method != "clone" || !call.args.is_empty() {
        return None;
    }

    expr_path_tail(&call.receiver).map(|name| (call.method.span().start().line, name))
}

fn init_is_owned(expr: &syn::Expr) -> bool {
    matches!(
        expr,
        syn::Expr::Call(_) | syn::Expr::Struct(_) | syn::Expr::Array(_) | syn::Expr::Tuple(_)
    ) || matches!(
        expr,
        syn::Expr::Macro(expr) if expr.mac.path.is_ident("vec") || expr.mac.path.is_ident("format")
    ) || matches!(expr, syn::Expr::Lit(lit) if matches!(lit.lit, syn::Lit::Str(_)))
}
