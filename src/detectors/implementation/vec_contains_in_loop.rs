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
                    if ctx
                        .binding_origin(&call.receiver)
                        .is_some_and(|origin| origin >= ctx.loop_start)
                    {
                        FindingConfidence::Low
                    } else {
                        FindingConfidence::High
                    },
                ));
            }
        });
        findings.into_iter().map(|(line, confidence)| Smell::new(
 SmellCategory::Performance, "Vec Contains in Loop", Severity::Info, confidence,
 SourceLocation::new(file.path.clone(), line, line, None), if confidence == FindingConfidence::High { "Vec membership performs a linear scan inside a loop" } else { "A locally created Vec is searched inside a loop; repeated lookup benefit is unproven" }, "Consider a companion set when repeated membership lookup dominates; retain Vec when ordering or small size justifies it. Building a set for one lookup may add cost.",
 )).collect()
    }
}
