use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use qualirs::analysis::engine::Engine;
use qualirs::domain::config::{Config, PolicyConfig, Precision};
use qualirs::domain::smell::{RULES, rule_code_for};

const CASES_DIR: &str = "tests/cases";
const FIXTURE_MANIFEST_FILE: &str = "Cargo.fixture.toml";
const EXPECT_PREFIX: &str = "// EXPECT: ";

#[test]
fn showcase_cases_emit_expected_findings() {
    let mut showcased_rule_codes = BTreeSet::new();

    for case_dir in case_dirs() {
        let source_path = case_dir.join("src/lib.rs");
        let expected = expected_findings(&source_path);
        assert!(
            !expected.is_empty(),
            "{} should declare at least one `// EXPECT: ...` marker",
            source_path.display()
        );

        let temp_dir = tempfile::tempdir().expect("create case temp dir");
        let analysis_root = temp_dir.path().join(
            case_dir
                .file_name()
                .expect("case path should have a folder name"),
        );
        copy_dir(&case_dir, &analysis_root);

        let mut engine = Engine::new(Config {
            precision: Precision::Exploratory,
            policy: PolicyConfig {
                skip_tests: false,
                ..PolicyConfig::default()
            },
            ..Config::default()
        });
        engine.register_defaults();

        let report = engine.analyze(&analysis_root);
        assert!(
            report.parse_errors.is_empty(),
            "{} should parse cleanly: {:?}",
            case_dir.display(),
            report.parse_errors
        );

        let actual = report
            .smells
            .iter()
            .filter_map(|smell| rule_code_for(&smell.name).map(|code| (code, smell.name.as_str())))
            .collect::<BTreeSet<_>>();

        for expected_name in expected {
            let expected_code = rule_code_for(&expected_name)
                .unwrap_or_else(|| panic!("`{expected_name}` should map to a known rule code"));
            showcased_rule_codes.insert(expected_code);
            assert!(
                actual.iter().any(|(code, _)| *code == expected_code),
                "{} should emit `{}` ({expected_code}). Actual findings: {:?}",
                case_dir.display(),
                expected_name,
                actual
            );
        }
    }

    let documented_rule_codes = RULES.iter().map(|rule| rule.code).collect::<BTreeSet<_>>();
    assert_eq!(
        showcased_rule_codes, documented_rule_codes,
        "showcase fixtures should cover every documented detector rule"
    );
}

fn case_dirs() -> Vec<PathBuf> {
    let mut dirs = fs::read_dir(CASES_DIR)
        .expect("read tests/cases")
        .map(|entry| entry.expect("read case entry").path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    dirs.sort();
    dirs
}

fn expected_findings(source_path: &Path) -> Vec<String> {
    fs::read_to_string(source_path)
        .unwrap_or_else(|err| panic!("read {}: {err}", source_path.display()))
        .lines()
        .filter_map(|line| line.strip_prefix(EXPECT_PREFIX))
        .map(str::trim)
        .map(str::to_string)
        .collect()
}

fn copy_dir(source: &Path, destination: &Path) {
    fs::create_dir_all(destination)
        .unwrap_or_else(|err| panic!("create {}: {err}", destination.display()));

    for entry in
        fs::read_dir(source).unwrap_or_else(|err| panic!("read {}: {err}", source.display()))
    {
        let entry = entry.expect("read directory entry");
        let entry_source = entry.path();
        let file_name = entry.file_name();
        if file_name == "target" {
            continue;
        }
        let entry_destination = if file_name == FIXTURE_MANIFEST_FILE {
            destination.join("Cargo.toml")
        } else {
            destination.join(file_name)
        };

        if entry_source.is_dir() {
            copy_dir(&entry_source, &entry_destination);
        } else {
            fs::copy(&entry_source, &entry_destination).unwrap_or_else(|err| {
                panic!(
                    "copy {} to {}: {err}",
                    entry_source.display(),
                    entry_destination.display()
                )
            });
        }
    }
}
