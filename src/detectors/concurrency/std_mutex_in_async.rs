use crate::analysis::{
    detector::Detector,
    evidence::{self, Kind},
};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};
use syn::spanned::Spanned;
pub struct StdMutexInAsyncDetector;
impl Detector for StdMutexInAsyncDetector {
    fn name(&self) -> &str {
        "Std Mutex in Async"
    }
    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut lines = std::collections::BTreeSet::new();
        evidence::inspect(&file.ast, |expr, ctx| {
            if ctx.in_async && *ctx.expr(expr).value() == Kind::SyncLock {
                lines.insert(expr.span().start().line);
            }
        });
        lines.into_iter().map(|line| Smell::new(SmellCategory::Concurrency, self.name(), Severity::Info, FindingConfidence::Low,
 SourceLocation::new(file.path.clone(), line, line, None), "A standard synchronous lock is used in async code",
 "Review contention and guard lifetime. Short synchronous critical sections can be appropriate.")).collect()
    }
}
