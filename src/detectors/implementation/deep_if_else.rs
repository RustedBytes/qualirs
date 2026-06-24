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

        for item in &file.ast.items {
            if let syn::Item::Fn(fn_item) = item {
                let mut visitor = IfDepthVisitor {
                    current_depth: 0,
                    max_depth: 0,
                };
                visitor.visit_block(&fn_item.block);

                if visitor.max_depth > thresholds.r#impl.control_flow.deep_if_else {
                    let line = fn_item.sig.fn_token.span.start().line;

                    smells.push(if_depth_smell(
                        file,
                        &fn_item.sig.ident,
                        visitor.max_depth,
                        thresholds.r#impl.control_flow.deep_if_else,
                        line,
                    ));
                }
            }
        }

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
    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        self.current_depth += 1;
        if self.current_depth > self.max_depth {
            self.max_depth = self.current_depth;
        }

        // Visit the then-block (contains nested ifs)
        syn::visit::visit_expr_if(self, node);

        self.current_depth -= 1;
    }
}
