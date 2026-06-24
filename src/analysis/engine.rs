use std::path::Path;
use std::sync::{Mutex, OnceLock};

use rayon::prelude::*;

use crate::analysis::detector::Detector;
use crate::domain::config::Config;
use crate::domain::smell::{Severity, Smell};
use crate::domain::source::SourceFile;
use crate::infrastructure::walker::RustFileWalker;

static ANALYSIS_CONFIG_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// The central analysis engine. Owns a set of detectors and runs them
/// across a codebase.
pub struct Engine {
    detectors: Vec<Box<dyn Detector>>,
    config: Config,
}

impl Engine {
    pub fn new(config: Config) -> Self {
        Self {
            detectors: Vec::new(),
            config,
        }
    }

    /// Register a detector. Call before `analyze`.
    #[inline]
    pub fn register(&mut self, detector: Box<dyn Detector>) {
        self.detectors.push(detector);
    }

    /// Register all built-in detectors.
    pub fn register_defaults(&mut self) {
        use crate::detectors;

        // Architecture
        self.register(Box::new(
            detectors::architecture::god_module::GodModuleDetector,
        ));
        self.register(Box::new(
            detectors::architecture::public_api_explosion::PublicApiExplosionDetector,
        ));
        self.register(Box::new(
            detectors::architecture::feature_concentration::FeatureConcentrationDetector,
        ));
        self.register(Box::new(
            detectors::architecture::cyclic_crate_dependency::CyclicDependencyDetector,
        ));
        self.register(Box::new(
            detectors::architecture::layer_violation::LayerViolationDetector,
        ));
        self.register(Box::new(
            detectors::architecture::unstable_dependency::UnstableDependencyDetector,
        ));
        self.register(Box::new(
            detectors::architecture::leaky_error::LeakyErrorAbstractionDetector,
        ));
        self.register(Box::new(
            detectors::architecture::hidden_global_state::HiddenGlobalStateDetector,
        ));
        self.register(Box::new(
            detectors::architecture::public_api_leak::PublicApiLeakDetector,
        ));
        self.register(Box::new(
            detectors::architecture::project_hygiene::TestOnlyDependencyInProductionDetector,
        ));
        self.register(Box::new(
            detectors::architecture::project_hygiene::DuplicateDependencyVersionsDetector,
        ));
        self.register(Box::new(
            detectors::architecture::project_hygiene::FeatureFlagSprawlDetector,
        ));
        self.register(Box::new(
            detectors::architecture::project_hygiene::CircularModuleDependencyDetector,
        ));

        // Design
        self.register(Box::new(detectors::design::large_trait::LargeTraitDetector));
        self.register(Box::new(
            detectors::design::excessive_generics::ExcessiveGenericsDetector,
        ));
        self.register(Box::new(
            detectors::design::anemic_struct::AnemicStructDetector,
        ));
        self.register(Box::new(
            detectors::design::wide_hierarchy::WideHierarchyDetector,
        ));
        self.register(Box::new(
            detectors::design::trait_impl_leakage::TraitImplLeakageDetector,
        ));
        self.register(Box::new(
            detectors::design::feature_envy::FeatureEnvyDetector,
        ));
        self.register(Box::new(
            detectors::design::broken_constructor::BrokenConstructorDetector,
        ));
        self.register(Box::new(
            detectors::design::rebellious_impl::RebelliousImplDetector,
        ));
        self.register(Box::new(detectors::design::deref_abuse::DerefAbuseDetector));
        self.register(Box::new(detectors::design::manual_drop::ManualDropDetector));
        self.register(Box::new(detectors::design::fat_impl::FatImplDetector));
        self.register(Box::new(
            detectors::design::primitive_obsession::PrimitiveObsessionDetector,
        ));
        self.register(Box::new(detectors::design::data_clumps::DataClumpsDetector));
        self.register(Box::new(
            detectors::design::multiple_impl_blocks::MultipleImplBlocksDetector,
        ));
        self.register(Box::new(detectors::design::god_struct::GodStructDetector));
        self.register(Box::new(
            detectors::design::boolean_flag_argument::BooleanFlagArgumentDetector,
        ));
        self.register(Box::new(
            detectors::design::stringly_typed_domain::StringlyTypedDomainDetector,
        ));
        self.register(Box::new(
            detectors::design::large_error_enum::LargeErrorEnumDetector,
        ));

        // Implementation
        self.register(Box::new(
            detectors::implementation::long_function::LongFunctionDetector,
        ));
        self.register(Box::new(
            detectors::implementation::too_many_arguments::TooManyArgumentsDetector,
        ));
        self.register(Box::new(
            detectors::implementation::excessive_unwrap::ExcessiveUnwrapDetector,
        ));
        self.register(Box::new(
            detectors::implementation::deep_match::DeepMatchDetector,
        ));
        self.register(Box::new(
            detectors::implementation::excessive_clone::ExcessiveCloneDetector,
        ));
        self.register(Box::new(
            detectors::implementation::magic_numbers::MagicNumbersDetector,
        ));
        self.register(Box::new(
            detectors::implementation::large_enum::LargeEnumDetector,
        ));
        self.register(Box::new(
            detectors::implementation::cyclomatic_complexity::CyclomaticComplexityDetector,
        ));
        self.register(Box::new(
            detectors::implementation::deep_if_else::DeepIfElseDetector,
        ));
        self.register(Box::new(
            detectors::implementation::long_method_chain::LongMethodChainDetector,
        ));
        self.register(Box::new(
            detectors::implementation::unused_result::UnusedResultDetector,
        ));
        self.register(Box::new(
            detectors::implementation::panic_in_library::PanicInLibraryDetector,
        ));
        self.register(Box::new(
            detectors::implementation::unsafe_overuse::UnsafeOveruseDetector,
        ));
        self.register(Box::new(
            detectors::implementation::lifetime_explosion::LifetimeExplosionDetector,
        ));
        self.register(Box::new(
            detectors::implementation::copy_drop_conflict::CopyDropConflictDetector,
        ));
        self.register(Box::new(
            detectors::implementation::deeply_nested_type::DeeplyNestedTypeDetector,
        ));
        self.register(Box::new(
            detectors::implementation::interior_mutability_abuse::InteriorMutabilityAbuseDetector,
        ));
        self.register(Box::new(detectors::implementation::unnecessary_allocation_in_loop::UnnecessaryAllocationInLoopDetector));
        self.register(Box::new(
            detectors::implementation::collect_then_iterate::CollectThenIterateDetector,
        ));
        self.register(Box::new(detectors::implementation::repeated_regex_construction::RepeatedRegexConstructionDetector));
        self.register(Box::new(
            detectors::implementation::missing_collection_preallocation::MissingCollectionPreallocationDetector,
        ));
        self.register(Box::new(
            detectors::implementation::repeated_string_conversion::RepeatedStringConversionDetector,
        ));
        self.register(Box::new(
            detectors::implementation::needless_intermediate_string_formatting::NeedlessIntermediateStringFormattingDetector,
        ));
        self.register(Box::new(
            detectors::implementation::vec_contains_in_loop::VecContainsInLoopDetector,
        ));
        self.register(Box::new(
            detectors::implementation::sort_before_min_max::SortBeforeMinMaxDetector,
        ));
        self.register(Box::new(
            detectors::implementation::full_sort_for_single_element::FullSortForSingleElementDetector,
        ));
        self.register(Box::new(
            detectors::implementation::clone_before_move_into_collection::CloneBeforeMoveIntoCollectionDetector,
        ));
        self.register(Box::new(
            detectors::implementation::inefficient_iterator_step::InefficientIteratorStepDetector,
        ));
        self.register(Box::new(
            detectors::implementation::chars_count_length_check::CharsCountLengthCheckDetector,
        ));
        self.register(Box::new(
            detectors::implementation::repeated_expensive_construction::RepeatedExpensiveConstructionDetector,
        ));
        self.register(Box::new(
            detectors::implementation::needless_dynamic_dispatch::NeedlessDynamicDispatchDetector,
        ));
        self.register(Box::new(
            detectors::implementation::local_lock_in_single_threaded_scope::LocalLockInSingleThreadedScopeDetector,
        ));
        self.register(Box::new(
            detectors::implementation::clone_on_copy::CloneOnCopyDetector,
        ));
        self.register(Box::new(
            detectors::implementation::large_value_passed_by_value::LargeValuePassedByValueDetector,
        ));
        self.register(Box::new(
            detectors::implementation::inline_candidate::InlineCandidateDetector,
        ));
        self.register(Box::new(
            detectors::implementation::manual_default_constructor::ManualDefaultConstructorDetector,
        ));
        self.register(Box::new(detectors::implementation::manual_option_result_mapping::ManualOptionResultMappingDetector));
        self.register(Box::new(
            detectors::implementation::manual_find_loop::ManualFindLoopDetector,
        ));
        self.register(Box::new(
            detectors::implementation::needless_explicit_lifetime::NeedlessExplicitLifetimeDetector,
        ));
        self.register(Box::new(
            detectors::implementation::derivable_impl::DerivableImplDetector,
        ));
        self.register(Box::new(
            detectors::implementation::duplicate_match_arms::DuplicateMatchArmsDetector,
        ));
        self.register(Box::new(
            detectors::implementation::long_closure::LongClosureDetector,
        ));
        self.register(Box::new(
            detectors::implementation::deep_closure_nesting::DeepClosureNestingDetector,
        ));

        // Concurrency
        self.register(Box::new(
            detectors::concurrency::blocking_in_async::BlockingInAsyncDetector,
        ));
        self.register(Box::new(
            detectors::concurrency::large_future::LargeFutureDetector,
        ));
        self.register(Box::new(
            detectors::concurrency::arc_mutex_overuse::ArcMutexOveruseDetector,
        ));
        self.register(Box::new(
            detectors::concurrency::deadlock_risk::DeadlockRiskDetector,
        ));
        self.register(Box::new(
            detectors::concurrency::spawn_without_join::SpawnWithoutJoinDetector,
        ));
        self.register(Box::new(
            detectors::concurrency::missing_send_bound::MissingSendBoundDetector,
        ));
        self.register(Box::new(
            detectors::concurrency::sync_drop_blocking::SyncDropBlockingDetector,
        ));
        self.register(Box::new(
            detectors::concurrency::async_trait_overhead::AsyncTraitOverheadDetector,
        ));
        self.register(Box::new(
            detectors::concurrency::std_mutex_in_async::StdMutexInAsyncDetector,
        ));
        self.register(Box::new(
            detectors::concurrency::blocking_channel_in_async::BlockingChannelInAsyncDetector,
        ));
        self.register(Box::new(
            detectors::concurrency::holding_lock_across_await::HoldingLockAcrossAwaitDetector,
        ));
        self.register(Box::new(
            detectors::concurrency::dropped_join_handle::DroppedJoinHandleDetector,
        ));

        // Unsafe
        self.register(Box::new(
            detectors::r#unsafe::unsafe_without_comment::UnsafeWithoutCommentDetector,
        ));
        self.register(Box::new(
            detectors::r#unsafe::transmute_usage::TransmuteUsageDetector,
        ));
        self.register(Box::new(
            detectors::r#unsafe::raw_pointer_arithmetic::RawPointerArithmeticDetector,
        ));
        self.register(Box::new(
            detectors::r#unsafe::multi_mut_ref_unsafe::MultiMutRefUnsafeDetector,
        ));
        self.register(Box::new(
            detectors::r#unsafe::ffi_without_wrapper::FfiWithoutWrapperDetector,
        ));
        self.register(Box::new(
            detectors::r#unsafe::inline_assembly::InlineAssemblyDetector,
        ));
        self.register(Box::new(
            detectors::r#unsafe::unsafe_fn_missing_safety_docs::UnsafeFnMissingSafetyDocsDetector,
        ));
        self.register(Box::new(
            detectors::r#unsafe::unsafe_impl_safety_docs::UnsafeImplSafetyDocsDetector,
        ));
        self.register(Box::new(
            detectors::r#unsafe::large_unsafe_block::LargeUnsafeBlockDetector,
        ));
        self.register(Box::new(
            detectors::r#unsafe::ffi_type_not_repr_c::FfiTypeNotReprCDetector,
        ));
    }

