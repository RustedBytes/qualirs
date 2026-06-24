use std::collections::HashMap;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects if the same struct has multiple `impl` blocks in the same file.
///
/// Scattering the implementation of a single type across multiple blocks makes it
/// harder to understand the type's full capabilities and behavior.
pub struct MultipleImplBlocksDetector;

impl Detector for MultipleImplBlocksDetector {
    fn name(&self) -> &str {
        "Multiple Impl Blocks"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();
        // Maps struct name -> count of inherent impl blocks
        let mut impl_counts: HashMap<String, Vec<usize>> = HashMap::new();

        for item in &file.ast.items {
            if let syn::Item::Impl(imp) = item
                && imp.trait_.is_none()
                && let syn::Type::Path(tp) = &*imp.self_ty
                && let Some(seg) = tp.path.segments.last()
            {
                let type_name = seg.ident.to_string();
                let start_line = imp.impl_token.span.start().line;
                impl_counts.entry(type_name).or_default().push(start_line);
            }
        }

        for (type_name, lines) in impl_counts {
            if lines.len() > 1 {
                let first_line = lines[1]; // Report on the second occurrence
                smells.push(Smell::new(
                    SmellCategory::Architecture,
                    "Multiple Impl Blocks",
                    Severity::Info,
                                        crate::domain::smell::FindingConfidence::Low,
                    SourceLocation::new(file.path.clone(), first_line, first_line, None),
                    format!("Struct `{}` has {} inherent `impl` blocks in this file", type_name, lines.len()),
                    "Consolidate inherent `impl` blocks for the same type into a single block to improve readability.",
                ));
            }
        }

        smells
    }
}
