//! Associate explanations with syntax, rather than a fixed number of lines.
use std::collections::HashSet;

use proc_macro2::{Span, TokenStream, TokenTree};
use quote::ToTokens;
use syn::{spanned::Spanned, visit::Visit};

use crate::domain::source::SourceFile;

pub(crate) struct SourceNotes {
    chars: Vec<char>,
    starts: Vec<usize>,
    ends: Vec<usize>,
}

impl SourceNotes {
    pub fn new(code: &str) -> Self {
        let chars: Vec<_> = code.chars().collect();
        let mut starts = vec![0];
        for (i, ch) in chars.iter().enumerate() {
            if *ch == '\n' {
                starts.push(i + 1);
            }
        }
        let mut notes = Self {
            chars,
            starts,
            ends: Vec::new(),
        };
        fn tokens(stream: TokenStream, notes: &mut SourceNotes) {
            for token in stream {
                if let TokenTree::Group(group) = token {
                    notes.ends.push(notes.offset(group.span_open().end()));
                    tokens(group.stream(), notes);
                    notes.ends.push(notes.offset(group.span_close().end()));
                } else {
                    notes.ends.push(notes.offset(token.span().end()));
                }
            }
        }
        if let Ok(stream) = code.parse() {
            tokens(stream, &mut notes);
        }
        notes.ends.sort_unstable();
        notes
    }

    fn offset(&self, p: proc_macro2::LineColumn) -> usize {
        self.starts
            .get(p.line.saturating_sub(1))
            .copied()
            .unwrap_or(self.chars.len())
            .saturating_add(p.column)
            .min(self.chars.len())
    }

    /// Only the gap after the previous token is examined. String literals and
    /// code in an unrelated statement cannot masquerade as comments.
    pub fn leading(&self, span: Span) -> String {
        let end = self.offset(span.start());
        let index = self.ends.partition_point(|p| *p <= end);
        let start = index.checked_sub(1).map(|i| self.ends[i]).unwrap_or(0);
        let gap: String = self.chars[start..end].iter().collect();
        // A trailing comment on the preceding statement explains that
        // statement, not the next one. Opening braces may introduce a body's
        // leading comment on the same line.
        if start > 0
            && self.chars[start - 1] != '{'
            && let Some((_, rest)) = gap.split_once('\n')
        {
            rest.to_string()
        } else {
            gap
        }
    }

