use colored::*;
use comfy_table::{Cell, Color as TableColor, Table, presets::UTF8_FULL};

use crate::analysis::engine::AnalysisReport;
use crate::cli::llm_snippet::{print_fenced_code, source_snippet, source_snippet_with_context};
use crate::cli::suggested_code::suggested_code;
use crate::domain::smell::{Severity, Smell, SmellCategory};

/// Print the full analysis report to stdout.
pub fn print_report(report: &AnalysisReport) {
    print_header();
    print_summary(report);

    if !report.smells.is_empty() {
        println!();
        print_smell_table(report);
    }

    if !report.parse_errors.is_empty() {
        println!();
        print_parse_errors(report);
    }

    print_footer(report);
}

/// Print findings as a compact categorized list.
pub fn print_compact_report(report: &AnalysisReport) {
    print_header();
    print_summary(report);

    if !report.smells.is_empty() {
        println!();
        print_compact_smells(report);
    }

    if !report.parse_errors.is_empty() {
        println!();
        print_parse_errors(report);
    }

    print_footer(report);
}

/// Print findings with source snippets and improvement guidance.
pub fn print_how_fix_report(report: &AnalysisReport) {
    print_header();
    print_summary(report);

    if !report.smells.is_empty() {
        println!();
        print_how_fix_smells(report);
    }

    if !report.parse_errors.is_empty() {
        println!();
        print_parse_errors(report);
    }

    print_footer(report);
}

/// Print findings as paste-friendly Markdown for coding assistants.
pub fn print_llm_report(report: &AnalysisReport) {
    println!("# QualiRS Findings");
    println!();
    println!(
        "Fix the following Rust code smells. Preserve existing behavior and keep changes focused."
    );
    println!();
    println!("- Files analyzed: {}", report.total_files);
    println!("- Findings: {}", report.total_smells());
    println!(
        "- Severity counts: critical={}, warning={}, info={}",
        report.count_by_severity(Severity::Critical),
        report.count_by_severity(Severity::Warning),
        report.count_by_severity(Severity::Info),
    );

    if report.smells.is_empty() {
        println!();
        println!("No findings.");
    } else {
        println!();
        print_llm_smells(report);
    }

    if !report.parse_errors.is_empty() {
        println!();
        println!("## Parse Errors");
        for error in &report.parse_errors {
            println!();
            println!("```text");
            println!("{error}");
            println!("```");
        }
    }
}

fn print_header() {
    println!();
    println!(
        "{} {}",
        "QualiRS".bright_cyan().bold(),
        "— Rust Code Smell Detector".dimmed()
    );
    println!("{}", "━".repeat(60).dimmed());
}

fn print_summary(report: &AnalysisReport) {
    let total = report.total_smells();
    let critical = report.count_by_severity(Severity::Critical);
    let warnings = report.count_by_severity(Severity::Warning);
    let info = report.count_by_severity(Severity::Info);

    println!();
    println!(
        "  {} {} files analyzed, {} smell(s) detected",
        "→".bright_cyan(),
        report.total_files.to_string().bold(),
        total.to_string().bold()
    );

    if total > 0 {
        println!(
            "    {} critical  {} warning  {} info",
            critical.to_string().red().bold(),
            warnings.to_string().yellow().bold(),
            info.to_string().blue().bold()
        );
    }
}

fn print_llm_smells(report: &AnalysisReport) {
    let categories = [
        SmellCategory::Architecture,
        SmellCategory::Design,
        SmellCategory::Implementation,
        SmellCategory::Performance,
        SmellCategory::Idiomaticity,
        SmellCategory::Concurrency,
        SmellCategory::Unsafe,
    ];

    for category in categories {
        let mut smells: Vec<_> = report
            .smells
            .iter()
            .filter(|smell| smell.category == category)
            .collect();

        if smells.is_empty() {
            continue;
        }

        smells.sort_by(|a, b| {
            b.severity
                .cmp(&a.severity)
                .then_with(|| a.location.cmp(&b.location))
                .then_with(|| a.name.cmp(&b.name))
        });

        println!("## {category}");

        for smell in smells {
            println!();
            println!("```text");
            println!("Severity: {}", smell.severity);
            println!("Code: {}", smell.code);
            println!("Category: {}", smell.category);
            println!("Smell: {}", smell.name);
            println!("Location: {}", smell.location);
            println!("Message: {}", smell.message);
            println!("Suggestion: {}", smell.suggestion);
            println!("```");

            if let Some(snippet) = source_snippet(&smell.location) {
                println!();
                print_fenced_code("rust", &snippet);
            }
        }

        println!();
    }
}

