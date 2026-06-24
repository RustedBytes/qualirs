use quote::ToTokens;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects public APIs that expose infrastructure/library types.
pub struct PublicApiLeakDetector;

impl Detector for PublicApiLeakDetector {
    fn name(&self) -> &str {
        "Public API Leak"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();
        for item in &file.ast.items {
            if let syn::Item::Fn(func) = item
                && matches!(func.vis, syn::Visibility::Public(_))
            {
                let sig = func.sig.to_token_stream().to_string();
                if let Some(leak) = leaked_crate(&sig) {
                    let line = func.sig.fn_token.span.start().line;
                    smells.push(Smell::new(
                            SmellCategory::Architecture,
                            "Public API Leak",
                            Severity::Warning,
                                                        crate::domain::smell::FindingConfidence::Medium,
                            SourceLocation::new(file.path.clone(), line, line, None),
                            format!("Public function `{}` exposes `{leak}` in its signature", func.sig.ident),
                            "Hide infrastructure types behind domain types or stable adapter interfaces.",
                        ));
                }
            }
        }
        smells
    }
}

fn leaked_crate(text: &str) -> Option<&'static str> {
    [
        "sqlx", "reqwest", "hyper", "tokio", "redis", "diesel", "sea_orm", "tonic",
    ]
    .into_iter()
    .find(|krate| text.contains(&format!("{krate} ::")) || text.contains(&format!("{krate}::")))
}
