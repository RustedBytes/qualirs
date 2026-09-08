use crate::analysis::{
    detector::Detector,
    evidence::{self, Kind},
};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};
use syn::spanned::Spanned;
pub struct CollectThenIterateDetector;
impl Detector for CollectThenIterateDetector {
    fn name(&self) -> &str {
        "Collect Then Iterate"
    }
    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut findings = Vec::new();
        evidence::inspect(&file.ast, |expr, ctx| {
            let syn::Expr::MethodCall(query) = expr else {
                return;
            };
            if !matches!(
                query.method.to_string().as_str(),
                "iter" | "into_iter" | "len" | "is_empty"
            ) || !query.args.is_empty()
            {
                return;
            }
            let syn::Expr::MethodCall(collect) = &*query.receiver else {
                return;
            };
            if collect.method != "collect" {
                return;
            }
            let Some(args) = &collect.turbofish else {
                return;
            };
            let Some(syn::GenericArgument::Type(ty)) = args.args.first() else {
                return;
            };
            if ctx.kind(ty) != Kind::Vec {
                return;
            }
            let established_iterator = matches!(&*collect.receiver, syn::Expr::MethodCall(c)
 if matches!(c.method.to_string().as_str(), "iter" | "into_iter") && *ctx.expr(&c.receiver).value() == Kind::Vec);
            findings.push(Smell::new(SmellCategory::Performance, self.name(), Severity::Info,
 if established_iterator { FindingConfidence::High } else { FindingConfidence::Low },
 SourceLocation::new(file.path.clone(), expr.span().start().line, expr.span().end().line, None),
 "An explicit Vec collection is immediately iterated or queried",
 "Consider removing the intermediate Vec only if ownership, evaluation order, and iterator side effects remain equivalent."));
        });
        findings
    }
}
