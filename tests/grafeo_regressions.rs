//! Reductions of the Grafeo audit, with supported positives and boundary cases.
use qualirs::detectors::{concurrency as c, implementation as i, r#unsafe as u};
use qualirs::{
    analysis::detector::Detector,
    domain::{
        smell::{FindingConfidence as Confidence, Severity, Smell},
        source::SourceFile,
    },
};
use std::{fs, process::Command};

fn detect(d: &dyn Detector, source: &str) -> Vec<Smell> {
    d.detect(&SourceFile::from_source("src/lib.rs".into(), source.into()).unwrap())
}
fn clean(d: &dyn Detector, source: &str) {
    assert!(
        detect(d, source).is_empty(),
        "{source}\n{:?}",
        detect(d, source)
    );
}
fn confidence(d: &dyn Detector, source: &str, expected: Confidence) {
    let findings = detect(d, source);
    assert!(!findings.is_empty(), "{source}");
    assert!(
        findings.iter().all(|f| f.confidence == expected),
        "{findings:?}"
    );
}

#[test]
fn safety_explanations_survive_comments_attributes_and_unicode() {
    let d = u::unsafe_without_comment::UnsafeWithoutCommentDetector;
    for prefix in [
        "// SAFETY: valid pointer.\n// reason: conversion fits\n#[allow(unsafe_code)]",
        "// SAFETY: valid pointer.\n// Long explanation\n// continues\n// over several\n// lines.\n#[allow(unsafe_code)]",
        "/* SAFETY: valid pointer.\n * Long explanation.\n */\n#[allow(unsafe_code)]",
        "// SAFETY: указатель действителен.\n#[allow(\n    unsafe_code,\n    unused_variables\n)]",
    ] {
        clean(
            &d,
            &format!("fn read(p: *const u8) {{\n{prefix}\nlet value = unsafe {{ *p }};\n}}"),
        );
    }
    let source = "fn read(p: *const u8) {\n// ordinary comment\nlet value = unsafe { *p };\n}";
    let found = detect(&d, source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].location.line_start, 3);
}

#[test]
fn safety_explanations_cover_explicit_operation_groups() {
    let d = u::unsafe_without_comment::UnsafeWithoutCommentDetector;
    clean(
        &d,
        "fn startup(flag: bool) {\n// SAFETY: no threads exist yet.\n#[allow(unsafe_code)]\nif flag { unsafe { std::env::set_var(\"X\", \"Y\") }; } else { unsafe { std::env::remove_var(\"X\") }; }\n}",
    );
    clean(
        &d,
        "fn mmap(entry: Entry) {\n// SAFETY: the file and range are valid.\n// Bounds explained here.\n#[allow(unused)]\nlet len = entry.length as usize;\nlet mapping = unsafe { map(len) };\n}",
    );
    clean(
        &d,
        "fn ffi(env: Env) {\nlet mut a = std::ptr::null_mut();\n// SAFETY: env is valid and the output pointer is writable.\ncheck(unsafe { sys::create(env, &raw mut a) });\nlet mut b = std::ptr::null_mut();\ncheck(unsafe { sys::create(env, &raw mut b) });\n}",
    );
    let source = "fn ffi(env: Env) {\n// SAFETY: first operation is valid.\nunsafe { sys::first(env) };\nunsafe { sys::other(env) };\n}";
    assert_eq!(detect(&d, source).len(), 1);
    clean(
        &d,
        "fn f(x: Option<u8>, p: *const u8) { match x {\n// SAFETY: pointer is valid.\nSome(_) => unsafe { *p },\nNone => 0,\n}; }",
    );
    clean(
        &d,
        "/// # Safety\n/// Both pointers must be valid.\nfn f(a: *const u8, b: *const u8) { let x = unsafe { *a }; let y = unsafe { *b }; }",
    );
}

