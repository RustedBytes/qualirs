use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects functions with too many parameters.
pub struct TooManyArgumentsDetector;

impl Detector for TooManyArgumentsDetector {
    fn name(&self) -> &str {
        "Too Many Arguments"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Fn(fn_item) = item {
                let arg_count = fn_item.sig.inputs.len();
                if arg_count > thresholds.r#impl.control_flow.too_many_arguments {
                    let line = fn_item.sig.fn_token.span.start().line;

                    smells.push(Smell::new(
                        SmellCategory::Implementation,
                        "Too Many Arguments",
                        Severity::Warning,
                        crate::domain::smell::FindingConfidence::Medium,
                        SourceLocation {
                            file: file.path.clone(),
                            line_start: line,
                            line_end: line,
                            column: None,
                        },
                        format!(
                            "Function `{}` has {} arguments (threshold: {})",
                            fn_item.sig.ident,
                            arg_count,
                            thresholds.r#impl.control_flow.too_many_arguments
                        ),
                        "Group related parameters into a struct or use the Builder pattern.",
                    ));
                }
            }
        }

        smells
    }
}
