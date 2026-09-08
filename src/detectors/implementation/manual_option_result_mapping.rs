use syn::visit::Visit;

use crate::analysis::{
    detector::Detector,
    evidence::{self, Context},
};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};

/// Detects bounded runtime matches compatible with synchronous map/map_err closures.
pub struct ManualOptionResultMappingDetector;

impl Detector for ManualOptionResultMappingDetector {
    fn name(&self) -> &str {
        "Manual Option/Result Mapping"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut findings = Vec::new();
        evidence::inspect(&file.ast, |expr, ctx| {
            let syn::Expr::Match(node) = expr else {
                return;
            };
            if ctx.in_const {
                return;
            }
            let [first, second] = node.arms.as_slice() else {
                return;
            };
            let Some(a) = arm_shape(first, ctx) else {
                return;
            };
            let Some(b) = arm_shape(second, ctx) else {
                return;
            };
            let paired = matches!(
                (a.0, b.0),
                (Variant::Some, Variant::None)
                    | (Variant::None, Variant::Some)
                    | (Variant::Ok, Variant::Err)
                    | (Variant::Err, Variant::Ok)
            );
            if !paired {
                return;
            }
            // Transforming both Result branches may require conflicting closure captures.
            let confidence = if matches!(a.0, Variant::Ok | Variant::Err) && !a.1 && !b.1 {
                FindingConfidence::Low
            } else {
                FindingConfidence::High
            };
            let line = node.match_token.span.start().line;
            findings.push(Smell::new(SmellCategory::Idiomaticity, self.name(), Severity::Info,
                confidence, SourceLocation::new(file.path.clone(), line, line, None),
                "Match expression manually maps Option or Result variants",
                "Consider map or map_err if closure captures and borrowing permit an equivalent transformation."));
        });
        findings
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    Some,
    None,
    Ok,
    Err,
}

fn variant(path: &syn::Path, ctx: &Context) -> Option<Variant> {
    match ctx.path(path).as_str() {
        "Some" | "Option::Some" | "std::option::Option::Some" | "core::option::Option::Some" => {
            Some(Variant::Some)
        }
        "None" | "Option::None" | "std::option::Option::None" | "core::option::Option::None" => {
            Some(Variant::None)
        }
        "Ok" | "Result::Ok" | "std::result::Result::Ok" | "core::result::Result::Ok" => {
            Some(Variant::Ok)
        }
        "Err" | "Result::Err" | "std::result::Result::Err" | "core::result::Result::Err" => {
            Some(Variant::Err)
        }
        _ => None,
    }
}

fn arm_shape(arm: &syn::Arm, ctx: &Context) -> Option<(Variant, bool)> {
    let (pattern, binding) = match &arm.pat {
        syn::Pat::TupleStruct(p) if p.elems.len() == 1 => {
            let binding = match p.elems.first()? {
                syn::Pat::Ident(p) if p.by_ref.is_none() && p.subpat.is_none() => Some(&p.ident),
                syn::Pat::Wild(_) => None,
                _ => return None,
            };
            (variant(&p.path, ctx)?, binding)
        }
        syn::Pat::Path(p) => (variant(&p.path, ctx)?, None),
        syn::Pat::Ident(p) if p.by_ref.is_none() && p.subpat.is_none() => {
            (variant(&syn::Path::from(p.ident.clone()), ctx)?, None)
        }
        _ => return None, // Guards and refutable subpatterns cannot become map closures.
    };
    let body = transparent_expr(&arm.body);
    if pattern == Variant::None {
        return matches!(body, syn::Expr::Path(p) if variant(&p.path, ctx) == Some(Variant::None))
            .then_some((pattern, true));
    }
    let syn::Expr::Call(call) = body else {
        return None;
    };
    let syn::Expr::Path(path) = &*call.func else {
        return None;
    };
    if variant(&path.path, ctx)? != pattern || call.args.len() != 1 {
        return None;
    }
    let value = call.args.first()?;
    let mut boundary = ClosureBoundary(false);
    boundary.visit_expr(value);
    if boundary.0 {
        return None;
    }
    let identity = matches!(value, syn::Expr::Path(p) if binding.is_some_and(|b| p.path.is_ident(&b.to_string())));
    Some((pattern, identity))
}

fn transparent_expr(expr: &syn::Expr) -> &syn::Expr {
    match expr {
        syn::Expr::Block(b) if b.label.is_none() => match b.block.stmts.as_slice() {
            [syn::Stmt::Expr(e, None)] => transparent_expr(e),
            _ => expr,
        },
        syn::Expr::Group(g) => transparent_expr(&g.expr),
        syn::Expr::Paren(p) => transparent_expr(&p.expr),
        _ => expr,
    }
}

struct ClosureBoundary(bool);
impl<'a> Visit<'a> for ClosureBoundary {
    fn visit_expr(&mut self, expr: &'a syn::Expr) {
        if matches!(
            expr,
            syn::Expr::Await(_)
                | syn::Expr::Try(_)
                | syn::Expr::Return(_)
                | syn::Expr::Break(_)
                | syn::Expr::Continue(_)
                | syn::Expr::Yield(_)
                | syn::Expr::Macro(_)
        ) {
            self.0 = true;
        }
        syn::visit::visit_expr(self, expr);
    }
}
