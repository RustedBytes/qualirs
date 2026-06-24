use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects `unsafe` blocks that lack a safety comment explaining why they are sound.
pub struct UnsafeWithoutCommentDetector;

impl Detector for UnsafeWithoutCommentDetector {
    fn name(&self) -> &str {
        "Unsafe Without Comment"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        if !crate::domain::config::current_thresholds()
            .r#unsafe
            .unsafe_without_comment
        {
            return Vec::new();
        }

        let mut smells = Vec::new();
        let source_lines: Vec<&str> = file.code.lines().collect();

        let mut visitor = UnsafeVisitor {
            source_lines: &source_lines,
            smells: &mut smells,
            file_path: &file.path,
        };
        visitor.visit_file(&file.ast);

        smells
    }
}

struct UnsafeVisitor<'a> {
    source_lines: &'a [&'a str],
    smells: &'a mut Vec<Smell>,
    file_path: &'a std::path::Path,
}

impl<'ast, 'a> Visit<'ast> for UnsafeVisitor<'a> {
    fn visit_expr_unsafe(&mut self, node: &'ast syn::ExprUnsafe) {
        let line = node.unsafe_token.span.start().line;

        if !has_safety_comment(self.source_lines, line) {
            self.smells.push(Smell::new(
                SmellCategory::Unsafe,
                "Unsafe Without Comment",
                Severity::Warning,
                crate::domain::smell::FindingConfidence::High,
                SourceLocation {
                    file: self.file_path.to_path_buf(),
                    line_start: line,
                    line_end: line,
                    column: None,
                },
                String::from("Unsafe block without a SAFETY comment explaining why it is sound"),
                String::from(
                    "Add a '// SAFETY:' comment explaining why this unsafe code is correct.",
                ),
            ));
        }

        syn::visit::visit_expr_unsafe(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if let Some(unsafety) = node.unsafety {
            let line = unsafety.span.start().line;

            if !has_safety_comment(self.source_lines, line) {
                self.smells.push(Smell::new(
                    SmellCategory::Unsafe,
                    "Unsafe Without Comment",
                    Severity::Warning,
                    crate::domain::smell::FindingConfidence::High,
                    SourceLocation {
                        file: self.file_path.to_path_buf(),
                        line_start: line,
                        line_end: line,
                        column: None,
                    },
                    String::from("Unsafe impl without a SAFETY comment"),
                    String::from(
                        "Add a '// SAFETY:' comment explaining why this unsafe impl is correct.",
                    ),
                ));
            }
        }

        syn::visit::visit_item_impl(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if let Some(unsafety) = node.sig.unsafety {
            let line = unsafety.span.start().line;

            if !has_safety_comment(self.source_lines, line) {
                self.smells.push(Smell::new(
                    SmellCategory::Unsafe,
                    "Unsafe Without Comment",
                    Severity::Warning,
                    crate::domain::smell::FindingConfidence::High,
                    SourceLocation {
                        file: self.file_path.to_path_buf(),
                        line_start: line,
                        line_end: line,
                        column: None,
                    },
                    format!("Unsafe function `{}` without a SAFETY comment", node.sig.ident),
                    String::from("Add a '// SAFETY:' comment explaining why this unsafe function is correct."),
                ));
            }
        }

        syn::visit::visit_item_fn(self, node);
    }
}

const SAFETY_COMMENT_LOOKBACK: usize = 3;

fn has_safety_comment(lines: &[&str], line_number: usize) -> bool {
    let start = line_number.saturating_sub(SAFETY_COMMENT_LOOKBACK);
    let end = line_number;

    (start..end).any(|i| {
        lines
            .get(i)
            .is_some_and(|line| line.to_lowercase().contains("safety"))
    })
}
