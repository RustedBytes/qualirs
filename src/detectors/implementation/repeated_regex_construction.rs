use crate::analysis::evidence;
use crate::domain::smell::FindingConfidence;
use std::collections::HashMap;
use syn::{
    spanned::Spanned,
    visit::{
        Visit, visit_expr_call, visit_expr_for_loop, visit_expr_loop, visit_expr_method_call,
        visit_expr_while,
    },
};

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects Regex::new in functions and loops.
pub struct RepeatedRegexConstructionDetector;

impl Detector for RepeatedRegexConstructionDetector {
    fn name(&self) -> &str {
        "Repeated Regex Construction"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut candidates = HashMap::new();
        evidence::inspect(&file.ast, |expr, ctx| {
            let syn::Expr::Call(call) = expr else {
                return;
            };
            let syn::Expr::Path(path) = &*call.func else {
                return;
            };
            let resolved = ctx.path(&path.path);
            let known = matches!(
                resolved.as_str(),
                "regex::Regex::new" | "regex::bytes::Regex::new" | "regex_lite::Regex::new"
            );
            if !known && resolved != "Regex::new" {
                return;
            }
            let literal = matches!(call.args.first(), Some(syn::Expr::Lit(l)) if matches!(l.lit, syn::Lit::Str(_)));
            candidates.insert(
                evidence::span_key(call.span()),
                (
                    ctx.loop_depth > 0,
                    if known && literal {
                        FindingConfidence::High
                    } else {
                        FindingConfidence::Low
                    },
                ),
            );
        });
        let mut visitor = RegexVisitor {
            loop_depth: 0,
            lazy_initializer_depth: 0,
            findings: Vec::new(),
            candidates,
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|(line, in_loop, confidence)| {
                Smell::new(
                    SmellCategory::Performance,
                    "Repeated Regex Construction",
                    if in_loop {
                        Severity::Warning
                    } else {
                        Severity::Info
                    },
                    confidence,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    if confidence == FindingConfidence::High { "A fixed regex pattern is compiled at runtime" } else { "Regex construction may be repeated; pattern reuse is unproven" },
                    if confidence == FindingConfidence::High { "If reused, cache this fixed pattern in LazyLock or OnceLock." } else { "For reused dynamic patterns, consider query-scoped or keyed caching; a single static regex may change behavior." },
                )
            })
            .collect()
    }
}

struct RegexVisitor {
    loop_depth: usize,
    lazy_initializer_depth: usize,
    findings: Vec<(usize, bool, FindingConfidence)>,
    candidates: HashMap<(usize, usize, usize, usize), (bool, FindingConfidence)>,
}

impl<'ast> Visit<'ast> for RegexVisitor {
    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.loop_depth += 1;
        visit_expr_for_loop(self, node);
        self.loop_depth -= 1;
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.loop_depth += 1;
        visit_expr_while(self, node);
        self.loop_depth -= 1;
    }

    fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
        self.loop_depth += 1;
        visit_expr_loop(self, node);
        self.loop_depth -= 1;
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*node.func {
            let path_str = path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            if is_lazy_initializer(&path_str) {
                self.visit_lazy_initializer_args(&node.args);
                return;
            }
            if let Some(&(in_loop, confidence)) =
                self.candidates.get(&evidence::span_key(node.span()))
                && self.lazy_initializer_depth == 0
            {
                let line = path.path.segments.last().unwrap().ident.span().start().line;
                self.findings.push((line, in_loop, confidence));
            }
        }
        visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if is_lazy_initializer_method(&node.method.to_string()) {
            self.visit_lazy_initializer_args(&node.args);
            return;
        }
        visit_expr_method_call(self, node);
    }
}

impl RegexVisitor {
    fn visit_lazy_initializer_args(
        &mut self,
        args: &syn::punctuated::Punctuated<syn::Expr, syn::Token![,]>,
    ) {
        for arg in args {
            if matches!(arg, syn::Expr::Closure(_)) {
                self.lazy_initializer_depth += 1;
                self.visit_expr(arg);
                self.lazy_initializer_depth -= 1;
            } else {
                self.visit_expr(arg);
            }
        }
    }
}

fn is_lazy_initializer(path: &str) -> bool {
    matches!(
        path,
        "LazyLock::new" | "OnceLock::new" | "Lazy::new" | "OnceCell::new"
    ) || path.ends_with("::LazyLock::new")
        || path.ends_with("::OnceLock::new")
        || path.ends_with("::Lazy::new")
        || path.ends_with("::OnceCell::new")
}

fn is_lazy_initializer_method(method: &str) -> bool {
    matches!(method, "get_or_init" | "get_or_try_init")
}
