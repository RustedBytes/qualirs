use std::collections::HashSet;
use syn::{spanned::Spanned, visit::Visit};

use crate::analysis::{detector::Detector, evidence};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};

/// Finds foreign declarations for which no local safe caller can be established.
pub struct FfiWithoutWrapperDetector;

impl Detector for FfiWithoutWrapperDetector {
    fn name(&self) -> &str {
        "FFI Without Wrapper"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut findings = Vec::new();
        inspect_module(&file.ast.items, file, &mut findings);
        findings
    }
}

fn inspect_module(items: &[syn::Item], file: &SourceFile, findings: &mut Vec<Smell>) {
    let mut declarations = Vec::new();
    for item in items {
        match item {
            syn::Item::ForeignMod(m) => {
                for item in &m.items {
                    if let syn::ForeignItem::Fn(f) = item {
                        declarations
                            .push((f.sig.ident.to_string(), f.sig.fn_token.span.start().line));
                    }
                }
            }
            syn::Item::Mod(m) if !crate::detectors::policy::has_test_cfg(&m.attrs) => {
                if let Some((_, items)) = &m.content {
                    inspect_module(items, file, findings);
                }
            }
            _ => {}
        }
    }
    if declarations.is_empty() {
        return;
    }
    // Analyze one lexical module at a time. A similarly named wrapper in another
    // module, or an unrelated function with a matching name, proves nothing.
    let ast = syn::File {
        frontmatter: None,
        shebang: None,
        attrs: Vec::new(),
        items: items.to_vec(),
    };
    let mut callers = SafeCalls {
        safe: false,
        calls: HashSet::new(),
    };
    callers.visit_file(&ast);
    let mut wrapped = HashSet::new();
    evidence::inspect(&ast, |expr, ctx| {
        if !callers.calls.contains(&evidence::span_key(expr.span())) {
            return;
        }
        let syn::Expr::Call(c) = expr else {
            return;
        };
        let syn::Expr::Path(p) = &*c.func else {
            return;
        };
        let resolved = ctx.path(&p.path);
        wrapped.insert(
            resolved
                .strip_prefix("self::")
                .unwrap_or(&resolved)
                .to_string(),
        );
    });
    for (name, line) in declarations {
        if wrapped.contains(&name) {
            continue;
        }
        findings.push(Smell::new(SmellCategory::Unsafe, "FFI Without Wrapper", Severity::Warning,
            FindingConfidence::Low, SourceLocation::new(file.path.clone(), line, line, None),
            format!("No local safe caller was resolved for FFI function `{name}`; a wrapper may exist elsewhere"),
            "Review callers and provide a safe wrapper that validates inputs when the API permits one."));
    }
}

struct SafeCalls {
    safe: bool,
    calls: HashSet<(usize, usize, usize, usize)>,
}
impl<'a> Visit<'a> for SafeCalls {
    fn visit_item_mod(&mut self, _: &'a syn::ItemMod) {}
    fn visit_item_fn(&mut self, n: &'a syn::ItemFn) {
        if crate::detectors::policy::has_test_cfg(&n.attrs) {
            return;
        }
        let prev = self.safe;
        self.safe = !matches!(n.sig.safety, syn::Safety::Unsafe(_));
        self.visit_block(&n.block);
        self.safe = prev;
    }
    fn visit_impl_item_fn(&mut self, n: &'a syn::ImplItemFn) {
        if crate::detectors::policy::has_test_cfg(&n.attrs) {
            return;
        }
        let prev = self.safe;
        self.safe = !matches!(n.sig.safety, syn::Safety::Unsafe(_));
        self.visit_block(&n.block);
        self.safe = prev;
    }
    fn visit_trait_item_fn(&mut self, n: &'a syn::TraitItemFn) {
        let prev = self.safe;
        self.safe = !matches!(n.sig.safety, syn::Safety::Unsafe(_));
        if let Some(block) = &n.default {
            self.visit_block(block);
        }
        self.safe = prev;
    }
    fn visit_expr_call(&mut self, n: &'a syn::ExprCall) {
        if self.safe {
            self.calls.insert(evidence::span_key(n.span()));
        }
        syn::visit::visit_expr_call(self, n);
    }
}
