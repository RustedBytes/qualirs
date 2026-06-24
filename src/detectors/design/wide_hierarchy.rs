use crate::analysis::detector::Detector;
use crate::detectors::policy::{is_dto_template_or_config_struct, is_test_path};
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects enums with too many variants or structs with too many fields.
pub struct WideHierarchyDetector;

impl Detector for WideHierarchyDetector {
    fn name(&self) -> &str {
        "Wide Hierarchy"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        if is_test_path(&file.path) {
            return smells;
        }

        for item in &file.ast.items {
            match item {
                syn::Item::Enum(e) => {
                    let count = e.variants.len();
                    if count > thresholds.design.wide_hierarchy {
                        let line = e.brace_token.span.open().start().line;
                        smells.push(enum_smell(
                            file,
                            &e.ident,
                            count,
                            thresholds.design.wide_hierarchy,
                            line,
                        ));
                    }
                }
                syn::Item::Struct(s) => {
                    if is_threshold_config_struct(&s.ident) || is_dto_template_or_config_struct(s) {
                        continue;
                    }
                    if let syn::Fields::Named(named) = &s.fields {
                        let count = named.named.len();
                        if count > thresholds.design.wide_hierarchy {
                            let line = named.brace_token.span.open().start().line;
                            smells.push(struct_smell(
                                file,
                                &s.ident,
                                count,
                                thresholds.design.wide_hierarchy,
                                line,
                            ));
                        }
                    }
                }
                _ => {}
            }
        }

        smells
    }
}

fn is_threshold_config_struct(ident: &syn::Ident) -> bool {
    ident.to_string().ends_with("Thresholds")
}

fn enum_smell(
    file: &SourceFile,
    ident: &syn::Ident,
    count: usize,
    threshold: usize,
    line: usize,
) -> Smell {
    Smell::new(
        SmellCategory::Design,
        "Wide Hierarchy",
        Severity::Warning,
        crate::domain::smell::FindingConfidence::Medium,
        SourceLocation {
            file: file.path.clone(),
            line_start: line,
            line_end: line,
            column: None,
        },
        format!("Enum `{ident}` has {count} variants (threshold: {threshold})"),
        "Consider grouping variants into sub-enums or using the type-state pattern.",
    )
}

fn struct_smell(
    file: &SourceFile,
    ident: &syn::Ident,
    count: usize,
    threshold: usize,
    line: usize,
) -> Smell {
    Smell::new(
        SmellCategory::Design,
        "Wide Hierarchy",
        Severity::Info,
        crate::domain::smell::FindingConfidence::Medium,
        SourceLocation {
            file: file.path.clone(),
            line_start: line,
            line_end: line,
            column: None,
        },
        format!("Struct `{ident}` has {count} fields (threshold: {threshold})"),
        "Consider grouping related fields into sub-structs.",
    )
}
