use crate::analysis::detector::Detector;
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};
pub struct SortBeforeMinMaxDetector;
impl Detector for SortBeforeMinMaxDetector {
    fn name(&self) -> &str {
        "Sort Before Min or Max"
    }
    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        super::perf_utils::sort_selections(&file.ast, false)
            .into_iter()
            .map(|line| {
                Smell::new(
                    SmellCategory::Performance,
                    self.name(),
                    Severity::Warning,
                    FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    "An owned Vec is sorted only to select an extremum",
                    "Use min/max when full ordering is unnecessary.",
                )
            })
            .collect()
    }
}
