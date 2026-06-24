use syn::spanned::Spanned;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects manual Drop implementations that may indicate resource management issues.
///
/// Manual Drop impls are error-prone and should be reviewed carefully.
/// Often a wrapper type or guard pattern is safer.
pub struct ManualDropDetector;

impl Detector for ManualDropDetector {
    fn name(&self) -> &str {
        "Manual Drop"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Impl(imp) = item
                && let Some((_, trait_path, _)) = &imp.trait_
            {
                let trait_name = path_last(trait_path);
                if trait_name == "Drop"
                    && let syn::Type::Path(tp) = &*imp.self_ty
                    && let Some(seg) = tp.path.segments.last()
                {
                    let line = imp.self_ty.span().start().line;
                    smells.push(Smell::new(
                        SmellCategory::Idiomaticity,
                        "Manual Drop",
                        Severity::Info,
                        crate::domain::smell::FindingConfidence::Medium,
                        SourceLocation {
                            file: file.path.clone(),
                            line_start: line,
                            line_end: line,
                            column: None,
                        },
                        format!(
                            "Type `{}` implements Drop manually — review for safety",
                            seg.ident
                        ),
                        "Consider using a RAII wrapper or guard instead of manual Drop.",
                    ));
                }
            }
        }

        smells
    }
}

fn path_last(path: &syn::Path) -> String {
    path.segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default()
}
