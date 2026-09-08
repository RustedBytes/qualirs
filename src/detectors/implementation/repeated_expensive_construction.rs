use syn::spanned::Spanned;

use crate::analysis::{detector::Detector, evidence};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};

/// Looks for known constructors with fixed inputs in an executed loop body.
pub struct RepeatedExpensiveConstructionDetector;

impl Detector for RepeatedExpensiveConstructionDetector {
    fn name(&self) -> &str {
        "Repeated Expensive Construction in Loop"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut findings = Vec::new();
        evidence::inspect(&file.ast, |expr, ctx| {
            if ctx.loop_depth == 0 || ctx.in_const {
                return;
            }
            let syn::Expr::Call(call) = expr else {
                return;
            };
            let syn::Expr::Path(path) = &*call.func else {
                return;
            };
            let path = ctx.path(&path.path);
            let confidence = match path.as_str() {
                "url::Url::parse"
                | "glob::Pattern::new"
                | "globset::Glob::new"
                | "scraper::Selector::parse" => FindingConfidence::High,
                // Unresolved names and owned path construction do not prove avoidable work.
                "Url::parse" | "Selector::parse" | "PathBuf::from" | "std::path::PathBuf::from" => {
                    FindingConfidence::Low
                }
                _ => return,
            };
            // Names and method calls can change every iteration even if declared outside it.
            if call.args.is_empty() || !call.args.iter().all(literal_input) {
                return;
            }
            let line = expr.span().start().line;
            findings.push(Smell::new(SmellCategory::Performance, self.name(), Severity::Info,
                confidence, SourceLocation::new(file.path.clone(), line, line, None),
                format!("`{path}` is constructed in a loop body from fixed input"),
                "Consider reusing the constructed value if ownership and mutation allow it; owned values stored each iteration may require separate construction."));
        });
        findings
    }
}

fn literal_input(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Lit(_) => true,
        syn::Expr::Reference(e) => literal_input(&e.expr),
        syn::Expr::Paren(e) => literal_input(&e.expr),
        _ => false,
    }
}
