use std::collections::HashMap;

use syn::visit::{Visit, visit_expr_method_call, visit_item_fn};

use crate::analysis::detector::Detector;
use crate::detectors::implementation::perf_utils::expr_path_tail;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects sorting a collection only to read its first or last element.
pub struct SortBeforeMinMaxDetector;

impl Detector for SortBeforeMinMaxDetector {
    fn name(&self) -> &str {
        "Sort Before Min or Max"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = SortMinMaxVisitor {
            sorted: HashMap::new(),
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|(line, receiver, accessor)| {
                let replacement = if accessor == "first" { "min" } else { "max" };
                Smell::new(
                    SmellCategory::Performance,
                    "Sort Before Min or Max",
                    Severity::Warning,
                                        crate::domain::smell::FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    format!("`{receiver}` is sorted before only reading `{accessor}()`"),
                    format!("Use `iter().{replacement}()` or `iter().{replacement}_by_key(...)` when the full ordering is not needed."),
                )
            })
            .collect()
    }
}

struct SortMinMaxVisitor {
    sorted: HashMap<String, usize>,
    findings: Vec<(usize, String, String)>,
}

impl<'ast> Visit<'ast> for SortMinMaxVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let previous = std::mem::take(&mut self.sorted);
        visit_item_fn(self, node);
        self.sorted = previous;
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        let receiver = expr_path_tail(&node.receiver);

        if let Some(receiver) = receiver.as_ref() {
            if is_sort_method(&method) {
                self.sorted
                    .insert(receiver.clone(), node.method.span().start().line);
            } else if matches!(method.as_str(), "first" | "last")
                && self.sorted.remove(receiver).is_some()
            {
                self.findings.push((
                    node.method.span().start().line,
                    receiver.clone(),
                    method.clone(),
                ));
            } else if method_uses_order_or_mutates(&method) {
                self.sorted.remove(receiver);
            }
        }

        visit_expr_method_call(self, node);
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
