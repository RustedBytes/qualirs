use crate::analysis::detector::Detector;
use crate::domain::config::Thresholds;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects functions, structs, and enums with too many generic parameters.
pub struct ExcessiveGenericsDetector;

impl Detector for ExcessiveGenericsDetector {
    fn name(&self) -> &str {
        "Excessive Generics"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        for item in &file.ast.items {
            match item {
                syn::Item::Fn(fn_item) => {
                    check_generics(
                        &fn_item.sig.generics,
                        &format!("Function `{}`", fn_item.sig.ident),
                        &file.path,
                        &thresholds,
                        &mut smells,
                    );
                }
                syn::Item::Struct(s) => {
                    check_generics(
                        &s.generics,
                        &format!("Struct `{}`", s.ident),
                        &file.path,
                        &thresholds,
                        &mut smells,
                    );
                }
                syn::Item::Enum(e) => {
                    check_generics(
                        &e.generics,
                        &format!("Enum `{}`", e.ident),
                        &file.path,
                        &thresholds,
                        &mut smells,
                    );
                }
                _ => {}
            }
        }

        smells
    }
}

fn check_generics(
    generics: &syn::Generics,
    context: &str,
    file_path: &std::path::Path,
    thresholds: &Thresholds,
    smells: &mut Vec<Smell>,
) {
    let count = generics.params.len();
    if count > thresholds.design.excessive_generics {
        report_excessive_generics(generics, count, context, file_path, thresholds, smells);
    }

    check_trait_bounds(generics, context, file_path, thresholds, smells);
}

fn report_excessive_generics(
    generics: &syn::Generics,
    count: usize,
    context: &str,
    file_path: &std::path::Path,
    thresholds: &Thresholds,
    smells: &mut Vec<Smell>,
) {
    let line = generics
        .lt_token
        .map(|lt| lt.span.start().line)
        .unwrap_or(1);

    smells.push(Smell::new(
        SmellCategory::Design,
        "Excessive Generics",
        Severity::Warning,
        crate::domain::smell::FindingConfidence::Medium,
        SourceLocation {
            file: file_path.to_path_buf(),
            line_start: line,
            line_end: line,
            column: None,
        },
        format!(
            "{context} has {count} generic parameters (threshold: {})",
            thresholds.design.excessive_generics
        ),
        "Reduce generic parameters. Consider concrete types or trait objects for complex cases.",
    ));
}

fn check_trait_bounds(
    generics: &syn::Generics,
    context: &str,
    file_path: &std::path::Path,
    thresholds: &Thresholds,
    smells: &mut Vec<Smell>,
) {
    for param in &generics.params {
        if let syn::GenericParam::Type(tp) = param {
            let bound_count = tp.bounds.len();
            if bound_count > thresholds.design.deep_trait_bounds {
                let line = tp.colon_token.map(|c| c.span.start().line).unwrap_or(1);

                smells.push(Smell::new(
                    SmellCategory::Design,
                    "Deep Trait Bounds",
                    Severity::Info,
                                        crate::domain::smell::FindingConfidence::Medium,
                    SourceLocation {
                        file: file_path.to_path_buf(),
                        line_start: line,
                        line_end: line,
                        column: None,
                    },
                    format!("{context}: type parameter `{}` has {bound_count} trait bounds (threshold: {})", tp.ident, thresholds.design.deep_trait_bounds),
                    "Consider creating a supertrait that combines common bounds.",
                ));
            }
        }
    }
}
