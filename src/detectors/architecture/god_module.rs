use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects modules that are too large (too many lines or too many items).
pub struct GodModuleDetector;

impl Detector for GodModuleDetector {
    fn name(&self) -> &str {
        "God Module"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        if file.path.to_string_lossy().contains("tests") {
            return smells;
        }

        let item_count = file
            .ast
            .items
            .iter()
            .filter(|item| count_item(item))
            .count();
        let is_module_registry = item_count > 0
            && file
                .ast
                .items
                .iter()
                .filter(|item| count_item(item))
                .all(|item| matches!(item, syn::Item::Mod(_)));

        if file.line_count > thresholds.arch.god_module_loc {
            smells.push(Smell::new(
                SmellCategory::Architecture,
                "God Module",
                Severity::Warning,
                crate::domain::smell::FindingConfidence::Medium,
                SourceLocation {
                    file: file.path.clone(),
                    line_start: 1,
                    line_end: file.line_count,
                    column: None,
                },
                format!(
                    "Module has {} lines (threshold: {})",
                    file.line_count, thresholds.arch.god_module_loc
                ),
                "Split this module into smaller, focused modules with clear responsibilities.",
            ));
        }

        if item_count > thresholds.arch.god_module_items && !is_module_registry {
            smells.push(Smell::new(
                SmellCategory::Architecture,
                "God Module (items)",
                Severity::Warning,
                crate::domain::smell::FindingConfidence::Medium,
                SourceLocation {
                    file: file.path.clone(),
                    line_start: 1,
                    line_end: file.line_count,
                    column: None,
                },
                format!(
                    "Module has {} top-level items (threshold: {})",
                    item_count, thresholds.arch.god_module_items
                ),
                "Decompose into multiple modules, each with a single responsibility.",
            ));
        }

        smells
    }
}

fn count_item(item: &syn::Item) -> bool {
    !matches!(item, syn::Item::Use(_)) && !is_test_item(item)
}

fn is_test_item(item: &syn::Item) -> bool {
    let syn::Item::Mod(module) = item else {
        return false;
    };

    module
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("cfg") && attr.meta.require_list().is_ok_and(is_test_cfg))
}

fn is_test_cfg(list: &syn::MetaList) -> bool {
    list.tokens.to_string().contains("test")
}
