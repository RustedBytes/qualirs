mod analysis;
mod cli;
mod detectors;
mod domain;
mod infrastructure;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use analysis::engine::{AnalysisReport, Engine};
use cli::args::{Args, Command, OutputFormat};
use domain::config::{Config, Precision};
use domain::smell::{Severity, SmellCategory};
use infrastructure::source::{
    GitReference, SourceRequest, prepare_source, prepare_source_in, prepare_source_with_options,
};

fn main() -> anyhow::Result<ExitCode> {
    run()
}

fn run() -> anyhow::Result<ExitCode> {
    let args = Args::parse_args();

    if let Some(command) = &args.command {
        run_command(command)?;
        return Ok(ExitCode::SUCCESS);
    }

    if args.list_detectors {
        cli::detector_list::print_detector_list();
        return Ok(ExitCode::SUCCESS);
    }

    let source = prepare_analysis_source(&args)?;
    if let Some(path) = source.preserved_path() {
        eprintln!("Preserving temporary analysis source at {}", path.display());
    }
    let config = get_config(&args, source.path())?;
    let engine = setup_engine(config);
    let mut report = engine.analyze(source.path());

    apply_category_filter(&mut report, args.filters.category.as_deref())?;
    emit_report(&report, &args.output_options)?;

    Ok(exit_code_for_report(&report))
}

fn run_command(command: &Command) -> anyhow::Result<()> {
    match command {
        Command::InitConfig { output, force } => {
            Config::write_default_file(output, *force)?;
            println!("Created {}", output.display());
            Ok(())
        }
    }
}

fn prepare_analysis_source(args: &Args) -> anyhow::Result<infrastructure::source::PreparedSource> {
    let source = &args.source;

    if source.git.is_none() && (source.branch.is_some() || source.tag.is_some()) {
        anyhow::bail!("--branch and --tag can only be used with --git");
    }

    let request = if let Some(url) = source.git.as_deref() {
        SourceRequest::Git {
            url,
            reference: git_reference(source),
        }
    } else if let Some(name) = source.crate_name.as_deref() {
        SourceRequest::Crate {
            name,
            version: source.crate_version.as_deref(),
        }
    } else {
        let path = source.path.as_deref().unwrap_or_else(|| Path::new("."));
        SourceRequest::Local(path)
    };

    if source.keep_temp {
        prepare_source_with_options(request, source.temp_dir.as_deref(), true)
    } else if let Some(temp_dir) = source.temp_dir.as_deref() {
        prepare_source_in(request, Some(temp_dir))
    } else {
        prepare_source(request)
    }
}

fn git_reference(source: &cli::args::SourceOptions) -> Option<GitReference<'_>> {
    source
        .branch
        .as_deref()
        .map(GitReference::Branch)
        .or_else(|| source.tag.as_deref().map(GitReference::Tag))
}

fn get_config(args: &Args, analysis_path: &Path) -> anyhow::Result<Config> {
    let path = analysis_path
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(analysis_path));
    let filters = &args.filters;
    let mut config = if let Some(config_path) = &filters.config {
        Config::load_from_file(config_path)?
    } else {
        Config::load_or_default(&path)
    };

    if let Some(min_severity) = &filters.min_severity {
        config.min_severity = match min_severity.to_lowercase().as_str() {
            "critical" => Severity::Critical,
            "warning" | "warn" => Severity::Warning,
            "info" => Severity::Info,
            other => {
                anyhow::bail!("Unknown severity level: {other}. Use: info, warning, critical");
            }
        };
    }

    if let Some(precision) = &filters.precision {
        config.precision = precision.parse::<Precision>().map_err(anyhow::Error::msg)?;
    }

    if let Some(threads) = filters.threads {
        config.threads = threads;
    }

    Ok(config)
}

fn setup_engine(config: Config) -> Engine {
    let mut engine = Engine::new(config);
    engine.register_defaults();
    engine
}

fn apply_category_filter(
    report: &mut AnalysisReport,
    category: Option<&str>,
) -> anyhow::Result<()> {
    let Some(category) = category else {
        return Ok(());
    };
    let category = category
        .parse::<SmellCategory>()
        .map_err(anyhow::Error::msg)?;

    report.smells.retain(|smell| smell.category == category);
    Ok(())
}

