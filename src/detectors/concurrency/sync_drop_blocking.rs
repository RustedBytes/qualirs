use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects potentially blocking code (I/O, Sleep, networking) inside `impl Drop`.
///
/// Running synchronous blocking code inside Drop allows it to be covertly invoked
/// across async boundaries, blocking the executor threads and causing massive latency spikes.
pub struct SyncDropBlockingDetector;

impl Detector for SyncDropBlockingDetector {
    fn name(&self) -> &str {
        "Sync Drop Blocking (Async Hazard)"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Impl(imp) = item
                && let Some((_, path, _)) = &imp.trait_
                && path.is_ident("Drop")
            {
                let mut visitor = BlockingDropVisitor {
                    violations: Vec::new(),
                };
                for it_item in &imp.items {
                    visitor.visit_impl_item(it_item);
                }

                if !visitor.violations.is_empty() {
                    let type_name = if let syn::Type::Path(tp) = &*imp.self_ty {
                        tp.path
                            .segments
                            .last()
                            .map(|s| s.ident.to_string())
                            .unwrap_or_else(|| "Unknown".to_string())
                    } else {
                        "Unknown".to_string()
                    };

                    for (line, method) in visitor.violations {
                        smells.push(Smell::new(
                                                    SmellCategory::Concurrency,
                                                    "Sync Drop Blocking (Async Hazard)",
                                                    Severity::Critical,
                                                                                                        crate::domain::smell::FindingConfidence::High,
                                                    SourceLocation::new(file.path.clone(), line, line, None),
                                                    format!(
                                                        "`Drop` impl for `{}` calls potentially blocking method `{}`", type_name, method
                                                    ),
                                                    "Use `tokio::spawn(async move { ... })` for background cleanup or provide a separate `async fn shutdown()` method rather than blocking in Drop.",
                                                ));
                    }
                }
            }
        }

        smells
    }
}

struct BlockingDropVisitor {
    violations: Vec<(usize, String)>,
}

impl<'ast> Visit<'ast> for BlockingDropVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        let blocking_methods = [
            "read",
            "read_to_end",
            "read_to_string",
            "write",
            "write_all",
            "flush",
            "lock",
            "recv",
            "send",
        ];

        if blocking_methods.contains(&method.as_str()) {
            let line = node.method.span().start().line;
            self.violations.push((line, method));
        }

        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(tp) = &*node.func
            && let Some(seg) = tp.path.segments.last()
        {
            let name = seg.ident.to_string();
            if name == "sleep" || name == "park" {
                let line = seg.ident.span().start().line;
                self.violations.push((line, name));
            }
        }
        syn::visit::visit_expr_call(self, node);
    }
}
