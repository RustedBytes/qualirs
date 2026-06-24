use syn::visit::{Visit, visit_expr_closure};

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects nested closures that make control flow hard to follow.
pub struct DeepClosureNestingDetector;

impl Detector for DeepClosureNestingDetector {
    fn name(&self) -> &str {
        "Deep Closure Nesting"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let threshold = crate::domain::config::current_thresholds()
            .r#impl
            .control_flow
            .deep_closure_nesting;
        let mut visitor = ClosureDepthVisitor {
            threshold,
            depth: 0,
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|(line, depth)| {
                Smell::new(
                    SmellCategory::Implementation,
                    "Deep Closure Nesting",
                    Severity::Info,
                    crate::domain::smell::FindingConfidence::Medium,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    format!("Closure nesting depth is {depth} (threshold: {threshold})"),
                    "Extract nested closures into named functions or flatten the control flow.",
                )
            })
            .collect()
    }
}

struct ClosureDepthVisitor {
    threshold: usize,
    depth: usize,
    findings: Vec<(usize, usize)>,
}

impl<'ast> Visit<'ast> for ClosureDepthVisitor {
    fn visit_expr_closure(&mut self, node: &'ast syn::ExprClosure) {
        self.depth += 1;
        if self.depth > self.threshold {
            self.findings
                .push((node.or1_token.span.start().line, self.depth));
        }
        visit_expr_closure(self, node);
        self.depth -= 1;
    }
}
