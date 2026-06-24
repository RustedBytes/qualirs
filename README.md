# QualiRS

**Structural and architectural code smell detector for Rust.**

QualiRS parses your Rust source code with AST analysis and detects structural code smells across 7 categories: Architecture, Design, Implementation, Performance, Idiomaticity, Concurrency, and Unsafe. It is designed to complement `clippy` — where clippy focuses on lint-level correctness and idioms, QualiRS focuses on structural, architectural, and design-level problems.

## Features

- 96 built-in smell detectors across 7 categories
- Parallel analysis via rayon, with configurable thread count
- Configurable thresholds via `qualirs.toml`
- Stable `Q0001`-style finding codes with config-based and inline ignores
- False-positive policy controls for tests, DTOs, templates, and config structs
- Compact terminal output by default, with table, LLM, quiet, and JSON modes
- CI-friendly: exits with code 1 on critical smells
- Respects `.gitignore` automatically

## Quick Start

```bash
# Build
cargo build --release

# Analyze current project
cargo run --release -- .

# Analyze a specific path
qualirs ~/projects/my-crate

# Analyze a git repository
qualirs --git https://github.com/bivex/QualiR
qualirs --git git@github.com:bivex/QualiR.git

# Use a specific parent directory for temporary git/crate sources
qualirs --git https://github.com/bivex/QualiR --temp-dir /var/tmp/qualirs

# Keep temporary git/crate sources for inspection after analysis
qualirs --crate serde --keep-temp --temp-dir /var/tmp/qualirs

# Analyze a specific git branch or tag
qualirs --git https://github.com/bivex/QualiR --branch main
qualirs --git git@github.com:bivex/QualiR.git --tag v1.0.0

# Analyze the latest published crates.io source for a crate
qualirs --crate serde

# Analyze a specific crates.io crate version
qualirs --crate serde --crate-version 1.0.228

# List all detectors
qualirs --list-detectors

# Create a default configuration file
qualirs init-config

# Only show warnings and critical
qualirs --min-severity warning .

# Use four analysis threads
qualirs --threads 4 .

# Quiet mode (summary only, great for CI)
qualirs --quiet .

# JSON output
qualirs --format json --output qualirs-report.json .
```

## CLI Reference

```
qualirs [OPTIONS] [PATH] [COMMAND]

Arguments:
  [PATH]  Path to the Rust project or file to analyze (defaults to current directory)

Commands:
  init-config  Generate a default qualirs.toml configuration file
  help         Print this message or the help of the given subcommand(s)

Options:
      --git <URL>                    Git repository URL to clone and analyze
      --branch <BRANCH>              Git branch to check out when using --git
      --tag <TAG>                    Git tag to check out when using --git
      --crate <CRATE>                crates.io crate name to download and analyze
      --crate-version <VERSION>      crates.io crate version to download when using --crate
      --temp-dir <DIR>               Directory to create temporary git and crate analysis folders in
      --keep-temp                    Preserve temporary git and crate analysis folders after the run
  -c, --config <CONFIG>              Configuration file path (default: qualirs.toml in project root)
      --threads <THREADS>            Number of analysis threads to use (0 = all logical CPUs)
  -m, --min-severity <MIN_SEVERITY>  Minimum severity to report: info, warning, critical
      --precision <PRECISION>        Precision mode: conservative, balanced, exploratory
  -t, --category <CATEGORY>          Show only smells of a specific category
  -q, --quiet                        Quiet mode: only show summary counts
      --compact                      Compact mode: show findings as a categorized list (default)
      --table                        Table mode: show findings in the legacy table layout
      --llm                          LLM mode: show compact Markdown with fenced finding blocks for coding assistants
      --how-fix                      Explain each finding with current source code and improvement guidance
      --format <FORMAT>              Output format [possible values: json]
      --output <OUTPUT_PATH>         Write JSON findings to a file instead of stdout
      --list-detectors               List available detectors and exit
  -h, --help                         Print help
  -V, --version                      Print version
```

### Generate Configuration

```
qualirs init-config [OPTIONS]

Options:
  -o, --output <OUTPUT>  Config file to create [default: qualirs.toml]
  -f, --force            Overwrite an existing config file
  -h, --help             Print help
```

## Detectors

Run `qualirs --list-detectors` for the complete detector inventory. See the [detector reference](docs/detectors.md) for explanations and good/bad examples for every rule. The current built-in set is grouped as follows:

