use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// QualiRS — structural and architectural code smell detector for Rust.
#[derive(Parser, Debug)]
#[command(name = "qualirs", version, about)]
pub struct Args {
    #[command(subcommand)]
    pub(crate) command: Option<Command>,

    #[command(flatten)]
    pub(crate) source: SourceOptions,

    #[command(flatten)]
    pub(crate) filters: FilterOptions,

    #[command(flatten)]
    pub(crate) output_options: OutputOptions,

    /// List available detectors and exit
    #[arg(long)]
    pub(crate) list_detectors: bool,
}

#[derive(clap::Args, Debug)]
pub(crate) struct SourceOptions {
    /// Path to the Rust project or file to analyze (defaults to current directory)
    #[arg(conflicts_with_all = ["git", "crate_name"])]
    pub(crate) path: Option<PathBuf>,

    /// Git repository URL to clone and analyze
    #[arg(long, value_name = "URL", conflicts_with = "crate_name")]
    pub(crate) git: Option<String>,

    /// Git branch to check out when using --git
    #[arg(
        long,
        value_name = "BRANCH",
        requires = "git",
        conflicts_with_all = ["tag", "crate_name"]
    )]
    pub(crate) branch: Option<String>,

    /// Git tag to check out when using --git
    #[arg(
        long,
        value_name = "TAG",
        requires = "git",
        conflicts_with_all = ["branch", "crate_name"]
    )]
    pub(crate) tag: Option<String>,

    /// crates.io crate name to download and analyze
    #[arg(long = "crate", value_name = "CRATE", conflicts_with = "git")]
    pub(crate) crate_name: Option<String>,

    /// crates.io crate version to download when using --crate
    #[arg(
        long,
        value_name = "VERSION",
        requires = "crate_name",
        conflicts_with = "git"
    )]
    pub(crate) crate_version: Option<String>,

    /// Directory to create temporary git and crate analysis folders in
    #[arg(long, value_name = "DIR")]
    pub(crate) temp_dir: Option<PathBuf>,

    /// Preserve temporary git and crate analysis folders after the run
    #[arg(long)]
    pub(crate) keep_temp: bool,
}

#[derive(clap::Args, Debug)]
pub(crate) struct FilterOptions {
    /// Configuration file path (default: qualirs.toml in project root)
    #[arg(short, long)]
    pub(crate) config: Option<PathBuf>,

    /// Number of analysis threads to use (0 = all logical CPUs)
    #[arg(long)]
    pub(crate) threads: Option<usize>,

    /// Minimum severity to report: info, warning, critical
    #[arg(short = 'm', long)]
    pub(crate) min_severity: Option<String>,

    /// Precision mode: conservative, balanced, exploratory
    #[arg(long)]
    pub(crate) precision: Option<String>,

    /// Show only smells of a specific category
    #[arg(short = 't', long)]
    pub(crate) category: Option<String>,
}

#[derive(clap::Args, Debug)]
pub(crate) struct OutputOptions {
    /// Quiet mode: only show summary counts
    #[arg(short, long)]
    pub(crate) quiet: bool,

    /// Compact mode: show findings as a categorized list (default)
    #[arg(long, conflicts_with_all = ["quiet", "table"])]
    pub(crate) compact: bool,

    /// Table mode: show findings in the legacy table layout
    #[arg(long, conflicts_with_all = ["quiet", "compact", "llm"])]
    pub(crate) table: bool,

    /// LLM mode: show compact Markdown with fenced finding blocks for coding assistants
    #[arg(long, conflicts_with_all = ["quiet", "compact", "how_fix"])]
    pub(crate) llm: bool,

    /// How-fix mode: explain each finding with the current source code and improvement guidance
    #[arg(
        long,
        conflicts_with_all = ["quiet", "compact", "table", "llm", "format", "list_detectors"]
    )]
    pub(crate) how_fix: bool,

    /// Output format
    #[arg(
        long,
        value_enum,
        conflicts_with_all = ["quiet", "compact", "table", "llm", "how_fix", "list_detectors"]
    )]
    pub(crate) format: Option<OutputFormat>,

    /// Write JSON findings to a file instead of stdout
    #[arg(
        long = "output",
        requires = "format",
        conflicts_with = "list_detectors"
    )]
    pub(crate) output_path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum OutputFormat {
    Json,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// Generate a default qualirs.toml configuration file
    InitConfig {
        /// Config file to create
        #[arg(short, long, default_value = "qualirs.toml")]
        output: PathBuf,

        /// Overwrite an existing config file
        #[arg(short, long)]
        force: bool,
    },
}

impl Args {
    pub fn parse_args() -> Self {
        Parser::parse()
    }
}
