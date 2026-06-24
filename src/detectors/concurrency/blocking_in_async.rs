use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects blocking calls inside async functions.
///
/// Blocking operations like std::thread::sleep, std::fs operations,
/// or heavy computation in async context block the executor.
pub struct BlockingInAsyncDetector;

impl Detector for BlockingInAsyncDetector {
    fn name(&self) -> &str {
        "Blocking in Async"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Fn(fn_item) = item
                && fn_item.sig.asyncness.is_some()
            {
                let mut visitor = BlockingCallVisitor {
                    blocking_calls: Vec::new(),
                };
                visitor.visit_item_fn(fn_item);

                if !visitor.blocking_calls.is_empty() {
                    let start = fn_item.block.brace_token.span.open().start().line;
                    let end = fn_item.block.brace_token.span.close().start().line;
                    let calls: Vec<String> = visitor
                        .blocking_calls
                        .iter()
                        .map(|(name, _)| name.clone())
                        .collect();

                    smells.push(Smell::new(
                        SmellCategory::Concurrency,
                        "Blocking in Async",
                        Severity::Warning,
                        crate::domain::smell::FindingConfidence::High,
                        SourceLocation {
                            file: file.path.clone(),
                            line_start: start,
                            line_end: end,
                            column: None,
                        },
                        format!(
                            "Async function `{}` contains blocking calls: {}",
                            fn_item.sig.ident,
                            calls.join(", ")
                        ),
                        "Use async alternatives (tokio::fs, tokio::time::sleep, spawn_blocking).",
                    ));
                }
            }
        }

        smells
    }
}

struct BlockingCallVisitor {
    blocking_calls: Vec<(String, usize)>,
}

impl<'ast> Visit<'ast> for BlockingCallVisitor {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(expr_path) = &*node.func {
            let path_str = path_to_string(&expr_path.path);
            let lower = path_str.to_lowercase();

            let is_blocking = lower.contains("std::thread::sleep")
                || lower.contains("std::thread::spawn")
                || lower.contains("std::fs::read")
                || lower.contains("std::fs::write")
                || lower.contains("std::fs::open")
                || lower.contains("std::fs::create")
                || lower.contains("std::fs::remove")
                || lower.contains("std::fs::rename")
                || lower.contains("std::fs::copy")
                || lower.contains("std::net::tcpstream")
                || lower.contains("std::net::tcplistener")
                || lower.contains("std::net::udpsocket")
                || lower.contains("std::io::stdin")
                || lower.contains("std::io::stdout")
                || (lower.contains("std::io::") && lower.contains("read"))
                || (lower.contains("std::io::") && lower.contains("write"));

            if is_blocking {
                let line = node.paren_token.span.open().start().line;
                self.blocking_calls.push((path_str, line));
            }
        }
        syn::visit::visit_expr_call(self, node);
    }
}

fn path_to_string(path: &syn::Path) -> String {
    let idents: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
    idents.join("::")
}