| Category | Count | Examples |
|---|---:|---|
| Architecture | 13 | God Module, Layer Violation, Public API Leak, Duplicate Dependency Versions |
| Design | 16 | Large Trait, Anemic Struct, Data Clumps, God Struct, Large Error Enum |
| Implementation | 14 | Long Function, Magic Numbers, Deep If/Else Nesting, Duplicate Match Arms |
| Performance | 23 | Excessive Clone, Missing Collection Preallocation, Repeated Regex Construction, Inline Candidate |
| Idiomaticity | 11 | Excessive Unwrap, Unused Result Ignored, Manual Find/Any Loop, Derivable Impl |
| Concurrency | 9 | Blocking in Async, Spawn Without Join, Holding Lock Across Await |
| Unsafe | 10 | Unsafe Without Comment, FFI Without Wrapper, Unsafe Fn Missing Safety Docs |

Several detectors use configurable numeric thresholds. Others report any matching pattern because the match is specific enough to warrant review.

### Magic Number Whitelist

The following numbers are **not** flagged as magic: `0`, `1`, `-1`, `2`, `10`, `100`, `1000`, `255`, `256`, `1024`.

## Configuration

Create a `qualirs.toml` in your project root. All fields are optional — missing values use defaults.

```bash
qualirs init-config
```

```toml
exclude_paths = [
    "target",
    ".git",
    "node_modules",
]
min_severity = "info"
precision = "conservative"
threads = 0
ignore_findings = [
    # "Q0001", # God Module
]

[thresholds.arch]
god_module_loc = 1000
god_module_items = 20
public_api_ratio = 0.7
feature_concentration = 15
hidden_global_state = 3

[thresholds.design]
large_trait_methods = 15
excessive_generics = 5
deep_trait_bounds = 4
wide_hierarchy = 10
fat_impl_methods = 20
god_struct_fields = 20
primitive_obsession_fields = 4
data_clumps_args = 3
data_clumps_occurrences = 3
stringly_typed_fields = 3
large_error_enum_variants = 12

[thresholds.impl]
long_function_loc = 50
long_closure_loc = 25
deep_closure_nesting = 3
cyclomatic_complexity = 15
too_many_arguments = 6
deep_match_nesting = 3
deep_if_else = 4
excessive_unwrap = 3
large_enum_variants = 20
long_method_chain = 4
lifetime_explosion = 4
unsafe_block_overuse = 5
deeply_nested_type = 3
interior_mutability_abuse = 5

[thresholds.concurrency]
large_future_loc = 100
arc_mutex_overuse = 3

[thresholds.unsafe]
unsafe_without_comment = true

[policy]
skip_tests = true
test_path_markers = ["tests", "test", "tests.rs", "_tests.rs", "fuzz", "fuzz_targets"]
skip_examples = true
skip_benches = true
skip_generated = true
skip_macro_heavy_files = true
skip_data_carrier_structs = true
skip_template_structs = true
data_carrier_struct_suffixes = [
    "Activity",
    "Command",
    "Config",
    "ConfigFile",
    "Descriptor",
    "Details",
    "Dto",
    "DTO",
    "Entry",
    "Event",
    "Failure",
    "Finding",
    "FormData",
    "Grant",
    "Hit",
    "Inspection",
    "Item",
    "Link",
    "Metrics",
    "Notification",
    "Options",
    "Outcome",
    "Overview",
    "Page",
    "Query",
    "Report",
    "Request",
    "Response",
    "Result",
    "Settings",
    "SettingsFile",
    "Session",
    "Snapshot",
    "Stats",
    "Summary",
    "Template",
    "View",
    "Vulnerability",
]
```

Policy settings control broad false-positive suppression. Set `skip_tests = false`, `skip_examples = false`, or `skip_benches = false` to analyze those files with the same rules as production code. Set `skip_generated = false`, `skip_macro_heavy_files = false`, `skip_data_carrier_structs = false`, or edit `data_carrier_struct_suffixes` if those sources should be checked by design detectors.

Precision controls how much heuristic signal is shown by default. `conservative` reports only high-confidence findings, `balanced` includes medium-confidence structural findings, and `exploratory` includes every detector result. Use `--precision balanced` or `--precision exploratory` to override the config for one run.

