use quote::ToTokens;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects unsafe functions without a Safety documentation section.
pub struct UnsafeFnMissingSafetyDocsDetector;

impl Detector for UnsafeFnMissingSafetyDocsDetector {
    fn name(&self) -> &str {
        "Unsafe Fn Missing Safety Docs"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();
        for item in &file.ast.items {
            if let syn::Item::Fn(func) = item
                && func.sig.unsafety.is_some()
                && is_public(&func.vis)
                && !has_safety_docs(&func.attrs)
            {
                let line = func.sig.fn_token.span.start().line;
                smells.push(Smell::new(
                    SmellCategory::Unsafe,
                    "Unsafe Fn Missing Safety Docs",
                    Severity::Warning,
                    crate::domain::smell::FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    format!(
                        "Public unsafe function `{}` lacks a # Safety docs section",
                        func.sig.ident
                    ),
                    "Document the caller obligations under a `# Safety` heading.",
                ));
            }
        }
        smells
    }
}

fn is_public(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
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