#[test]
fn function_body_preconditions_and_adjacent_marker_impls_are_documented() {
    let source = "struct Chunk;\n// SAFETY: contains no shared mutable state.\nunsafe impl Send for Chunk {}\nunsafe impl Sync for Chunk {}\n";
    clean(
        &u::unsafe_without_comment::UnsafeWithoutCommentDetector,
        source,
    );
    clean(
        &u::unsafe_impl_safety_docs::UnsafeImplSafetyDocsDetector,
        source,
    );
    let source = "struct Model;\n// SAFETY: synchronization protects state.\n// All access respects this invariant.\n// More detail here.\n#[allow(unsafe_code)]\nunsafe impl Send for Model {}\n#[allow(unsafe_code)]\nunsafe impl Sync for Model {}";
    clean(
        &u::unsafe_without_comment::UnsafeWithoutCommentDetector,
        source,
    );
    clean(
        &u::unsafe_impl_safety_docs::UnsafeImplSafetyDocsDetector,
        source,
    );
    for name in ["dot", "euclidean", "cosine", "manhattan"] {
        clean(
            &u::unsafe_without_comment::UnsafeWithoutCommentDetector,
            &format!(
                "unsafe fn {name}() {{\nuse std::arch::x86_64::*;\n// SAFETY: dispatcher checked lengths and features.\nlet n = 0;\n}}"
            ),
        );
    }
    confidence(
        &u::unsafe_impl_safety_docs::UnsafeImplSafetyDocsDetector,
        "struct Raw(*mut u8); unsafe impl Send for Raw {}",
        Confidence::High,
    );
}

#[test]
fn safety_notes_do_not_leak_into_unrelated_code_or_execution_contexts() {
    let d = u::unsafe_without_comment::UnsafeWithoutCommentDetector;
    for source in [
        "fn f(p: *const u8) { ordinary(); // SAFETY: this comment belongs to ordinary\nunsafe { *p }; }",
        "fn f(p: *const u8) { let text = r#\"// SAFETY: pretend comment\"#; unsafe { *p }; }",
        "fn f(p: *const u8) {\n// SAFETY: first pointer is valid\nunsafe { *p };\nordinary_call();\nunsafe { *p };\n}",
        "fn f(p: *const u8) {\n// SAFETY: constructing the closure is safe\nlet f = || { unsafe { *p }; };\n}",
        "fn f(p: *const u8) {\n// SAFETY: constructing the future is safe\nlet f = async { unsafe { *p }; };\n}",
        "// SAFETY: first function contract\nunsafe fn a() {}\nunsafe fn b() {}",
    ] {
        assert!(!detect(&d, source).is_empty(), "{source}");
    }
    assert_eq!(detect(&u::unsafe_impl_safety_docs::UnsafeImplSafetyDocsDetector,
        "struct A; struct B;\n// SAFETY: A is thread safe.\nunsafe impl Send for A {}\nunsafe impl Sync for B {}").len(), 1);
}

#[test]
fn all_audited_default_delegations_are_already_idiomatic() {
    let d = i::manual_default_constructor::ManualDefaultConstructorDetector;
    for name in [
        "CompactStoreBuilder",
        "ZoneMap",
        "ProjectionSpec",
        "Statistics",
        "RdfStatistics",
        "RdfStatisticsCollector",
        "EmbeddingOptions",
    ] {
        clean(
            &d,
            &format!(
                "#[derive(Default)] struct {name} {{ values: Vec<u8> }} impl {name} {{ fn new() -> Self {{ Self::default() }} }}"
            ),
        );
    }
    clean(
        &d,
        "struct Bag { values: Vec<u8> } impl Bag { fn new() -> Self { Self { values: Vec::new() } } } impl Default for Bag { fn default() -> Self { Self::new() } }",
    );
    clean(
        &d,
        "struct Bag { values: Vec<u8> } impl Bag { const fn new() -> Self { Self { values: Vec::new() } } }",
    );
    clean(
        &d,
        "struct Vec; struct Bag { values: Vec } impl Bag { fn new() -> Self { Self { values: Vec::new() } } }",
    );
    confidence(
        &d,
        "use std::vec::Vec as List; struct Bag { values: List<u8> } impl Bag { fn new() -> Self { Self { values: List::new() } } }",
        Confidence::High,
    );
}

