use std::collections::HashSet;

use syn::punctuated::Punctuated;
use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::detectors::policy::is_test_path;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects `let _ = expr()` where the expression returns a `Result` or `Option`.
///
/// Silently discarding Results is a common source of hidden bugs.
pub struct UnusedResultDetector;

impl Detector for UnusedResultDetector {
    fn name(&self) -> &str {
        "Unused Result Ignored"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        if is_test_path(&file.path) {
            return smells;
        }

        for item in &file.ast.items {
            if let syn::Item::Fn(fn_item) = item {
                let mut visitor = UnusedResultVisitor {
                    findings: Vec::new(),
                    string_writers: collect_string_writer_params(&fn_item.sig),
                };
                visitor.visit_block(&fn_item.block);

                for (line, expr_desc) in visitor.findings {
                    smells.push(Smell::new(
                        SmellCategory::Idiomaticity,
                        "Unused Result Ignored",
                        Severity::Warning,
                        crate::domain::smell::FindingConfidence::High,
                        SourceLocation {
                            file: file.path.clone(),
                            line_start: line,
                            line_end: line,
                            column: None,
                        },
                        format!(
                            "Function `{}` discards a Result/Option with `let _ = ...` ({})",
                            fn_item.sig.ident, expr_desc
                        ),
                        "Handle the error explicitly with match, if let, or propagate with ?.",
                    ));
                }
            }
        }

        smells
    }
}

struct UnusedResultVisitor {
    findings: Vec<(usize, String)>,
    string_writers: HashSet<String>,
}

impl<'ast> Visit<'ast> for UnusedResultVisitor {
    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let Some(name) = local_string_writer_binding(local) {
            self.string_writers.insert(name);
        }

        // Check for `let _ = expr;` pattern
        if let syn::Pat::Wild(wild) = &local.pat
            && let Some(init) = &local.init
            && !is_intentional_discard(&init.expr, &self.string_writers)
            && expr_looks_result_like(&init.expr)
        {
            let description = describe_expr(&init.expr);
            let line = wild.underscore_token.span.start().line;
            self.findings.push((line, description));
        }
        syn::visit::visit_local(self, local);
    }
}

fn collect_string_writer_params(sig: &syn::Signature) -> HashSet<String> {
    sig.inputs
        .iter()
        .filter_map(|input| match input {
            syn::FnArg::Typed(arg) if type_is_string_writer(&arg.ty) => pat_ident(&arg.pat),
            _ => None,
        })
        .collect()
}

fn local_string_writer_binding(local: &syn::Local) -> Option<String> {
    let init = local.init.as_ref()?;
    if is_string_constructor(&init.expr) {
        pat_ident(&local.pat)
    } else {
        None
    }
}

fn is_intentional_discard(expr: &syn::Expr, string_writers: &HashSet<String>) -> bool {
    is_infallible_string_write(expr, string_writers) || is_channel_send_discard(expr)
}

fn is_infallible_string_write(expr: &syn::Expr, string_writers: &HashSet<String>) -> bool {
    let syn::Expr::Macro(expr_macro) = expr else {
        return false;
    };
    if !expr_macro.mac.path.is_ident("write") && !expr_macro.mac.path.is_ident("writeln") {
        return false;
    }

    macro_first_expr_ident(&expr_macro.mac)
        .is_some_and(|target| string_writers.contains(target.as_str()))
}

fn is_channel_send_discard(expr: &syn::Expr) -> bool {
    let syn::Expr::MethodCall(call) = expr else {
        return false;
    };
    call.method == "send"
        && receiver_path_tail(&call.receiver).is_some_and(|name| {
            name == "sender" || name == "tx" || name.ends_with("_sender") || name.ends_with("_tx")
        })
}

