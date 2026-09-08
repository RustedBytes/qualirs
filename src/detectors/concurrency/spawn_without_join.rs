use crate::analysis::{detector::Detector, evidence};
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};
pub struct SpawnWithoutJoinDetector;
impl Detector for SpawnWithoutJoinDetector {
    fn name(&self) -> &str {
        "Spawn Without Join"
    }
    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        evidence::discarded_spawns(&file.ast, true)
            .into_iter()
            .map(|line| {
                Smell::new(
                    SmellCategory::Concurrency,
                    self.name(),
                    Severity::Warning,
                    FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    "A known task/thread spawn call discards its JoinHandle",
                    "Keep and join/await the handle, or document intentional detachment.",
                )
            })
            .collect()
    }
}
