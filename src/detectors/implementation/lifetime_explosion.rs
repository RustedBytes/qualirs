use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects functions, structs, and enums with too many lifetime parameters.
///
/// Excessive lifetimes make code hard to read and maintain. They often
/// indicate a need to restructure data ownership.
pub struct LifetimeExplosionDetector;

impl Detector for LifetimeExplosionDetector {
    fn name(&self) -> &str {
        "Lifetime Explosion"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        for item in &file.ast.items {
            match item {
                syn::Item::Fn(fn_item) => {
                    let count = count_lifetimes(&fn_item.sig.generics);
                    if count > thresholds.r#impl.control_flow.lifetime_explosion {
                        let line = fn_item.sig.fn_token.span.start().line;
                        smells.push(make_smell(
                            &file.path,
                            line,
                            &format!("Function `{}`", fn_item.sig.ident),
                            count,
                            thresholds.r#impl.control_flow.lifetime_explosion,
                        ));
                    }
                }
                syn::Item::Struct(s) => {
                    let count = count_lifetimes(&s.generics);
                    if count > thresholds.r#impl.control_flow.lifetime_explosion {
                        let line = s.struct_token.span.start().line;
                        smells.push(make_smell(
                            &file.path,
                            line,
                            &format!("Struct `{}`", s.ident),
                            count,
                            thresholds.r#impl.control_flow.lifetime_explosion,
                        ));
                    }
                }
                syn::Item::Enum(e) => {
                    let count = count_lifetimes(&e.generics);
                    if count > thresholds.r#impl.control_flow.lifetime_explosion {
                        let line = e.enum_token.span.start().line;
                        smells.push(make_smell(
                            &file.path,
                            line,
                            &format!("Enum `{}`", e.ident),
                            count,
                            thresholds.r#impl.control_flow.lifetime_explosion,
                        ));
                    }
                }
                _ => {}
            }
        }

        smells
    }
}

fn count_lifetimes(generics: &syn::Generics) -> usize {
    generics
        .params
        .iter()
        .filter(|p| matches!(p, syn::GenericParam::Lifetime(_)))
        .count()
}

fn make_smell(
    file_path: &std::path::Path,
    line: usize,
    context: &str,
    count: usize,
    threshold: usize,
) -> Smell {
    Smell::new(
        SmellCategory::Implementation,
        "Lifetime Explosion",
        Severity::Warning,
        crate::domain::smell::FindingConfidence::Medium,
        SourceLocation {
            file: file_path.to_path_buf(),
            line_start: line,
            line_end: line,
            column: None,
        },
        format!("{context} has {count} lifetime parameters (threshold: {threshold})"),
        "Reduce lifetimes by restructuring ownership. Use owned types, Arc, or restructure data flow.",
    )
}
