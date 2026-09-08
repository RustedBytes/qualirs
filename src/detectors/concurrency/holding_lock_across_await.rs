use crate::analysis::{detector::Detector, evidence};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};
use syn::spanned::Spanned;
pub struct HoldingLockAcrossAwaitDetector;
impl Detector for HoldingLockAcrossAwaitDetector {
    fn name(&self) -> &str {
        "Holding Lock Across Await"
    }
    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut findings = Vec::new();
        evidence::inspect(&file.ast, |expr, ctx| {
            if ctx.in_async && matches!(expr, syn::Expr::Await(_)) && ctx.has_guard() {
                findings.push((
                    expr.span().start().line,
                    "A synchronous lock guard remains in scope at this await".to_string(),
                ));
            }
        });
        findings
            .into_iter()
            .map(|(line, message)| {
                Smell::new(
                    SmellCategory::Concurrency,
                    "Holding Lock Across Await",
                    Severity::Critical,
                    FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    message,
                    "Release the synchronous guard before awaiting.",
                )
            })
            .collect()
    }
}
