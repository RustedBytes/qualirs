use syn::spanned::Spanned;
use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects overuse of Arc<Mutex<T>> patterns in a file.
///
/// Excessive Arc<Mutex<...>> suggests poor concurrency design where
/// message passing or finer-grained locking would be better.
pub struct ArcMutexOveruseDetector;

impl Detector for ArcMutexOveruseDetector {
    fn name(&self) -> &str {
        "Arc Mutex Overuse"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        let mut visitor = ArcMutexVisitor {
            arc_mutex_count: 0,
            first_line: None,
        };
        syn::visit::visit_file(&mut visitor, &file.ast);

        if visitor.arc_mutex_count > thresholds.concurrency.arc_mutex_overuse {
            let line = visitor.first_line.unwrap_or(1);
            smells.push(Smell::new(
                SmellCategory::Performance,
                "Arc Mutex Overuse",
                Severity::Warning,
                crate::domain::smell::FindingConfidence::Medium,
                SourceLocation {
                    file: file.path.clone(),
                    line_start: line,
                    line_end: line,
                    column: None,
                },
                format!(
                    "File uses Arc<Mutex<...>> {} times (threshold: {})",
                    visitor.arc_mutex_count, thresholds.concurrency.arc_mutex_overuse
                ),
                "Consider message passing (channels) or finer-grained locking (RwLock, atomics).",
            ));
        }

        smells
    }
}

struct ArcMutexVisitor {
    arc_mutex_count: usize,
    first_line: Option<usize>,
}

impl<'ast> Visit<'ast> for ArcMutexVisitor {
    fn visit_type(&mut self, node: &'ast syn::Type) {
        if is_arc_lock_type(node) {
            self.arc_mutex_count += 1;
            if self.first_line.is_none()
                && let syn::Type::Path(tp) = node
            {
                self.first_line = Some(tp.path.span().start().line);
            }
        }
        syn::visit::visit_type(self, node);
    }
}

fn is_arc_lock_type(ty: &syn::Type) -> bool {
    let syn::Type::Path(tp) = ty else {
        return false;
    };
    let Some(segment) = tp.path.segments.last() else {
        return false;
    };
    if segment.ident != "Arc" {
        return false;
    }

    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return false;
    };

    args.args.iter().any(|arg| {
        matches!(
            arg,
            syn::GenericArgument::Type(inner) if type_path_tail_is(inner, &["Mutex", "RwLock"])
        )
    })
}

fn type_path_tail_is(ty: &syn::Type, names: &[&str]) -> bool {
    let syn::Type::Path(tp) = ty else {
        return false;
    };

    tp.path
        .segments
        .last()
        .is_some_and(|segment| names.iter().any(|name| segment.ident == name))
}
