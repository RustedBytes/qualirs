use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects files that import from too many different external crates.
pub struct FeatureConcentrationDetector;

impl Detector for FeatureConcentrationDetector {
    fn name(&self) -> &str {
        "Feature Concentration"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        let mut crates = std::collections::HashSet::new();
        let mut visitor = UseCrateVisitor {
            crates: &mut crates,
        };
        visitor.visit_file(&file.ast);

        if crates.len() > thresholds.arch.feature_concentration {
            smells.push(Smell::new(
                SmellCategory::Architecture,
                "Feature Concentration",
                Severity::Warning,
                crate::domain::smell::FindingConfidence::Low,
                SourceLocation {
                    file: file.path.clone(),
                    line_start: 1,
                    line_end: file.line_count,
                    column: None,
                },
                format!(
                    "File imports from {} different crates (threshold: {})",
                    crates.len(),
                    thresholds.arch.feature_concentration
                ),
                "Split responsibilities across multiple modules to reduce coupling.",
            ));
        }

        smells
    }
}

struct UseCrateVisitor<'a> {
    crates: &'a mut std::collections::HashSet<String>,
}

impl<'ast> Visit<'ast> for UseCrateVisitor<'_> {
    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        collect_root_crates(&node.tree, self.crates);
    }
}

fn collect_root_crates(tree: &syn::UseTree, crates: &mut std::collections::HashSet<String>) {
    match tree {
        syn::UseTree::Path(path) => {
            // External crate: first segment is the crate name
            // Skip "self", "super", "crate" which are local
            let ident = path.ident.to_string();
            if !matches!(ident.as_str(), "self" | "super" | "crate") {
                crates.insert(ident);
            }
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                collect_root_crates(item, crates);
            }
        }
        syn::UseTree::Rename(rename) => {
            let ident = rename.ident.to_string();
            if !matches!(ident.as_str(), "self" | "super" | "crate") {
                crates.insert(ident);
            }
        }
        syn::UseTree::Name(name) => {
            let ident = name.ident.to_string();
            if !matches!(ident.as_str(), "self" | "super" | "crate") {
                crates.insert(ident);
            }
        }
        syn::UseTree::Glob(_) => {}
    }
}
