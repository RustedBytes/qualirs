use syn::visit::{Visit, visit_local};

use crate::analysis::detector::Detector;
use crate::detectors::implementation::perf_utils::{pat_ident, type_contains_dyn, type_path_tail};
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects local dynamic dispatch where a concrete or generic type may be enough.
pub struct NeedlessDynamicDispatchDetector;

impl Detector for NeedlessDynamicDispatchDetector {
    fn name(&self) -> &str {
        "Needless Dynamic Dispatch"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = DynamicDispatchVisitor {
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|(line, name)| {
                Smell::new(
                    SmellCategory::Performance,
                    "Needless Dynamic Dispatch",
                    Severity::Info,
                                        crate::domain::smell::FindingConfidence::Low,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    format!("Local binding `{name}` uses `dyn Trait` dispatch"),
                    "Use a concrete type or a generic parameter when this local code does not require heterogeneous values.",
                )
            })
            .collect()
    }
}

struct DynamicDispatchVisitor {
    findings: Vec<(usize, String)>,
}

impl<'ast> Visit<'ast> for DynamicDispatchVisitor {
    fn visit_local(&mut self, node: &'ast syn::Local) {
        if let syn::Pat::Type(pat_type) = &node.pat
            && type_contains_dyn(&pat_type.ty)
            && !is_dyn_collection(&pat_type.ty)
            && node
                .init
                .as_ref()
                .is_some_and(|init| initializer_is_single_concrete_value(&init.expr))
            && let Some(name) = pat_ident(&node.pat)
        {
            self.findings
                .push((pat_type.colon_token.span.start().line, name));
        }

        visit_local(self, node);
    }
}

fn is_dyn_collection(ty: &syn::Type) -> bool {
    type_path_tail(ty).is_some_and(|tail| {
        matches!(
            tail.as_str(),
            "Vec" | "HashMap" | "HashSet" | "BTreeMap" | "BTreeSet"
        )
    })
}

fn initializer_is_single_concrete_value(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Call(_) | syn::Expr::Struct(_) | syn::Expr::Reference(_) => true,
        syn::Expr::MethodCall(call) => call.method == "into",
        _ => false,
    }
}
