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
                syn::Item::Struct(s) => {
                    if is_dto_template_or_config_struct(s) {
                        continue;
                    }
                    let all_pub = match &s.fields {
                        syn::Fields::Named(named) => named
                            .named
                            .iter()
                            .all(|f| matches!(f.vis, syn::Visibility::Public(_))),
                        syn::Fields::Unnamed(unnamed) => unnamed
                            .unnamed
                            .iter()
                            .all(|f| matches!(f.vis, syn::Visibility::Public(_))),
                        syn::Fields::Unit => false,
                    };
                    let field_count = match &s.fields {
                        syn::Fields::Named(named) => named.named.len(),
                        syn::Fields::Unnamed(unnamed) => unnamed.unnamed.len(),
                        syn::Fields::Unit => 0,
                    };
                    let has_default_derive = s.attrs.iter().any(|attr| {
                        if attr.path().is_ident("derive") {
                            if let Ok(nested) = attr.parse_args_with(syn::punctuated::Punctuated::<syn::Meta, syn::token::Comma>::parse_terminated) {
                                nested.iter().any(|m| m.path().is_ident("Default"))
                            } else { false }
                        } else { false }
                    });

                    structs.push(StructInfo {
                        id: StructIdentity {
                            name: s.ident.to_string(),
                            line: line_of_struct(s),
                        },
                        all_pub,
                        field_count,
                        has_default_derive,
                        has_phantom_data_field: has_phantom_data_field(s),
                    });
                }
                syn::Item::Impl(imp) => {
                    if let syn::Type::Path(tp) = &*imp.self_ty
                        && let Some(seg) = tp.path.segments.last()
                    {
                        let type_name = seg.ident.to_string();

                        // Check for new() or other constructors
                        if imp.trait_.is_none() {
                            for item in &imp.items {
                                if let syn::ImplItem::Fn(method) = item {
                                    let method_name = method.sig.ident.to_string();
                                    if method_name == "new"
                                        || method_name.starts_with("from_")
                                        || method_name.starts_with("with_")
                                        || method_name.starts_with("parse_")
                                    {
                                        has_new.insert(type_name.clone());
                                    }
                                }
                            }
                        } else if let Some((_, path, _)) = &imp.trait_ {
                            // Check for impl Default
                            if path.is_ident("Default") {
                                has_new.insert(type_name.clone());
                            }
                        }
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
