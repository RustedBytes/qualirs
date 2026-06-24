use std::sync::{OnceLock, RwLock};

use crate::domain::smell::FindingConfidence;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ArchThresholds {
    pub god_module_loc: usize,
    pub god_module_items: usize,
    pub public_api_ratio: f64,
    pub feature_concentration: usize,
    pub hidden_global_state: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct DesignThresholds {
    pub large_trait_methods: usize,
    pub excessive_generics: usize,
    pub deep_trait_bounds: usize,
    pub wide_hierarchy: usize,
    pub fat_impl_methods: usize,
    pub god_struct_fields: usize,
    pub primitive_obsession_fields: usize,
    pub data_clumps_args: usize,
    pub data_clumps_occurrences: usize,
    pub stringly_typed_fields: usize,
    pub large_error_enum_variants: usize,
}

/// Thresholds for control-flow complexity smells.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub(crate) struct ControlFlowThresholds {
    pub long_function_loc: usize,
    pub long_closure_loc: usize,
    pub deep_closure_nesting: usize,
    pub cyclomatic_complexity: usize,
    pub too_many_arguments: usize,
    pub deep_match_nesting: usize,
    pub deep_if_else: usize,
    pub excessive_unwrap: usize,
    pub large_enum_variants: usize,
    pub long_method_chain: usize,
    pub lifetime_explosion: usize,
}

/// Thresholds for type-safety and unsafe-usage smells.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub(crate) struct TypeSafetyThresholds {
    pub unsafe_block_overuse: usize,
    pub deeply_nested_type: usize,
    pub interior_mutability_abuse: usize,
}

/// Combined implementation thresholds composed of focused sub-groups.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ImplThresholds {
    #[serde(default, flatten)]
    pub(crate) control_flow: ControlFlowThresholds,
    #[serde(default, flatten)]
    pub(crate) type_safety: TypeSafetyThresholds,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ConcurrencyThresholds {
    pub large_future_loc: usize,
    pub arc_mutex_overuse: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct UnsafeThresholds {
    pub unsafe_without_comment: bool,
}

/// Policy toggles for suppressing findings that are intentionally noisy in
/// common Rust layouts.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PolicyConfig {
    #[serde(default = "default_true")]
    pub skip_tests: bool,
    #[serde(default = "default_test_path_markers")]
    pub test_path_markers: Vec<String>,
    #[serde(default = "default_true")]
    pub skip_examples: bool,
    #[serde(default = "default_true")]
    pub skip_benches: bool,
    #[serde(default = "default_true")]
    pub skip_generated: bool,
    #[serde(default = "default_true")]
    pub skip_macro_heavy_files: bool,
    #[serde(default = "default_true")]
    pub skip_data_carrier_structs: bool,
    #[serde(default = "default_true")]
    pub skip_template_structs: bool,
    #[serde(default = "default_data_carrier_struct_suffixes")]
    pub data_carrier_struct_suffixes: Vec<String>,
}

/// User-facing precision mode for deciding which confidence levels are reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Precision {
    #[serde(alias = "Conservative")]
    Conservative,
    #[serde(alias = "Balanced")]
    Balanced,
    #[serde(alias = "Exploratory")]
    Exploratory,
}

impl Precision {
    pub(crate) fn includes(self, confidence: FindingConfidence) -> bool {
        match self {
            Self::Conservative => confidence == FindingConfidence::High,
            Self::Balanced => confidence >= FindingConfidence::Medium,
            Self::Exploratory => true,
        }
    }
}

impl std::fmt::Display for Precision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conservative => write!(f, "conservative"),
            Self::Balanced => write!(f, "balanced"),
            Self::Exploratory => write!(f, "exploratory"),
        }
    }
}

impl std::str::FromStr for Precision {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_lowercase().as_str() {
            "conservative" => Ok(Self::Conservative),
            "balanced" => Ok(Self::Balanced),
            "exploratory" => Ok(Self::Exploratory),
            other => Err(format!(
                "Unknown precision: {other}. Use: conservative, balanced, exploratory"
            )),
        }
    }
}

/// Thresholds that control when a smell is reported.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Thresholds {
    #[serde(default)]
    pub arch: ArchThresholds,
    #[serde(default)]
    pub design: DesignThresholds,
    #[serde(default)]
    pub r#impl: ImplThresholds,
    #[serde(default)]
    pub concurrency: ConcurrencyThresholds,
    #[serde(default)]
    pub r#unsafe: UnsafeThresholds,
}

