use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects extern blocks (FFI declarations) without corresponding safe wrappers.
///
/// Raw FFI functions should be wrapped in safe Rust functions that validate
/// inputs and handle errors properly.
pub struct FfiWithoutWrapperDetector;

impl Detector for FfiWithoutWrapperDetector {
    fn name(&self) -> &str {
        "FFI Without Wrapper"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        // Collect extern function names
        let mut extern_fns: Vec<(String, usize)> = Vec::new();
        // Collect all pub fn names that could be wrappers
        let mut safe_wrappers: std::collections::HashSet<String> = std::collections::HashSet::new();

        for item in &file.ast.items {
            match item {
                syn::Item::ForeignMod(fm) => {
                    for item in &fm.items {
                        if let syn::ForeignItem::Fn(fn_decl) = item {
                            let name = fn_decl.sig.ident.to_string();
                            let line = fn_decl.sig.fn_token.span.start().line;
                            extern_fns.push((name, line));
                        }
                    }
                }
                syn::Item::Fn(fn_item)
                    // A safe wrapper is a non-extern pub fn
                    if fn_item.sig.unsafety.is_none() => {
                        safe_wrappers.insert(fn_item.sig.ident.to_string());
                    }
                _ => {}
            }
        }

        for (name, line) in &extern_fns {
            // Check if there's a corresponding safe wrapper
            let has_wrapper = safe_wrappers.iter().any(|w| {
                // Common patterns: ffi_name -> ffi_name_wrapper, safe_ffi_name, wrap_ffi_name
                w.contains(name) || name.contains(w)
            });

            if !has_wrapper {
                smells.push(Smell::new(
                    SmellCategory::Unsafe,
                    "FFI Without Wrapper",
                    Severity::Warning,
                    crate::domain::smell::FindingConfidence::High,
                    SourceLocation {
                        file: file.path.clone(),
                        line_start: *line,
                        line_end: *line,
                        column: None,
                    },
                    format!(
                        "FFI function `{}` has no safe Rust wrapper in this file",
                        name
                    ),
                    "Create a safe wrapper function that validates inputs and handles errors.",
                ));
            }
        }

        smells
    }
}
