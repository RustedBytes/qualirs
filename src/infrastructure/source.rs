use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};
use flate2::read::GzDecoder;
use tempfile::TempDir;

#[derive(Debug, Clone, Copy)]
pub enum GitReference<'a> {
    Branch(&'a str),
    Tag(&'a str),
}

impl GitReference<'_> {
    fn name(self) -> String {
        match self {
            Self::Branch(name) | Self::Tag(name) => name.to_owned(),
        }
    }

    fn kind(self) -> &'static str {
        match self {
            Self::Branch(_) => "branch",
            Self::Tag(_) => "tag",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SourceRequest<'a> {
    Local(&'a Path),
    Git {
        url: &'a str,
        reference: Option<GitReference<'a>>,
    },
    Crate {
        name: &'a str,
        version: Option<&'a str>,
    },
}

#[derive(Debug)]
pub struct PreparedSource {
    path: PathBuf,
    keep_temp: bool,
    _temp_dir: Option<TempDir>,
}

impl PreparedSource {
    pub fn local(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            keep_temp: false,
            _temp_dir: None,
        }
    }

    #[inline]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn preserved_path(&self) -> Option<&Path> {
        self.keep_temp.then_some(&*self.path)
    }
}

pub fn prepare_source(request: SourceRequest<'_>) -> anyhow::Result<PreparedSource> {
    prepare_source_with_options(request, None, false)
}

pub fn prepare_source_in(
    request: SourceRequest<'_>,
    temp_parent: Option<&Path>,
) -> anyhow::Result<PreparedSource> {
    prepare_source_with_options(request, temp_parent, false)
}

pub fn prepare_source_with_options(
    request: SourceRequest<'_>,
    temp_parent: Option<&Path>,
    keep_temp: bool,
) -> anyhow::Result<PreparedSource> {
    match request {
        SourceRequest::Local(path) => Ok(PreparedSource::local(path)),
        SourceRequest::Git { url, reference } => {
            clone_git_repository(url, reference, temp_parent, keep_temp)
        }
        SourceRequest::Crate { name, version } => {
            download_crate(name, version, temp_parent, keep_temp)
        }
    }
}

fn clone_git_repository(
    url: &str,
    reference: Option<GitReference<'_>>,
    temp_parent: Option<&Path>,
    keep_temp: bool,
) -> anyhow::Result<PreparedSource> {
    let temp_dir = create_temp_dir("qualirs-git-", temp_parent, keep_temp)
        .context("create temporary directory for git clone")?;
    let checkout_dir = temp_dir.path().join("repo");

    let mut command = Command::new("git");
    command.args(["clone", "--depth", "1", "--single-branch"]);

    let reference_name = reference.map(GitReference::name);
    if let Some(reference_name) = &reference_name {
        command.args(["--branch", reference_name]);
    }

    command.arg(url).arg(&checkout_dir);

    let output = command.output().context("run git clone")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reference_context = reference
            .map(|reference| format!(" {} `{}`", reference.kind(), reference.name()))
            .unwrap_or_default();
        bail!("failed to clone git repository{reference_context}: {stderr}");
    }

    Ok(PreparedSource {
        path: checkout_dir,
        keep_temp,
        _temp_dir: Some(temp_dir),
    })
}

fn download_crate(
    name: &str,
    version: Option<&str>,
    temp_parent: Option<&Path>,
    keep_temp: bool,
) -> anyhow::Result<PreparedSource> {
    validate_crate_name(name)?;

    let temp_dir = create_temp_dir("qualirs-crate-", temp_parent, keep_temp)
        .context("create temporary directory for crate download")?;
    let unpack_dir = temp_dir.path().join("crate");
    fs::create_dir(&unpack_dir).context("create crate unpack directory")?;

    let version = resolve_crate_version(name, version)?;
    let url = crates_io_download_url(name, &version);
    let response = ureq::get(&url)
        .header("Accept", "application/octet-stream")
        .header("User-Agent", user_agent())
        .call()
        .with_context(|| format!("download crate `{name}` version {version} from crates.io"))?;

    let decoder = GzDecoder::new(response.into_body().into_reader());
    let mut archive = tar::Archive::new(decoder);
    archive
        .unpack(&unpack_dir)
        .with_context(|| format!("unpack crate `{name}` version {version}"))?;

    let crate_root = find_unpacked_crate_root(&unpack_dir)
        .with_context(|| format!("locate unpacked crate root for `{name}` version {version}"))?;

    Ok(PreparedSource {
        path: crate_root,
        keep_temp,
        _temp_dir: Some(temp_dir),
    })
}

