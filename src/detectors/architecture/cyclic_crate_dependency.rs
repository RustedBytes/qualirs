use std::collections::HashSet;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects circular `use` dependencies between modules within the same crate.
///
/// Tracks `use crate::module_a::...` and `use crate::module_b::...` to build
/// a local dependency graph and find cycles.
pub struct CyclicDependencyDetector;

impl Detector for CyclicDependencyDetector {
    fn name(&self) -> &str {
        "Cyclic Crate Dependency"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        // Determine this file's module identity from its path
        let module_name = file_to_module(&file.path);
        let deps = collect_crate_deps(&file.ast);

        // If this module has few deps, no cycle risk on its own
        if deps.len() < 2 {
            return smells;
        }

        // Check for self-referential or obviously cyclic patterns
        // A file that imports module A and is itself imported by module A
        // can only be detected cross-file. Here we detect obvious patterns:
        // files that import each other's module paths
        if deps
            .iter()
            .any(|dep| is_same_module_or_child(dep, &module_name))
        {
            smells.push(Smell::new(
                SmellCategory::Architecture,
                "Cyclic Crate Dependency",
                Severity::Critical,
                crate::domain::smell::FindingConfidence::Medium,
                SourceLocation {
                    file: file.path.clone(),
                    line_start: 1,
                    line_end: file.line_count,
                    column: None,
                },
                format!(
                    "Module `{}` imports from itself (self-referential dependency)",
                    module_name
                ),
                "Remove the self-referential import and restructure the module.",
            ));
        }

        // Detect bidirectional deps: if this file has many crate-internal deps
        // it's a cycle risk indicator
        let internal_deps: HashSet<String> = deps
            .iter()
            .filter_map(|dep| dep.split("::").next())
            .filter(|root| !root.contains('_'))
            .map(str::to_string)
            .collect();

        if internal_deps.len() > 5 {
            smells.push(Smell::new(
                SmellCategory::Architecture,
                "Cyclic Crate Dependency",
                Severity::Warning,
                                crate::domain::smell::FindingConfidence::Medium,
                SourceLocation {
                    file: file.path.clone(),
                    line_start: 1,
                    line_end: file.line_count,
                    column: None,
                },
                format!(
                    "Module `{}` has {} internal dependencies — high cycle risk",
                    module_name, internal_deps.len()
                ),
                "Reduce internal coupling. Consider extracting shared logic into a separate module.",
            ));
        }

        smells
    }
}

fn is_same_module_or_child(dep: &str, module_name: &str) -> bool {
    dep == module_name
        || dep
            .strip_prefix(module_name)
            .is_some_and(|rest| rest.starts_with("::"))
}

fn file_to_module(path: &std::path::Path) -> String {
    let mut parts = module_path_parts(path);

    if parts.is_empty() {
        return file_stem_or_unknown(path).to_string();
    }

    if parts.last().is_some_and(|part| part == "mod") {
        parts.pop();
    }

    parts.join("::")
}

fn module_path_parts(path: &std::path::Path) -> Vec<String> {
    let mut parts = Vec::new();
    let mut in_src = false;

    for component in path.components() {
        if !in_src {
            in_src = component.as_os_str() == "src";
            continue;
        }

        if let Some(component) = component.as_os_str().to_str() {
            parts.push(component.trim_end_matches(".rs").to_string());
        }
    }

    parts
}

fn file_stem_or_unknown(path: &std::path::Path) -> &str {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("unknown")
}

fn collect_crate_deps(ast: &syn::File) -> HashSet<String> {
    let mut deps = HashSet::new();
    for item in &ast.items {
        if let syn::Item::Use(use_item) = item {
            if is_crate_internal_use(&use_item.tree) {
                if let Some(module) = extract_crate_module_path(&use_item.tree) {
                    deps.insert(module);
                }
            } else {
                // External use
                let root = extract_root_ident(&use_item.tree);
                if let Some(r) = root {
                    deps.insert(r);
                }
            }
        }
    }
    deps
}

fn is_crate_internal_use(tree: &syn::UseTree) -> bool {
    match tree {
        syn::UseTree::Path(p) => p.ident == "crate" || p.ident == "self" || p.ident == "super",
        _ => false,
    }
}

fn extract_crate_module_path(tree: &syn::UseTree) -> Option<String> {
    match tree {
        syn::UseTree::Path(p) => {
            if p.ident == "crate" {
                extract_module_path(&p.tree)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn extract_module_path(tree: &syn::UseTree) -> Option<String> {
    match tree {
        syn::UseTree::Path(path) => {
            let mut parts = vec![path.ident.to_string()];
            if let Some(rest) = extract_module_path(&path.tree) {
                parts.push(rest);
            }
            Some(parts.join("::"))
        }
        syn::UseTree::Name(name) => Some(name.ident.to_string()),
        syn::UseTree::Rename(rename) => Some(rename.ident.to_string()),
        syn::UseTree::Group(group) => group.items.first().and_then(extract_module_path),
        syn::UseTree::Glob(_) => None,
    }
}

fn extract_root_ident(tree: &syn::UseTree) -> Option<String> {
    match tree {
        syn::UseTree::Path(p) => Some(p.ident.to_string()),
        syn::UseTree::Name(n) => Some(n.ident.to_string()),
        syn::UseTree::Rename(r) => Some(r.ident.to_string()),
        syn::UseTree::Group(g) => g.items.first().and_then(extract_root_ident),
        syn::UseTree::Glob(_) => None,
    }
}
