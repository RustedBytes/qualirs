use syn::spanned::Spanned;
use syn::visit::{Visit, visit_expr_unsafe};

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects unsafe blocks that cover more code than necessary.
pub struct LargeUnsafeBlockDetector;

impl Detector for LargeUnsafeBlockDetector {
    fn name(&self) -> &str {
        "Large Unsafe Block"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = LargeUnsafeVisitor {
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);
        visitor
            .findings
            .into_iter()
            .map(|(line, loc)| {
                Smell::new(
                    SmellCategory::Unsafe,
                    "Large Unsafe Block",
                    Severity::Warning,
                    crate::domain::smell::FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    format!("Unsafe block spans {loc} lines"),
                    "Keep unsafe blocks as small as possible and move safe code outside.",
                )
            })
            .collect()
    }
}

struct LargeUnsafeVisitor {
    findings: Vec<(usize, usize)>,
}

impl<'ast> Visit<'ast> for LargeUnsafeVisitor {
    fn visit_expr_unsafe(&mut self, node: &'ast syn::ExprUnsafe) {
        let start = node.span().start().line;
        let end = node.span().end().line;
        let loc = end.saturating_sub(start) + 1;
        if loc > 8 {
            self.findings.push((start, loc));
        }
        visit_expr_unsafe(self, node);
    }
}
