use syn::Item;

use crate::analysis::detector::Detector;
use crate::domain::smell::{Severity, Smell, SmellCategory, SourceLocation};
use crate::domain::source::SourceFile;

/// Detects files where too high a ratio of items are `pub`.
pub struct PublicApiExplosionDetector;

impl Detector for PublicApiExplosionDetector {
    fn name(&self) -> &str {
        "Public API Explosion"
    }

    fn detect(&self, file: &SourceFile) -> Vec<Smell> {
        let thresholds = crate::domain::config::current_thresholds();
        let mut smells = Vec::new();

        // Ignore mod.rs/lib.rs boilerplate that only contains module exports or imports
        let is_boilerplate = file.ast.items.iter().all(|item| {
            matches!(
                item,
                syn::Item::Mod(_) | syn::Item::Use(_) | syn::Item::ExternCrate(_)
            )
        });
        if is_boilerplate {
            return smells;
        }

        let items: Vec<_> = file
            .ast
            .items
            .iter()
            .filter(|item| {
                !matches!(
                    item,
                    syn::Item::Use(_) | syn::Item::ExternCrate(_) | syn::Item::Mod(_)
                )
            })
            .collect();

        let total = items.len();
        if total == 0 {
            return smells;
        }

        let pub_count = items.iter().filter(|&&item| is_pub(item)).count();
        let ratio = pub_count as f64 / total as f64;

        if ratio > thresholds.arch.public_api_ratio && total > 5 {
            smells.push(Smell::new(
                SmellCategory::Architecture,
                "Public API Explosion",
                Severity::Info,
                                crate::domain::smell::FindingConfidence::Low,
                SourceLocation {
                    file: file.path.clone(),
                    line_start: 1,
                    line_end: file.line_count,
                    column: None,
                },
                format!(
                    "{:.0}% of items are pub ({}/{}), threshold: {:.0}%",
                    ratio * 100.0, pub_count, total, thresholds.arch.public_api_ratio * 100.0
                ),
                "Reduce public surface. Make items private unless they are part of the intended API.",
            ));
        }

        smells
    }
}

fn is_pub(item: &Item) -> bool {
    let vis = match item {
        Item::Const(i) => &i.vis,
        Item::Enum(i) => &i.vis,
        Item::ExternCrate(i) => &i.vis,
        Item::Fn(i) => &i.vis,
        Item::Mod(i) => &i.vis,
        Item::Static(i) => &i.vis,
        Item::Struct(i) => &i.vis,
        Item::Trait(i) => &i.vis,
        Item::TraitAlias(i) => &i.vis,
        Item::Type(i) => &i.vis,
        Item::Union(i) => &i.vis,
        Item::Use(i) => &i.vis,
        // Items without visibility or with implicit pub
        Item::ForeignMod(_) | Item::Impl(_) | Item::Macro(_) | _ => return false,
    };

    matches!(vis, syn::Visibility::Public(_))
}
