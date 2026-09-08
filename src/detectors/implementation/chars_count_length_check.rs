use crate::analysis::{
    detector::Detector,
    evidence::{self, Kind},
};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};
use syn::spanned::Spanned;
pub struct CharsCountLengthCheckDetector;
impl Detector for CharsCountLengthCheckDetector {
    fn name(&self) -> &str {
        "Chars Count Length Check"
    }
    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut findings = Vec::new();
        evidence::inspect(&file.ast, |expr, ctx| {
            let syn::Expr::Binary(b) = expr else {
                return;
            };
            if !matches!(
                b.op,
                syn::BinOp::Eq(_)
                    | syn::BinOp::Ne(_)
                    | syn::BinOp::Lt(_)
                    | syn::BinOp::Le(_)
                    | syn::BinOp::Gt(_)
                    | syn::BinOp::Ge(_)
            ) {
                return;
            }
            let (count, limit, reversed) =
                if let Some(n) = super::perf_utils::int_lit_value(&b.right) {
                    (&*b.left, n, false)
                } else if let Some(n) = super::perf_utils::int_lit_value(&b.left) {
                    (&*b.right, n, true)
                } else {
                    return;
                };
            let syn::Expr::MethodCall(count) = count else {
                return;
            };
            let syn::Expr::MethodCall(chars) = &*count.receiver else {
                return;
            };
            if count.method != "count"
                || !count.args.is_empty()
                || chars.method != "chars"
                || !chars.args.is_empty()
                || *ctx.expr(&chars.receiver).value() != Kind::String
            {
                return;
            }
            let emptiness = limit == 0
                && (matches!(b.op, syn::BinOp::Eq(_) | syn::BinOp::Ne(_))
                    || !reversed && matches!(b.op, syn::BinOp::Gt(_))
                    || reversed && matches!(b.op, syn::BinOp::Lt(_)));
            if limit == 0 && !emptiness {
                return;
            }
            findings.push(Smell::new(SmellCategory::Performance, self.name(), Severity::Info,
 if emptiness { FindingConfidence::High } else { FindingConfidence::Low },
 SourceLocation::new(file.path.clone(), expr.span().start().line, expr.span().end().line, Some(expr.span().start().column)),
 if emptiness { "String emptiness does not require counting Unicode scalar values" } else { "This Unicode scalar-count comparison traverses the string" },
 if emptiness { "Use is_empty() or !is_empty(), preserving the comparison's meaning." } else { "If this is a hot path, consider bounded character iteration. Byte length is not equivalent to Unicode scalar count." }));
        });
        findings
    }
}
