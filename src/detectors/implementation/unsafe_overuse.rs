use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects files with too many `unsafe` blocks.
///
/// Excessive use of unsafe suggests the codebase may not be leveraging
/// Rust's safety guarantees effectively.
pub struct UnsafeOveruseDetector;

impl Detector for UnsafeOveruseDetector {
    fn name(&self) -> &str {
        "Unsafe Block Overuse"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        let mut visitor = UnsafeCounter { count: 0 };
        visitor.visit_file(&file.ast);

        if visitor.count > thresholds.r#impl.type_safety.unsafe_block_overuse {
            smells.push(Smell::new(
                SmellCategory::Implementation,
                "Unsafe Block Overuse",
                Severity::Warning,
                crate::domain::smell::FindingConfidence::Medium,
                SourceLocation {
                    file: file.path.clone(),
                    line_start: 1,
                    line_end: file.line_count,
                    column: None,
                },
                format!(
                    "File has {} unsafe blocks (threshold: {})",
                    visitor.count, thresholds.r#impl.type_safety.unsafe_block_overuse
                ),
                "Minimize unsafe usage. Wrap each unsafe block in a safe abstraction.",
            ));
        }

        smells
    }
}

struct UnsafeCounter {
    count: usize,
}

impl<'ast> Visit<'ast> for UnsafeCounter {
    fn visit_expr_unsafe(&mut self, node: &'ast syn::ExprUnsafe) {
        self.count += 1;
        syn::visit::visit_expr_unsafe(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if node.sig.unsafety.is_some() {
            self.count += 1;
        }
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if node.unsafety.is_some() {
            self.count += 1;
        }
        syn::visit::visit_item_impl(self, node);
    }
}
