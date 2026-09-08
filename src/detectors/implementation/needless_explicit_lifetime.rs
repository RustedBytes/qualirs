use crate::analysis::detector::Detector;
use crate::domain::{
    smell::{FindingConfidence, Severity, Smell, SmellCategory, SourceLocation},
    source::SourceFile,
};
use syn::visit::Visit;
pub struct NeedlessExplicitLifetimeDetector;
impl Detector for NeedlessExplicitLifetimeDetector {
    fn name(&self) -> &str {
        "Needless Explicit Lifetime"
    }
    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        file.ast.items.iter().filter_map(|item| {
 let syn::Item::Fn(f) = item else { return None; };
 if !elidable(f) { return None; }
 let line = f.sig.fn_token.span.start().line;
 Some(Smell::new(SmellCategory::Idiomaticity, self.name(), Severity::Info, FindingConfidence::High,
 SourceLocation::new(file.path.clone(), line, line, None),
 "The sole unbounded lifetime only relates a simple input reference to its output",
 "Use Rust's input/output lifetime elision."))
 }).collect()
    }
}
pub(crate) fn elidable(f: &syn::ItemFn) -> bool {
    let sig = &f.sig;
    if sig.generics.params.len() != 1
        || sig.generics.where_clause.is_some()
        || sig.inputs.len() != 1
    {
        return false;
    }
    let Some(syn::GenericParam::Lifetime(lt)) = sig.generics.params.first() else {
        return false;
    };
    if !lt.bounds.is_empty() {
        return false;
    }
    let Some(syn::FnArg::Typed(arg)) = sig.inputs.first() else {
        return false;
    };
    let syn::Type::Reference(input) = &*arg.ty else {
        return false;
    };
    if input.lifetime.as_ref() != Some(&lt.lifetime) || !simple_type(&input.elem) {
        return false;
    }
    if let syn::ReturnType::Type(_, output) = &sig.output {
        match &**output {
            syn::Type::Reference(r)
                if r.lifetime.as_ref() == Some(&lt.lifetime) && simple_type(&r.elem) => {}
            t if simple_type(t) => {}
            _ => return false,
        }
    }
    struct Lifetimes(bool);
    impl<'a> Visit<'a> for Lifetimes {
        fn visit_lifetime(&mut self, _: &'a syn::Lifetime) {
            self.0 = true;
        }
    }
    let mut body = Lifetimes(false);
    body.visit_block(&f.block);
    // Macro tokens can also depend on the explicit lifetime.
    !body.0
        && !quote::ToTokens::to_token_stream(&f.block)
            .to_string()
            .contains(&format!("'{}", lt.lifetime.ident))
}
fn simple_type(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(p) if p.qself.is_none() && p.path.segments.iter().all(|s| matches!(s.arguments, syn::PathArguments::None)))
        || matches!(ty, syn::Type::Tuple(t) if t.elems.is_empty())
}
