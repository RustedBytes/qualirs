use std::collections::HashSet;
use std::path::Path;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

use super::shared::{find_upwards, is_entry_file};

/// Detects duplicate crate versions in Cargo.lock.
pub struct DuplicateDependencyVersionsDetector;

impl Detector for DuplicateDependencyVersionsDetector {
    fn name(&self) -> &str {
        "Duplicate Dependency Versions"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        if !is_primary_entry_file(&file.path) {
            return Vec::new();
        }
        let Some(manifest) = find_upwards(&file.path, "Cargo.toml") else {
            return Vec::new();
        };
        let duplicates = duplicate_packages(&manifest);
        duplicates
            .into_iter()
            .map(|name| {
                Smell::new(
                    SmellCategory::Architecture,
                    "Duplicate Dependency Versions",
                    Severity::Info,
                    crate::domain::smell::FindingConfidence::Medium,
                    SourceLocation::new(file.path.clone(), 1, 1, None),
                    format!("Cargo.lock contains multiple versions of `{name}`"),
                    "Align dependency version requirements to reduce duplicate builds.",
                )
            })
            .collect()
    }
}

fn is_primary_entry_file(path: &Path) -> bool {
    if !is_entry_file(path) {
        return false;
    }

    let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };

    file_name == "lib.rs"
        || path
            .parent()
            .map(|parent| !parent.join("lib.rs").exists())
            .unwrap_or(true)
}

fn duplicate_packages(manifest: &Path) -> Vec<String> {
    let Some(project_dir) = manifest.parent() else {
        return Vec::new();
    };
    let Ok(output) = std::process::Command::new("cargo")
        .args([
            "tree",
            "-d",
            "--locked",
            "--prefix",
            "none",
            "--manifest-path",
        ])
        .arg(manifest)
        .current_dir(project_dir)
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    duplicate_packages_from_cargo_tree(&stdout)
}

fn duplicate_packages_from_cargo_tree(output: &str) -> Vec<String> {
    let mut versions = PackageVersions::default();

    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else {
            continue;
        };
        let Some(version) = parts.next() else {
            continue;
        };
        if let Some(version) = version.strip_prefix('v') {
            versions.insert(name, version);
        }
    }

    versions
        .duplicate_names()
        .into_iter()
        .map(str::to_string)
        .collect()
}

#[derive(Default)]
struct PackageVersions<'a>(Vec<PackageVersionSet<'a>>);

struct PackageVersionSet<'a> {
    name: &'a str,
    versions: HashSet<&'a str>,
}

impl<'a> PackageVersions<'a> {
    fn insert(&mut self, name: &'a str, version: &'a str) {
        if let Some(package) = self.0.iter_mut().find(|package| package.name == name) {
            package.versions.insert(version);
        } else {
            self.0.push(PackageVersionSet::new(name, version));
        }
    }

    fn duplicate_names(self) -> Vec<&'a str> {
        self.0
            .into_iter()
            .filter_map(|package| (package.versions.len() > 1).then_some(package.name))
            .collect()
    }
}

impl<'a> PackageVersionSet<'a> {
    fn new(name: &'a str, version: &'a str) -> Self {
        let mut versions = HashSet::new();
        versions.insert(version);
        Self { name, versions }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_tree_duplicates_require_multiple_versions() {
        let output = "\
hashbrown v0.15.5
some-crate v1.0.0
hashbrown v0.17.0
other-crate v2.0.0
";
        assert_eq!(
            duplicate_packages_from_cargo_tree(output),
            vec!["hashbrown".to_string()]
        );
    }

    #[test]
    fn cargo_tree_repeated_same_version_is_not_duplicate() {
        let output = "\
proc-macro2 v1.0.106
quote v1.0.45
proc-macro2 v1.0.106
syn v2.0.117
syn v2.0.117
";
        assert!(duplicate_packages_from_cargo_tree(output).is_empty());
    }
}
