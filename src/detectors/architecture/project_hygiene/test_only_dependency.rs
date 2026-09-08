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
                    let entry = is_crate_entry(&file.path, &manifest);
                    smells.push(Smell::new(
                        SmellCategory::Architecture,
                        "Test-only Dependency in Production",
                        Severity::Warning,
                        if entry { crate::domain::smell::FindingConfidence::Medium }
                        else { crate::domain::smell::FindingConfidence::Low },
                        SourceLocation::new(file.path.clone(), line, line, None),
                        if entry { format!("A crate entry point imports dev-only dependency `{used}`") }
                        else { format!("Source imports dev-only dependency `{used}`; its production build context is unresolved") },
                        "Check whether this source is built for production or only included by tests or documentation before changing dependency sections.",
                    ));
                }
            }
        }
        smells
    }
}

fn is_test_path(path: &Path) -> bool {
    crate::detectors::policy::is_test_path(path)
}

fn dev_dependencies(manifest: &Path) -> std::collections::HashSet<String> {
    let Ok(content) = std::fs::read_to_string(manifest) else {
        return std::collections::HashSet::new();
    };
    let Ok(manifest) = toml::from_str::<toml::Value>(&content) else {
        return std::collections::HashSet::new();
    };
    let mut dev = std::collections::HashSet::new();
    let mut production = std::collections::HashSet::new();
    collect_dependencies(&manifest, &mut dev, &mut production);
    if let Some(targets) = manifest.get("target").and_then(toml::Value::as_table) {
        for target in targets.values() {
            collect_dependencies(target, &mut dev, &mut production);
        }
    }
    // A dependency may be optional, renamed, inherited from the workspace, or
    // available only on a target. Its presence as a dev dependency is not proof
    // that production imports are invalid.
    dev.retain(|name| !production.contains(name));
    dev
}

fn is_crate_entry(file: &Path, manifest: &Path) -> bool {
    let Some(root) = manifest.parent() else {
        return false;
    };
    if ["src/lib.rs", "src/main.rs"]
        .iter()
        .any(|path| file == root.join(path))
    {
        return true;
    }
    let Some(value) = std::fs::read_to_string(manifest)
        .ok()
        .and_then(|text| toml::from_str::<toml::Value>(&text).ok())
    else {
        return false;
    };
    value
        .get("lib")
        .and_then(|lib| lib.get("path"))
        .and_then(toml::Value::as_str)
        .is_some_and(|path| file == root.join(path))
        || value
            .get("bin")
            .and_then(toml::Value::as_array)
            .is_some_and(|bins| {
                bins.iter().any(|bin| {
                    bin.get("path")
                        .and_then(toml::Value::as_str)
                        .is_some_and(|path| file == root.join(path))
                })
            })
}

fn collect_dependencies(
    table: &toml::Value,
    dev: &mut std::collections::HashSet<String>,
    production: &mut std::collections::HashSet<String>,
) {
    for (section, names) in [("dev-dependencies", dev), ("dependencies", production)] {
        if let Some(deps) = table.get(section).and_then(toml::Value::as_table) {
            names.extend(deps.keys().map(|name| name.replace('-', "_")));
        }
    }
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
