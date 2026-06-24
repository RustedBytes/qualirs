use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects patterns that create multiple mutable references through unsafe.
///
/// Patterns like `&mut *ptr` inside unsafe blocks that could lead to
/// aliasing violations (undefined behavior).
pub struct MultiMutRefUnsafeDetector;

impl Detector for MultiMutRefUnsafeDetector {
    fn name(&self) -> &str {
        "Multi Mut Ref Unsafe"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        let mut visitor = MutRefVisitor {
            mut_ref_casts: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        // Flag if there are multiple mutable ref casts in the same file
        if visitor.mut_ref_casts.len() >= 2 {
            for (line, context) in &visitor.mut_ref_casts {
                smells.push(Smell::new(
                    SmellCategory::Unsafe,
                    "Multi Mut Ref Unsafe",
                    Severity::Critical,
                                        crate::domain::smell::FindingConfidence::High,
                    SourceLocation {
                        file: file.path.clone(),
                        line_start: *line,
                        line_end: *line,
                        column: None,
                    },
                    format!("Multiple mutable reference casts via unsafe: {context}"),
                    "Multiple &mut references to the same data is UB. Use RefCell, Cell, or split_at_mut.",
                ));
            }
        }

        smells
    }
}

struct MutRefVisitor {
    mut_ref_casts: Vec<(usize, String)>,
}

impl<'ast> Visit<'ast> for MutRefVisitor {
    fn visit_expr(&mut self, expr: &'ast syn::Expr) {
        // Match &mut *expr pattern (deref + mutable reborrow)
        if let syn::Expr::Reference(r) = expr
            && r.mutability.is_some()
            && let syn::Expr::Unary(u) = &*r.expr
            && let syn::UnOp::Deref(_) = u.op
        {
            let line = r.and_token.span.start().line;
            let inner = expr_to_short_string(&u.expr);
            self.mut_ref_casts.push((line, format!("&mut *{}", inner)));
        }

        // Also match ptr.as_mut().unwrap() patterns
        if let syn::Expr::MethodCall(mc) = expr
            && mc.method == "as_mut"
        {
            let line = mc.method.span().start().line;
            let receiver = expr_to_short_string(&mc.receiver);
            self.mut_ref_casts
                .push((line, format!("{}.as_mut()", receiver)));
        }

        syn::visit::visit_expr(self, expr);
    }
}

fn expr_to_short_string(expr: &syn::Expr) -> String {
    match expr {
        syn::Expr::Path(p) => {
            let last_seg = p.path.segments.last();
            last_seg
                .map(|s| s.ident.to_string())
                .unwrap_or_else(|| "_".into())
        }
        syn::Expr::Field(f) => {
            let base = expr_to_short_string(&f.base);
            match &f.member {
                syn::Member::Named(n) => format!("{}.{}", base, n),
                syn::Member::Unnamed(i) => format!("{}.{}", base, i.index),
            }
        }
        _ => "_".to_string(),
    }
}
