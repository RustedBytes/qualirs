//! Helix audit regressions: operation-specific notes above iterator closures.
use qualirs::{
    analysis::detector::Detector,
    detectors::r#unsafe::{
        transmute_usage::TransmuteUsageDetector,
        unsafe_without_comment::UnsafeWithoutCommentDetector,
    },
    domain::{smell::Smell, source::SourceFile},
};
use std::{fs, process::Command};

fn detect(d: &dyn Detector, code: &str) -> Vec<Smell> {
    d.detect(&SourceFile::from_source("src/lib.rs".into(), code.into()).unwrap())
}

#[test]
fn named_lifetime_explanations_cover_the_single_transmute_in_an_iterator_closure() {
    for storage in ["diff_base", "doc"] {
        let code = format!(
            "use std::mem::transmute;\nfn update() {{\n\
            // Safety: This transmute only changes the lifetime.\n\
            // The backing storage is stored in self.{storage} and remains valid.\n\
            // It is only replaced after the interner is cleared.\n\
            let lines = self.{storage}.lines().map(|line: Slice| -> Slice {{ unsafe {{ transmute(line) }} }});\n\
            consume(lines);\n}}"
        );
        assert!(detect(&UnsafeWithoutCommentDetector, &code).is_empty());
        let found = detect(&TransmuteUsageDetector, &code);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].location.line_start, 6);
    }
}

#[test]
fn operation_notes_do_not_cover_other_calls_statements_or_deferred_bodies() {
    for expression in [
        "|| unsafe { unrelated(value) }",
        "|| { unsafe { cast(value) }; unsafe { cast(value) }; }",
        "|| { unsafe { cast(value) }; unsafe { *ptr }; }",
        "async { unsafe { cast(value) } }",
        "async || unsafe { cast(value) }",
        "|| { fn nested() { unsafe { cast(value) }; } }",
        "|| { || unsafe { cast(value) } }",
    ] {
        let code = format!(
            "fn f() {{\n// SAFETY: cast preserves the valid lifetime.\nlet work = {expression};\n}}"
        );
        assert!(
            !detect(&UnsafeWithoutCommentDetector, &code).is_empty(),
            "{code}"
        );
    }
    for note in [
        "// SAFETY: constructing this closure does not execute cast.",
        "// SAFETY: constructing a closure is safe; captured values have a valid lifetime.",
    ] {
        let code = format!("fn f() {{\n{note}\nlet work = || unsafe {{ cast(value) }};\n}}");
        assert!(
            !detect(&UnsafeWithoutCommentDetector, &code).is_empty(),
            "{code}"
        );
    }
    let code = "fn f() {\n// SAFETY: cast preserves the valid lifetime.\nlet unrelated = || {};\nlet work = || unsafe { cast(value) };\n}";
    assert_eq!(detect(&UnsafeWithoutCommentDetector, code).len(), 1);
    let code = "fn f() { let text = \"SAFETY: cast preserves a valid lifetime\"; let work = || unsafe { cast(value) }; }";
    assert_eq!(detect(&UnsafeWithoutCommentDetector, code).len(), 1);
}

#[test]
fn precision_reports_keep_real_transmute_findings_and_accurate_locations() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.rs");
    let source = "fn f(values: Vec<u32>) {\n// SAFETY: transmute preserves initialized bits; both types have equal size.\nlet values = values.into_iter().map(|value| unsafe { std::mem::transmute::<u32, f32>(value) });\nconsume(values);\n}";
    fs::write(&path, source).unwrap();
    for mode in ["conservative", "balanced", "exploratory"] {
        let out = Command::new(env!("CARGO_BIN_EXE_qualirs"))
            .arg(&path)
            .args(["--precision", mode, "--format", "json"])
            .output()
            .unwrap();
        // The real transmute remains critical. Only the false documentation warning is removed.
        assert_eq!(out.status.code(), Some(1));
        let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let findings = report["smells"].as_array().unwrap();
        assert!(findings.iter().all(|f| f["code"] != "Q0087"));
        let transmute: Vec<_> = findings.iter().filter(|f| f["code"] == "Q0088").collect();
        assert_eq!(transmute.len(), 1);
        assert_eq!(transmute[0]["location"]["line_start"], 3);
    }
    fs::write(
        &path,
        source.replace("let values =", "// qualirs:ignore Q0088\nlet values ="),
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_qualirs"))
        .arg(&path)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
}
