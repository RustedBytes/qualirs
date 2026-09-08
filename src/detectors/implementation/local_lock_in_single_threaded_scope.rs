use std::collections::{HashMap, HashSet};
use syn::{spanned::Spanned, visit::Visit};

use crate::analysis::{
    detector::Detector,
    evidence::{self, Kind},
};
use crate::detectors::implementation::perf_utils::{macro_mentions_any_ident, pat_ident};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};

/// Detects locally constructed standard locks whose uses stay in one execution scope.
pub struct LocalLockInSingleThreadedScopeDetector;

impl Detector for LocalLockInSingleThreadedScopeDetector {
    fn name(&self) -> &str {
        "Local Lock in Single-Threaded Scope"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut locals = Locals(HashMap::new());
        locals.visit_file(&file.ast);
        let mut locks: HashMap<(usize, usize), Lock> = HashMap::new();
        evidence::inspect(&file.ast, |expr, ctx| {
            record_lock_construction(expr, ctx, &locals, &mut locks);
            observe_lock_uses(expr, ctx, &mut locks);
        });
        let mut findings = Vec::new();
        for lock in locks.values() {
            findings.extend(lock.finding(file));
        }
        findings.sort_by_key(|f| f.location.line_start);
        findings
    }
}

struct Lock {
    name: String,
    kind: &'static str,
    execution: (usize, usize),
    uses: usize,
    locking_uses: usize,
    escaped: bool,
    line: Option<usize>,
}

type Position = (usize, usize);
type SpanKey = (usize, usize, usize, usize);
struct Locals(HashMap<SpanKey, (Position, String)>);
impl<'a> Visit<'a> for Locals {
    fn visit_local(&mut self, n: &'a syn::Local) {
        if let Some(name) = pat_ident(&n.pat)
            && let Some(init) = &n.init
        {
            let start = match &n.pat {
                syn::Pat::Ident(p) => p.ident.span().start(),
                syn::Pat::Type(p) => p.pat.span().start(),
                _ => n.pat.span().start(),
            };
            self.0.insert(
                evidence::span_key(init.expr.span()),
                ((start.line, start.column), name),
            );
        }
        syn::visit::visit_local(self, n);
    }
}

fn observe_lock_uses(
    expr: &syn::Expr,
    ctx: &evidence::Context,
    locks: &mut HashMap<Position, Lock>,
) {
    if let syn::Expr::Path(_) = expr
        && let Some(origin) = ctx.binding_origin(expr)
        && let Some(lock) = locks.get_mut(&origin)
    {
        lock.uses += 1;
        lock.escaped |= ctx.execution != lock.execution;
    }
    if let syn::Expr::MethodCall(call) = expr
        && matches!(call.method.to_string().as_str(), "lock" | "read" | "write")
        && call.args.is_empty()
        && ctx.expr(&call.receiver) == Kind::SyncLock
        && let Some(origin) = ctx.binding_origin(&call.receiver)
        && let Some(lock) = locks.get_mut(&origin)
    {
        lock.locking_uses += 1;
        lock.line.get_or_insert(call.method.span().start().line);
    }
    if let syn::Expr::Macro(mac) = expr {
        for (origin, lock) in locks {
            let path: syn::Expr = syn::parse_str(&lock.name).unwrap();
            if ctx.binding_origin(&path) == Some(*origin)
                && macro_mentions_any_ident(&mac.mac, &HashSet::from([lock.name.clone()]))
            {
                lock.escaped = true;
            }
        }
    }
}

impl Lock {
    fn finding(&self, file: &SourceFile) -> Option<Smell> {
        // Inspect every later use before claiming the value is unshared.
        // Borrows, moves, aliases, macros, and deferred uses invalidate that claim.
        if self.escaped || self.uses != self.locking_uses {
            return None;
        }
        if let Some(line) = self.line {
            Some(Smell::new(
                SmellCategory::Performance,
                LocalLockInSingleThreadedScopeDetector.name(),
                Severity::Info,
                FindingConfidence::High,
                SourceLocation::new(file.path.clone(), line, line, None),
                format!(
                    "Local standard {} binding '{}' is used only for locking in one execution scope",
                    self.kind, self.name
                ),
                "Consider plain mutable state when synchronization and poisoning semantics are unnecessary.",
            ))
        } else {
            None
        }
    }
}

fn record_lock_construction(
    expr: &syn::Expr,
    ctx: &evidence::Context,
    locals: &Locals,
    locks: &mut HashMap<Position, Lock>,
) {
    if let Some((origin, name)) = locals.0.get(&evidence::span_key(expr.span()))
        && let syn::Expr::Call(call) = expr
        && let syn::Expr::Path(path) = &*call.func
    {
        let kind = match ctx.path(&path.path).as_str() {
            "std::sync::Mutex::new" => Some("Mutex"),
            "std::sync::RwLock::new" => Some("RwLock"),
            _ => None,
        };
        if let Some(kind) = kind {
            locks.insert(
                *origin,
                Lock {
                    name: name.clone(),
                    kind,
                    execution: ctx.execution,
                    uses: 0,
                    locking_uses: 0,
                    escaped: false,
                    line: None,
                },
            );
        }
    }
}
