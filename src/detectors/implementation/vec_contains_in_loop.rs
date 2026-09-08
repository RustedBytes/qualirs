use crate::analysis::{
    detector::Detector,
    evidence::{self, Kind},
};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};
pub struct VecContainsInLoopDetector;
impl Detector for VecContainsInLoopDetector {
    fn name(&self) -> &str {
        "Vec Contains in Loop"
    }
    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut findings = Vec::new();
        evidence::inspect(&file.ast, |expr, ctx| {
            if ctx.loop_depth > 0
                && let syn::Expr::MethodCall(call) = expr
                && call.method == "contains"
                && *ctx.expr(&call.receiver).value() == Kind::Vec
            {
                findings.push((
                    call.method.span().start().line,
                    "Vec membership performs a linear scan inside a loop".to_string(),
                ));
            }
        });
        findings.into_iter().map(|(line, message)| Smell::new(
 SmellCategory::Performance, "Vec Contains in Loop", Severity::Info, FindingConfidence::High,
 SourceLocation::new(file.path.clone(), line, line, None), message, "Consider a set when repeated membership lookup dominates; retain Vec when ordering or small size justifies it.",
 )).collect()
    }
}
