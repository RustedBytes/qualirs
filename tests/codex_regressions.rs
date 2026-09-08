//! Source reductions from the Codex checkout audit.
use qualirs::{
    analysis::detector::Detector,
    detectors::{implementation as i, r#unsafe as unsafe_detectors},
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
fn codex_async_and_fallible_match_arms_cannot_become_sync_map_closures() {
    let d = i::manual_option_result_mapping::ManualOptionResultMappingDetector;
    // apps_processor, thread_processor, debug_sandbox, guardian/review, session,
    // goal/api (two sites), skills/loader, proxy, state/projects, app_server_events.
    for value in [
        "thread.config_snapshot().await.cwd().to_path_buf()",
        "self.paginated_resume_initial_turns_page(id, params).await?",
        "spec.start_proxy().await.map_err(convert)?",
        "network_proxy.proxy().current_cfg().await?",
        "(sender, self.reserve_user_input_order().await)",
        "runtime.goal_state_permit().await.map_err(convert)?",
        "canonicalize_for_skill_identity(file_system, plugin_root).await",
        "task.await",
        "project_from_row_in_tx(&mut tx, &row).await?",
        "channel.store.lock().await",
        // tooltips: two date fields and the optional version regex.
        "NaiveDate::parse_from_str(&date, \"%Y-%m-%d\").ok()?",
        "Regex::new(&pattern).ok()?",
    ] {
        clean(
            &d,
            &format!(
                "async fn f(input: Option<Input>) {{ match input {{ Some(value) => Some({value}), None => None }}; }}"
            ),
        );
    }
    for value in [
        "{ return None; 1 }",
        "{ break 'outer; 1 }",
        "{ continue; 1 }",
        "maybe_return!(value)",
    ] {
        clean(
            &d,
            &format!(
                "fn f(input: Option<i32>) {{ 'outer: loop {{ match input {{ Some(value) => Some({value}), None => None }}; }} }}"
            ),
        );
    }
    // Awaiting the scrutinee does not move the await into the proposed closure.
    let findings = detect(
        &d,
        "async fn f() { match next().await { Some(x) => Some(x + 1), None => None }; }",
    );
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].confidence, Confidence::High);
}

#[test]
fn mapping_respects_const_contexts_patterns_guards_and_custom_variants() {
    let d = i::manual_option_result_mapping::ManualOptionResultMappingDetector;
    let mapping = "match value { Some(value) => Some(Self(value)), None => None }";
    clean(
        &d,
        &format!(
            "struct Version(u32); impl Version {{ pub const fn new(value: Option<u32>) -> Option<Self> {{ {mapping} }} }}"
        ),
    );
    for prefix in [
        "const X: Option<i32> =",
        "static X: Option<i32> =",
        "fn f() { let _ = const",
    ] {
        let tail = if prefix.ends_with("const") {
            "}; }"
        } else {
            "};"
        };
        clean(
            &d,
            &format!("{prefix} {{ match Some(1) {{ Some(x) => Some(x + 1), None => None }} {tail}"),
        );
    }
    clean(
        &d,
        "struct D; impl D { const X: Option<i32> = match Some(1) { Some(x) => Some(x+1), None => None }; }",
    );
    clean(
        &d,
        "trait D { const X: Option<i32> = match Some(1) { Some(x) => Some(x+1), None => None }; }",
    );
    for pattern in [
        "Some(x @ State::Only)",
        "Some(1)",
        "Some(ref x)",
        "Some(x) if ready()",
    ] {
        clean(
            &d,
            &format!(
                "fn f(input: Option<i32>) {{ match input {{ {pattern} => Some(1), None => None }}; }}"
            ),
        );
    }
    clean(
        &d,
        "enum Custom { Some(i32), None } use Custom::{Some, None}; fn f(x: Custom) { match x { Some(x) => Some(x+1), None => None }; }",
    );
    clean(
        &d,
        "fn f(x: Custom) { match x { custom::Some(x) => custom::Some(x+1), custom::None => custom::None }; }",
    );
    let source = "use std::option::Option::{Some as Present, None as Absent}; fn f(x: Option<i32>) { match x { Present(x) => Present(x+1), Absent => Absent }; }";
    assert_eq!(detect(&d, source).len(), 1);
    let source = "const fn outer() { fn inner(x: Option<i32>) { match x { Some(x) => Some(x+1), None => None }; } }";
    assert_eq!(detect(&d, source).len(), 1);
    let found = detect(
        &d,
        "fn f(x: Result<i32,i32>) { match x { Ok(x) => Ok(x+1), Err(e) => Err(e+1) }; }",
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].confidence, Confidence::Low);
}

