use crate::analysis::source_notes::{SourceNotes, doc_text};
use crate::analysis::{
    detector::Detector,
    evidence::{self, Kind},
};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};
use std::collections::HashMap;
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
        struct Discards<'a> {
            values: HashMap<(usize, usize, usize, usize), (bool, bool)>,
            notes: &'a SourceNotes,
            documented: bool,
            in_drop: bool,
        }
        fn intentional(text: &str) -> bool {
            let text = text.to_ascii_lowercase();
            [
                "best-effort",
                "best effort",
                "non-fatal",
                "swallows",
                "intentionally ignore",
            ]
            .iter()
            .any(|s| text.contains(s))
        }
        impl<'a> Visit<'a> for Discards<'_> {
            fn visit_expr_for_loop(&mut self, n: &'a syn::ExprForLoop) {
                let saved = self.documented;
                self.documented |= intentional(&self.notes.leading(n.span()));
                syn::visit::visit_expr_for_loop(self, n);
                self.documented = saved;
            }
            fn visit_local(&mut self, n: &'a syn::Local) {
                if matches!(n.pat, syn::Pat::Wild(_))
                    && let Some(init) = &n.init
                {
                    self.values.insert(
                        evidence::span_key(init.expr.span()),
                        (
                            self.documented || intentional(&self.notes.leading(n.span())),
                            self.in_drop,
                        ),
                    );
                }
                syn::visit::visit_local(self, n);
            }
            fn visit_item_fn(&mut self, n: &'a syn::ItemFn) {
                let saved = (self.documented, self.in_drop);
                self.documented = intentional(&doc_text(&n.attrs));
                self.in_drop = false;
                syn::visit::visit_item_fn(self, n);
                (self.documented, self.in_drop) = saved;
            }
            fn visit_impl_item_fn(&mut self, n: &'a syn::ImplItemFn) {
                let saved = self.documented;
                self.documented = intentional(&doc_text(&n.attrs));
                syn::visit::visit_impl_item_fn(self, n);
                self.documented = saved;
            }
            fn visit_item_impl(&mut self, n: &'a syn::ItemImpl) {
                let saved = self.in_drop;
                self.in_drop = n.trait_.as_ref().is_some_and(|(p, _)| p.is_ident("Drop"));
                syn::visit::visit_item_impl(self, n);
                self.in_drop = saved;
            }
            fn visit_expr_closure(&mut self, n: &'a syn::ExprClosure) {
                let saved = (self.documented, self.in_drop);
                self.documented = false;
                self.in_drop = false;
                syn::visit::visit_expr_closure(self, n);
                (self.documented, self.in_drop) = saved;
            }
            fn visit_expr_async(&mut self, n: &'a syn::ExprAsync) {
                let saved = (self.documented, self.in_drop);
                self.documented = false;
                self.in_drop = false;
                syn::visit::visit_expr_async(self, n);
                (self.documented, self.in_drop) = saved;
            }
        }
        let notes = SourceNotes::new(&file.code);
        let mut discarded = Discards {
            values: HashMap::new(),
            notes: &notes,
            documented: false,
            in_drop: false,
        };
        discarded.visit_file(&file.ast);
        let mut findings = Vec::new();
        evidence::inspect(&file.ast, |expr, ctx| {
            // Remove the expression position so wrappers cannot accidentally report an inner Result.
            let Some((documented, in_drop)) =
                discarded.values.remove(&evidence::span_key(expr.span()))
            else {
                return;
            };
            let intent = match (documented, in_drop) {
                (true, _) => DiscardIntent::BestEffort,
                (false, true) => DiscardIntent::Destructor,
                (false, false) => DiscardIntent::Undocumented,
            };
            findings.extend(discarded_result_finding(expr, ctx, file, intent));
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

fn discarded_result_finding(
    expr: &syn::Expr,
    ctx: &evidence::Context,
    file: &SourceFile,
    intent: DiscardIntent,
) -> Option<Smell> {
    let p = expr.span().start();
    let cleanup = matches!(expr, syn::Expr::Call(c) if matches!(&*c.func, syn::Expr::Path(p)
    if matches!(ctx.path(&p.path).as_str(), "std::fs::remove_file" | "std::fs::remove_dir")));
    let documented = matches!(intent, DiscardIntent::BestEffort)
        || matches!(intent, DiscardIntent::Destructor) && cleanup;
    let confidence = if matches!(ctx.expr(expr), Kind::Result(_)) {
        Some(if documented {
            FindingConfidence::Low
        } else {
            FindingConfidence::High
        })
    } else if ctx.expr(expr) == Kind::Unknown && heuristic_result(expr, ctx) {
        Some(FindingConfidence::Low)
    } else {
        None
    };
    let confidence = confidence?;
    Some(Smell::new(
        SmellCategory::Idiomaticity,
        UnusedResultDetector.name(),
        Severity::Warning,
        confidence,
        SourceLocation::new(file.path.clone(), p.line, p.line, None),
        if confidence == FindingConfidence::High {
            "A Result is discarded without handling its error"
        } else if documented && matches!(ctx.expr(expr), Kind::Result(_)) {
            "A Result is discarded in destructor or documented best-effort code; review error observability"
        } else {
            "This discarded expression may return a Result; its type is unresolved"
        },
        "Handle the error or document why discarding it is intentional.",
    ))
}

enum DiscardIntent {
    Undocumented,
    BestEffort,
    Destructor,
}
