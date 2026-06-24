use std::collections::HashSet;

use syn::visit::{Visit, visit_item_fn};

use crate::{
    analysis::detector::Detector,
    domain::{
        smell::{Severity, Smell, SmellCategory, SourceLocation},
        source::SourceFile,
    },
};

/// Heuristically detects lock guards that may live across an await point.
pub struct HoldingLockAcrossAwaitDetector;

impl Detector for HoldingLockAcrossAwaitDetector {
    fn name(&self) -> &str {
        "Holding Lock Across Await"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = LockAwaitVisitor {
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|line| {
                Smell::new(
                    SmellCategory::Concurrency,
                    "Holding Lock Across Await",
                    Severity::Critical,
                                        crate::domain::smell::FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    "Async function appears to hold a lock across an await point",
                    "Drop the guard before awaiting or move the awaited work outside the critical section.",
                )
            })
            .collect()
    }
}

struct LockAwaitVisitor {
    findings: Vec<usize>,
}

impl<'ast> Visit<'ast> for LockAwaitVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if node.sig.asyncness.is_some() {
            let mut block_visitor = BlockIssueVisitor { found: false };
            block_visitor.visit_block(&node.block);
            if block_visitor.found {
                self.findings.push(node.sig.fn_token.span.start().line);
            }
        }
        visit_item_fn(self, node);
    }
}

struct BlockIssueVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for BlockIssueVisitor {
    fn visit_block(&mut self, block: &'ast syn::Block) {
        if self.found {
            return;
        }
        if block_holds_lock_across_await(block) {
            self.found = true;
            return;
        }
        syn::visit::visit_block(self, block);
    }
}

fn block_holds_lock_across_await(block: &syn::Block) -> bool {
    let mut active_guards = HashSet::new();

    for stmt in &block.stmts {
        if !active_guards.is_empty() && contains_await_in_stmt(stmt) {
            return true;
        }

        remove_explicitly_dropped_guards(stmt, &mut active_guards);

        if let syn::Stmt::Local(local) = stmt
            && local
                .init
                .as_ref()
                .is_some_and(|init| returns_lock_guard(&init.expr))
        {
            collect_pat_idents(&local.pat, &mut active_guards);
        }
    }

    false
}

fn collect_pat_idents(pat: &syn::Pat, idents: &mut HashSet<String>) {
    match pat {
        syn::Pat::Ident(ident) => {
            idents.insert(ident.ident.to_string());
        }
        syn::Pat::Tuple(tuple) => {
            for elem in &tuple.elems {
                collect_pat_idents(elem, idents);
            }
        }
        syn::Pat::TupleStruct(tuple) => {
            for elem in &tuple.elems {
                collect_pat_idents(elem, idents);
            }
        }
        syn::Pat::Struct(strukt) => {
            for field in &strukt.fields {
                collect_pat_idents(&field.pat, idents);
            }
        }
        _ => {}
    }
}

fn remove_explicitly_dropped_guards(stmt: &syn::Stmt, active_guards: &mut HashSet<String>) {
    let mut visitor = DropVisitor {
        dropped: HashSet::new(),
    };
    visitor.visit_stmt(stmt);
    for dropped in visitor.dropped {
        active_guards.remove(&dropped);
    }
}

fn contains_await_in_stmt(stmt: &syn::Stmt) -> bool {
    let mut visitor = AwaitVisitor { found: false };
    visitor.visit_stmt(stmt);
    visitor.found
}

fn returns_lock_guard(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Await(await_expr) => returns_lock_guard(&await_expr.base),
        syn::Expr::MethodCall(call) if is_lock_method(&call.method.to_string()) => true,
        syn::Expr::MethodCall(call)
            if matches!(call.method.to_string().as_str(), "unwrap" | "expect") =>
        {
            returns_lock_guard(&call.receiver)
        }
        _ => false,
    }
}

struct AwaitVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for AwaitVisitor {
    fn visit_expr_await(&mut self, _node: &'ast syn::ExprAwait) {
        self.found = true;
    }
}

fn is_lock_method(method: &str) -> bool {
    matches!(method, "lock" | "read" | "write")
}

struct DropVisitor {
    dropped: HashSet<String>,
}

impl<'ast> Visit<'ast> for DropVisitor {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*node.func
            && path.path.is_ident("drop")
            && let Some(syn::Expr::Path(arg)) = node.args.first()
            && let Some(ident) = arg.path.get_ident()
        {
            self.dropped.insert(ident.to_string());
            return;
        }
        syn::visit::visit_expr_call(self, node);
    }
}
