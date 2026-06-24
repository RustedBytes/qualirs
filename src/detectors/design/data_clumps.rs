use std::collections::HashMap;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects Data Clumps: sequences of identical parameters repeated across multiple functions.
///
/// If multiple functions take the same group of parameters, they likely belong together
/// in a struct to encapsulate their relationship.
pub struct DataClumpsDetector;

impl Detector for DataClumpsDetector {
    fn name(&self) -> &str {
        "Data Clumps"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        struct ClumpOccurrence {
            fn_name: String,
            line: usize,
        }

        // Map from a signature string to a list of function names
        let mut param_groups: HashMap<String, Vec<ClumpOccurrence>> = HashMap::new();

        for item in &file.ast.items {
            if let syn::Item::Fn(fn_item) = item {
                let sig_string = signature_to_string(&fn_item.sig.inputs);
                if signature_part_count(&sig_string) >= thresholds.design.data_clumps_args {
                    let line = fn_item.sig.ident.span().start().line;
                    param_groups
                        .entry(sig_string)
                        .or_default()
                        .push(ClumpOccurrence {
                            fn_name: fn_item.sig.ident.to_string(),
                            line,
                        });
                }
            } else if let syn::Item::Impl(imp) = item {
                if imp.trait_.is_some() {
                    continue;
                }

                for impl_item in &imp.items {
                    if let syn::ImplItem::Fn(method) = impl_item {
                        let sig_string = signature_to_string(&method.sig.inputs);
                        if signature_part_count(&sig_string) >= thresholds.design.data_clumps_args {
                            let line = method.sig.ident.span().start().line;
                            param_groups
                                .entry(sig_string)
                                .or_default()
                                .push(ClumpOccurrence {
                                    fn_name: method.sig.ident.to_string(),
                                    line,
                                });
                        }
                    }
                }
            }
        }

        for (sig_string, usages) in param_groups {
            if usages.len() >= thresholds.design.data_clumps_occurrences {
                // We'll report it on the first found occurrence for simplicity
                let first_line = usages[0].line;
                let fn_names: Vec<String> = usages.into_iter().map(|u| u.fn_name).collect();
                let param_count = signature_part_count(&sig_string);

                smells.push(Smell::new(
                    SmellCategory::Design,
                    "Data Clumps",
                    Severity::Warning,
                    crate::domain::smell::FindingConfidence::Medium,
                    SourceLocation::new(file.path.clone(), first_line, first_line, None),
                    format!(
                        "Data Clump: Same {} parameters appear in {} functions: {}",
                        param_count,
                        fn_names.len(),
                        fn_names.join(", ")
                    ),
                    "Combine these parameters into a single struct/DTO.",
                ));
            }
        }

        smells
    }
}

fn signature_part_count(sig_string: &str) -> usize {
    if sig_string.is_empty() {
        0
    } else {
        sig_string.split(", ").count()
    }
}

fn signature_to_string(
    inputs: &syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>,
) -> String {
    let mut parts = Vec::with_capacity(inputs.len());
    for input in inputs {
        if let syn::FnArg::Typed(pat_type) = input {
            // Include both name and type roughly, as data clumps usually have the same names too
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                let name = pat_ident.ident.to_string();
                parts.push(format!("{}:[type]", name));
            }
        }
    }
    parts.join(", ")
}
