use syn::visit::{Visit, visit_expr_method_call};

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects `push_str(&format!(...))`, which allocates an avoidable temporary String.
pub struct NeedlessIntermediateStringFormattingDetector;

impl Detector for NeedlessIntermediateStringFormattingDetector {
    fn name(&self) -> &str {
        "Needless Intermediate String Formatting"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = FormattingVisitor {
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|line| {
                Smell::new(
                    SmellCategory::Performance,
                    "Needless Intermediate String Formatting",
                    Severity::Info,
                                        crate::domain::smell::FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    "`push_str(&format!(...))` allocates a temporary String",
                    "Use `write!`/`writeln!` with `std::fmt::Write` to append formatted data directly.",
                )
            })
            .collect()
    }
}

struct FormattingVisitor {
    findings: Vec<usize>,
}

impl<'ast> Visit<'ast> for FormattingVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "push_str"
            && node.args.len() == 1
            && node.args.first().is_some_and(is_reference_to_format_macro)
        {
            self.findings.push(node.method.span().start().line);
        }

        visit_expr_method_call(self, node);
    }
}

fn is_reference_to_format_macro(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Reference(reference) => {
            matches!(&*reference.expr, syn::Expr::Macro(expr) if expr.mac.path.is_ident("format"))
        }
        syn::Expr::Paren(paren) => is_reference_to_format_macro(&paren.expr),
        _ => false,
    }
}
