use syn::spanned::Spanned;
use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects excessively long method chains like `a.b().c().d().e()`.
pub struct LongMethodChainDetector;

impl Detector for LongMethodChainDetector {
    fn name(&self) -> &str {
        "Long Method Chain"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        // Skip test files as they often use long builder patterns or setup chains
        let path_str = file.path.to_string_lossy().to_lowercase();
        if path_str.contains("test") || path_str.contains("spec") {
            return smells;
        }

        for item in &file.ast.items {
            if let syn::Item::Fn(fn_item) = item {
                let mut visitor = ChainVisitor {
                    threshold: thresholds.r#impl.control_flow.long_method_chain,
                    candidates: Vec::new(),
                };
                visitor.visit_block(&fn_item.block);

                // Deduplicate: keep only the longest chain per line
                visitor
                    .candidates
                    .sort_by_key(|candidate| std::cmp::Reverse(candidate.0));
                let mut seen_lines = std::collections::HashSet::new();
                for (depth, line) in visitor.candidates {
                    if seen_lines.insert(line) {
                        smells.push(Smell::new(
                            SmellCategory::Implementation,
                            "Long Method Chain",
                            Severity::Info,
                            crate::domain::smell::FindingConfidence::Medium,
                            SourceLocation {
                                file: file.path.clone(),
                                line_start: line,
                                line_end: line,
                                column: None,
                            },
                            format!(
                                "Function `{}` has a method chain of length {} (threshold: {})",
                                fn_item.sig.ident,
                                depth,
                                thresholds.r#impl.control_flow.long_method_chain
                            ),
                            "Break long chains into intermediate variables with descriptive names.",
                        ));
                    }
                }
            }
        }

        smells
    }
}

struct ChainVisitor {
    threshold: usize,
    candidates: Vec<(usize, usize)>, // (depth, line)
}

impl ChainVisitor {
    fn is_builder_method(ident: &syn::Ident) -> bool {
        let s = ident.to_string();
        matches!(
            s.as_str(),
            "arg"
                | "args"
                | "body"
                | "build"
                | "delete"
                | "env"
                | "envs"
                | "fallback"
                | "fallback_service"
                | "get"
                | "header"
                | "http_only"
                | "id"
                | "label"
                | "layer"
                | "max_age"
                | "merge"
                | "method_not_allowed_fallback"
                | "name"
                | "nest"
                | "on"
                | "patch"
                | "post"
                | "put"
                | "route"
                | "route_layer"
                | "service"
                | "same_site"
                | "secure"
                | "split"
                | "status"
                | "uri"
                | "value"
                | "with"
                | "with_state"
                | "path"
                | "and_then"
                | "or_else"
                | "ok"
                | "map_err"
                | "next"
                | "unwrap_or"
                | "unwrap_or_else"
                | "unwrap_or_default"
                | "trim"
                | "trim_start"
                | "trim_end"
                | "trim_start_matches"
                | "trim_end_matches"
                | "trim_matches"
                | "strip_prefix"
                | "strip_suffix"
                | "to_str"
                | "as_str"
                | "as_ref"
                | "as_deref"
                | "read"
        )
    }

    fn chain_depth(expr: &syn::Expr) -> usize {
        match expr {
            syn::Expr::MethodCall(call) => {
                let depth = Self::chain_depth(&call.receiver);
                if Self::is_builder_method(&call.method) {
                    depth
                } else {
                    1 + depth
                }
            }
            syn::Expr::Field(field) => Self::chain_depth(&field.base),
            syn::Expr::Await(a) => Self::chain_depth(&a.base),
            syn::Expr::Try(t) => Self::chain_depth(&t.expr),
            _ => 0,
        }
    }
}

impl<'ast> Visit<'ast> for ChainVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let receiver_depth = Self::chain_depth(&node.receiver);
        let depth = if Self::is_builder_method(&node.method) {
            receiver_depth
        } else {
            1 + receiver_depth
        };
        if depth > self.threshold {
            let line = node.span().start().line;
            self.candidates.push((depth, line));
        }
        // Don't recurse into the receiver — it would produce shorter chains
        // that we don't care about. Only visit the method arguments.
        for arg in &node.args {
            syn::visit::visit_expr(self, arg);
        }
        // Also visit the turbofish if present
        if let Some(turbofish) = &node.turbofish {
            for arg in &turbofish.args {
                syn::visit::visit_generic_argument(self, arg);
            }
        }
    }
}
