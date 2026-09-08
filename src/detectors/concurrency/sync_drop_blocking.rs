use crate::analysis::{
    detector::Detector,
    evidence::{self, Context, Kind},
};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};
use syn::{spanned::Spanned, visit::Visit};

/// Blocking work in a destructor is not by itself proof of an async hazard.
pub struct SyncDropBlockingDetector;
impl Detector for SyncDropBlockingDetector {
    fn name(&self) -> &str {
        "Sync Drop Blocking (Async Hazard)"
    }
    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut drops = Drops {
            items: &file.ast.items,
            scopes: Vec::new(),
        };
        drops.visit_file(&file.ast);
        let mut findings = Vec::new();
        evidence::inspect(&file.ast, |expr, ctx| {
            let Some((_, fields)) = drops.scopes.iter().find(|(f, _)| {
                let p = f.sig.ident.span().start();
                ctx.execution == (p.line, p.column)
            }) else {
                return;
            };
            let evidence = blocking_operation(expr, ctx, fields);
            if let Some((confidence, operation)) = evidence {
                let line = expr.span().start().line;
                findings.push(Smell::new(SmellCategory::Concurrency, self.name(), Severity::Warning, confidence,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    if confidence == FindingConfidence::High { format!("Drop executes synchronous blocking operation `{operation}`") }
                    else { format!("Drop calls `{operation}`; blocking behavior and async execution are unproven") },
                    "Review destructor latency. If cleanup can block or fail, consider explicit shutdown before dropping; background cleanup requires an owned lifetime and an available runtime."));
            }
        });
        findings
    }
}

struct Drops<'a> {
    items: &'a [syn::Item],
    scopes: Vec<(syn::ImplItemFn, Vec<(syn::Member, syn::Type)>)>,
}
impl<'a> Visit<'a> for Drops<'a> {
    fn visit_item_impl(&mut self, i: &'a syn::ItemImpl) {
        let ctx = Context::new(self.items);
        if !i.trait_.as_ref().is_some_and(|(p, _)| {
            matches!(
                ctx.path(p).as_str(),
                "Drop" | "std::ops::Drop" | "core::ops::Drop"
            )
        }) {
            return;
        }
        let fields = self.items.iter().find_map(|item| match item {
            syn::Item::Struct(s) if matches!(&*i.self_ty, syn::Type::Path(t) if t.path.is_ident(&s.ident.to_string())) => Some(s.fields.iter().enumerate().map(|(n, f)| (
                f.ident.clone().map(syn::Member::Named).unwrap_or_else(|| syn::Member::Unnamed(n.into())), f.ty.clone()
            )).collect::<Vec<_>>()), _ => None,
        }).unwrap_or_default();
        for item in &i.items {
            if let syn::ImplItem::Fn(f) = item
                && f.sig.ident == "drop"
            {
                self.scopes.push((f.clone(), fields.clone()));
            }
        }
    }
    fn visit_item_mod(&mut self, m: &'a syn::ItemMod) {
        if let Some((_, items)) = &m.content {
            let saved = self.items;
            self.items = items;
            for i in items {
                self.visit_item(i);
            }
            self.items = saved;
        }
    }
}

fn blocking_operation(
    expr: &syn::Expr,
    ctx: &Context,
    fields: &[(syn::Member, syn::Type)],
) -> Option<(FindingConfidence, String)> {
    match expr {
        syn::Expr::Call(c) => {
            let syn::Expr::Path(p) = &*c.func else {
                return None;
            };
            let path = ctx.path(&p.path);
            if matches!(
                path.as_str(),
                "std::thread::sleep" | "std::thread::park" | "std::thread::park_timeout"
            ) || path.starts_with("std::fs::") && evidence::is_fs_result(&path)
            {
                Some((FindingConfidence::High, path))
            } else {
                None
            }
        }
        syn::Expr::MethodCall(c) => blocking_method(c, ctx, fields),
        _ => None,
    }
}

fn blocking_method(
    c: &syn::ExprMethodCall,
    ctx: &Context,
    fields: &[(syn::Member, syn::Type)],
) -> Option<(FindingConfidence, String)> {
    let method = c.method.to_string();
    let field = if let syn::Expr::Field(f) = &*c.receiver
        && matches!(&*f.base, syn::Expr::Path(p) if p.path.is_ident("self"))
    {
        fields
            .iter()
            .find(|(member, _)| *member == f.member)
            .map(|(_, ty)| ty)
    } else {
        None
    };
    let kind = field
        .map(|t| ctx.kind(t))
        .unwrap_or_else(|| ctx.expr(&c.receiver));
    if matches!(kind.value(), Kind::AsyncLock) {
        return None;
    }
    if owned_lock_acquisition(field, &kind, &method, ctx) {
        return None;
    }
    blocking_method_confidence(&kind, &method).map(|confidence| (confidence, method))
}

fn owned_lock_acquisition(
    field: Option<&syn::Type>,
    kind: &Kind,
    method: &str,
    ctx: &Context,
) -> bool {
    // An owned mutex is exclusively accessible during Drop. An Arc or
    // reference to a mutex does not carry this guarantee.
    if field.is_some() && *kind == Kind::SyncLock && matches!(method, "lock" | "read" | "write") {
        return true;
    }
    if let Some(syn::Type::Path(p)) = field
        && matches!(
            ctx.path(&p.path).as_str(),
            "std::sync::Mutex" | "std::sync::RwLock" | "parking_lot::Mutex" | "parking_lot::RwLock"
        )
        && matches!(method, "lock" | "read" | "write")
    {
        return true;
    }
    false
}

fn blocking_method_confidence(kind: &Kind, method: &str) -> Option<FindingConfidence> {
    let known = matches!(
        (kind.value(), method),
        (
            Kind::Writer,
            "read"
                | "read_to_end"
                | "read_to_string"
                | "write"
                | "write_all"
                | "flush"
                | "sync_all"
                | "sync_data"
        ) | (Kind::SyncReceiver, "recv" | "recv_timeout")
    );
    if known {
        Some(FindingConfidence::High)
    } else if matches!(
        method,
        "read"
            | "read_to_end"
            | "read_to_string"
            | "write"
            | "write_all"
            | "flush"
            | "lock"
            | "recv"
            | "send"
    ) {
        Some(FindingConfidence::Low)
    } else {
        None
    }
}
