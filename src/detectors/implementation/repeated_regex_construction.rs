use syn::visit::{
    Visit, visit_expr_call, visit_expr_for_loop, visit_expr_loop, visit_expr_method_call,
    visit_expr_while,
};

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects Regex::new in functions and loops.
pub struct RepeatedRegexConstructionDetector;

impl Detector for RepeatedRegexConstructionDetector {
    fn name(&self) -> &str {
        "Repeated Regex Construction"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = RegexVisitor {
            loop_depth: 0,
            lazy_initializer_depth: 0,
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|(line, in_loop)| {
                Smell::new(
                    SmellCategory::Performance,
                    "Repeated Regex Construction",
                    if in_loop {
                        Severity::Warning
                    } else {
                        Severity::Info
                    },
                    crate::domain::smell::FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    "Regex is constructed at runtime".to_string(),
                    "Store regexes in LazyLock, OnceLock, or lazy_static when they are reused.",
                )
            })
            .collect()
    }
}

struct RegexVisitor {
    loop_depth: usize,
    lazy_initializer_depth: usize,
    findings: Vec<(usize, bool)>,
}

impl<'ast> Visit<'ast> for RegexVisitor {
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

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*node.func {
            let path_str = path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            if is_lazy_initializer(&path_str) {
                self.visit_lazy_initializer_args(&node.args);
                return;
            }
            if path_str.ends_with("Regex::new") && self.lazy_initializer_depth == 0 {
                let line = path.path.segments.last().unwrap().ident.span().start().line;
                self.findings.push((line, self.loop_depth > 0));
            }
        }
        visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if is_lazy_initializer_method(&node.method.to_string()) {
            self.visit_lazy_initializer_args(&node.args);
            return;
        }
        visit_expr_method_call(self, node);
    }
}

impl RegexVisitor {
    fn visit_lazy_initializer_args(
        &mut self,
        args: &syn::punctuated::Punctuated<syn::Expr, syn::Token![,]>,
    ) {
        for arg in args {
            if matches!(arg, syn::Expr::Closure(_)) {
                self.lazy_initializer_depth += 1;
                self.visit_expr(arg);
                self.lazy_initializer_depth -= 1;
            } else {
                self.visit_expr(arg);
            }
        }
    }
}

fn is_lazy_initializer(path: &str) -> bool {
    matches!(
        path,
        "LazyLock::new" | "OnceLock::new" | "Lazy::new" | "OnceCell::new"
    ) || path.ends_with("::LazyLock::new")
        || path.ends_with("::OnceLock::new")
        || path.ends_with("::Lazy::new")
        || path.ends_with("::OnceCell::new")
}

fn is_lazy_initializer_method(method: &str) -> bool {
    matches!(method, "get_or_init" | "get_or_try_init")
}
