use crate::analysis::{
    detector::Detector,
    evidence::{self, Kind},
};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};
use std::collections::HashSet;
use syn::{spanned::Spanned, visit::Visit};
pub struct UnusedResultDetector;
impl Detector for UnusedResultDetector {
    fn name(&self) -> &str {
        "Unused Result Ignored"
    }
    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        if crate::detectors::policy::is_test_path(&file.path) {
            return Vec::new();
        }
        struct Discards(HashSet<(usize, usize, usize, usize)>);
        impl<'a> Visit<'a> for Discards {
            fn visit_local(&mut self, n: &'a syn::Local) {
                if matches!(n.pat, syn::Pat::Wild(_))
                    && let Some(init) = &n.init
                {
                    self.0.insert(evidence::span_key(init.expr.span()));
                }
                syn::visit::visit_local(self, n);
            }
        }
        let mut discarded = Discards(HashSet::new());
        discarded.visit_file(&file.ast);
        let mut findings = Vec::new();
        evidence::inspect(&file.ast, |expr, ctx| {
            let p = expr.span().start();
            // Remove the expression position so wrappers cannot accidentally report an inner Result.
            if !discarded.0.remove(&evidence::span_key(expr.span())) {
                return;
            }
            let confidence = if matches!(ctx.expr(expr), Kind::Result(_)) {
                Some(FindingConfidence::High)
            } else if ctx.expr(expr) == Kind::Unknown && heuristic_result(expr, ctx) {
                Some(FindingConfidence::Low)
            } else {
                None
            };
            if let Some(confidence) = confidence {
                findings.push(Smell::new(
                    SmellCategory::Idiomaticity,
                    self.name(),
                    Severity::Warning,
                    confidence,
                    SourceLocation::new(file.path.clone(), p.line, p.line, None),
                    if confidence == FindingConfidence::High {
                        "A Result is discarded without handling its error"
                    } else {
                        "This discarded expression may return a Result; its type is unresolved"
                    },
                    "Handle the error or document why discarding it is intentional.",
                ));
            }
        });
        findings
    }
}
fn heuristic_result(expr: &syn::Expr, ctx: &evidence::Context) -> bool {
    match expr {
        syn::Expr::MethodCall(c) => {
            let method = c.method.to_string();
            // A handled Result produces its success value, not another Result by default.
            if matches!(method.as_str(), "unwrap" | "expect") {
                return false;
            }
            // Preserve explicit best-effort channel sends as intentional.
            if method == "send" {
                return false;
            }
            method.starts_with("try_")
                || method.ends_with("_result")
                || matches!(
                    method.as_str(),
                    "write" | "write_all" | "flush" | "read" | "read_to_end" | "read_to_string"
                )
        }
        syn::Expr::Call(c) => {
            if let syn::Expr::Path(p) = &*c.func {
                let path = ctx.path(&p.path);
                let name = path.rsplit("::").next().unwrap_or("");
                name.starts_with("try_") || name.ends_with("_result")
            } else {
                false
            }
        }
        syn::Expr::Macro(m) if m.mac.path.is_ident("write") || m.mac.path.is_ident("writeln") => {
            let args = m.mac.parse_body_with(
                syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated,
            );
            !args
                .ok()
                .and_then(|a| a.first().map(|e| *ctx.expr(e).value() == Kind::String))
                .unwrap_or(false)
        }
        _ => false,
    }
}
