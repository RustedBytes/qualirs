use crate::analysis::evidence::{self, Kind};
use crate::domain::smell::FindingConfidence;
use quote::ToTokens;
use std::collections::{HashMap, HashSet};
use syn::spanned::Spanned;
use syn::visit::{Visit, visit_expr};

use crate::analysis::detector::Detector;
use crate::detectors::policy::is_test_path;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects functions with excessive `.unwrap()` or `.expect()` calls.
pub struct ExcessiveUnwrapDetector;

impl Detector for ExcessiveUnwrapDetector {
    fn name(&self) -> &str {
        "Excessive Unwrap"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        if is_test_path(&file.path) {
            return smells;
        }

        let mut infallible = HashSet::new();
        let mut fallible = HashSet::new();
        struct Types(HashMap<(usize, usize), syn::Type>);
        impl<'a> Visit<'a> for Types {
            fn visit_pat_type(&mut self, p: &'a syn::PatType) {
                if let syn::Pat::Ident(name) = &*p.pat {
                    let at = name.ident.span().start();
                    self.0.insert((at.line, at.column), (*p.ty).clone());
                }
                syn::visit::visit_pat_type(self, p);
            }
            fn visit_fn_arg(&mut self, arg: &'a syn::FnArg) {
                if let syn::FnArg::Typed(p) = arg
                    && let syn::Pat::Ident(name) = &*p.pat
                {
                    let at = name.ident.span().start();
                    self.0.insert((at.line, at.column), (*p.ty).clone());
                }
                syn::visit::visit_fn_arg(self, arg);
            }
        }
        let mut types = Types(HashMap::new());
        types.visit_file(&file.ast);
        evidence::inspect(&file.ast, |expr, ctx| {
            record_scalar_conversion(expr, ctx, &types.0, &mut infallible);
            record_unwrap_evidence(expr, ctx, &mut infallible, &mut fallible);
        });

        for item in &file.ast.items {
            if let syn::Item::Fn(fn_item) = item {
                let mut visitor = UnwrapCounter {
                    unwrap_count: 0,
                    fallible_count: 0,
                    infallible: &infallible,
                    fallible: &fallible,
                };
                visitor.visit_block(&fn_item.block);

                if visitor.unwrap_count > thresholds.r#impl.control_flow.excessive_unwrap {
                    let line = fn_item.sig.fn_token.span.start().line;

                    smells.push(Smell::new(
                        SmellCategory::Idiomaticity,
                        "Excessive Unwrap",
                        Severity::Warning,
                        if visitor.fallible_count > thresholds.r#impl.control_flow.excessive_unwrap { FindingConfidence::High } else { FindingConfidence::Low },
                        SourceLocation {
                            file: file.path.clone(),
                            line_start: line,
                            line_end: line,
                            column: None,
                        },
                        format!(
                            "Function `{}` has {} unwrap/expect calls (threshold: {})",
                            fn_item.sig.ident,
                            visitor.unwrap_count,
                            thresholds.r#impl.control_flow.excessive_unwrap
                        ),
                        "Review fallible unwraps for error handling. Unresolved calls and documented invariants do not by themselves establish a reachable panic.",
                    ));
                }
            }
        }

        smells
    }
}

struct UnwrapCounter<'a> {
    unwrap_count: usize,
    fallible_count: usize,
    infallible: &'a HashSet<(usize, usize, usize, usize)>,
    fallible: &'a HashSet<(usize, usize, usize, usize)>,
}

