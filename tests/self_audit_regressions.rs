//! Regressions from QualiRS's own exploratory report.
use qualirs::{
    analysis::detector::Detector,
    detectors::{
        concurrency::{
            deadlock_risk::DeadlockRiskDetector,
            holding_lock_across_await::HoldingLockAcrossAwaitDetector,
        },
        implementation::{
            cyclomatic_complexity::CyclomaticComplexityDetector, deep_if_else::DeepIfElseDetector,
        },
    },
    domain::{
        smell::{FindingConfidence, Smell},
        source::SourceFile,
    },
};
use std::{fs, process::Command};

fn detect(detector: &dyn Detector, source: &str) -> Vec<Smell> {
    detector.detect(&SourceFile::from_source("src/lib.rs".into(), source.into()).unwrap())
}

#[test]
fn exclusive_output_branches_do_not_imply_deadlock() {
    let source = "use std::io::{self, Write}; fn emit(json: bool, text: &[u8]) -> io::Result<()> {
        if json { io::stdout().flush()?; io::stderr().lock().write_all(text) }
        else { io::stdout().lock().write_all(text) }
    }";
    assert!(detect(&DeadlockRiskDetector, source).is_empty());
}

#[test]
fn deadlock_hints_require_a_live_guard_and_known_blocking_acquisition() {
    for body in [
        "if flag { let first = a.lock().unwrap(); } else { let second = b.lock().unwrap(); }",
        "{ let first = a.lock().unwrap(); } let second = b.lock().unwrap();",
        "let first = a.lock().unwrap(); drop(first); let second = b.lock().unwrap();",
        "let first = a.lock().unwrap(); consume(first); let second = b.lock().unwrap();",
        "let first = a.lock().unwrap(); let moved = first; drop(moved); let second = b.lock().unwrap();",
        "let first = a.lock().unwrap(); let second = b.try_lock();",
        "a.lock().unwrap(); b.lock().unwrap();",
        "let first = a.lock().unwrap(); let f = || b.lock().unwrap();",
        "let first = a.lock().unwrap(); let f = async { b.lock().unwrap() };",
        "let first = a.lock().unwrap(); fn child(b: &Mutex<i32>) { let second = b.lock().unwrap(); }",
        "let first = a.lock().unwrap(); let b = Custom; b.lock();",
        "let first = a.lock().unwrap(); ({ drop(first); b }).lock().unwrap();",
    ] {
        let source = format!(
            "use std::sync::Mutex; fn f(a: &Mutex<i32>, b: &Mutex<i32>, flag: bool) {{ {body} }}"
        );
        assert!(
            detect(&DeadlockRiskDetector, &source).is_empty(),
            "{source}"
        );
    }
    for prefix in [
        "",
        "use tokio::sync::Mutex;",
        "struct Mutex;",
        "use custom::Mutex;",
    ] {
        let source = format!(
            "{prefix} fn f(a: &Mutex, b: &Mutex) {{ let x = a.lock().unwrap(); let y = b.lock().unwrap(); }}"
        );
        assert!(
            detect(&DeadlockRiskDetector, &source).is_empty(),
            "{source}"
        );
    }
    let source = "fn write(a: &mut File, b: &mut File) { a.write(bytes); b.write(bytes); }";
    assert!(detect(&DeadlockRiskDetector, source).is_empty());
}

#[test]
fn overlapping_standard_guards_remain_exploratory_at_the_second_acquisition() {
    let source = "use std::sync::Mutex as Lock;
fn f(a: &Lock<i32>, b: &Lock<i32>) {
    let first = a.lock().unwrap();
    let second = b.lock().unwrap();
}";
    let findings = detect(&DeadlockRiskDetector, source);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].location.line_start, 4);
    assert_eq!(findings[0].confidence, FindingConfidence::Low);
    assert!(findings[0].message.contains("unproven"));
    // A wildcard binding does not move/drop an existing guard.
    let source = source.replace("let second", "let _ = first; let second");
    assert_eq!(detect(&DeadlockRiskDetector, &source).len(), 1);
}

#[test]
fn guard_move_and_drop_also_updates_await_evidence() {
    let source = "use std::sync::Mutex; async fn f(lock: &Mutex<i32>) {
        let guard = lock.lock().unwrap(); let moved = guard; drop(moved); pending().await;
    }";
    assert!(detect(&HoldingLockAcrossAwaitDetector, source).is_empty());
    let source = source.replace("drop(moved);", "");
    assert_eq!(detect(&HoldingLockAcrossAwaitDetector, &source).len(), 1);
}

fn branches() -> String {
    (0..20)
        .map(|i| format!("if x > {i} {{ touch(); }} "))
        .collect()
}

