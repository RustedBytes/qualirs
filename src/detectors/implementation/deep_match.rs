use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects deeply nested match expressions.
pub struct DeepMatchDetector;

impl Detector for DeepMatchDetector {
    fn name(&self) -> &str {
        "Deep Match Nesting"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Fn(fn_item) = item {
                let mut visitor = MatchDepthVisitor::new();
                visitor.visit_block(&fn_item.block);

                if visitor.max_depth > thresholds.r#impl.control_flow.deep_match_nesting {
                    let line = fn_item.sig.fn_token.span.start().line;

                    smells.push(match_depth_smell(
                        file,
                        &fn_item.sig.ident,
                        visitor.max_depth,
                        thresholds.r#impl.control_flow.deep_match_nesting,
                        line,
                    ));
                }
            }
        }

        smells
    }
}

fn match_depth_smell(
    file: &SourceFile,
    ident: &syn::Ident,
    depth: usize,
    threshold: usize,
    line: usize,
) -> Smell {
    Smell::new(
        SmellCategory::Implementation,
        "Deep Match Nesting",
        Severity::Warning,
        crate::domain::smell::FindingConfidence::Medium,
        SourceLocation {
            file: file.path.clone(),
            line_start: line,
            line_end: line,
            column: None,
        },
        format!("Function `{ident}` has match nesting depth of {depth} (threshold: {threshold})"),
        "Flatten nested matches using early returns, combinators, or extract helper functions.",
    )
}

struct MatchDepthVisitor {
    current_depth: usize,
    max_depth: usize,
}

impl MatchDepthVisitor {
    fn new() -> Self {
        Self {
            current_depth: 0,
            max_depth: 0,
        }
    }
}

impl<'ast> Visit<'ast> for MatchDepthVisitor {
    fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
        self.current_depth += 1;
        if self.current_depth > self.max_depth {
            self.max_depth = self.current_depth;
        }
        syn::visit::visit_expr_match(self, node);
        self.current_depth -= 1;
    }
}