    /// Analyze all Rust files under `path` and return detected smells.
    pub fn analyze(&self, path: &Path) -> AnalysisReport {
        if self.config.threads > 0 {
            return rayon::ThreadPoolBuilder::new()
                .num_threads(self.config.threads)
                .build()
                .expect("analysis thread pool should be valid")
                .install(|| self.analyze_with_current_pool(path));
        }

        self.analyze_with_current_pool(path)
    }

    fn analyze_with_current_pool(&self, path: &Path) -> AnalysisReport {
        let _config_guard = ANALYSIS_CONFIG_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("analysis config lock poisoned");
        crate::detectors::policy::configure(&self.config.policy);
        crate::domain::config::configure_thresholds(&self.config.thresholds);

        let walker = RustFileWalker::new(path, &self.config.exclude_paths);
        let files = walker.collect_files();
        let ignored_findings = self.ignored_findings();

        let (all_smells, errors) = files
            .par_iter()
            .filter(|file_path| {
                !crate::detectors::policy::is_skipped_path_with_policy(
                    file_path,
                    &self.config.policy,
                )
            })
            .map(|file_path| match SourceFile::from_path(file_path.clone()) {
                Ok(source) => {
                    if crate::detectors::policy::is_skipped_source_with_policy(
                        &source,
                        &self.config.policy,
                    ) {
                        return (Vec::new(), Vec::new());
                    }

                    let mut smells = Vec::new();
                    for detector in &self.detectors {
                        smells.extend(detector.detect(&source));
                    }
                    smells.retain(|smell| {
                        should_report_smell(&self.config, &ignored_findings, &source, smell)
                    });
                    (smells, Vec::new())
                }
                Err(e) => (Vec::new(), vec![e]),
            })
            .reduce(
                || (Vec::new(), Vec::new()),
                |mut left, mut right| {
                    left.0.append(&mut right.0);
                    left.1.append(&mut right.1);
                    left
                },
            );

        let total_files = files.len();

        AnalysisReport::new(all_smells, total_files, errors)
    }

