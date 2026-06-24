use syn::visit::{Visit, visit_expr_closure, visit_item_fn};

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects blocking channel operations inside async functions.
pub struct BlockingChannelInAsyncDetector;

impl Detector for BlockingChannelInAsyncDetector {
    fn name(&self) -> &str {
        "Blocking Channel in Async"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = BlockingChannelVisitor {
            in_async: false,
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|line| {
                Smell::new(
                    SmellCategory::Concurrency,
                    "Blocking Channel in Async",
                    Severity::Warning,
                    crate::domain::smell::FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    "Blocking channel operation appears inside an async function",
                    "Use async channels or spawn blocking work explicitly.",
                )
            })
            .collect()
    }
}

struct BlockingChannelVisitor {
    in_async: bool,
    findings: Vec<usize>,
}

impl<'ast> Visit<'ast> for BlockingChannelVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let prev = self.in_async;
        self.in_async = node.sig.asyncness.is_some();
        visit_item_fn(self, node);
        self.in_async = prev;
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if self.in_async {
            let method = node.method.to_string();
            if is_channel_method(&method) {
                self.findings.push(node.method.span().start().line);
            }
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_await(&mut self, node: &'ast syn::ExprAwait) {
        if let syn::Expr::MethodCall(method_call) = &*node.base
            && is_channel_method(&method_call.method.to_string())
        {
            self.visit_expr(&method_call.receiver);
            for arg in &method_call.args {
                self.visit_expr(arg);
            }
            return;
        }
        syn::visit::visit_expr_await(self, node);
    }

    fn visit_expr_closure(&mut self, node: &'ast syn::ExprClosure) {
        let prev = self.in_async;
        if node.asyncness.is_none() {
            self.in_async = false;
        }
        visit_expr_closure(self, node);
        self.in_async = prev;
    }
}

fn is_channel_method(method: &str) -> bool {
    matches!(method, "recv" | "recv_timeout" | "send_timeout")
}
