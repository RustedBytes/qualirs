use syn::visit::{Visit, visit_expr_for_loop};

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects loops that look like manual find/any/all operations.
pub struct ManualFindLoopDetector;

impl Detector for ManualFindLoopDetector {
    fn name(&self) -> &str {
        "Manual Find/Any Loop"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = ManualFindVisitor {
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|line| {
                Smell::new(
                    SmellCategory::Idiomaticity,
                    "Manual Find/Any Loop",
                    Severity::Info,
                    crate::domain::smell::FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    "Loop returns early based on a predicate",
                    "Consider iterator adapters such as find, any, all, or position.",
                )
            })
            .collect()
    }
}

struct ManualFindVisitor {
    findings: Vec<usize>,
}

impl<'ast> Visit<'ast> for ManualFindVisitor {
    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        if loop_has_direct_conditional_return(&node.body) {
            self.findings.push(node.for_token.span.start().line);
        }
        visit_expr_for_loop(self, node);
    }
}

fn loop_has_direct_conditional_return(block: &syn::Block) -> bool {
    let [syn::Stmt::Expr(syn::Expr::If(condition), _)] = block.stmts.as_slice() else {
        return false;
    };
    if condition.else_branch.is_some() {
        return false;
    }
    // A predicate adapter cannot discard branch work or move an executed await
    // or a function-level early exit into a synchronous predicate closure.
    let [syn::Stmt::Expr(result, _)] = condition.then_branch.stmts.as_slice() else {
        return false;
    };
    let mut boundary = PredicateBoundary(false);
    boundary.visit_expr(&condition.cond);
    expr_is_bool_return(result) && !boundary.0
}

struct PredicateBoundary(bool);
impl<'ast> Visit<'ast> for PredicateBoundary {
    fn visit_expr(&mut self, expr: &'ast syn::Expr) {
        if matches!(
            expr,
            syn::Expr::Await(_)
                | syn::Expr::Try(_)
                | syn::Expr::Return(_)
                | syn::Expr::Break(_)
                | syn::Expr::Continue(_)
                | syn::Expr::Yield(_)
                | syn::Expr::Macro(_)
        ) {
            self.0 = true;
        }
        syn::visit::visit_expr(self, expr);
    }
    fn visit_macro(&mut self, _: &'ast syn::Macro) {
        self.0 = true;
    }
}

fn expr_is_bool_return(expr: &syn::Expr) -> bool {
    let syn::Expr::Return(return_expr) = expr else {
        return false;
    };
    return_expr.expr.as_ref().is_some_and(
        |expr| matches!(&**expr, syn::Expr::Lit(lit) if matches!(lit.lit, syn::Lit::Bool(_))),
    )
}
