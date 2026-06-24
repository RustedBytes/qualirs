use syn::ItemTrait;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects traits with too many methods.
pub struct LargeTraitDetector;

impl Detector for LargeTraitDetector {
    fn name(&self) -> &str {
        "Large Trait"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Trait(item_trait) = item {
                let method_count = count_trait_methods(item_trait);
                if method_count > thresholds.design.large_trait_methods {
                    let line = item_trait.brace_token.span.open().start().line;

                    smells.push(Smell::new(
                        SmellCategory::Design,
                        "Large Trait",
                        Severity::Warning,
                                                crate::domain::smell::FindingConfidence::Medium,
                        SourceLocation {
                            file: file.path.clone(),
                            line_start: line,
                            line_end: line,
                            column: None,
                        },
                        format!(
                            "Trait `{}` has {} methods (threshold: {})",
                            item_trait.ident, method_count, thresholds.design.large_trait_methods
                        ),
                        "Split the trait into smaller, focused traits (Interface Segregation Principle).",
                    ));
                }
            }
        }

        smells
    }
}

fn count_trait_methods(item_trait: &ItemTrait) -> usize {
    item_trait
        .items
        .iter()
        .filter(|ti| matches!(ti, syn::TraitItem::Fn(_)))
        .count()
}
