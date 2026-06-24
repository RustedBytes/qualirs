use syn::visit::{Visit, visit_expr_method_call};

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects iterator chains that collect only to immediately iterate again.
pub struct CollectThenIterateDetector;

impl Detector for CollectThenIterateDetector {
    fn name(&self) -> &str {
        "Collect Then Iterate"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = CollectThenIterateVisitor {
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|line| {
                Smell::new(
                    SmellCategory::Performance,
                    "Collect Then Iterate",
                    Severity::Info,
                    crate::domain::smell::FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    "Iterator chain collects into a Vec and immediately iterates or queries it",
                    "Keep the chain lazy, or collect once and reuse the collection meaningfully.",
                )
            })
            .collect()
    }
}

struct CollectThenIterateVisitor {
    findings: Vec<usize>,
}

impl<'ast> Visit<'ast> for CollectThenIterateVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if matches!(method.as_str(), "iter" | "into_iter" | "len" | "is_empty")
            && is_collect_call(&node.receiver)
        {
            self.findings.push(node.method.span().start().line);
        }
        visit_expr_method_call(self, node);
    }
}

fn is_collect_call(expr: &syn::Expr) -> bool {
    matches!(expr, syn::Expr::MethodCall(call) if call.method == "collect")
}
