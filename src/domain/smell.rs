use std::fmt;
use std::path::PathBuf;

/// High-level category grouping related smells together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SmellCategory {
    Architecture,
    Design,
    Implementation,
    Performance,
    Idiomaticity,
    Concurrency,
    Unsafe,
}

impl fmt::Display for SmellCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Architecture => write!(f, "Architecture"),
            Self::Design => write!(f, "Design"),
            Self::Implementation => write!(f, "Implementation"),
            Self::Performance => write!(f, "Performance"),
            Self::Idiomaticity => write!(f, "Idiomaticity"),
            Self::Concurrency => write!(f, "Concurrency"),
            Self::Unsafe => write!(f, "Unsafe"),
        }
    }
}

impl std::str::FromStr for SmellCategory {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_lowercase().as_str() {
            "architecture" | "arch" => Ok(Self::Architecture),
            "design" => Ok(Self::Design),
            "implementation" | "impl" => Ok(Self::Implementation),
            "performance" | "perf" => Ok(Self::Performance),
            "idiomaticity" | "idiomatic" | "idiom" => Ok(Self::Idiomaticity),
            "concurrency" | "concurrent" => Ok(Self::Concurrency),
            "unsafe" => Ok(Self::Unsafe),
            other => Err(format!(
                "Unknown category: {other}. Use: architecture, design, implementation, performance, idiomaticity, concurrency, unsafe"
            )),
        }
    }
}

/// How severe a detected smell is.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    #[serde(alias = "Info")]
    Info,
    #[serde(alias = "Warning")]
    Warning,
    #[serde(alias = "Critical")]
    Critical,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "INFO"),
            Self::Warning => write!(f, "WARN"),
            Self::Critical => write!(f, "CRIT"),
        }
    }
}

/// How confident QualiRS is that a finding should be user-visible by default.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum FindingConfidence {
    #[serde(alias = "Low")]
    Low,
    #[serde(alias = "Medium")]
    Medium,
    #[serde(alias = "High")]
    High,
}

impl fmt::Display for FindingConfidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
        }
    }
}

/// Location in source code where a smell was detected.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceLocation {
    pub file: PathBuf,
    pub line_start: usize,
    pub line_end: usize,
    pub column: Option<usize>,
}

impl SourceLocation {
    #[allow(dead_code)]
    pub fn new(file: PathBuf, line_start: usize, line_end: usize, column: Option<usize>) -> Self {
        Self {
            file,
            line_start,
            line_end,
            column,
        }
    }
}

impl fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.file.display(), self.line_start)
    }
}

/// A single detected code smell with full context.
#[derive(Debug, Clone)]
pub struct Smell {
    pub code: String,
    pub category: SmellCategory,
    pub name: String,
    pub severity: Severity,
    pub confidence: FindingConfidence,
    pub location: SourceLocation,
    pub message: String,
    pub suggestion: String,
}

impl Smell {
    pub fn new(
        category: SmellCategory,
        name: impl Into<String>,
        severity: Severity,
        confidence: FindingConfidence,
        location: SourceLocation,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        let name = name.into();
        Self {
            code: rule_code_for(&name)
                .unwrap_or(UNKNOWN_RULE_CODE)
                .to_string(),
            category,
            name,
            severity,
            confidence,
            location,
            message: message.into(),
            suggestion: suggestion.into(),
        }
    }
}

pub const UNKNOWN_RULE_CODE: &str = "Q0000";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleMetadata {
    pub code: &'static str,
    pub name: &'static str,
}

