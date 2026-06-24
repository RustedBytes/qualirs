use std::collections::HashSet;

use syn::visit::{
    Visit, visit_arm, visit_expr_closure, visit_expr_for_loop, visit_expr_loop,
    visit_expr_method_call, visit_expr_struct, visit_expr_while, visit_item_fn, visit_item_mod,
    visit_local,
};

use crate::analysis::detector::Detector;
use crate::detectors::implementation::perf_utils::{collect_pat_idents, expr_contains_any_ident};
use crate::detectors::policy::has_test_cfg;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects repeated string conversions in loops and iterator adapter closures.
pub struct RepeatedStringConversionDetector;

impl Detector for RepeatedStringConversionDetector {
    fn name(&self) -> &str {
        "Repeated String Conversion in Hot Path"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = StringConversionVisitor {
            loop_depth: 0,
            iterator_closure_depth: 0,
            pending_iterator_closure: 0,
            struct_literal_depth: 0,
            hot_bindings: Vec::new(),
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        let mut seen = HashSet::new();
        visitor
            .findings
            .into_iter()
            .filter(|finding| seen.insert(finding.clone()))
            .map(|(line, method)| {
                Smell::new(
                    SmellCategory::Performance,
                    "Repeated String Conversion in Hot Path",
                    Severity::Info,
                                        crate::domain::smell::FindingConfidence::Medium,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    format!("Repeated `.{method}()` allocation appears in a loop or iterator chain"),
                    "Keep values borrowed where possible, or delay formatting until an owned String is actually needed.",
                )
            })
            .collect()
    }
}

struct StringConversionVisitor {
    loop_depth: usize,
    iterator_closure_depth: usize,
    pending_iterator_closure: usize,
    struct_literal_depth: usize,
    hot_bindings: Vec<HashSet<String>>,
    findings: Vec<(usize, String)>,
}

impl<'ast> Visit<'ast> for StringConversionVisitor {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if has_test_cfg(&node.attrs) {
            return;
        }
        visit_item_mod(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if has_test_cfg(&node.attrs) {
            return;
        }
        visit_item_fn(self, node);
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        let mut bindings = HashSet::new();
        collect_pat_idents(&node.pat, &mut bindings);
        self.loop_depth += 1;
        self.hot_bindings.push(bindings);
        visit_expr_for_loop(self, node);
        self.hot_bindings.pop();
        self.loop_depth -= 1;
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.loop_depth += 1;
        self.hot_bindings.push(HashSet::new());
        visit_expr_while(self, node);
        self.hot_bindings.pop();
        self.loop_depth -= 1;
    }

    fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
        self.loop_depth += 1;
        self.hot_bindings.push(HashSet::new());
        visit_expr_loop(self, node);
        self.hot_bindings.pop();
        self.loop_depth -= 1;
    }

    fn visit_arm(&mut self, node: &'ast syn::Arm) {
        if self.in_hot_path() {
            let mut bindings = HashSet::new();
            collect_pat_idents(&node.pat, &mut bindings);
            self.hot_bindings.push(bindings);
            visit_arm(self, node);
            self.hot_bindings.pop();
        } else {
            visit_arm(self, node);
        }
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        visit_local(self, node);
        if self.in_hot_path() {
            self.add_hot_pattern(&node.pat);
        }
    }

    fn visit_expr_let(&mut self, node: &'ast syn::ExprLet) {
        self.visit_expr(&node.expr);
        if self.in_hot_path() {
            self.add_hot_pattern(&node.pat);
        }
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if self.in_hot_path() && is_error_mapping_method(&method) {
            self.visit_expr(&node.receiver);
            return;
        }

        let is_hot_conversion = matches!(method.as_str(), "to_string" | "to_owned")
            && (self.loop_depth > 0 || self.iterator_closure_depth > 0);

        if is_hot_conversion
            && self.struct_literal_depth == 0
            && !receiver_is_string_literal(&node.receiver)
            && !self.receiver_depends_on_hot_binding(&node.receiver)
        {
            self.findings
                .push((node.method.span().start().line, method.clone()));
        }

        if is_iterator_adapter(&method) {
            self.pending_iterator_closure += 1;
            for arg in &node.args {
                self.visit_expr(arg);
            }
            self.pending_iterator_closure -= 1;
            self.visit_expr(&node.receiver);
            return;
        }

        visit_expr_method_call(self, node);
    }

    fn visit_expr_closure(&mut self, node: &'ast syn::ExprClosure) {
        if self.pending_iterator_closure > 0 {
            let mut bindings = HashSet::new();
            for input in &node.inputs {
                collect_pat_idents(input, &mut bindings);
            }
            self.iterator_closure_depth += 1;
            self.hot_bindings.push(bindings);
            visit_expr_closure(self, node);
            self.hot_bindings.pop();
            self.iterator_closure_depth -= 1;
        } else {
            visit_expr_closure(self, node);
        }
    }

    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        self.struct_literal_depth += 1;
        visit_expr_struct(self, node);
        self.struct_literal_depth -= 1;
    }
}

impl StringConversionVisitor {
    #[inline]
    fn in_hot_path(&self) -> bool {
        self.loop_depth > 0 || self.iterator_closure_depth > 0
    }

    fn add_hot_pattern(&mut self, pat: &syn::Pat) {
        if let Some(bindings) = self.hot_bindings.last_mut() {
            collect_pat_idents(pat, bindings);
        }
    }

    fn receiver_depends_on_hot_binding(&self, expr: &syn::Expr) -> bool {
        let bindings = self
            .hot_bindings
            .iter()
            .flat_map(|bindings| bindings.iter().cloned())
            .collect::<HashSet<_>>();
        expr_contains_any_ident(expr, &bindings)
    }
}

fn is_iterator_adapter(method: &str) -> bool {
    matches!(
        method,
        "map"
            | "filter_map"
            | "flat_map"
            | "for_each"
            | "fold"
            | "try_fold"
            | "reduce"
            | "scan"
            | "partition"
            | "any"
            | "all"
            | "find"
            | "position"
    )
}

fn is_error_mapping_method(method: &str) -> bool {
    matches!(method, "map_err" | "ok_or_else")
}

fn receiver_is_string_literal(expr: &syn::Expr) -> bool {
    matches!(expr, syn::Expr::Lit(lit) if matches!(lit.lit, syn::Lit::Str(_)))
}
