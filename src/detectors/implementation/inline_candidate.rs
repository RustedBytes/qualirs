use std::collections::HashMap;

use syn::visit::{
    Visit, visit_expr_async, visit_expr_await, visit_expr_break, visit_expr_closure,
    visit_expr_continue, visit_expr_for_loop, visit_expr_if, visit_expr_loop, visit_expr_macro,
    visit_expr_match, visit_expr_method_call, visit_expr_unsafe, visit_expr_while,
    visit_impl_item_fn, visit_item_fn, visit_item_impl, visit_item_mod,
};

use crate::analysis::detector::Detector;
use crate::detectors::policy::has_test_cfg;
use crate::domain::smell::Smell;
use crate::domain::source::SourceFile;

/// Detects tiny hot helpers that may benefit from an explicit inline hint.
pub struct InlineCandidateDetector;

impl Detector for InlineCandidateDetector {
    fn name(&self) -> &str {
        "Inline Candidate"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        analysis::detect(file)
    }
}

mod analysis {
    use super::*;
    use crate::domain::smell::{Severity, SmellCategory, SourceLocation};

    const MAX_INLINE_BODY_LINES: usize = 8;
    const MAX_INLINE_STATEMENTS: usize = 3;
    const MIN_CALL_SITES: usize = 3;

    pub(super) fn detect(file: &SourceFile) -> Vec<Smell> {
        let mut collector = CandidateCollector {
            candidates: Vec::new(),
        };
        collector.visit_file(&file.ast);

        let mut counter = CallCounter::default();
        counter.visit_file(&file.ast);

        let method_candidate_counts = method_candidate_counts(&collector.candidates);

        collector
            .candidates
            .into_iter()
            .filter_map(|candidate| {
                candidate_smell(file, &counter, &method_candidate_counts, candidate)
            })
            .collect()
    }

    fn candidate_smell(
        file: &SourceFile,
        counter: &CallCounter,
        method_candidate_counts: &HashMap<String, usize>,
        candidate: InlineCandidate,
    ) -> Option<Smell> {
        if candidate.has_ambiguous_method_name(method_candidate_counts) {
            return None;
        }

        let call_sites = counter.call_sites(&candidate);
        (call_sites >= MIN_CALL_SITES).then(|| {
            Smell::new(
                SmellCategory::Performance,
                "Inline Candidate",
                inline_candidate_severity(call_sites),
                crate::domain::smell::FindingConfidence::Low,
                SourceLocation::new(file.path.clone(), candidate.line, candidate.line, None),
                format!(
                    "Function `{}` is tiny and called {call_sites} times in this file",
                    candidate.display_name
                ),
                "Consider #[inline] for small hot helpers after profiling confirms call overhead.",
            )
        })
    }

    fn inline_candidate_severity(call_sites: usize) -> Severity {
        if call_sites >= MIN_CALL_SITES * 2 {
            Severity::Warning
        } else {
            Severity::Info
        }
    }

    fn method_candidate_counts(candidates: &[InlineCandidate]) -> HashMap<String, usize> {
        let mut counts = HashMap::with_capacity(candidates.len());
        for candidate in candidates {
            if matches!(candidate.call_kind, CallKind::Method) {
                *counts.entry(candidate.name.clone()).or_default() += 1;
            }
        }
        counts
    }

    struct InlineCandidate {
        name: String,
        display_name: String,
        line: usize,
        call_kind: CallKind,
    }

    impl InlineCandidate {
        fn has_ambiguous_method_name(
            &self,
            method_candidate_counts: &HashMap<String, usize>,
        ) -> bool {
            method_candidate_counts
                .get(&self.name)
                .is_some_and(|count| *count > 1)
                && matches!(self.call_kind, CallKind::Method)
        }
    }

    #[derive(Clone)]
    enum CallKind {
        Function,
        Method,
        AssociatedFunction { type_name: String },
    }

    struct CandidateCollector {
        candidates: Vec<InlineCandidate>,
    }

    impl<'ast> Visit<'ast> for CandidateCollector {
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

            if let Some(candidate) = candidate_from_item_fn(node) {
                self.candidates.push(candidate);
            }
            visit_item_fn(self, node);
        }

        fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
            if has_test_cfg(&node.attrs) || node.trait_.is_some() {
                return;
            }

            let impl_type_name = impl_self_type_name(&node.self_ty);
            for item in &node.items {
                if let syn::ImplItem::Fn(method) = item
                    && !has_test_cfg(&method.attrs)
                    && let Some(candidate) =
                        candidate_from_impl_fn(method, impl_type_name.as_deref())
                {
                    self.candidates.push(candidate);
                }
            }