pub const RULES: &[RuleMetadata] = &[
    RuleMetadata::new("Q0001", "God Module"),
    RuleMetadata::new("Q0002", "Public API Explosion"),
    RuleMetadata::new("Q0003", "Feature Concentration"),
    RuleMetadata::new("Q0004", "Cyclic Crate Dependency"),
    RuleMetadata::new("Q0005", "Layer Violation"),
    RuleMetadata::new("Q0006", "Unstable Dependency"),
    RuleMetadata::new("Q0007", "Leaky Error Abstraction"),
    RuleMetadata::new("Q0008", "Hidden Global State"),
    RuleMetadata::new("Q0009", "Public API Leak"),
    RuleMetadata::new("Q0010", "Test-only Dependency in Production"),
    RuleMetadata::new("Q0011", "Duplicate Dependency Versions"),
    RuleMetadata::new("Q0012", "Feature Flag Sprawl"),
    RuleMetadata::new("Q0013", "Circular Module Dependency"),
    RuleMetadata::new("Q0014", "Large Trait"),
    RuleMetadata::new("Q0015", "Excessive Generics"),
    RuleMetadata::new("Q0016", "Anemic Struct"),
    RuleMetadata::new("Q0017", "Wide Hierarchy"),
    RuleMetadata::new("Q0018", "Trait Impl Leakage"),
    RuleMetadata::new("Q0019", "Feature Envy"),
    RuleMetadata::new("Q0020", "Broken Constructor"),
    RuleMetadata::new("Q0021", "Rebellious Impl"),
    RuleMetadata::new("Q0022", "Fat Impl"),
    RuleMetadata::new("Q0023", "Primitive Obsession"),
    RuleMetadata::new("Q0024", "Data Clumps"),
    RuleMetadata::new("Q0025", "Multiple Impl Blocks"),
    RuleMetadata::new("Q0026", "God Struct"),
    RuleMetadata::new("Q0027", "Boolean Flag Argument"),
    RuleMetadata::new("Q0028", "Stringly Typed Domain"),
    RuleMetadata::new("Q0029", "Large Error Enum"),
    RuleMetadata::new("Q0030", "Long Function"),
    RuleMetadata::new("Q0031", "Too Many Arguments"),
    RuleMetadata::new("Q0032", "Deep Match Nesting"),
    RuleMetadata::new("Q0033", "Magic Numbers"),
    RuleMetadata::new("Q0034", "Large Enum"),
    RuleMetadata::new("Q0035", "High Cyclomatic Complexity"),
    RuleMetadata::new("Q0036", "Deep If/Else Nesting"),
    RuleMetadata::new("Q0037", "Long Method Chain"),
    RuleMetadata::new("Q0038", "Unsafe Block Overuse"),
    RuleMetadata::new("Q0039", "Lifetime Explosion"),
    RuleMetadata::new("Q0040", "Deeply Nested Type"),
    RuleMetadata::new("Q0041", "Duplicate Match Arms"),
    RuleMetadata::new("Q0042", "Long Closure"),
    RuleMetadata::new("Q0043", "Deep Closure Nesting"),
    RuleMetadata::new("Q0044", "Excessive Clone"),
    RuleMetadata::new("Q0045", "Arc Mutex Overuse"),
    RuleMetadata::new("Q0046", "Large Future"),
    RuleMetadata::new("Q0047", "Async Trait Overhead"),
    RuleMetadata::new("Q0048", "Interior Mutability Abuse"),
    RuleMetadata::new("Q0049", "Unnecessary Allocation in Loop"),
    RuleMetadata::new("Q0050", "Collect Then Iterate"),
    RuleMetadata::new("Q0051", "Repeated Regex Construction"),
    RuleMetadata::new("Q0052", "Missing Collection Preallocation"),
    RuleMetadata::new("Q0053", "Repeated String Conversion in Hot Path"),
    RuleMetadata::new("Q0054", "Needless Intermediate String Formatting"),
    RuleMetadata::new("Q0055", "Vec Contains in Loop"),
    RuleMetadata::new("Q0056", "Sort Before Min or Max"),
    RuleMetadata::new("Q0057", "Full Sort for Single Element"),
    RuleMetadata::new("Q0058", "Clone Before Move Into Collection"),
    RuleMetadata::new("Q0059", "Inefficient Iterator Step"),
    RuleMetadata::new("Q0060", "Chars Count Length Check"),
    RuleMetadata::new("Q0061", "Repeated Expensive Construction in Loop"),
    RuleMetadata::new("Q0062", "Needless Dynamic Dispatch"),
    RuleMetadata::new("Q0063", "Local Lock in Single-Threaded Scope"),
    RuleMetadata::new("Q0064", "Clone on Copy"),
    RuleMetadata::new("Q0065", "Large Value Passed By Value"),
    RuleMetadata::new("Q0066", "Inline Candidate"),
    RuleMetadata::new("Q0067", "Excessive Unwrap"),
    RuleMetadata::new("Q0068", "Unused Result Ignored"),
    RuleMetadata::new("Q0069", "Panic in Library"),
    RuleMetadata::new("Q0070", "Copy + Drop Conflict"),
    RuleMetadata::new("Q0071", "Deref Abuse"),
    RuleMetadata::new("Q0072", "Manual Drop"),
    RuleMetadata::new("Q0073", "Manual Default Constructor"),
    RuleMetadata::new("Q0074", "Manual Option/Result Mapping"),
    RuleMetadata::new("Q0075", "Manual Find/Any Loop"),
    RuleMetadata::new("Q0076", "Needless Explicit Lifetime"),
    RuleMetadata::new("Q0077", "Derivable Impl"),
    RuleMetadata::new("Q0078", "Blocking in Async"),
    RuleMetadata::new("Q0079", "Deadlock Risk"),
    RuleMetadata::new("Q0080", "Spawn Without Join"),
    RuleMetadata::new("Q0081", "Missing Send Bound"),
    RuleMetadata::new("Q0082", "Sync Drop Blocking"),
    RuleMetadata::new("Q0083", "Std Mutex in Async"),
    RuleMetadata::new("Q0084", "Blocking Channel in Async"),
    RuleMetadata::new("Q0085", "Holding Lock Across Await"),
    RuleMetadata::new("Q0086", "Dropped JoinHandle"),
    RuleMetadata::new("Q0087", "Unsafe Without Comment"),
    RuleMetadata::new("Q0088", "Transmute Usage"),
    RuleMetadata::new("Q0089", "Raw Pointer Arithmetic"),
    RuleMetadata::new("Q0090", "Multi Mut Ref Unsafe"),
    RuleMetadata::new("Q0091", "FFI Without Wrapper"),
    RuleMetadata::new("Q0092", "Inline Assembly"),
    RuleMetadata::new("Q0093", "Unsafe Fn Missing Safety Docs"),
    RuleMetadata::new("Q0094", "Unsafe Impl Missing Safety Docs"),
    RuleMetadata::new("Q0095", "Large Unsafe Block"),
    RuleMetadata::new("Q0096", "FFI Type Not repr(C)"),
];

