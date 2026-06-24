use syn::visit::{Visit, visit_item_mod};

use crate::analysis::detector::Detector;
use crate::detectors::policy::{has_test_cfg, is_test_path};
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects usage of deeply nested generic types (e.g. `Arc<Mutex<HashMap<String, Vec<T>>>>`).
///
/// Deeply nested generic types are a form of Type Alias Explosion or Primitive Obsession
/// and should be replaced with Newtypes implementing custom methods.
pub struct DeeplyNestedTypeDetector;

impl Detector for DeeplyNestedTypeDetector {
    fn name(&self) -> &str {
        "Deeply Nested Type"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        if is_test_path(&file.path) {
            return smells;
        }

        let mut visitor = TypeVisitor {
            max_depth_threshold: thresholds.r#impl.type_safety.deeply_nested_type,
            violations: Vec::new(),
        };

        visitor.visit_file(&file.ast);

        for violation in visitor.violations {
            smells.push(nested_type_smell(
                file,
                violation,
                thresholds.r#impl.type_safety.deeply_nested_type,
            ));
        }

        smells
    }
}

fn nested_type_smell(
    file: &SourceFile,
    (line, depth, type_str): (usize, usize, String),
    threshold: usize,
) -> Smell {
    Smell::new(
        SmellCategory::Implementation,
        "Deeply Nested Type",
        Severity::Info,
        crate::domain::smell::FindingConfidence::Medium,
        SourceLocation::new(file.path.clone(), line, line, None),
        format!(
            "Type parameter nesting is {depth} levels deep (threshold: {threshold}). Approx type: `{type_str}`"
        ),
        "Wrap complex nested types in a Newtype struct to encapsulate their behavior and clean up signatures.",
    )
}

struct TypeVisitor {
    max_depth_threshold: usize,
    violations: Vec<(usize, usize, String)>,
}

impl<'ast> Visit<'ast> for TypeVisitor {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if has_test_cfg(&node.attrs) {
            return;
        }
        visit_item_mod(self, node);
    }

    fn visit_type(&mut self, node: &'ast syn::Type) {
        let depth = get_type_depth(node);
        if depth > self.max_depth_threshold {
            let line = match node {
                syn::Type::Path(tp) => tp
                    .path
                    .segments
                    .first()
                    .map(|s| s.ident.span().start().line)
                    .unwrap_or(1),
                _ => 1,
            };

            // Generate a rough name representation
            let mut name = String::new();
            if let syn::Type::Path(tp) = node
                && let Some(seg) = tp.path.segments.last()
            {
                name = seg.ident.to_string();
            }
            if name.is_empty() {
                name = "ComplexType".to_string();
            }

            self.violations.push((line, depth, name));
        } else {
            // Only visit children if this parent isn't already failing, to avoid double counting
            syn::visit::visit_type(self, node);
        }
    }
}

fn get_type_depth(ty: &syn::Type) -> usize {
    match ty {
        syn::Type::Path(tp) => {
            if let Some(seg) = tp.path.segments.last() {
                match &seg.arguments {
                    syn::PathArguments::AngleBracketed(args) => {
                        let depths = args.args.iter().map(|arg| match arg {
                            syn::GenericArgument::Type(inner_ty) => get_type_depth(inner_ty),
                            _ => 0,
                        });
                        let inner_max = depths.max().unwrap_or(0);
                        if is_transparent_wrapper(&seg.ident) {
                            inner_max.max(1)
                        } else {
                            1 + inner_max
                        }
                    }
                    _ => 1,
                }
            } else {
                1
            }
        }
        syn::Type::Reference(tr) => get_type_depth(&tr.elem),
        syn::Type::Slice(slice) => get_type_depth(&slice.elem),
        syn::Type::Array(array) => get_type_depth(&array.elem),
        syn::Type::Tuple(tup) => {
            let depths = tup.elems.iter().map(get_type_depth);
            depths.max().unwrap_or(1)
        }
        syn::Type::Paren(paren) => get_type_depth(&paren.elem),
        syn::Type::Group(group) => get_type_depth(&group.elem),
        _ => 1,
    }
}

fn is_transparent_wrapper(ident: &syn::Ident) -> bool {
    matches!(ident.to_string().as_str(), "Option" | "Result")
}
