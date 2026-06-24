use syn::visit::{Visit, visit_expr_method_call};

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects `.clone()` on obvious Copy values.
pub struct CloneOnCopyDetector;

impl Detector for CloneOnCopyDetector {
    fn name(&self) -> &str {
        "Clone on Copy"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = CloneOnCopyVisitor {
            copy_vars: std::collections::HashSet::new(),
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|(line, name)| {
                Smell::new(
                    SmellCategory::Performance,
                    "Clone on Copy",
                    Severity::Info,
                    crate::domain::smell::FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    format!("Value `{name}` appears to be Copy but is cloned"),
                    "Use the value directly instead of calling clone().",
                )
            })
            .collect()
    }
}

struct CloneOnCopyVisitor {
    copy_vars: std::collections::HashSet<String>,
    findings: Vec<(usize, String)>,
}

impl<'ast> Visit<'ast> for CloneOnCopyVisitor {
    fn visit_local(&mut self, node: &'ast syn::Local) {
        if let syn::Pat::Type(pat_ty) = &node.pat
            && is_copy_type(&pat_ty.ty)
            && let syn::Pat::Ident(ident) = &*pat_ty.pat
        {
            self.copy_vars.insert(ident.ident.to_string());
        }
        syn::visit::visit_local(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "clone" {
            if let syn::Expr::Path(path) = &*node.receiver {
                if let Some(seg) = path.path.segments.last() {
                    let name = seg.ident.to_string();
                    if self.copy_vars.contains(&name) {
                        self.findings.push((node.method.span().start().line, name));
                    }
                }
            } else if matches!(&*node.receiver, syn::Expr::Lit(_)) {
                self.findings
                    .push((node.method.span().start().line, "literal".into()));
            }
        }
        visit_expr_method_call(self, node);
    }
}

fn is_copy_type(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(path) if path.path.segments.last().map(|s| {
        matches!(s.ident.to_string().as_str(), "bool" | "char" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "f32" | "f64")
    }).unwrap_or(false))
}
