use syn::visit::{Visit, visit_expr};

use crate::analysis::detector::Detector;
use crate::detectors::policy::is_test_path;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects functions with excessive `.unwrap()` or `.expect()` calls.
pub struct ExcessiveUnwrapDetector;

impl Detector for ExcessiveUnwrapDetector {
    fn name(&self) -> &str {
        "Excessive Unwrap"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        if is_test_path(&file.path) {
            return smells;
        }

        for item in &file.ast.items {
            if let syn::Item::Fn(fn_item) = item {
                let mut visitor = UnwrapCounter::new();
                visitor.visit_block(&fn_item.block);

                if visitor.unwrap_count > thresholds.r#impl.control_flow.excessive_unwrap {
                    let line = fn_item.sig.fn_token.span.start().line;

                    smells.push(Smell::new(
                        SmellCategory::Idiomaticity,
                        "Excessive Unwrap",
                        Severity::Warning,
                        crate::domain::smell::FindingConfidence::High,
                        SourceLocation {
                            file: file.path.clone(),
                            line_start: line,
                            line_end: line,
                            column: None,
                        },
                        format!(
                            "Function `{}` has {} unwrap/expect calls (threshold: {})",
                            fn_item.sig.ident,
                            visitor.unwrap_count,
                            thresholds.r#impl.control_flow.excessive_unwrap
                        ),
                        "Use proper error handling with ?, map_err, or match instead of unwrap().",
                    ));
                }
            }
        }

        smells
    }
}

struct UnwrapCounter {
    unwrap_count: usize,
}

impl UnwrapCounter {
    fn new() -> Self {
        Self { unwrap_count: 0 }
    }
}

impl<'ast> Visit<'ast> for UnwrapCounter {
    fn visit_expr(&mut self, expr: &'ast syn::Expr) {
        if let syn::Expr::MethodCall(call) = expr
            && (call.method == "unwrap" || call.method == "expect")
            && !is_regex_literal_constructor(&call.receiver)
        {
            self.unwrap_count += 1;
        }
        visit_expr(self, expr);
    }
}

fn is_regex_literal_constructor(expr: &syn::Expr) -> bool {
    let syn::Expr::Call(call) = expr else {
        return false;
    };
    let syn::Expr::Path(path) = &*call.func else {
        return false;
    };
    let Some(last) = path.path.segments.last() else {
        return false;
    };

    last.ident == "new"
        && path
            .path
            .segments
            .iter()
            .rev()
            .nth(1)
            .is_some_and(|segment| segment.ident == "Regex")
        && call.args.first().is_some_and(
            |arg| matches!(arg, syn::Expr::Lit(lit) if matches!(lit.lit, syn::Lit::Str(_))),
        )
}
