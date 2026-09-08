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
            findings.extend(intermediate_collection_finding(expr, ctx, file));
        });
        findings
    }
}

fn intermediate_collection_finding(
    expr: &syn::Expr,
    ctx: &evidence::Context,
    file: &SourceFile,
) -> Option<Smell> {
    let syn::Expr::MethodCall(query) = expr else {
        return None;
    };
    if !matches!(
        query.method.to_string().as_str(),
        "iter" | "into_iter" | "len" | "is_empty"
    ) || !query.args.is_empty()
    {
        return None;
    }
    let syn::Expr::MethodCall(collect) = &*query.receiver else {
        return None;
    };
    if collect.method != "collect" {
        return None;
    }
    let Some(args) = &collect.turbofish else {
        return None;
    };
    let Some(syn::GenericArgument::Type(ty)) = args.args.first() else {
        return None;
    };
    if ctx.kind(ty) != Kind::Vec {
        return None;
    }
    let established_iterator = matches!(&*collect.receiver, syn::Expr::MethodCall(c)
if matches!(c.method.to_string().as_str(), "iter" | "into_iter") && *ctx.expr(&c.receiver).value() == Kind::Vec);
    Some(Smell::new(
        SmellCategory::Performance,
        CollectThenIterateDetector.name(),
        Severity::Info,
        if established_iterator {
            FindingConfidence::High
        } else {
            FindingConfidence::Low
        },
        SourceLocation::new(
            file.path.clone(),
            expr.span().start().line,
            expr.span().end().line,
            None,
        ),
        "An explicit Vec collection is immediately iterated or queried",
        "Consider removing the intermediate Vec only if ownership, evaluation order, and iterator side effects remain equivalent.",
    ))
}
