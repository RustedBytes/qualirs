use syn::visit::{Visit, visit_expr};

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

const EXCESSIVE_CLONE_THRESHOLD: usize = 10;

/// Detects functions with excessive `.clone()` calls.
pub struct ExcessiveCloneDetector;

impl Detector for ExcessiveCloneDetector {
    fn name(&self) -> &str {
        "Excessive Clone"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Fn(fn_item) = item {
                let mut visitor = CloneCounter::new();
                visitor.visit_block(&fn_item.block);

                if visitor.clone_count > EXCESSIVE_CLONE_THRESHOLD {
                    let line = fn_item.sig.fn_token.span.start().line;

                    smells.push(Smell::new(
                        SmellCategory::Performance,
                        "Excessive Clone",
                        Severity::Info,
                                                crate::domain::smell::FindingConfidence::Medium,
                        SourceLocation {
                            file: file.path.clone(),
                            line_start: line,
                            line_end: line,
                            column: None,
                        },
                        format!(
                            "Function `{}` has {} .clone() calls (threshold: {})",
                            fn_item.sig.ident, visitor.clone_count, EXCESSIVE_CLONE_THRESHOLD
                        ),
                        "Consider using references, lifetimes, or Rc/Arc to avoid excessive cloning.",
                    ));
                }
            }
        }

        smells
    }
}

struct CloneCounter {
    clone_count: usize,
}

impl CloneCounter {
    fn new() -> Self {
        Self { clone_count: 0 }
    }
}

impl<'ast> Visit<'ast> for CloneCounter {
    fn visit_expr(&mut self, expr: &'ast syn::Expr) {
        if let syn::Expr::MethodCall(call) = expr
            && call.method == "clone"
        {
            self.clone_count += 1;
        }
        visit_expr(self, expr);
    }
}
