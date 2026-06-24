use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects modules that depend heavily on unstable/internal/experimental APIs.
///
/// Flags imports from paths containing `internal`, `unstable`, `experimental`,
/// `nightly`, or `_sys` which indicate reliance on non-stable interfaces.
pub struct UnstableDependencyDetector;

impl Detector for UnstableDependencyDetector {
    fn name(&self) -> &str {
        "Unstable Dependency"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        let mut visitor = UnstableUseVisitor {
            unstable_imports: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        if !visitor.unstable_imports.is_empty() {
            let ratio = visitor.unstable_imports.len() as f64 / file.ast.items.len().max(1) as f64;

            if ratio > 0.4 {
                let line = visitor.unstable_imports[0].1;
                let names: Vec<String> = visitor
                    .unstable_imports
                    .iter()
                    .map(|(name, _)| name.clone())
                    .collect();

                smells.push(Smell::new(
                    SmellCategory::Architecture,
                    "Unstable Dependency",
                    Severity::Warning,
                    crate::domain::smell::FindingConfidence::Medium,
                    SourceLocation {
                        file: file.path.clone(),
                        line_start: line,
                        line_end: line,
                        column: None,
                    },
                    format!(
                        "File has {} unstable imports ({:.0}% of items): {}",
                        visitor.unstable_imports.len(),
                        ratio * 100.0,
                        names.join(", ")
                    ),
                    "Stabilize your dependencies. Wrap unstable APIs behind your own abstractions.",
                ));
            }
        }

        smells
    }
}

struct UnstableUseVisitor {
    unstable_imports: Vec<(String, usize)>,
}

impl<'ast> Visit<'ast> for UnstableUseVisitor {
    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        let path = use_tree_to_string(&node.tree).to_lowercase();
        let is_unstable = path.contains("internal")
            || path.contains("unstable")
            || path.contains("experimental")
            || path.contains("nightly")
            || path.contains("_sys")
            || path.contains("raw")
            || path.contains("private");

        if is_unstable {
            let line = node.use_token.span.start().line;
            self.unstable_imports
                .push((use_tree_to_string(&node.tree), line));
        }
    }
}

fn use_tree_to_string(tree: &syn::UseTree) -> String {
    match tree {
        syn::UseTree::Path(p) => format!("{}::{}", p.ident, use_tree_to_string(&p.tree)),
        syn::UseTree::Name(n) => n.ident.to_string(),
        syn::UseTree::Rename(r) => format!("{} as {}", r.ident, r.rename),
        syn::UseTree::Glob(_) => "*".to_string(),
        syn::UseTree::Group(g) => {
            let items: Vec<String> = g.items.iter().map(use_tree_to_string).collect();
            format!("{{{}}}", items.join(", "))
        }
    }
}
