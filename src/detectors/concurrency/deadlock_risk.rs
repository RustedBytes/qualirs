use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects potential deadlock patterns: acquiring multiple locks in the same scope.
///
/// When a function calls .lock() or .write() on multiple shared values,
/// lock ordering issues can cause deadlocks.
pub struct DeadlockRiskDetector;

impl Detector for DeadlockRiskDetector {
    fn name(&self) -> &str {
        "Deadlock Risk"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Fn(fn_item) = item {
                let mut visitor = LockCallVisitor {
                    lock_calls: Vec::new(),
                };
                visitor.visit_item_fn(fn_item);

                if visitor.lock_calls.len() >= 2 {
                    let start = fn_item.block.brace_token.span.open().start().line;
                    let end = fn_item.block.brace_token.span.close().start().line;

                    let lock_names: Vec<String> = visitor
                        .lock_calls
                        .iter()
                        .map(|(name, _)| name.clone())
                        .collect();

                    smells.push(Smell::new(
                        SmellCategory::Concurrency,
                        "Deadlock Risk",
                        Severity::Critical,
                                                crate::domain::smell::FindingConfidence::Medium,
                        SourceLocation {
                            file: file.path.clone(),
                            line_start: start,
                            line_end: end,
                            column: None,
                        },
                        format!(
                            "Function `{}` acquires {} locks ({}) — potential deadlock",
                            fn_item.sig.ident,
                            visitor.lock_calls.len(),
                            lock_names.join(", ")
                        ),
                        "Always acquire locks in a consistent order. Consider using a single lock or channels.",
                    ));
                }
            }
        }

        smells
    }
}

struct LockCallVisitor {
    lock_calls: Vec<(String, usize)>,
}

impl<'ast> Visit<'ast> for LockCallVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if method == "lock" || method == "write" || method == "read" || method == "try_lock" {
            let receiver_str = expr_to_string(&node.receiver);
            let line = node.method.span().start().line;
            self.lock_calls
                .push((format!("{}.{}()", receiver_str, method), line));
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

fn expr_to_string(expr: &syn::Expr) -> String {
    match expr {
        syn::Expr::Path(p) => {
            let last_seg = p.path.segments.last();
            last_seg
                .map(|s| s.ident.to_string())
                .unwrap_or_else(|| "_".into())
        }
        syn::Expr::Field(f) => {
            let base = expr_to_string(&f.base);
            let member = match &f.member {
                syn::Member::Named(n) => n.to_string(),
                syn::Member::Unnamed(i) => format!("{}", i.index),
            };
            format!("{}.{}", base, member)
        }
        syn::Expr::Reference(r) => expr_to_string(&r.expr),
        _ => "_".to_string(),
    }
}
