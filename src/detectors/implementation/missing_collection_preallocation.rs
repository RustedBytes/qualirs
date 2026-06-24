use std::collections::HashMap;

use syn::visit::{
    Visit, visit_expr_assign, visit_expr_for_loop, visit_expr_loop, visit_expr_method_call,
    visit_expr_while, visit_item_fn, visit_item_mod, visit_local, visit_macro,
};

use crate::analysis::detector::Detector;
use crate::detectors::implementation::perf_utils::{
    expr_path_tail, macro_first_expr_ident, pat_ident, path_to_string,
};
use crate::detectors::policy::has_test_cfg;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects growable collections that are initialized empty and repeatedly grown in loops.
pub struct MissingCollectionPreallocationDetector;

impl Detector for MissingCollectionPreallocationDetector {
    fn name(&self) -> &str {
        "Missing Collection Preallocation"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = PreallocationVisitor {
            loop_depth: 0,
            loop_capacity_stack: Vec::new(),
            candidates: HashMap::new(),
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|finding| {
                Smell::new(
                    SmellCategory::Performance,
                    "Missing Collection Preallocation",
                    Severity::Warning,
                                        crate::domain::smell::FindingConfidence::Medium,
                    SourceLocation::new(file.path.clone(), finding.line, finding.line, None),
                    format!(
                        "`{}` is created with `{}::new()` and repeatedly grown in a loop",
                        finding.name,
                        finding.kind.type_name()
                    ),
                    format!(
                        "Use `{}::with_capacity(...)` or call `reserve` before the loop when the expected size is known.",
                        finding.kind.type_name()
                    ),
                )
            })
            .collect()
    }
}

#[derive(Clone, Copy)]
enum CollectionKind {
    Vec,
    HashMap,
    String,
}

impl CollectionKind {
    fn type_name(self) -> &'static str {
        match self {
            Self::Vec => "Vec",
            Self::HashMap => "HashMap",
            Self::String => "String",
        }
    }

    fn grows_with(self, method: &str) -> bool {
        match self {
            Self::Vec => matches!(method, "push" | "extend" | "insert"),
            Self::HashMap => matches!(method, "insert" | "extend" | "entry"),
            Self::String => matches!(method, "push" | "push_str"),
        }
    }
}

struct Candidate {
    kind: CollectionKind,
    reserved: bool,
    invalidated: bool,
    growth: Option<Growth>,
}

impl Candidate {
    fn new(kind: CollectionKind) -> Self {
        Self {
            kind,
            reserved: false,
            invalidated: false,
            growth: None,
        }
    }
}

#[derive(Clone, Copy)]
struct Growth {
    line: usize,
}

struct Finding {
    line: usize,
    name: String,
    kind: CollectionKind,
}

struct PreallocationVisitor {
    loop_depth: usize,
    loop_capacity_stack: Vec<bool>,
    candidates: HashMap<String, Candidate>,
    findings: Vec<Finding>,
}

impl<'ast> Visit<'ast> for PreallocationVisitor {
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
        let previous = std::mem::take(&mut self.candidates);
        let previous_loop_capacity_stack = std::mem::take(&mut self.loop_capacity_stack);
        visit_item_fn(self, node);
        self.flush_candidates();
        self.candidates = previous;
        self.loop_capacity_stack = previous_loop_capacity_stack;
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        if let Some(name) = pat_ident(&node.pat) {
            if let Some(kind) = node
                .init
                .as_ref()
                .and_then(|init| empty_collection_kind(&init.expr))
                && !is_diagnostic_collection_name(&name)
            {
                self.candidates.insert(name, Candidate::new(kind));
            } else if self.candidates.contains_key(&name) {
                self.candidates.remove(&name);
            }
        }

