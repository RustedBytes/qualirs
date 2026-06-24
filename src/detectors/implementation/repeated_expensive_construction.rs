use std::collections::HashSet;

use syn::visit::{
    Visit, visit_expr_call, visit_expr_for_loop, visit_expr_loop, visit_expr_while, visit_item_fn,
    visit_item_mod, visit_local,
};

use crate::analysis::detector::Detector;
use crate::detectors::implementation::perf_utils::{
    collect_pat_idents, expr_contains_any_ident, path_to_string,
};
use crate::detectors::policy::has_test_cfg;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects invariant expensive constructors inside loops.
pub struct RepeatedExpensiveConstructionDetector;

impl Detector for RepeatedExpensiveConstructionDetector {
    fn name(&self) -> &str {
        "Repeated Expensive Construction in Loop"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = ExpensiveConstructionVisitor {
            loop_depth: 0,
            loop_bindings: Vec::new(),
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|(line, call)| {
                Smell::new(
                    SmellCategory::Performance,
                    "Repeated Expensive Construction in Loop",
                    Severity::Info,
                                        crate::domain::smell::FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    format!("`{call}` is constructed inside a loop from loop-invariant input"),
                    "Hoist invariant parsers, URL patterns, glob patterns, and path templates outside the loop.",
                )
            })
            .collect()
    }
}

struct ExpensiveConstructionVisitor {
    loop_depth: usize,
    loop_bindings: Vec<HashSet<String>>,
    findings: Vec<(usize, String)>,
}

impl<'ast> Visit<'ast> for ExpensiveConstructionVisitor {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if has_test_cfg(&node.attrs) {
            return;
        }
        visit_item_mod(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if has_test_cfg(&node.attrs) {
            return;
        }
        visit_item_fn(self, node);
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        let mut bindings = HashSet::new();
        collect_pat_idents(&node.pat, &mut bindings);
        self.loop_depth += 1;
        self.loop_bindings.push(bindings);
        visit_expr_for_loop(self, node);
        self.loop_bindings.pop();
        self.loop_depth -= 1;
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.loop_depth += 1;
        self.loop_bindings.push(HashSet::new());
        visit_expr_while(self, node);
        self.loop_bindings.pop();
        self.loop_depth -= 1;
    }

    fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
        self.loop_depth += 1;
        self.loop_bindings.push(HashSet::new());
        visit_expr_loop(self, node);
        self.loop_bindings.pop();
        self.loop_depth -= 1;
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        visit_local(self, node);
        if self.loop_depth > 0
            && let Some(bindings) = self.loop_bindings.last_mut()
        {
            collect_pat_idents(&node.pat, bindings);
        }
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if self.loop_depth == 0 {
            visit_expr_call(self, node);
            return;
        }

        if let syn::Expr::Path(path) = &*node.func {
            let call = path_to_string(&path.path);
            if is_expensive_constructor(&call, node) && self.args_look_loop_invariant(node) {
                self.findings.push((
                    path.path
                        .segments
                        .last()
                        .map(|segment| segment.ident.span().start().line)
                        .unwrap_or(1),
                    call,
                ));
            }
        }

        visit_expr_call(self, node);
    }
}

impl ExpensiveConstructionVisitor {
    fn args_look_loop_invariant(&self, node: &syn::ExprCall) -> bool {
        let loop_bindings = self
            .loop_bindings
            .iter()
            .flat_map(|bindings| bindings.iter().cloned())
            .collect::<HashSet<_>>();

        if loop_bindings.is_empty() {
            return node.args.iter().all(expr_is_literal_like);
        }

        node.args
            .iter()
            .all(|arg| !expr_contains_any_ident(arg, &loop_bindings))
    }
}

fn is_expensive_constructor(call: &str, node: &syn::ExprCall) -> bool {
    matches!(
        call,
        "Url::parse"
            | "url::Url::parse"
            | "glob::Pattern::new"
            | "globset::Glob::new"
            | "Selector::parse"
            | "scraper::Selector::parse"
    ) || (call.ends_with("PathBuf::from") && node.args.iter().all(expr_is_literal_like))
}

fn expr_is_literal_like(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Lit(_) => true,
        syn::Expr::Reference(reference) => expr_is_literal_like(&reference.expr),
        syn::Expr::Paren(paren) => expr_is_literal_like(&paren.expr),
        syn::Expr::Array(array) => array.elems.iter().all(expr_is_literal_like),
        _ => false,
    }
}