static CURRENT_THRESHOLDS: OnceLock<RwLock<Thresholds>> = OnceLock::new();

pub(crate) fn configure_thresholds(thresholds: &Thresholds) {
    *threshold_store().write().expect("threshold lock poisoned") = thresholds.clone();
}

pub(crate) fn current_thresholds() -> Thresholds {
    threshold_store()
        .read()
        .expect("threshold lock poisoned")
        .clone()
}

fn threshold_store() -> &'static RwLock<Thresholds> {
    CURRENT_THRESHOLDS.get_or_init(|| RwLock::new(Thresholds::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default_thresholds_are_sane() {
        let config = Config::default();
        assert_eq!(config.thresholds.r#impl.control_flow.long_function_loc, 50);
        assert_eq!(config.thresholds.r#impl.control_flow.too_many_arguments, 6);
        assert_eq!(config.thresholds.r#impl.control_flow.excessive_unwrap, 3);
        assert_eq!(
            config.thresholds.r#impl.control_flow.cyclomatic_complexity,
            15
        );
        assert_eq!(config.thresholds.r#impl.control_flow.deep_match_nesting, 3);
        assert_eq!(config.thresholds.r#impl.control_flow.deep_if_else, 4);
        assert_eq!(
            config.thresholds.r#impl.control_flow.large_enum_variants,
            20
        );
        assert_eq!(config.thresholds.r#impl.control_flow.lifetime_explosion, 4);
        assert_eq!(config.thresholds.r#impl.type_safety.deeply_nested_type, 3);
        assert_eq!(
            config
                .thresholds
                .r#impl
                .type_safety
                .interior_mutability_abuse,
            5
        );
        assert_eq!(config.precision, Precision::Conservative);

        // Arch
        assert_eq!(config.thresholds.arch.hidden_global_state, 3);

        // Design
        assert_eq!(config.thresholds.design.fat_impl_methods, 20);
        assert_eq!(config.thresholds.design.primitive_obsession_fields, 4);
        assert_eq!(config.thresholds.design.data_clumps_args, 3);
        assert_eq!(config.thresholds.design.data_clumps_occurrences, 3);

        // Policy
        assert!(config.policy.skip_tests);
        assert!(config.policy.skip_examples);
        assert!(config.policy.skip_benches);
        assert!(config.policy.skip_generated);
        assert!(config.policy.skip_macro_heavy_files);
        assert!(config.policy.skip_data_carrier_structs);
        assert!(config.policy.skip_template_structs);
        assert_eq!(config.threads, 0);
        assert!(
            config
                .policy
                .data_carrier_struct_suffixes
                .iter()
                .any(|suffix| suffix == "Command")
        );
        assert!(
            config
                .policy
                .data_carrier_struct_suffixes
                .iter()
                .any(|suffix| suffix == "Session")
        );
    }

    #[test]
    fn default_toml_round_trips() {
        let toml = Config::default_toml().expect("serialize default config");
        let config: Config = toml::from_str(&toml).expect("parse default config");

        assert!(toml.contains("min_severity = \"info\""));
        assert!(toml.contains("threads = 0"));
        assert_eq!(config.min_severity, crate::domain::smell::Severity::Info);
        assert_eq!(config.precision, Precision::Conservative);
        assert_eq!(config.threads, 0);
        assert_eq!(config.thresholds.arch.god_module_loc, 1000);
        assert!(config.exclude_paths.iter().any(|path| path == "target"));
        assert!(config.ignore_findings.is_empty());
        assert!(toml.contains("ignore_findings = []"));
        assert!(toml.contains("precision = \"conservative\""));
        assert!(toml.contains("[policy]"));
        assert!(toml.contains("skip_tests = true"));
        assert!(toml.contains("skip_examples = true"));
        assert!(toml.contains("skip_benches = true"));
        assert!(toml.contains("skip_generated = true"));
        assert!(toml.contains("skip_macro_heavy_files = true"));
    }

    #[test]
    fn config_accepts_precision_modes() {
        let config: Config = toml::from_str(
            r#"
precision = "balanced"
"#,
        )
        .expect("parse balanced precision");

        assert_eq!(config.precision, Precision::Balanced);
        assert!(config.validate().is_ok());

        let config: Config = toml::from_str(
            r#"
precision = "Exploratory"
"#,
        )
        .expect("parse title-case exploratory precision");

        assert_eq!(config.precision, Precision::Exploratory);
    }

    #[test]
    fn config_rejects_malformed_precision() {
        let err = toml::from_str::<Config>(
            r#"
precision = "strict"
"#,
        )
        .expect_err("invalid precision should fail to parse");

        assert!(err.to_string().contains("precision"));
    }

    #[test]
    fn policy_config_accepts_partial_toml() {
        let config: Config = toml::from_str(
            r#"
[policy]
skip_tests = false
"#,
        )
        .expect("parse partial policy config");

        assert!(!config.policy.skip_tests);
        assert!(config.policy.skip_examples);
        assert!(config.policy.skip_data_carrier_structs);
        assert!(
            config
                .policy
                .test_path_markers
                .iter()
                .any(|marker| marker == "tests")
        );
        assert!(
            config
                .policy
                .test_path_markers
                .iter()
                .any(|marker| marker == "fuzz")
        );
    }

    #[test]
    fn config_accepts_legacy_title_case_severity() {
        let toml = Config::default_toml()
            .expect("serialize default config")
            .replace("min_severity = \"info\"", "min_severity = \"Info\"");
        let config: Config = toml::from_str(&toml).expect("parse default config");

        assert_eq!(config.min_severity, crate::domain::smell::Severity::Info);
    }

    #[test]
    fn config_accepts_ignored_finding_codes() {
        let config: Config = toml::from_str(
            r#"
ignore_findings = ["Q0001", "q0068"]
"#,
        )
        .expect("parse ignored finding codes");

        assert_eq!(config.ignore_findings, ["Q0001", "q0068"]);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn config_accepts_thread_count() {
        let config: Config = toml::from_str(
            r#"
threads = 4
"#,
        )
        .expect("parse thread count");

        assert_eq!(config.threads, 4);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn config_rejects_malformed_ignored_finding_codes() {
        let config: Config = toml::from_str(
            r#"
ignore_findings = ["god_module"]
"#,
        )
        .expect("parse ignored finding codes");

        let err = config.validate().expect_err("invalid ignored finding code");
        assert!(err.to_string().contains("invalid ignored finding code"));
    }

    #[test]
    fn write_default_file_refuses_to_overwrite_without_force() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("qualirs.toml");
        std::fs::write(&path, "existing = true\n").expect("write existing config");

        let err = Config::write_default_file(&path, false).expect_err("should refuse overwrite");
        assert!(err.to_string().contains("already exists"));
        assert_eq!(
            std::fs::read_to_string(&path).expect("read existing config"),
            "existing = true\n"
        );
    }

    #[test]
    fn write_default_file_can_force_overwrite() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("qualirs.toml");
        std::fs::write(&path, "existing = true\n").expect("write existing config");

        Config::write_default_file(&path, true).expect("force write default config");
        let config = Config::load_from_file(&path).expect("load written config");

        assert_eq!(config.min_severity, crate::domain::smell::Severity::Info);
        assert_eq!(config.thresholds.r#impl.control_flow.long_function_loc, 50);
    }
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            arch: ArchThresholds {
                god_module_loc: 1000,
                god_module_items: 20,
                public_api_ratio: 0.7,
                feature_concentration: 15,
                hidden_global_state: 3,
            },
            design: DesignThresholds {
                large_trait_methods: 15,
                excessive_generics: 5,
                deep_trait_bounds: 4,
                wide_hierarchy: 10,
                fat_impl_methods: 20,
                god_struct_fields: 20,
                primitive_obsession_fields: 4,
                data_clumps_args: 3,
                data_clumps_occurrences: 3,
                stringly_typed_fields: 3,
                large_error_enum_variants: 12,
            },
            r#impl: ImplThresholds {
                control_flow: ControlFlowThresholds {
                    long_function_loc: 50,
                    long_closure_loc: 25,
                    deep_closure_nesting: 3,
                    cyclomatic_complexity: 15,
                    too_many_arguments: 6,
                    deep_match_nesting: 3,
                    deep_if_else: 4,
                    excessive_unwrap: 3,
                    large_enum_variants: 20,
                    long_method_chain: 4,
                    lifetime_explosion: 4,
                },
                type_safety: TypeSafetyThresholds {
                    unsafe_block_overuse: 5,
                    deeply_nested_type: 3,
                    interior_mutability_abuse: 5,
                },
            },
            concurrency: ConcurrencyThresholds {
                large_future_loc: 100,
                arc_mutex_overuse: 3,
            },
            r#unsafe: UnsafeThresholds {
                unsafe_without_comment: true,
            },
        }
    }
}

/// Root configuration loaded from qualirs.toml or defaults.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    /// Number of Rayon worker threads to use for analysis. A value of 0 uses
    /// Rayon's default, which is typically all logical CPUs.
    #[serde(default)]
    pub threads: usize,
    #[serde(default)]
    pub thresholds: Thresholds,
    #[serde(default)]
    pub policy: PolicyConfig,
    #[serde(default = "default_exclude_paths")]
    pub exclude_paths: Vec<String>,
    #[serde(default, alias = "ignored_findings", alias = "ignore_codes")]
    pub ignore_findings: Vec<String>,
    #[serde(default = "default_min_severity")]
    pub min_severity: crate::domain::smell::Severity,
    #[serde(default)]
    pub precision: Precision,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            threads: 0,
            thresholds: Thresholds::default(),
            policy: PolicyConfig::default(),
            exclude_paths: default_exclude_paths(),
            ignore_findings: Vec::new(),
            min_severity: default_min_severity(),
            precision: Precision::default(),
        }
    }
}

fn default_exclude_paths() -> Vec<String> {
    vec!["target".into(), ".git".into(), "node_modules".into()]
}

fn default_min_severity() -> crate::domain::smell::Severity {
    crate::domain::smell::Severity::Info
}

impl Default for Precision {
    fn default() -> Self {
        Self::Conservative
    }
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            skip_tests: true,
            test_path_markers: default_test_path_markers(),
            skip_examples: true,
            skip_benches: true,
            skip_generated: true,
            skip_macro_heavy_files: true,
            skip_data_carrier_structs: true,
            skip_template_structs: true,
            data_carrier_struct_suffixes: default_data_carrier_struct_suffixes(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_test_path_markers() -> Vec<String> {
    vec![
        "tests".into(),
        "test".into(),
        "tests.rs".into(),
        "_tests.rs".into(),
        "fuzz".into(),
        "fuzz_targets".into(),
    ]
}

fn default_data_carrier_struct_suffixes() -> Vec<String> {
    vec![
        "Activity".into(),
        "Command".into(),
        "Config".into(),
        "ConfigFile".into(),
        "Descriptor".into(),
        "Details".into(),
        "Dto".into(),
        "DTO".into(),
        "Entry".into(),
        "Event".into(),
        "Failure".into(),
        "Finding".into(),
        "FormData".into(),
        "Grant".into(),
        "Hit".into(),
        "Inspection".into(),
        "Item".into(),
        "Link".into(),
        "Metrics".into(),
        "Notification".into(),
        "Options".into(),
        "Outcome".into(),
        "Overview".into(),
        "Page".into(),
        "Query".into(),
        "Report".into(),
        "Request".into(),
        "Response".into(),
        "Result".into(),
        "Settings".into(),
        "SettingsFile".into(),
        "Session".into(),
        "Snapshot".into(),
        "Stats".into(),
        "Summary".into(),
        "Template".into(),
        "Variant".into(),
        "View".into(),
        "Vulnerability".into(),
    ]
}

impl Config {
    pub fn default_toml() -> anyhow::Result<String> {
        toml::to_string_pretty(&Self::default()).map_err(Into::into)
    }

    pub fn write_default_file(path: &std::path::Path, force: bool) -> anyhow::Result<()> {
        if path.exists() && !force {
            anyhow::bail!(
                "{} already exists. Use --force to overwrite it.",
                path.display()
            );
        }

        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(path, Self::default_toml()?)?;
        Ok(())
    }

    pub fn load_from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    pub fn load_or_default(dir: &std::path::Path) -> Self {
        let config_path = dir.join("qualirs.toml");
        if config_path.exists() {
            Self::load_from_file(&config_path).unwrap_or_else(|e| {
                eprintln!("Warning: failed to load {}: {e}", config_path.display());
                Self::default()
            })
        } else {
            Self::default()
        }
    }

    fn validate(&self) -> anyhow::Result<()> {
        for code in &self.ignore_findings {
            if !is_rule_code(code) {
                anyhow::bail!(
                    "invalid ignored finding code `{code}`. Use Q followed by four digits, for example Q0001"
                );
            }
        }

        Ok(())
    }
}

pub(crate) fn is_rule_code(code: &str) -> bool {
    let bytes = code.as_bytes();
    bytes.len() == 5
        && bytes[0].eq_ignore_ascii_case(&b'Q')
        && bytes[1..].iter().all(u8::is_ascii_digit)
}