    fn ignored_findings(&self) -> Vec<String> {
        self.config
            .ignore_findings
            .iter()
            .map(|code| code.to_ascii_uppercase())
            .collect()
    }
}

fn should_report_smell(
    config: &Config,
    ignored_findings: &[String],
    source: &SourceFile,
    smell: &Smell,
) -> bool {
    smell.severity >= config.min_severity
        && config.precision.includes(smell.confidence)
        && !ignored_findings.iter().any(|code| code == &smell.code)
        && !has_pre_line_ignore(source, smell)
}

fn has_pre_line_ignore(source: &SourceFile, smell: &Smell) -> bool {
    if smell.location.line_start <= 1 {
        return false;
    }

    let previous_line_number = smell.location.line_start - 1;

    source
        .code
        .lines()
        .nth(previous_line_number.saturating_sub(1))
        .is_some_and(|line| pre_line_ignore_matches(line, &smell.code))
}

fn pre_line_ignore_matches(line: &str, smell_code: &str) -> bool {
    let Some(comment) = comment_text(line) else {
        return false;
    };

    let Some(directive) = ignore_directive(comment) else {
        return false;
    };

    let ignored_codes: Vec<&str> = directive
        .split(|ch: char| ch == ',' || ch.is_whitespace())
        .filter(|part| !part.is_empty())
        .collect();

    ignored_codes.is_empty()
        || ignored_codes
            .iter()
            .any(|code| code.eq_ignore_ascii_case(smell_code))
}

