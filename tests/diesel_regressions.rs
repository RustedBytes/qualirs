//! Regressions reduced from the Diesel audit.
use qualirs::{
    analysis::detector::Detector,
    detectors::{
        architecture::project_hygiene::TestOnlyDependencyInProductionDetector, implementation as i,
        r#unsafe as u,
    },
    domain::{
        smell::{FindingConfidence as Confidence, Smell},
        source::SourceFile,
    },
};
use std::{fs, process::Command};

fn detect(d: &dyn Detector, code: &str) -> Vec<Smell> {
    d.detect(&SourceFile::from_source("src/lib.rs".into(), code.into()).unwrap())
}
fn clean(d: &dyn Detector, code: &str) {
    assert!(detect(d, code).is_empty(), "{code}\n{:?}", detect(d, code));
}

#[test]
fn diesel_send_sync_explanations_do_not_require_a_safety_label() {
    // Seven audited impls: MysqlLikeConnection, mysql Statement, PgConnection,
    // ConnectionManager, SqliteConnection, OwnedSqliteValue, sqlite Statement.
    for (name, note, trait_name) in [
        (
            "MysqlLikeConnection",
            "// mysql connection can be shared between threads according to libmysqlclients documentation\n#[allow(unsafe_code)]",
            "Send",
        ),
        (
            "MysqlStatement",
            "// mysql connection can be shared between threads according to libmysqlclients documentation\n#[allow(unsafe_code)]",
            "Send",
        ),
        (
            "PgConnection",
            "// according to libpq documentation a connection can be transferred to other threads\n#[allow(unsafe_code)]",
            "Send",
        ),
        (
            "ConnectionManager",
            "#[allow(unsafe_code)] // we do not actually hold a reference to T",
            "Sync",
        ),
        (
            "SqliteConnection",
            "// This relies on the invariant that RawConnection or Statement are never\n// leaked. If a reference to one of those was held on a different thread, this\n// would not be thread safe.\n#[allow(unsafe_code)]",
            "Send",
        ),
        (
            "OwnedSqliteValue",
            "// Unsafe Send impl safe since sqlite3_value is built with sqlite3_value_dup\n// see https://www.sqlite.org/c3ref/value.html",
            "Send",
        ),
        (
            "SqliteStatement",
            "// This relies on the invariant that RawConnection or Statement are never\n// leaked. If a reference to one of those was held on a different thread, this\n// would not be thread safe.\n#[allow(unsafe_code)]",
            "Send",
        ),
    ] {
        let code = format!("struct {name};\n{note}\nunsafe impl {trait_name} for {name} {{}}");
        clean(
            &u::unsafe_without_comment::UnsafeWithoutCommentDetector,
            &code,
        );
        clean(
            &u::unsafe_impl_safety_docs::UnsafeImplSafetyDocsDetector,
            &code,
        );
    }
}

#[test]
fn natural_explanations_stay_attached_to_their_syntax() {
    let d = u::unsafe_without_comment::UnsafeWithoutCommentDetector;
    clean(
        &d,
        "fn populate() {\n// This is safe because we are re-binding the invalidated buffers\n// at the end of this function\nunsafe { fetch_column(); }\n}",
    );
    clean(
        &d,
        "fn input_bind() { with_binds(|ptr| {\n// This relies on the invariant that the current value of self.input_binds\n// will not change without this function being called\nunsafe { bind_param(ptr); }\n}); }",
    );
    for note in [
        "// Safe because the pointer remains valid for this call.",
        "/* Safe since the allocation outlives this access. */",
        "#[allow(unsafe_code)] // Safe because p is valid.",
        "#[allow(unsafe_code)] /* Safe because p is valid. */",
    ] {
        clean(
            &d,
            &format!("fn f(p: *const u8) {{\n{note}\nlet x = unsafe {{ *p }};\n}}"),
        );
    }
    for code in [
        "fn f(p: *const u8) { ordinary(); // Safe because the pointer is valid.\nunsafe { *p }; }",
        "fn f(p: *const u8) { let s = \"safe because pointer is valid\"; unsafe { *p }; }",
        "fn f(p: *const u8) {\n// Safe because this closure is never executed.\nlet work = || unsafe { *p };\n}",
        "fn f(p: *const u8) {\n// Safe because constructing a future executes no work.\nlet work = async { unsafe { *p }; };\n}",
        "fn f(p: *const u8) {\n// Safe because first access is valid.\nunsafe { *p }; unrelated(); unsafe { *p };\n}",
        "struct A; struct B;\n// Safe since A has no state.\nunsafe impl Send for A {}\nunsafe impl Send for B {}",
        "struct A;\n// TODO: review thread handling.\nunsafe impl Send for A {}",
        "struct A;\n#[allow(unsafe_code)] // use of unsafe\nunsafe impl Send for A {}",
    ] {
        assert!(!detect(&d, code).is_empty(), "{code}");
    }
    let found = detect(
        &u::unsafe_impl_safety_docs::UnsafeImplSafetyDocsDetector,
        "struct A;\n// TODO: review thread handling.\nunsafe impl Send for A {}",
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].confidence, Confidence::High);
    assert_eq!(found[0].location.line_start, 3);
}

