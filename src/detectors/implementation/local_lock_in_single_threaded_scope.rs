use std::collections::{HashMap, HashSet};

use syn::visit::{Visit, visit_expr_call, visit_expr_method_call, visit_item_fn, visit_local};

use crate::analysis::detector::Detector;
use crate::detectors::implementation::perf_utils::{expr_path_tail, pat_ident, path_to_string};
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects local Mutex/RwLock values that are immediately used without sharing.
pub struct LocalLockInSingleThreadedScopeDetector;

impl Detector for LocalLockInSingleThreadedScopeDetector {
    fn name(&self) -> &str {
        "Local Lock in Single-Threaded Scope"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = LocalLockVisitor {
            locks: HashMap::new(),
            shared: HashSet::new(),
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|(line, name, kind)| {
                Smell::new(
                    SmellCategory::Performance,
                    "Local Lock in Single-Threaded Scope",
                    Severity::Info,
                                        crate::domain::smell::FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    format!("Local `{kind}` binding `{name}` is locked without evidence of sharing"),
                    "Use plain mutable state for single-threaded code, or RefCell/Cell if interior mutability is specifically needed.",
                )
            })
            .collect()
    }
}

#[derive(Clone, Copy)]
enum LockKind {
    Mutex,
    RwLock,
}

impl LockKind {
    fn type_name(self) -> &'static str {
        match self {
            Self::Mutex => "Mutex",
            Self::RwLock => "RwLock",
        }
    }
}

struct LocalLockVisitor {
    locks: HashMap<String, LockKind>,
    shared: HashSet<String>,
    findings: Vec<(usize, String, &'static str)>,
}

impl<'ast> Visit<'ast> for LocalLockVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let previous_locks = std::mem::take(&mut self.locks);
        let previous_shared = std::mem::take(&mut self.shared);
        visit_item_fn(self, node);
        self.locks = previous_locks;
        self.shared = previous_shared;
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        if let Some(name) = pat_ident(&node.pat)
            && let Some(kind) = node
                .init
                .as_ref()
                .and_then(|init| lock_constructor(&init.expr))
        {
            self.locks.insert(name, kind);
        }

        visit_local(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*node.func {
            let call = path_to_string(&path.path);
            if call.ends_with("Arc::new") {
                for arg in &node.args {
                    if let Some(name) = expr_path_tail(arg) {
                        self.shared.insert(name);
                    }
                }
            }
        }

        visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if matches!(node.method.to_string().as_str(), "lock" | "read" | "write")
            && let Some(receiver) = expr_path_tail(&node.receiver)
            && let Some(kind) = self.locks.get(&receiver).copied()
            && !self.shared.contains(&receiver)
            && !self
                .findings
                .iter()
                .any(|(_, existing, _)| existing == &receiver)
        {
            self.findings
                .push((node.method.span().start().line, receiver, kind.type_name()));
        }

        visit_expr_method_call(self, node);
    }
}

fn lock_constructor(expr: &syn::Expr) -> Option<LockKind> {
    let syn::Expr::Call(call) = expr else {
        return None;
    };
    let syn::Expr::Path(path) = &*call.func else {
        return None;
    };
    let path = path_to_string(&path.path);
    if path.ends_with("Mutex::new") {
        Some(LockKind::Mutex)
    } else if path.ends_with("RwLock::new") {
        Some(LockKind::RwLock)
    } else {
        None
    }
}