#[test]
fn nested_helpers_are_measured_independently() {
    let source = format!(
        "fn outer() {{\nfn inner(x: u32) {{ {} }}\ninner(1);\n}}",
        branches()
    );
    let findings = detect(&CyclomaticComplexityDetector, &source);
    assert_eq!(findings.len(), 1);
    assert!(findings[0].message.contains("`inner`"));
    assert!(findings[0].message.contains("21"));
    assert_eq!(findings[0].location.line_start, 2);
    for body in [
        format!("let work = |x: u32| {{ {} }};", branches()),
        format!("let work = async {{ let x = 1; {} }};", branches()),
        format!("let work = const {{ let x = 1; {} }};", branches()),
    ] {
        assert!(
            detect(
                &CyclomaticComplexityDetector,
                &format!("fn outer() {{ {body} }}")
            )
            .is_empty()
        );
    }
}

#[test]
fn methods_and_trait_defaults_keep_their_own_complexity_findings() {
    let source = format!(
        "mod nested {{ struct T; impl T {{ fn method(x: u32) {{ {} }} }} trait Trait {{ fn default_method(x: u32) {{ {} }} }} }}",
        branches(),
        branches()
    );
    let findings = detect(&CyclomaticComplexityDetector, &source);
    assert_eq!(findings.len(), 2);
    assert!(findings[0].message.contains("`method`"));
    assert!(findings[1].message.contains("`default_method`"));
}

#[test]
fn else_if_ladders_are_not_nested_conditionals() {
    let ladder = (0..10)
        .map(|i| format!("if x == {i} {{ touch(); }}"))
        .collect::<Vec<_>>()
        .join(" else ");
    assert!(detect(&DeepIfElseDetector, &format!("fn f(x: u32) {{ {ladder} }}")).is_empty());
    let nested = format!("{}touch();{}", "if flag { ".repeat(5), "}".repeat(5));
    let findings = detect(
        &DeepIfElseDetector,
        &format!("fn outer() {{ fn inner(flag: bool) {{ {nested} }} }}"),
    );
    assert_eq!(findings.len(), 1);
    assert!(findings[0].message.contains("`inner`"));
    assert!(findings[0].message.contains("depth of 5"));
    for body in [
        format!("|| {{ {nested} }}"),
        format!("async {{ {nested} }}"),
        format!("const {{ {nested} }}"),
    ] {
        assert!(
            detect(
                &DeepIfElseDetector,
                &format!("fn outer() {{ let work = {body}; }}")
            )
            .is_empty()
        );
    }
}

#[test]
fn reports_respect_precision_locations_and_inline_ignores() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.rs");
    let source = "use std::sync::Mutex;\nfn f(a: &Mutex<i32>, b: &Mutex<i32>) {\nlet first = a.lock().unwrap();\nlet second = b.lock().unwrap();\n}";
    for ignored in [false, true] {
        fs::write(
            &path,
            if ignored {
                source.replace("let second", "// qualirs:ignore Q0079\nlet second")
            } else {
                source.into()
            },
        )
        .unwrap();
        for mode in ["conservative", "balanced", "exploratory"] {
            let output = Command::new(env!("CARGO_BIN_EXE_qualirs"))
                .arg(&path)
                .args(["--precision", mode, "--format", "json"])
                .output()
                .unwrap();
            let expected = !ignored && mode == "exploratory";
            assert_eq!(output.status.code(), Some(i32::from(expected)));
            let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            let findings: Vec<_> = report["smells"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|s| s["code"] == "Q0079")
                .collect();
            assert_eq!(findings.len(), usize::from(expected));
            if expected {
                assert_eq!(findings[0]["location"]["line_start"], 4);
            }
        }
    }
}

#[test]
fn excluded_test_helpers_do_not_contribute_to_metrics() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.rs");
    fs::write(
        &path,
        format!(
            "fn outer() {{ #[cfg(test)] fn inner(x: u32) {{ {} }} }}",
            branches()
        ),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_qualirs"))
        .arg(&path)
        .args(["--precision", "exploratory", "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        report["smells"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["code"] != "Q0035")
    );
    let config = dir.path().join("qualirs.toml");
    fs::write(&config, "[policy]\nskip_tests = false\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_qualirs"))
        .arg(&path)
        .arg("--config")
        .arg(&config)
        .args(["--precision", "balanced", "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        report["smells"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["code"] == "Q0035")
            .count(),
        1
    );
}

#[test]
fn inline_hints_do_not_claim_method_name_matches_resolve_to_the_helper() {
    use qualirs::detectors::implementation::inline_candidate::InlineCandidateDetector;
    let source = "struct Docs; impl Docs { fn contains(&self, value: &str) -> bool { true } }
        fn f(text: String) { text.contains(\"a\"); text.contains(\"b\"); text.contains(\"c\"); }";
    let findings = detect(&InlineCandidateDetector, source);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].confidence, FindingConfidence::Low);
    assert!(
        findings[0]
            .message
            .contains("shares its name with 3 method calls")
    );
    assert!(
        findings[0]
            .message
            .contains("receiver types and runtime frequency are unverified")
    );
    assert!(!findings[0].message.contains("called 3 times"));
}
