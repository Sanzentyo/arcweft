//! Stable schema identities emitted by component View lowering.
//!
//! These helpers canonicalize syntax into deterministic digest input. They do
//! not inspect lowering state, so keeping them separate prevents layout and
//! input-resource concerns from becoming part of schema identity generation.

use arcweft_bundle::{container::BundleDigest, resource_codec::types::DigestRef};
use arcweft_lang_syntax::{
    ast::{
        pattern::{Pattern, VariantPatternPayload},
        view::{ViewForEach, ViewMatchArm},
    },
    expr::Expr,
};

use super::bundle_view::expr_source;

pub(in crate::app) fn expr_schema_ref(expr: &Expr) -> DigestRef {
    schema_ref_for_source(&expr_source(expr))
}

pub(in crate::app) fn match_arm_schema_ref(scrutinee: &Expr, arm: &ViewMatchArm) -> DigestRef {
    let guard = arm.guard().map(expr_source).unwrap_or_default();
    schema_ref_for_source(&format!(
        "match:{}=>{} when {}",
        expr_source(scrutinee),
        pattern_schema_source(arm.pattern()),
        guard
    ))
}

pub(in crate::app) fn repeat_key_schema_ref(view_for_each: &ViewForEach) -> DigestRef {
    view_for_each.key().map_or_else(
        || {
            schema_ref_for_source(&format!(
                "source_order:{} in {}",
                pattern_schema_source(view_for_each.pattern()),
                expr_source(view_for_each.source())
            ))
        },
        expr_schema_ref,
    )
}

pub(in crate::app) fn schema_ref_for_source(source: &str) -> DigestRef {
    DigestRef {
        digest: BundleDigest::of(source.as_bytes()),
    }
}

pub(in crate::app) fn pattern_schema_source(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Ident(name) => name.clone(),
        Pattern::MutIdent(name) => format!("mut {name}"),
        Pattern::Literal(expr) => expr_source(expr),
        Pattern::Entity(entity) => entity.body().to_owned(),
        Pattern::Variant {
            path,
            name,
            payload,
        } => format!(
            "{}{}{}",
            path.as_ref().map_or("", String::as_str),
            name,
            payload
                .as_ref()
                .map_or_else(String::new, variant_pattern_payload_source)
        ),
        Pattern::Discard => "_".to_owned(),
        Pattern::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(pattern_schema_source)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Pattern::Record { path, fields, rest } => {
            let mut fields = fields
                .iter()
                .map(|field| {
                    format!(
                        "{}: {}",
                        field.name(),
                        pattern_schema_source(field.pattern())
                    )
                })
                .collect::<Vec<_>>();
            if *rest {
                fields.push("..".to_owned());
            }
            format!(
                "{}{{{}}}",
                path.as_ref().map_or("", String::as_str),
                fields.join(", ")
            )
        }
        Pattern::BracketSeq { items, rest } => {
            let mut items = items.iter().map(pattern_schema_source).collect::<Vec<_>>();
            if let Some(rest) = rest {
                items.push(format!("..{rest}"));
            }
            format!("[{}]", items.join(", "))
        }
        Pattern::Whole { name, pattern } => format!("{name} @ {}", pattern_schema_source(pattern)),
        Pattern::Typed { name, ty } => format!("{name}: {ty:?}"),
        Pattern::Raw(source) => source.clone(),
    }
}

fn variant_pattern_payload_source(payload: &VariantPatternPayload) -> String {
    match payload {
        VariantPatternPayload::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(pattern_schema_source)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        VariantPatternPayload::Record { fields, rest } => {
            let mut fields = fields
                .iter()
                .map(|field| {
                    format!(
                        "{}: {}",
                        field.name(),
                        pattern_schema_source(field.pattern())
                    )
                })
                .collect::<Vec<_>>();
            if *rest {
                fields.push("..".to_owned());
            }
            format!("{{{}}}", fields.join(", "))
        }
    }
}
