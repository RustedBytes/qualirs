use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects `impl` blocks with too many methods.
///
/// Structs with excessive numbers of methods often violate the Single Responsibility Principle,
/// becoming God Objects that are hard to maintain and test.
pub struct FatImplDetector;

impl Detector for FatImplDetector {
    fn name(&self) -> &str {
        "Fat Impl (God Object)"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Impl(imp) = item {
                // Focus on inherent impls (no trait)
                if imp.trait_.is_none() {
                    let method_count = imp
                        .items
                        .iter()
                        .filter(|i| matches!(i, syn::ImplItem::Fn(_)))
                        .count();

                    if method_count > thresholds.design.fat_impl_methods {
                        let type_name = if let syn::Type::Path(tp) = &*imp.self_ty {
                            tp.path
                                .segments
                                .last()
                                .map(|s| s.ident.to_string())
                                .unwrap_or_else(|| "Unknown".to_string())
                        } else {
                            "Unknown".to_string()
                        };

                        let start_line = imp.impl_token.span.start().line;

                        smells.push(Smell::new(
                            SmellCategory::Design,
                            "Fat Impl (God Object)",
                            Severity::Warning,
                            crate::domain::smell::FindingConfidence::Medium,
                            SourceLocation::new(file.path.clone(), start_line, start_line, None),
                            format!(
                                "Struct `{}` has {} methods in a single impl block (threshold: {})",
                                type_name, method_count, thresholds.design.fat_impl_methods
                            ),
                            "Consider breaking the struct into smaller, more focused components.",
                        ));
                    }
                }
            }
        }

        smells
    }
}
