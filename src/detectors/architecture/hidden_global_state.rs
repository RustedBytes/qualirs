use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects excessive use of global state.
///
/// Global state (`static mut`, `lazy_static!`, `OnceLock`, `Atomic*` at static scope)
/// hinders testing, parallelism, and leads to hidden dependencies.
pub struct HiddenGlobalStateDetector;

impl Detector for HiddenGlobalStateDetector {
    fn name(&self) -> &str {
        "Hidden Global State"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        let mut globals = Vec::new();

        // 1. Check for `static` items
        for item in &file.ast.items {
            if let syn::Item::Static(st) = item {
                let line = st.ident.span().start().line;
                globals.push(line);
            }
        }

        // 2. Check for `lazy_static!` or `thread_local!` macros
        let mut visitor = MacrosVisitor { lines: Vec::new() };
        visitor.visit_file(&file.ast);
        globals.extend(visitor.lines);

        if globals.len() > thresholds.arch.hidden_global_state {
            let first_line = globals[0];
            smells.push(Smell::new(
                SmellCategory::Architecture,
                "Hidden Global State",
                Severity::Warning,
                                crate::domain::smell::FindingConfidence::Medium,
                SourceLocation::new(file.path.clone(), first_line, first_line, None),
                format!("File contains {} global state objects/macros (threshold: {})", globals.len(), thresholds.arch.hidden_global_state),
                "Refactor to pass state explicitly (Dependency Injection) rather than using globals.",
            ));
        }

        smells
    }
}

struct MacrosVisitor {
    lines: Vec<usize>,
}

impl<'ast> Visit<'ast> for MacrosVisitor {
    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if let Some(segment) = node.path.segments.last() {
            let ident = segment.ident.to_string();
            if ident == "lazy_static" || ident == "thread_local" {
                let line = segment.ident.span().start().line;
                self.lines.push(line);
            }
        }
        syn::visit::visit_macro(self, node);
    }
}