fn emit_report(
    report: &AnalysisReport,
    output_options: &cli::args::OutputOptions,
) -> anyhow::Result<()> {
    if output_options.quiet {
        print_summary(report);
        return Ok(());
    }

    if output_options.format == Some(OutputFormat::Json) {
        cli::json_output::emit_json_report(report, output_options.output_path.as_deref())?;
        return Ok(());
    }

    if output_options.llm {
        cli::output::print_llm_report(report);
        return Ok(());
    }

    if output_options.how_fix {
        cli::output::print_how_fix_report(report);
        return Ok(());
    }

    if output_options.table {
        cli::output::print_report(report);
        return Ok(());
    }

    cli::output::print_compact_report(report);
    Ok(())
}

fn exit_code_for_report(report: &AnalysisReport) -> ExitCode {
    if report.count_by_severity(Severity::Critical) > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn print_summary(report: &AnalysisReport) {
    println!(
        "Files: {} | Smells: {} (Critical: {}, Warning: {}, Info: {})",
        report.total_files,
        report.total_smells(),
        report.count_by_severity(Severity::Critical),
        report.count_by_severity(Severity::Warning),
        report.count_by_severity(Severity::Info),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use cli::args::{FilterOptions, OutputOptions, SourceOptions};

    fn args_with_config(config: PathBuf, min_severity: Option<String>) -> Args {
        Args {
            command: None,
            source: SourceOptions {
                path: None,
                git: None,
                branch: None,
                tag: None,
                crate_name: None,
                crate_version: None,
                temp_dir: None,
                keep_temp: false,
            },
            filters: FilterOptions {
                config: Some(config),
                threads: None,
                min_severity,
                precision: None,
                category: None,
            },
            output_options: OutputOptions {
                quiet: false,
                compact: false,
                table: false,
                llm: false,
                how_fix: false,
                format: None,
                output_path: None,
            },
            list_detectors: false,
        }
    }

    #[test]
    fn config_min_severity_is_used_when_cli_flag_is_absent() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let config_path = dir.path().join("qualirs.toml");
        std::fs::write(&config_path, "min_severity = \"critical\"\n").expect("write config");

        let config =
            get_config(&args_with_config(config_path, None), dir.path()).expect("load config");

        assert_eq!(config.min_severity, Severity::Critical);
    }

    #[test]
    fn cli_min_severity_overrides_config_value() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let config_path = dir.path().join("qualirs.toml");
        std::fs::write(&config_path, "min_severity = \"critical\"\n").expect("write config");

        let config = get_config(
            &args_with_config(config_path, Some("warning".into())),
            dir.path(),
        )
        .expect("load config");

        assert_eq!(config.min_severity, Severity::Warning);
    }

    #[test]
    fn cli_threads_overrides_config_value() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let config_path = dir.path().join("qualirs.toml");
        std::fs::write(&config_path, "threads = 2\n").expect("write config");
        let mut args = args_with_config(config_path, None);
        args.filters.threads = Some(4);

        let config = get_config(&args, dir.path()).expect("load config");

        assert_eq!(config.threads, 4);
    }

    #[test]
    fn config_precision_is_used_when_cli_flag_is_absent() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let config_path = dir.path().join("qualirs.toml");
        std::fs::write(&config_path, "precision = \"balanced\"\n").expect("write config");

        let config =
            get_config(&args_with_config(config_path, None), dir.path()).expect("load config");

        assert_eq!(config.precision, Precision::Balanced);
    }

    #[test]
    fn cli_precision_overrides_config_value() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let config_path = dir.path().join("qualirs.toml");
        std::fs::write(&config_path, "precision = \"balanced\"\n").expect("write config");
        let mut args = args_with_config(config_path, None);
        args.filters.precision = Some("exploratory".into());

        let config = get_config(&args, dir.path()).expect("load config");

        assert_eq!(config.precision, Precision::Exploratory);
    }

    #[test]
    fn cli_rejects_malformed_precision() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let config_path = dir.path().join("qualirs.toml");
        std::fs::write(&config_path, "").expect("write config");
        let mut args = args_with_config(config_path, None);
        args.filters.precision = Some("strict".into());

        let err = get_config(&args, dir.path()).expect_err("invalid precision");

        assert!(err.to_string().contains("Unknown precision"));
    }
}
