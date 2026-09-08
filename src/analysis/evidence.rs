//! Deliberately bounded source evidence. Unresolved names never imply a type.
use std::collections::HashMap;
use syn::spanned::Spanned;
use syn::visit::Visit;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Unknown,
    Copy,
    String,
    Vec,
    RawPointer,
    SyncLock,
    AsyncLock,
    Guard,
    SyncReceiver,
    SyncSender,
    Writer,
    Unit,
    Result(Box<Kind>),
    Future(Box<Kind>),
    Reference(Box<Kind>),
}

impl Kind {
    pub fn value(&self) -> &Self {
        match self {
            Self::Reference(inner) => inner.value(),
            _ => self,
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct Context {
    items: ItemScope,
    bindings: Vec<HashMap<String, Kind>>,
    origins: HashMap<String, (usize, usize)>,
    pub in_async: bool,
    pub in_const: bool,
    pub loop_depth: usize,
    pub loop_start: (usize, usize),
    pub execution: (usize, usize),
    pub block: (usize, usize),
}

impl Context {
    pub fn new(items: &[syn::Item]) -> Self {
        let mut ctx = Self::default();
        ctx.add_items(items);
        ctx
    }

    fn add_items(&mut self, items: &[syn::Item]) {
        self.items.add_declarations(items);
        self.items.add_imports_and_aliases(items);
        self.add_return_types(items);
    }

    fn add_return_types(&mut self, items: &[syn::Item]) {
        for item in items {
            if let syn::Item::Fn(item) = item {
                let mut signature_context = self.clone();
                for p in &item.sig.generics.params {
                    if let syn::GenericParam::Type(t) = p {
                        signature_context
                            .items
                            .names
                            .insert(t.ident.to_string(), "<generic>".into());
                        signature_context.items.aliases.remove(&t.ident.to_string());
                    }
                }
                let output = match &item.sig.output {
                    syn::ReturnType::Default => Kind::Unit,
                    syn::ReturnType::Type(_, ty) => signature_context.kind(ty),
                };
                self.items.returns.insert(
                    item.sig.ident.to_string(),
                    if item.sig.asyncness.is_some() {
                        Kind::Future(Box::new(output))
                    } else {
                        output
                    },
                );
            }
        }
    }

    pub fn path(&self, path: &syn::Path) -> String {
        let mut parts = path.segments.iter().map(|s| s.ident.to_string());
        let Some(first) = parts.next() else {
            return String::new();
        };
        let root = if path.leading_colon.is_none() {
            if self.bindings.iter().rev().any(|s| s.contains_key(&first)) {
                "<binding>"
            } else {
                self.items
                    .names
                    .get(&first)
                    .map(String::as_str)
                    .unwrap_or(&first)
            }
        } else {
            &first
        };
        std::iter::once(root.to_string())
            .chain(parts)
            .collect::<Vec<_>>()
            .join("::")
    }

    pub fn kind(&self, ty: &syn::Type) -> Kind {
        self.kind_at(ty, 0)
    }

    /// Deriving Eq requires Eq on every field even when PartialEq is manual.
    /// Only closed standard types and resolved aliases are proven here.
    pub fn proves_eq(&self, ty: &syn::Type) -> bool {
        fn proven(ctx: &Context, ty: &syn::Type, depth: usize) -> bool {
            if depth > 16 {
                return false;
            }
            match ty {
                syn::Type::Tuple(t) => t.elems.iter().all(|t| proven(ctx, t, depth + 1)),
                syn::Type::Array(a) => proven(ctx, &a.elem, depth + 1),
                syn::Type::Slice(s) => proven(ctx, &s.elem, depth + 1),
                syn::Type::Reference(r) => proven(ctx, &r.elem, depth + 1),
                syn::Type::Paren(p) => proven(ctx, &p.elem, depth + 1),
                syn::Type::Group(g) => proven(ctx, &g.elem, depth + 1),
                syn::Type::Path(p) if p.qself.is_none() => {
                    if let Some(name) = p.path.get_ident()
                        && let Some(alias) = ctx.items.aliases.get(&name.to_string())
                    {
                        return proven(ctx, alias, depth + 1);
                    }
                    match ctx.path(&p.path).as_str() {
                        "bool"
                        | "char"
                        | "u8"
                        | "u16"
                        | "u32"
                        | "u64"
                        | "u128"
                        | "usize"
                        | "i8"
                        | "i16"
                        | "i32"
                        | "i64"
                        | "i128"
                        | "isize"
                        | "str"
                        | "String"
                        | "std::string::String"
                        | "alloc::string::String" => true,
                        "Vec"
                        | "std::vec::Vec"
                        | "alloc::vec::Vec"
                        | "Option"
                        | "std::option::Option"
                        | "core::option::Option"
                        | "Box"
                        | "std::boxed::Box"
                        | "alloc::boxed::Box"
                        | "std::sync::Arc"
                        | "alloc::sync::Arc" => {
                            let Some(segment) = p.path.segments.last() else {
                                return false;
                            };
                            let syn::PathArguments::AngleBracketed(args) = &segment.arguments
                            else {
                                return false;
                            };
                            matches!(args.args.first(), Some(syn::GenericArgument::Type(t)) if args.args.len() == 1 && proven(ctx, t, depth + 1))
                        }
                        _ => false,
                    }
                }
                _ => false,
            }
        }
        proven(self, ty, 0)
    }

    fn kind_at(&self, ty: &syn::Type, depth: usize) -> Kind {
        if depth > 16 {
            return Kind::Unknown;
        }
        match ty {
            syn::Type::Reference(r) => Kind::Reference(Box::new(self.kind_at(&r.elem, depth + 1))),
            syn::Type::Ptr(_) => Kind::RawPointer,
            syn::Type::Paren(p) => self.kind_at(&p.elem, depth + 1),
            syn::Type::Group(g) => self.kind_at(&g.elem, depth + 1),
            syn::Type::Tuple(t) if t.elems.is_empty() => Kind::Unit,
            syn::Type::Path(p) if p.qself.is_none() => self.path_kind(p, depth),
            _ => Kind::Unknown,
        }
    }

    fn path_kind(&self, p: &syn::TypePath, depth: usize) -> Kind {
        if let Some(name) = p.path.get_ident()
            && let Some(alias) = self.items.aliases.get(&name.to_string())
        {
            return self.kind_at(alias, depth + 1);
        }
        let path = self.path(&p.path);
        match path.as_str() {
            "bool" | "char" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16"
            | "i32" | "i64" | "i128" | "isize" | "f32" | "f64" => Kind::Copy,
            "String" | "str" | "std::string::String" | "alloc::string::String" => Kind::String,
            "Vec" | "std::vec::Vec" | "alloc::vec::Vec" => Kind::Vec,
            "std::sync::Mutex" | "std::sync::RwLock" => Kind::SyncLock,
            "std::sync::MutexGuard"
            | "std::sync::RwLockReadGuard"
            | "std::sync::RwLockWriteGuard" => Kind::Guard,
            "tokio::sync::Mutex" | "tokio::sync::RwLock" => Kind::AsyncLock,
            "std::sync::mpsc::Receiver" | "crossbeam_channel::Receiver" => Kind::SyncReceiver,
            "std::sync::mpsc::Sender" | "std::sync::mpsc::SyncSender" => Kind::SyncSender,
            "std::fs::File" | "std::io::BufWriter" => Kind::Writer,
            "Result"
            | "std::result::Result"
            | "core::result::Result"
            | "std::io::Result"
            | "std::fmt::Result" => {
                let inner = self.first_type_argument(&p.path, depth);
                Kind::Result(Box::new(inner))
            }
            "std::sync::Arc" | "alloc::sync::Arc" | "Box" | "std::boxed::Box"
            | "alloc::boxed::Box" => {
                let inner = self.first_type_argument(&p.path, depth);
                Kind::Reference(Box::new(inner))
            }
            _ => Kind::Unknown,
        }
    }

    fn first_type_argument(&self, path: &syn::Path, depth: usize) -> Kind {
        let Some(segment) = path.segments.last() else {
            return Kind::Unknown;
        };
        let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
            return Kind::Unknown;
        };
        args.args
            .iter()
            .find_map(|arg| match arg {
                syn::GenericArgument::Type(ty) => Some(self.kind_at(ty, depth + 1)),
                _ => None,
            })
            .unwrap_or(Kind::Unknown)
    }

    pub fn expr(&self, expr: &syn::Expr) -> Kind {
        match expr {
            syn::Expr::Path(_) => ident(expr)
                .and_then(|n| self.bindings.iter().rev().find_map(|s| s.get(&n)))
                .cloned()
                .unwrap_or(Kind::Unknown),
            syn::Expr::Paren(p) => self.expr(&p.expr),
            syn::Expr::Group(g) => self.expr(&g.expr),
            syn::Expr::Reference(r) => Kind::Reference(Box::new(self.expr(&r.expr))),
            syn::Expr::Cast(c) => self.kind(&c.ty),
            syn::Expr::Lit(l) => literal_kind(&l.lit),
            syn::Expr::Try(t) => match self.expr(&t.expr) {
                Kind::Result(k) => *k,
                _ => Kind::Unknown,
            },
            syn::Expr::Await(a) => match self.expr(&a.base) {
                Kind::Future(k) => *k,
                _ => Kind::Unknown,
            },
            syn::Expr::Call(c) => self.call_kind(c),
            syn::Expr::MethodCall(c) => self.method_kind(c),
            syn::Expr::Macro(m) if m.mac.path.is_ident("vec") => Kind::Vec,
            syn::Expr::Macro(m) if m.mac.path.is_ident("format") => Kind::String,
            _ => Kind::Unknown,
        }
    }

    fn call_kind(&self, c: &syn::ExprCall) -> Kind {
        let syn::Expr::Path(p) = &*c.func else {
            return Kind::Unknown;
        };
        let local_name = p.path.get_ident().map(ToString::to_string);
        if let Some(name) = local_name.as_deref()
            && !self.bindings.iter().rev().any(|s| s.contains_key(name))
            && let Some(k) = self.items.returns.get(name)
        {
            return k.clone();
        }
        let path = self.path(&p.path);
        match path.as_str() {
            "String::new"
            | "String::from"
            | "String::with_capacity"
            | "std::string::String::new" => Kind::String,
            "Vec::new" | "Vec::with_capacity" | "std::vec::Vec::new" => Kind::Vec,
            "std::sync::Mutex::new" | "std::sync::RwLock::new" => Kind::SyncLock,
            "tokio::sync::Mutex::new" | "tokio::sync::RwLock::new" => Kind::AsyncLock,
            _ if is_fs_result(&path) => {
                let result = Kind::Result(Box::new(Kind::Unknown));
                if path.starts_with("tokio::") {
                    Kind::Future(Box::new(result))
                } else {
                    result
                }
            }
            _ => Kind::Unknown,
        }
    }

    fn method_kind(&self, c: &syn::ExprMethodCall) -> Kind {
        let receiver = self.expr(&c.receiver);
        match (receiver.value(), c.method.to_string().as_str()) {
            (Kind::Result(k), "unwrap" | "expect") => *k.clone(),
            (Kind::SyncLock, "lock" | "read" | "write") => Kind::Result(Box::new(Kind::Guard)),
            (Kind::SyncReceiver, "recv" | "recv_timeout")
            | (Kind::SyncSender, "send")
            | (
                Kind::Writer,
                "write" | "write_all" | "read" | "read_to_end" | "read_to_string" | "flush",
            ) => Kind::Result(Box::new(Kind::Unknown)),
            (Kind::String, "to_owned" | "to_string" | "clone") => Kind::String,
            (Kind::Vec, "clone") => Kind::Vec,
            _ => Kind::Unknown,
        }
    }

    pub fn bind(&mut self, pat: &syn::Pat, kind: Kind) {
        match pat {
            syn::Pat::Ident(p) => {
                let start = p.ident.span().start();
                self.origins
                    .insert(p.ident.to_string(), (start.line, start.column));
                self.bindings.last_mut().unwrap().insert(
                    p.ident.to_string(),
                    if p.by_ref.is_some() {
                        Kind::Reference(Box::new(kind))
                    } else {
                        kind
                    },
                );
            }
            syn::Pat::Type(p) => {
                let k = self.kind(&p.ty);
                self.bind(&p.pat, k);
            }
            _ => {
                struct Names(Vec<String>);
                impl<'a> Visit<'a> for Names {
                    fn visit_pat_ident(&mut self, p: &'a syn::PatIdent) {
                        self.0.push(p.ident.to_string());
                    }
                }
                let mut names = Names(Vec::new());
                names.visit_pat(pat);
                for n in names.0 {
                    self.bindings.last_mut().unwrap().insert(n, Kind::Unknown);
                }
            }
        }
    }

    pub fn has_guard(&self) -> bool {
        self.bindings
            .iter()
            .any(|s| s.values().any(|k| *k == Kind::Guard))
    }
    pub fn binding_origin(&self, expr: &syn::Expr) -> Option<(usize, usize)> {
        self.origins.get(&ident(expr)?).copied()
    }
    pub fn is_current_binding(&self, expr: &syn::Expr) -> bool {
        ident(expr).is_some_and(|name| {
            self.bindings.last().is_some_and(|s| s.contains_key(&name))
                || self.bindings.len() == 2 && self.bindings[0].contains_key(&name)
        })
    }
    fn forget_guards(&mut self) {
        for scope in &mut self.bindings {
            for k in scope.values_mut() {
                if *k == Kind::Guard {
                    *k = Kind::Unknown;
                }
            }
        }
    }
    fn forget(&mut self, name: &str) {
        for s in self.bindings.iter_mut().rev() {
            if let Some(k) = s.get_mut(name) {
                *k = Kind::Unknown;
                break;
            }
        }
    }
    fn forget_captured_guards(&mut self, expr: &syn::Expr) {
        struct Uses(std::collections::HashSet<String>);
        impl<'a> Visit<'a> for Uses {
            fn visit_expr_path(&mut self, p: &'a syn::ExprPath) {
                if let Some(n) = p.path.get_ident() {
                    self.0.insert(n.to_string());
                }
            }
        }
        let mut uses = Uses(Default::default());
        uses.visit_expr(expr);
        for name in uses.0 {
            if self.bindings.iter().rev().find_map(|s| s.get(&name)) == Some(&Kind::Guard) {
                self.forget(&name);
            }
        }
    }
}

pub(crate) fn ident(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(p) => p.path.get_ident().map(ToString::to_string),
        syn::Expr::Paren(p) => ident(&p.expr),
        _ => None,
    }
}

pub(crate) fn is_fs_result(path: &str) -> bool {
    ["std::fs::", "tokio::fs::"].iter().any(|prefix| {
        path.strip_prefix(prefix).is_some_and(|tail| {
            matches!(
                tail,
                "read"
                    | "read_to_string"
                    | "write"
                    | "remove_file"
                    | "remove_dir"
                    | "remove_dir_all"
                    | "create_dir"
                    | "create_dir_all"
                    | "rename"
                    | "copy"
                    | "metadata"
                    | "canonicalize"
                    | "File::open"
                    | "File::create"
            )
        })
    })
}

pub(crate) fn is_spawn(path: &str) -> bool {
    matches!(
        path,
        "std::thread::spawn"
            | "tokio::spawn"
            | "tokio::task::spawn"
            | "tokio::task::spawn_blocking"
            | "async_std::task::spawn"
            | "async_std::task::spawn_blocking"
    )
}

pub(crate) fn span_key(span: proc_macro2::Span) -> (usize, usize, usize, usize) {
    (
        span.start().line,
        span.start().column,
        span.end().line,
        span.end().column,
    )
}

pub(crate) fn discarded_spawns(file: &syn::File, statements: bool) -> Vec<usize> {
    struct Discards {
        keys: std::collections::HashSet<(usize, usize, usize, usize)>,
        statements: bool,
    }
    impl Discards {
        fn record(&mut self, expr: &syn::Expr) {
            match expr {
                syn::Expr::Paren(p) => self.record(&p.expr),
                syn::Expr::Group(g) => self.record(&g.expr),
                syn::Expr::Call(_) => {
                    self.keys.insert(span_key(expr.span()));
                }
                _ => {}
            }
        }
    }
    impl<'a> Visit<'a> for Discards {
        fn visit_stmt(&mut self, s: &'a syn::Stmt) {
            if self.statements
                && let syn::Stmt::Expr(e, Some(_)) = s
            {
                self.record(e);
            }
            syn::visit::visit_stmt(self, s);
        }
        fn visit_local(&mut self, n: &'a syn::Local) {
            if matches!(n.pat, syn::Pat::Wild(_))
                && let Some(i) = &n.init
            {
                self.record(&i.expr);
            }
            syn::visit::visit_local(self, n);
        }
    }
    let mut discarded = Discards {
        keys: Default::default(),
        statements,
    };
    discarded.visit_file(file);
    let mut lines = Vec::new();
    inspect(file, |expr, ctx| {
        if discarded.keys.contains(&span_key(expr.span()))
            && let syn::Expr::Call(c) = expr
            && let syn::Expr::Path(p) = &*c.func
            && is_spawn(&ctx.path(&p.path))
        {
            lines.push(expr.span().start().line);
        }
    });
    lines
}

/// Visit expressions with scope-local evidence, in evaluation order.
pub(crate) fn inspect(file: &syn::File, callback: impl FnMut(&syn::Expr, &Context)) {
    Scanner {
        context: Context::new(&file.items),
        callback,
    }
    .visit_file(file);
}

struct Scanner<F> {
    context: Context,
    callback: F,
}
impl<F: FnMut(&syn::Expr, &Context)> Scanner<F> {
    fn function(&mut self, sig: &syn::Signature, block: &syn::Block) {
        let prev = self.context.clone();
        self.context.bindings = vec![HashMap::new()];
        self.context.origins.clear();
        self.context.in_async = sig.asyncness.is_some();
        self.context.in_const = sig.constness.is_some();
        self.context.loop_depth = 0;
        let start = sig.ident.span().start();
        self.context.execution = (start.line, start.column);
        for generic in &sig.generics.params {
            if let syn::GenericParam::Type(t) = generic {
                self.context
                    .items
                    .names
                    .insert(t.ident.to_string(), "<generic>".into());
            }
        }
        for input in &sig.inputs {
            if let syn::FnArg::Typed(a) = input {
                let k = self.context.kind(&a.ty);
                self.context.bind(&a.pat, k);
            }
        }
        self.visit_block(block);
        self.context = prev;
    }
}

impl<'a, F: FnMut(&syn::Expr, &Context)> Visit<'a> for Scanner<F> {
    fn visit_stmt(&mut self, n: &'a syn::Stmt) {
        // Statement-position macros are also opaque uses of bindings. Expose
        // the macro to consumers without pretending to expand its token body.
        if let syn::Stmt::Macro(m) = n {
            let expr = syn::Expr::Macro(syn::ExprMacro {
                attrs: m.attrs.clone(),
                mac: m.mac.clone(),
            });
            (self.callback)(&expr, &self.context);
        }
        syn::visit::visit_stmt(self, n);
    }
    fn visit_item_const(&mut self, n: &'a syn::ItemConst) {
        let prev = self.context.clone();
        self.context.in_const = true;
        self.context.loop_depth = 0;
        self.visit_expr(&n.expr);
        self.context = prev;
    }
    fn visit_item_static(&mut self, n: &'a syn::ItemStatic) {
        let prev = self.context.clone();
        self.context.in_const = true;
        self.context.loop_depth = 0;
        self.visit_expr(&n.expr);
        self.context = prev;
    }
    fn visit_impl_item_const(&mut self, n: &'a syn::ImplItemConst) {
        let prev = self.context.clone();
        self.context.in_const = true;
        self.context.loop_depth = 0;
        self.visit_expr(&n.expr);
        self.context = prev;
    }
    fn visit_trait_item_const(&mut self, n: &'a syn::TraitItemConst) {
        let prev = self.context.clone();
        self.context.in_const = true;
        self.context.loop_depth = 0;
        if let Some((_, expr)) = &n.default {
            self.visit_expr(expr);
        }
        self.context = prev;
    }
    fn visit_expr_const(&mut self, n: &'a syn::ExprConst) {
        let prev = self.context.clone();
        self.context.in_const = true;
        self.context.loop_depth = 0;
        self.visit_block(&n.block);
        self.context = prev;
    }
    fn visit_item_fn(&mut self, n: &'a syn::ItemFn) {
        if !crate::detectors::policy::has_test_cfg(&n.attrs) {
            self.function(&n.sig, &n.block);
        }
    }
    fn visit_impl_item_fn(&mut self, n: &'a syn::ImplItemFn) {
        if !crate::detectors::policy::has_test_cfg(&n.attrs) {
            self.function(&n.sig, &n.block);
        }
    }
    fn visit_trait_item_fn(&mut self, n: &'a syn::TraitItemFn) {
        if !crate::detectors::policy::has_test_cfg(&n.attrs)
            && let Some(b) = &n.default
        {
            self.function(&n.sig, b);
        }
    }
    fn visit_item_mod(&mut self, n: &'a syn::ItemMod) {
        if crate::detectors::policy::has_test_cfg(&n.attrs) {
            return;
        }
        if let Some((_, items)) = &n.content {
            let prev = self.context.clone();
            self.context = Context::new(items);
            for item in items {
                self.visit_item(item);
            }
            self.context = prev;
        }
    }
    fn visit_block(&mut self, n: &'a syn::Block) {
        let block = self.context.block;
        let start = n.brace_token.span.open().start();
        self.context.block = (start.line, start.column);
        let saved_items = self.context.items.clone();
        let origins = self.context.origins.clone();
        let items: Vec<_> = n
            .stmts
            .iter()
            .filter_map(|s| {
                if let syn::Stmt::Item(i) = s {
                    Some(i.clone())
                } else {
                    None
                }
            })
            .collect();
        self.context.add_items(&items);
        self.context.bindings.push(HashMap::new());
        for stmt in &n.stmts {
            self.visit_stmt(stmt);
            if matches!(
                stmt,
                syn::Stmt::Expr(
                    syn::Expr::Return(_) | syn::Expr::Break(_) | syn::Expr::Continue(_),
                    _
                )
            ) {
                break;
            }
        }
        self.context.bindings.pop();
        self.context.items = saved_items;
        self.context.origins = origins;
        self.context.block = block;
    }
    fn visit_local(&mut self, n: &'a syn::Local) {
        let kind = n
            .init
            .as_ref()
            .map(|i| self.context.expr(&i.expr))
            .unwrap_or(Kind::Unknown);
        if let Some(i) = &n.init {
            self.visit_expr(&i.expr);
            if let Some((_, e)) = &i.diverge {
                self.visit_expr(e);
            }
            let pattern = match &n.pat {
                syn::Pat::Type(p) => &*p.pat,
                pattern => pattern,
            };
            // A guard moved to another binding is no longer owned by its old
            // name. Otherwise dropping the new owner leaves stale live-guard
            // evidence. Wildcard and by-reference bindings do not move it.
            if kind == Kind::Guard
                && matches!(pattern, syn::Pat::Ident(p) if p.by_ref.is_none())
                && let Some(name) = ident(&i.expr)
            {
                self.context.forget(&name);
            }
        }
        self.context.bind(&n.pat, kind);
    }
    fn visit_expr(&mut self, n: &'a syn::Expr) {
        // Suspension happens after evaluating the awaited expression (including drops).
        if let syn::Expr::Await(a) = n {
            self.visit_expr(&a.base);
            (self.callback)(n, &self.context);
            return;
        }
        (self.callback)(n, &self.context);
        syn::visit::visit_expr(self, n);
        if let syn::Expr::Call(c) = n {
            // A guard passed by value may escape or be dropped. Do not keep claiming it is live.
            for arg in &c.args {
                if let Some(name) = ident(arg)
                    && self.context.expr(arg) == Kind::Guard
                {
                    self.context.forget(&name);
                }
            }
        }
    }
    fn visit_expr_closure(&mut self, n: &'a syn::ExprClosure) {
        let prev = self.context.clone();
        self.context.forget_guards();
        self.context.loop_depth = 0;
        self.context.in_async = n.asyncness.is_some();
        self.context.bindings.push(HashMap::new());
        let start = n.span().start();
        self.context.execution = (start.line, start.column);
        for p in &n.inputs {
            self.context.bind(p, Kind::Unknown);
        }
        self.visit_expr(&n.body);
        self.context = prev;
        self.context.forget_captured_guards(&n.body);
    }
    fn visit_expr_async(&mut self, n: &'a syn::ExprAsync) {
        let prev = self.context.clone();
        self.context.forget_guards();
        self.context.in_async = true;
        self.context.loop_depth = 0;
        let start = n.async_token.span.start();
        self.context.execution = (start.line, start.column);
        self.visit_block(&n.block);
        self.context = prev;
    }
    fn visit_expr_if(&mut self, n: &'a syn::ExprIf) {
        self.visit_expr(&n.cond);
        let prev = self.context.clone();
        self.context.bindings.push(HashMap::new());
        if let syn::Expr::Let(l) = &*n.cond {
            self.context.bind(&l.pat, Kind::Unknown);
        }
        self.visit_block(&n.then_branch);
        self.context = prev.clone();
        if let Some((_, e)) = &n.else_branch {
            self.visit_expr(e);
        }
        self.context = prev;
        self.context.forget_guards();
    }
    fn visit_expr_match(&mut self, n: &'a syn::ExprMatch) {
        self.visit_expr(&n.expr);
        let prev = self.context.clone();
        for arm in &n.arms {
            self.context = prev.clone();
            self.context.bindings.push(HashMap::new());
            self.context.bind(&arm.pat, Kind::Unknown);
            if let syn::Pat::Guard(g) = &arm.pat {
                self.visit_expr(&g.guard);
            }
            self.visit_expr(&arm.body);
        }
        self.context = prev;
        self.context.forget_guards();
    }
    fn visit_expr_for_loop(&mut self, n: &'a syn::ExprForLoop) {
        self.visit_expr(&n.expr);
        let prev = self.context.clone();
        if self.context.loop_depth == 0 {
            let p = n.for_token.span.start();
            self.context.loop_start = (p.line, p.column);
        }
        self.context.loop_depth += 1;
        self.context.bindings.push(HashMap::new());
        self.context.bind(&n.pat, Kind::Unknown);
        self.visit_block(&n.body);
        self.context = prev;
        self.context.forget_guards();
    }
    fn visit_expr_while(&mut self, n: &'a syn::ExprWhile) {
        let prev = self.context.clone();
        if self.context.loop_depth == 0 {
            let p = n.while_token.span.start();
            self.context.loop_start = (p.line, p.column);
        }
        self.context.loop_depth += 1;
        self.context.bindings.push(HashMap::new());
        self.visit_expr(&n.cond);
        if let syn::Expr::Let(l) = &*n.cond {
            self.context.bind(&l.pat, Kind::Unknown);
        }
        self.visit_block(&n.body);
        self.context = prev;
        self.context.forget_guards();
    }
    fn visit_expr_loop(&mut self, n: &'a syn::ExprLoop) {
        let prev = self.context.clone();
        if self.context.loop_depth == 0 {
            let p = n.loop_token.span.start();
            self.context.loop_start = (p.line, p.column);
        }
        self.context.loop_depth += 1;
        self.visit_block(&n.body);
        self.context = prev;
        self.context.forget_guards();
    }
}

fn literal_kind(lit: &syn::Lit) -> Kind {
    match lit {
        syn::Lit::Str(_) => Kind::Reference(Box::new(Kind::String)),
        syn::Lit::Int(_)
        | syn::Lit::Float(_)
        | syn::Lit::Bool(_)
        | syn::Lit::Char(_)
        | syn::Lit::Byte(_) => Kind::Copy,
        _ => Kind::Unknown,
    }
}

#[derive(Clone, Default)]
struct ItemScope {
    names: HashMap<String, String>,
    aliases: HashMap<String, syn::Type>,
    returns: HashMap<String, Kind>,
}

impl ItemScope {
    fn add_declarations(&mut self, items: &[syn::Item]) {
        // Item declarations shadow both prelude names and external crate names.
        for item in items {
            let ident = match item {
                syn::Item::Struct(i) => Some(&i.ident),
                syn::Item::Enum(i) => Some(&i.ident),
                syn::Item::Type(i) => Some(&i.ident),
                syn::Item::Mod(i) => Some(&i.ident),
                syn::Item::Fn(i) => Some(&i.sig.ident),
                syn::Item::Trait(i) => Some(&i.ident),
                _ => None,
            };
            if let Some(ident) = ident {
                self.names.insert(ident.to_string(), "<local>".into());
                self.aliases.remove(&ident.to_string());
                self.returns.remove(&ident.to_string());
            }
        }
    }

