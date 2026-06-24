use std::collections::HashSet;

use syn::visit::{
    Visit, visit_arm, visit_expr_for_loop, visit_expr_loop, visit_expr_while, visit_item_fn,
    visit_item_mod, visit_local,
};

use crate::analysis::detector::Detector;
use crate::detectors::implementation::perf_utils::{
    collect_pat_idents, expr_contains_any_ident, macro_mentions_any_ident, path_to_string,
};
use crate::detectors::policy::has_test_cfg;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects allocation-heavy calls inside loops.
pub struct UnnecessaryAllocationInLoopDetector;

impl Detector for UnnecessaryAllocationInLoopDetector {
    fn name(&self) -> &str {
        "Unnecessary Allocation in Loop"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = AllocationLoopVisitor {
            loop_depth: 0,
            loop_bindings: Vec::new(),
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|(line, call)| {
                Smell::new(
                    SmellCategory::Performance,
                    "Unnecessary Allocation in Loop",
                    Severity::Info,
                    crate::domain::smell::FindingConfidence::Medium,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    format!("Allocation-like call `{call}` appears inside a loop"),
                    "Move reusable allocation outside the loop or borrow data where possible.",
                )
            })
            .collect()
    }
}

struct AllocationLoopVisitor {
    loop_depth: usize,
    loop_bindings: Vec<HashSet<String>>,
    findings: Vec<(usize, String)>,
}

