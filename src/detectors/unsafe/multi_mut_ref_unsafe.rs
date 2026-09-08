use crate::analysis::{
    detector::Detector,
    evidence::{self, Kind},
};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};
use std::collections::HashMap;
use syn::spanned::Spanned;

pub struct MultiMutRefUnsafeDetector;
impl Detector for MultiMutRefUnsafeDetector {
    fn name(&self) -> &str {
        "Multi Mut Ref Unsafe"
    }
    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut groups: HashMap<_, Vec<usize>> = HashMap::new();
        evidence::inspect(&file.ast, |expr, ctx| {
            let pointer = match expr {
                syn::Expr::Reference(r) if r.mutability.is_some() => match &*r.expr {
                    syn::Expr::Unary(u) if matches!(u.op, syn::UnOp::Deref(_)) => Some(&*u.expr),
                    _ => None,
                },
                syn::Expr::MethodCall(c) if c.method == "as_mut" => Some(&*c.receiver),
                _ => None,
            };
            if let Some(p) = pointer
                && ctx.expr(p) == Kind::RawPointer
                && let Some(name) = evidence::ident(p)
            {
                groups
                    .entry((ctx.execution, ctx.block, ctx.binding_origin(p), name))
                    .or_default()
                    .push(expr.span().start().line);
            }
        });
        groups.into_iter().filter(|(_, lines)| lines.len() >= 2).flat_map(|((_, _, _, name), lines)| lines.into_iter().map(move |line| (line, name.clone())))
 .map(|(line, name)| Smell::new(SmellCategory::Unsafe, self.name(), Severity::Warning, FindingConfidence::Low,
 SourceLocation::new(file.path.clone(), line, line, None),
 format!("Repeated mutable reference construction from raw pointer `{name}`; overlapping lifetimes are unproven"),
 "Verify provenance and reference lifetimes; separate uses may be valid.")).collect()
    }
}
