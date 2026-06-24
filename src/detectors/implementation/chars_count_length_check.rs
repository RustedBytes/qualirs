use syn::visit::{Visit, visit_expr_binary};

use crate::analysis::detector::Detector;
use crate::detectors::implementation::perf_utils::int_lit_value;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects `chars().count()` used for simple length checks.
pub struct CharsCountLengthCheckDetector;

impl Detector for CharsCountLengthCheckDetector {
    fn name(&self) -> &str {
        "Chars Count Length Check"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = CharsCountVisitor {
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|finding| {
                Smell::new(
                    SmellCategory::Performance,
                    "Chars Count Length Check",
                    Severity::Info,
                    crate::domain::smell::FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), finding.line, finding.line, None),
                    "`chars().count()` walks the whole string for a length comparison",
                    finding.suggestion,
                )
            })
            .collect()
    }
}

struct Finding {
    line: usize,
    suggestion: &'static str,
}

struct CharsCountVisitor {
    findings: Vec<Finding>,
}

impl<'ast> Visit<'ast> for CharsCountVisitor {
    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        let left = chars_count_line(&node.left);
        let right = chars_count_line(&node.right);

        if let Some(line) = left
            && let Some(limit) = int_lit_value(&node.right)
        {
            self.findings.push(Finding {
                line,
                suggestion: suggestion_for_limit(limit),
            });
        } else if let Some(line) = right
            && let Some(limit) = int_lit_value(&node.left)
        {
            self.findings.push(Finding {
                line,
                suggestion: suggestion_for_limit(limit),
            });
        }

        visit_expr_binary(self, node);
    }
}

fn chars_count_line(expr: &syn::Expr) -> Option<usize> {
    let syn::Expr::MethodCall(count) = expr else {
        return None;
    };
    if count.method != "count" {
        return None;
    }

    let syn::Expr::MethodCall(chars) = &*count.receiver else {
        return None;
    };
    if chars.method != "chars" {
        return None;
    }

    Some(count.method.span().start().line)
}

fn suggestion_for_limit(limit: u128) -> &'static str {
    if limit == 0 {
        "Use `is_empty()`/`!is_empty()` when byte emptiness is what matters. Keep `chars().count()` only when Unicode scalar count is intentional."
    } else {
        "Use `len()` for byte length checks, or keep `chars().count()` only when Unicode scalar count is intentional."
    }
}