impl<'ast> Visit<'ast> for AllocationLoopVisitor {
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
        self.loop_bindings.push(bindings);
        visit_expr_for_loop(self, node);
        self.loop_bindings.pop();
        self.loop_depth -= 1;
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.loop_depth += 1;
        self.loop_bindings.push(HashSet::new());
        visit_expr_while(self, node);
        self.loop_bindings.pop();
        self.loop_depth -= 1;
    }

    fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
        self.loop_depth += 1;
        self.loop_bindings.push(HashSet::new());
        visit_expr_loop(self, node);
        self.loop_bindings.pop();
        self.loop_depth -= 1;
    }

    fn visit_arm(&mut self, node: &'ast syn::Arm) {
        if self.loop_depth > 0 {
            let mut bindings = HashSet::new();
            collect_pat_idents(&node.pat, &mut bindings);
            self.loop_bindings.push(bindings);
            visit_arm(self, node);
            self.loop_bindings.pop();
        } else {
            visit_arm(self, node);
        }
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if self.loop_depth > 0
            && (is_diagnostic_collection_mutation(node)
                || is_error_mapping_method(node)
                || is_owned_metadata_method(node))
        {
            return;
        }

        if self.loop_depth > 0 {
            let method = node.method.to_string();
            if method == "to_owned" && !self.expr_depends_on_loop_binding(&node.receiver) {
                self.findings
                    .push((node.method.span().start().line, method));
            }
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if self.loop_depth > 0 && (is_diagnostic_constructor(node) || is_error_constructor(node)) {
            return;
        }

        if self.loop_depth > 0
            && let syn::Expr::Path(path) = &*node.func
        {
            let call = path_to_string(&path.path);
            if call == "String::from"
                && !node
                    .args
                    .iter()
                    .any(|arg| self.expr_depends_on_loop_binding(arg))
            {
                self.findings.push((
                    path.path.segments.last().unwrap().ident.span().start().line,
                    call,
                ));
            }
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        if self.loop_depth > 0 && is_diagnostic_collection_init(node) {
            return;
        }

        visit_local(self, node);
        if self.loop_depth > 0
            && let Some(bindings) = self.loop_bindings.last_mut()
        {
            collect_pat_idents(&node.pat, bindings);
        }
    }

    fn visit_expr_reference(&mut self, node: &'ast syn::ExprReference) {
        if self.loop_depth > 0
            && matches!(&*node.expr, syn::Expr::Macro(expr) if expr.mac.path.is_ident("format"))
        {
            return;
        }

        syn::visit::visit_expr_reference(self, node);
    }

    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        if self.loop_depth > 0 && is_visitor_struct(&node.path) {
            return;
        }

        syn::visit::visit_expr_struct(self, node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if self.loop_depth > 0
            && node.path.is_ident("format")
            && !self.macro_depends_on_loop_binding(node)
        {
            self.findings.push((
                node.path.segments[0].ident.span().start().line,
                "format!".into(),
            ));
        }
        syn::visit::visit_macro(self, node);
    }
}

impl AllocationLoopVisitor {
    fn active_loop_bindings(&self) -> HashSet<String> {
        self.loop_bindings
            .iter()
            .flat_map(|bindings| bindings.iter().cloned())
            .collect()
    }

    fn expr_depends_on_loop_binding(&self, expr: &syn::Expr) -> bool {
        expr_contains_any_ident(expr, &self.active_loop_bindings())
    }

    fn macro_depends_on_loop_binding(&self, mac: &syn::Macro) -> bool {
        macro_mentions_any_ident(mac, &self.active_loop_bindings())
    }
}

fn is_diagnostic_constructor(node: &syn::ExprCall) -> bool {
    let syn::Expr::Path(path) = &*node.func else {
        return false;
    };

    let mut segments = path.path.segments.iter().rev();
    matches!(
        (segments.next(), segments.next()),
        (Some(method), Some(receiver))
            if method.ident == "new"
                && (receiver.ident == "Smell" || receiver.ident == "SourceLocation")
    )
}

fn is_error_constructor(node: &syn::ExprCall) -> bool {
    let syn::Expr::Path(path) = &*node.func else {
        return false;
    };

    let mut segments = path.path.segments.iter().rev();
    let Some(method) = segments.next() else {
        return false;
    };
    if method.ident == "Err" {
        return true;
    }

    let Some(receiver) = segments.next() else {
        return false;
    };
    let receiver = receiver.ident.to_string();
    let method = method.ident.to_string();
    (receiver == "ApplicationError" || receiver.ends_with("Error"))
        && (method == "new"
            || method == "fatal"
            || method == "external"
            || method == "conflict"
            || method.chars().next().is_some_and(char::is_uppercase))
}

fn is_visitor_struct(path: &syn::Path) -> bool {
    path.segments
        .last()
        .map(|segment| segment.ident.to_string().ends_with("Visitor"))
        .unwrap_or(false)
}

fn is_error_mapping_method(node: &syn::ExprMethodCall) -> bool {
    matches!(node.method.to_string().as_str(), "map_err" | "ok_or_else")
}

fn is_owned_metadata_method(node: &syn::ExprMethodCall) -> bool {
    matches!(node.method.to_string().as_str(), "with_origin")
}

fn is_diagnostic_collection_mutation(node: &syn::ExprMethodCall) -> bool {
    if !matches!(
        node.method.to_string().as_str(),
        "push" | "extend" | "insert"
    ) {
        return false;
    }

    receiver_path_tail(&node.receiver).is_some_and(is_diagnostic_collection_name)
}

fn is_diagnostic_collection_init(local: &syn::Local) -> bool {
    pat_ident(&local.pat).is_some_and(is_diagnostic_collection_name)
        && local.init.as_ref().is_some_and(
            |init| matches!(&*init.expr, syn::Expr::Macro(expr) if expr.mac.path.is_ident("vec")),
        )
}

fn receiver_path_tail(expr: &syn::Expr) -> Option<&syn::Ident> {
    match expr {
        syn::Expr::Path(path) => path.path.segments.last().map(|segment| &segment.ident),
        syn::Expr::Field(field) => match &field.member {
            syn::Member::Named(name) => Some(name),
            syn::Member::Unnamed(_) => None,
        },
        _ => None,
    }
}

fn pat_ident(pat: &syn::Pat) -> Option<&syn::Ident> {
    match pat {
        syn::Pat::Ident(ident) => Some(&ident.ident),
        _ => None,
    }
}

fn is_diagnostic_collection_name(name: &syn::Ident) -> bool {
    matches!(
        name.to_string().as_str(),
        "smells"
            | "findings"
            | "evidence"
            | "usages"
            | "blocking_calls"
            | "lock_calls"
            | "spawns"
            | "parts"
    )
}