#[test]
fn file_locks_and_custom_names_are_not_standard_memory_locks() {
    let d = i::local_lock_in_single_threaded_scope::LocalLockInSingleThreadedScopeDetector;
    for path in ["fd_lock::RwLock", "tokio::sync::RwLock", "custom::RwLock"] {
        clean(
            &d,
            &format!(
                "use {path}; fn f() {{ let mut lock = RwLock::new(open_file()?); let _ = lock.write()?; }}"
            ),
        );
    }
    clean(
        &d,
        "fn f() { let lock = fd_lock::RwLock::new(file); lock.write(); }",
    );
    clean(
        &d,
        "struct Mutex; fn f() { let lock = Mutex::new(0); lock.lock(); }",
    );
    clean(&d, "fn f() { let lock = Mutex::new(0); lock.lock(); }");
    clean(
        &d,
        "mod std { pub mod sync { pub struct Mutex; } } fn f() { let lock = std::sync::Mutex::new(0); lock.lock(); }",
    );
    for code in [
        "use std::sync::Mutex; fn f() { let lock = Mutex::new(0); lock.lock(); }",
        "use std::sync::Mutex as Gate; fn f() { let lock = Gate::new(0); lock.lock(); }",
        "fn f() { let lock = std::sync::RwLock::new(0); lock.write(); }",
    ] {
        let found = detect(&d, code);
        assert_eq!(found.len(), 1, "{code}");
        assert_eq!(found[0].confidence, Confidence::High);
    }
}

