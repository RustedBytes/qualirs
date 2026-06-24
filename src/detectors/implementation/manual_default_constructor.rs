use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects `new()` constructors that simply return default field values.
pub struct ManualDefaultConstructorDetector;

impl Detector for ManualDefaultConstructorDetector {
    fn name(&self) -> &str {
        "Manual Default Constructor"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Impl(imp) = item {
                for impl_item in &imp.items {
                    if let syn::ImplItem::Fn(func) = impl_item
                        && func.sig.ident == "new"
                        && returns_self(&func.sig.output)
                        && has_no_inputs(&func.sig.inputs)
                        && body_is_defaultish(&func.block)
                    {
                        let line = func.sig.fn_token.span.start().line;
                        smells.push(Smell::new(
                                SmellCategory::Idiomaticity,
                                "Manual Default Constructor",
                                Severity::Info,
                                                                crate::domain::smell::FindingConfidence::High,
                                SourceLocation::new(file.path.clone(), line, line, None),
                                "Constructor `new` appears to return only default field values",
                                "Implement or derive Default and delegate `new()` to `Self::default()`.",
                            ));
                    }
                }
            }
        }

        smells
    }
}

fn returns_self(output: &syn::ReturnType) -> bool {
    matches!(output, syn::ReturnType::Type(_, ty) if matches!(&**ty, syn::Type::Path(path) if path.path.is_ident("Self")))
}

fn has_no_inputs(inputs: &syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>) -> bool {
    inputs.is_empty()
}

fn body_is_defaultish(block: &syn::Block) -> bool {
    single_tail_expr(block).is_some_and(is_defaultish_expr)
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