fn print_compact_smells(report: &AnalysisReport) {
    let categories = [
        SmellCategory::Architecture,
        SmellCategory::Design,
        SmellCategory::Implementation,
        SmellCategory::Performance,
        SmellCategory::Idiomaticity,
        SmellCategory::Concurrency,
        SmellCategory::Unsafe,
    ];

    for category in categories {
        let mut smells: Vec<_> = report
            .smells
            .iter()
            .filter(|smell| smell.category == category)
            .collect();

        if smells.is_empty() {
            continue;
        }

        smells.sort_by(|a, b| {
            b.severity
                .cmp(&a.severity)
                .then_with(|| a.location.cmp(&b.location))
                .then_with(|| a.name.cmp(&b.name))
        });

        println!(
            "{} {}",
            "▸".bright_magenta(),
            compact_category_label(&category).bold()
        );

        for smell in smells {
            println!(
                "  {} {} {} {}",
                compact_severity_label(&smell.severity),
                smell.code.cyan().bold(),
                smell.name.bold(),
                smell.location.to_string().dimmed()
            );
            println!("    {}", smell.message);
        }

        println!();
    }
}

fn print_how_fix_smells(report: &AnalysisReport) {
    let categories = [
        SmellCategory::Architecture,
        SmellCategory::Design,
        SmellCategory::Implementation,
        SmellCategory::Performance,
        SmellCategory::Idiomaticity,
        SmellCategory::Concurrency,
        SmellCategory::Unsafe,
    ];

    for category in categories {
        let mut smells: Vec<_> = report
            .smells
            .iter()
            .filter(|smell| smell.category == category)
            .collect();

        if smells.is_empty() {
            continue;
        }

        smells.sort_by(|a, b| {
            b.severity
                .cmp(&a.severity)
                .then_with(|| a.location.cmp(&b.location))
                .then_with(|| a.name.cmp(&b.name))
        });

        println!(
            "{} {}",
            "▸".bright_magenta(),
            compact_category_label(&category).bold()
        );

        let total = smells.len();
        for (index, smell) in smells.into_iter().enumerate() {
            print_how_fix_smell(smell, index + 1, total);
        }

        println!();
    }
}

fn print_how_fix_smell(smell: &Smell, index: usize, total: usize) {
    println!(
        "  {} {} {} {} {}",
        "╭─".dimmed(),
        format!("Finding {index}/{total}").bold(),
        compact_severity_label(&smell.severity),
        smell.code.bright_cyan().bold(),
        smell.name.bold()
    );
    println!(
        "  {} {} {}",
        "│".dimmed(),
        "Location".bright_black().bold(),
        smell.location.to_string().dimmed()
    );
    println!("  {}", "│".dimmed());

    print_how_fix_section("Problem", &smell.message);
    print_how_fix_section("Fix", &smell.suggestion);
    print_how_fix_section("Why", enhancement_explanation(smell));

    println!(
        "  {} {}",
        "│".dimmed(),
        "Current code".bright_black().bold()
    );
    if let Some(snippet) = source_snippet_with_context(&smell.location, 2) {
        print_source_excerpt(&snippet);
    } else {
        println!(
            "  {}   {}",
            "│".dimmed(),
            "source snippet unavailable; the file may have moved or been deleted".dimmed()
        );
    }

    if let Some(suggestion) = suggested_code(smell) {
        println!(
            "  {} {}",
            "│".dimmed(),
            "Suggested code".bright_black().bold()
        );
        print_code_candidate(&suggestion);
    }

    println!("  {}", "╰─".dimmed());
}

fn enhancement_explanation(smell: &Smell) -> &'static str {
    match smell.category {
        SmellCategory::Architecture => {
            "Refactor toward clearer boundaries so dependencies and module responsibilities stay easy to reason about."
        }
        SmellCategory::Design => {
            "Move behavior or data into a shape that makes the type's responsibility explicit and easier to extend."
        }
        SmellCategory::Implementation => {
            "Prefer the simpler Rust construct so the code keeps the same behavior with less maintenance overhead."
        }
        SmellCategory::Performance => {
            "Remove avoidable work or allocation while keeping the observable behavior unchanged."
        }
        SmellCategory::Idiomaticity => {
            "Use the common Rust idiom so intent is obvious to future readers and reviewers."
        }
        SmellCategory::Concurrency => {
            "Make ownership, scheduling, or locking behavior explicit so concurrent code is less likely to block or race unexpectedly."
        }
        SmellCategory::Unsafe => {
            "Narrow and document the unsafe boundary so the invariants are auditable at the call site."
        }
    }
}

fn print_how_fix_section(label: &str, text: &str) {
    println!("  {} {}", "│".dimmed(), label.bright_black().bold());
    for line in text.lines() {
        println!("  {}   {}", "│".dimmed(), line);
    }
    println!("  {}", "│".dimmed());
}

