use std::fmt::Write;
use std::path::Path;

use anyhow::{Context, bail};

use crate::analysis::engine::AnalysisReport;
use crate::domain::smell::{Severity, Smell, SourceLocation};

/// Print or write the analysis report as JSON.
pub fn emit_json_report(report: &AnalysisReport, output: Option<&Path>) -> anyhow::Result<()> {
    let json = render_json_report(report)?;
    if let Some(path) = output {
        std::fs::write(path, json)
            .with_context(|| format!("write JSON report to {}", path.display()))?;
    } else {
        println!("{json}");
    }
    Ok(())
}

pub fn render_json_report(report: &AnalysisReport) -> anyhow::Result<String> {
    let mut json = String::new();
    write_report_json(report, &mut json)?;
    validate_generated_json(&json)?;
    Ok(json)
}

fn write_report_json(report: &AnalysisReport, json: &mut String) -> std::fmt::Result {
    write!(json, "{{")?;
    write_summary_json(report, json)?;
    write_smells_json(report, json)?;
    write_parse_errors_json(report, json)?;
    write!(json, "}}")
}

fn write_smells_json(report: &AnalysisReport, json: &mut String) -> std::fmt::Result {
    write!(json, ",\"smells\":[")?;
    for (index, smell) in report.smells.iter().enumerate() {
        if index > 0 {
            write!(json, ",")?;
        }
        write_smell_json(smell, json)?;
    }
    write!(json, "]")
}

fn write_smell_json(smell: &Smell, json: &mut String) -> std::fmt::Result {
    write!(json, "{{")?;
    write_json_field(json, "code", &smell.code)?;
    write!(json, ",")?;
    write_json_field(json, "severity", severity_json(smell.severity))?;
    write!(json, ",")?;
    write_json_field(json, "confidence", &smell.confidence.to_string())?;
    write!(json, ",")?;
    write_json_field(json, "category", &smell.category.to_string())?;
    write!(json, ",")?;
    write_json_field(json, "name", &smell.name)?;
    write!(json, ",\"location\":")?;
    write_location_json(&smell.location, json)?;
    write!(json, ",")?;
    write_json_field(json, "message", &smell.message)?;
    write!(json, ",")?;
    write_json_field(json, "suggestion", &smell.suggestion)?;
    write!(json, "}}")
}

fn write_parse_errors_json(report: &AnalysisReport, json: &mut String) -> std::fmt::Result {
    write!(json, ",\"parse_errors\":[")?;
    for (index, error) in report.parse_errors.iter().enumerate() {
        if index > 0 {
            write!(json, ",")?;
        }
        write!(json, "{{")?;
        write_json_field(json, "message", &error.to_string())?;
        write!(json, "}}")?;
    }
    write!(json, "]")
}

fn write_summary_json(report: &AnalysisReport, json: &mut String) -> std::fmt::Result {
    write!(
        json,
        "\"summary\":{{\"files_analyzed\":{},\"findings\":{}",
        report.total_files,
        report.total_smells()
    )?;
    write!(json, ",\"severity_counts\":{{")?;
    write!(
        json,
        "\"critical\":{},\"warning\":{},\"info\":{}",
        report.count_by_severity(Severity::Critical),
        report.count_by_severity(Severity::Warning),
        report.count_by_severity(Severity::Info)
    )?;
    write!(json, "}}}}")
}

fn write_location_json(location: &SourceLocation, json: &mut String) -> std::fmt::Result {
    write!(json, "{{")?;
    write_json_field(json, "file", &location.file.display().to_string())?;
    write!(
        json,
        ",\"line_start\":{},\"line_end\":{},\"column\":",
        location.line_start, location.line_end
    )?;
    match location.column {
        Some(column) => write!(json, "{column}")?,
        None => write!(json, "null")?,
    }
    write!(json, "}}")
}

fn write_json_field(json: &mut String, key: &str, value: &str) -> std::fmt::Result {
    write_json_string(json, key)?;
    write!(json, ":")?;
    write_json_string(json, value)
}

fn write_json_string(json: &mut String, value: &str) -> std::fmt::Result {
    write!(json, "\"")?;
    for ch in value.chars() {
        write_json_char(json, ch)?;
    }
    write!(json, "\"")
}

fn write_json_char(json: &mut String, ch: char) -> std::fmt::Result {
    match ch {
        '"' => write!(json, "\\\""),
        '\\' => write!(json, "\\\\"),
        '\n' => write!(json, "\\n"),
        '\r' => write!(json, "\\r"),
        '\t' => write!(json, "\\t"),
        ch if ch.is_control() => write!(json, "\\u{:04x}", ch as u32),
        ch => write!(json, "{ch}"),
    }
}

fn severity_json(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical => "critical",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

fn validate_generated_json(json: &str) -> anyhow::Result<()> {
    let bytes = json.as_bytes();
    let end = fixed_json::validate_json(bytes).context("generated report JSON is invalid")?;
    if end != bytes.len() {
        bail!("generated report JSON has trailing bytes at offset {end}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::domain::smell::{Smell, SmellCategory, SourceLocation};

    use super::*;

    #[test]
    fn render_json_report_escapes_strings_and_validates() {
        let report = AnalysisReport::new(
            vec![Smell::new(
                SmellCategory::Implementation,
                "Quoted \"Name\"",
                Severity::Warning,
                crate::domain::smell::FindingConfidence::High,
                SourceLocation::new(PathBuf::from("src\\main.rs"), 7, 9, Some(3)),
                "line one\nline two",
                "Use a tab\tcarefully",
            )],
            1,
            Vec::new(),
        );

        let json = render_json_report(&report).expect("render JSON report");

        assert!(json.contains(r#""name":"Quoted \"Name\"""#));
        assert!(json.contains(r#""code":"Q0000""#));
        assert!(json.contains(r#""confidence":"high""#));
        assert!(json.contains(r#""file":"src\\main.rs""#));
        assert!(fixed_json::validate_json(json.as_bytes()).is_ok());
    }
}
