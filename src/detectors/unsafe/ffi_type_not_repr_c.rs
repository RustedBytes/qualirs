use quote::ToTokens;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects exported types with FFI-shaped names that lack repr(C).
pub struct FfiTypeNotReprCDetector;

impl Detector for FfiTypeNotReprCDetector {
    fn name(&self) -> &str {
        "FFI Type Not repr(C)"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();
        let has_ffi_context = has_ffi_context(file);
        for item in &file.ast.items {
            match item {
                syn::Item::Struct(strukt)
                    if is_public(&strukt.vis)
                        && has_ffi_context
                        && ffi_named(&strukt.ident)
                        && !has_repr_c(&strukt.attrs) =>
                {
                    let line = strukt.struct_token.span.start().line;
                    smells.push(smell(file, line, &strukt.ident.to_string()));
                }
                syn::Item::Enum(enm)
                    if is_public(&enm.vis)
                        && has_ffi_context
                        && ffi_named(&enm.ident)
                        && !has_repr_c(&enm.attrs) =>
                {
                    let line = enm.enum_token.span.start().line;
                    smells.push(smell(file, line, &enm.ident.to_string()));
                }
                _ => {}
            }
        }
        smells
    }
}

fn smell(file: &SourceFile, line: usize, name: &str) -> Smell {
    Smell::new(
        SmellCategory::Unsafe,
        "FFI Type Not repr(C)",
        Severity::Warning,
        crate::domain::smell::FindingConfidence::Medium,
        SourceLocation::new(file.path.clone(), line, line, None),
        format!("FFI-facing type `{name}` lacks #[repr(C)]"),
        "Add #[repr(C)] or avoid exposing the type across an FFI boundary.",
    )
}

fn is_public(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
}

fn has_ffi_context(file: &SourceFile) -> bool {
    path_has_ffi_component(&file.path)
        || file.ast.items.iter().any(|item| match item {
            syn::Item::ForeignMod(foreign_mod) => is_ffi_abi(&foreign_mod.abi),
            _ => has_ffi_export_attr(item_attrs(item)),
        })
}

fn path_has_ffi_component(path: &std::path::Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .map(|component| component.eq_ignore_ascii_case("ffi"))
            .unwrap_or(false)
    })
}

fn is_ffi_abi(abi: &syn::Abi) -> bool {
    abi.name
        .as_ref()
        .map(|name| matches!(name.value().as_str(), "C" | "system"))
        .unwrap_or(false)
}

fn item_attrs(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(item) => &item.attrs,
        syn::Item::Enum(item) => &item.attrs,
        syn::Item::Fn(item) => &item.attrs,
        syn::Item::ForeignMod(item) => &item.attrs,
        syn::Item::Mod(item) => &item.attrs,
        syn::Item::Static(item) => &item.attrs,
        syn::Item::Struct(item) => &item.attrs,
        syn::Item::Type(item) => &item.attrs,
        syn::Item::Union(item) => &item.attrs,
        _ => &[],
    }
}

fn has_ffi_export_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("no_mangle")
            || attr.path().is_ident("export_name")
            || attr.path().is_ident("link_name")
    })
}

fn ffi_named(ident: &syn::Ident) -> bool {
    let name = ident.to_string();
    name.contains("FFI")
        || name.contains("Ffi")
        || name
            .strip_prefix('C')
            .and_then(|rest| rest.chars().next())
            .map(|next| next == '_' || next.is_ascii_uppercase())
            .unwrap_or(false)
}

fn has_repr_c(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("repr") && attr.meta.to_token_stream().to_string().contains('C')
    })
}
