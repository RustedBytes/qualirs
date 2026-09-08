use crate::analysis::{detector::Detector, evidence};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};
use syn::spanned::Spanned;
pub struct TransmuteUsageDetector;
impl Detector for TransmuteUsageDetector {
    fn name(&self) -> &str {
        "Transmute Usage"
    }
    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut findings = Vec::new();
        evidence::inspect(&file.ast, |expr, ctx| {
            if let syn::Expr::Call(call) = expr
                && let syn::Expr::Path(path) = &*call.func
                && matches!(
                    ctx.path(&path.path).as_str(),
                    "std::mem::transmute"
                        | "core::mem::transmute"
                        | "std::mem::transmute_copy"
                        | "core::mem::transmute_copy"
                )
            {
                findings.push((
                    expr.span().start().line,
                    "Explicit memory transmutation requires a safety review".to_string(),
                ));
            }
        });
        findings
            .into_iter()
            .map(|(line, message)| {
                Smell::new(
                    SmellCategory::Unsafe,
                    "Transmute Usage",
                    Severity::Critical,
                    FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    message,
                    "Prefer a checked conversion where it preserves the intended representation.",
                )
            })
            .collect()
    }
}
