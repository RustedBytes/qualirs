use syn::spanned::Spanned;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects impl blocks that contain methods inconsistent with the type's naming.
///
/// For example, a `UserValidator` should not have `save_to_database()` methods.
/// This detector looks for impl blocks on types with clear responsibilities
/// that contain methods leaking into other responsibility domains.
pub struct RebelliousImplDetector;

impl Detector for RebelliousImplDetector {
    fn name(&self) -> &str {
        "Rebellious Impl"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        for item in &file.ast.items {
            if let syn::Item::Impl(imp) = item {
                if imp.trait_.is_some() {
                    continue; // Skip trait impls
                }

                if let syn::Type::Path(tp) = &*imp.self_ty
                    && let Some(seg) = tp.path.segments.last()
                {
                    let type_name = seg.ident.to_string().to_lowercase();

                    // Collect all method names
                    let methods: Vec<String> = imp
                        .items
                        .iter()
                        .filter_map(|item| {
                            if let syn::ImplItem::Fn(method) = item {
                                Some(method.sig.ident.to_string())
                            } else {
                                None
                            }
                        })
                        .collect();

                    let rebellion = detect_rebellion(&type_name, &methods);
                    if rebellion {
                        let line = imp.self_ty.span().start().line;
                        let method_list: Vec<&str> = methods.iter().map(|m| m.as_str()).collect();

                        smells.push(Smell::new(
                                SmellCategory::Design,
                                "Rebellious Impl",
                                Severity::Info,
                                                                crate::domain::smell::FindingConfidence::Low,
                                SourceLocation {
                                    file: file.path.clone(),
                                    line_start: line,
                                    line_end: line,
                                    column: None,
                                },
                                format!(
                                    "Impl for `{}` contains methods that seem outside its responsibility: {}",
                                    seg.ident,
                                    method_list.join(", ")
                                ),
                                "Consider extracting unrelated methods into separate types or modules.",
                            ));
                    }
                }
            }
        }

        smells
    }
}

fn detect_rebellion(type_name: &str, methods: &[String]) -> bool {
    // Database-related type should not have UI/print methods
    if type_name.contains("repo") || type_name.contains("database") || type_name.contains("store") {
        let io_methods = methods.iter().any(|m| {
            let m = m.to_lowercase();
            m.contains("print")
                || m.contains("render")
                || m.contains("display")
                || m.contains("format")
        });
        return io_methods;
    }

    // Validator should not have persistence methods
    if type_name.contains("validator") || type_name.contains("checker") {
        let persist = methods.iter().any(|m| {
            let m = m.to_lowercase();
            m.contains("save")
                || m.contains("delete")
                || m.contains("insert")
                || m.contains("update")
        });
        return persist;
    }

    // Handler/controller with too many methods (catch-all)
    if type_name.contains("handler") || type_name.contains("controller") {
        return methods.len() > 10;
    }

    false
}
