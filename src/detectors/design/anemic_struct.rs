use crate::analysis::detector::Detector;
use crate::detectors::policy::{is_dto_template_or_config_struct, is_test_path};
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects structs that have fields but no associated methods (impl blocks).
///
/// This often indicates data-only types with logic scattered elsewhere.
pub struct AnemicStructDetector;

impl Detector for AnemicStructDetector {
    fn name(&self) -> &str {
        "Anemic Struct"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        if is_test_path(&file.path) {
            return smells;
        }

        // Collect struct names that have at least one field and no derive macros
        let structs_with_fields: Vec<&syn::ItemStruct> = file
            .ast
            .items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Struct(s) => {
                    let has_fields = has_fields(s);
                    let has_derive = s.attrs.iter().any(|attr| attr.path().is_ident("derive"));
                    (has_fields && !has_derive && !is_dto_template_or_config_struct(s)).then_some(s)
                }
                _ => None,
            })
            .collect();

        if structs_with_fields.is_empty() {
            return smells;
        }

        // Collect struct names that have impl blocks (inherent or trait) in this file
        let impl_targets: Vec<&syn::Ident> = file
            .ast
            .items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Impl(imp) => extract_type_ident(&imp.self_ty),
                _ => None,
            })
            .collect();

        for s in &structs_with_fields {
            let has_impl = impl_targets.contains(&&s.ident);
            if !has_impl {
                let line = line_of_struct(s);

                smells.push(Smell::new(
                    SmellCategory::Design,
                    "Anemic Struct",
                    Severity::Info,
                                        crate::domain::smell::FindingConfidence::Low,
                    SourceLocation {
                        file: file.path.clone(),
                        line_start: line,
                        line_end: line,
                        column: None,
                    },
                    format!("Struct `{}` has fields but no impl block in this file", s.ident),
                    "Consider adding behavior to the struct or convert to a value object if data-only is intentional.",
                ));
            }
        }

        smells
    }
}

fn has_fields(s: &syn::ItemStruct) -> bool {
    match &s.fields {
        syn::Fields::Named(f) => !f.named.is_empty(),
        syn::Fields::Unnamed(f) => !f.unnamed.is_empty(),
        syn::Fields::Unit => false,
    }
}

fn extract_type_ident(ty: &syn::Type) -> Option<&syn::Ident> {
    if let syn::Type::Path(tp) = ty {
        tp.path.segments.first().map(|s| &s.ident)
    } else {
        None
    }
}

fn line_of_struct(s: &syn::ItemStruct) -> usize {
    match &s.fields {
        syn::Fields::Named(f) => f.brace_token.span.open().start().line,
        syn::Fields::Unnamed(f) => f.paren_token.span.open().start().line,
        syn::Fields::Unit => s.struct_token.span.start().line,
    }
}
