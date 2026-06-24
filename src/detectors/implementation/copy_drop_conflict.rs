use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects types that implement both `Copy` and `Drop`.
///
/// Implementing `Copy` on a type with a custom `Drop` impl is almost always
/// a mistake: the destructor will run on both the original and the copy,
/// potentially causing double-free or resource leaks.
pub struct CopyDropConflictDetector;

impl Detector for CopyDropConflictDetector {
    fn name(&self) -> &str {
        "Copy + Drop Conflict"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        let drop_types = collect_drop_types(&file.ast);
        let copy_types = collect_copy_types(&file.ast);

        // Find intersection
        for copy_type in &copy_types {
            if drop_types.iter().any(|d| d.name == copy_type.name) {
                smells.push(conflict_smell(file, copy_type));
            }
        }

        smells
    }
}

#[derive(PartialEq)]
struct TypeInfo<'ast> {
    name: &'ast syn::Ident,
    line: usize,
}

fn conflict_smell(file: &SourceFile, copy_type: &TypeInfo<'_>) -> Smell {
    let line = copy_type.line;
    Smell::new(
        SmellCategory::Idiomaticity,
        "Copy + Drop Conflict",
        Severity::Critical,
        crate::domain::smell::FindingConfidence::High,
        SourceLocation {
            file: file.path.clone(),
            line_start: line,
            line_end: line,
            column: None,
        },
        format!(
            "Type `{}` implements both Copy and Drop — destructor runs on every copy",
            copy_type.name
        ),
        "Remove Copy or remove Drop. If you need custom cleanup, the type should not be Copy.",
    )
}

fn collect_drop_types(ast: &syn::File) -> Vec<TypeInfo<'_>> {
    let mut drop_types = Vec::with_capacity(ast.items.len());
    for item in &ast.items {
        if let syn::Item::Impl(imp) = item
            && let Some((_, trait_path, _)) = &imp.trait_
            && is_trait(trait_path, "Drop")
            && let Some(name) = extract_impl_target_name(&imp.self_ty)
        {
            drop_types.push(TypeInfo {
                name,
                line: imp.impl_token.span.start().line,
            });
        }
    }
    drop_types
}

fn collect_copy_types(ast: &syn::File) -> Vec<TypeInfo<'_>> {
    let mut copy_types = Vec::with_capacity(ast.items.len());

    for item in &ast.items {
        match item {
            syn::Item::Struct(s) if has_derive_copy(&s.attrs) => {
                copy_types.push(TypeInfo {
                    name: &s.ident,
                    line: s.struct_token.span.start().line,
                });
            }
            syn::Item::Enum(e) if has_derive_copy(&e.attrs) => {
                copy_types.push(TypeInfo {
                    name: &e.ident,
                    line: e.enum_token.span.start().line,
                });
            }
            syn::Item::Impl(imp) if imp.trait_.is_none() => {
                // Check for manual Copy impl within the inherent impl block
                // This is actually done via trait impl, not inherent, so skip
            }
            _ => {}
        }
    }

    copy_types
}

fn has_derive_copy(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("derive") {
            return false;
        }

        let mut found = false;
        attr.parse_nested_meta(|meta| {
            if is_trait(&meta.path, "Copy") {
                found = true;
            }
            Ok(())
        })
        .is_ok()
            && found
    })
}

fn is_trait(path: &syn::Path, name: &str) -> bool {
    path.segments
        .last()
        .map(|s| s.ident == name)
        .unwrap_or(false)
}

fn extract_impl_target_name(ty: &syn::Type) -> Option<&syn::Ident> {
    if let syn::Type::Path(tp) = ty {
        tp.path.segments.first().map(|s| &s.ident)
    } else {
        None
    }
}
