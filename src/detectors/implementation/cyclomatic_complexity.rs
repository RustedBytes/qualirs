use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects functions with high cyclomatic complexity.
///
/// CC = 1 + branches, where branches are: `if`, `while`, `for`, `loop`,
/// `match` arms (each arm after the first), `&&`, `||`, `?`.
pub struct CyclomaticComplexityDetector;

impl Detector for CyclomaticComplexityDetector {
    fn name(&self) -> &str {
        "High Cyclomatic Complexity"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Fn(fn_item) = item {
                let mut visitor = CcVisitor { cc: 1 };
                visitor.visit_block(&fn_item.block);

                if visitor.cc > thresholds.r#impl.control_flow.cyclomatic_complexity {
                    let line = fn_item.sig.fn_token.span.start().line;

                    smells.push(complexity_smell(
                        file,
                        &fn_item.sig.ident,
                        visitor.cc,
                        thresholds.r#impl.control_flow.cyclomatic_complexity,
                        line,
                    ));
                }
            }
        }

        smells
    }
}

fn complexity_smell(
    file: &SourceFile,
    ident: &syn::Ident,
    complexity: usize,
    threshold: usize,
    line: usize,
) -> Smell {
    Smell::new(
        SmellCategory::Implementation,
        "High Cyclomatic Complexity",
        if complexity > threshold * 2 {
            Severity::Critical
        } else {
            Severity::Warning
        },
        crate::domain::smell::FindingConfidence::Medium,
        SourceLocation {
            file: file.path.clone(),
            line_start: line,
            line_end: line,
            column: None,
        },
        format!(
            "Function `{ident}` has cyclomatic complexity of {complexity} (threshold: {threshold})"
        ),
        "Reduce branching. Extract helper functions, use early returns, or leverage combinators.",
    )
}

struct CcVisitor {
    cc: usize,
}

impl<'ast> Visit<'ast> for CcVisitor {
    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        self.cc += 1;
        syn::visit::visit_expr_if(self, node);
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.cc += 1;
        syn::visit::visit_expr_while(self, node);
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.cc += 1;
        syn::visit::visit_expr_for_loop(self, node);
    }

    fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
        self.cc += 1;
        syn::visit::visit_expr_loop(self, node);
    }

    fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
        // Each match arm after the first adds +1
        if node.arms.len() > 1 {
            self.cc += node.arms.len() - 1;
        }
        syn::visit::visit_expr_match(self, node);
    }

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        match node.op {
            syn::BinOp::And(_) | syn::BinOp::Or(_) => self.cc += 1,
            _ => {}
        }
        syn::visit::visit_expr_binary(self, node);
    }

    fn visit_expr_try(&mut self, node: &'ast syn::ExprTry) {
        self.cc += 1;
        syn::visit::visit_expr_try(self, node);
    }
}
