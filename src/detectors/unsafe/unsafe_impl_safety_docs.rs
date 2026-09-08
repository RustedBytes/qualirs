use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects public unsafe Send/Sync impls without safety documentation.
pub struct UnsafeImplSafetyDocsDetector;

impl Detector for UnsafeImplSafetyDocsDetector {
    fn name(&self) -> &str {
        "Unsafe Impl Missing Safety Docs"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();
        let docs = crate::analysis::source_notes::SafetyDocs::new(file);
        let ctx = crate::analysis::evidence::Context::new(&file.ast.items);
        for item in &file.ast.items {
            if let syn::Item::Impl(imp) = item
                && imp.unsafety.is_some()
                && !docs.contains(imp.unsafety.unwrap().span)
            {
                let trait_path = imp
                    .trait_
                    .as_ref()
                    .map(|(path, _)| ctx.path(path))
                    .unwrap_or_default();
                if matches!(
                    trait_path.as_str(),
                    "Send"
                        | "Sync"
                        | "std::marker::Send"
                        | "std::marker::Sync"
                        | "core::marker::Send"
                        | "core::marker::Sync"
                ) {
                    let trait_name = trait_path.rsplit("::").next().unwrap_or("");
                    let line = imp.impl_token.span.start().line;
                    smells.push(Smell::new(
                            SmellCategory::Unsafe,
                            "Unsafe Impl Missing Safety Docs",
                            Severity::Critical,
                                                        crate::domain::smell::FindingConfidence::High,
                            SourceLocation::new(file.path.clone(), line, line, None),
                            format!("Unsafe `{trait_name}` impl lacks a safety explanation"),
                            "Add a SAFETY comment explaining why the impl upholds Send/Sync invariants.",
                        ));
                }
            }
        }
        smells
    }
}
