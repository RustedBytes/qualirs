use syn::visit::Visit;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects usage of inline assembly (`asm!` or `global_asm!`).
///
/// Inline assembly is highly architecture-specific, unsafe, and non-portable.
/// Its usage should be strictly isolated to low-level platform code.
pub struct InlineAssemblyDetector;

impl Detector for InlineAssemblyDetector {
    fn name(&self) -> &str {
        "Inline Assembly"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();
        let mut visitor = AsmVisitor { usages: Vec::new() };

        visitor.visit_file(&file.ast);

        for (line, macro_name) in visitor.usages {
            smells.push(Smell::new(
                SmellCategory::Unsafe,
                "Inline Assembly",
                Severity::Warning,
                                crate::domain::smell::FindingConfidence::High,
                SourceLocation::new(file.path.clone(), line, line, None),
                format!("Use of highly-platform specific `{}` macro", macro_name),
                "Abstract inline assembly behind a safe, cross-platform interface or use compiler intrinsics if possible.",
            ));
        }

        smells
    }
}

struct AsmVisitor {
    usages: Vec<(usize, String)>,
}

impl<'ast> Visit<'ast> for AsmVisitor {
    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if let Some(segment) = node.path.segments.last() {
            let ident = segment.ident.to_string();
            if ident == "asm" || ident == "global_asm" {
                let line = segment.ident.span().start().line;
                self.usages.push((line, format!("{}!", ident)));
            }
        }
        syn::visit::visit_macro(self, node);
    }
}