        visit_local(self, node);
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.loop_depth += 1;
        self.loop_capacity_stack
            .push(expr_has_capacity_hint(&node.expr));
        visit_expr_for_loop(self, node);
        self.loop_capacity_stack.pop();
        self.loop_depth -= 1;
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.loop_depth += 1;
        self.loop_capacity_stack.push(false);
        visit_expr_while(self, node);
        self.loop_capacity_stack.pop();
        self.loop_depth -= 1;
    }

    fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
        self.loop_depth += 1;
        self.loop_capacity_stack.push(false);
        visit_expr_loop(self, node);
        self.loop_capacity_stack.pop();
        self.loop_depth -= 1;
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let Some(receiver) = expr_path_tail(&node.receiver) else {
            visit_expr_method_call(self, node);
            return;
        };

        let method = node.method.to_string();
        if matches!(method.as_str(), "reserve" | "reserve_exact" | "try_reserve")
            && let Some(candidate) = self.candidates.get_mut(&receiver)
        {
            candidate.reserved = true;
        }

        if self.loop_depth > 0
            && method == "clear"
            && let Some(candidate) = self.candidates.get_mut(&receiver)
        {
            candidate.invalidated = true;
        }

        if self.loop_depth > 0 {
            self.record_growth(&receiver, &method, node.method.span().start().line);
        }

        visit_expr_method_call(self, node);
    }

    fn visit_expr_assign(&mut self, node: &'ast syn::ExprAssign) {
        if self.loop_depth > 0
            && let Some(target) = expr_path_tail(&node.left)
            && let Some(candidate) = self.candidates.get_mut(&target)
        {
            candidate.invalidated = true;
        }

        visit_expr_assign(self, node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if self.loop_depth > 0
            && (node.path.is_ident("write") || node.path.is_ident("writeln"))
            && let Some(target) = macro_first_expr_ident(node)
        {
            self.record_growth(
                &target,
                "push_str",
                node.path.segments[0].ident.span().start().line,
            );
        }

        visit_macro(self, node);
    }
}

impl PreallocationVisitor {
    fn record_growth(&mut self, receiver: &str, method: &str, line: usize) {
        if !self.in_capacity_informative_loop() {
            return;
        }

        let Some(candidate) = self.candidates.get_mut(receiver) else {
            return;
        };
        if candidate.growth.is_some()
            || candidate.reserved
            || candidate.invalidated
            || !candidate.kind.grows_with(method)
        {
            return;
        }

        candidate.growth = Some(Growth { line });
    }

    fn in_capacity_informative_loop(&self) -> bool {
        self.loop_capacity_stack.last().copied().unwrap_or(false)
    }

    fn flush_candidates(&mut self) {
        for (name, candidate) in self.candidates.drain() {
            if candidate.reserved || candidate.invalidated {
                continue;
            }

            if let Some(growth) = candidate.growth {
                self.findings.push(Finding {
                    line: growth.line,
                    name,
                    kind: candidate.kind,
                });
            }
        }
    }
}

fn empty_collection_kind(expr: &syn::Expr) -> Option<CollectionKind> {
    let syn::Expr::Call(call) = expr else {
        return None;
    };
    let syn::Expr::Path(path) = &*call.func else {
        return None;
    };
    if !call.args.is_empty() {
        return None;
    }

    let path = path_to_string(&path.path);
    if path.ends_with("Vec::new") {
        Some(CollectionKind::Vec)
    } else if path.ends_with("HashMap::new") {
        Some(CollectionKind::HashMap)
    } else if path.ends_with("String::new") {
        Some(CollectionKind::String)
    } else {
        None
    }
}

fn is_diagnostic_collection_name(name: &str) -> bool {
    matches!(
        name,
        "smells"
            | "findings"
            | "diagnostics"
            | "evidence"
            | "errors"
            | "warnings"
            | "messages"
            | "violations"
    )
}

fn expr_has_capacity_hint(expr: &syn::Expr) -> bool {
    if let Some(inner) = transparent_expr(expr) {
        return expr_has_capacity_hint(inner);
    }

    match expr {
        syn::Expr::Array(_) | syn::Expr::Field(_) | syn::Expr::Path(_) | syn::Expr::Tuple(_) => {
            true
        }
        syn::Expr::MethodCall(method) => {
            matches!(
                method.method.to_string().as_str(),
                "iter" | "iter_mut" | "into_iter"
            ) && expr_has_capacity_hint(&method.receiver)
        }
        syn::Expr::Range(range) => range
            .end
            .as_ref()
            .is_some_and(|end| range_bound_has_capacity_hint(end)),
        _ => false,
    }
}

fn range_bound_has_capacity_hint(expr: &syn::Expr) -> bool {
    if let Some(inner) = transparent_expr(expr) {
        return range_bound_has_capacity_hint(inner);
    }

    matches!(expr, syn::Expr::Lit(_)) || is_len_method_call(expr) || expr_has_capacity_hint(expr)
}

fn is_len_method_call(expr: &syn::Expr) -> bool {
    matches!(expr, syn::Expr::MethodCall(method) if method.method == "len")
}

fn transparent_expr(expr: &syn::Expr) -> Option<&syn::Expr> {
    if let syn::Expr::Reference(reference) = expr {
        return Some(&reference.expr);
    }
    if let syn::Expr::Paren(paren) = expr {
        return Some(&paren.expr);
    }
    if let syn::Expr::Group(group) = expr {
        return Some(&group.expr);
    }
    None
}
