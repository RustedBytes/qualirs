use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects async functions with very large bodies (large futures).
///
/// Large futures cause stack issues and slow compilation.
pub struct LargeFutureDetector;

impl Detector for LargeFutureDetector {
    fn name(&self) -> &str {
        "Large Future"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Fn(fn_item) = item
                && fn_item.sig.asyncness.is_some()
            {
                let start = fn_item.block.brace_token.span.open().start().line;
                let end = fn_item.block.brace_token.span.close().start().line;
                let loc = end.saturating_sub(start).saturating_sub(1);

                if loc > thresholds.concurrency.large_future_loc {
                    smells.push(Smell::new(
                        SmellCategory::Performance,
                        "Large Future",
                        if loc > thresholds.concurrency.large_future_loc * 2 {
                            Severity::Critical
                        } else {
                            Severity::Warning
                        },
                        crate::domain::smell::FindingConfidence::Medium,
                        SourceLocation {
                            file: file.path.clone(),
                            line_start: start,
                            line_end: end,
                            column: None,
                        },
                        format!(
                            "Async function `{}` is ~{} lines (threshold: {})",
                            fn_item.sig.ident, loc, thresholds.concurrency.large_future_loc
                        ),
                        "Break large async functions into smaller composable futures.",
                    ));
                }
            }
        }

        smells
    }
}
