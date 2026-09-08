use crate::analysis::{
    detector::Detector,
    evidence::{Context, Kind},
};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};

/// Suggest Default only for proven default fields without an existing implementation.
pub struct ManualDefaultConstructorDetector;
impl Detector for ManualDefaultConstructorDetector {
    fn name(&self) -> &str {
        "Manual Default Constructor"
    }
    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let ctx = Context::new(&file.ast.items);
        let mut findings = Vec::new();
        for item in &file.ast.items {
            let syn::Item::Impl(imp) = item else {
                continue;
            };
            if imp.trait_.is_some() || !imp.generics.params.is_empty() {
                continue;
            }
            let syn::Type::Path(ty) = &*imp.self_ty else {
                continue;
            };
            let Some(name) = ty.path.get_ident() else {
                continue;
            };
            let Some(strukt) = file.ast.items.iter().find_map(|i| match i {
                syn::Item::Struct(s) if s.ident == *name => Some(s),
                _ => None,
            }) else {
                continue;
            };
            if !strukt.generics.params.is_empty() || existing_default(file, strukt, &ctx) {
                continue;
            }
            let syn::Fields::Named(fields) = &strukt.fields else {
                continue;
            };
            for item in &imp.items {
                let syn::ImplItem::Fn(f) = item else {
                    continue;
                };
                if constructor_defaults(f, fields, &ctx) {
                    let line = f.sig.fn_token.span.start().line;
                    findings.push(Smell::new(SmellCategory::Idiomaticity, self.name(), Severity::Info, FindingConfidence::High,
                        SourceLocation::new(file.path.clone(), line, line, None),
                        "Constructor initializes default fields and no local Default implementation exists",
                        "Consider deriving Default; keep one implementation of initialization and avoid mutual new/default delegation."));
                }
            }
        }
        findings
    }
}

fn existing_default(file: &SourceFile, strukt: &syn::ItemStruct, ctx: &Context) -> bool {
    strukt.attrs.iter().any(|a| a.path().is_ident("derive") && a.parse_args_with(
        syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
    ).is_ok_and(|paths| paths.iter().any(|p| is_default(&ctx.path(p)))))
        || file.ast.items.iter().any(|item| matches!(item, syn::Item::Impl(i)
            if matches!(&*i.self_ty, syn::Type::Path(p) if p.path.is_ident(&strukt.ident.to_string()))
            && i.trait_.as_ref().is_some_and(|(p, _)| is_default(&ctx.path(p)))))
}
fn is_default(path: &str) -> bool {
    matches!(
        path,
        "Default" | "std::default::Default" | "core::default::Default"
    )
}

fn constructor_defaults(f: &syn::ImplItemFn, fields: &syn::FieldsNamed, ctx: &Context) -> bool {
    if f.sig.ident != "new"
        || !f.sig.inputs.is_empty()
        || f.sig.constness.is_some()
        || !f.sig.generics.params.is_empty()
        || !matches!(&f.sig.output, syn::ReturnType::Type(_, t) if matches!(&**t, syn::Type::Path(p) if p.path.is_ident("Self")))
    {
        return false;
    }
    let [syn::Stmt::Expr(syn::Expr::Struct(body), None)] = f.block.stmts.as_slice() else {
        return false;
    };
    if !body.path.is_ident("Self")
        || body.rest.is_some()
        || body.fields.is_empty()
        || body.fields.len() != fields.named.len()
    {
        return false;
    }
    fields
        .named
        .iter()
        .all(|field| field_is_default(field, body, ctx))
}

fn field_is_default(field: &syn::Field, body: &syn::ExprStruct, ctx: &Context) -> bool {
    let Some(value) = body
        .fields
        .iter()
        .find(|v| matches!(&v.member, syn::Member::Named(n) if Some(n) == field.ident.as_ref()))
    else {
        return false;
    };
    let syn::Expr::Call(c) = &value.expr else {
        return false;
    };
    let syn::Expr::Path(p) = &*c.func else {
        return false;
    };
    if !c.args.is_empty() {
        return false;
    }
    let path = ctx.path(&p.path);
    matches!(
        path.as_str(),
        "Default::default" | "std::default::Default::default" | "core::default::Default::default"
    ) || ctx.kind(&field.ty) == Kind::Vec
        && matches!(
            path.as_str(),
            "Vec::new" | "std::vec::Vec::new" | "alloc::vec::Vec::new"
        )
        || ctx.kind(&field.ty) == Kind::String
            && matches!(
                path.as_str(),
                "String::new" | "std::string::String::new" | "alloc::string::String::new"
            )
}