fn expr_looks_result_like(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Call(call) => call_looks_result_like(call),
        syn::Expr::MethodCall(call) => method_call_looks_result_like(call),
        syn::Expr::Macro(expr_macro) => macro_looks_result_like(&expr_macro.mac),
        syn::Expr::Try(_) | syn::Expr::Await(_) => true,
        syn::Expr::Paren(paren) => expr_looks_result_like(&paren.expr),
        syn::Expr::Reference(reference) => expr_looks_result_like(&reference.expr),
        _ => false,
    }
}

fn call_looks_result_like(call: &syn::ExprCall) -> bool {
    let name = extract_path_string(&call.func);
    let tail = name.rsplit("::").next().unwrap_or(name.as_str());

    name.starts_with("std::fs::")
        || name.starts_with("tokio::fs::")
        || name.starts_with("std::io::")
        || tail.starts_with("try_")
        || tail.ends_with("_result")
        || matches!(
            tail,
            "open"
                | "create"
                | "remove_file"
                | "read"
                | "read_to_string"
                | "read_to_end"
                | "write"
                | "write_all"
                | "flush"
                | "rename"
                | "copy"
                | "metadata"
                | "canonicalize"
        )
}

fn method_call_looks_result_like(call: &syn::ExprMethodCall) -> bool {
    let method = call.method.to_string();
    method.starts_with("try_")
        || method.ends_with("_result")
        || matches!(
            method.as_str(),
            "unwrap"
                | "expect"
                | "send"
                | "write"
                | "write_all"
                | "flush"
                | "read"
                | "read_to_string"
                | "read_to_end"
        )
}

fn macro_looks_result_like(mac: &syn::Macro) -> bool {
    mac.path.is_ident("write") || mac.path.is_ident("writeln")
}

fn macro_first_expr_ident(mac: &syn::Macro) -> Option<String> {
    let args = mac
        .parse_body_with(Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated)
        .ok()?;
    expr_ident(args.first()?)
}

fn expr_ident(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) => path.path.get_ident().map(ToString::to_string),
        syn::Expr::Reference(reference) => expr_ident(&reference.expr),
        syn::Expr::Paren(paren) => expr_ident(&paren.expr),
        _ => None,
    }
}

fn receiver_path_tail(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        syn::Expr::Field(field) => match &field.member {
            syn::Member::Named(name) => Some(name.to_string()),
            syn::Member::Unnamed(_) => None,
        },
        syn::Expr::Reference(reference) => receiver_path_tail(&reference.expr),
        _ => None,
    }
}

fn pat_ident(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(ident) => Some(ident.ident.to_string()),
        _ => None,
    }
}

fn type_is_string_writer(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => path.path.is_ident("String"),
        syn::Type::Reference(reference) => type_is_string_writer(&reference.elem),
        _ => false,
    }
}

fn is_string_constructor(expr: &syn::Expr) -> bool {
    let syn::Expr::Call(call) = expr else {
        return false;
    };
    let syn::Expr::Path(path) = &*call.func else {
        return false;
    };
    let mut segments = path.path.segments.iter().rev();
    matches!(
        (segments.next(), segments.next()),
        (Some(method), Some(receiver))
            if receiver.ident == "String"
                && matches!(method.ident.to_string().as_str(), "new" | "with_capacity")
    )
}

fn describe_expr(expr: &syn::Expr) -> String {
    match expr {
        syn::Expr::Call(call) => {
            let func_name = extract_path_string(&call.func);
            format!("call to `{}`", func_name)
        }
        syn::Expr::MethodCall(call) => {
            format!("`.{}()` call", call.method)
        }
        syn::Expr::Path(path) => {
            format!("`{}`", path_to_string(&path.path))
        }
        _ => String::from("expression"),
    }
}

fn extract_path_string(expr: &syn::Expr) -> String {
    if let syn::Expr::Path(p) = expr {
        path_to_string(&p.path)
    } else {
        String::from("...")
    }
}

fn path_to_string(path: &syn::Path) -> String {
    let idents: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
    idents.join("::")
}