fn resolve_crate_version(name: &str, version: Option<&str>) -> anyhow::Result<String> {
    if let Some(version) = version {
        validate_crate_version(version)?;
        Ok(version.to_owned())
    } else {
        latest_crate_version(name)
    }
}

fn create_temp_dir(
    prefix: &str,
    temp_parent: Option<&Path>,
    keep_temp: bool,
) -> anyhow::Result<TempDir> {
    let mut builder = tempfile::Builder::new();
    builder.prefix(prefix);

    let mut temp_dir = if let Some(parent) = temp_parent {
        fs::create_dir_all(parent)
            .with_context(|| format!("create temporary parent directory {}", parent.display()))?;
        builder
            .tempdir_in(parent)
            .with_context(|| format!("create temporary directory in {}", parent.display()))
    } else {
        builder.tempdir().context("create temporary directory")
    }?;

    temp_dir.disable_cleanup(keep_temp);
    Ok(temp_dir)
}

fn latest_crate_version(name: &str) -> anyhow::Result<String> {
    #[derive(Debug, serde::Deserialize)]
    struct CratesIoResponse {
        #[serde(rename = "crate")]
        krate: CratesIoCrate,
    }

    #[derive(Debug, serde::Deserialize)]
    struct CratesIoCrate {
        max_version: String,
    }

    let url = crates_io_metadata_url(name);
    let response = ureq::get(&url)
        .header("Accept", "application/json")
        .header("User-Agent", user_agent())
        .call()
        .with_context(|| format!("fetch crate metadata for `{name}` from crates.io"))?;

    let metadata: CratesIoResponse = serde_json::from_reader(response.into_body().into_reader())
        .with_context(|| format!("parse crate metadata for `{name}` from crates.io"))?;

    Ok(metadata.krate.max_version)
}

fn crates_io_metadata_url(name: &str) -> String {
    format!("https://crates.io/api/v1/crates/{name}")
}

fn crates_io_download_url(name: &str, version: &str) -> String {
    format!("https://static.crates.io/crates/{name}/{name}-{version}.crate")
}

fn user_agent() -> &'static str {
    concat!("qualirs/", env!("CARGO_PKG_VERSION"), " (source analysis)")
}

fn validate_crate_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() {
        bail!("crate name cannot be empty");
    }

    if name.len() > 64 {
        bail!("crate name `{name}` is too long");
    }

    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        bail!("crate name `{name}` contains invalid characters");
    }

    Ok(())
}

fn validate_crate_version(version: &str) -> anyhow::Result<()> {
    if version.is_empty() {
        bail!("crate version cannot be empty");
    }

    if version.len() > 128 {
        bail!("crate version `{version}` is too long");
    }

    if !version.as_bytes()[0].is_ascii_digit() {
        bail!("crate version `{version}` must start with a digit");
    }

    if !version
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-' || byte == b'+')
    {
        bail!("crate version `{version}` contains invalid characters");
    }

    Ok(())
}

