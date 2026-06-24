use std::collections::HashMap;

use syn::visit::{Visit, visit_expr_index, visit_expr_method_call, visit_item_fn};

use crate::analysis::detector::Detector;
use crate::detectors::implementation::perf_utils::{expr_path_tail, int_lit_value};
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects full sorts followed by reading one indexed element.
pub struct FullSortForSingleElementDetector;

impl Detector for FullSortForSingleElementDetector {
    fn name(&self) -> &str {
        "Full Sort for Single Element"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = FullSortVisitor {
            sorted: HashMap::new(),
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|(line, receiver)| {
                Smell::new(
                    SmellCategory::Performance,
                    "Full Sort for Single Element",
                    Severity::Info,
                                        crate::domain::smell::FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    format!("`{receiver}` is fully sorted before selecting one indexed element"),
                    "Use `select_nth_unstable` when only one rank is needed and the rest of the order is irrelevant.",
                )
            })
            .collect()
    }
}

struct FullSortVisitor {
    sorted: HashMap<String, usize>,
    findings: Vec<(usize, String)>,
}

impl<'ast> Visit<'ast> for FullSortVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let previous = std::mem::take(&mut self.sorted);
        visit_item_fn(self, node);
        self.sorted = previous;
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if let Some(receiver) = expr_path_tail(&node.receiver) {
            if is_sort_method(&method) {
                self.sorted
                    .insert(receiver, node.method.span().start().line);
            } else if method == "get"
                && node
                    .args
                    .first()
                    .is_none_or(|arg| int_lit_value(arg) != Some(0))
                && self.sorted.remove(&receiver).is_some()
            {
                self.findings
                    .push((node.method.span().start().line, receiver));
            } else if method_uses_order_or_mutates(&method) {
                self.sorted.remove(&receiver);
            }
        }

        visit_expr_method_call(self, node);
    }

    fn visit_expr_index(&mut self, node: &'ast syn::ExprIndex) {
        if let Some(receiver) = expr_path_tail(&node.expr)
            && int_lit_value(&node.index) != Some(0)
            && self.sorted.remove(&receiver).is_some()
        {
            self.findings
                .push((node.bracket_token.span.open().start().line, receiver));
        }

        visit_expr_index(self, node);
    }
}

fn is_sort_method(method: &str) -> bool {
    matches!(
        method,
        "sort"
            | "sort_unstable"
            | "sort_by"
            | "sort_by_key"
            | "sort_unstable_by"
            | "sort_unstable_by_key"
    )
}

fn method_uses_order_or_mutates(method: &str) -> bool {
    matches!(
        method,
        "iter"
            | "into_iter"
            | "binary_search"
            | "binary_search_by"
            | "binary_search_by_key"
            | "dedup"
            | "reverse"
            | "push"
            | "insert"
            | "remove"
            | "retain"
            | "truncate"
            | "clear"
            | "extend"
    )
}