Each detector emits a stable `QNNNN` code in terminal and JSON output. Add codes to `ignore_findings` to suppress every matching finding, for example `ignore_findings = ["Q0001", "Q0011"]`. Run `qualirs --list-detectors` to see the full code list.

To suppress a single finding inline, put a QualiRS ignore comment on the line immediately before the reported source line:

```rust
// qualirs:ignore Q0068
let _ = fallible_operation();
```

The code match is case-insensitive, and multiple codes can be separated by spaces or commas. Use `// qualirs:ignore` without codes to suppress any QualiRS finding on the next line.

## Severity Levels

| Level | Meaning | Exit code impact |
|---|---|---|
| **Info** | Style/convention suggestion, no action required | Exit 0 |
| **Warning** | Structural problem that should be addressed | Exit 0 |
| **Critical** | Serious smell requiring immediate attention | Exit 1 |

Use `--min-severity warning` to hide info-level smells, or `--min-severity critical` to only see the worst.

## Example Output

```
QualiRS — Rust Code Smell Detector
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  → 32 files analyzed, 8 smell(s) detected
    0 critical  2 warning  6 info

▸ Architecture
  INFO Q0005 Public API Explosion src/detectors/mod.rs:1
    Module exposes a high ratio of public items (7/7)

▸ Design
  INFO Q0016 Anemic Struct src/domain/smell.rs:30
    Struct `SourceLocation` has fields but no impl block in this file

▸ Implementation
  WARN Q0017 Long Function src/main.rs:12
    Function `main` is ~58 lines long (threshold: 50)
  WARN Q0017 Long Function src/detectors/generics.rs:44
    Function `check_generics` is ~53 lines long (threshold: 50)

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Found 8 smell(s). Review warnings above.
```

## Architecture

QualiRS follows a clean layered architecture with strict dependency direction:

```
┌─────────────────────────────────────────────────────────┐
│  CLI (clap, compact/table/LLM/JSON output)              │
├─────────────────────────────────────────────────────────┤
│  Analysis Engine (Detector trait, policy, parallel run)  │
├──────────────┬──────────────────────────────────────────┤
│  Detectors   │  Domain (Smell, SourceLocation, Config)  │
│  (83 rules)  │                                          │
├──────────────┴──────────────────────────────────────────┤
│  Infrastructure (ignore-aware file walker)              │
├─────────────────────────────────────────────────────────┤
│  Source (syn AST, proc_macro2 spans)                    │
└─────────────────────────────────────────────────────────┘

  Dependencies flow inward only.
  No outer layer is referenced by inner layers.
```

### Writing a Custom Detector

Implement the `Detector` trait:

```rust
use crate::analysis::detector::Detector;
use crate::domain::smell::{Smell, SmellCategory, Severity, SourceLocation};
use crate::domain::source::SourceFile;

pub struct MyCustomDetector;

impl Detector for MyCustomDetector {
    fn name(&self) -> &str {
        "My Custom Smell"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        // Inspect file.ast (syn::File) and file.code (raw source)
        for item in &file.ast.items {
            if let syn::Item::Fn(fn_item) = item {
                // Your detection logic here
                if /* condition */ {
                    smells.push(Smell::new(
                        SmellCategory::Implementation,
                        "My Custom Smell",
                        Severity::Warning,
                        SourceLocation {
                            file: file.path.clone(),
                            line_start: fn_item.sig.fn_token.span.start().line,
                            line_end: fn_item.sig.fn_token.span.start().line,
                            column: None,
                        },
                        "Description of the problem".into(),
                        "How to fix it".into(),
                    ));
                }
            }
        }

        smells
    }
}
```

Then register it in `engine.rs`:

```rust
self.register(Box::new(MyCustomDetector));
```

## QualiRS vs Clippy

| Aspect | Clippy | QualiRS |
|---|---|---|
| Focus | Correctness, idioms, style | Structure, architecture, design |
| Granularity | Expression/statement level | Function/module/crate level |
| Configurability | Lint levels (allow/warn/deny) | Numeric thresholds and suppression policy |
| Unsafe analysis | Basic (`unsafe_removed_from_code`) | SAFETY comment enforcement |
| Structural metrics | None | LOC, CC, item count, pub ratio, nesting depth, method chains, lifetimes |
| Overlap | Minimal | Complementary |

## License

MIT
