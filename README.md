# QualiRS

**Structural and architectural code smell detector for Rust.**

QualiRS scans Rust source files using a `syn` AST and 96 built-in detectors across seven categories. It highlights maintainability issues, structural metrics, and potential performance or safety problems for review. Run it alongside the compiler and Clippy.

Source scanning does not require the target project to compile. Type-sensitive rules use bounded local evidence, including resolvable imports, types, and bindings. QualiRS does not perform compiler-level type checking or expand macros, and a finding is not proof of a bug. The default **conservative** mode reports only high-confidence findings; broader structural checks and uncertain heuristics are available through other precision modes.

## Build and run

Use a Rust toolchain supporting the project's Rust 2024 edition and locked dependencies. Git is needed for cloning repositories.

```bash
git clone https://github.com/RustedBytes/qualirs.git
cd qualirs
cargo build --release --locked

# Run the built binary on the current directory
./target/release/qualirs .

# Optional: install the local checkout into Cargo's bin directory
cargo install --path . --locked
```

On Windows PowerShell, run `.\target\release\qualirs.exe .`. The examples below use `qualirs` from your `PATH`.

```bash
# A directory or a standalone Rust file
qualirs /path/to/my-crate
qualirs src/lib.rs --config qualirs.toml

# Include structural findings, or all confidence levels
qualirs --precision balanced .
qualirs --precision exploratory .

# Filter the report and set analysis parallelism
qualirs --min-severity warning --category performance --threads 4 .

# Summary, resource usage, or detailed guidance
qualirs --quiet .
qualirs --stats .
qualirs --how-fix --precision balanced .

# JSON on stdout or in a file
qualirs --format json .
qualirs --format json --output qualirs-report.json .

# Discover rules and generate a complete configuration
qualirs --list-detectors
qualirs init-config
```

### Remote sources

```bash
qualirs --git https://github.com/RustedBytes/qualirs.git --branch main
qualirs --git git@github.com:RustedBytes/qualirs.git
qualirs --crate serde
qualirs --crate serde --crate-version 1.0.228
qualirs --crate serde --keep-temp --temp-dir /var/tmp/qualirs
```

Choose one source: a local path, `--git`, or `--crate`. Git sources accept either `--branch` or `--tag`. Without `--crate-version`, QualiRS requests the latest published crate version. Downloaded and cloned sources are temporary unless `--keep-temp` is set; `--temp-dir` chooses their parent directory. Remote acquisition requires network access and the appropriate repository credentials.

## Precision, severity, and CI

Confidence describes the strength of a detector's evidence. Severity describes the reported concern. They are independent: a low-confidence heuristic can carry critical severity when shown in exploratory mode.

| Precision | Confidence levels reported | Intended use |
|---|---|---|
| `conservative` (default) | High | Findings with stronger source evidence |
| `balanced` | High and medium | Also review structural and maintainability findings |
| `exploratory` | High, medium, and low | Also inspect unproven heuristics |

`--precision` overrides the configured mode for that run. All modes still respect severity, category, policy exclusions, and ignores. A clean conservative report does not imply that an exploratory report will be empty.

| Severity | Meaning | Effect on a successful analysis run |
|---|---|---|
| Info | Suggestion or review hint | Does not fail the run |
| Warning | Concern worth reviewing | Does not fail the run |
| Critical | Higher-severity concern, including large structural metrics | Exit code 1 if retained in the report |

With no reported critical findings, analysis exits with code 0. Precision, severity, category, and ignore filters apply before this decision. Clap usage errors use exit code 2; configuration, source-acquisition, and other operational errors can also fail the command. Parse errors are reported separately and do not by themselves cause the critical-finding exit. Check JSON `parse_errors` if CI must require every source file to parse.

## CLI options

Run `qualirs --help` for the complete command syntax: `qualirs [OPTIONS] [PATH] [COMMAND]`.

| Option | Purpose |
|---|---|
| `--git <URL>` | Clone and analyze a Git repository |
| `--branch <BRANCH>`, `--tag <TAG>` | Select a Git reference; mutually exclusive |
| `--crate <CRATE>`, `--crate-version <VERSION>` | Download a crates.io source package |
| `--temp-dir <DIR>`, `--keep-temp` | Choose temporary storage and optionally preserve it |
| `-c, --config <CONFIG>` | Load an explicit configuration file |
| `--threads <THREADS>` | Rayon worker count; 0 uses the default pool, typically all logical CPUs |
| `-m, --min-severity <LEVEL>` | `info`, `warning`, or `critical` |
| `--precision <MODE>` | `conservative`, `balanced`, or `exploratory` |
| `-t, --category <CATEGORY>` | Filter by one of the seven categories below |
| `-q, --quiet` | Summary counts only |
| `--compact` | Categorized terminal list; the default output |
| `--table` | Table output |
| `--llm` | Markdown with fenced finding blocks for coding assistants |
| `--how-fix` | Current source and improvement guidance; no files are changed |
| `--stats` | Append elapsed time, CPU time, and peak process memory |
| `--format json` | Machine-readable report |
| `--output <OUTPUT_PATH>` | Write JSON to a file; requires `--format json` |
| `--list-detectors` | Print the built-in rule inventory and exit |
| `-h, --help`, `-V, --version` | Show help or version |