#[test]
fn constructors_need_executed_loop_and_resolved_constant_input() {
    let d = i::repeated_expensive_construction::RepeatedExpensiveConstructionDetector;
    clean(
        &d,
        "use std::path::PathBuf; fn f() { for p in [PathBuf::from(\"C:/\"), PathBuf::from(\"C:/Windows\")] { consume(p); } }",
    );
    for body in [
        "let work = || url::Url::parse(\"https://example.com\");",
        "let work = async { url::Url::parse(\"https://example.com\") };",
        "fn later() { url::Url::parse(\"https://example.com\"); }",
        "const X: () = { PathBuf::from(\"root\"); };",
        "url::Url::parse(next());",
        "url::Url::parse(input); input = next();",
        "let input = next(); url::Url::parse(input);",
    ] {
        clean(
            &d,
            &format!("fn f(mut input: &str) {{ for _ in 0..10 {{ {body} }} }}"),
        );
    }
    clean(
        &d,
        "struct Url; impl Url { fn parse(_: &str) {} } fn f() { loop { Url::parse(\"abc\"); } }",
    );
    clean(
        &d,
        "mod url { pub struct Url; } fn f() { loop { url::Url::parse(\"abc\"); } }",
    );
    for body in [
        "for _ in 0..10 { url::Url::parse(\"https://example.com\"); }",
        "for _ in 0..10 { for x in [url::Url::parse(\"https://example.com\")] {} }",
        "while url::Url::parse(\"https://example.com\").is_ok() {}",
        "loop { use url::Url as Address; Address::parse(\"https://example.com\"); }",
    ] {
        let found = detect(&d, &format!("fn f() {{ {body} }}"));
        assert_eq!(found.len(), 1, "{body}");
        assert_eq!(found[0].confidence, Confidence::High);
    }
    // plugin_cmd owns each path in a separate issue; hoisting alone would move it.
    let found = detect(
        &d,
        "use std::path::PathBuf; fn f() { for item in items { issues.push(Issue { path: PathBuf::from(\"<invalid config>\") }); } }",
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].confidence, Confidence::Low);
}

#[test]
fn ffi_wrapper_evidence_comes_from_calls_in_safe_functions_and_methods() {
    let d = unsafe_detectors::ffi_without_wrapper::FfiWithoutWrapperDetector;
    // Every audited Q0091 declaration has a safe caller, often with a different name.
    for name in [
        "proc_listchildpids",
        "NtCreateFile",
        "CFNetworkCopyProxiesForURL",
        "CFNetworkExecuteProxyAutoConfigurationURL",
        "CFNetworkExecuteProxyAutoConfigurationScript",
        "posix_spawn_file_actions_addchdir_np",
        "NtResumeProcess",
        "free_library",
        "NtSetInformationFile",
    ] {
        for wrapper in [
            format!("fn safe_operation() {{ unsafe {{ {name}(); }} }}"),
            format!("struct D; impl D {{ fn operation(&self) {{ unsafe {{ {name}(); }} }} }}"),
            format!("fn safe_operation() {{ execute(|| unsafe {{ {name}(); }}); }}"),
            format!("fn safe_operation() {{ use self::{name} as raw; unsafe {{ raw(); }} }}"),
        ] {
            clean(
                &d,
                &format!("unsafe extern \"C\" {{ fn {name}(); }} {wrapper}"),
            );
        }
    }
    clean(
        &d,
        "mod inner { unsafe extern \"C\" { fn raw(); } fn wrap() { unsafe { raw(); } } }",
    );
    for code in [
        "unsafe extern \"C\" { fn raw(); } fn raw_wrapper() {}",
        "unsafe extern \"C\" { fn raw(); } unsafe fn wrapper() { raw(); }",
        "unsafe extern \"C\" { fn raw(); } fn wrapper(raw: fn()) { raw(); }",
        "unsafe extern \"C\" { fn raw(); } fn wrapper() { let raw = || {}; raw(); }",
        "unsafe extern \"C\" { fn raw(); } mod inner { fn wrapper() { raw(); } }",
        "unsafe extern \"C\" { fn raw(); } fn wrapper() { unsafe fn nested() { raw(); } }",
        "unsafe extern \"C\" { fn raw(); } #[cfg(test)] fn wrapper() { unsafe { raw(); } }",
    ] {
        let found = detect(&d, code);
        assert_eq!(found.len(), 1, "{code}");
        assert_eq!(found[0].confidence, Confidence::Low);
    }
}