impl RuleMetadata {
    const fn new(code: &'static str, name: &'static str) -> Self {
        Self { code, name }
    }
}

pub fn rule_code_for(name: &str) -> Option<&'static str> {
    find_rule(name).map(|rule| rule.code)
}

fn find_rule(name: &str) -> Option<&'static RuleMetadata> {
    RULES
        .iter()
        .find(|rule| rule.name == name)
        .or_else(|| find_base_rule(name))
}

fn find_base_rule(name: &str) -> Option<&'static RuleMetadata> {
    let (base, _) = name.rsplit_once(" (")?;
    RULES.iter().find(|rule| rule.name == base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_smell_categories() {
        assert_eq!(
            "architecture".parse::<SmellCategory>(),
            Ok(SmellCategory::Architecture)
        );
        assert_eq!("design".parse::<SmellCategory>(), Ok(SmellCategory::Design));
        assert_eq!(
            "implementation".parse::<SmellCategory>(),
            Ok(SmellCategory::Implementation)
        );
        assert_eq!(
            "performance".parse::<SmellCategory>(),
            Ok(SmellCategory::Performance)
        );
        assert_eq!(
            "idiomaticity".parse::<SmellCategory>(),
            Ok(SmellCategory::Idiomaticity)
        );
        assert_eq!(
            "concurrency".parse::<SmellCategory>(),
            Ok(SmellCategory::Concurrency)
        );
        assert_eq!("unsafe".parse::<SmellCategory>(), Ok(SmellCategory::Unsafe));
    }

    #[test]
    fn parses_common_category_aliases() {
        assert_eq!(
            "arch".parse::<SmellCategory>(),
            Ok(SmellCategory::Architecture)
        );
        assert_eq!(
            "impl".parse::<SmellCategory>(),
            Ok(SmellCategory::Implementation)
        );
        assert_eq!(
            "perf".parse::<SmellCategory>(),
            Ok(SmellCategory::Performance)
        );
        assert_eq!(
            "idiom".parse::<SmellCategory>(),
            Ok(SmellCategory::Idiomaticity)
        );
        assert_eq!(
            "idiomatic".parse::<SmellCategory>(),
            Ok(SmellCategory::Idiomaticity)
        );
    }

    #[test]
    fn assigns_rule_codes_to_builtin_smells() {
        let smell = Smell::new(
            SmellCategory::Architecture,
            "God Module (items)",
            Severity::Warning,
            FindingConfidence::High,
            SourceLocation::new(PathBuf::from("src/lib.rs"), 1, 1, None),
            "message",
            "suggestion",
        );

        assert_eq!(smell.code, "Q0001");
        assert_eq!(
            rule_code_for("Duplicate Dependency Versions"),
            Some("Q0011")
        );
    }
}