#[test]
fn all_audited_eq_suggestions_respect_field_requirements() {
    let d = i::derivable_impl::DerivableImplDetector;
    for (name, fields) in [
        ("OrderedFloat64", "f64"),
        ("HashableValue", "Value"),
        ("HeapEntry", "Vec<Value>, Vec<SortKey>"),
        ("Neighbor", "u64, f32"),
        ("FurthestCandidate", "u64, f32"),
    ] {
        clean(
            &d,
            &format!("struct {name}({fields}); impl Eq for {name} {{}}"),
        );
    }
    clean(
        &d,
        "type Scalar = f64; struct Key(Scalar); impl Eq for Key {}",
    );
    clean(&d, "struct String; struct Key(String); impl Eq for Key {}");
    clean(&d, "struct Key<T>(T); impl<T> Eq for Key<T> {}");
    confidence(
        &d,
        "type Id = u64; struct Key(Vec<Id>, String, [u8; 4]); impl Eq for Key {}",
        Confidence::High,
    );
}

#[test]
fn drop_requires_blocking_evidence_and_executed_work() {
    let d = c::sync_drop_blocking::SyncDropBlockingDetector;
    clean(
        &d,
        "type Lock = std::sync::Mutex<u8>; struct D { lock: Lock } impl Drop for D { fn drop(&mut self) { self.lock.lock(); } }",
    );
    for primitive in [
        "std::sync::Mutex",
        "parking_lot::Mutex",
        "tokio::sync::Mutex",
    ] {
        clean(
            &d,
            &format!(
                "struct Owned {{ lock: {primitive}<u32> }} impl Drop for Owned {{ fn drop(&mut self) {{ self.lock.lock(); }} }}"
            ),
        );
    }
    clean(
        &d,
        "struct D; impl Drop for D { fn drop(&mut self) { let later = async { std::thread::park(); }; let callback = || std::thread::park(); fn nested() { std::thread::park(); } } }",
    );
    clean(
        &d,
        "fn sleep() {} struct D; impl Drop for D { fn drop(&mut self) { sleep(); } }",
    );
    confidence(
        &d,
        "struct D; impl Drop for D { fn drop(&mut self) { self.flush(); } }",
        Confidence::Low,
    );
    confidence(
        &d,
        "use std::thread::park as pause; struct D; impl Drop for D { fn drop(&mut self) { pause(); } }",
        Confidence::High,
    );
    let found = detect(
        &d,
        "struct D { file: std::fs::File } impl Drop for D { fn drop(&mut self) { self.file.sync_all(); } }",
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].confidence, Confidence::High);
    assert_eq!(found[0].severity, Severity::Warning);
}

#[test]
fn regex_reuse_and_collection_lifetime_control_confidence() {
    let d = i::repeated_regex_construction::RepeatedRegexConstructionDetector;
    confidence(
        &d,
        "use regex::Regex as R; fn f() { R::new(\"abc\"); }",
        Confidence::High,
    );
    confidence(
        &d,
        "use regex::Regex; fn f(pattern: &str) { Regex::new(pattern); }",
        Confidence::Low,
    );
    clean(&d, "struct Regex; fn f() { Regex::new(\"abc\"); }");
    clean(
        &d,
        "use regex::Regex; use std::sync::LazyLock; static R: LazyLock<Regex> = LazyLock::new(|| Regex::new(\"abc\").unwrap());",
    );
    let d = i::vec_contains_in_loop::VecContainsInLoopDetector;
    confidence(
        &d,
        "fn f(values: Vec<u8>) { for x in 0..10 { values.contains(&x); } }",
        Confidence::High,
    );
    confidence(
        &d,
        "fn values() -> Vec<u8> { vec![] } fn f() { for x in 0..10 { let fresh = values(); { fresh.contains(&x); } } }",
        Confidence::Low,
    );
    confidence(
        &d,
        "fn f(v: Vec<u8>) { for x in 0..10 { let v: Vec<u8> = vec![]; v.contains(&x); } }",
        Confidence::Low,
    );
}