#[test]
fn reporting_modes_locations_ignores_and_test_policy_preserve_corrected_behavior() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.rs");
    let source = "unsafe extern \"C\" {\n    fn raw();\n}\nfn f() {\n    for _ in 0..2 {\n        std::path::PathBuf::from(\"root\");\n    }\n}\nasync fn g(x: Option<u8>) { match x { Some(x) => Some(work(x).await), None => None }; }\n";
    fs::write(&path, source).unwrap();
    for mode in ["conservative", "balanced", "exploratory"] {
        let output = Command::new(env!("CARGO_BIN_EXE_qualirs"))
            .arg(&path)
            .args(["--precision", mode, "--format", "json"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let matches: Vec<_> = report["smells"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| ["Q0091", "Q0061", "Q0074"].contains(&f["code"].as_str().unwrap()))
            .collect();
        assert_eq!(matches.len(), if mode == "exploratory" { 2 } else { 0 });
        for finding in matches {
            assert_eq!(finding["confidence"], "low");
            assert_eq!(
                finding["location"]["line_start"],
                if finding["code"] == "Q0091" { 2 } else { 6 }
            );
        }
    }
    let config = dir.path().join("qualirs.toml");
    fs::write(
        &config,
        "precision = \"exploratory\"\nignore_findings = [\"Q0091\"]\n",
    )
    .unwrap();
    fs::write(
        &path,
        source.replace(
            "        std::",
            "        // qualirs:ignore Q0061\n        std::",
        ),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_qualirs"))
        .arg(&path)
        .arg("--config")
        .arg(&config)
        .args(["--format", "json"])
        .output()
        .unwrap();
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        report["smells"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["code"] != "Q0091" && f["code"] != "Q0061")
    );
    fs::write(
        &path,
        "#[cfg(test)] fn f(x: Option<i32>) { match x { Some(x) => Some(x+1), None => None }; }",
    )
    .unwrap();
    for skip in [true, false] {
        fs::write(&config, format!("[policy]\nskip_tests = {skip}\n")).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_qualirs"))
            .arg(&path)
            .arg("--config")
            .arg(&config)
            .args(["--format", "json"])
            .output()
            .unwrap();
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            report["smells"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|f| f["code"] == "Q0074")
                .count(),
            usize::from(!skip)
        );
    }
}

#[test]
fn supported_map_transformations_compile_and_preserve_values() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("mapping.rs");
    let binary = dir
        .path()
        .join(format!("mapping{}", std::env::consts::EXE_SUFFIX));
    fs::write(&source, r#"
fn original(x: Option<i32>) -> Option<i32> { match x { Some(x) => Some(x+1), None => None } }
fn mapped(x: Option<i32>) -> Option<i32> { x.map(|x| x+1) }
fn original_result(x: Result<i32,i32>) -> Result<i32,i32> { match x { Ok(x) => Ok(x+1), Err(e) => Err(e) } }
fn mapped_result(x: Result<i32,i32>) -> Result<i32,i32> { x.map(|x| x+1) }
fn main() {
    for x in [None, Some(-1), Some(42)] { assert_eq!(original(x), mapped(x)); }
    for x in [Ok(-1), Ok(42), Err(7)] { assert_eq!(original_result(x), mapped_result(x)); }
}
"#).unwrap();
    let out = Command::new("rustc")
        .args(["--edition", "2024"])
        .arg(&source)
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