fn comment_text(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if let Some(comment) = trimmed.strip_prefix("//") {
        Some(comment.trim_start())
    } else {
        trimmed
            .strip_prefix("/*")
            .map(|comment| comment.trim_end_matches("*/").trim())
    }
}

fn ignore_directive(comment: &str) -> Option<&str> {
    let trimmed = comment.trim_start();
    let lower = trimmed.to_ascii_lowercase();

    if lower.starts_with("qualirs:ignore") {
        Some(trimmed["qualirs:ignore".len()..].trim())
    } else if lower.starts_with("qualirs: ignore") {
        Some(trimmed["qualirs: ignore".len()..].trim())
    } else if lower.starts_with("qualirs-ignore") {
        Some(trimmed["qualirs-ignore".len()..].trim())
    } else {
        None
    }
}

/// Result of analyzing a codebase.
pub struct AnalysisReport {
    pub smells: Vec<Smell>,
    pub total_files: usize,
    pub parse_errors: Vec<crate::domain::source::ParseError>,
}

#[derive(Default)]
pub struct SmellList<'a>(pub Vec<&'a Smell>);

#[allow(dead_code)]
pub struct CategorySmells<'a>(
    pub std::collections::HashMap<crate::domain::smell::SmellCategory, SmellList<'a>>,
);

