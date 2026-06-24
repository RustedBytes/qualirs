use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects `panic!`, `todo!`, and `unimplemented!` macros in non-test code.
///
/// Library crates should propagate errors rather than panic.
pub struct PanicInLibraryDetector;

impl Detector for PanicInLibraryDetector {
    fn name(&self) -> &str {
        "Panic in Library"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        let is_test_file = file.path.to_string_lossy().contains("test")
            || file.path.to_string_lossy().ends_with("_test.rs")
            || file.path.to_string_lossy().ends_with("tests.rs");

        if is_test_file {
            return smells;
        }

        for item in &file.ast.items {
            if let syn::Item::Fn(fn_item) = item {
                // Skip functions annotated with #[test]
                if is_test_fn(fn_item) {
                    continue;
                }

                let mut visitor = PanicVisitor { panics: Vec::new() };
                visitor.visit_block(&fn_item.block);

                for incident in visitor.panics {
                    smells.push(Smell::new(
                        SmellCategory::Idiomaticity,
                        "Panic in Library",
                        Severity::Warning,
                                                crate::domain::smell::FindingConfidence::High,
                        SourceLocation {
                            file: file.path.clone(),
                            line_start: incident.line,
                            line_end: incident.line,
                            column: None,
                        },
                        format!(
                            "Function `{}` uses `{}` — panics should be avoided in library code",
                            fn_item.sig.ident, incident.macro_name
                        ),
                        "Return a Result or use a proper error handling strategy instead of panicking.",
                    ));
                }
            }
        }

        smells
    }
}

fn is_test_fn(fn_item: &syn::ItemFn) -> bool {
    fn_item.attrs.iter().any(|attr| {
        let path = &attr.path();
        path.segments.len() == 1 && path.segments[0].ident == "test"
    })
}

struct PanicIncident {
    line: usize,
    macro_name: &'static str,
}

impl PanicIncident {
    pub fn new(line: usize, macro_name: &'static str) -> Self {
        Self { line, macro_name }
    }
}

struct PanicVisitor {
    panics: Vec<PanicIncident>,
}

impl<'ast> Visit<'ast> for PanicVisitor {
    fn visit_expr_macro(&mut self, node: &'ast syn::ExprMacro) {
        if let Some(name) = check_macro_name(&node.mac.path) {
            let line = node.mac.path.segments[0].ident.span().start().line;
            self.panics.push(PanicIncident::new(line, name));
        }
        syn::visit::visit_expr_macro(self, node);
    }

    fn visit_stmt_macro(&mut self, node: &'ast syn::StmtMacro) {
        if let Some(name) = check_macro_name(&node.mac.path) {
            let line = node.mac.path.segments[0].ident.span().start().line;
            self.panics.push(PanicIncident::new(line, name));
        }
        syn::visit::visit_stmt_macro(self, node);
    }
}

fn check_macro_name(path: &syn::Path) -> Option<&'static str> {
    let last_seg = path.segments.last()?;
    let ident = last_seg.ident.to_string();
    match ident.as_str() {
        "panic" => Some("panic!"),
        "todo" => Some("todo!"),
        "unimplemented" => Some("unimplemented!"),
        _ => None,
    }
}
