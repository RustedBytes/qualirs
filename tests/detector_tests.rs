mod common;

use common::{assert_clean, assert_smell_count};
use qualirs::analysis::detector::Detector;
use qualirs::domain::source::SourceFile;
use std::path::PathBuf;

// ─── Architecture ──────────────────────────────────────────

mod god_module {
    use super::*;
    use qualirs::detectors::architecture::god_module::GodModuleDetector;

    static DETECTOR: GodModuleDetector = GodModuleDetector;

    #[test]
    fn detects_many_items() {
        // Generate 25 functions to exceed the 20-item threshold
        let fns: String = (0..25)
            .map(|i| format!("fn func_{i}() {{}}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_smell_count(&DETECTOR, &fns, "God Module (items)", 1);
    }

    #[test]
    fn clean_few_items() {
        let code = "fn main() {}";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_module_registry_file() {
        let mods: String = (0..25)
            .map(|i| format!("pub mod detector_{i};"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_clean(&DETECTOR, &mods);
    }
}

mod public_api_explosion {
    use super::*;
    use qualirs::detectors::architecture::public_api_explosion::PublicApiExplosionDetector;

    static DETECTOR: PublicApiExplosionDetector = PublicApiExplosionDetector;

    #[test]
    fn detects_all_pub() {
        let code = "\
pub fn a() {}
pub fn b() {}
pub fn c() {}
pub fn d() {}
pub fn e() {}
pub fn f() {}
";
        assert_smell_count(&DETECTOR, code, "Public API Explosion", 1);
    }

    #[test]
    fn clean_mostly_private() {
        let code = "\
fn a() {}
fn b() {}
fn c() {}
fn d() {}
pub fn e() {}
";
        assert_clean(&DETECTOR, code);
    }
}

mod cyclic_crate_dependency {
    use super::*;
    use qualirs::detectors::architecture::cyclic_crate_dependency::CyclicDependencyDetector;

    static DETECTOR: CyclicDependencyDetector = CyclicDependencyDetector;

    #[test]
    fn clean_same_file_name_in_different_module() {
        let code = "\
use crate::de::Deserialize;
use crate::ser::Serialize;
";
        let source = SourceFile::from_source("src/private/de.rs".into(), code.to_string()).unwrap();
        let smells = DETECTOR.detect(&source);
        assert!(
            smells
                .iter()
                .all(|smell| smell.name != "Cyclic Crate Dependency"),
            "Should not treat crate::de as private::de importing itself. Smells found: {smells:?}"
        );
    }

    #[test]
    fn detects_exact_self_import() {
        let code = "\
use crate::private::de::Helper;
use crate::ser::Serialize;
";
        let source = SourceFile::from_source("src/private/de.rs".into(), code.to_string()).unwrap();
        let smells = DETECTOR.detect(&source);
        assert_eq!(
            smells
                .iter()
                .filter(|smell| smell.name == "Cyclic Crate Dependency")
                .count(),
            1
        );
    }
}

// ─── Design ────────────────────────────────────────────────

mod large_trait {
    use super::*;
    use qualirs::detectors::design::large_trait::LargeTraitDetector;

    static DETECTOR: LargeTraitDetector = LargeTraitDetector;

    #[test]
    fn detects_many_methods() {
        let methods: String = (0..16)
            .map(|i| format!("fn method_{i}(&self);"))
            .collect::<Vec<_>>()
            .join("\n    ");
        let code = format!("trait Huge {{ {methods} }}");
        assert_smell_count(&DETECTOR, &code, "Large Trait", 1);
    }

    #[test]
    fn clean_small_trait() {
        let code = "trait Small { fn do_it(&self); }";
        assert_clean(&DETECTOR, code);
    }
}

mod excessive_generics {
    use super::*;
    use qualirs::detectors::design::excessive_generics::ExcessiveGenericsDetector;

    static DETECTOR: ExcessiveGenericsDetector = ExcessiveGenericsDetector;

    #[test]
    fn detects_many_generics() {
        let code = "fn foo<A, B, C, D, E, F>(a: A, b: B, c: C, d: D, e: E, f: F) {}";
        assert_smell_count(&DETECTOR, code, "Excessive Generics", 1);
    }

    #[test]
    fn clean_few_generics() {
        let code = "fn foo<T>(a: T) where T: Clone {}";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn detects_deep_trait_bounds() {
        let code = "fn foo<T: Clone + Copy + Default + Debug + Send>(a: T) {}";
        assert_smell_count(&DETECTOR, code, "Deep Trait Bounds", 1);
    }
}

mod anemic_struct {
    use super::*;
    use qualirs::detectors::design::anemic_struct::AnemicStructDetector;

    static DETECTOR: AnemicStructDetector = AnemicStructDetector;

    #[test]
    fn detects_struct_without_impl() {
        let code = "struct Point { x: f64, y: f64 }";
        assert_smell_count(&DETECTOR, code, "Anemic Struct", 1);
    }

    #[test]
    fn clean_struct_with_impl() {
        let code = "\
struct Point { x: f64, y: f64 }
impl Point {
    fn distance(&self) -> f64 { self.x }
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_template_struct() {
        let code = r#"
#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    title: String,
    count: usize,
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

// ─── Implementation ────────────────────────────────────────

mod long_function {
    use super::*;
    use qualirs::detectors::implementation::long_function::LongFunctionDetector;

    static DETECTOR: LongFunctionDetector = LongFunctionDetector;

    #[test]
    fn detects_long_fn() {
        let body: String = (0..55)
            .map(|i| format!("let _ = {i};"))
            .collect::<Vec<_>>()
            .join("\n");
        let code = format!("fn long() {{\n{body}\n}}");
        assert_smell_count(&DETECTOR, &code, "Long Function", 1);
    }

    #[test]
    fn clean_short_fn() {
        let code = "fn short() { let x = 1; }";
        assert_clean(&DETECTOR, code);
    }
}

mod too_many_arguments {
    use super::*;
    use qualirs::detectors::implementation::too_many_arguments::TooManyArgumentsDetector;

    static DETECTOR: TooManyArgumentsDetector = TooManyArgumentsDetector;

    #[test]
    fn detects_many_args() {
        let code = "fn foo(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32) {}";
        assert_smell_count(&DETECTOR, code, "Too Many Arguments", 1);
    }

    #[test]
    fn clean_few_args() {
        let code = "fn foo(a: i32, b: i32) {}";
        assert_clean(&DETECTOR, code);
    }
}

mod large_value_passed_by_value {
    use super::*;
    use qualirs::detectors::implementation::large_value_passed_by_value::LargeValuePassedByValueDetector;

    static DETECTOR: LargeValuePassedByValueDetector = LargeValuePassedByValueDetector;

    #[test]
    fn detects_large_inline_array() {
        let code = "fn process(bytes: [u8; 64]) {}";
        assert_smell_count(&DETECTOR, code, "Large Value Passed By Value", 1);
    }

    #[test]
    fn clean_owned_heap_containers_are_cheap_to_move() {
        let code = "fn process(name: String, bytes: Vec<u8>, lookup: HashMap<String, String>) {}";
        assert_clean(&DETECTOR, code);
    }
}

mod excessive_unwrap {
    use super::*;
    use qualirs::detectors::implementation::excessive_unwrap::ExcessiveUnwrapDetector;

    static DETECTOR: ExcessiveUnwrapDetector = ExcessiveUnwrapDetector;

    #[test]
    fn detects_many_unwrap() {
        let code = "\
fn risky() {
    let a = Some(1).unwrap();
    let b = Some(2).unwrap();
    let c = Some(3).unwrap();
    let d = Some(4).unwrap();
}
";
        assert_smell_count(&DETECTOR, code, "Excessive Unwrap", 1);
    }

    #[test]
    fn clean_single_unwrap() {
        let code = "fn ok() { let x = Some(1).unwrap(); }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_regex_literal_expect_calls() {
        let code = r#"
use regex::Regex;
use std::sync::LazyLock;

fn redact(value: &str) -> String {
    static A: LazyLock<Regex> = LazyLock::new(|| Regex::new("a").expect("valid regex"));
    static B: LazyLock<Regex> = LazyLock::new(|| Regex::new("b").expect("valid regex"));
    static C: LazyLock<Regex> = LazyLock::new(|| Regex::new("c").expect("valid regex"));
    static D: LazyLock<Regex> = LazyLock::new(|| Regex::new("d").expect("valid regex"));
    A.replace_all(value, "x").to_string()
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_test_file() {
        let code = "\
fn risky() {
    let a = Some(1).unwrap();
    let b = Some(2).unwrap();
    let c = Some(3).unwrap();
    let d = Some(4).unwrap();
}
";
        let file =
            SourceFile::from_source(PathBuf::from("src/tests.rs"), code.to_string()).unwrap();
        assert!(DETECTOR.detect(&file).is_empty());
    }
}

mod deep_match {
    use super::*;
    use qualirs::detectors::implementation::deep_match::DeepMatchDetector;

    static DETECTOR: DeepMatchDetector = DeepMatchDetector;

    #[test]
    fn detects_nested_match() {
        let code = "\
fn deep() {
    match 1 {
        1 => match 2 {
            2 => match 3 {
                3 => match 4 {
                    4 => (),
                    _ => (),
                },
                _ => (),
            },
            _ => (),
        },
        _ => (),
    }
}
";
        assert_smell_count(&DETECTOR, code, "Deep Match Nesting", 1);
    }

    #[test]
    fn clean_flat_match() {
        let code = "\
fn flat(x: i32) {
    match x {
        1 => (),
        2 => (),
        _ => (),
    }
}
";
        assert_clean(&DETECTOR, code);
    }
}

mod excessive_clone {
    use super::*;
    use qualirs::detectors::implementation::excessive_clone::ExcessiveCloneDetector;

    static DETECTOR: ExcessiveCloneDetector = ExcessiveCloneDetector;

    #[test]
    fn detects_many_clones() {
        let clones: String = (0..12)
            .map(|i| format!("let v{i} = x.clone();"))
            .collect::<Vec<_>>()
            .join("\n    ");
        let code = format!("fn clone_happy(x: String) {{\n    {clones}\n}}");
        assert_smell_count(&DETECTOR, &code, "Excessive Clone", 1);
    }

    #[test]
    fn clean_few_clones() {
        let code = "fn ok(x: String) { let y = x.clone(); }";
        assert_clean(&DETECTOR, code);
    }
}

mod magic_numbers {
    use super::*;
    use qualirs::detectors::implementation::magic_numbers::MagicNumbersDetector;

    static DETECTOR: MagicNumbersDetector = MagicNumbersDetector;

    #[test]
    fn detects_magic_number() {
        let code = "fn calc() { let x = 42; let y = 1337; }";
        assert_smell_count(&DETECTOR, code, "Magic Numbers", 1);
    }

    #[test]
    fn clean_whitelisted_numbers() {
        let code = "fn calc() { let x = 0; let y = 1; let z = 100; }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_test_file() {
        let code = "fn calc() { let x = 42; let y = 1337; }";
        let file =
            SourceFile::from_source(PathBuf::from("tests/magic.rs"), code.to_string()).unwrap();
        assert!(DETECTOR.detect(&file).is_empty());
    }

    #[test]
    fn clean_default_value_provider() {
        let code = "fn default_retry_backoff_millis() -> u64 { 250 }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_database_column_indexes() {
        let code = r#"
fn map_project(row: &Row) -> Result<Project, Error> {
    Ok(Project {
        id: parse_uuid(get_string(row, 3)?, 3)?,
        created_at: parse_datetime(get_string(row, 4)?, 4)?,
        source: parse_project_source(get_string(row, 5)?, 5)?,
        claims: parse_claims_json(get_string(row, 6)?, 6)?,
        error: StoreError { column: offset + 7 },
    })
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_named_local_constants() {
        let code =
            r#"fn human_bytes() { const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"]; }"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_contextual_literals_from_protocol_and_display_code() {
        let code = r#"
fn looks_like_executable_binary(contents: &[u8]) -> bool {
    contents.starts_with(&[0xfe, 0xed, 0xfa, 0xce]) || contents.starts_with(&[0x1f, 0x8b])
}

fn severity_rank(severity: &str) -> u8 {
    match severity {
        "critical" => 5,
        "high" => 4,
        "medium" => 3,
        "low" => 2,
        _ => 0,
    }
}

fn severity_color(severity: u8) -> u32 {
    match severity {
        5 => 0xe74c3c,
        4 => 0xe67e22,
        _ => 0x7f8c8d,
    }
}

fn redact(captures: regex::Captures<'_>) {
    let _scheme_or_value = &captures[3];
}

fn inspect(bytes: &[u8]) {
    let mut header = [0_u8; 8];
    let _window = &bytes[..bytes.len().min(8)];
}

fn sql_server_config(url: Url, mut config: Config) {
    config.port(url.port().unwrap_or(1433));
}

fn limit(items: Vec<String>) {
    let _visible = items.into_iter().take(96).collect::<Vec<_>>();
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod large_enum {
    use super::*;
    use qualirs::detectors::implementation::large_enum::LargeEnumDetector;

    static DETECTOR: LargeEnumDetector = LargeEnumDetector;

    #[test]
    fn detects_many_variants() {
        let variants: String = (0..22)
            .map(|i| format!("V{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let code = format!("enum Huge {{ {variants} }}");
        assert_smell_count(&DETECTOR, &code, "Large Enum", 1);
    }

    #[test]
    fn clean_small_enum() {
        let code = "enum Color { Red, Green, Blue }";
        assert_clean(&DETECTOR, code);
    }
}

mod cyclomatic_complexity {
    use super::*;
    use qualirs::detectors::implementation::cyclomatic_complexity::CyclomaticComplexityDetector;

    static DETECTOR: CyclomaticComplexityDetector = CyclomaticComplexityDetector;

    #[test]
    fn detects_complex_fn() {
        // 16 if statements = CC 17 > 15
        let branches: String = (0..16)
            .map(|i| format!("if x > {i} {{"))
            .collect::<Vec<_>>()
            .join("\n");
        let closes = "}".repeat(16);
        let code = format!("fn complex(x: i32) {{\n{branches}\n(){closes}\n}}");
        assert_smell_count(&DETECTOR, &code, "High Cyclomatic Complexity", 1);
    }

    #[test]
    fn clean_simple_fn() {
        let code = "fn simple() { let x = 1; }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn counts_match_arms() {
        let arms: String = (0..20)
            .map(|i| format!("{i} => (),"))
            .collect::<Vec<_>>()
            .join(" ");
        let code = format!("fn matchy(x: i32) {{ match x {{ {arms} _ => () }} }}");
        assert_smell_count(&DETECTOR, &code, "High Cyclomatic Complexity", 1);
    }
}

mod deep_if_else {
    use super::*;
    use qualirs::detectors::implementation::deep_if_else::DeepIfElseDetector;

    static DETECTOR: DeepIfElseDetector = DeepIfElseDetector;

    #[test]
    fn detects_deep_nesting() {
        let code = "\
fn nested(x: i32) {
    if x > 0 {
        if x > 10 {
            if x > 20 {
                if x > 30 {
                    if x > 40 {
                        ()
                    }
                }
            }
        }
    }
}
";
        assert_smell_count(&DETECTOR, code, "Deep If/Else Nesting", 1);
    }

    #[test]
    fn clean_shallow_if() {
        let code = "fn shallow(x: i32) { if x > 0 { () } }";
        assert_clean(&DETECTOR, code);
    }
}

mod long_method_chain {
    use super::*;
    use qualirs::detectors::implementation::long_method_chain::LongMethodChainDetector;

    static DETECTOR: LongMethodChainDetector = LongMethodChainDetector;

    #[test]
    fn detects_long_chain() {
        // 5 chained calls: iter().filter().map().flatten().collect() — depth > 4
        // Must use a non-test file path because the detector skips test files
        let code = "fn chain(x: Vec<i32>) { x.iter().filter(|&&x| x > 0).map(|&x| x * 2).flatten().collect::<Vec<i32>>(); }";
        let file = SourceFile::from_source(PathBuf::from("main.rs"), code.to_string()).unwrap();
        let smells = DETECTOR.detect(&file);
        assert!(
            smells.iter().any(|s| s.name == "Long Method Chain"),
            "Should detect long chain: {:?}",
            smells
        );
    }

    #[test]
    fn clean_short_chain() {
        // Must use a non-test file path because the detector skips test files
        let code = "fn short(x: Vec<i32>) { x.iter().count(); }";
        let file = SourceFile::from_source(PathBuf::from("main.rs"), code.to_string()).unwrap();
        let smells = DETECTOR.detect(&file);
        assert!(
            smells.is_empty(),
            "Expected no smells, but found: {:?}",
            smells
        );
    }

    #[test]
    fn clean_async_result_iterator_pipeline() {
        let code = r#"
async fn tenants(state: State) -> Result<Vec<TenantView>, Error> {
    Ok(state
        .app
        .list_tenants()
        .await?
        .into_iter()
        .map(tenant_view)
        .collect())
}
"#;
        let file = SourceFile::from_source(PathBuf::from("main.rs"), code.to_string()).unwrap();
        assert!(
            DETECTOR.detect(&file).is_empty(),
            "async/try syntax should not inflate method-chain depth"
        );
    }

    #[test]
    fn clean_router_builder_chain() {
        let code = r#"
fn routes(state: AppState, layer: Layer) -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/admin", get(admin_index).post(admin_submit))
        .merge(package_routes())
        .method_not_allowed_fallback(method_not_allowed)
        .fallback(not_found)
        .layer(layer)
        .with_state(state)
}
"#;
        let file = SourceFile::from_source(PathBuf::from("main.rs"), code.to_string()).unwrap();
        assert!(
            DETECTOR.detect(&file).is_empty(),
            "fluent builder APIs should not be reported as long method chains"
        );
    }

    #[test]
    fn clean_option_result_cleanup_chain() {
        let code = r#"
fn relative_path(root: &Path, raw: &str) -> Option<String> {
    let path = Path::new(raw);
    path.strip_prefix(root)
        .ok()
        .or_else(|| path.strip_prefix(".").ok())
        .unwrap_or(path)
        .to_str()
        .map(|value| value.replace('\\', "/"))
        .filter(|value| !value.is_empty())
}
"#;
        let file = SourceFile::from_source(PathBuf::from("main.rs"), code.to_string()).unwrap();
        assert!(
            DETECTOR.detect(&file).is_empty(),
            "Option/Result cleanup chains should not be reported as long method chains"
        );
    }

    #[test]
    fn clean_string_normalization_chain() {
        let code = r#"
fn normalize(raw: &str) -> Option<IpAddr> {
    raw.trim()
        .trim_matches('"')
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim()
        .parse()
        .ok()
}
"#;
        let file = SourceFile::from_source(PathBuf::from("main.rs"), code.to_string()).unwrap();
        assert!(
            DETECTOR.detect(&file).is_empty(),
            "cleanup chains should not be reported as long method chains"
        );
    }
}

mod unused_result {
    use super::*;
    use qualirs::detectors::implementation::unused_result::UnusedResultDetector;

    static DETECTOR: UnusedResultDetector = UnusedResultDetector;

    #[test]
    fn detects_let_underscore() {
        let code = "fn discard() { let _ = std::fs::read_to_string(\"x\"); }";
        assert_smell_count(&DETECTOR, code, "Unused Result Ignored", 1);
    }

    #[test]
    fn clean_used_result() {
        let code = "fn ok() { let _x = std::fs::read_to_string(\"x\"); }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_test_file() {
        let code = "fn discard() { let _ = std::fs::remove_file(\"x\"); }";
        let file =
            SourceFile::from_source(PathBuf::from("src/settings/tests.rs"), code.to_string())
                .unwrap();
        assert!(DETECTOR.detect(&file).is_empty());
    }

    #[test]
    fn clean_string_buffer_write_result() {
        let code = r#"
fn render() -> String {
    let mut out = String::new();
    let _ = writeln!(out, "hello");
    out
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_string_param_write_result() {
        let code = r#"
fn render(out: &mut String) {
    let _ = write!(out, "hello");
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn detects_unknown_writer_discard() {
        let code = r#"
struct Writer;
fn render(writer: Writer) {
    let _ = writeln!(writer, "hello");
}
"#;
        assert_smell_count(&DETECTOR, code, "Unused Result Ignored", 1);
    }

    #[test]
    fn clean_channel_sender_discard() {
        let code = "fn notify(sender: Sender) { let _ = sender.send(1); }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_non_result_like_wildcard_assignments() {
        let code = r#"
fn ignore_values(value: String, iter: Vec<i32>, regex: Regex, guard: Guard) {
    let _ = value.clone();
    let _ = iter.iter().map(|item| item + 1);
    let _ = regex.is_match("abc");
    let _ = guard;
    let _ = 1 + 2;
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod repeated_regex_construction {
    use super::*;
    use qualirs::detectors::implementation::repeated_regex_construction::RepeatedRegexConstructionDetector;

    static DETECTOR: RepeatedRegexConstructionDetector = RepeatedRegexConstructionDetector;

    #[test]
    fn detects_runtime_regex_construction() {
        let code = r#"
use regex::Regex;

fn validate(value: &str) -> bool {
    Regex::new(r"^[a-z]+$").unwrap().is_match(value)
}
"#;
        assert_smell_count(&DETECTOR, code, "Repeated Regex Construction", 1);
    }

    #[test]
    fn clean_lazy_lock_regex_initializer() {
        let code = r#"
use regex::Regex;
use std::sync::LazyLock;

fn validate(value: &str) -> bool {
    static VALID: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^[a-z]+$").expect("valid regex")
    });
    VALID.is_match(value)
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_once_lock_get_or_init_regex_initializer() {
        let code = r#"
use regex::Regex;
use std::sync::OnceLock;

fn validate(value: &str) -> bool {
    static VALID: OnceLock<Regex> = OnceLock::new();
    VALID
        .get_or_init(|| Regex::new(r"^[a-z]+$").expect("valid regex"))
        .is_match(value)
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod inline_candidate {
    use super::*;
    use qualirs::detectors::implementation::inline_candidate::InlineCandidateDetector;

    static DETECTOR: InlineCandidateDetector = InlineCandidateDetector;

    #[test]
    fn detects_tiny_repeated_function() {
        let code = r#"
fn score(value: u32) -> u32 {
    value + 1
}

fn total() -> u32 {
    score(1) + score(2) + score(3)
}
"#;
        assert_smell_count(&DETECTOR, code, "Inline Candidate", 1);
    }

    #[test]
    fn detects_tiny_repeated_method() {
        let code = r#"
struct Price(u32);

impl Price {
    fn cents(&self) -> u32 {
        self.0
    }
}

fn total(a: Price, b: Price, c: Price) -> u32 {
    a.cents() + b.cents() + c.cents()
}
"#;
        assert_smell_count(&DETECTOR, code, "Inline Candidate", 1);
    }

    #[test]
    fn detects_tiny_repeated_associated_function_for_same_type() {
        let code = r#"
struct Token(u32);

impl Token {
    fn generated() -> Self {
        Self(1)
    }
}

fn build() -> (Token, Token, Token) {
    (Token::generated(), Token::generated(), Token::generated())
}
"#;
        assert_smell_count(&DETECTOR, code, "Inline Candidate", 1);
    }

    #[test]
    fn clean_unrelated_new_constructors() {
        let code = r#"
struct Local;

impl Local {
    fn new() -> Self {
        Self
    }
}

struct Other;

impl Other {
    fn new() -> Self {
        Self
    }
}

fn build() {
    let _local = Local::new();
    let _other = Other::new();
    let _items = Vec::<u8>::new();
    let _text = String::new();
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_repeated_new_constructor() {
        let code = r#"
struct Status;

impl Status {
    fn new() -> Self {
        Self
    }
}

fn build() -> (Status, Status, Status, Status) {
    (Status::new(), Status::new(), Status::new(), Status::new())
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_ambiguous_same_named_methods() {
        let code = r#"
struct EmailConfig;

impl EmailConfig {
    fn validate(&self) -> bool {
        true
    }
}

struct WebConfig;

impl WebConfig {
    fn validate(&self) -> bool {
        true
    }
}

fn validate_all(email: EmailConfig, web: WebConfig) -> bool {
    email.validate() && web.validate() && email.validate() && web.validate()
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_existing_inline_hint() {
        let code = r#"
#[inline]
fn score(value: u32) -> u32 {
    value + 1
}

fn total() -> u32 {
    score(1) + score(2) + score(3)
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_low_call_count() {
        let code = r#"
fn score(value: u32) -> u32 {
    value + 1
}

fn total() -> u32 {
    score(1)
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_complex_small_function() {
        let code = r#"
fn score(value: u32) -> u32 {
    if value > 10 { value } else { 10 }
}

fn total() -> u32 {
    score(1) + score(2) + score(3)
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_cfg_test_helpers() {
        let code = r#"
#[cfg(test)]
mod tests {
    fn fixture(value: u32) -> u32 {
        value + 1
    }

    fn smoke() -> u32 {
        fixture(1) + fixture(2) + fixture(3)
    }
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod manual_option_result_mapping {
    use super::*;
    use qualirs::detectors::implementation::manual_option_result_mapping::ManualOptionResultMappingDetector;

    static DETECTOR: ManualOptionResultMappingDetector = ManualOptionResultMappingDetector;

    #[test]
    fn detects_simple_option_map() {
        let code = r#"
fn map_value(value: Option<i32>) -> Option<i32> {
    match value {
        Some(value) => Some(value + 1),
        None => None,
    }
}
"#;
        assert_smell_count(&DETECTOR, code, "Manual Option/Result Mapping", 1);
    }

    #[test]
    fn detects_simple_result_map() {
        let code = r#"
fn map_value(value: Result<i32, Error>) -> Result<i32, Error> {
    match value {
        Ok(value) => Ok(value + 1),
        Err(error) => Err(error),
    }
}
"#;
        assert_smell_count(&DETECTOR, code, "Manual Option/Result Mapping", 1);
    }

    #[test]
    fn clean_destructuring_match_that_builds_many_options() {
        let code = r#"
fn columns(identity: Option<Identity>) -> (Option<String>, Option<String>) {
    match identity {
        Some(identity) => (Some(identity.issuer), Some(identity.subject)),
        None => (None, None),
    }
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_result_match_with_nested_result_handling() {
        let code = r#"
fn scanner(result: Result<Compiled, Error>) -> Scanner {
    match result {
        Ok(compiled) => Scanner::loaded(compiled),
        Err(error) => match fallback() {
            Ok(compiled) => Scanner::loaded(compiled),
            Err(fallback_error) => Scanner::unavailable(error, fallback_error),
        },
    }
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod panic_in_library {
    use super::*;
    use qualirs::detectors::implementation::panic_in_library::PanicInLibraryDetector;

    static DETECTOR: PanicInLibraryDetector = PanicInLibraryDetector;

    #[test]
    fn detects_panic() {
        // File name must NOT contain "test" — detector skips test files
        let code = "fn crash() { panic!(\"oops\"); }";
        let file = SourceFile::from_source(PathBuf::from("main.rs"), code.to_string()).unwrap();
        let smells = DETECTOR.detect(&file);
        assert!(
            smells.iter().any(|s| s.name == "Panic in Library"),
            "Should detect panic!: {:?}",
            smells
        );
    }

    #[test]
    fn detects_todo() {
        let code = "fn unfinished() { todo!(); }";
        let file = SourceFile::from_source(PathBuf::from("main.rs"), code.to_string()).unwrap();
        let smells = DETECTOR.detect(&file);
        assert!(
            smells.iter().any(|s| s.name == "Panic in Library"),
            "Should detect todo!: {:?}",
            smells
        );
    }

    #[test]
    fn clean_result_return() {
        let code = "fn ok() -> Result<(), std::io::Error> { Ok(()) }";
        assert_clean(&DETECTOR, code);
    }
}

mod unsafe_overuse {
    use super::*;
    use qualirs::detectors::implementation::unsafe_overuse::UnsafeOveruseDetector;

    static DETECTOR: UnsafeOveruseDetector = UnsafeOveruseDetector;

    #[test]
    fn detects_many_unsafe_blocks() {
        let blocks: String = (0..6)
            .map(|_| String::from("let _ = unsafe { 1 };"))
            .collect::<Vec<_>>()
            .join("\n");
        let code = format!("fn risky() {{\n{blocks}\n}}");
        assert_smell_count(&DETECTOR, &code, "Unsafe Block Overuse", 1);
    }

    #[test]
    fn clean_single_unsafe() {
        let code = "fn ok() { let x = unsafe { 1 }; }";
        assert_clean(&DETECTOR, code);
    }
}

mod lifetime_explosion {
    use super::*;
    use qualirs::detectors::implementation::lifetime_explosion::LifetimeExplosionDetector;

    static DETECTOR: LifetimeExplosionDetector = LifetimeExplosionDetector;

    #[test]
    fn detects_many_lifetimes() {
        let code = "struct S<'a, 'b, 'c, 'd, 'e> { a: &'a i32, b: &'b i32, c: &'c i32, d: &'d i32, e: &'e i32 }";
        assert_smell_count(&DETECTOR, code, "Lifetime Explosion", 1);
    }

    #[test]
    fn clean_few_lifetimes() {
        let code = "struct S<'a> { x: &'a i32 }";
        assert_clean(&DETECTOR, code);
    }
}

mod copy_drop_conflict {
    use super::*;
    use qualirs::detectors::implementation::copy_drop_conflict::CopyDropConflictDetector;

    static DETECTOR: CopyDropConflictDetector = CopyDropConflictDetector;

    #[test]
    fn detects_copy_and_drop() {
        let code = "\
#[derive(Copy, Clone)]
struct Bad { x: i32 }
impl Drop for Bad {
    fn drop(&mut self) {}
}
";
        assert_smell_count(&DETECTOR, code, "Copy + Drop Conflict", 1);
    }

    #[test]
    fn clean_copy_only() {
        let code = "#[derive(Copy, Clone)] struct Good { x: i32 }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_drop_only() {
        let code = "\
struct Good { x: Box<i32> }
impl Drop for Good {
    fn drop(&mut self) {}
}
";
        assert_clean(&DETECTOR, code);
    }
}

// ─── Unsafe ────────────────────────────────────────────────

mod unsafe_without_comment {
    use super::*;
    use qualirs::detectors::r#unsafe::unsafe_without_comment::UnsafeWithoutCommentDetector;

    static DETECTOR: UnsafeWithoutCommentDetector = UnsafeWithoutCommentDetector;

    #[test]
    fn detects_unsafe_without_comment() {
        let code = "fn risky() { unsafe { let x = 1; } }";
        assert_smell_count(&DETECTOR, code, "Unsafe Without Comment", 1);
    }

    #[test]
    fn clean_unsafe_with_safety_comment() {
        let code = "\
fn ok() {
    // SAFETY: this is safe because reasons
    unsafe { let x = 1; }
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn unsafe_fn_and_impl_use_stable_rule_code() {
        let code = r#"
pub unsafe fn from_raw(ptr: *const i32) -> i32 { unsafe { *ptr } }
unsafe impl Send for Marker {}
struct Marker;
"#;
        let smells = common::detect(&DETECTOR, code);

        assert_eq!(smells.len(), 3, "smells: {smells:?}");
        assert!(
            smells
                .iter()
                .all(|smell| smell.name == "Unsafe Without Comment")
        );
        assert!(smells.iter().all(|smell| smell.code == "Q0087"));
    }
}

// ─── Concurrency ───────────────────────────────────────────

mod blocking_in_async {
    use super::*;
    use qualirs::detectors::concurrency::blocking_in_async::BlockingInAsyncDetector;

    static DETECTOR: BlockingInAsyncDetector = BlockingInAsyncDetector;

    #[test]
    fn detects_std_sleep_in_async() {
        let code = "async fn foo() { std::thread::sleep(std::time::Duration::from_secs(1)); }";
        assert_smell_count(&DETECTOR, code, "Blocking in Async", 1);
    }

    #[test]
    fn detects_fs_read_in_async() {
        let code = "async fn foo() { let _ = std::fs::read_to_string(\"foo.txt\"); }";
        assert_smell_count(&DETECTOR, code, "Blocking in Async", 1);
    }

    #[test]
    fn clean_no_blocking_in_async() {
        let code =
            "async fn foo() { tokio::time::sleep(std::time::Duration::from_secs(1)).await; }";
        assert_clean(&DETECTOR, code);
    }
}

mod deadlock_risk {
    use super::*;
    use qualirs::detectors::concurrency::deadlock_risk::DeadlockRiskDetector;

    static DETECTOR: DeadlockRiskDetector = DeadlockRiskDetector;

    #[test]
    fn detects_multiple_locks() {
        let code = "\
fn foo(m1: &Mutex<i32>, m2: &Mutex<i32>) {
    let _g1 = m1.lock().unwrap();
    let _g2 = m2.lock().unwrap();
}
";
        assert_smell_count(&DETECTOR, code, "Deadlock Risk", 1);
    }

    #[test]
    fn clean_single_lock() {
        let code = "\
fn foo(m1: &Mutex<i32>) {
    let _g1 = m1.lock().unwrap();
}
";
        assert_clean(&DETECTOR, code);
    }
}

mod arc_mutex_overuse {
    use super::*;
    use qualirs::detectors::concurrency::arc_mutex_overuse::ArcMutexOveruseDetector;

    static DETECTOR: ArcMutexOveruseDetector = ArcMutexOveruseDetector;

    #[test]
    fn detects_many_arc_mutex() {
        // Threshold is 3 by default.
        let code = "\
struct State {
    a: Arc<Mutex<i32>>,
    b: Arc<Mutex<i32>>,
    c: Arc<RwLock<i32>>,
    d: Arc<tokio::sync::Mutex<i32>>,
}
";
        assert_smell_count(&DETECTOR, code, "Arc Mutex Overuse", 1);
    }

    #[test]
    fn clean_few_usages() {
        let code = "\
struct State {
    data: Arc<Mutex<i32>>,
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_no_arc_mutex() {
        let code = "struct State { value: i32 }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_many_arc_trait_dependencies() {
        let code = "\
struct Services {
    store: Arc<dyn Store>,
    cache: Arc<dyn Cache>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdGenerator>,
    mailer: Arc<dyn Mailer>,
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_standalone_locks_without_arc() {
        let code = "\
struct State {
    operation_lock: Mutex<()>,
    buckets: RwLock<HashMap<String, Bucket>>,
}
";
        assert_clean(&DETECTOR, code);
    }
}

mod large_future {
    use super::*;
    use qualirs::detectors::concurrency::large_future::LargeFutureDetector;

    static DETECTOR: LargeFutureDetector = LargeFutureDetector;

    #[test]
    fn detects_long_async_fn() {
        // Threshold is 100 lines by default
        let body: String = (0..120)
            .map(|i| format!("let _ = {i};"))
            .collect::<Vec<_>>()
            .join("\n");
        let code = format!("async fn big() {{\n{body}\n}}");
        assert_smell_count(&DETECTOR, &code, "Large Future", 1);
    }

    #[test]
    fn clean_short_async() {
        let code = "async fn short() { let x = 1; }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_long_sync_function() {
        // Long non-async functions are not flagged by LargeFutureDetector
        let body: String = (0..120)
            .map(|i| format!("let _ = {i};"))
            .collect::<Vec<_>>()
            .join("\n");
        let code = format!("fn big_sync() {{\n{body}\n}}");
        assert_clean(&DETECTOR, &code);
    }
}

// ─── Missing Design ──────────────────────────────────────────

mod manual_drop {
    use super::*;
    use qualirs::detectors::design::manual_drop::ManualDropDetector;

    static DETECTOR: ManualDropDetector = ManualDropDetector;

    #[test]
    fn detects_manual_drop() {
        let code = "\
struct Resource;
impl Drop for Resource {
    fn drop(&mut self) {}
}
";
        assert_smell_count(&DETECTOR, code, "Manual Drop", 1);
    }

    #[test]
    fn clean_no_drop_impl() {
        let code = "struct Resource;";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_trait_impl_not_drop() {
        let code = "\
struct Resource;
impl std::fmt::Debug for Resource {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { Ok(()) }
}
";
        assert_clean(&DETECTOR, code);
    }
}

mod deref_abuse {
    use super::*;
    use qualirs::detectors::design::deref_abuse::DerefAbuseDetector;

    static DETECTOR: DerefAbuseDetector = DerefAbuseDetector;

    #[test]
    fn detects_deref_on_non_pointer() {
        let code = "\
struct MyStruct { inner: String }
impl std::ops::Deref for MyStruct {
    type Target = String;
    fn deref(&self) -> &Self::Target { &self.inner }
}
";
        assert_smell_count(&DETECTOR, code, "Deref Abuse", 1);
    }

    #[test]
    fn clean_deref_on_pointer_named_type() {
        let code = "\
struct MyPtr<T> { inner: Box<T> }
impl<T> std::ops::Deref for MyPtr<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target { &self.inner }
}
";
        assert_clean(&DETECTOR, code);
    }
}

// ─── Missing Unsafe ──────────────────────────────────────────

mod transmute_usage {
    use super::*;
    use qualirs::detectors::r#unsafe::transmute_usage::TransmuteUsageDetector;

    static DETECTOR: TransmuteUsageDetector = TransmuteUsageDetector;

    #[test]
    fn detects_transmute() {
        let code = "fn risky() { let x: i32 = unsafe { std::mem::transmute(1.0f32) }; }";
        assert_smell_count(&DETECTOR, code, "Transmute Usage", 1);
    }

    #[test]
    fn clean_no_transmute() {
        let code = "fn safe() { let x = 1 as f32; }";
        assert_clean(&DETECTOR, code);
    }
}

// ─── Extended Architecture ──────────────────────────────

mod feature_concentration {
    use super::*;
    use qualirs::detectors::architecture::feature_concentration::FeatureConcentrationDetector;
    static DETECTOR: FeatureConcentrationDetector = FeatureConcentrationDetector;

    #[test]
    fn detects_high_concentration() {
        let mut code = String::new();
        for i in 0..16 {
            code.push_str(&format!("use crate{}::item;\n", i));
        }
        assert_smell_count(&DETECTOR, &code, "Feature Concentration", 1);
    }

    #[test]
    fn clean_few_external_crates() {
        let code = "\
use serde::Deserialize;
use tokio::runtime::Runtime;
use clap::Parser;
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_only_local_imports() {
        let code = "\
use crate::domain::User;
use super::config;
use self::inner::Helper;
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_repeated_crate_counts_once() {
        // Same crate imported via different paths counts only once
        let code = "\
use serde::Deserialize;
use serde::Serialize;
use serde::de::Deserializer;
";
        assert_clean(&DETECTOR, code);
    }
}

mod layer_violation {
    use super::*;
    use qualirs::detectors::architecture::layer_violation::LayerViolationDetector;
    static DETECTOR: LayerViolationDetector = LayerViolationDetector;

    #[test]
    fn detects_domain_to_infra_violation() {
        let code = "use crate::infrastructure::db::UserRepo;";
        let source =
            SourceFile::from_source("src/domain/user.rs".into(), code.to_string()).unwrap();
        let smells = DETECTOR.detect(&source);
        assert!(!smells.is_empty(), "Expected layer violation smell");
        assert_eq!(smells[0].name, "Layer Violation");
    }

    #[test]
    fn clean_layering() {
        let code = "use crate::domain::model::User;";
        let source =
            SourceFile::from_source("src/infrastructure/db.rs".into(), code.to_string()).unwrap();
        let smells = DETECTOR.detect(&source);
        assert!(smells.is_empty());
    }

    #[test]
    fn clean_io_substring_domain() {
        // 'action' contains 'io', but it's not the 'io' module
        let code = "use crate::domain::action::UserAction;";
        let source =
            SourceFile::from_source("src/domain/user.rs".into(), code.to_string()).unwrap();
        let smells = DETECTOR.detect(&source);
        assert!(
            smells.is_empty(),
            "Should not flag 'action' as 'io' violation. Smells found: {:?}",
            smells
        );
    }

    #[test]
    fn clean_option_domain() {
        // 'option' contains 'io', but it's std::option
        let code = "use std::option::Option;";
        let source =
            SourceFile::from_source("src/domain/user.rs".into(), code.to_string()).unwrap();
        let smells = DETECTOR.detect(&source);
        assert!(
            smells.is_empty(),
            "Should not flag 'option' as 'io' violation. Smells found: {:?}",
            smells
        );
    }

    #[test]
    fn clean_client_domain() {
        // 'client' contains 'cli', but it's not the 'cli' module
        // 'HttpClient' contains 'http', but it's not the 'http' module
        let code = "use crate::app::client::HttpClient;";
        let source =
            SourceFile::from_source("src/domain/user.rs".into(), code.to_string()).unwrap();
        let smells = DETECTOR.detect(&source);
        assert!(
            smells.is_empty(),
            "Should not flag 'client' as 'cli' or 'HttpClient' as 'http'. Smells found: {:?}",
            smells
        );
    }
}

// ─── Extended Design ────────────────────────────────────

mod trait_impl_leakage {
    use super::*;
    use qualirs::detectors::design::trait_impl_leakage::TraitImplLeakageDetector;
    static DETECTOR: TraitImplLeakageDetector = TraitImplLeakageDetector;

    #[test]
    fn detects_std_trait_overload() {
        let code = "\
struct Data;
impl Debug for Data {}
impl Clone for Data {}
impl Copy for Data {}
impl Hash for Data {}
impl Default for Data {}
";
        assert_smell_count(&DETECTOR, code, "Trait Impl Leakage", 1);
    }

    #[test]
    fn clean_with_domain_trait() {
        // Even with 5+ std traits, having one domain trait makes it safe
        let code = "\
struct Data;
impl Debug for Data {}
impl Clone for Data {}
impl Copy for Data {}
impl Hash for Data {}
impl Default for Data {}
impl crate::auth::Authorize for Data {}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_few_std_traits() {
        let code = "\
struct Data;
impl Debug for Data {}
impl Clone for Data {}
";
        assert_clean(&DETECTOR, code);
    }
}

mod feature_envy {
    use super::*;
    use qualirs::detectors::design::feature_envy::FeatureEnvyDetector;
    static DETECTOR: FeatureEnvyDetector = FeatureEnvyDetector;

    #[test]
    fn detects_envy() {
        let code = "\
pub fn process(other: &Other) {
    other.do_a();
    other.do_b();
    other.do_c();
    other.do_d();
    other.do_e();
    other.do_f();
}
";
        assert_smell_count(&DETECTOR, code, "Feature Envy", 1);
    }

    #[test]
    fn clean_private_function() {
        // Private functions are never flagged, even with many calls
        let code = "\
fn process(other: &Other) {
    other.do_a();
    other.do_b();
    other.do_c();
    other.do_d();
    other.do_e();
    other.do_f();
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_within_threshold() {
        let code = "\
pub fn process(other: &Other) {
    other.do_a();
    other.do_b();
    other.do_c();
}
";
        assert_clean(&DETECTOR, code);
    }
}

mod wide_hierarchy {
    use super::*;
    use qualirs::detectors::design::wide_hierarchy::WideHierarchyDetector;
    static DETECTOR: WideHierarchyDetector = WideHierarchyDetector;

    #[test]
    fn detects_wide_enum() {
        let code = "\
enum Huge {
    V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11
}
";
        assert_smell_count(&DETECTOR, code, "Wide Hierarchy", 1);
    }

    #[test]
    fn clean_small_enum() {
        let code = "enum Color { Red, Green, Blue }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_enum_at_threshold() {
        // Exactly 10 variants does NOT trigger (>10 required)
        let code = "enum Status { V1, V2, V3, V4, V5, V6, V7, V8, V9, V10 }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_struct_few_fields() {
        let code = "struct Point { x: f64, y: f64, z: f64 }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_tuple_struct_many_fields() {
        // Tuple structs are not checked regardless of field count
        let code = "pub struct Wrapper(pub i32, pub i32, pub i32, pub i32, pub i32, pub i32, pub i32, pub i32, pub i32, pub i32, pub i32);";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_threshold_config_struct() {
        let code = "\
struct DesignThresholds {
    a: usize, b: usize, c: usize, d: usize, e: usize, f: usize,
    g: usize, h: usize, i: usize, j: usize, k: usize,
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_config_and_template_structs() {
        let code = "\
struct Settings { a:i32,b:i32,c:i32,d:i32,e:i32,f:i32,g:i32,h:i32,i:i32,j:i32,k:i32,l:i32 }
struct DashboardTemplate { a:i32,b:i32,c:i32,d:i32,e:i32,f:i32,g:i32,h:i32,i:i32,j:i32,k:i32,l:i32 }
";
        assert_clean(&DETECTOR, code);
    }
}

mod broken_constructor {
    use super::*;
    use qualirs::detectors::design::broken_constructor::BrokenConstructorDetector;
    static DETECTOR: BrokenConstructorDetector = BrokenConstructorDetector;

    #[test]
    fn detects_missing_new() {
        let code = "\
pub struct State {
    pub a: i32,
    pub b: i32,
    pub c: i32,
}
";
        assert_smell_count(&DETECTOR, code, "Broken Constructor", 1);
    }

    #[test]
    fn clean_with_new_constructor() {
        let code = "\
pub struct State {
    pub a: i32,
    pub b: i32,
    pub c: i32,
}
impl State {
    pub fn new(a: i32, b: i32, c: i32) -> Self { Self { a, b, c } }
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_with_default_derive() {
        let code = "\
#[derive(Default)]
pub struct State {
    pub a: i32,
    pub b: i32,
    pub c: i32,
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_private_fields() {
        let code = "\
pub struct State {
    a: i32,
    pub b: i32,
    pub c: i32,
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_few_fields() {
        let code = "\
pub struct Pair {
    pub a: i32,
    pub b: i32,
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_phantom_data_seed_struct() {
        let code = "\
use std::marker::PhantomData;

pub struct AdjacentlyTaggedEnumVariantSeed<F> {
    pub enum_name: &'static str,
    pub variants: &'static [&'static str],
    pub fields_enum: PhantomData<F>,
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_variant_data_carrier() {
        let code = "\
pub struct AdjacentlyTaggedEnumVariant {
    pub enum_name: &'static str,
    pub variant_index: u32,
    pub variant_name: &'static str,
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_dto_template_and_config_structs() {
        let code = r#"
pub struct CreateUserCommand {
    pub name: String,
    pub email: String,
    pub age: i32,
}

#[derive(Template)]
#[template(path = "user.html")]
pub struct UserTemplate {
    pub name: String,
    pub email: String,
    pub age: i32,
}

pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub tls: bool,
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

// ─── Extended Concurrency ───────────────────────────────

mod spawn_without_join {
    use super::*;
    use qualirs::detectors::concurrency::spawn_without_join::SpawnWithoutJoinDetector;
    static DETECTOR: SpawnWithoutJoinDetector = SpawnWithoutJoinDetector;

    #[test]
    fn detects_orphaned_spawn() {
        let code = "fn foo() { std::thread::spawn(|| {}); }";
        assert_smell_count(&DETECTOR, code, "Spawn Without Join", 1);
    }

    #[test]
    fn detects_underscore_spawn() {
        let code = "fn foo() { let _ = tokio::spawn(async {}); }";
        assert_smell_count(&DETECTOR, code, "Spawn Without Join", 1);
    }

    #[test]
    fn clean_no_spawn() {
        let code = "fn foo() { let x = 1 + 2; }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_assigned_spawn() {
        // Spawn assigned to a named variable is safe
        let code = "fn foo() { let handle = std::thread::spawn(|| {}); handle.join().unwrap(); }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_regular_function_with_spawn_in_name() {
        let code = "fn foo() { spawn_detached_mirror_refresh_job(); }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_spawn_handle_returned_from_function() {
        let code = "fn foo() -> Option<JoinHandle> { Some(tokio::spawn(async {})) }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_rayon_spawn_without_join_handle() {
        let code = "fn foo() { rayon::spawn(|| {}); }";
        assert_clean(&DETECTOR, code);
    }
}

mod holding_lock_across_await {
    use super::*;
    use qualirs::detectors::concurrency::holding_lock_across_await::HoldingLockAcrossAwaitDetector;

    static DETECTOR: HoldingLockAcrossAwaitDetector = HoldingLockAcrossAwaitDetector;

    #[test]
    fn detects_bound_guard_held_across_later_await() {
        let code = r#"
async fn foo(lock: &tokio::sync::Mutex<i32>) {
    let guard = lock.lock().await;
    do_work().await;
    drop(guard);
}
"#;
        assert_smell_count(&DETECTOR, code, "Holding Lock Across Await", 1);
    }

    #[test]
    fn clean_lock_temporary_used_in_single_statement_before_await() {
        let code = r#"
async fn logout(state: State) {
    let removed = state.sessions.write().await.remove("session-id");
    if removed.is_some() {
        record_audit_event().await;
    }
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_bound_guard_without_later_await() {
        let code = r#"
async fn require_session(state: State) {
    let mut sessions = state.sessions.write().await;
    sessions.remove("session-id");
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_explicit_drop_before_await() {
        let code = r#"
async fn foo(lock: &tokio::sync::Mutex<i32>) {
    let guard = lock.lock().await;
    drop(guard);
    do_work().await;
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod blocking_channel_in_async {
    use super::*;
    use qualirs::detectors::concurrency::blocking_channel_in_async::BlockingChannelInAsyncDetector;

    static DETECTOR: BlockingChannelInAsyncDetector = BlockingChannelInAsyncDetector;

    #[test]
    fn detects_blocking_recv_in_async() {
        let code = r#"
async fn wait(receiver: std::sync::mpsc::Receiver<i32>) {
    let _ = receiver.recv();
}
"#;
        assert_smell_count(&DETECTOR, code, "Blocking Channel in Async", 1);
    }

    #[test]
    fn clean_awaited_async_recv() {
        let code = r#"
async fn wait(mut signal: tokio::sync::watch::Receiver<bool>) {
    signal.recv().await;
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_send_inside_non_async_worker_closure() {
        let code = r#"
async fn inspect() {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    rayon::spawn(move || {
        let _ = sender.send(1);
    });
    let _ = receiver.await;
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_nonblocking_send_in_async() {
        let code = r#"
async fn notify(sender: tokio::sync::watch::Sender<bool>) {
    sender.send(true).expect("send shutdown");
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod missing_send_bound {
    use super::*;
    use qualirs::detectors::concurrency::missing_send_bound::MissingSendBoundDetector;
    static DETECTOR: MissingSendBoundDetector = MissingSendBoundDetector;

    #[test]
    fn detects_missing_send() {
        let code = "fn run<T>(data: T) { spawn(move || { let _ = data; }); }";
        let smells = common::detect(&DETECTOR, code);

        assert_eq!(smells.len(), 1, "smells: {smells:?}");
        assert_eq!(smells[0].name, "Missing Send Bound");
        assert_eq!(
            smells[0].confidence,
            qualirs::domain::smell::FindingConfidence::Medium
        );
    }

    #[test]
    fn clean_with_send_bound() {
        let code = "fn run<T: Send>(data: T) { spawn(move || { let _ = data; }); }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_non_generic_fn() {
        // Non-generic functions with spawn are not flagged
        let code = "fn run() { spawn(|| {}); }";
        assert_clean(&DETECTOR, code);
    }
}

// ─── Extended Unsafe ────────────────────────────────────

mod raw_pointer_arithmetic {
    use super::*;
    use qualirs::detectors::r#unsafe::raw_pointer_arithmetic::RawPointerArithmeticDetector;
    static DETECTOR: RawPointerArithmeticDetector = RawPointerArithmeticDetector;

    #[test]
    fn detects_pointer_arith() {
        let code = "fn foo(raw_ptr: *const i32) { unsafe { let _ = raw_ptr.offset(1); } }";
        assert_smell_count(&DETECTOR, code, "Raw Pointer Arithmetic", 1);
    }

    #[test]
    fn clean_non_pointer_method() {
        // .add() on a Vec receiver does not trigger — receiver must contain "ptr" or "raw"
        let code = "fn foo(v: Vec<i32>) { let _ = v.len(); }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_no_pointer_methods() {
        let code = "fn foo(p: *const i32) { let _ = p; }";
        assert_clean(&DETECTOR, code);
    }
}

mod ffi_without_wrapper {
    use super::*;
    use qualirs::detectors::r#unsafe::ffi_without_wrapper::FfiWithoutWrapperDetector;
    static DETECTOR: FfiWithoutWrapperDetector = FfiWithoutWrapperDetector;

    #[test]
    fn detects_naked_ffi() {
        let code = "extern \"C\" { fn unsafe_c_api(); }";
        assert_smell_count(&DETECTOR, code, "FFI Without Wrapper", 1);
    }

    #[test]
    fn clean_with_wrapper() {
        let code = "\
extern \"C\" { fn some_api(); }
pub fn some_api_wrapper() { unsafe { some_api(); } }
";
        assert_clean(&DETECTOR, code);
    }
}

mod ffi_type_not_repr_c {
    use super::*;
    use qualirs::detectors::r#unsafe::ffi_type_not_repr_c::FfiTypeNotReprCDetector;
    static DETECTOR: FfiTypeNotReprCDetector = FfiTypeNotReprCDetector;

    #[test]
    fn detects_ffi_type_without_repr_c() {
        let code = r#"
extern "C" { fn use_config(config: *const CConfig); }
pub struct CConfig {
    value: i32,
}
"#;
        let smells = common::detect(&DETECTOR, code);

        assert_eq!(smells.len(), 1, "smells: {smells:?}");
        assert_eq!(smells[0].name, "FFI Type Not repr(C)");
        assert_eq!(
            smells[0].confidence,
            qualirs::domain::smell::FindingConfidence::Medium
        );
    }

    #[test]
    fn clean_repr_c_ffi_type() {
        let code = r#"
extern "C" { fn use_config(config: *const CConfig); }
#[repr(C)]
pub struct CConfig {
    value: i32,
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_public_c_named_rust_types_outside_ffi_context() {
        let code = r#"
pub struct Config;
pub struct CategorySmells;
pub struct CloneOnCopyDetector;
pub struct ConcurrencyThresholds;
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_detector_names_outside_ffi_context() {
        let code = r#"
pub struct FfiTypeNotReprCDetector;
pub struct FfiWithoutWrapperDetector;
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod duplicate_match_arms {
    use super::*;
    use qualirs::detectors::implementation::duplicate_match_arms::DuplicateMatchArmsDetector;
    static DETECTOR: DuplicateMatchArmsDetector = DuplicateMatchArmsDetector;

    #[test]
    fn detects_duplicate_match_arm_bodies() {
        let code = r#"
fn classify(value: i32) -> i32 {
    match value {
        1 => score(),
        2 => score(),
        _ => 0,
    }
}
"#;
        assert_smell_count(&DETECTOR, code, "Duplicate Match Arms", 1);
    }

    #[test]
    fn clean_same_field_projection_from_different_variant_payloads() {
        let code = r#"
enum Item {
    Const(ConstItem),
    Enum(EnumItem),
}

struct ConstItem {
    vis: bool,
}

struct EnumItem {
    vis: bool,
}

fn is_pub(item: Item) -> bool {
    let vis = match item {
        Item::Const(i) => &i.vis,
        Item::Enum(i) => &i.vis,
    };
    *vis
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_duplicate_body_that_depends_on_pattern_binding() {
        let code = r#"
fn collect_pat_idents(pat: &syn::Pat) {
    match pat {
        syn::Pat::Tuple(tuple) => {
            for elem in &tuple.elems {
                collect_pat_idents(elem);
            }
        }
        syn::Pat::TupleStruct(tuple) => {
            for elem in &tuple.elems {
                collect_pat_idents(elem);
            }
        }
        _ => {}
    }
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod inline_assembly {
    use super::*;
    use qualirs::detectors::r#unsafe::inline_assembly::InlineAssemblyDetector;
    static DETECTOR: InlineAssemblyDetector = InlineAssemblyDetector;

    #[test]
    fn detects_asm_macro() {
        let code = "fn foo() { unsafe { std::arch::asm!(\"nop\"); } }";
        assert_smell_count(&DETECTOR, code, "Inline Assembly", 1);
    }

    #[test]
    fn detects_global_asm_macro() {
        let code = "core::arch::global_asm!(\".global _start\");";
        assert_smell_count(&DETECTOR, code, "Inline Assembly", 1);
    }

    #[test]
    fn clean_other_macros() {
        let code = "fn foo() { println!(\"hello\"); vec![1, 2, 3]; }";
        assert_clean(&DETECTOR, code);
    }
}

// ─── False Positive Tests: Architecture ──────────────────────

mod hidden_global_state {
    use super::*;
    use qualirs::detectors::architecture::hidden_global_state::HiddenGlobalStateDetector;
    static DETECTOR: HiddenGlobalStateDetector = HiddenGlobalStateDetector;

    #[test]
    fn detects_many_statics() {
        let code = "\
static A: i32 = 1;
static B: i32 = 2;
static C: i32 = 3;
static D: i32 = 4;
";
        assert_smell_count(&DETECTOR, code, "Hidden Global State", 1);
    }

    #[test]
    fn clean_few_statics() {
        let code = "\
static COUNTER: i32 = 0;
static VERSION: &str = \"1.0\";
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_const_not_static() {
        // const items should not be counted as global state
        let code = "\
const MAX: i32 = 100;
const MIN: i32 = 0;
const NAME: &str = \"app\";
const VER: &str = \"1.0\";
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_no_globals() {
        let code = "fn main() { let x = 1; }";
        assert_clean(&DETECTOR, code);
    }
}

mod leaky_error {
    use super::*;
    use qualirs::detectors::architecture::leaky_error::LeakyErrorAbstractionDetector;
    static DETECTOR: LeakyErrorAbstractionDetector = LeakyErrorAbstractionDetector;

    #[test]
    fn detects_leaky_error() {
        // One smell per leaking variant
        let code = "\
pub enum AppError {
    Db(sqlx::Error),
    Http(reqwest::Error),
}
";
        assert_smell_count(&DETECTOR, code, "Leaky Error Abstraction", 2);
    }

    #[test]
    fn clean_private_error_enum() {
        // Non-public enums are safe
        let code = "\
enum AppError {
    Db(sqlx::Error),
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_non_error_name() {
        // Enum name not ending in "Error" is safe
        let code = "\
pub enum AppResult {
    Db(sqlx::Error),
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_named_field_variant() {
        // Named-field variants are safe (only tuple variants checked)
        let code = "\
pub enum AppError {
    Io { inner: std::io::Error },
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_non_flagged_crate() {
        // Crates not in the hardcoded list are safe
        let code = "\
pub enum AppError {
    Other(diesel::result::Error),
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_boxed_error() {
        // Box<reqwest::Error> is safe — first segment is "Box", not "reqwest"
        let code = "\
pub enum AppError {
    Http(Box<reqwest::Error>),
}
";
        assert_clean(&DETECTOR, code);
    }
}

// ─── False Positive Tests: Design ────────────────────────────

mod fat_impl {
    use super::*;
    use qualirs::detectors::design::fat_impl::FatImplDetector;
    static DETECTOR: FatImplDetector = FatImplDetector;

    #[test]
    fn detects_fat_impl() {
        let methods: String = (0..21)
            .map(|i| format!("fn method_{i}(&self) {{}}"))
            .collect::<Vec<_>>()
            .join("\n    ");
        let code = format!("struct Big;\nimpl Big {{\n    {methods}\n}}");
        assert_smell_count(&DETECTOR, &code, "Fat Impl (God Object)", 1);
    }

    #[test]
    fn clean_reasonable_impl() {
        let code = "\
struct Service;
impl Service {
    fn new() -> Self { Service }
    fn run(&self) {}
    fn stop(&self) {}
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_trait_impl_not_counted() {
        // Trait impls are not counted toward the fat impl limit
        let methods: String = (0..21)
            .map(|i| format!("fn method_{i}(&self);"))
            .collect::<Vec<_>>()
            .join("\n    ");
        let code = format!("trait Big {{\n    {methods}\n}}");
        assert_clean(&DETECTOR, &code);
    }

    #[test]
    fn clean_at_threshold() {
        // Exactly 20 methods does NOT trigger
        let methods: String = (0..20)
            .map(|i| format!("fn method_{i}(&self) {{}}"))
            .collect::<Vec<_>>()
            .join("\n    ");
        let code = format!("struct Big;\nimpl Big {{\n    {methods}\n}}");
        assert_clean(&DETECTOR, &code);
    }
}

mod primitive_obsession {
    use super::*;
    use qualirs::detectors::design::primitive_obsession::PrimitiveObsessionDetector;
    static DETECTOR: PrimitiveObsessionDetector = PrimitiveObsessionDetector;

    #[test]
    fn detects_primitive_obsession() {
        let code = "struct Data { a: i32, b: i32, c: i32, d: i32, e: i32 }";
        assert_smell_count(&DETECTOR, code, "Primitive Obsession", 1);
    }

    #[test]
    fn clean_mixed_types() {
        // A struct with non-primitive fields is safe
        let code = "struct Data { a: i32, b: i32, c: i32, d: i32, e: Vec<String> }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_few_fields() {
        let code = "struct Point { x: f64, y: f64, z: f64 }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_thresholds_suffix() {
        // Structs ending in "Thresholds" are excluded
        let code = "struct MyThresholds { a: i32, b: i32, c: i32, d: i32, e: i32 }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_config_and_view_carriers() {
        let code = "\
struct RateLimitConfig { a: i32, b: i32, c: i32, d: i32, e: i32 }
struct DashboardView { a: String, b: String, c: String, d: String, e: String }
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_option_not_primitive() {
        // Option<i32> is NOT considered primitive
        let code = "struct Data { a: i32, b: i32, c: i32, d: i32, e: Option<i32> }";
        assert_clean(&DETECTOR, code);
    }
}

mod data_clumps {
    use super::*;
    use qualirs::detectors::design::data_clumps::DataClumpsDetector;
    static DETECTOR: DataClumpsDetector = DataClumpsDetector;

    #[test]
    fn detects_data_clumps() {
        let code = "\
fn build_user(name: String, age: i32, email: String) {}
fn update_user(name: String, age: i32, email: String) {}
fn migrate_user(name: String, age: i32, email: String) {}
";
        assert_smell_count(&DETECTOR, code, "Data Clumps", 1);
    }

    #[test]
    fn clean_few_params() {
        // Functions with <3 params are never grouped
        let code = "\
fn build_user(name: String, age: i32) {}
fn update_user(name: String, age: i32) {}
fn migrate_user(name: String, age: i32) {}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_destructured_arguments_do_not_create_one_param_clumps() {
        let code = "\
fn simple_index(State(state): State, Path(tenant): Path<String>, headers: HeaderMap) {}
fn simple_project(State(state): State, Path(tenant): Path<String>, headers: HeaderMap) {}
fn download_artifact(State(state): State, Path(tenant): Path<String>, headers: HeaderMap) {}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_unique_signatures() {
        // Different parameter names break the match
        let code = "\
fn build_user(name: String, age: i32, email: String) {}
fn update_user(user_name: String, user_age: i32, user_email: String) {}
fn migrate_user(full_name: String, years: i32, mail: String) {}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_only_two_occurrences() {
        // Need 3+ occurrences of the same signature
        let code = "\
fn build_user(name: String, age: i32, email: String) {}
fn update_user(name: String, age: i32, email: String) {}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_trait_required_method_signatures() {
        let code = "\
trait Deserialize {
    fn deserialize_enum(self, name: &str, variants: &'static [&'static str], visitor: Visitor);
}

struct U8;
impl Deserialize for U8 {
    fn deserialize_enum(self, name: &str, variants: &'static [&'static str], visitor: Visitor) {}
}

struct U16;
impl Deserialize for U16 {
    fn deserialize_enum(self, name: &str, variants: &'static [&'static str], visitor: Visitor) {}
}

struct U32;
impl Deserialize for U32 {
    fn deserialize_enum(self, name: &str, variants: &'static [&'static str], visitor: Visitor) {}
}
";
        assert_clean(&DETECTOR, code);
    }
}

mod multiple_impl_blocks {
    use super::*;
    use qualirs::detectors::design::multiple_impl_blocks::MultipleImplBlocksDetector;
    static DETECTOR: MultipleImplBlocksDetector = MultipleImplBlocksDetector;

    #[test]
    fn detects_scattered_impl() {
        let code = "\
struct S;
impl S { fn a(&self) {} }
impl S { fn b(&self) {} }
";
        assert_smell_count(&DETECTOR, code, "Multiple Impl Blocks", 1);
    }

    #[test]
    fn clean_single_impl() {
        let code = "\
struct S;
impl S { fn a(&self) {} fn b(&self) {} }
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_trait_impls_only() {
        // Only inherent impls count; trait impls are excluded
        let code = "\
struct S;
impl S { fn a(&self) {} }
impl Debug for S { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { Ok(()) } }
impl Clone for S { fn clone(&self) -> Self { S } }
";
        assert_clean(&DETECTOR, code);
    }
}

mod rebellious_impl {
    use super::*;
    use qualirs::detectors::design::rebellious_impl::RebelliousImplDetector;
    static DETECTOR: RebelliousImplDetector = RebelliousImplDetector;

    #[test]
    fn detects_rebellious_repo() {
        let code = "\
struct UserRepository;
impl UserRepository {
    fn find_by_id(&self) {}
    fn print_report(&self) {}
}
";
        assert_smell_count(&DETECTOR, code, "Rebellious Impl", 1);
    }

    #[test]
    fn clean_unrelated_type() {
        // Type name doesn't match any keyword pattern
        let code = "\
struct UserService;
impl UserService {
    fn print_report(&self) {}
    fn format_output(&self) {}
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_well_behaved_repo() {
        // Repo with only data-access methods is safe
        let code = "\
struct UserRepository;
impl UserRepository {
    fn find_by_id(&self) {}
    fn save(&self) {}
    fn delete(&self) {}
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_well_behaved_validator() {
        // Validator with only validation methods is safe
        let code = "\
struct InputValidator;
impl InputValidator {
    fn is_valid(&self) -> bool { true }
    fn check_length(&self) -> bool { true }
}
";
        assert_clean(&DETECTOR, code);
    }
}

// ─── False Positive Tests: Implementation ────────────────────

mod deeply_nested_type {
    use super::*;
    use qualirs::detectors::implementation::deeply_nested_type::DeeplyNestedTypeDetector;
    static DETECTOR: DeeplyNestedTypeDetector = DeeplyNestedTypeDetector;

    #[test]
    fn detects_deep_nesting() {
        let code = "struct S { data: Arc<Mutex<HashMap<String, Vec<i32>>>> }";
        assert_smell_count(&DETECTOR, code, "Deeply Nested Type", 1);
    }

    #[test]
    fn clean_shallow_nesting() {
        let code = "struct S { data: HashMap<String, Vec<i32>> }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_no_generics() {
        let code = "struct S { value: i32 }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_option_result() {
        let code = "struct S { data: Option<Result<String, std::io::Error>> }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_common_result_option_vec_return() {
        let code = "trait Store { fn get(&self) -> Result<Option<Vec<u8>>, Error>; }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_references_do_not_inflate_depth() {
        let code = "fn parse(value: &&Option<Result<String, Error>>) {}";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_tuple_of_shallow_nested_types() {
        let code = "fn row(values: (&Option<Result<String, Error>>, &Option<Vec<String>>)) {}";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_cfg_test_module_nested_types() {
        let code = r#"
#[cfg(test)]
mod tests {
    struct FakeStore {
        values: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    }
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_fuzz_target_path() {
        let code = "struct DomainInput { data: Arc<Mutex<HashMap<String, Vec<u8>>>> }";
        let file = SourceFile::from_source(
            PathBuf::from("fuzz/fuzz_targets/domain_inputs.rs"),
            code.to_string(),
        )
        .unwrap();
        assert!(DETECTOR.detect(&file).is_empty());
    }
}

mod interior_mutability_abuse {
    use super::*;
    use qualirs::detectors::implementation::interior_mutability_abuse::InteriorMutabilityAbuseDetector;
    static DETECTOR: InteriorMutabilityAbuseDetector = InteriorMutabilityAbuseDetector;

    #[test]
    fn detects_many_refcell() {
        let fields: String = (0..6)
            .map(|i| format!("f{i}: RefCell<i32>"))
            .collect::<Vec<_>>()
            .join(", ");
        let code = format!("struct S {{ {fields} }}");
        assert_smell_count(&DETECTOR, &code, "Interior Mutability Abuse", 1);
    }

    #[test]
    fn clean_few_usages() {
        let code = "struct S { a: RefCell<i32>, b: Cell<bool> }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_no_interior_mutability() {
        let code = "struct S { value: i32 }";
        assert_clean(&DETECTOR, code);
    }
}

// ─── False Positive Tests: Concurrency ───────────────────────

mod sync_drop_blocking {
    use super::*;
    use qualirs::detectors::concurrency::sync_drop_blocking::SyncDropBlockingDetector;
    static DETECTOR: SyncDropBlockingDetector = SyncDropBlockingDetector;

    #[test]
    fn detects_blocking_in_drop() {
        let code = "\
struct Conn;
impl Drop for Conn {
    fn drop(&mut self) {
        self.flush();
    }
}
";
        assert_smell_count(&DETECTOR, code, "Sync Drop Blocking (Async Hazard)", 1);
    }

    #[test]
    fn clean_safe_drop() {
        let code = "\
struct Handle;
impl Drop for Handle {
    fn drop(&mut self) {
        println!(\"dropped\");
    }
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_no_drop_impl() {
        let code = "struct Handle;";
        assert_clean(&DETECTOR, code);
    }
}

mod async_trait_overhead {
    use super::*;
    use qualirs::detectors::concurrency::async_trait_overhead::AsyncTraitOverheadDetector;
    static DETECTOR: AsyncTraitOverheadDetector = AsyncTraitOverheadDetector;

    #[test]
    fn detects_async_trait() {
        let code = "\
#[async_trait]
pub trait Handler {
    async fn handle(&self);
}
";
        assert_smell_count(&DETECTOR, code, "Async Trait Overhead", 1);
    }

    #[test]
    fn clean_no_async_trait() {
        let code = "\
pub trait Handler {
    fn handle(&self);
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_other_attributes() {
        let code = "\
#[derive(Clone)]
pub struct Handler;
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_dyn_compatible_port_trait() {
        let code = "\
#[async_trait]
pub trait Handler: Send + Sync {
    async fn handle(&self);
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_async_trait_impl_duplicate() {
        let code = "\
#[async_trait]
impl Handler for Service {
    async fn handle(&self) {}
}
";
        assert_clean(&DETECTOR, code);
    }
}

mod unnecessary_allocation_in_loop {
    use super::*;
    use qualirs::detectors::implementation::unnecessary_allocation_in_loop::UnnecessaryAllocationInLoopDetector;
    static DETECTOR: UnnecessaryAllocationInLoopDetector = UnnecessaryAllocationInLoopDetector;

    #[test]
    fn detects_clear_invariant_loop_allocations() {
        let code = r#"
fn build(items: &[&str], prefix: &str) {
    for item in items {
        let a = String::from("fixed");
        let b = prefix.to_owned();
        let c = format!("prefix: {prefix}");
    }
}
"#;
        assert_smell_count(&DETECTOR, code, "Unnecessary Allocation in Loop", 3);
    }

    #[test]
    fn clean_loop_item_dependent_allocations() {
        let code = r#"
fn build(items: &[&str]) {
    for item in items {
        let a = String::from(*item);
        let b = item.to_owned();
        let c = format!("item: {item}");
    }
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_owned_reporting_and_grouping_work() {
        let code = r#"
fn report(items: &[i32]) {
    for item in items {
        let label = item.to_string();
        let selected: Vec<_> = [1, 2, 3].iter().filter(|value| **value > 1).collect();
        println!("{label}: {}", selected.len());
    }
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_borrowed_format_argument() {
        let code = r#"
fn report(items: &[i32]) {
    for item in items {
        consume(&format!("item: {item}"));
    }
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_error_formatting_inside_loop() {
        let code = r#"
fn validate(items: &[Item]) -> Result<(), ApplicationError> {
    for item in items {
        if item.is_invalid() {
            return Err(ApplicationError::Conflict(format!("invalid item {}", item.id)));
        }
        item.check().map_err(|error| format!("invalid item: {error}"))?;
    }
    Ok(())
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_match_arm_error_formatting_inside_loop() {
        let code = r#"
fn validate(items: &[Item]) {
    for item in items {
        match item.parse() {
            Ok(value) => consume(value),
            Err(error) => record(format!("invalid item: {error}")),
        }
    }
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_diagnostic_evidence_collection() {
        let code = r#"
fn scan(items: &[Rule]) {
    for item in items {
        let mut evidence = vec![format!("rule={}", item.name)];
        evidence.push(format!("tags={}", item.tags));
        evidence.extend(item.meta.iter().map(|meta| format!("meta={meta}")));
    }
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_inline_cfg_test_module() {
        let code = r#"
#[cfg(test)]
mod tests {
    fn fixture() {
        for index in 0..10 {
            let name = format!("fixture_{index}");
        }
    }
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_owned_origin_metadata_inside_loop() {
        let code = r#"
fn compile(files: &[RuleFile]) {
    for file in files {
        compiler.add_source(SourceCode::from(file.contents).with_origin(format!("bundled:{}", file.path)));
    }
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod manual_default_constructor {
    use super::*;
    use qualirs::detectors::implementation::manual_default_constructor::ManualDefaultConstructorDetector;
    static DETECTOR: ManualDefaultConstructorDetector = ManualDefaultConstructorDetector;

    #[test]
    fn detects_no_arg_defaultish_new() {
        let code = "\
struct Bag { items: Vec<String> }
impl Bag {
    fn new() -> Self {
        Self { items: Vec::new() }
    }
}
";
        assert_smell_count(&DETECTOR, code, "Manual Default Constructor", 1);
    }

    #[test]
    fn clean_constructor_uses_parameter() {
        let code = "\
struct Engine { detectors: Vec<String>, config: Config }
struct Config;
impl Engine {
    fn new(config: Config) -> Self {
        Self { detectors: Vec::new(), config }
    }
}
";
        assert_clean(&DETECTOR, code);
    }
}

mod manual_find_loop {
    use super::*;
    use qualirs::detectors::implementation::manual_find_loop::ManualFindLoopDetector;
    static DETECTOR: ManualFindLoopDetector = ManualFindLoopDetector;

    #[test]
    fn detects_direct_conditional_return_in_loop() {
        let code = "\
fn has_even(values: &[i32]) -> bool {
    for value in values {
        if *value % 2 == 0 {
            return true;
        }
    }
    false
}
";
        assert_smell_count(&DETECTOR, code, "Manual Find/Any Loop", 1);
    }

    #[test]
    fn clean_return_inside_nested_closure() {
        let code = "\
pub fn process(items: &[i32]) {
    for item in items {
        let selected = [1, 2, 3].iter().filter_map(|value| {
            if *value == *item {
                return Some(value);
            }
            None
        });
        let _ = selected.count();
    }
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_loop_returning_constructed_value() {
        let code = "\
fn find_path(paths: &[&str]) -> Option<String> {
    for path in paths {
        if path.ends_with(\".rs\") {
            return Some(path.to_string());
        }
    }
    None
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_stateful_loop_with_conditional_return() {
        let code = "\
fn block_holds_lock_across_await(block: &Block) -> bool {
    let mut active_guards = HashSet::new();

    for stmt in &block.stmts {
        if !active_guards.is_empty() && contains_await_in_stmt(stmt) {
            return true;
        }

        remove_explicitly_dropped_guards(stmt, &mut active_guards);
    }

    false
}
";
        assert_clean(&DETECTOR, code);
    }
}

mod derivable_impl {
    use super::*;
    use qualirs::detectors::implementation::derivable_impl::DerivableImplDetector;
    static DETECTOR: DerivableImplDetector = DerivableImplDetector;

    #[test]
    fn clean_custom_default_values() {
        let code = "\
struct Thresholds { limit: usize }
impl Default for Thresholds {
    fn default() -> Self {
        Self { limit: 50 }
    }
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn detects_mechanical_default_impl() {
        let code = "\
struct State { name: String, items: Vec<String> }
impl Default for State {
    fn default() -> Self {
        Self { name: String::new(), items: Vec::new() }
    }
}
";
        assert_smell_count(&DETECTOR, code, "Derivable Impl", 1);
    }

    #[test]
    fn clean_generic_clone_that_avoids_derive_bounds() {
        let code = "\
use std::marker::PhantomData;

struct StringDeserializer<E> {
    value: String,
    marker: PhantomData<E>,
}

impl<E> Clone for StringDeserializer<E> {
    fn clone(&self) -> Self {
        StringDeserializer {
            value: self.value.clone(),
            marker: PhantomData,
        }
    }
}
";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_cfg_sensitive_debug_impl() {
        let code = "\
struct Error {
    err: String,
}

impl Debug for Error {
    fn fmt(&self, formatter: &mut Formatter) -> Result {
        let mut debug = formatter.debug_tuple(\"Error\");
        #[cfg(feature = \"std\")]
        debug.field(&self.err);
        debug.finish()
    }
}
";
        assert_clean(&DETECTOR, code);
    }
}

mod needless_explicit_lifetime {
    use super::*;
    use qualirs::detectors::implementation::needless_explicit_lifetime::NeedlessExplicitLifetimeDetector;
    static DETECTOR: NeedlessExplicitLifetimeDetector = NeedlessExplicitLifetimeDetector;

    #[test]
    fn detects_lifetime_elidable_from_single_reference_input() {
        let code = "fn name<'a>(value: &'a str) -> &'a str { value }";
        assert_smell_count(&DETECTOR, code, "Needless Explicit Lifetime", 1);
    }

    #[test]
    fn clean_lifetime_used_in_where_bound() {
        let code = "\
fn missing_field<'de, V, E>(field: &'static str) -> Result<V, E>
where
    V: Deserialize<'de>,
    E: Error,
{
    todo!()
}
";
        assert_clean(&DETECTOR, code);
    }
}

mod missing_collection_preallocation {
    use super::*;
    use qualirs::detectors::implementation::missing_collection_preallocation::MissingCollectionPreallocationDetector;
    static DETECTOR: MissingCollectionPreallocationDetector =
        MissingCollectionPreallocationDetector;

    #[test]
    fn detects_empty_vec_grown_in_loop() {
        let code = r#"
fn build(items: &[u32]) -> Vec<u32> {
    let mut out = Vec::new();
    for item in items {
        out.push(*item);
    }
    out
}
"#;
        assert_smell_count(&DETECTOR, code, "Missing Collection Preallocation", 1);
    }

    #[test]
    fn clean_when_capacity_is_reserved() {
        let code = r#"
fn build(items: &[u32]) -> Vec<u32> {
    let mut out = Vec::new();
    out.reserve(items.len());
    for item in items {
        out.push(*item);
    }
    out
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_when_loop_size_is_not_obvious() {
        let code = r#"
fn build(reader: &mut Reader) -> Vec<u32> {
    let mut out = Vec::new();
    while let Some(item) = reader.next() {
        out.push(item);
    }
    out
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_reused_string_buffer() {
        let code = r#"
fn ascii_strings(contents: &[u8]) -> String {
    let mut current = String::new();
    for byte in contents {
        current.push(*byte as char);
        if *byte == 0 {
            current.clear();
        }
    }
    current
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod repeated_string_conversion {
    use super::*;
    use qualirs::detectors::implementation::repeated_string_conversion::RepeatedStringConversionDetector;
    static DETECTOR: RepeatedStringConversionDetector = RepeatedStringConversionDetector;

    #[test]
    fn detects_invariant_conversion_in_iterator_chain() {
        let code = r#"
fn labels(items: &[u32], prefix: &str) -> Vec<String> {
    items.iter().map(|item| prefix.to_string()).collect()
}
"#;
        assert_smell_count(&DETECTOR, code, "Repeated String Conversion in Hot Path", 1);
    }

    #[test]
    fn clean_loop_item_conversion_in_iterator_chain() {
        let code = r#"
fn labels(items: &[u32]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_loop_local_conversion() {
        let code = r#"
fn labels(items: &[Item]) -> Vec<String> {
    let mut out = Vec::new();
    for item in items {
        let label = item.label();
        out.push(label.to_string());
    }
    out
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_error_mapping_conversion() {
        let code = r#"
fn convert(result: Result<u32, Error>) -> Result<u32, String> {
    result.map_err(|error| error.to_string())
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_error_conversion_inside_loop() {
        let code = r#"
fn convert(items: &[Item]) -> Vec<String> {
    let mut errors = Vec::new();
    for item in items {
        match item.parse() {
            Ok(value) => consume(value),
            Err(error) => errors.push(error.to_string()),
        }

        item.check().map_err(|error| error.to_string()).ok();
    }
    errors
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_owned_struct_field_conversion() {
        let code = r#"
struct Activity {
    tenant_slug: String,
}

fn recent(projects: &[Project], tenant: &Tenant) -> Vec<Activity> {
    let mut out = Vec::new();
    for project in projects {
        consume(project);
        out.push(Activity {
            tenant_slug: tenant.slug.as_str().to_string(),
        });
    }
    out
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod needless_intermediate_string_formatting {
    use super::*;
    use qualirs::detectors::implementation::needless_intermediate_string_formatting::NeedlessIntermediateStringFormattingDetector;
    static DETECTOR: NeedlessIntermediateStringFormattingDetector =
        NeedlessIntermediateStringFormattingDetector;

    #[test]
    fn detects_push_str_format_temporary() {
        let code = r#"
fn render(id: u64, out: &mut String) {
    out.push_str(&format!("id={id}"));
}
"#;
        assert_smell_count(
            &DETECTOR,
            code,
            "Needless Intermediate String Formatting",
            1,
        );
    }

    #[test]
    fn clean_direct_write() {
        let code = r#"
fn render(id: u64, out: &mut String) {
    let _ = write!(out, "id={id}");
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod vec_contains_in_loop {
    use super::*;
    use qualirs::detectors::implementation::vec_contains_in_loop::VecContainsInLoopDetector;
    static DETECTOR: VecContainsInLoopDetector = VecContainsInLoopDetector;

    #[test]
    fn detects_vec_contains_inside_loop() {
        let code = r#"
fn count(ids: Vec<u64>, items: &[u64]) -> usize {
    let mut count = 0;
    for item in items {
        if ids.contains(item) {
            count += 1;
        }
    }
    count
}
"#;
        assert_smell_count(&DETECTOR, code, "Vec Contains in Loop", 1);
    }

    #[test]
    fn clean_slice_contains_inside_loop() {
        let code = r#"
fn count(ids: &[u64], items: &[u64]) -> usize {
    let mut count = 0;
    for item in items {
        if ids.contains(item) {
            count += 1;
        }
    }
    count
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod sort_before_min_max {
    use super::*;
    use qualirs::detectors::implementation::sort_before_min_max::SortBeforeMinMaxDetector;
    static DETECTOR: SortBeforeMinMaxDetector = SortBeforeMinMaxDetector;

    #[test]
    fn detects_sort_then_first() {
        let code = r#"
fn smallest(mut values: Vec<u32>) -> Option<u32> {
    values.sort();
    values.first().copied()
}
"#;
        assert_smell_count(&DETECTOR, code, "Sort Before Min or Max", 1);
    }

    #[test]
    fn clean_when_sorted_order_is_used() {
        let code = r#"
fn ordered(mut values: Vec<u32>) -> Vec<u32> {
    values.sort();
    values.iter().copied().collect()
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod full_sort_for_single_element {
    use super::*;
    use qualirs::detectors::implementation::full_sort_for_single_element::FullSortForSingleElementDetector;
    static DETECTOR: FullSortForSingleElementDetector = FullSortForSingleElementDetector;

    #[test]
    fn detects_sort_then_median_index() {
        let code = r#"
fn median(mut values: Vec<u32>) -> u32 {
    values.sort_unstable();
    values[values.len() / 2]
}
"#;
        assert_smell_count(&DETECTOR, code, "Full Sort for Single Element", 1);
    }

    #[test]
    fn clean_sort_then_first_index() {
        let code = r#"
fn first_sorted(mut values: Vec<u32>) -> u32 {
    values.sort();
    values[0]
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod clone_before_move_into_collection {
    use super::*;
    use qualirs::detectors::implementation::clone_before_move_into_collection::CloneBeforeMoveIntoCollectionDetector;
    static DETECTOR: CloneBeforeMoveIntoCollectionDetector = CloneBeforeMoveIntoCollectionDetector;

    #[test]
    fn detects_clone_pushed_without_later_use() {
        let code = r#"
fn store(value: String, out: &mut Vec<String>) {
    out.push(value.clone());
}
"#;
        assert_smell_count(&DETECTOR, code, "Clone Before Move Into Collection", 1);
    }

    #[test]
    fn clean_when_value_is_used_later() {
        let code = r#"
fn store(value: String, out: &mut Vec<String>) {
    out.push(value.clone());
    println!("{value}");
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_nested_push_when_outer_value_is_used_later() {
        let code = r#"
fn store(value: String, out: &mut Vec<String>, enabled: bool) {
    if enabled {
        out.push(value.clone());
    }
    consume(value);
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod inefficient_iterator_step {
    use super::*;
    use qualirs::detectors::implementation::inefficient_iterator_step::InefficientIteratorStepDetector;
    static DETECTOR: InefficientIteratorStepDetector = InefficientIteratorStepDetector;

    #[test]
    fn detects_nth_zero_and_skip_next() {
        let code = r#"
fn pick(mut iter: impl Iterator<Item = u32>, n: usize) {
    let _ = iter.nth(0);
    let _ = iter.skip(n).next();
}
"#;
        assert_smell_count(&DETECTOR, code, "Inefficient Iterator Step", 2);
    }

    #[test]
    fn clean_direct_next_and_nth() {
        let code = r#"
fn pick(mut iter: impl Iterator<Item = u32>, n: usize) {
    let _ = iter.next();
    let _ = iter.nth(n);
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod chars_count_length_check {
    use super::*;
    use qualirs::detectors::implementation::chars_count_length_check::CharsCountLengthCheckDetector;
    static DETECTOR: CharsCountLengthCheckDetector = CharsCountLengthCheckDetector;

    #[test]
    fn detects_chars_count_empty_check() {
        let code = r#"
fn empty(s: &str) -> bool {
    s.chars().count() == 0
}
"#;
        assert_smell_count(&DETECTOR, code, "Chars Count Length Check", 1);
    }

    #[test]
    fn clean_plain_count_value() {
        let code = r#"
fn scalar_count(s: &str) -> usize {
    s.chars().count()
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod repeated_expensive_construction {
    use super::*;
    use qualirs::detectors::implementation::repeated_expensive_construction::RepeatedExpensiveConstructionDetector;
    static DETECTOR: RepeatedExpensiveConstructionDetector = RepeatedExpensiveConstructionDetector;

    #[test]
    fn detects_invariant_url_parse_inside_loop() {
        let code = r#"
fn build(items: &[u32]) {
    for item in items {
        let url = Url::parse("https://example.com").unwrap();
        println!("{url:?} {item}");
    }
}
"#;
        assert_smell_count(
            &DETECTOR,
            code,
            "Repeated Expensive Construction in Loop",
            1,
        );
    }

    #[test]
    fn clean_loop_dependent_url_parse() {
        let code = r#"
fn parse_all(items: &[&str]) {
    for item in items {
        let url = Url::parse(item).unwrap();
        println!("{url:?}");
    }
}
"#;
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_loop_local_url_parse() {
        let code = r#"
fn parse_all(items: &[Item]) {
    for item in items {
        let raw = item.url();
        let url = Url::parse(raw).unwrap();
        println!("{url:?}");
    }
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod needless_dynamic_dispatch {
    use super::*;
    use qualirs::detectors::implementation::needless_dynamic_dispatch::NeedlessDynamicDispatchDetector;
    static DETECTOR: NeedlessDynamicDispatchDetector = NeedlessDynamicDispatchDetector;

    #[test]
    fn detects_local_box_dyn_from_concrete_value() {
        let code = r#"
trait Handler {}
struct Local;
impl Handler for Local {}

fn run() {
    let handler: Box<dyn Handler> = Box::new(Local);
}
"#;
        assert_smell_count(&DETECTOR, code, "Needless Dynamic Dispatch", 1);
    }

    #[test]
    fn clean_heterogeneous_dyn_collection() {
        let code = r#"
trait Handler {}

fn run() {
    let handlers: Vec<Box<dyn Handler>> = Vec::new();
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

mod local_lock_in_single_threaded_scope {
    use super::*;
    use qualirs::detectors::implementation::local_lock_in_single_threaded_scope::LocalLockInSingleThreadedScopeDetector;
    static DETECTOR: LocalLockInSingleThreadedScopeDetector =
        LocalLockInSingleThreadedScopeDetector;

    #[test]
    fn detects_local_mutex_lock() {
        let code = r#"
fn bump() {
    let lock = Mutex::new(0);
    *lock.lock().unwrap() += 1;
}
"#;
        assert_smell_count(&DETECTOR, code, "Local Lock in Single-Threaded Scope", 1);
    }

    #[test]
    fn clean_lock_moved_into_arc() {
        let code = r#"
fn share() {
    let lock = Mutex::new(0);
    let shared = Arc::new(lock);
}
"#;
        assert_clean(&DETECTOR, code);
    }
}

// ─── False Positive Tests: Unsafe ────────────────────────────

mod multi_mut_ref_unsafe {
    use super::*;
    use qualirs::detectors::r#unsafe::multi_mut_ref_unsafe::MultiMutRefUnsafeDetector;
    static DETECTOR: MultiMutRefUnsafeDetector = MultiMutRefUnsafeDetector;

    #[test]
    fn detects_multiple_mut_ref() {
        // Reports one smell per instance when threshold (>=2) is met
        let code = "\
fn foo(a: &mut i32, b: &mut i32) {
    let x = &mut *a;
    let y = &mut *b;
}
";
        assert_smell_count(&DETECTOR, code, "Multi Mut Ref Unsafe", 2);
    }

    #[test]
    fn clean_single_instance() {
        // A single &mut *expr is safe (< 2 threshold)
        let code = "fn foo(a: &mut i32) { let x = &mut *a; }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_immut_deref() {
        // Immutable deref is safe
        let code = "fn foo(a: &i32) { let x = &*a; let y = &*a; }";
        assert_clean(&DETECTOR, code);
    }

    #[test]
    fn clean_normal_refs() {
        let code = "fn foo(a: &mut i32) { *a = 42; }";
        assert_clean(&DETECTOR, code);
    }
}
