use std::collections::{HashMap, HashSet};

use syn::visit::{
    Visit, visit_expr_for_loop, visit_expr_loop, visit_expr_method_call, visit_expr_while,
    visit_local,
};

use crate::analysis::detector::Detector;
use crate::detectors::implementation::perf_utils::{expr_path_tail, pat_ident, type_path_tail};
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects linear `Vec::contains` lookups in loops or repeated local membership checks.
pub struct VecContainsInLoopDetector;

impl Detector for VecContainsInLoopDetector {
    fn name(&self) -> &str {
        "Vec Contains in Loop"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Fn(item_fn) = item {
                let mut visitor = VecContainsVisitor {
                    loop_depth: 0,
                    vec_vars: collect_vec_params(&item_fn.sig),
                    loop_findings: Vec::new(),
                    contains_counts: HashMap::new(),
                };
                visitor.visit_block(&item_fn.block);

                let mut reported = HashSet::new();
                for (line, name) in visitor.loop_findings {
                    reported.insert(name.clone());
                    smells.push(smell(file, line, &name, true));
                }

                for (name, calls) in visitor.contains_counts {
                    if reported.contains(&name) || calls.len() < 3 {
                        continue;
                    }
                    smells.push(smell(file, calls[0], &name, false));
                }
            }
        }

        smells
    }
}

struct VecContainsVisitor {
    loop_depth: usize,
    vec_vars: HashSet<String>,
    loop_findings: Vec<(usize, String)>,
    contains_counts: HashMap<String, Vec<usize>>,
}

impl<'ast> Visit<'ast> for VecContainsVisitor {
    fn visit_local(&mut self, node: &'ast syn::Local) {
        if local_is_vec(node)
            && let Some(name) = pat_ident(&node.pat)
        {
            self.vec_vars.insert(name);
        }

        visit_local(self, node);
    }

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

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "contains"
            && let Some(receiver) = expr_path_tail(&node.receiver)
            && self.vec_vars.contains(&receiver)
        {
            let line = node.method.span().start().line;
            self.contains_counts
                .entry(receiver.clone())
                .or_default()
                .push(line);
            if self.loop_depth > 0 && !self.loop_findings.iter().any(|(_, name)| name == &receiver)
            {
                self.loop_findings.push((line, receiver));
            }
        }

        visit_expr_method_call(self, node);
    }
}

fn collect_vec_params(sig: &syn::Signature) -> HashSet<String> {
    sig.inputs
        .iter()
        .filter_map(|input| match input {
            syn::FnArg::Typed(pat_type)
                if type_path_tail(&pat_type.ty).as_deref() == Some("Vec") =>
            {
                pat_ident(&pat_type.pat)
            }
            _ => None,
        })
        .collect()
}

fn local_is_vec(local: &syn::Local) -> bool {
    match &local.pat {
        syn::Pat::Type(pat_type) => type_path_tail(&pat_type.ty).as_deref() == Some("Vec"),
        _ => local.init.as_ref().is_some_and(
            |init| matches!(&*init.expr, syn::Expr::Macro(expr) if expr.mac.path.is_ident("vec")),
        ),
    }
}

fn smell(file: &SourceFile, line: usize, name: &str, in_loop: bool) -> Smell {
    let message = if in_loop {
        format!("`{name}.contains(...)` performs a linear scan inside a loop")
    } else {
        format!("`{name}.contains(...)` is repeated several times on the same Vec")
    };

    Smell::new(
        SmellCategory::Performance,
        "Vec Contains in Loop",
        Severity::Info,
        crate::domain::smell::FindingConfidence::High,
        SourceLocation::new(file.path.clone(), line, line, None),
        message,
        "Use a HashSet or BTreeSet when membership lookup dominates and ordering is not the main concern.",
    )
}
