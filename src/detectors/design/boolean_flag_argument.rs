use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects boolean parameters that often hide multiple execution modes.
pub struct BooleanFlagArgumentDetector;

impl Detector for BooleanFlagArgumentDetector {
    fn name(&self) -> &str {
        "Boolean Flag Argument"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Fn(func) = item {
                let bool_params: Vec<String> = func
                    .sig
                    .inputs
                    .iter()
                    .filter_map(|input| match input {
                        syn::FnArg::Typed(pat_ty) if is_bool(&pat_ty.ty) => {
                            Some(pattern_name(&pat_ty.pat))
                        }
                        _ => None,
                    })
                    .collect();

                let suspicious = bool_params.len() > 1
                    || bool_params.iter().any(|name| {
                        name.starts_with("is_")
                            || name.starts_with("has_")
                            || name.starts_with("use_")
                            || name.starts_with("enable_")
                            || name.starts_with("allow_")
                    });

                if suspicious && !bool_params.is_empty() {
                    let line = func.sig.fn_token.span.start().line;
                    smells.push(Smell::new(
                        SmellCategory::Design,
                        "Boolean Flag Argument",
                        Severity::Info,
                                                crate::domain::smell::FindingConfidence::Medium,
                        SourceLocation::new(file.path.clone(), line, line, None),
                        format!(
                            "Function `{}` takes boolean flag argument(s): {}",
                            func.sig.ident,
                            bool_params.join(", ")
                        ),
                        "Use an enum, options struct, or separate functions to make modes explicit.",
                    ));
                }
            }
        }

        smells
    }
}

fn is_bool(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(path) if path.path.is_ident("bool"))
}

fn pattern_name(pat: &syn::Pat) -> String {
    match pat {
        syn::Pat::Ident(ident) => ident.ident.to_string(),
        _ => "_".to_string(),
    }
}
