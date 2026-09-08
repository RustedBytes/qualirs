use std::path::Path;
use std::process::{Command, Output};

fn run(path: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_qualirs"))
        .args(args)
        .arg(path)
        .output()
        .expect("run qualirs")
}

fn source() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("input.rs");
    std::fs::write(&file, "fn identity(value: u32) -> u32 { value }\n").unwrap();
    (dir, file)
}

fn assert_footer(text: &str) {
    assert_eq!(text.matches("Analysis resources:").count(), 1);
    assert!(text.contains("Elapsed time:"));
    assert!(text.contains("CPU time (all process threads):"));
    assert!(text.contains("Peak memory (process lifetime):"));
    assert!(text.ends_with("CPU and memory exclude child processes.\n"));
    #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
    assert!(!text.contains("unavailable"));
}

#[test]
fn stats_are_opt_in_and_help_documents_the_flag() {
    let (_dir, path) = source();
    let output = run(&path, &[]);
    assert!(output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Analysis resources:"));
    assert!(output.stderr.is_empty());
    let help = Command::new(env!("CARGO_BIN_EXE_qualirs"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--stats"));
}

#[test]
fn stats_append_to_each_text_output_mode() {
    let (_dir, path) = source();
    for mode in [
        None,
        Some("--quiet"),
        Some("--compact"),
        Some("--table"),
        Some("--llm"),
        Some("--how-fix"),
    ] {
        let mut args = vec!["--stats"];
        if let Some(mode) = mode {
            args.push(mode);
        }
        let output = run(&path, &args);
        assert!(
            output.status.success(),
            "{mode:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert_footer(&stdout);
        assert!(stdout.find("Analysis resources:").unwrap() > 0);
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn json_remains_parseable_and_stats_go_to_stderr() {
    let (dir, path) = source();
    let output = run(&path, &["--stats", "--format", "json"]);
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["summary"]["findings"], 0);
    assert!(report.get("stats").is_none());
    assert_footer(&String::from_utf8(output.stderr).unwrap());

    let report_path = dir.path().join("report.json");
    let output = run(
        &path,
        &[
            "--stats",
            "--format",
            "json",
            "--output",
            report_path.to_str().unwrap(),
        ],
    );
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(report_path).unwrap()).unwrap();
    assert_eq!(saved, report);
    assert_footer(&String::from_utf8(output.stderr).unwrap());
}

#[test]
fn stats_preserve_critical_exit_status_and_work_for_empty_analysis() {
    let (dir, path) = source();
    std::fs::write(
        &path,
        "fn cast(x:u32) { unsafe { std::mem::transmute::<u32,f32>(x); } }\n",
    )
    .unwrap();
    let output = run(&path, &["--stats"]);
    assert_eq!(output.status.code(), Some(1));
    assert_footer(&String::from_utf8(output.stdout).unwrap());

    let empty = dir.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    let output = run(&empty, &["--stats", "--quiet"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Files: 0"));
    assert_footer(&stdout);
}
