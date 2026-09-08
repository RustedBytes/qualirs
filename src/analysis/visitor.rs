// Convenience module for AST visitor utilities.
// Detectors should iterate file.ast.items directly for simple traversals.
// Use syn::visit::Visit for recursive AST traversal.

use syn::visit::Visit;

/// Enumerate named execution bodies independently, including nested helpers and
/// methods. Metrics on each body must exclude nested execution contexts.
pub(crate) fn visit_functions(
    file: &syn::File,
    callback: impl FnMut(&syn::Signature, &syn::Block),
) {
    struct Functions<F>(F);
    impl<'ast, F: FnMut(&syn::Signature, &syn::Block)> Visit<'ast> for Functions<F> {
        fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
            (self.0)(&node.sig, &node.block);
            syn::visit::visit_item_fn(self, node);
        }
        fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
            (self.0)(&node.sig, &node.block);
            syn::visit::visit_impl_item_fn(self, node);
        }
        fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
            if let Some(block) = &node.default {
                (self.0)(&node.sig, block);
            }
            syn::visit::visit_trait_item_fn(self, node);
        }
    }
    Functions(callback).visit_file(file);
}
