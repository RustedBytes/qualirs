use crate::analysis::detector::Detector;
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};
pub struct CloneBeforeMoveIntoCollectionDetector;
impl Detector for CloneBeforeMoveIntoCollectionDetector {
    fn name(&self) -> &str {
        "Clone Before Move Into Collection"
    }
    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        super::perf_utils::movable_clones(&file.ast)
            .into_iter()
            .map(|line| {
                Smell::new(
                    SmellCategory::Performance,
                    self.name(),
                    Severity::Info,
                    FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    "An owned standard collection value is cloned on its final use",
                    "Move the owned value directly into the Vec.",
                )
            })
            .collect()
    }
}
