//! Reductions from the Polkadot SDK false-positive audit.
use qualirs::{
    analysis::detector::Detector,
    detectors::r#unsafe::{
        unsafe_impl_safety_docs::UnsafeImplSafetyDocsDetector,
        unsafe_without_comment::UnsafeWithoutCommentDetector,
    },
    domain::{smell::Smell, source::SourceFile},
};
use std::{fs, process::Command};

fn detect(detector: &dyn Detector, code: &str) -> Vec<Smell> {
    detector.detect(&SourceFile::from_source("src/lib.rs".into(), code.into()).unwrap())
}

#[test]
fn wasm_threading_rationales_document_marker_impls() {
    for note in [
        "// Wasm does not support threads, so this is safe; qed.",
        "// This is safe here since we are single-threaded in WASM",
        "// NOTE: Safe only in wasm (guarded above) because there's only one thread.",
        "// The lock serializes all access; therefore sharing is safe.",
    ] {
        let source = format!(
            "struct Provider;\n{note}\nunsafe impl Send for Provider {{}}\nunsafe impl Sync for Provider {{}}"
        );
        assert!(
            detect(&UnsafeImplSafetyDocsDetector, &source).is_empty(),
            "{source}"
        );
        assert!(
            detect(&UnsafeWithoutCommentDetector, &source).is_empty(),
            "{source}"
        );
    }
}

#[test]
fn unrelated_or_missing_rationales_still_report_marker_impls() {
    for note in [
        "",
        "// This is safe.",
        "// This is unsafe because access is not synchronized.",
        "// Safe here since",
        "// So this is safe",
        "// We construct a safe wrapper.",
        "const TEXT: &str = \"Wasm has no threads, so this is safe\";",
        "// Wasm has no threads, so this is safe\nfn unrelated() {}",
    ] {
        let source = format!("struct Provider;\n{note}\nunsafe impl Sync for Provider {{}}");
        assert_eq!(
            detect(&UnsafeImplSafetyDocsDetector, &source).len(),
            1,
            "{source}"
        );
        assert_eq!(
            detect(&UnsafeWithoutCommentDetector, &source).len(),
            1,
            "{source}"
        );
    }
}

#[test]
fn safety_rationales_do_not_leak_into_other_types_or_execution_contexts() {
    let source = "struct A; struct B;\n// This is safe here since the runtime is single-threaded.\nunsafe impl Send for A {}\nunsafe impl Sync for B {}";
    assert_eq!(detect(&UnsafeImplSafetyDocsDetector, source).len(), 1);
    for expr in ["|| unsafe { *ptr }", "async { unsafe { *ptr } }"] {
        let source = format!(
            "fn f() {{\n// Captures are initialized, so constructing this is safe.\nlet work = {expr};\n}}"
        );
        assert_eq!(detect(&UnsafeWithoutCommentDetector, &source).len(), 1);
    }
}

#[test]
fn safety_reports_preserve_modes_locations_ignores_and_ci_exit_status() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.rs");
    let documented = "struct Provider;\n// Wasm does not support threads, so this is safe; qed.\nunsafe impl Sync for Provider {}\n";
    for mode in ["conservative", "balanced", "exploratory"] {
        for (suffix, expected) in [
            ("", 0),
            ("struct Other;\nunsafe impl Send for Other {}\n", 1),
            (
                "struct Other;\n// qualirs:ignore Q0087 Q0094\nunsafe impl Send for Other {}\n",
                0,
            ),
        ] {
            fs::write(&path, format!("{documented}{suffix}")).unwrap();
            let output = Command::new(env!("CARGO_BIN_EXE_qualirs"))
                .arg(&path)
                .args(["--precision", mode, "--format", "json"])
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(expected));
            let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            let findings = report["smells"].as_array().unwrap();
            let markers: Vec<_> = findings.iter().filter(|f| f["code"] == "Q0094").collect();
            assert_eq!(markers.len(), expected as usize);
            if expected == 1 {
                assert_eq!(markers[0]["location"]["line_start"], 5);
            }
        }
    }
    let config = dir.path().join("qualirs.toml");
    fs::write(&config, "ignore_findings = [\"Q0087\", \"Q0094\"]\n").unwrap();
    fs::write(
        &path,
        "struct Provider;\nunsafe impl Sync for Provider {}\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_qualirs"))
        .arg(&path)
        .arg("--config")
        .arg(&config)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["smells"].as_array().unwrap().is_empty());
}

