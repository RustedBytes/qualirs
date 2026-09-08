use crate::analysis::{
    detector::Detector,
    evidence::{self, Kind},
};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};
pub struct CloneOnCopyDetector;
impl Detector for CloneOnCopyDetector {
    fn name(&self) -> &str {
        "Clone on Copy"
    }
    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut findings = Vec::new();
        evidence::inspect(&file.ast, |expr, ctx| {
            if let syn::Expr::MethodCall(call) = expr
                && call.method == "clone"
                && call.args.is_empty()
                && ctx.expr(&call.receiver) == Kind::Copy
            {
                findings.push((
                    call.method.span().start().line,
                    "An established primitive Copy value is cloned".to_string(),
                ));
            }
        });
        findings
            .into_iter()
            .map(|(line, message)| {
                Smell::new(
                    SmellCategory::Performance,
                    "Clone on Copy",
                    Severity::Info,
                    FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    message,
                    "Use the Copy value directly.",
                )
            })
            .collect()
    }
}
