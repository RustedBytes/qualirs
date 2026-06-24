use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects uses of std::mem::transmute — one of the most dangerous Rust operations.
///
/// Transmute can reinterpret arbitrary bit patterns as any type, completely
/// bypassing the type system. Use only with extreme caution.
pub struct TransmuteUsageDetector;

impl Detector for TransmuteUsageDetector {
    fn name(&self) -> &str {
        "Transmute Usage"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        let mut visitor = TransmuteVisitor { usages: Vec::new() };
        visitor.visit_file(&file.ast);

        for (line, context) in &visitor.usages {
            smells.push(Smell::new(
                SmellCategory::Unsafe,
                "Transmute Usage",
                Severity::Critical,
                crate::domain::smell::FindingConfidence::High,
                SourceLocation {
                    file: file.path.clone(),
                    line_start: *line,
                    line_end: *line,
                    column: None,
                },
                format!("Unsafe transmute used: {context}"),
                "Prefer safe conversions: From/Into, as, bytemuck, or zerocopy.",
            ));
        }

        smells
    }
}

struct TransmuteVisitor {
    usages: Vec<(usize, String)>,
}

impl<'ast> Visit<'ast> for TransmuteVisitor {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(ep) = &*node.func {
            let path_str = path_to_string(&ep.path);
            if path_str.contains("transmute") {
                let line = node.paren_token.span.open().start().line;
                self.usages.push((line, path_str));
            }
        }
        syn::visit::visit_expr_call(self, node);
    }
}

fn path_to_string(path: &syn::Path) -> String {
    let segments: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();

    segments.join("::")
}
