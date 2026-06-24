use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects excessive use of interior mutability (`RefCell`, `Cell`) in a single file.
///
/// While sometimes necessary, overusing interior mutability disables compile-time
/// borrow checking and risks runtime panics (e.g. `BorrowMutError`).
pub struct InteriorMutabilityAbuseDetector;

impl Detector for InteriorMutabilityAbuseDetector {
    fn name(&self) -> &str {
        "Interior Mutability Abuse"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        let mut visitor = MutVisitor {
            count: 0,
            first_line: 0,
        };
        visitor.visit_file(&file.ast);

        if visitor.count > thresholds.r#impl.type_safety.interior_mutability_abuse {
            smells.push(Smell::new(
                SmellCategory::Performance,
                "Interior Mutability Abuse",
                Severity::Warning,
                                crate::domain::smell::FindingConfidence::Medium,
                SourceLocation::new(file.path.clone(), visitor.first_line, visitor.first_line, None),
                format!("File contains {} usages of RefCell/Cell (threshold: {})", visitor.count, thresholds.r#impl.type_safety.interior_mutability_abuse),
                "Refactor to use standard Rust mutability (`&mut T`) where possible, or rethink structural ownership.",
            ));
        }

        smells
    }
}

struct MutVisitor {
    count: usize,
    first_line: usize,
}

impl<'ast> Visit<'ast> for MutVisitor {
    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        if let Some(seg) = node.path.segments.last() {
            let ident = seg.ident.to_string();
            if ident == "RefCell" || ident == "Cell" || ident == "OnceCell" {
                self.count += 1;
                if self.first_line == 0 {
                    self.first_line = seg.ident.span().start().line;
                }
            }
        }
        syn::visit::visit_type_path(self, node);
    }
}