    fn add_imports_and_aliases(&mut self, items: &[syn::Item]) {
        for item in items {
            if let syn::Item::Type(item) = item
                && item.generics.params.is_empty()
            {
                self.aliases
                    .insert(item.ident.to_string(), (*item.ty).clone());
            }
            if let syn::Item::Use(item) = item {
                self.add_use(
                    if item.leading_colon.is_some() {
                        "::"
                    } else {
                        ""
                    },
                    &item.tree,
                );
            }
        }
    }

    fn add_use(&mut self, prefix: &str, tree: &syn::UseTree) {
        match tree {
            syn::UseTree::Path(p) => self.add_use(&format!("{prefix}{}::", p.ident), &p.tree),
            syn::UseTree::Name(n) => {
                let target = if n.ident == "self" {
                    prefix.trim_end_matches("::").to_string()
                } else {
                    format!("{prefix}{}", n.ident)
                };
                let name = target.rsplit("::").next().unwrap_or("").to_string();
                self.import_name(name, target);
            }
            syn::UseTree::Rename(n) => {
                self.import_name(n.rename.to_string(), format!("{prefix}{}", n.ident));
            }
            syn::UseTree::Group(g) => {
                for item in &g.items {
                    self.add_use(prefix, item);
                }
            }
            syn::UseTree::Glob(_) => {} // A glob provides no reliable local resolution.
        }
    }

    fn import_name(&mut self, name: String, target: String) {
        let mut parts = target.splitn(2, "::");
        let first = parts.next().unwrap_or("");
        let resolved = if let Some(absolute) = target.strip_prefix("::") {
            absolute.to_string()
        } else if let Some(root) = self.names.get(first) {
            parts
                .next()
                .map(|rest| format!("{root}::{rest}"))
                .unwrap_or_else(|| root.clone())
        } else {
            target
        };
        self.returns.remove(&name);
        self.aliases.remove(&name);
        self.names.insert(name, resolved);
    }
}