impl<'ast> Visit<'ast> for UnwrapCounter<'_> {
    fn visit_expr(&mut self, expr: &'ast syn::Expr) {
        if let syn::Expr::MethodCall(call) = expr
            && (call.method == "unwrap" || call.method == "expect")
            && !is_regex_literal_constructor(&call.receiver)
            && !self.infallible.contains(&evidence::span_key(call.span()))
        {
            self.unwrap_count += 1;
            if self.fallible.contains(&evidence::span_key(call.span())) {
                self.fallible_count += 1;
            }
        }
        visit_expr(self, expr);
    }
    fn visit_item(&mut self, _: &'ast syn::Item) {}
    fn visit_expr_closure(&mut self, _: &'ast syn::ExprClosure) {}
    fn visit_expr_async(&mut self, _: &'ast syn::ExprAsync) {}
}

fn scalar_width(name: &str) -> Option<usize> {
    match name {
        "u16" | "i16" => Some(2),
        "u32" | "i32" | "f32" => Some(4),
        "u64" | "i64" | "f64" => Some(8),
        "u128" | "i128" => Some(16),
        _ => None,
    }
}

fn exact_array_conversion(
    expr: &syn::Expr,
    width: usize,
    ctx: &evidence::Context,
    types: &HashMap<(usize, usize), syn::Type>,
) -> bool {
    let syn::Expr::MethodCall(unwrap) = expr else {
        return false;
    };
    if !matches!(unwrap.method.to_string().as_str(), "unwrap" | "expect") {
        return false;
    }
    let syn::Expr::MethodCall(convert) = &*unwrap.receiver else {
        return false;
    };
    if convert.method != "try_into" || !convert.args.is_empty() {
        return false;
    }
    let syn::Expr::Index(index) = &*convert.receiver else {
        return false;
    };
    // A custom Index/try_into API need not implement a byte-array conversion.
    let Some(ty) = ctx
        .binding_origin(&index.expr)
        .and_then(|origin| types.get(&origin))
    else {
        return false;
    };
    if !byte_storage(ty, ctx) {
        return false;
    }
    slice_width_matches(&index.index, width)
}

fn is_regex_literal_constructor(expr: &syn::Expr) -> bool {
    let syn::Expr::Call(call) = expr else {
        return false;
    };
    let syn::Expr::Path(path) = &*call.func else {
        return false;
    };
    let Some(last) = path.path.segments.last() else {
        return false;
    };

    last.ident == "new"
        && path
            .path
            .segments
            .iter()
            .rev()
            .nth(1)
            .is_some_and(|segment| segment.ident == "Regex")
        && call.args.first().is_some_and(
            |arg| matches!(arg, syn::Expr::Lit(lit) if matches!(lit.lit, syn::Lit::Str(_))),
        )
}

fn byte_storage(ty: &syn::Type, ctx: &evidence::Context) -> bool {
    match ty {
        syn::Type::Reference(r) => byte_storage(&r.elem, ctx),
        syn::Type::Slice(s) => {
            matches!(&*s.elem, syn::Type::Path(p) if ctx.path(&p.path) == "u8")
        }
        syn::Type::Array(a) => {
            matches!(&*a.elem, syn::Type::Path(p) if ctx.path(&p.path) == "u8")
        }
        _ => false,
    }
}

fn number(e: &syn::Expr) -> Option<usize> {
    if let syn::Expr::Lit(l) = e
        && let syn::Lit::Int(n) = &l.lit
    {
        n.base10_parse().ok()
    } else {
        None
    }
}

fn slice_width_matches(expr: &syn::Expr, width: usize) -> bool {
    let syn::Expr::Range(range) = expr else {
        return false;
    };
    if !matches!(range.limits, syn::RangeLimits::HalfOpen(_)) {
        return false;
    }
    let (Some(start), Some(end)) = (&range.start, &range.end) else {
        return false;
    };
    if let (Some(a), Some(b)) = (number(start), number(end)) {
        return b.checked_sub(a) == Some(width);
    }
    if !matches!(&**start, syn::Expr::Path(p) if p.path.get_ident().is_some()) {
        return false;
    }
    matches!(&**end, syn::Expr::Binary(b) if matches!(b.op, syn::BinOp::Add(_))
        && b.left.to_token_stream().to_string() == start.to_token_stream().to_string() && number(&b.right) == Some(width))
}

fn record_scalar_conversion(
    expr: &syn::Expr,
    ctx: &evidence::Context,
    types: &HashMap<(usize, usize), syn::Type>,
    infallible: &mut HashSet<(usize, usize, usize, usize)>,
) {
    if let syn::Expr::Call(c) = expr
        && let syn::Expr::Path(p) = &*c.func
    {
        let path = ctx.path(&p.path);
        if let Some((scalar, method)) = path.split_once("::")
            && matches!(method, "from_le_bytes" | "from_be_bytes" | "from_ne_bytes")
            && let Some(size) = scalar_width(scalar)
            && let Some(arg) = c.args.first()
            && exact_array_conversion(arg, size, ctx, types)
        {
            infallible.insert(evidence::span_key(arg.span()));
        }
    }
}

fn record_unwrap_evidence(
    expr: &syn::Expr,
    ctx: &evidence::Context,
    infallible: &mut HashSet<(usize, usize, usize, usize)>,
    fallible: &mut HashSet<(usize, usize, usize, usize)>,
) {
    if let syn::Expr::MethodCall(c) = expr
        && matches!(c.method.to_string().as_str(), "unwrap" | "expect")
    {
        if matches!(ctx.expr(&c.receiver), Kind::Result(_)) {
            fallible.insert(evidence::span_key(c.span()));
        }
        if let syn::Expr::Call(init) = &*c.receiver
            && let syn::Expr::Path(p) = &*init.func
            && matches!(
                ctx.path(&p.path).as_str(),
                "std::ffi::CString::new" | "alloc::ffi::CString::new"
            )
            && let Some(syn::Expr::Lit(lit)) = init.args.first()
            && matches!(&lit.lit, syn::Lit::Str(s) if !s.value().contains('\0'))
        {
            infallible.insert(evidence::span_key(c.span()));
        }
    }
}
