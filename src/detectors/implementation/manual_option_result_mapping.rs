use syn::visit::{Visit, visit_expr_match};

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects manual matches that can usually be written with map/map_err/and_then.
pub struct ManualOptionResultMappingDetector;

impl Detector for ManualOptionResultMappingDetector {
    fn name(&self) -> &str {
        "Manual Option/Result Mapping"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let mut visitor = ManualMappingVisitor {
            findings: Vec::new(),
        };
        visitor.visit_file(&file.ast);

        visitor
            .findings
            .into_iter()
            .map(|line| {
                Smell::new(
                    SmellCategory::Idiomaticity,
                    "Manual Option/Result Mapping",
                    Severity::Info,
                                        crate::domain::smell::FindingConfidence::High,
                    SourceLocation::new(file.path.clone(), line, line, None),
                    "Match expression manually maps Option or Result variants",
                    "Use map, map_err, and_then, or the ? operator where it keeps the code clearer.",
                )
            })
            .collect()
    }
}

struct ManualMappingVisitor {
    findings: Vec<usize>,
}

impl<'ast> Visit<'ast> for ManualMappingVisitor {
    fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
        if is_direct_variant_mapping(node) {
            self.findings.push(node.match_token.span.start().line);
        }
        visit_expr_match(self, node);
    }
}

fn is_direct_variant_mapping(node: &syn::ExprMatch) -> bool {
    let Some((first, second)) = two_arm_shapes(node) else {
        return false;
    };

    shapes_are_direct_variant_mapping(first, second)
}

fn two_arm_shapes(node: &syn::ExprMatch) -> Option<(ArmShape, ArmShape)> {
    let [first, second] = node.arms.as_slice() else {
        return None;
    };
    Some((arm_shape(first)?, arm_shape(second)?))
}

fn shapes_are_direct_variant_mapping(first: ArmShape, second: ArmShape) -> bool {
    matches!(
        (first, second),
        (
            ArmShape {
                pattern: Variant::Some,
                body: Variant::Some,
            },
            ArmShape {
                pattern: Variant::None,
                body: Variant::None,
            },
        ) | (
            ArmShape {
                pattern: Variant::None,
                body: Variant::None,
            },
            ArmShape {
                pattern: Variant::Some,
                body: Variant::Some,
            },
        ) | (
            ArmShape {
                pattern: Variant::Ok,
                body: Variant::Ok,
            },
            ArmShape {
                pattern: Variant::Err,
                body: Variant::Err,
            },
        ) | (
            ArmShape {
                pattern: Variant::Err,
                body: Variant::Err,
            },
            ArmShape {
                pattern: Variant::Ok,
                body: Variant::Ok,
            },
        )
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ArmShape {
    pattern: Variant,
    body: Variant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    Some,
    None,
    Ok,
    Err,
}

fn arm_shape(arm: &syn::Arm) -> Option<ArmShape> {
    Some(ArmShape {
        pattern: pat_variant(&arm.pat)?,
        body: expr_variant(&arm.body)?,
    })
}

fn pat_variant(pat: &syn::Pat) -> Option<Variant> {
    match pat {
        syn::Pat::TupleStruct(tuple) => path_variant(&tuple.path),
        syn::Pat::Path(path) => path_variant(&path.path),
        syn::Pat::Ident(ident) if ident.ident == "None" => Some(Variant::None),
        _ => None,
    }
}

fn expr_variant(expr: &syn::Expr) -> Option<Variant> {
    match transparent_expr(expr) {
        syn::Expr::Call(call) => match &*call.func {
            syn::Expr::Path(path) => path_variant(&path.path),
            _ => None,
        },
        syn::Expr::Path(path) => path_variant(&path.path),
        _ => None,
    }
}

fn transparent_expr(expr: &syn::Expr) -> &syn::Expr {
    match expr {
        syn::Expr::Block(block) if block.block.stmts.len() == 1 => match &block.block.stmts[0] {
            syn::Stmt::Expr(expr, _) => transparent_expr(expr),
            _ => expr,
        },
        syn::Expr::Group(group) => transparent_expr(&group.expr),
        syn::Expr::Paren(paren) => transparent_expr(&paren.expr),
        _ => expr,
    }
}

fn path_variant(path: &syn::Path) -> Option<Variant> {
    match path.segments.last()?.ident.to_string().as_str() {
        "Some" => Some(Variant::Some),
        "None" => Some(Variant::None),
        "Ok" => Some(Variant::Ok),
        "Err" => Some(Variant::Err),
        _ => None,
    }
}
