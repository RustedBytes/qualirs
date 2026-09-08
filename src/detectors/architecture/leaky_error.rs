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

        for item in &file.ast.items {
            let syn::Item::Enum(e) = item else {
                continue;
            };
            if !matches!(e.vis, syn::Visibility::Public(_))
                || !e.ident.to_string().ends_with("Error")
            {
                continue;
            }
            for variant in &e.variants {
                let syn::Fields::Unnamed(fields) = &variant.fields else {
                    continue;
                };
                for field in &fields.unnamed {
                    if let Some(crate_name) = exposed_crate(field) {
                        smells.push(exposed_error(file, e, variant, &crate_name));
                    }
                }
            }
        }

        smells
    }
}

fn exposed_crate(field: &syn::Field) -> Option<String> {
    let syn::Type::Path(tp) = &field.ty else {
        return None;
    };
    let name = tp.path.segments.first()?.ident.to_string();
    [
        "sqlx",
        "reqwest",
        "hyper",
        "serde_json",
        "tokio",
        "tungstenite",
        "redis",
    ]
    .contains(&name.as_str())
    .then_some(name)
}

fn exposed_error(
    file: &SourceFile,
    e: &syn::ItemEnum,
    variant: &syn::Variant,
    crate_name: &str,
) -> Smell {
    let line = variant.ident.span().start().line;
    Smell::new(
        SmellCategory::Architecture,
        "Leaky Error Abstraction",
        Severity::Warning,
        crate::domain::smell::FindingConfidence::Medium,
        SourceLocation::new(file.path.clone(), line, line, None),
        format!(
            "Public enum `{}` contains variant `{}` wrapping `{}`",
            e.ident, variant.ident, crate_name
        ),
        "Do not expose underlying library errors in public domain interfaces. Wrap or map them to domain-specific variants.",
    )
}