Choose one output mode. `--stats` works with the analysis output modes. Replacement snippets in detailed guidance are limited to supported transformations; uncertain cases provide guidance without replacement code.

### JSON

Reports contain `summary`, `smells`, and `parse_errors`. Each finding includes `code`, `severity`, `confidence`, `category`, `name`, `location`, `message`, and `suggestion`. Locations contain `file`, `line_start`, `line_end`, and an optional `column`. Line numbers start at 1; columns, when present, start at 0.

The `files_analyzed` summary currently counts discovered Rust files before policy exclusions and parse failures, so it is not the number of files on which every detector ran.

### Resource statistics

Add `--stats` for a footer such as:

```text
Analysis resources:
  Elapsed time: 1 min 34.559 s
  CPU time (all process threads): 1 min 43.828 s
  Peak memory (process lifetime): 262.75 MiB
  CPU and memory exclude child processes.
```

Elapsed and CPU times cover file discovery, parsing, and detectors. Cloning/downloading, configuration loading, and report formatting are outside that interval. CPU time sums all QualiRS threads and can exceed elapsed time. Peak memory is the process's highest resident memory usage up to the end of analysis, including preparation; it is not total allocated bytes. Cargo and Git child processes are excluded from CPU and memory counters.

Durations use units from nanoseconds through days; memory uses bytes and binary units such as KiB, MiB, and GiB. CPU and memory counters are supported on Windows, Linux, and macOS, with `unavailable` shown when a counter cannot be read. Without `--stats`, these resource counters are not collected.

For JSON output, the footer goes to **stderr**, including when `--output` writes the report to a file. Statistics do not add JSON fields or change exit-code behavior.

## Configuration and ignores

```bash
qualirs init-config
qualirs init-config --output config/qualirs.toml
# Overwrite an existing configuration explicitly
qualirs init-config --force
```

Without `--config`, QualiRS looks for `qualirs.toml` directly in the analyzed directory; it does not search parent directories for configuration. For standalone file analysis, pass `--config` explicitly to use your project's settings. An invalid automatically discovered config prints a warning and falls back to defaults; an invalid explicit config fails the command.

A small configuration can override reporting and policy settings:

```toml
precision = "conservative"
min_severity = "info"
threads = 0
exclude_paths = ["target", ".git", "node_modules"]
ignore_findings = []

[policy]
skip_tests = true
skip_examples = true
skip_benches = true
skip_generated = true
skip_macro_heavy_files = true
skip_data_carrier_structs = true
skip_template_structs = true
```

For numeric thresholds, start with the complete file produced by `init-config` and edit its values. Partial threshold tables are not merged field-by-field with the built-in defaults. The generated file also lists `test_path_markers` and `data_carrier_struct_suffixes` for customizing policy matching. The repository's [qualirs.toml](qualirs.toml) provides an annotated configuration aligned with these defaults.

Tests, examples, benches, recognized generated sources, and macro-heavy files are excluded by default. Data-carrier and template policies exempt matching structs from applicable design rules. Disable the corresponding policy setting to include those sources. Test-only items are masked before detection while preserving source locations: conditions requiring `test` are excluded, while production-capable conditions such as `not(test)` and `any(test, feature = "...")` remain eligible. This is not general Cargo feature evaluation.

Built-in finding codes run from `Q0001` through `Q0096`. Ignore a rule throughout a scan with, for example, `ignore_findings = ["Q0001", "Q0011"]`. For an individual finding, put the directive immediately before its reported source line:

```rust
// qualirs:ignore Q0068
let _ = fallible_operation();
```

Codes are case-insensitive; separate multiple codes with spaces or commas. `// qualirs:ignore` with no codes suppresses all findings on the next line.

## Detectors and analysis limits

See [docs/detectors.md](docs/detectors.md) for evidence requirements, examples, and confidence notes. `qualirs --list-detectors` prints every built-in code and name.

