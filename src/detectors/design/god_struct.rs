use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects structs that carry too much state.
pub struct GodStructDetector;

impl Detector for GodStructDetector {
    fn name(&self) -> &str {
        "God Struct"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let threshold = crate::domain::config::current_thresholds()
            .design
            .god_struct_fields;
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Struct(strukt) = item {
                let field_count = match &strukt.fields {
                    syn::Fields::Named(fields) => fields.named.len(),
                    syn::Fields::Unnamed(fields) => fields.unnamed.len(),
                    syn::Fields::Unit => 0,
                };

                if field_count > threshold {
                    let line = strukt.struct_token.span.start().line;
                    smells.push(Smell::new(
                        SmellCategory::Design,
                        "God Struct",
                        Severity::Warning,
                        crate::domain::smell::FindingConfidence::Medium,
                        SourceLocation::new(file.path.clone(), line, line, None),
                        format!(
                            "Struct `{}` has {} fields (threshold: {})",
                            strukt.ident, field_count, threshold
                        ),
                        "Split unrelated state into focused structs or value objects.",
                    ));
                }
            }
        }

        smells
    }
}
