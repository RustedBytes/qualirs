use crate::analysis::{detector::Detector, evidence::Context};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};
pub struct DerivableImplDetector;
impl Detector for DerivableImplDetector {
    fn name(&self) -> &str {
        "Derivable Impl"
    }
    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let ctx = Context::new(&file.ast.items);
        let mut findings = Vec::new();
        for item in &file.ast.items {
            let syn::Item::Impl(imp) = item else {
                continue;
            };
            if !imp.generics.params.is_empty()
                || imp.generics.where_clause.is_some()
                || !imp.attrs.is_empty()
            {
                continue;
            }
            let Some((trait_path, _)) = &imp.trait_ else {
                continue;
            };
            let trait_name = ctx.path(trait_path);
            let standard = match trait_name.as_str() {
                "Default" | "std::default::Default" | "core::default::Default" => "Default",
                "Clone" | "std::clone::Clone" | "core::clone::Clone" => "Clone",
                "Eq" | "std::cmp::Eq" | "core::cmp::Eq" => "Eq",
                _ => continue,
            };
            let syn::Type::Path(self_ty) = &*imp.self_ty else {
                continue;
            };
            let Some(name) = self_ty.path.get_ident() else {
                continue;
            };
            let Some(strukt) = file.ast.items.iter().find_map(|i| match i {
                syn::Item::Struct(s) if s.ident == *name => Some(s),
                _ => None,
            }) else {
                continue;
            };
            if !strukt.generics.params.is_empty()
                || strukt.generics.where_clause.is_some()
                || strukt
                    .attrs
                    .iter()
                    .any(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"))
                || strukt.fields.iter().any(|f| !f.attrs.is_empty())
            {
                continue;
            }
            let equivalent = if standard == "Eq" {
                imp.items.is_empty()
            } else {
                fieldwise(imp, strukt, standard, &ctx)
            };
            if equivalent {
                let line = imp.impl_token.span.start().line;
                findings.push(Smell::new(SmellCategory::Idiomaticity, self.name(), Severity::Info, FindingConfidence::High,
 SourceLocation::new(file.path.clone(), line, line, None),
 format!("Manual {standard} implementation is equivalent to a derive for this nongeneric struct"),
 format!("Use #[derive({standard})] on the struct.")));
            }
        }
        findings
    }
}
fn fieldwise(imp: &syn::ItemImpl, strukt: &syn::ItemStruct, standard: &str, ctx: &Context) -> bool {
    let [syn::ImplItem::Fn(f)] = imp.items.as_slice() else {
        return false;
    };
    if !f.attrs.is_empty()
        || !f.sig.generics.params.is_empty()
        || f.sig.generics.where_clause.is_some()
    {
        return false;
    }
    if standard == "Default" && (f.sig.ident != "default" || !f.sig.inputs.is_empty())
        || standard == "Clone" && (f.sig.ident != "clone" || f.sig.inputs.len() != 1)
    {
        return false;
    }
    if !matches!(&f.sig.output, syn::ReturnType::Type(_, ty) if matches!(&**ty, syn::Type::Path(p) if p.path.is_ident("Self")))
    {
        return false;
    }
    let [syn::Stmt::Expr(syn::Expr::Struct(body), None)] = f.block.stmts.as_slice() else {
        return false;
    };
    if !(body.path.is_ident("Self") || body.path.is_ident(&strukt.ident.to_string()))
        || body.rest.is_some()
        || body.fields.len() != strukt.fields.len()
    {
        return false;
    }
    let syn::Fields::Named(fields) = &strukt.fields else {
        return false;
    };
    fields.named.iter().all(|field| {
 let Some(name) = &field.ident else { return false; };
 let Some(value) = body.fields.iter().find(|v| matches!(&v.member, syn::Member::Named(n) if n == name)) else { return false; };
 if standard == "Clone" {
 matches!(&value.expr, syn::Expr::MethodCall(c) if c.method == "clone" && c.args.is_empty()
 && matches!(&*c.receiver, syn::Expr::Field(f) if matches!(&f.member, syn::Member::Named(n) if n == name)
 && matches!(&*f.base, syn::Expr::Path(p) if p.path.is_ident("self"))))
 } else {
 let syn::Expr::Call(c) = &value.expr else { return false; };
 if !c.args.is_empty() { return false; }
 let syn::Expr::Path(p) = &*c.func else { return false; };
 let path = ctx.path(&p.path);
 matches!(path.as_str(), "Default::default" | "std::default::Default::default" | "core::default::Default::default")
 || matches!(ctx.kind(&field.ty), crate::analysis::evidence::Kind::String) && matches!(path.as_str(), "String::new" | "std::string::String::new")
 || matches!(ctx.kind(&field.ty), crate::analysis::evidence::Kind::Vec) && matches!(path.as_str(), "Vec::new" | "std::vec::Vec::new")
 }
 })
}
