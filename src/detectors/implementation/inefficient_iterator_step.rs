use syn::visit::{Visit, visit_expr_method_call};

use crate::analysis::detector::Detector;
use crate::detectors::implementation::perf_utils::int_lit_value;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects no-op or split iterator stepping patterns.
pub struct InefficientIteratorStepDetector;

impl Detector for InefficientIteratorStepDetector {
    fn name(&self) -> &str {
        "Inefficient Iterator Step"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = IteratorStepVisitor {
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|finding| {
                Smell::new(
                    SmellCategory::Performance,
                    "Inefficient Iterator Step",
                    Severity::Info,
                    crate::domain::smell::FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), finding.line, finding.line, None),
                    finding.message,
                    finding.suggestion,
                )
            })
            .collect()
    }
}

struct Finding {
    line: usize,
    message: &'static str,
    suggestion: &'static str,
}

struct IteratorStepVisitor {
    findings: Vec<Finding>,
}

impl<'ast> Visit<'ast> for IteratorStepVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "nth"
            && node
                .args
                .first()
                .is_some_and(|arg| int_lit_value(arg) == Some(0))
        {
            self.findings.push(Finding {
                line: node.method.span().start().line,
                message: "`nth(0)` is equivalent to `next()`",
                suggestion: "Use `next()` for the first iterator item.",
            });
        }

        if node.method == "next"
            && matches!(&*node.receiver, syn::Expr::MethodCall(receiver) if receiver.method == "skip")
        {
            self.findings.push(Finding {
                line: node.method.span().start().line,
                message: "`skip(n).next()` steps an iterator in two adapters",
                suggestion: "Use `nth(n)` to skip and take the desired item directly.",
            });
        }

        visit_expr_method_call(self, node);
    }
}
