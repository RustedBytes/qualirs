use syn::spanned::Spanned;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

const LARGE_ARRAY_LENGTH: usize = 32;

/// Detects large arrays and containers passed by value.
pub struct LargeValuePassedByValueDetector;

impl Detector for LargeValuePassedByValueDetector {
    fn name(&self) -> &str {
        "Large Value Passed By Value"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();
        for item in &file.ast.items {
            if let syn::Item::Fn(func) = item {
                for input in &func.sig.inputs {
                    if let syn::FnArg::Typed(arg) = input
                        && is_large_by_value(&arg.ty)
                    {
                        let line = arg.pat.span().start().line;
                        smells.push(Smell::new(
                            SmellCategory::Performance,
                            "Large Value Passed By Value",
                            Severity::Info,
                            crate::domain::smell::FindingConfidence::Medium,
                            SourceLocation::new(file.path.clone(), line, line, None),
                            format!(
                                "Function `{}` takes a potentially large value by value",
                                func.sig.ident
                            ),
                            "Take a reference or a slice unless ownership is required.",
                        ));
                    }
                }
            }
        }
        smells
    }
}

fn is_large_by_value(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Array(arr) => match &arr.len {
            syn::Expr::Lit(lit) => match &lit.lit {
                syn::Lit::Int(int) => int
                    .base10_parse::<usize>()
                    .map(|n| n > LARGE_ARRAY_LENGTH)
                    .unwrap_or(false),
                _ => false,
            },
            _ => false,
        },
        syn::Type::Tuple(tuple) => tuple.elems.iter().any(is_large_by_value),
        _ => false,
    }
}
