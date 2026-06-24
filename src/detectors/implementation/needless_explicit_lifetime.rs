use quote::ToTokens;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects simple function signatures where a named lifetime can be elided.
pub struct NeedlessExplicitLifetimeDetector;

impl Detector for NeedlessExplicitLifetimeDetector {
    fn name(&self) -> &str {
        "Needless Explicit Lifetime"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Fn(func) = item
                && func.sig.generics.lifetimes().count() == 1
                && has_one_reference_input(&func.sig.inputs)
                && !lifetime_used_in_bounds(&func.sig.generics)
            {
                let line = func.sig.fn_token.span.start().line;
                smells.push(Smell::new(
                        SmellCategory::Idiomaticity,
                        "Needless Explicit Lifetime",
                        Severity::Info,
                                                crate::domain::smell::FindingConfidence::High,
                        SourceLocation::new(file.path.clone(), line, line, None),
                        format!("Function `{}` appears to use an elidable explicit lifetime", func.sig.ident),
                        "Remove the named lifetime when lifetime elision rules can express the signature.",
                    ));
            }
        }

        smells
    }
}

fn lifetime_used_in_bounds(generics: &syn::Generics) -> bool {
    let Some(lifetime) = generics.lifetimes().next() else {
        return false;
    };
    let lifetime = lifetime.lifetime.ident.to_string();
    let lifetime_token = format!("'{}", lifetime);

    generics.params.iter().any(|param| {
        match param {
            syn::GenericParam::Lifetime(param) => param
                .bounds
                .iter()
                .any(|bound| bound.ident == lifetime),
            syn::GenericParam::Type(param) => param.bounds.iter().any(|bound| {
                matches!(bound, syn::TypeParamBound::Lifetime(bound) if bound.ident == lifetime)
            }),
            syn::GenericParam::Const(_) => false,
        }
    }) || generics.where_clause.as_ref().is_some_and(|where_clause| {
        if where_clause
            .to_token_stream()
            .to_string()
            .contains(&lifetime_token)
        {
            return true;
        }

        where_clause.predicates.iter().any(|predicate| {
            match predicate {
                syn::WherePredicate::Lifetime(predicate) => {
                    predicate.lifetime.ident == lifetime
                        || predicate
                            .bounds
                            .iter()
                            .any(|bound| bound.ident == lifetime)
                }
                syn::WherePredicate::Type(predicate) => predicate.bounds.iter().any(|bound| {
                    matches!(bound, syn::TypeParamBound::Lifetime(bound) if bound.ident == lifetime)
                }),
                _ => false,
            }
        })
    })
}

fn has_one_reference_input(
    inputs: &syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>,
) -> bool {
    inputs
        .iter()
        .filter(|input| matches!(input, syn::FnArg::Typed(pat_ty) if matches!(&*pat_ty.ty, syn::Type::Reference(_))))
        .count()
        == 1
}
