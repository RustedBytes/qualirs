use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects domain concepts represented as bare strings.
pub struct StringlyTypedDomainDetector;

impl Detector for StringlyTypedDomainDetector {
    fn name(&self) -> &str {
        "Stringly Typed Domain"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let threshold = crate::domain::config::current_thresholds()
            .design
            .stringly_typed_fields;
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Struct(strukt) = item {
                let names: Vec<String> = match &strukt.fields {
                    syn::Fields::Named(fields) => fields
                        .named
                        .iter()
                        .filter(|field| is_string_like(&field.ty))
                        .filter_map(|field| field.ident.as_ref().map(|i| i.to_string()))
                        .filter(|name| is_domain_name(name))
                        .collect(),
                    _ => Vec::new(),
                };

                if names.len() >= threshold {
                    let line = strukt.struct_token.span.start().line;
                    smells.push(Smell::new(
                        SmellCategory::Design,
                        "Stringly Typed Domain",
                        Severity::Info,
                                                crate::domain::smell::FindingConfidence::Low,
                        SourceLocation::new(file.path.clone(), line, line, None),
                        format!(
                            "Struct `{}` has stringly typed domain fields: {}",
                            strukt.ident,
                            names.join(", ")
                        ),
                        "Introduce newtypes or enums for domain values such as ids, status, roles, and codes.",
                    ));
                }
            }
        }

        smells
    }
}

fn is_string_like(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => {
            let segment = path.path.segments.last();
            segment.map(|seg| seg.ident == "String").unwrap_or(false)
        }
        syn::Type::Reference(reference) => {
            matches!(&*reference.elem, syn::Type::Path(path) if path.path.is_ident("str"))
        }
        _ => false,
    }
}

fn is_domain_name(name: &str) -> bool {
    const PARTS: &[&str] = &[
        "id", "email", "status", "state", "role", "kind", "type", "code", "currency", "country",
        "locale", "token",
    ];
    let lower = name.to_lowercase();
    PARTS.iter().any(|part| {
        lower == *part
            || lower.ends_with(&format!("_{part}"))
            || lower.contains(&format!("_{part}_"))
    })
}
