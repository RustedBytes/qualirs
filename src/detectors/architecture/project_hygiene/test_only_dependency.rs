use std::path::Path;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

use super::shared::find_upwards;

/// Detects dev-dependencies imported from production source files.
pub struct TestOnlyDependencyInProductionDetector;

impl Detector for TestOnlyDependencyInProductionDetector {
    fn name(&self) -> &str {
        "Test-only Dependency in Production"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        if is_test_path(&file.path) {
            return Vec::new();
        }
        let Some(manifest) = find_upwards(&file.path, "Cargo.toml") else {
            return Vec::new();
        };
        let dev_deps = dev_dependencies(&manifest);
        if dev_deps.is_empty() {
            return Vec::new();
        }

        let mut smells = Vec::new();
        for item in &file.ast.items {
            if let syn::Item::Use(use_item) = item {
                let used = use_tree_root(&use_item.tree);
                if dev_deps.contains(&used) {
                    let line = use_item.use_token.span.start().line;
                    smells.push(Smell::new(
                        SmellCategory::Architecture,
                        "Test-only Dependency in Production",
                        Severity::Warning,
                                                crate::domain::smell::FindingConfidence::Medium,
                        SourceLocation::new(file.path.clone(), line, line, None),
                        format!("Production source imports dev-dependency `{used}`"),
                        "Move the dependency to [dependencies] or keep the import under test-only cfg.",
                    ));
                }
            }
        }
        smells
    }
}

fn is_test_path(path: &Path) -> bool {
    path.components().any(|c| c.as_os_str() == "tests") || path.to_string_lossy().contains("_test")
}

fn dev_dependencies(manifest: &Path) -> std::collections::HashSet<String> {
    let Ok(content) = std::fs::read_to_string(manifest) else {
        return std::collections::HashSet::new();
    };
    let mut in_dev = false;
    let mut deps = std::collections::HashSet::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_dev = trimmed == "[dev-dependencies]";
            continue;
        }
        if in_dev && let Some((name, _)) = trimmed.split_once('=') {
            let name = name.trim().trim_matches('"').replace('-', "_");
            if !name.is_empty() {
                deps.insert(name);
            }
        }
    }
    deps
}

fn use_tree_root(tree: &syn::UseTree) -> String {
    match tree {
        syn::UseTree::Path(path) => path.ident.to_string().replace('-', "_"),
        syn::UseTree::Name(name) => name.ident.to_string().replace('-', "_"),
        syn::UseTree::Rename(rename) => rename.ident.to_string().replace('-', "_"),
        syn::UseTree::Group(group) => group.items.first().map(use_tree_root).unwrap_or_default(),
        syn::UseTree::Glob(_) => String::new(),
    }
}
