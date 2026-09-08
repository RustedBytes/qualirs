use crate::analysis::detector::Detector;
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};
pub struct FullSortForSingleElementDetector;
impl Detector for FullSortForSingleElementDetector {
    fn name(&self) -> &str {
        "Full Sort for Single Element"
    }
    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        super::perf_utils::sort_selections(&file.ast, true)
            .into_iter()
            .map(|line| {
                Smell::new(
                    SmellCategory::Performance,
                    self.name(),
                    Severity::Info,
                    FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    "An owned Vec is sorted only to select one rank",
                    "Consider select_nth_unstable when the remaining ordering is irrelevant.",
                )
            })
            .collect()
    }
}
