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
    let mut statements = block.stmts.iter();
    let Some(stmt) = statements.next() else {
        return false;
    };
    if statements.next().is_some() {
        return false;
    }

    stmt_is_conditional_return(stmt)
}

fn stmt_is_conditional_return(stmt: &syn::Stmt) -> bool {
    match stmt {
        syn::Stmt::Expr(expr, _) => expr_is_conditional_return(expr),
        syn::Stmt::Local(local) => local
            .init
            .as_ref()
            .is_some_and(|init| expr_is_conditional_return(&init.expr)),
        _ => false,
    }
}

fn expr_is_conditional_return(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::If(if_expr) => {
            block_contains_return(&if_expr.then_branch)
                || if_expr.else_branch.as_ref().is_some_and(|(_, else_expr)| {
                    expr_is_bool_return(else_expr) || expr_is_conditional_return(else_expr)
                })
        }
        syn::Expr::Block(block) => loop_has_direct_conditional_return(&block.block),
        _ => false,
    }
}

fn block_contains_return(block: &syn::Block) -> bool {
    block.stmts.iter().any(|stmt| match stmt {
        syn::Stmt::Expr(expr, _) => expr_is_bool_return(expr),
        _ => false,
    })
}

fn expr_is_bool_return(expr: &syn::Expr) -> bool {
    let syn::Expr::Return(return_expr) = expr else {
        return false;
    };
    return_expr.expr.as_ref().is_some_and(
        |expr| matches!(&**expr, syn::Expr::Lit(lit) if matches!(lit.lit, syn::Lit::Bool(_))),
    )
}