fn find_unpacked_crate_root(unpack_dir: &Path) -> anyhow::Result<PathBuf> {
    let mut candidates = Vec::with_capacity(1);
    for entry in fs::read_dir(unpack_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() && entry.path().join("Cargo.toml").is_file() {
            candidates.push(entry.path());
        }
    }

    match candidates.len() {
        1 => Ok(candidates.remove(0)),
        0 => bail!("crate archive did not contain a top-level Cargo.toml"),
        _ => bail!("crate archive contained multiple top-level Cargo.toml files"),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;

    #[test]
    fn validates_crate_names() {
        validate_crate_name("serde").expect("valid crate name");
        validate_crate_name("tokio-util").expect("valid crate name");
        validate_crate_name("proc_macro2").expect("valid crate name");

        assert!(validate_crate_name("").is_err());
        assert!(validate_crate_name("../serde").is_err());
        assert!(validate_crate_name("https://example.com/serde").is_err());
    }

    #[test]
    fn validates_crate_versions() {
        validate_crate_version("1.0.228").expect("valid crate version");
        validate_crate_version("0.1.0-alpha.1").expect("valid prerelease crate version");
        validate_crate_version("1.0.0+build.1").expect("valid build metadata crate version");

        assert!(validate_crate_version("").is_err());
        assert!(validate_crate_version("v1.0.0").is_err());
        assert!(validate_crate_version("../1.0.0").is_err());
    }

    #[test]
    fn creates_crates_io_download_url() {
        assert_eq!(
            crates_io_metadata_url("serde"),
            "https://crates.io/api/v1/crates/serde"
        );
        assert_eq!(
            crates_io_download_url("serde", "1.0.228"),
            "https://static.crates.io/crates/serde/serde-1.0.228.crate"
        );
    }

    #[test]
    fn finds_unpacked_crate_root() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let crate_dir = dir.path().join("demo-0.1.0");
        fs::create_dir(&crate_dir).expect("create crate dir");
        fs::write(crate_dir.join("Cargo.toml"), "[package]\nname = \"demo\"\n")
            .expect("write manifest");

        assert_eq!(
            find_unpacked_crate_root(dir.path()).expect("find crate root"),
            crate_dir
        );
    }

    #[test]
    fn creates_temp_dir_under_custom_parent() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let parent = dir.path().join("qualirs-temp");
        let temp_dir =
            create_temp_dir("qualirs-test-", Some(&parent), false).expect("create custom temp dir");
        let temp_path = temp_dir.path().to_path_buf();

        assert!(parent.is_dir());
        assert!(temp_path.starts_with(&parent));
        assert!(
            temp_path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("qualirs-test-"))
        );

        drop(temp_dir);
        assert!(!temp_path.exists());
    }

    #[test]
    fn can_preserve_temp_dir_for_inspection() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let parent = dir.path().join("qualirs-temp");
        let temp_dir =
            create_temp_dir("qualirs-test-", Some(&parent), true).expect("create custom temp dir");
        let temp_path = temp_dir.path().to_path_buf();

        drop(temp_dir);
        assert!(temp_path.exists());

        fs::remove_dir_all(temp_path).expect("remove preserved temp dir");
    }

    #[test]
    fn prepares_git_branch_source() {
        if !git_is_available() {
            return;
        }

        let repo = git_fixture();
        let url = repo.path().to_str().expect("repo path is UTF-8");

        let source = prepare_source(SourceRequest::Git {
            url,
            reference: Some(GitReference::Branch("feature")),
        })
        .expect("prepare git source");

        assert!(source.path().join("src/feature.rs").is_file());
        assert!(!source.path().join("src/tagged.rs").is_file());
    }

    #[test]
    fn prepares_git_tag_source() {
        if !git_is_available() {
            return;
        }

        let repo = git_fixture();
        let url = repo.path().to_str().expect("repo path is UTF-8");

        let source = prepare_source(SourceRequest::Git {
            url,
            reference: Some(GitReference::Tag("v1.0.0")),
        })
        .expect("prepare git source");

        assert!(source.path().join("src/tagged.rs").is_file());
        assert!(!source.path().join("src/feature.rs").is_file());
    }

    fn git_is_available() -> bool {
        Command::new("git")
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
    }

    fn git_fixture() -> TempDir {
        let dir = tempfile::tempdir().expect("create git fixture dir");
        run_git(dir.path(), ["init"]);
        run_git(
            dir.path(),
            ["config", "user.email", "qualirs@example.invalid"],
        );
        run_git(dir.path(), ["config", "user.name", "QualiRS Test"]);

        fs::create_dir(dir.path().join("src")).expect("create src dir");
        fs::write(dir.path().join("src/lib.rs"), "pub fn stable() {}\n").expect("write lib.rs");
        fs::write(dir.path().join("src/tagged.rs"), "pub fn tagged() {}\n")
            .expect("write tagged.rs");
        run_git(dir.path(), ["add", "."]);
        run_git(dir.path(), ["commit", "-m", "initial"]);
        run_git(dir.path(), ["tag", "v1.0.0"]);

        run_git(dir.path(), ["checkout", "-b", "feature"]);
        fs::remove_file(dir.path().join("src/tagged.rs")).expect("remove tagged.rs");
        fs::write(dir.path().join("src/feature.rs"), "pub fn feature() {}\n")
            .expect("write feature.rs");
        run_git(dir.path(), ["add", "."]);
        run_git(dir.path(), ["commit", "-m", "feature"]);

        dir
    }

    fn run_git<I, S>(dir: &Path, args: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("run git");

        assert!(
            output.status.success(),
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
