use quote::ToTokens;

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
        for item in &file.ast.items {
            if let syn::Item::Impl(imp) = item
                && imp.unsafety.is_some()
                && !has_safety_docs(&imp.attrs)
            {
                let trait_name = imp
                    .trait_
                    .as_ref()
                    .and_then(|(_, path, _)| path.segments.last())
                    .map(|seg| seg.ident.to_string())
                    .unwrap_or_default();
                if matches!(trait_name.as_str(), "Send" | "Sync") {
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

fn has_safety_docs(attrs: &[syn::Attribute]) -> bool {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .any(doc_attr_mentions_safety)
}

fn doc_attr_mentions_safety(attr: &syn::Attribute) -> bool {
    let tokens = attr.meta.to_token_stream();
    let text = tokens.to_string();
    text.to_lowercase().contains("safety")
}