#[test]
fn manual_search_preserves_async_and_conditional_branch_work() {
    use qualirs::detectors::implementation::manual_find_loop::ManualFindLoopDetector;
    for body in [
        "if !known(item) { log_unknown(item); modify_reputation(item).await; return false; }",
        "if !known(item) { metrics.record(); return false; }",
        "if predicate(item).await { return true; }",
        "if predicate(item)? { return true; }",
        "if { if item == 0 { return false; } item > 1 } { return true; }",
        "if predicate!(item) { return true; }",
        "if known(item) { return true; } else { update(item); }",
    ] {
        let code = format!(
            "async fn check(items: Vec<u32>) -> bool {{ for item in items {{ {body} }} false }}"
        );
        assert!(detect(&ManualFindLoopDetector, &code).is_empty(), "{code}");
    }
    let code = "fn check(items: &[u32]) -> bool { for item in items { if *item > 2 { return true; } } false }";
    assert_eq!(detect(&ManualFindLoopDetector, code).len(), 1);
}

#[test]
fn cleanup_loop_notes_are_scoped_and_keep_errors_exploratory() {
    use qualirs::detectors::implementation::unused_result::UnusedResultDetector;
    use qualirs::domain::smell::FindingConfidence;
    let code = "fn clean(paths: Vec<&str>) {
        // This is best-effort cleanup, so ignore any errors.
        for path in paths {
            if is_dir(path) { let _ = std::fs::remove_dir_all(path); }
            else { let _ = std::fs::remove_file(path); }
        }
        let _ = std::fs::write(\"state\", b\"data\");
    }";
    let findings = detect(&UnusedResultDetector, code);
    assert_eq!(findings.len(), 3);
    assert_eq!(findings[0].confidence, FindingConfidence::Low);
    assert_eq!(findings[1].confidence, FindingConfidence::Low);
    assert_eq!(findings[2].confidence, FindingConfidence::High);
    for nested in [
        "fn other() { let _ = std::fs::remove_file(\"x\"); }",
        "let work = || { let _ = std::fs::remove_file(\"x\"); };",
        "let work = async { let _ = std::fs::remove_file(\"x\"); };",
    ] {
        let code =
            format!("fn f() {{\n// Best-effort cleanup\nfor path in paths {{ {nested} }} }}");
        let findings = detect(&UnusedResultDetector, &code);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].confidence, FindingConfidence::High, "{code}");
    }
}

