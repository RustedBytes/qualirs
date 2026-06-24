use crate::analysis::detector::Detector;
use crate::domain::config::Thresholds;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects functions that are too long (by lines of code).
pub struct LongFunctionDetector;

impl Detector for LongFunctionDetector {
    fn name(&self) -> &str {
        "Long Function"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        if file.path.to_string_lossy().contains("tests") {
            return smells;
        }

        for item in &file.ast.items {
            if let syn::Item::Fn(fn_item) = item {
                check_function(&file.path, fn_item, &thresholds, &mut smells);
            }
        }

        smells
    }
}

fn check_function(
    file_path: &std::path::Path,
    fn_item: &syn::ItemFn,
    thresholds: &Thresholds,
    smells: &mut Vec<Smell>,
) {
    let brace_open = fn_item.block.brace_token.span.open();
    let start_line = brace_open.start().line;

    let brace_close = fn_item.block.brace_token.span.close();
    let end_line = brace_close.start().line;

    let loc_raw = end_line.saturating_sub(start_line);
    let loc = loc_raw.saturating_sub(1);

    if loc > thresholds.r#impl.control_flow.long_function_loc {
        smells.push(Smell::new(
            SmellCategory::Implementation,
            "Long Function",
            if loc > thresholds.r#impl.control_flow.long_function_loc * 2 {
                Severity::Critical
            } else {
                Severity::Warning
            },
            crate::domain::smell::FindingConfidence::Medium,
            SourceLocation {
                file: file_path.to_path_buf(),
                line_start: start_line,
                line_end: end_line,
                column: None,
            },
            format!(
                "Function `{}` is ~{} lines long (threshold: {})",
                fn_item.sig.ident, loc, thresholds.r#impl.control_flow.long_function_loc
            ),
            "Extract helper functions. Each function should do one thing.",
        ));
    }
}
