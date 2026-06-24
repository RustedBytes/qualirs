use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects excessive feature gates in one file.
pub struct FeatureFlagSprawlDetector;

impl Detector for FeatureFlagSprawlDetector {
    fn name(&self) -> &str {
        "Feature Flag Sprawl"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let count = file.code.matches("cfg(feature").count();
        if count > 8 {
            vec![Smell::new(
                SmellCategory::Architecture,
                "Feature Flag Sprawl",
                Severity::Info,
                crate::domain::smell::FindingConfidence::Medium,
                SourceLocation::new(file.path.clone(), 1, 1, None),
                format!("File contains {count} feature-gated cfgs"),
                "Consolidate feature-specific code behind modules or adapter layers.",
            )]
        } else {
            Vec::new()
        }
    }
}
