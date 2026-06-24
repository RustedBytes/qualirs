use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects `let _ = spawn(...)` task handles that are immediately dropped.
pub struct DroppedJoinHandleDetector;

impl Detector for DroppedJoinHandleDetector {
    fn name(&self) -> &str {
        "Dropped JoinHandle"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = DroppedJoinHandleVisitor {
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|line| {
                Smell::new(
                    SmellCategory::Concurrency,
                    "Dropped JoinHandle",
                    Severity::Warning,
                    crate::domain::smell::FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    "Spawned task handle is assigned to `_` and immediately dropped",
                    "Keep the JoinHandle and await, abort, or document intentional detachment.",
                )
            })
            .collect()
    }
}

struct DroppedJoinHandleVisitor {
    findings: Vec<usize>,
}

impl<'ast> Visit<'ast> for DroppedJoinHandleVisitor {
    fn visit_local(&mut self, node: &'ast syn::Local) {
        if matches!(node.pat, syn::Pat::Wild(_))
            && let Some(init) = &node.init
            && is_spawn_call(&init.expr)
        {
            self.findings.push(node.let_token.span.start().line);
        }
        syn::visit::visit_local(self, node);
    }
}

fn is_spawn_call(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Call(call) => {
            matches!(&*call.func, syn::Expr::Path(path) if path.path.segments.last().map(|s| s.ident == "spawn").unwrap_or(false))
        }
        _ => false,
    }
}
