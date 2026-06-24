use quote::ToTokens;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects impls that are often better expressed with derive.
pub struct DerivableImplDetector;

impl Detector for DerivableImplDetector {
    fn name(&self) -> &str {
        "Derivable Impl"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Impl(imp) = item
                && let Some((_, trait_path, _)) = &imp.trait_
                && let Some(trait_ident) = trait_path.segments.last().map(|seg| &seg.ident)
            {
                if !is_derivable_trait(trait_ident)
                    || !is_low_risk_derivable_candidate(imp, trait_ident)
                {
                    continue;
                }
                if trait_ident == "Default" && !is_derived_equivalent_default_impl(imp) {
                    continue;
                }
                let line = imp.impl_token.span.start().line;
                smells.push(derivable_impl_smell(file, trait_ident, line));
            }
        }

        smells
    }
}

fn is_derivable_trait(ident: &syn::Ident) -> bool {
    ident == "Debug"
        || ident == "Clone"
        || ident == "Default"
        || ident == "PartialEq"
        || ident == "Eq"
        || ident == "Hash"
}

fn is_low_risk_derivable_candidate(imp: &syn::ItemImpl, trait_ident: &syn::Ident) -> bool {
    if trait_ident == "Default" {
        return imp.items.len() <= 2;
    }

    if !imp.generics.params.is_empty() || imp.generics.where_clause.is_some() {
        return false;
    }

    if imp.to_token_stream().to_string().contains("# [cfg") {
        return false;
    }

    match trait_ident.to_string().as_str() {
        "Eq" => imp.items.is_empty(),
        "Debug" => has_single_method_named(imp, "fmt"),
        "Clone" => has_single_method_named(imp, "clone"),
        "PartialEq" => has_single_method_named(imp, "eq"),
        "Hash" => has_single_method_named(imp, "hash"),
        _ => false,
    }
}

fn has_single_method_named(imp: &syn::ItemImpl, method_name: &str) -> bool {
    matches!(
        imp.items.as_slice(),
        [syn::ImplItem::Fn(method)] if method.sig.ident == method_name
    )
}

fn derivable_impl_smell(file: &SourceFile, trait_ident: &syn::Ident, line: usize) -> Smell {
    Smell::new(
        SmellCategory::Idiomaticity,
        "Derivable Impl",
        Severity::Info,
        crate::domain::smell::FindingConfidence::High,
        SourceLocation::new(file.path.clone(), line, line, None),
        format!("Manual `{trait_ident}` impl may be derivable"),
        "Prefer #[derive(...)] when the implementation is mechanical.",
    )
}

fn is_derived_equivalent_default_impl(imp: &syn::ItemImpl) -> bool {
    imp.items.iter().any(|item| {
        let syn::ImplItem::Fn(func) = item else {
            return false;
        };
        func.sig.ident == "default"
            && func.sig.inputs.is_empty()
            && returns_self(&func.sig.output)
            && single_tail_expr(&func.block).is_some_and(is_defaultish_expr)
    })
}

fn returns_self(output: &syn::ReturnType) -> bool {
    matches!(output, syn::ReturnType::Type(_, ty) if matches!(&**ty, syn::Type::Path(path) if path.path.is_ident("Self")))
}

fn single_tail_expr(block: &syn::Block) -> Option<&syn::Expr> {
    match block.stmts.as_slice() {
        [syn::Stmt::Expr(expr, None)] => Some(expr),
        _ => None,
    }
}

fn is_defaultish_expr(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Call(call) => is_default_call(&call.func),
        syn::Expr::Struct(strukt) => {
            strukt.path.is_ident("Self")
                && !strukt.fields.is_empty()
                && strukt
                    .fields
                    .iter()
                    .all(|field| is_defaultish_expr(&field.expr))
        }
        _ => false,
    }
}

fn is_default_call(func: &syn::Expr) -> bool {
    let syn::Expr::Path(path) = func else {
        return false;
    };
    let mut segments = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string());
    matches!(
        (
            segments.next().as_deref(),
            segments.next().as_deref(),
            segments.next()
        ),
        (
            Some("Default" | "Self" | "String" | "Vec"),
            Some("default" | "new"),
            None
        )
    )
}
