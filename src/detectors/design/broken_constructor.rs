use crate::analysis::detector::Detector;
use crate::detectors::policy::{is_dto_template_or_config_struct, is_test_path};
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects structs with public fields but no constructor (new() function).
///
/// When a struct exposes public fields without a constructor, callers can
/// create invalid states. This breaks encapsulation.
pub struct BrokenConstructorDetector;

impl Detector for BrokenConstructorDetector {
    fn name(&self) -> &str {
        "Broken Constructor"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        if is_test_path(&file.path) {
            return smells;
        }

        // Collect struct info: name, has_pub_fields, has_constructor
        let mut structs: Vec<StructInfo> = Vec::new();
        let mut has_new: std::collections::HashSet<String> = std::collections::HashSet::new();

        for item in &file.ast.items {
            match item {
                syn::Item::Struct(s) if !is_dto_template_or_config_struct(s) => {
                    structs.push(StructInfo::from_struct(s));
                }
                syn::Item::Impl(imp) if provides_constructor(imp) => {
                    if let syn::Type::Path(tp) = &*imp.self_ty
                        && let Some(seg) = tp.path.segments.last()
                    {
                        has_new.insert(seg.ident.to_string());
                    }
                }
                _ => {}
            }
        }

        for s in &structs {
            // Flag structs with all pub fields and no constructor/Default
            if s.all_pub
                && s.field_count >= 3
                && !has_new.contains(&s.id.name)
                && !s.has_default_derive
                && !s.has_phantom_data_field
            {
                smells.push(Smell::new(
                    SmellCategory::Design,
                    "Broken Constructor",
                    Severity::Warning,
                    crate::domain::smell::FindingConfidence::Low,
                    SourceLocation {
                        file: file.path.clone(),
                        line_start: s.id.line,
                        line_end: s.id.line,
                        column: None,
                    },
                    format!(
                        "Struct `{}` has {} public fields but no `new()` constructor",
                        s.id.name, s.field_count
                    ),
                    "Add a constructor to control initialization and validate invariants.",
                ));
            }
        }

        smells
    }
}

#[derive(Debug)]
struct StructIdentity {
    name: String,
    line: usize,
}

#[derive(Debug)]
struct StructInfo {
    id: StructIdentity,
    all_pub: bool,
    field_count: usize,
    has_default_derive: bool,
    has_phantom_data_field: bool,
}

fn line_of_struct(s: &syn::ItemStruct) -> usize {
    match &s.fields {
        syn::Fields::Named(f) => f.brace_token.span.open().start().line,
        syn::Fields::Unnamed(f) => f.paren_token.span.open().start().line,
        syn::Fields::Unit => s.ident.span().start().line,
    }
}

fn has_phantom_data_field(s: &syn::ItemStruct) -> bool {
    match &s.fields {
        syn::Fields::Named(named) => named
            .named
            .iter()
            .any(|field| type_contains_ident(&field.ty, "PhantomData")),
        syn::Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .any(|field| type_contains_ident(&field.ty, "PhantomData")),
        syn::Fields::Unit => false,
    }
}

fn type_contains_ident(ty: &syn::Type, ident: &str) -> bool {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .iter()
            .any(|segment| segment.ident == ident),
        _ => false,
    }
}

impl StructInfo {
    fn from_struct(s: &syn::ItemStruct) -> Self {
        Self {
            id: StructIdentity {
                name: s.ident.to_string(),
                line: line_of_struct(s),
            },
            all_pub: !matches!(s.fields, syn::Fields::Unit)
                && s.fields
                    .iter()
                    .all(|f| matches!(f.vis, syn::Visibility::Public(_))),
            field_count: s.fields.len(),
            has_default_derive: derives_default(&s.attrs),
            has_phantom_data_field: has_phantom_data_field(s),
        }
    }
}

fn derives_default(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("derive")
            && attr
                .parse_args_with(
                    syn::punctuated::Punctuated::<syn::Meta, syn::token::Comma>::parse_terminated,
                )
                .is_ok_and(|nested| nested.iter().any(|m| m.path().is_ident("Default")))
    })
}

fn provides_constructor(imp: &syn::ItemImpl) -> bool {
    if let Some((path, _)) = &imp.trait_ {
        return path.is_ident("Default");
    }
    imp.items.iter().any(|item| {
        let syn::ImplItem::Fn(method) = item else {
            return false;
        };
        let name = method.sig.ident.to_string();
        name == "new"
            || ["from_", "with_", "parse_"]
                .iter()
                .any(|prefix| name.starts_with(prefix))
    })
}