| Category | Count | Examples |
|---|---:|---|
| Architecture | 13 | God Module, Layer Violation, Public API Leak, Duplicate Dependency Versions |
| Design | 16 | Large Trait, Anemic Struct, Data Clumps, God Struct, Large Error Enum |
| Implementation | 14 | Long Function, High Cyclomatic Complexity, Deep If/Else Nesting, Duplicate Match Arms |
| Performance | 23 | Excessive Clone, Missing Collection Preallocation, Repeated Regex Construction, Inline Candidate |
| Idiomaticity | 11 | Excessive Unwrap, Unused Result Ignored, Manual Find/Any Loop, Derivable Impl |
| Concurrency | 9 | Blocking in Async, Spawn Without Join, Holding Lock Across Await |
| Unsafe | 10 | Unsafe Without Comment, FFI Without Wrapper, Unsafe Fn Missing Safety Docs |

Structural thresholds measure source shape, not runtime defects. Locally unresolved types stay unknown. Possible pointer aliasing, unproven lock overlap, nonzero Unicode character-count comparisons, and name-based inline candidates remain exploratory hints. Safety documentation checks recognize written explanations; they do not verify the soundness of unsafe code.

Most rules inspect individual source files. Some architecture checks inspect nearby manifests or module files. Q0011 invokes `cargo tree -d --locked`; dependency resolution may access the network, and the rule produces no finding if Cargo is unavailable or fails. Set Cargo's `CARGO_NET_OFFLINE=true` environment variable when dependency checks must use only cached data. There is no QualiRS `--offline` flag.

The walker respects Git ignore rules and `exclude_paths` and skips hidden entries. Macros are not expanded, so generated Rust inside macro token streams is not generally analyzed. Syntax-only analysis can miss problems and produce false positives. The [self-audit](docs/self-audit.md) and repository audit reports in [docs](docs) record regression-driven improvements and their limits.

## Example output

Given `example.rs`:

```rust
fn work() -> Result<(), ()> { Ok(()) }
fn main() {
    let _ = work();
}
```

Running `qualirs example.rs` produces:

```text
QualiRS — Rust Code Smell Detector
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  → 1 files analyzed, 1 smell(s) detected
    0 critical  1 warning  0 info

▸ Idiomaticity
  WARN Q0068 Unused Result Ignored example.rs:3
    A Result is discarded without handling its error

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Found 1 smell(s). Review warnings above.
```

## Code organization and extensions

| Directory | Responsibility |
|---|---|
| [src/cli](src/cli) | Arguments, terminal/JSON reporting, guidance, resource statistics |
| [src/analysis](src/analysis) | Engine, detector trait, local evidence, source-note and visitor helpers |
| [src/detectors](src/detectors) | Built-in rules, shared policy, and detector-specific helpers |
| [src/domain](src/domain) | Findings, codes, source files, and configuration |
| [src/infrastructure](src/infrastructure) | Source acquisition and ignore-aware file discovery |
| [tests](tests) | Detector, CLI, and audit regressions |
| [vscode](vscode/README.md) | VS Code extension and packaging instructions |

The library exposes `Detector` and `Engine` for registering custom checks in Rust. There is no CLI plugin-loading flag. This complete example adds a file-length review rule to the built-in set:

```rust
use qualirs::{
    analysis::{detector::Detector, engine::Engine},
    domain::{
        config::{Config, Precision},
        smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
        source::SourceFile,
    },
};

struct FileLengthReview;

impl Detector for FileLengthReview {
    fn name(&self) -> &str {
        "File Length Review"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        if file.line_count <= 2_000 {
            return Vec::new();
        }
        vec![Smell::new(
            SmellCategory::Implementation,
            self.name(),
            Severity::Info,
            FindingConfidence::Medium,
            SourceLocation::new(file.path.clone(), 1, 1, None),
            format!("File has {} lines", file.line_count),
            "Review whether this file has responsibilities that belong in separate modules.",
        )]
    }
}

fn main() {
    let mut engine = Engine::new(Config {
        precision: Precision::Balanced,
        ..Config::default()
    });
    engine.register_defaults();
    engine.register(Box::new(FileLengthReview));
    let report = engine.analyze(std::path::Path::new("."));
    println!("{} findings", report.total_smells());
}
```

`Smell::new` assigns codes using the built-in name registry; unknown names receive `Q0000`. When contributing a built-in detector, add its code metadata in [src/domain/smell.rs](src/domain/smell.rs), register it in [src/analysis/engine.rs](src/analysis/engine.rs), and update the [CLI inventory](src/cli/detector_list.rs) and [detector reference](docs/detectors.md).

## Development

```bash
cargo test --locked
cargo build --release --locked

# With dependencies already cached
cargo test --locked --offline
cargo build --release --locked --offline
```

Add genuine positive examples alongside false-positive regressions when changing a rule. Verify reporting precision, source locations, and ignores for affected findings. Prefer behavior-preserving refactoring over changing thresholds to make a report empty.

## License

MIT, as declared in [Cargo.toml](Cargo.toml).
