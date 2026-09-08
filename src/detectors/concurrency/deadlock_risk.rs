use crate::analysis::{
    detector::Detector,
    evidence::{self, Kind},
};
use crate::domain::smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Source evidence of overlapping synchronous lock acquisitions. Without a
/// resolved lock graph, this is a review hint rather than proof of deadlock.
pub struct DeadlockRiskDetector;

impl Detector for DeadlockRiskDetector {
    fn name(&self) -> &str {
        "Deadlock Risk"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();
        evidence::inspect(&file.ast, |expr, ctx| {
            let syn::Expr::MethodCall(call) = expr else {
                return;
            };
            if !matches!(call.method.to_string().as_str(), "lock" | "read" | "write")
                || !call.args.is_empty()
                || *ctx.expr(&call.receiver).value() != Kind::SyncLock
                || !ctx.has_guard()
            {
                return;
            }
            // The callback precedes receiver evaluation. Restrict this hint to
            // a simple binding so evaluating it cannot itself release a guard.
            let Some(receiver) = evidence::ident(&call.receiver) else {
                return;
            };
            let line = call.method.span().start().line;
            smells.push(overlapping_lock_hint(file, &receiver, line));
        });
        smells
    }
}

fn overlapping_lock_hint(file: &SourceFile, receiver: &str, line: usize) -> Smell {
    Smell::new(
        SmellCategory::Concurrency,
        "Deadlock Risk",
        Severity::Critical,
        FindingConfidence::Low,
        SourceLocation::new(file.path.clone(), line, line, None),
        format!(
            "Synchronous lock `{receiver}` is acquired while a synchronous guard remains in scope; a deadlock cycle is unproven"
        ),
        "Review whether these guards must overlap and whether all callers use a consistent lock order.",
    )
}
