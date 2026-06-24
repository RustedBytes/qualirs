use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects types that implement many standard library traits without a clear domain purpose.
///
/// Flags types that implement 5+ standard traits (Debug, Clone, Copy, Hash, Eq, Ord, etc.)
/// without implementing any domain-specific traits, suggesting the type is a data bag
/// with leaked implementation details.
pub struct TraitImplLeakageDetector;

impl Detector for TraitImplLeakageDetector {
    fn name(&self) -> &str {
        "Trait Impl Leakage"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        // Collect all type names and their impl blocks
        let mut type_impls: std::collections::HashMap<String, Vec<TraitKind>> =
            std::collections::HashMap::new();

        for item in &file.ast.items {
            if let syn::Item::Impl(imp) = item
                && let Some((_, trait_path, _)) = &imp.trait_
            {
                let trait_name = trait_path_to_string(trait_path);
                let kind = classify_trait(&trait_name);
                if let syn::Type::Path(tp) = &*imp.self_ty {
                    let type_name = tp
                        .path
                        .segments
                        .last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_default();
                    if !type_name.is_empty() {
                        type_impls.entry(type_name).or_default().push(kind);
                    }
                }
            }
        }

        for (type_name, traits) in &type_impls {
            let std_count = traits
                .iter()
                .filter(|k| matches!(k, TraitKind::Std))
                .count();
            let domain_count = traits
                .iter()
                .filter(|k| matches!(k, TraitKind::Domain))
                .count();

            if std_count >= 5 && domain_count == 0 {
                smells.push(Smell::new(
                    SmellCategory::Design,
                    "Trait Impl Leakage",
                    Severity::Info,
                                        crate::domain::smell::FindingConfidence::Low,
                    SourceLocation {
                        file: file.path.clone(),
                        line_start: 1,
                        line_end: file.line_count,
                        column: None,
                    },
                    format!(
                        "Type `{}` implements {} std traits but no domain traits — may be an anemic type",
                        type_name, std_count
                    ),
                    "Add domain behavior to this type or implement domain-specific traits.",
                ));
            }
        }

        smells
    }
}

#[derive(Clone, Copy)]
enum TraitKind {
    Std,
    Domain,
}

fn classify_trait(name: &str) -> TraitKind {
    let std_traits = [
        "Debug",
        "Clone",
        "Copy",
        "Hash",
        "Eq",
        "PartialEq",
        "Ord",
        "PartialOrd",
        "Display",
        "FromStr",
        "Default",
        "From",
        "Into",
        "AsRef",
        "AsMut",
        "Borrow",
        "ToOwned",
        "ToString",
        "Iterator",
        "ExactSizeIterator",
        "Read",
        "Write",
        "Seek",
        "BufRead",
        "Error",
        "Send",
        "Sync",
        "Unpin",
        "Sized",
        "Fn",
        "FnMut",
        "FnOnce",
    ];

    if std_traits.contains(&name) {
        TraitKind::Std
    } else {
        TraitKind::Domain
    }
}

fn trait_path_to_string(path: &syn::Path) -> String {
    path.segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default()
}