    /// A trailing comment on an attribute belongs to the attributed item.
    /// Unlike a trailing statement comment, it can explain that item's safety.
    fn attribute_comments(&self, attrs: &[syn::Attribute]) -> String {
        attrs
            .iter()
            .filter_map(|attr| {
                let start = self.offset(attr.span().end());
                let line: String = self.chars[start..]
                    .iter()
                    .take_while(|c| **c != '\n')
                    .collect();
                let text = line.trim_start();
                if text.starts_with("//") {
                    Some(line)
                } else if text.starts_with("/*") {
                    Some(text.split("*/").next().unwrap_or("").to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub(crate) fn doc_text(attrs: &[syn::Attribute]) -> String {
    attrs
        .iter()
        .filter_map(|a| {
            if !a.path().is_ident("doc") {
                return None;
            }
            let syn::Meta::NameValue(meta) = &a.meta else {
                return None;
            };
            let syn::Expr::Lit(lit) = &meta.value else {
                return None;
            };
            let syn::Lit::Str(text) = &lit.lit else {
                return None;
            };
            Some(text.value())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn mentions_safety(text: &str) -> bool {
    let words: Vec<_> = text
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    let text = format!(" {} ", words.join(" "));
    words.iter().any(|w| w == "safety")
        || causal_safety_note(&words)
        || text.contains("relies on the invariant")
        || text.contains("according to")
            && text.contains("documentation")
            && words.iter().any(|w| w == "thread" || w == "threads")
        || text.contains("do not")
            && words.iter().any(|w| w == "hold")
            && words.iter().any(|w| w == "reference" || w == "references")
}

// Natural-language explanations need not spell `SAFETY:` or put `because`
// immediately after `safe`. Keep the causal connector close to the assertion
// and require explanatory text on its other side. This recognizes a written
// rationale; it does not certify that the rationale is sound.
fn causal_safety_note(words: &[String]) -> bool {
    words.iter().enumerate().any(|(index, word)| {
        if word != "safe" {
            return false;
        }
        let after = &words[index + 1..];
        let reason_after = after
            .iter()
            .take(6)
            .enumerate()
            .any(|(i, w)| matches!(w.as_str(), "because" | "since") && after.len() > i + 1);
        let reason_before = words[index.saturating_sub(4).max(3).min(index)..index]
            .iter()
            .any(|w| matches!(w.as_str(), "so" | "therefore" | "hence"));
        reason_after || reason_before
    })
}

pub(crate) struct SafetyDocs(HashSet<(usize, usize)>);
impl SafetyDocs {
    pub fn new(file: &SourceFile) -> Self {
        let mut visitor = SafetyVisitor {
            notes: SourceNotes::new(&file.code),
            documented: false,
            found: HashSet::new(),
            previous_impl: None,
        };
        visitor.visit_file(&file.ast);
        Self(visitor.found)
    }
    pub fn contains(&self, span: Span) -> bool {
        self.0.contains(&(span.start().line, span.start().column))
    }
}

struct SafetyVisitor {
    notes: SourceNotes,
    documented: bool,
    found: HashSet<(usize, usize)>,
    previous_impl: Option<(String, bool)>,
}

impl SafetyVisitor {
    fn record(&mut self, span: Span) {
        if self.documented {
            self.found.insert((span.start().line, span.start().column));
        }
    }
    fn own(&self, span: Span, attrs: &[syn::Attribute]) -> bool {
        mentions_safety(&self.notes.leading(span))
            || mentions_safety(&doc_text(attrs))
            || mentions_safety(&self.notes.attribute_comments(attrs))
    }
    fn function(
        &mut self,
        sig: &syn::Signature,
        attrs: &[syn::Attribute],
        block: &syn::Block,
        span: Span,
    ) {
        let saved = self.documented;
        self.documented = self.own(span, attrs);
        if let syn::Safety::Unsafe(token) = &sig.safety {
            // A private unsafe helper may document its preconditions at the
            // start of its body, after imports and before executable work.
            for stmt in &block.stmts {
                if mentions_safety(&self.notes.leading(stmt.span())) {
                    self.documented = true;
                    break;
                }
                if !matches!(stmt, syn::Stmt::Item(syn::Item::Use(_))) {
                    break;
                }
            }
            self.record(token.span);
        }
        // A written function-level Safety contract also supplies the caller
        // obligations for its unsafe operations. Body-local preconditions stay
        // local; deferred bodies and nested functions reset the association.
        self.documented = mentions_safety(&doc_text(attrs));
        self.visit_block(block);
        self.documented = saved;
    }
}

impl<'a> Visit<'a> for SafetyVisitor {
    fn visit_local(&mut self, node: &'a syn::Local) {
        let saved = self.documented;
        self.documented |= self.own(node.span(), &node.attrs);
        syn::visit::visit_local(self, node);
        self.documented = saved;
    }
    fn visit_item(&mut self, item: &'a syn::Item) {
        let saved = self.documented;
        self.documented = false;
        let pair = if let syn::Item::Impl(i) = item {
            let marker = i.unsafety.is_some()
                && i.items.is_empty()
                && i.trait_
                    .as_ref()
                    .is_some_and(|(p, _)| p.is_ident("Send") || p.is_ident("Sync"));
            let name = i.self_ty.to_token_stream().to_string();
            self.documented = self.own(i.span(), &i.attrs)
                || marker
                    && self
                        .previous_impl
                        .as_ref()
                        .is_some_and(|(prev, docs)| prev == &name && *docs);
            marker.then_some((name, self.documented))
        } else {
            None
        };
        self.previous_impl = None;
        syn::visit::visit_item(self, item);
        self.previous_impl = pair;
        self.documented = saved;
    }
    fn visit_item_impl(&mut self, node: &'a syn::ItemImpl) {
        if let Some(token) = node.unsafety {
            self.record(token.span);
        }
        syn::visit::visit_item_impl(self, node);
    }
    fn visit_item_fn(&mut self, node: &'a syn::ItemFn) {
        self.function(&node.sig, &node.attrs, &node.block, node.span());
    }
    fn visit_impl_item_fn(&mut self, node: &'a syn::ImplItemFn) {
        self.function(&node.sig, &node.attrs, &node.block, node.span());
    }
    fn visit_block(&mut self, block: &'a syn::Block) {
        let inherited = self.documented;
        let mut pending = false;
        let mut previous_call = None;
        for stmt in &block.stmts {
            let gap = self.notes.leading(stmt.span());
            if mentions_safety(&gap)
                && let Some(span) = documented_closure_call(stmt, &gap)
            {
                self.found.insert((span.start().line, span.start().column));
            }
            let call = unsafe_call(stmt);
            let related = gap
                .lines()
                .skip(1)
                .filter(|line| line.trim().is_empty())
                .count()
                <= 1
                && call.is_some()
                && call == previous_call;
            self.documented = inherited || pending || mentions_safety(&gap) || related;
            let docs = self.documented;
            self.visit_stmt(stmt);
            let pure = pure_binding(stmt);
            pending = pure && docs;
            if !pure {
                previous_call = if docs { call } else { None };
            }
        }
        self.documented = inherited;
    }
    fn visit_expr_unsafe(&mut self, node: &'a syn::ExprUnsafe) {
        let saved = self.documented;
        self.documented |= self.own(node.span(), &node.attrs);
        self.record(node.unsafe_token.span);
        syn::visit::visit_expr_unsafe(self, node);
        self.documented = saved;
    }
    fn visit_arm(&mut self, node: &'a syn::Arm) {
        let saved = self.documented;
        self.documented |= self.own(node.span(), &node.attrs);
        syn::visit::visit_arm(self, node);
        self.documented = saved;
    }
    fn visit_expr_closure(&mut self, node: &'a syn::ExprClosure) {
        let saved = self.documented;
        self.documented = false;
        syn::visit::visit_expr_closure(self, node);
        self.documented = saved;
    }
    fn visit_expr_async(&mut self, node: &'a syn::ExprAsync) {
        let saved = self.documented;
        self.documented = false;
        syn::visit::visit_expr_async(self, node);
        self.documented = saved;
    }
}

/// A note that explicitly names one unsafe call can document that call inside
/// a synchronous closure in the associated statement. This associates written
/// explanations only; it does not carry execution state into a deferred body.
fn documented_closure_call(stmt: &syn::Stmt, note: &str) -> Option<Span> {
    let words: HashSet<_> = note
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|word| !word.is_empty())
        .collect();
    // A comment about merely constructing a closure is not a contract for its
    // unsafe work. Require an explicit condition as well as the operation name.
    if ![
        "valid",
        "lifetime",
        "invariant",
        "preconditions",
        "initialized",
        "bounds",
    ]
    .iter()
    .any(|word| words.contains(word))
    {
        return None;
    }
    let mut calls = DocumentedClosureCalls {
        words,
        in_closure: false,
        unsafe_count: 0,
        documented: None,
    };
    calls.visit_stmt(stmt);
    if calls.unsafe_count == 1 {
        calls.documented
    } else {
        None
    }
}

fn pure_binding(stmt: &syn::Stmt) -> bool {
    let syn::Stmt::Local(local) = stmt else {
        return false;
    };
    let Some(init) = &local.init else {
        return false;
    };
    fn pure(expr: &syn::Expr) -> bool {
        match expr {
            syn::Expr::Lit(_) | syn::Expr::Path(_) => true,
            syn::Expr::Cast(c) => pure(&c.expr),
            syn::Expr::Field(f) => pure(&f.base),
            syn::Expr::Reference(r) => pure(&r.expr),
            syn::Expr::Call(c) => {
                c.args.is_empty()
                    && matches!(&*c.func, syn::Expr::Path(p)
                if matches!(p.path.to_token_stream().to_string().as_str(), "std :: ptr :: null_mut" | "std :: ptr :: null"))
            }
            _ => false,
        }
    }
    init.diverge.is_none() && pure(&init.expr)
}

/// Adjacent calls with the same API and context argument may share an
/// explanation, with only trivial output bindings between them. Never carry
/// this association across a branch, closure, function, or unrelated call.
fn unsafe_call(stmt: &syn::Stmt) -> Option<String> {
    struct Calls(Vec<String>);
    impl<'a> Visit<'a> for Calls {
        fn visit_expr_unsafe(&mut self, node: &'a syn::ExprUnsafe) {
            if let [syn::Stmt::Expr(syn::Expr::Call(call), _)] = node.block.stmts.as_slice()
                && let syn::Expr::Path(path) = &*call.func
                && path.path.segments.len() > 1
                && let Some(first) = call.args.first()
            {
                self.0.push(format!(
                    "{}:{}",
                    path.to_token_stream(),
                    first.to_token_stream()
                ));
            }
        }
        fn visit_expr_closure(&mut self, _: &'a syn::ExprClosure) {}
        fn visit_expr_async(&mut self, _: &'a syn::ExprAsync) {}
        fn visit_item(&mut self, _: &'a syn::Item) {}
    }
    let mut calls = Calls(Vec::new());
    calls.visit_stmt(stmt);
    if calls.0.len() == 1 {
        calls.0.pop()
    } else {
        None
    }
}

struct DocumentedClosureCalls<'a> {
    words: HashSet<&'a str>,
    in_closure: bool,
    unsafe_count: usize,
    documented: Option<Span>,
}
impl<'a> Visit<'a> for DocumentedClosureCalls<'_> {
    fn visit_item(&mut self, _: &'a syn::Item) {}
    fn visit_expr_async(&mut self, _: &'a syn::ExprAsync) {}
    fn visit_expr_closure(&mut self, n: &'a syn::ExprClosure) {
        if self.in_closure || n.asyncness.is_some() {
            return;
        }
        self.in_closure = true;
        self.visit_expr(&n.body);
        self.in_closure = false;
    }
    fn visit_expr_unsafe(&mut self, n: &'a syn::ExprUnsafe) {
        self.unsafe_count += 1;
        if self.in_closure
            && let [syn::Stmt::Expr(syn::Expr::Call(call), _)] = n.block.stmts.as_slice()
            && let syn::Expr::Path(path) = &*call.func
            && let Some(name) = path.path.segments.last()
            && self.words.contains(name.ident.to_string().as_str())
        {
            self.documented = Some(n.unsafe_token.span);
        }
    }
}
