use crate::analysis::{
    detector::Detector,
    evidence::{self, Kind},
};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};
pub struct BlockingChannelInAsyncDetector;
impl Detector for BlockingChannelInAsyncDetector {
    fn name(&self) -> &str {
        "Blocking Channel in Async"
    }
    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut findings = Vec::new();
        evidence::inspect(&file.ast, |expr, ctx| {
            if ctx.in_async
                && let syn::Expr::MethodCall(call) = expr
                && *ctx.expr(&call.receiver).value() == Kind::SyncReceiver
                && matches!(call.method.to_string().as_str(), "recv" | "recv_timeout")
            {
                findings.push((
                    call.method.span().start().line,
                    "Synchronous channel receive blocks async execution".to_string(),
                ));
            }
        });
        findings
            .into_iter()
            .map(|(line, message)| {
                Smell::new(
                    SmellCategory::Concurrency,
                    "Blocking Channel in Async",
                    Severity::Warning,
                    FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    message,
                    "Use an async channel or move the receive into a blocking worker.",
                )
            })
            .collect()
    }
}