#[test]
fn lock_usage_accounts_for_later_sharing_shadowing_and_execution_scopes() {
    let d = i::local_lock_in_single_threaded_scope::LocalLockInSingleThreadedScopeDetector;
    for later in [
        "let shared = std::sync::Arc::new(lock);",
        "send(&lock);",
        "store(lock);",
        "let borrowed = &lock;",
        "let work = || { lock.lock(); };",
        "let work = async { lock.lock(); };",
        "observe!(lock);",
        "lock.get_mut();",
    ] {
        clean(
            &d,
            &format!(
                "use std::sync::Mutex; fn f() {{ let lock = Mutex::new(0); lock.lock(); {later} }}"
            ),
        );
    }
    clean(
        &d,
        "use std::sync::Mutex; fn f() { let lock = Mutex::new(0); let lock = other(); lock.lock(); }",
    );
    clean(
        &d,
        "use std::sync::Mutex; fn f() { let lock = Mutex::new(0); self.lock.lock(); }",
    );
    clean(
        &d,
        "use std::sync::Mutex; fn f() { let lock = Mutex::new(0); fn g(lock: Other) { lock.lock(); } }",
    );
    let source = "use std::sync::Mutex;\nfn f() { let lock = Mutex::new(0); lock.lock(); }\nfn g() { let lock = Mutex::new(0); lock.lock(); }";
    let found = detect(&d, source);
    assert_eq!(found.len(), 2);
    assert_eq!(
        found
            .iter()
            .map(|f| f.location.line_start)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
    let source = "use std::sync::Mutex; fn f() { let lock = Mutex::new(0); { let lock = other(); consume(lock); } lock.lock(); }";
    assert_eq!(detect(&d, source).len(), 1);
}

#[test]
fn normal_optional_target_and_renamed_dependencies_are_not_dev_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("src/lib.rs");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    for declarations in [
        "[dependencies]\nquickcheck = { version = \"1\", optional = true }\n[dev-dependencies]\nquickcheck = \"1\"",
        "[dependencies.quickcheck]\nversion = \"1\"\noptional = true\n[dev-dependencies.quickcheck]\nversion = \"1\"",
        "[target.'cfg(unix)'.dependencies]\nquickcheck = \"1\"\n[dev-dependencies]\nquickcheck = \"1\"",
        "[dependencies]\nquickcheck = { workspace = true }\n[dev-dependencies]\nquickcheck = { workspace = true }",
        "[dependencies]\nquickcheck = { package = \"actual-package\", version = \"1\" }\n[dev-dependencies]\nquickcheck = { package = \"actual-package\", version = \"1\" }",
    ] {
        fs::write(dir.path().join("Cargo.toml"), declarations).unwrap();
        let file =
            SourceFile::from_source(path.clone(), "use quickcheck::Arbitrary;".into()).unwrap();
        assert!(
            TestOnlyDependencyInProductionDetector
                .detect(&file)
                .is_empty(),
            "{declarations}"
        );
    }
    fs::write(
        dir.path().join("Cargo.toml"),
        "[dev-dependencies]\nquickcheck = \"1\"\n",
    )
    .unwrap();
    let file = SourceFile::from_source(path.clone(), "use quickcheck::Arbitrary;".into()).unwrap();
    let found = TestOnlyDependencyInProductionDetector.detect(&file);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].confidence, Confidence::Medium);
    let file = SourceFile::from_source(
        dir.path().join("src/doc_helper.rs"),
        "use quickcheck::Arbitrary;".into(),
    )
    .unwrap();
    let found = TestOnlyDependencyInProductionDetector.detect(&file);
    assert_eq!(found[0].confidence, Confidence::Low);
    assert!(found[0].message.contains("build context is unresolved"));
}

#[test]
fn audited_fixture_is_safe_for_default_ci_and_respects_ignores_and_test_settings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.rs");
    fs::write(&path, "struct A;\n#[allow(unsafe_code)] // we do not hold a reference to T\nunsafe impl Sync for A {}\nuse fd_lock::RwLock;\nfn f() { let lock = RwLock::new(file); lock.write(); }\n").unwrap();
    for mode in ["conservative", "balanced", "exploratory"] {
        let out = Command::new(env!("CARGO_BIN_EXE_qualirs"))
            .arg(&path)
            .args(["--format", "json", "--precision", mode])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
        let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(
            report["smells"]
                .as_array()
                .unwrap()
                .iter()
                .all(|s| !["Q0087", "Q0094", "Q0063"].contains(&s["code"].as_str().unwrap()))
        );
    }
    let config = dir.path().join("qualirs.toml");
    fs::write(
        &path,
        "struct A;\n// qualirs:ignore Q0094\nunsafe impl Send for A {}\n",
    )
    .unwrap();
    fs::write(&config, "ignore_findings = [\"Q0087\"]\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_qualirs"))
        .arg(&path)
        .arg("--config")
        .arg(&config)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["summary"]["findings"], 0);
    fs::write(
        &path,
        "#[cfg(test)] fn f() { let lock = std::sync::Mutex::new(0); lock.lock(); }",
    )
    .unwrap();
    for skip in [true, false] {
        fs::write(&config, format!("[policy]\nskip_tests = {skip}\n")).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_qualirs"))
            .arg(&path)
            .arg("--config")
            .arg(&config)
            .args(["--format", "json"])
            .output()
            .unwrap();
        let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(
            report["smells"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|s| s["code"] == "Q0063")
                .count(),
            usize::from(!skip)
        );
    }
}

#[test]
fn supported_local_mutex_simplification_preserves_results() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("check.rs");
    let binary = dir
        .path()
        .join(format!("check{}", std::env::consts::EXE_SUFFIX));
    fs::write(&path, "fn original(x: i32) -> i32 { let lock = std::sync::Mutex::new(x); *lock.lock().unwrap() += 1; let result = *lock.lock().unwrap(); result }\nfn simplified(mut x: i32) -> i32 { x += 1; x }\nfn main() { for x in [-1, 0, 42] { assert_eq!(original(x), simplified(x)); } }\n").unwrap();
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
