use crate::analysis::detector::Detector;
use crate::detectors::policy::{is_dto_template_or_config_struct, is_test_path};
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects structs where all fields are primitive types and the field count is high.
///
/// This indicates Primitive Obsession. Domain concepts should be modeled with Newtypes
/// to provide type safety and encapsulate behavior.
pub struct PrimitiveObsessionDetector;

impl Detector for PrimitiveObsessionDetector {
    fn name(&self) -> &str {
        "Primitive Obsession"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        if is_test_path(&file.path) {
            return smells;
        }

        for item in &file.ast.items {
            if let syn::Item::Struct(s) = item {
                // Ignore configuration structures which are naturally composed of primitives
                if s.ident.to_string().ends_with("Thresholds")
                    || is_dto_template_or_config_struct(s)
                {
                    continue;
                }

                let field_count = match &s.fields {
                    syn::Fields::Named(f) => f.named.len(),
                    syn::Fields::Unnamed(f) => f.unnamed.len(),
                    syn::Fields::Unit => 0,
                };

                if field_count > thresholds.design.primitive_obsession_fields {
                    let mut all_primitive = true;
                    let iter: Box<dyn Iterator<Item = &syn::Field>> = match &s.fields {
                        syn::Fields::Named(f) => Box::new(f.named.iter()),
                        syn::Fields::Unnamed(f) => Box::new(f.unnamed.iter()),
                        syn::Fields::Unit => Box::new(std::iter::empty()),
                    };

                    for field in iter {
                        if !is_primitive(&field.ty) {
                            all_primitive = false;
                            break;
                        }
                    }

                    if all_primitive {
                        let start_line = s.ident.span().start().line;
                        smells.push(Smell::new(
                            SmellCategory::Design,
                            "Primitive Obsession",
                            Severity::Info,
                                                        crate::domain::smell::FindingConfidence::Low,
                            SourceLocation::new(file.path.clone(), start_line, start_line, None),
                            format!(
                                "Struct `{}` has {} fields, and all of them are primitive types (threshold: {})",
                                s.ident, field_count, thresholds.design.primitive_obsession_fields
                            ),
                            "Introduce Custom Types / Newtypes to encapsulate domain concepts and add type safety.",
                        ));
                    }
                }
            }
        }

        smells
    }
}

fn is_primitive(ty: &syn::Type) -> bool {
    if let syn::Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            let ident = seg.ident.to_string();
            return matches!(
                ident.as_str(),
                "i8" | "i16"
                    | "i32"
                    | "i64"
                    | "i128"
                    | "isize"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "u64"
                    | "u128"
                    | "usize"
                    | "f32"
                    | "f64"
                    | "bool"
                    | "char"
                    | "String"
                    | "str"
            );
        }
    } else if let syn::Type::Reference(tr) = ty {
        return is_primitive(&tr.elem);
    }
    false
}
