use syn::visit::{Visit, visit_item_fn};

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects std::sync mutexes used inside async functions.
pub struct StdMutexInAsyncDetector;

impl Detector for StdMutexInAsyncDetector {
    fn name(&self) -> &str {
        "Std Mutex in Async"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = StdMutexVisitor {
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
                    "Std Mutex in Async",
                    Severity::Warning,
                                        crate::domain::smell::FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    "std::sync locking primitive appears inside an async function",
                    "Use tokio::sync/async-aware primitives or ensure the lock cannot block an executor thread.",
                )
            })
            .collect()
    }
}

struct StdMutexVisitor {
    in_async: bool,
    findings: Vec<usize>,
}

impl<'ast> Visit<'ast> for StdMutexVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let prev = self.in_async;
        self.in_async = node.sig.asyncness.is_some();
        visit_item_fn(self, node);
        self.in_async = prev;
    }

    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        if self.in_async {
            let text = node
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            if text.ends_with("Mutex") || text.ends_with("RwLock") {
                self.findings
                    .push(node.path.segments.last().unwrap().ident.span().start().line);
            }
        }
        syn::visit::visit_type_path(self, node);
    }
}
