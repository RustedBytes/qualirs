use syn::spanned::Spanned;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects types that implement Deref/DerefMut for non-pointer semantics.
///
/// Implementing Deref to get "automatic" method forwarding is an anti-pattern.
/// Deref should only be used for smart pointer types (Box, Rc, Arc, etc.).
pub struct DerefAbuseDetector;

impl Detector for DerefAbuseDetector {
    fn name(&self) -> &str {
        "Deref Abuse"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Impl(imp) = item
                && let Some((_, trait_path, _)) = &imp.trait_
            {
                let trait_name = path_last_segment(trait_path);

                if trait_name == "Deref" || trait_name == "DerefMut" {
                    // Check if the type looks like a smart pointer
                    if let syn::Type::Path(tp) = &*imp.self_ty
                        && let Some(seg) = tp.path.segments.last()
                    {
                        let type_name = seg.ident.to_string();

                        if !is_smart_pointer_name(&type_name) {
                            let line = imp.self_ty.span().start().line;
                            smells.push(Smell::new(
                                        SmellCategory::Idiomaticity,
                                        "Deref Abuse",
                                        Severity::Warning,
                                                                                crate::domain::smell::FindingConfidence::Low,
                                        SourceLocation {
                                            file: file.path.clone(),
                                            line_start: line,
                                            line_end: line,
                                            column: None,
                                        },
                                        format!(
                                            "Type `{}` implements {} but does not appear to be a smart pointer",
                                            type_name, trait_name
                                        ),
                                        "Use composition or From/Into instead of Deref for method forwarding.",
                                    ));
                        }
                    }
                }
            }
        }

        smells
    }
}

fn is_smart_pointer_name(name: &str) -> bool {
    let ptr_patterns = [
        "Box", "Rc", "Arc", "RefCell", "Mutex", "RwLock", "Cell", "Pin", "Cow", "NonNull", "Unique",
    ];
    ptr_patterns.iter().any(|p| name.contains(p))
        || name.ends_with("Ptr")
        || name.ends_with("Ref")
        || name.ends_with("Handle")
        || name.ends_with("Guard")
        || name.ends_with("Pointer")
}

fn path_last_segment(path: &syn::Path) -> String {
    path.segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default()
}
