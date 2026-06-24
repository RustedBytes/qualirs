use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects functions that access fields/methods of another type more than their own.
///
/// A function has "feature envy" if it calls methods on a parameter type more often
/// than on Self. This suggests the function belongs on the other type.
pub struct FeatureEnvyDetector;

impl Detector for FeatureEnvyDetector {
    fn name(&self) -> &str {
        "Feature Envy"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Fn(fn_item) = item {
                // Skip methods (in impl blocks, handled separately)
                if let syn::Visibility::Inherited = fn_item.vis {
                    continue;
                }

                let params: Vec<String> = fn_item
                    .sig
                    .inputs
                    .iter()
                    .filter_map(|arg| {
                        if let syn::FnArg::Typed(pat_type) = arg
                            && let syn::Pat::Ident(pat_ident) = &*pat_type.pat
                        {
                            return Some(pat_ident.ident.to_string());
                        }
                        None
                    })
                    .collect();

                if params.is_empty() {
                    continue;
                }

                let mut visitor = MethodCallVisitor {
                    param_calls: std::collections::HashMap::new(),
                    params,
                };
                visitor.visit_item_fn(fn_item);

                // Check if any parameter is accessed more than self
                let max_calls = visitor.param_calls.values().max().copied().unwrap_or(0);
                let self_calls = visitor.param_calls.get("self").copied().unwrap_or(0);

                if max_calls > 5 && max_calls > self_calls * 2 {
                    let envied = visitor
                        .param_calls
                        .iter()
                        .filter(|(_, c)| **c == max_calls)
                        .map(|(k, _)| k.clone())
                        .next()
                        .unwrap_or_default();

                    let start = fn_item.block.brace_token.span.open().start().line;
                    let end = fn_item.block.brace_token.span.close().start().line;

                    smells.push(Smell::new(
                        SmellCategory::Design,
                        "Feature Envy",
                        Severity::Info,
                                                crate::domain::smell::FindingConfidence::Low,
                        SourceLocation {
                            file: file.path.clone(),
                            line_start: start,
                            line_end: end,
                            column: None,
                        },
                        format!(
                            "Function `{}` calls methods on `{}` {} times — consider moving this method",
                            fn_item.sig.ident, envied, max_calls
                        ),
                        "Move this function to the type it is most interested in (Use Move refactoring).",
                    ));
                }
            }
        }

        smells
    }
}

struct MethodCallVisitor {
    param_calls: std::collections::HashMap<String, usize>,
    params: Vec<String>,
}

impl<'ast> Visit<'ast> for MethodCallVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if let syn::Expr::Path(expr_path) = &*node.receiver
            && let Some(ident) = expr_path.path.get_ident()
        {
            let name = ident.to_string();
            if self.params.contains(&name) || name == "self" {
                *self.param_calls.entry(name).or_insert(0) += 1;
            }
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}
