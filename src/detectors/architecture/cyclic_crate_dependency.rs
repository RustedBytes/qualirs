use std::collections::HashSet;

use crate::analysis::detector::Detector;
use crate::domain::smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// A bounded coupling hint, not proof of a dependency cycle. Source-local use
/// paths cannot establish reciprocal crate dependencies. In particular, imports
/// from a module's own children are ordinary Rust and do not establish a cycle.
pub struct CyclicDependencyDetector;

impl Detector for CyclicDependencyDetector {
    fn name(&self) -> &str {
        "Cyclic Crate Dependency"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut roots = HashSet::new();
        for item in &file.ast.items {
            if let syn::Item::Use(item) = item
                && item.leading_colon.is_none()
            {
                collect_crate_roots(&item.tree, &mut roots);
            }
        }
        if roots.len() <= 5 {
            return Vec::new();
        }
        vec![Smell::new(
            SmellCategory::Architecture,
            self.name(),
            Severity::Warning,
            FindingConfidence::Low,
            SourceLocation::new(file.path.clone(), 1, file.code.lines().count(), None),
            format!(
                "File imports from {} crate-relative roots; reciprocal dependencies are unverified",
                roots.len()
            ),
            "Review the dependency graph for unnecessary coupling before considering a cycle-breaking refactor.",
        )]
    }
}

fn collect_crate_roots(tree: &syn::UseTree, roots: &mut HashSet<String>) {
    match tree {
        syn::UseTree::Path(path) if path.ident == "crate" => collect_roots(&path.tree, roots),
        syn::UseTree::Group(group) => {
            for tree in &group.items {
                collect_crate_roots(tree, roots);
            }
        }
        _ => {} // External, self/super and unresolved relative paths are not crate-root evidence.
    }
}

fn collect_roots(tree: &syn::UseTree, roots: &mut HashSet<String>) {
    match tree {
        syn::UseTree::Path(path) => {
            roots.insert(path.ident.to_string());
        }
        syn::UseTree::Group(group) => {
            for tree in &group.items {
                collect_roots(tree, roots);
            }
        }
        _ => {} // A direct imported item or glob does not identify a namespace.
    }
}
