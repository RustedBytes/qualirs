use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects Leaky Error Abstractions.
///
/// If a public error type (e.g. `pub enum Error`) directly exposes an underlying
/// library's error type (like `reqwest::Error` or `sqlx::Error`), it couples
/// consumers to that specific library's types, leaking implementation details.
pub struct LeakyErrorAbstractionDetector;

impl Detector for LeakyErrorAbstractionDetector {
    fn name(&self) -> &str {
        "Leaky Error Abstraction"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        let known_leaky_crates = [
            "sqlx",
            "reqwest",
            "hyper",
            "serde_json",
            "tokio",
            "tungstenite",
            "redis",
        ];

        for item in &file.ast.items {
            if let syn::Item::Enum(e) = item {
                // Must be public and likely an error type
                if matches!(e.vis, syn::Visibility::Public(_))
                    && e.ident.to_string().ends_with("Error")
                {
                    for variant in &e.variants {
                        if let syn::Fields::Unnamed(fields) = &variant.fields {
                            for field in &fields.unnamed {
                                if let syn::Type::Path(tp) = &field.ty
                                    && let Some(first_seg) = tp.path.segments.first()
                                {
                                    let first_name = first_seg.ident.to_string();
                                    if known_leaky_crates.contains(&first_name.as_str()) {
                                        let start_line = variant.ident.span().start().line;
                                        smells.push(Smell::new(
                                                SmellCategory::Architecture,
                                                "Leaky Error Abstraction",
                                                Severity::Warning,
                                                                                                crate::domain::smell::FindingConfidence::Medium,
                                                SourceLocation::new(file.path.clone(), start_line, start_line, None),
                                                format!(
                                                    "Public enum `{}` contains variant `{}` wrapping `{}`",
                                                    e.ident, variant.ident, first_name
                                                ),
                                                "Do not expose underlying library errors in public domain interfaces. Wrap or map them to domain-specific variants.",
                                            ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        smells
    }
}