#[test]
fn unwraps_distinguish_infallible_conversions_from_unknown_and_fallible_calls() {
    let d = i::excessive_unwrap::ExcessiveUnwrapDetector;
    clean(
        &d,
        "fn f(data: &[u8]) { if data.len() < 16 { return; } u32::from_le_bytes(data[0..4].try_into().unwrap()); u32::from_le_bytes(data[4..8].try_into().unwrap()); u32::from_le_bytes(data[8..12].try_into().unwrap()); u32::from_le_bytes(data[12..16].try_into().unwrap()); }",
    );
    clean(
        &d,
        "use std::ffi::CString as C; fn f() { C::new(\"nodes\").expect(\"literal\"); C::new(\"edges\").unwrap(); C::new(\"value\").unwrap(); C::new(\"count\").unwrap(); }",
    );
    clean(
        &d,
        "fn f() { let future = async { a.unwrap(); b.unwrap(); c.unwrap(); d.unwrap(); }; }",
    );
    confidence(
        &d,
        "fn f() { a.unwrap(); b.unwrap(); c.unwrap(); d.unwrap(); }",
        Confidence::Low,
    );
    confidence(
        &d,
        "fn f() { std::fs::read(\"a\").unwrap(); std::fs::read(\"b\").unwrap(); std::fs::read(\"c\").unwrap(); std::fs::read(\"d\").unwrap(); }",
        Confidence::High,
    );
}

#[test]
fn intentional_results_remain_visible_without_claiming_unhandled_application_errors() {
    let d = i::unused_result::UnusedResultDetector;
    confidence(
        &d,
        "fn f() {\n// Best-effort cleanup; non-fatal.\nlet _ = std::fs::remove_file(\"stale\");\n}",
        Confidence::Low,
    );
    confidence(
        &d,
        "/// Swallows errors on a closed pipe.\nfn print() { let _ = std::fs::write(\"output\", b\"x\"); }",
        Confidence::Low,
    );
    confidence(
        &d,
        "fn f() { let message = \"best-effort\"; let _ = std::fs::write(\"output\", b\"x\"); }",
        Confidence::High,
    );
    confidence(
        &d,
        "/// Best-effort outer operation.\nfn f() { let later = async { let _ = std::fs::write(\"output\", b\"x\"); }; }",
        Confidence::High,
    );
    confidence(
        &d,
        "struct D; impl Drop for D { fn drop(&mut self) { let _ = std::fs::write(\"data\", b\"value\"); } }",
        Confidence::High,
    );
}

#[test]
fn audited_safe_fixture_does_not_fail_default_ci_and_modes_preserve_uncertainty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.rs");
    fs::write(&path, "struct D;\n// SAFETY: no state.\nunsafe impl Send for D {}\nunsafe impl Sync for D {}\nimpl Drop for D { fn drop(&mut self) { self.flush(); } }\n").unwrap();
    for mode in ["conservative", "balanced", "exploratory"] {
        let out = Command::new(env!("CARGO_BIN_EXE_qualirs"))
            .args(["--precision", mode, "--format", "json"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
        let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let matches: Vec<_> = report["smells"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["code"] == "Q0082")
            .collect();
        assert_eq!(matches.len(), usize::from(mode == "exploratory"));
    }
    fs::write(
        &path,
        "fn f(p: *const u8) {\n// qualirs:ignore Q0087\nunsafe { *p };\n}",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_qualirs"))
        .args(["--format", "json"])
        .arg(&path)
        .output()
        .unwrap();
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        report["smells"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["code"] != "Q0087")
    );
}

#[test]
fn eq_transformations_are_checked_by_the_compiler() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("check.rs");
    let binary = dir
        .path()
        .join(format!("check{}", std::env::consts::EXE_SUFFIX));
    let source = "#[derive(PartialEq)] struct Key(Vec<u32>); impl Eq for Key {} fn main() { assert!(Key(vec![1]) == Key(vec![1])); }";
    for source in [
        source.to_string(),
        source
            .replace("#[derive(PartialEq)]", "#[derive(PartialEq, Eq)]")
            .replace("impl Eq for Key {}", ""),
    ] {
        fs::write(&path, source).unwrap();
        let out = Command::new("rustc")
            .args(["--edition", "2024"])
            .arg(&path)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(Command::new(&binary).status().unwrap().success());
    }
    fs::write(
        &path,
        "#[derive(PartialEq, Eq)] struct Key(f64); fn main() {}",
    )
    .unwrap();
    let out = Command::new("rustc")
        .args(["--edition", "2024"])
        .arg(&path)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("E0277"));
}
