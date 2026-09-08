use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects deeply nested if/else chains.
pub struct DeepIfElseDetector;

impl Detector for DeepIfElseDetector {
    fn name(&self) -> &str {
        "Deep If/Else Nesting"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        crate::analysis::visitor::visit_functions(&file.ast, |sig, block| {
            let mut visitor = IfDepthVisitor {
                current_depth: 0,
                max_depth: 0,
            };
            visitor.visit_block(block);

            if visitor.max_depth > thresholds.r#impl.control_flow.deep_if_else {
                let line = sig.fn_token.span.start().line;

                smells.push(if_depth_smell(
                    file,
                    &sig.ident,
                    visitor.max_depth,
                    thresholds.r#impl.control_flow.deep_if_else,
                    line,
                ));
            }
        });

        smells
    }
}

fn if_depth_smell(
    file: &SourceFile,
    ident: &syn::Ident,
    depth: usize,
    threshold: usize,
    line: usize,
) -> Smell {
    Smell::new(
        SmellCategory::Implementation,
        "Deep If/Else Nesting",
        Severity::Warning,
        crate::domain::smell::FindingConfidence::Medium,
        SourceLocation {
            file: file.path.clone(),
            line_start: line,
            line_end: line,
            column: None,
        },
        format!("Function `{ident}` has if/else nesting depth of {depth} (threshold: {threshold})"),
        "Use early returns, guard clauses, or extract nested conditions into helper functions.",
    )
}

struct IfDepthVisitor {
    current_depth: usize,
    max_depth: usize,
}

impl<'ast> Visit<'ast> for IfDepthVisitor {
    fn visit_item(&mut self, _: &'ast syn::Item) {}
    fn visit_expr_closure(&mut self, _: &'ast syn::ExprClosure) {}
    fn visit_expr_async(&mut self, _: &'ast syn::ExprAsync) {}
    fn visit_expr_const(&mut self, _: &'ast syn::ExprConst) {}

    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        self.current_depth += 1;
        if self.current_depth > self.max_depth {
            self.max_depth = self.current_depth;
        }

        self.visit_expr(&node.cond);
        self.visit_block(&node.then_branch);
        if let Some((_, alternative)) = &node.else_branch {
            // `else if` continues the same decision chain; an explicit else
            // block can still contain a truly nested conditional.
            if matches!(&**alternative, syn::Expr::If(_)) {
                self.current_depth -= 1;
                self.visit_expr(alternative);
                self.current_depth += 1;
            } else {
                self.visit_expr(alternative);
            }
        }
        self.current_depth -= 1;
    }
}
