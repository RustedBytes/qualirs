use quote::ToTokens;
use syn::visit::{Visit, visit_expr_match};

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects match arms with repeated bodies.
pub struct DuplicateMatchArmsDetector;

impl Detector for DuplicateMatchArmsDetector {
    fn name(&self) -> &str {
        "Duplicate Match Arms"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = DuplicateArmVisitor {
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|line| {
                Smell::new(
                    SmellCategory::Implementation,
                    "Duplicate Match Arms",
                    Severity::Info,
                    crate::domain::smell::FindingConfidence::Medium,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    "Match expression has duplicate arm bodies",
                    "Combine equivalent arms with `|` patterns or extract the repeated body.",
                )
            })
            .collect()
    }
}

struct DuplicateArmVisitor {
    findings: Vec<usize>,
}

impl<'ast> Visit<'ast> for DuplicateArmVisitor {
    fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
        let mut seen = std::collections::HashSet::new();
        for arm in &node.arms {
            if body_uses_pattern_binding(&arm.pat, &arm.body) {
                continue;
            }
            let body = normalized_body(&arm.body);
            if body.len() > 3 && !seen.insert(body) {
                self.findings
                    .push(arm.fat_arrow_token.spans[0].start().line);
                break;
            }
        }
        visit_expr_match(self, node);
    }
}

fn body_uses_pattern_binding(pat: &syn::Pat, body: &syn::Expr) -> bool {
    let bindings = pattern_bindings(pat);
    !bindings.is_empty() && expr_references_binding(body, &bindings)
}

fn pattern_bindings(pat: &syn::Pat) -> std::collections::HashSet<String> {
    let mut bindings = std::collections::HashSet::new();
    collect_pattern_bindings(pat, &mut bindings);
    bindings
}

fn collect_pattern_bindings(pat: &syn::Pat, bindings: &mut std::collections::HashSet<String>) {
    match pat {
        syn::Pat::Ident(pat_ident) => {
            bindings.insert(pat_ident.ident.to_string());
        }
        syn::Pat::Or(or) => {
            for pat in &or.cases {
                collect_pattern_bindings(pat, bindings);
            }
        }
        syn::Pat::Paren(paren) => collect_pattern_bindings(&paren.pat, bindings),
        syn::Pat::Reference(reference) => collect_pattern_bindings(&reference.pat, bindings),
        syn::Pat::Slice(slice) => collect_pat_elem_bindings(&slice.elems, bindings),
        syn::Pat::Struct(strukt) => {
            for field in &strukt.fields {
                collect_pattern_bindings(&field.pat, bindings);
            }
        }
        syn::Pat::Tuple(tuple) => collect_pat_elem_bindings(&tuple.elems, bindings),
        syn::Pat::TupleStruct(tuple) => collect_pat_elem_bindings(&tuple.elems, bindings),
        syn::Pat::Type(typed) => collect_pattern_bindings(&typed.pat, bindings),
        _ => {}
    }
}

fn collect_pat_elem_bindings(
    elems: &syn::punctuated::Punctuated<syn::Pat, syn::token::Comma>,
    bindings: &mut std::collections::HashSet<String>,
) {
    for pat in elems {
        collect_pattern_bindings(pat, bindings);
    }
}

fn expr_references_binding(expr: &syn::Expr, bindings: &std::collections::HashSet<String>) -> bool {
    let mut visitor = BindingReferenceVisitor {
        bindings,
        found: false,
    };
    visitor.visit_expr(expr);
    visitor.found
}

struct BindingReferenceVisitor<'a> {
    bindings: &'a std::collections::HashSet<String>,
    found: bool,
}

impl<'ast> Visit<'ast> for BindingReferenceVisitor<'_> {
    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if node
            .path
            .get_ident()
            .is_some_and(|ident| self.bindings.contains(&ident.to_string()))
        {
            self.found = true;
            return;
        }
        syn::visit::visit_expr_path(self, node);
    }

    fn visit_expr_field(&mut self, node: &'ast syn::ExprField) {
        if let syn::Expr::Path(path) = &*node.base
            && path
                .path
                .get_ident()
                .is_some_and(|ident| self.bindings.contains(&ident.to_string()))
        {
            self.found = true;
            return;
        }
        syn::visit::visit_expr_field(self, node);
    }
}

fn normalized_body(body: &syn::Expr) -> String {
    normalize(&body.to_token_stream().to_string())
}

fn normalize(value: &str) -> String {
    value.split_whitespace().collect::<String>()
}