            visit_item_impl(self, node);
        }
    }

    #[derive(Default)]
    struct CallCounter {
        function_calls: HashMap<String, usize>,
        method_calls: HashMap<String, usize>,
        associated_calls: HashMap<(String, String), usize>,
        impl_type_stack: Vec<String>,
    }

    impl CallCounter {
        fn call_sites(&self, candidate: &InlineCandidate) -> usize {
            match &candidate.call_kind {
                CallKind::Function => self
                    .function_calls
                    .get(&candidate.name)
                    .copied()
                    .unwrap_or(0),
                CallKind::Method => self.method_calls.get(&candidate.name).copied().unwrap_or(0),
                CallKind::AssociatedFunction { type_name } => self
                    .associated_calls
                    .get(&(type_name.clone(), candidate.name.clone()))
                    .copied()
                    .unwrap_or(0),
            }
        }
    }

    impl<'ast> Visit<'ast> for CallCounter {
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

        fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
            if has_test_cfg(&node.attrs) {
                return;
            }
            visit_impl_item_fn(self, node);
        }

        fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
            if has_test_cfg(&node.attrs) {
                return;
            }

            let original_len = self.impl_type_stack.len();
            if let Some(type_name) = impl_self_type_name(&node.self_ty) {
                self.impl_type_stack.push(type_name);
            }

            visit_item_impl(self, node);
            self.impl_type_stack.truncate(original_len);
        }

        fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
            if let syn::Expr::Path(path) = &*node.func
                && let Some(target) = call_target(&path.path, self.impl_type_stack.last())
            {
                match target {
                    CallTarget::Function(name) => {
                        *self.function_calls.entry(name).or_default() += 1;
                    }
                    CallTarget::AssociatedFunction { type_name, name } => {
                        *self.associated_calls.entry((type_name, name)).or_default() += 1;
                    }
                }
            }

            syn::visit::visit_expr_call(self, node);
        }

        fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
            *self
                .method_calls
                .entry(node.method.to_string())
                .or_default() += 1;
            visit_expr_method_call(self, node);
        }
    }

    fn candidate_from_item_fn(func: &syn::ItemFn) -> Option<InlineCandidate> {
        let name = func.sig.ident.to_string();
        if is_constructor_name(&name) {
            return None;
        }

        is_inline_candidate(&func.attrs, &func.sig, &func.block).then(|| InlineCandidate {
            display_name: name.clone(),
            name,
            line: func.sig.ident.span().start().line,
            call_kind: CallKind::Function,
        })
    }

    fn candidate_from_impl_fn(
        func: &syn::ImplItemFn,
        impl_type_name: Option<&str>,
    ) -> Option<InlineCandidate> {
        let name = func.sig.ident.to_string();
        if is_constructor_name(&name) {
            return None;
        }

        is_inline_candidate(&func.attrs, &func.sig, &func.block).then(|| InlineCandidate {
            display_name: impl_type_name
                .filter(|_| !has_receiver(&func.sig))
                .map_or_else(|| name.clone(), |type_name| format!("{type_name}::{name}")),
            call_kind: if has_receiver(&func.sig) {
                CallKind::Method
            } else if let Some(type_name) = impl_type_name {
                CallKind::AssociatedFunction {
                    type_name: type_name.to_string(),
                }
            } else {
                CallKind::Function
            },
            name,
            line: func.sig.ident.span().start().line,
        })
    }

    fn is_constructor_name(name: &str) -> bool {
        name == "new"
    }

    enum CallTarget {
        Function(String),
        AssociatedFunction { type_name: String, name: String },
    }

    fn call_target(path: &syn::Path, impl_type_name: Option<&String>) -> Option<CallTarget> {
        let name = path.segments.last()?.ident.to_string();
        let qualifier = path
            .segments
            .iter()
            .rev()
            .nth(1)
            .map(|segment| segment.ident.to_string());

        match qualifier.as_deref() {
            Some("Self") => impl_type_name.map(|type_name| CallTarget::AssociatedFunction {
                type_name: type_name.clone(),
                name,
            }),
            Some(qualifier) if is_likely_type_name(qualifier) => {
                Some(CallTarget::AssociatedFunction {
                    type_name: qualifier.to_string(),
                    name,
                })
            }
            _ => Some(CallTarget::Function(name)),
        }
    }

    fn impl_self_type_name(ty: &syn::Type) -> Option<String> {
        let syn::Type::Path(tp) = ty else {
            return None;
        };
        tp.path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
    }

    fn is_likely_type_name(name: &str) -> bool {
        name.chars().next().is_some_and(char::is_uppercase)
    }

    fn is_inline_candidate(
        attrs: &[syn::Attribute],
        sig: &syn::Signature,
        block: &syn::Block,
    ) -> bool {
        if has_exclusion_attr(attrs) || sig.asyncness.is_some() || sig.unsafety.is_some() {
            return false;
        }

        if block.stmts.is_empty() || block.stmts.len() > MAX_INLINE_STATEMENTS {
            return false;
        }

        let body_lines = body_line_count(block);
        body_lines <= MAX_INLINE_BODY_LINES && block_is_simple(block)
    }

    fn body_line_count(block: &syn::Block) -> usize {
        let open_line = block.brace_token.span.open().start().line;
        let close_line = block.brace_token.span.close().start().line;
        close_line.saturating_sub(open_line).saturating_add(1)
    }

    fn block_is_simple(block: &syn::Block) -> bool {
        let mut visitor = ComplexityVisitor::default();
        visitor.visit_block(block);
        visitor.complex_nodes == 0
    }

    #[derive(Default)]
    struct ComplexityVisitor {
        complex_nodes: usize,
    }

    impl<'ast> Visit<'ast> for ComplexityVisitor {
        fn visit_item(&mut self, _node: &'ast syn::Item) {
            self.complex_nodes += 1;
        }

        fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
            self.complex_nodes += 1;
            visit_expr_if(self, node);
        }

        fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
            self.complex_nodes += 1;
            visit_expr_match(self, node);
        }

        fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
            self.complex_nodes += 1;
            visit_expr_for_loop(self, node);
        }

        fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
            self.complex_nodes += 1;
            visit_expr_while(self, node);
        }

        fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
            self.complex_nodes += 1;
            visit_expr_loop(self, node);
        }

        fn visit_expr_closure(&mut self, node: &'ast syn::ExprClosure) {
            self.complex_nodes += 1;
            visit_expr_closure(self, node);
        }

        fn visit_expr_async(&mut self, node: &'ast syn::ExprAsync) {
            self.complex_nodes += 1;
            visit_expr_async(self, node);
        }

        fn visit_expr_unsafe(&mut self, node: &'ast syn::ExprUnsafe) {
            self.complex_nodes += 1;
            visit_expr_unsafe(self, node);
        }

        fn visit_expr_await(&mut self, node: &'ast syn::ExprAwait) {
            self.complex_nodes += 1;
            visit_expr_await(self, node);
        }

        fn visit_expr_macro(&mut self, node: &'ast syn::ExprMacro) {
            self.complex_nodes += 1;
            visit_expr_macro(self, node);
        }

        fn visit_expr_break(&mut self, node: &'ast syn::ExprBreak) {
            self.complex_nodes += 1;
            visit_expr_break(self, node);
        }

        fn visit_expr_continue(&mut self, node: &'ast syn::ExprContinue) {
            self.complex_nodes += 1;
            visit_expr_continue(self, node);
        }
    }

    fn has_receiver(sig: &syn::Signature) -> bool {
        sig.inputs
            .first()
            .is_some_and(|arg| matches!(arg, syn::FnArg::Receiver(_)))
    }

    fn has_exclusion_attr(attrs: &[syn::Attribute]) -> bool {
        attrs.iter().any(|attr| {
            attr_is_named(attr, EXCLUDED_ATTRS) || attr_list_contains(attr, EXCLUDED_ATTRS)
        })
    }

    const EXCLUDED_ATTRS: &[&str] = &[
        "inline",
        "cold",
        "no_mangle",
        "export_name",
        "test",
        "bench",
        "proc_macro",
        "proc_macro_derive",
        "proc_macro_attribute",
    ];

    fn attr_is_named(attr: &syn::Attribute, names: &[&str]) -> bool {
        attr.path()
            .get_ident()
            .is_some_and(|ident| names.iter().any(|name| ident == name))
    }

    fn attr_list_contains(attr: &syn::Attribute, names: &[&str]) -> bool {
        let syn::Meta::List(list) = &attr.meta else {
            return false;
        };

        list.parse_args_with(
            syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
        )
        .is_ok_and(|metas| metas.iter().any(|meta| meta_matches(meta, names)))
    }

    fn meta_matches(meta: &syn::Meta, names: &[&str]) -> bool {
        match meta {
            syn::Meta::Path(path) | syn::Meta::List(syn::MetaList { path, .. }) => path
                .get_ident()
                .is_some_and(|ident| names.iter().any(|name| ident == name)),
            syn::Meta::NameValue(name_value) => name_value
                .path
                .get_ident()
                .is_some_and(|ident| names.iter().any(|name| ident == name)),
        }
    }
}
