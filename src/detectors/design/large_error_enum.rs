use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects broad error enums with too many variants.
pub struct LargeErrorEnumDetector;

impl Detector for LargeErrorEnumDetector {
    fn name(&self) -> &str {
        "Large Error Enum"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let threshold = crate::domain::config::current_thresholds()
            .design
            .large_error_enum_variants;
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Enum(enm) = item
                && enm.ident.to_string().ends_with("Error")
                && enm.variants.len() > threshold
            {
                let line = enm.enum_token.span.start().line;
                smells.push(Smell::new(
                        SmellCategory::Design,
                        "Large Error Enum",
                        Severity::Warning,
                                                crate::domain::smell::FindingConfidence::Medium,
                        SourceLocation::new(file.path.clone(), line, line, None),
                        format!(
                            "Error enum `{}` has {} variants (threshold: {})",
                            enm.ident,
                            enm.variants.len(),
                            threshold
                        ),
                        "Split broad errors by layer or use nested source errors behind stable public variants.",
                    ));
            }
        }

        smells
    }
}
