use syn::spanned::Spanned;
use syn::visit::{Visit, visit_expr_closure};

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects large closures that hide function-sized logic.
pub struct LongClosureDetector;

impl Detector for LongClosureDetector {
    fn name(&self) -> &str {
        "Long Closure"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let threshold = crate::domain::config::current_thresholds()
            .r#impl
            .control_flow
            .long_closure_loc;
        let mut visitor = LongClosureVisitor {
            threshold,
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|(line, loc)| {
                Smell::new(
                    SmellCategory::Implementation,
                    "Long Closure",
                    Severity::Info,
                    crate::domain::smell::FindingConfidence::Medium,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    format!("Closure spans {loc} lines (threshold: {threshold})"),
                    "Extract complex closure bodies into named functions.",
                )
            })
            .collect()
    }
}

struct LongClosureVisitor {
    threshold: usize,
    findings: Vec<(usize, usize)>,
}

impl<'ast> Visit<'ast> for LongClosureVisitor {
    fn visit_expr_closure(&mut self, node: &'ast syn::ExprClosure) {
        let start = node.span().start().line;
        let end = node.span().end().line;
        let loc = end.saturating_sub(start) + 1;
        if loc > self.threshold {
            self.findings.push((start, loc));
        }
        visit_expr_closure(self, node);
    }
}
