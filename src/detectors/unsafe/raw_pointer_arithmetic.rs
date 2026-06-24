use quote::ToTokens;
use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects raw pointer arithmetic (offset, add, sub, wrapping_offset).
///
/// Pointer arithmetic can lead to undefined behavior if the resulting pointer
/// is out of bounds. Prefer slice indexing or iterator arithmetic.
pub struct RawPointerArithmeticDetector;

impl Detector for RawPointerArithmeticDetector {
    fn name(&self) -> &str {
        "Raw Pointer Arithmetic"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        let mut visitor = PtrArithVisitor { usages: Vec::new() };
        visitor.visit_file(&file.ast);

        for (line, method) in &visitor.usages {
            smells.push(Smell::new(
                SmellCategory::Unsafe,
                "Raw Pointer Arithmetic",
                Severity::Warning,
                crate::domain::smell::FindingConfidence::High,
                SourceLocation {
                    file: file.path.clone(),
                    line_start: *line,
                    line_end: *line,
                    column: None,
                },
                format!("Pointer arithmetic used: {method}"),
                "Prefer slice indexing, iterators, or safe wrappers around pointer operations.",
            ));
        }

        smells
    }
}

struct PtrArithVisitor {
    usages: Vec<(usize, String)>,
}

impl<'ast> Visit<'ast> for PtrArithVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        let ptr_methods = [
            "offset",
            "add",
            "sub",
            "wrapping_offset",
            "wrapping_add",
            "wrapping_sub",
            "byte_offset",
            "byte_add",
            "byte_sub",
        ];

        if ptr_methods.iter().any(|m| *m == method) {
            // Check if we are inside an unsafe block or function
            // (Simplification: we just report it, but ideally we'd check nesting)
            // For now, let's just make it more specific by checking the receiver string
            let receiver_str = expr_to_string(&node.receiver);
            if receiver_str.contains("ptr") || receiver_str.contains("raw") {
                let line = node.method.span().start().line;
                self.usages
                    .push((line, format!("{}.{}()", receiver_str, method)));
            }
        }

        syn::visit::visit_expr_method_call(self, node);
    }
}

fn expr_to_string(expr: &syn::Expr) -> String {
    match expr {
        syn::Expr::Path(p) => {
            let last_seg = p.path.segments.last();
            last_seg
                .map(|s| s.ident.to_string())
                .unwrap_or_else(|| "ptr".into())
        }
        syn::Expr::Cast(c) => {
            let type_str = type_to_string(&c.ty);
            if type_str.contains('*') {
                "raw_ptr".to_string()
            } else {
                "cast_expr".to_string()
            }
        }
        syn::Expr::Unary(u) => expr_to_string(&u.expr),
        syn::Expr::Paren(p) => expr_to_string(&p.expr),
        _ => "expr".to_string(),
    }
}

fn type_to_string(ty: &syn::Type) -> String {
    ty.to_token_stream().to_string().to_lowercase()
}