fn print_source_excerpt(snippet: &str) {
    for line in snippet.lines() {
        let (selected, rest) = if let Some(rest) = line.strip_prefix("> ") {
            (true, rest)
        } else if let Some(rest) = line.strip_prefix("  ") {
            (false, rest)
        } else {
            (false, line)
        };

        let marker = if selected {
            "▶".yellow().bold()
        } else {
            " ".normal()
        };
        let Some((gutter, source)) = rest.split_once('|') else {
            println!("  {}   {} {}", "│".dimmed(), marker, rest);
            continue;
        };

        let source = source.strip_prefix(' ').unwrap_or(source);

        if selected {
            println!(
                "  {}   {} {}{}{}",
                "│".dimmed(),
                marker,
                gutter.trim_end().yellow().bold(),
                " │ ".yellow().bold(),
                source.bold()
            );
        } else {
            println!(
                "  {}   {} {}{}{}",
                "│".dimmed(),
                marker,
                gutter.trim_end().dimmed(),
                " │ ".dimmed(),
                source
            );
        }
    }
    println!("  {}", "│".dimmed());
}

fn print_code_candidate(code: &str) {
    for line in code.lines() {
        println!(
            "  {}   {} {}",
            "│".dimmed(),
            "+".green().bold(),
            line.green()
        );
    }
    println!("  {}", "│".dimmed());
}

fn print_smell_table(report: &AnalysisReport) {
    let mut table = Table::new();
    table.load_style(UTF8_FULL);
    table.set_header(vec![
        Cell::new("Severity"),
        Cell::new("Category"),
        Cell::new("Smell"),
        Cell::new("Location"),
        Cell::new("Message"),
    ]);

    let mut smells = report.smells.clone();
    smells.sort_by_key(|smell| std::cmp::Reverse(smell.severity));

    for smell in &smells {
        let sev_cell = severity_cell(&smell.severity);
        let cat_cell = category_cell(&smell.category);

        table.add_row(vec![
            sev_cell,
            cat_cell,
            Cell::new(format!("{} {}", smell.code, smell.name)),
            Cell::new(smell.location.to_string()),
            Cell::new(format!(
                "{}\n{}",
                smell.message.bold(),
                smell.suggestion.dimmed()
            )),
        ]);
    }

    println!("{table}");
}

fn compact_severity_label(severity: &Severity) -> colored::ColoredString {
    match severity {
        Severity::Critical => "CRIT".red().bold(),
        Severity::Warning => "WARN".yellow().bold(),
        Severity::Info => "INFO".blue().bold(),
    }
}

fn compact_category_label(category: &SmellCategory) -> colored::ColoredString {
    match category {
        SmellCategory::Architecture => category.to_string().magenta(),
        SmellCategory::Design => category.to_string().cyan(),
        SmellCategory::Implementation => category.to_string().green(),
        SmellCategory::Performance => category.to_string().blue(),
        SmellCategory::Idiomaticity => category.to_string().white(),
        SmellCategory::Concurrency => category.to_string().yellow(),
        SmellCategory::Unsafe => category.to_string().red(),
    }
}

fn severity_cell(severity: &Severity) -> Cell {
    match severity {
        Severity::Critical => Cell::new("CRIT").fg(TableColor::Red),
        Severity::Warning => Cell::new("WARN").fg(TableColor::Yellow),
        Severity::Info => Cell::new("INFO").fg(TableColor::Blue),
    }
}

fn category_cell(category: &SmellCategory) -> Cell {
    let color = match category {
        SmellCategory::Architecture => TableColor::Magenta,
        SmellCategory::Design => TableColor::Cyan,
        SmellCategory::Implementation => TableColor::Green,
        SmellCategory::Performance => TableColor::Blue,
        SmellCategory::Idiomaticity => TableColor::White,
        SmellCategory::Concurrency => TableColor::Yellow,
        SmellCategory::Unsafe => TableColor::Red,
    };
    Cell::new(category.to_string()).fg(color)
}

fn print_parse_errors(report: &AnalysisReport) {
    println!(
        "{}",
        "Parse errors (files could not be analyzed):"
            .yellow()
            .bold()
    );
    for error in &report.parse_errors {
        println!("  {} {error}", "✗".red());
    }
}

fn print_footer(report: &AnalysisReport) {
    println!("{}", "━".repeat(60).dimmed());
    if report.total_smells() == 0 {
        println!("{}", "  No smells detected. Clean code!".green().bold());
    } else {
        let total = report.total_smells();
        let critical = report.count_by_severity(Severity::Critical);
        if critical > 0 {
            println!("  Found {total} smell(s), {critical} critical — consider refactoring.",);
        } else {
            println!("  Found {total} smell(s). Review warnings above.");
        }
    }
    println!();
}
