use syn::visit::{Visit, visit_item_impl, visit_item_mod, visit_item_trait};

use crate::analysis::detector::Detector;
use crate::detectors::policy::has_test_cfg;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects usage of the `#[async_trait]` macro.
///
/// Under modern Rust (>= 1.75), native async traits are stabilized.
/// The `async_trait` macro introduces a performance penalty by boxing the Future
/// and using dynamic dispatch, which is often unnecessary now.
pub struct AsyncTraitOverheadDetector;

impl Detector for AsyncTraitOverheadDetector {
    fn name(&self) -> &str {
        "Async Trait Overhead"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut smells = Vec::new();

        let mut visitor = MacroAttributeVisitor {
            violations: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        for line in visitor.violations {
            smells.push(Smell::new(
                SmellCategory::Performance,
                "Async Trait Overhead",
                Severity::Info,
                crate::domain::smell::FindingConfidence::Low,
                SourceLocation::new(file.path.clone(), line, line, None),
                "Usage of `#[async_trait]` macro incurs unnecessary Future boxing overhead"
                    .to_string(),
                "Migrate to native async fn in traits (stabilized in Rust 1.75) if possible.",
            ));
        }

        smells
    }
}

struct MacroAttributeVisitor {
    violations: Vec<usize>,
}

impl<'ast> Visit<'ast> for MacroAttributeVisitor {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if has_test_cfg(&node.attrs) {
            return;
        }
        visit_item_mod(self, node);
    }

    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        if has_test_cfg(&node.attrs) {
            return;
        }
        if let Some(line) = async_trait_attr_line(&node.attrs)
            && !trait_is_likely_dyn_port(node)
        {
            self.violations.push(line);
        }
        visit_item_trait(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if has_test_cfg(&node.attrs) {
            return;
        }
        visit_item_impl(self, node);
    }
}

fn async_trait_attr_line(attrs: &[syn::Attribute]) -> Option<usize> {
    attrs.iter().find_map(|attr| {
        let seg = attr.path().segments.last()?;
        (seg.ident == "async_trait").then(|| seg.ident.span().start().line)
    })
}

fn trait_is_likely_dyn_port(item: &syn::ItemTrait) -> bool {
    item.supertraits.iter().any(|bound| {
        matches!(
            bound,
            syn::TypeParamBound::Trait(trait_bound)
                if trait_bound
                    .path
                    .segments
                    .last()
                    .is_some_and(|segment| matches!(segment.ident.to_string().as_str(), "Send" | "Sync"))
        )
    })
}