impl<'a> CategorySmells<'a> {
    pub fn new(
        map: std::collections::HashMap<crate::domain::smell::SmellCategory, SmellList<'a>>,
    ) -> Self {
        Self(map)
    }
}

impl AnalysisReport {
    pub fn new(
        smells: Vec<Smell>,
        total_files: usize,
        parse_errors: Vec<crate::domain::source::ParseError>,
    ) -> Self {
        Self {
            smells,
            total_files,
            parse_errors,
        }
    }

    /// Smells grouped by category.
    #[allow(dead_code)]
    pub fn by_category(&self) -> CategorySmells<'_> {
        let mut map = std::collections::HashMap::new();
        for smell in &self.smells {
            map.entry(smell.category)
                .or_insert_with(SmellList::default)
                .0
                .push(smell);
        }
        CategorySmells::new(map)
    }

    pub fn total_smells(&self) -> usize {
        self.smells.len()
    }

    pub fn count_by_severity(&self, severity: Severity) -> usize {
        self.smells
            .iter()
            .filter(|s| s.severity == severity)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use crate::domain::config::Precision;
    use crate::domain::smell::{FindingConfidence, SmellCategory, SourceLocation};

    use super::*;

    #[test]
    fn ignored_findings_filter_by_rule_code_case_insensitively() {
        let config = Config {
            ignore_findings: vec!["q0001".into()],
            ..Config::default()
        };
        let engine = Engine::new(config.clone());
        let ignored_findings = engine.ignored_findings();
        let smell = Smell::new(
            SmellCategory::Architecture,
            "God Module (items)",
            Severity::Warning,
            crate::domain::smell::FindingConfidence::High,
            SourceLocation::new(PathBuf::from("src/lib.rs"), 1, 1, None),
            "message",
            "suggestion",
        );

        let source =
            SourceFile::from_source(PathBuf::from("src/lib.rs"), String::from("fn main() {}\n"))
                .expect("parse source");

        assert!(!should_report_smell(
            &config,
            &ignored_findings,
            &source,
            &smell
        ));
    }

    #[test]
    fn pre_line_ignore_filters_matching_rule_code() {
        let config = Config::default();
        let ignored_findings = Vec::new();
        let source = SourceFile::from_source(
            PathBuf::from("src/lib.rs"),
            String::from("// qualirs:ignore q0031\nfn f(a: i32, b: i32, c: i32) {}\n"),
        )
        .expect("parse source");
        let smell = Smell::new(
            SmellCategory::Implementation,
            "Too Many Arguments",
            Severity::Warning,
            crate::domain::smell::FindingConfidence::High,
            SourceLocation::new(PathBuf::from("src/lib.rs"), 2, 2, None),
            "message",
            "suggestion",
        );

        assert!(!should_report_smell(
            &config,
            &ignored_findings,
            &source,
            &smell
        ));
    }

    #[test]
    fn pre_line_ignore_without_code_filters_any_rule_on_next_line() {
        let source = SourceFile::from_source(
            PathBuf::from("src/lib.rs"),
            String::from("// qualirs-ignore\nfn f(a: i32, b: i32, c: i32) {}\n"),
        )
        .expect("parse source");
        let smell = Smell::new(
            SmellCategory::Implementation,
            "Too Many Arguments",
            Severity::Warning,
            crate::domain::smell::FindingConfidence::High,
            SourceLocation::new(PathBuf::from("src/lib.rs"), 2, 2, None),
            "message",
            "suggestion",
        );

        assert!(has_pre_line_ignore(&source, &smell));
    }

    #[test]
    fn pre_line_ignore_does_not_filter_other_rule_codes() {
        let config = Config::default();
        let ignored_findings = Vec::new();
        let source = SourceFile::from_source(
            PathBuf::from("src/lib.rs"),
            String::from("// qualirs:ignore Q0001\nfn f(a: i32, b: i32, c: i32) {}\n"),
        )
        .expect("parse source");
        let smell = Smell::new(
            SmellCategory::Implementation,
            "Too Many Arguments",
            Severity::Warning,
            crate::domain::smell::FindingConfidence::High,
            SourceLocation::new(PathBuf::from("src/lib.rs"), 2, 2, None),
            "message",
            "suggestion",
        );

        assert!(should_report_smell(
            &config,
            &ignored_findings,
            &source,
            &smell
        ));
    }

    #[test]
    fn engine_applies_pre_line_ignores_after_detection() {
        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(
            dir.path().join("args.rs"),
            "// qualirs:ignore Q0031\nfn many_args(a: i32, b: i32, c: i32) -> i32 { a + b + c }\n",
        )
        .expect("write source");

        let strict_config = Config {
            precision: Precision::Balanced,
            thresholds: crate::domain::config::Thresholds {
                r#impl: crate::domain::config::ImplThresholds {
                    control_flow: crate::domain::config::ControlFlowThresholds {
                        too_many_arguments: 2,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Config::default()
        };
        let mut engine = Engine::new(strict_config);
        engine.register(Box::new(
            crate::detectors::implementation::too_many_arguments::TooManyArgumentsDetector,
        ));

        let report = engine.analyze(dir.path());
        assert_eq!(report.total_smells(), 0);
    }

    #[test]
    fn configured_thresholds_are_used_by_detectors() {
        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(
            dir.path().join("args.rs"),
            "fn many_args(a: i32, b: i32, c: i32) -> i32 { a + b + c }\n",
        )
        .expect("write source");

        let strict_config = Config {
            precision: Precision::Balanced,
            thresholds: crate::domain::config::Thresholds {
                r#impl: crate::domain::config::ImplThresholds {
                    control_flow: crate::domain::config::ControlFlowThresholds {
                        too_many_arguments: 2,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Config::default()
        };
        let mut strict_engine = Engine::new(strict_config);
        strict_engine.register(Box::new(
            crate::detectors::implementation::too_many_arguments::TooManyArgumentsDetector,
        ));
        let strict_report = strict_engine.analyze(dir.path());
        assert!(
            strict_report
                .smells
                .iter()
                .any(|smell| smell.name == "Too Many Arguments")
        );

        let lenient_config = Config {
            precision: Precision::Balanced,
            thresholds: crate::domain::config::Thresholds {
                r#impl: crate::domain::config::ImplThresholds {
                    control_flow: crate::domain::config::ControlFlowThresholds {
                        too_many_arguments: 10,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Config::default()
        };
        let mut lenient_engine = Engine::new(lenient_config);
        lenient_engine.register(Box::new(
            crate::detectors::implementation::too_many_arguments::TooManyArgumentsDetector,
        ));
        let lenient_report = lenient_engine.analyze(dir.path());
        assert_eq!(lenient_report.total_smells(), 0);
    }

    #[test]
    fn precision_filters_by_finding_confidence() {
        let config = Config::default();
        let ignored_findings = Vec::new();
        let source =
            SourceFile::from_source(PathBuf::from("src/lib.rs"), String::from("fn main() {}\n"))
                .expect("parse source");

        let high = test_smell("Unused Result Ignored", FindingConfidence::High);
        let medium = test_smell("Too Many Arguments", FindingConfidence::Medium);
        let low = test_smell("Magic Numbers", FindingConfidence::Low);

        assert!(should_report_smell(
            &config,
            &ignored_findings,
            &source,
            &high
        ));
        assert!(!should_report_smell(
            &config,
            &ignored_findings,
            &source,
            &medium
        ));
        assert!(!should_report_smell(
            &config,
            &ignored_findings,
            &source,
            &low
        ));

        let balanced = Config {
            precision: Precision::Balanced,
            ..Config::default()
        };
        assert!(should_report_smell(
            &balanced,
            &ignored_findings,
            &source,
            &medium
        ));
        assert!(!should_report_smell(
            &balanced,
            &ignored_findings,
            &source,
            &low
        ));

        let exploratory = Config {
            precision: Precision::Exploratory,
            ..Config::default()
        };
        assert!(should_report_smell(
            &exploratory,
            &ignored_findings,
            &source,
            &low
        ));
    }

    fn test_smell(name: &str, confidence: FindingConfidence) -> Smell {
        Smell::new(
            SmellCategory::Implementation,
            name,
            Severity::Warning,
            confidence,
            SourceLocation::new(PathBuf::from("src/lib.rs"), 1, 1, None),
            "message",
            "suggestion",
        )
    }

    #[test]
    fn unsafe_without_comment_threshold_can_disable_detector() {
        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(dir.path().join("unsafe.rs"), "fn f() { unsafe { } }\n")
            .expect("write source");

        let disabled_config = Config {
            thresholds: crate::domain::config::Thresholds {
                r#unsafe: crate::domain::config::UnsafeThresholds {
                    unsafe_without_comment: false,
                },
                ..Default::default()
            },
            ..Config::default()
        };
        let mut engine = Engine::new(disabled_config);
        engine.register(Box::new(
            crate::detectors::r#unsafe::unsafe_without_comment::UnsafeWithoutCommentDetector,
        ));

        let report = engine.analyze(dir.path());
        assert_eq!(report.total_smells(), 0);
    }

    #[test]
    fn configured_threads_are_used_by_analysis_pool() {
        struct ThreadCountDetector {
            observed: Arc<AtomicUsize>,
        }

        impl Detector for ThreadCountDetector {
            fn name(&self) -> &str {
                "Thread Count"
            }

            fn detect(&self, _file: &SourceFile) -> Vec<Smell> {
                self.observed
                    .store(rayon::current_num_threads(), Ordering::SeqCst);
                Vec::new()
            }
        }

        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(dir.path().join("lib.rs"), "fn f() {}\n").expect("write source");

        let observed = Arc::new(AtomicUsize::new(0));
        let mut engine = Engine::new(Config {
            threads: 2,
            ..Config::default()
        });
        engine.register(Box::new(ThreadCountDetector {
            observed: Arc::clone(&observed),
        }));

        let report = engine.analyze(dir.path());

        assert_eq!(report.total_files, 1);
        assert_eq!(observed.load(Ordering::SeqCst), 2);
    }
}
