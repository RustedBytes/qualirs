use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

use super::shared::path_has_pair;

/// Approximates circular module dependencies by finding reciprocal use paths in one file.
pub struct CircularModuleDependencyDetector;

impl Detector for CircularModuleDependencyDetector {
    fn name(&self) -> &str {
        "Circular Module Dependency"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let text = file.code.to_lowercase();
        let pairs = [
            ("domain", "infrastructure"),
            ("domain", "cli"),
            ("application", "infrastructure"),
            ("service", "repository"),
        ];
        for (a, b) in pairs {
            if path_has_pair(&text, a, b) && path_has_pair(&text, b, a) {
                return vec![Smell::new(
                    SmellCategory::Architecture,
                    "Circular Module Dependency",
                    Severity::Warning,
                    crate::domain::smell::FindingConfidence::Medium,
                    SourceLocation::new(file.path.clone(), 1, 1, None),
                    format!("File references both `{a} -> {b}` and `{b} -> {a}` paths"),
                    "Break the cycle with a trait, adapter, or dependency inversion boundary.",
                )];
            }
        }
        Vec::new()
    }
}
