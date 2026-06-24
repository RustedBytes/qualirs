use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects layer violations where a domain module depends on infrastructure.
///
/// Convention: files under `domain/` or with "domain" in path should NOT
/// import from `infra/`, `infrastructure/`, `io/`, `db/`, `http/`, `cli/`.
pub struct LayerViolationDetector;

impl Detector for LayerViolationDetector {
    fn name(&self) -> &str {
        "Layer Violation"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();
        let path_str = file.path.to_string_lossy().to_string();

        // Determine which layer this file belongs to
        let layer = determine_layer(&path_str);

        // Only check domain and application layers for violations
        if !matches!(layer, Layer::Domain | Layer::Application) {
            return smells;
        }

        let forbidden = match layer {
            Layer::Domain => &[
                "infra",
                "infrastructure",
                "io",
                "db",
                "http",
                "cli",
                "presentation",
                "transport",
                "network",
                "filesystem",
                "sql",
                "redis",
                "kafka",
            ][..],
            Layer::Application => &["cli", "presentation", "http", "transport"][..],
            _ => &[],
        };

        for item in &file.ast.items {
            if let syn::Item::Use(use_item) = item {
                let use_path = use_tree_to_string(&use_item.tree).to_lowercase();

                // Split path into segments, ignoring punctuation like {, }, and *
                let segments: Vec<String> = use_path
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();

                for &forbidden_mod in forbidden {
                    if let Some(idx) = segments.iter().position(|s| s == forbidden_mod) {
                        // Check if this is a domestic/safe path like `crate::domain::io`
                        // We only flag if it's the root of the path OR preceded by std/core/crate/super
                        let is_violation = if idx == 0 {
                            true
                        } else {
                            let prev = &segments[idx - 1];
                            // If it follows a domain-related segment, it's likely a domestic abstraction
                            !matches!(prev.as_str(), "domain" | "model" | "entity" | "logic")
                        };

                        if is_violation {
                            let line = use_item.use_token.span.start().line;
                            smells.push(Smell::new(
                                SmellCategory::Architecture,
                                "Layer Violation",
                                Severity::Critical,
                                                                crate::domain::smell::FindingConfidence::Medium,
                                SourceLocation {
                                    file: file.path.clone(),
                                    line_start: line,
                                    line_end: line,
                                    column: None,
                                },
                                format!(
                                    "{layer} layer imports from `{forbidden_mod}` — violates dependency direction",
                                    layer = format!("{:?}", layer).to_lowercase()
                                ),
                                "Domain should not depend on infrastructure. Inject dependencies through trait abstractions.",
                            ));
                            break; // One violation per use statement
                        }
                    }
                }
            }
        }

        smells
    }
}

#[derive(Debug, Clone, Copy)]
enum Layer {
    Domain,
    Application,
    Infrastructure,
    Presentation,
    Unknown,
}

fn determine_layer(path: &str) -> Layer {
    let lower = path.to_lowercase();
    if lower.contains("domain") || lower.contains("model") || lower.contains("entity") {
        Layer::Domain
    } else if lower.contains("application")
        || lower.contains("usecase")
        || lower.contains("service")
    {
        Layer::Application
    } else if lower.contains("infra") || lower.contains("db") || lower.contains("http") {
        Layer::Infrastructure
    } else if lower.contains("cli") || lower.contains("presentation") || lower.contains("ui") {
        Layer::Presentation
    } else {
        Layer::Unknown
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
