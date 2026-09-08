use crate::analysis::{
    detector::Detector,
    evidence::{self, Kind},
};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};
pub struct RawPointerArithmeticDetector;
impl Detector for RawPointerArithmeticDetector {
    fn name(&self) -> &str {
        "Raw Pointer Arithmetic"
    }
    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut findings = Vec::new();
        evidence::inspect(&file.ast, |expr, ctx| {
            if let syn::Expr::MethodCall(call) = expr
                && *ctx.expr(&call.receiver).value() == Kind::RawPointer
                && matches!(
                    call.method.to_string().as_str(),
                    "offset"
                        | "add"
                        | "sub"
                        | "wrapping_offset"
                        | "wrapping_add"
                        | "wrapping_sub"
                        | "byte_offset"
                        | "byte_add"
                        | "byte_sub"
                )
            {
                findings.push((
                    call.method.span().start().line,
                    format!("Raw pointer arithmetic: {}", call.method),
                ));
            }
        });
        findings.into_iter().map(|(line, message)| Smell::new(
 SmellCategory::Unsafe, "Raw Pointer Arithmetic", Severity::Warning, FindingConfidence::High,
 SourceLocation::new(file.path.clone(), line, line, None), message, "Prefer safe indexing when possible; otherwise verify pointer provenance and bounds.",
 )).collect()
    }
}