#[test]
fn cleanup_loop_reporting_respects_precision() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.rs");
    fs::write(&path, "fn cleanup(paths: Vec<&str>) {\n// Best-effort cleanup: ignore errors.\nfor path in paths { let _ = std::fs::remove_file(path); }\n}").unwrap();
    for mode in ["conservative", "balanced", "exploratory"] {
        let output = Command::new(env!("CARGO_BIN_EXE_qualirs"))
            .arg(&path)
            .args(["--precision", mode, "--format", "json"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let findings: Vec<_> = report["smells"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["code"] == "Q0068")
            .collect();
        assert_eq!(findings.len(), usize::from(mode == "exploratory"));
        if mode == "exploratory" {
            assert_eq!(findings[0]["location"]["line_start"], 3);
            assert_eq!(findings[0]["confidence"], "low");
        }
    }
}

#[test]
fn supported_search_transformation_compiles_and_preserves_short_circuit_behavior() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("search.rs");
    let program = dir
        .path()
        .join(format!("search{}", std::env::consts::EXE_SUFFIX));
    fs::write(
        &source,
        r#"
fn original(values: &[i32], visits: &mut usize) -> bool {
    for value in values { if { *visits += 1; *value % 2 == 0 } { return true; } }
    false
}
fn transformed(values: &[i32], visits: &mut usize) -> bool {
    values.iter().any(|value| { *visits += 1; *value % 2 == 0 })
}
fn main() {
    for values in [&[][..], &[1, 3, 5], &[1, 2, 3], &[2, 4, 6]] {
        let (mut before, mut after) = (0, 0);
        assert_eq!(original(values, &mut before), transformed(values, &mut after));
        assert_eq!(before, after);
    }
}
"#,
    )
    .unwrap();
    let output = Command::new("rustc")
        .arg("--edition=2024")
        .arg(&source)
        .arg("-o")
        .arg(&program)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(Command::new(program).status().unwrap().success());
}

#[test]
fn own_child_imports_and_external_dependencies_do_not_prove_cycles() {
    use qualirs::detectors::architecture::cyclic_crate_dependency::CyclicDependencyDetector;
    for (path, source) in [
        (
            "src/equivocation/mod.rs",
            "mod source; mod target; use crate::{equivocation::{source::Source, target::Target}, finality_base::Engine}; use async_trait::async_trait;",
        ),
        (
            "src/reward.rs",
            "use crate::reward::Reward; use std::collections::HashMap;",
        ),
        (
            "src/lib.rs",
            "use alpha::A; use beta::B; use gamma::C; use delta::D; use epsilon::E; use zeta::F; use eta::G;",
        ),
        (
            "src/lib.rs",
            "use crate::{One, Two, Three, Four, Five, Six};",
        ),
        ("src/lib.rs", "use crate::one::{A, B, C, D, E, F};"),
    ] {
        let file = SourceFile::from_source(path.into(), source.into()).unwrap();
        assert!(
            CyclicDependencyDetector.detect(&file).is_empty(),
            "{source}"
        );
    }
    let source = "use crate::{one::A as Renamed, two::B, three::C, four::D, five::E, root_with_underscore::F};";
    let found = detect(&CyclicDependencyDetector, source);
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].confidence,
        qualirs::domain::smell::FindingConfidence::Low
    );
    assert!(found[0].message.contains("6 crate-relative roots"));
    assert!(found[0].message.contains("unverified"));
}

#[test]
fn coupling_hints_are_exploratory_and_test_imports_are_filtered() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.rs");
    let source = "use crate::{one::A, two::B, three::C, four::D, five::E, six::F};";
    for test_only in [false, true] {
        fs::write(
            &path,
            format!("{}{source}", if test_only { "#[cfg(test)]\n" } else { "" }),
        )
        .unwrap();
        for mode in ["conservative", "balanced", "exploratory"] {
            let output = Command::new(env!("CARGO_BIN_EXE_qualirs"))
                .arg(&path)
                .args(["--precision", mode, "--format", "json"])
                .output()
                .unwrap();
            assert!(output.status.success());
            let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            let count = report["smells"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|s| s["code"] == "Q0004")
                .count();
            assert_eq!(count, usize::from(!test_only && mode == "exploratory"));
        }
    }
}

#[test]
fn documented_utf8_invariants_cover_header_accessors() {
    for field in ["name", "value"] {
        let source = format!(
            "struct Header {{ {field}: Vec<u8> }}\nimpl Header {{ fn get(&self) -> &str {{\n// Header bytes are always produced from `&str` so this is safe.\nunsafe {{ std::str::from_utf8_unchecked(&self.{field}) }}\n}} }}"
        );
        assert!(detect(&UnsafeWithoutCommentDetector, &source).is_empty());
    }
}
