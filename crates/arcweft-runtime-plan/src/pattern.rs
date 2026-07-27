//! Runtime pattern lowering.

use crate::expr::{lower_runtime_expr, lower_runtime_expr_strict};
use crate::labels::expr_label;
use arcweft_core::pattern::{RuntimePattern, RuntimeRecordPatternField};
use arcweft_core::value::{RuntimeExpr, RuntimeValue};
use arcweft_lang_syntax::ast::pattern::{Pattern, VariantPatternPayload};

/// Converts parser/HIR patterns into the Sans I/O runtime pattern model.
pub(crate) fn lower_runtime_pattern(pattern: &Pattern) -> RuntimePattern {
    lower_runtime_pattern_with_policy(pattern, PatternLoweringPolicy::Lossy)
        .expect("lossy pattern lowering is infallible")
}

/// Converts a pattern used by an executable checked boundary without turning
/// unsupported literal/recovery syntax into runtime strings.
pub(crate) fn lower_runtime_pattern_checked(pattern: &Pattern) -> Result<RuntimePattern, String> {
    lower_runtime_pattern_with_policy(pattern, PatternLoweringPolicy::Checked)
}

#[derive(Clone, Copy)]
enum PatternLoweringPolicy {
    Lossy,
    Checked,
}

fn lower_runtime_pattern_with_policy(
    pattern: &Pattern,
    policy: PatternLoweringPolicy,
) -> Result<RuntimePattern, String> {
    match pattern {
        Pattern::Ident(name) => Ok(RuntimePattern::Ident(name.clone())),
        Pattern::MutIdent(name) => Ok(RuntimePattern::MutIdent(name.clone())),
        Pattern::Discard => Ok(RuntimePattern::Discard),
        Pattern::Literal(expr) => lower_literal_pattern(expr, policy),
        Pattern::Entity(entity) => Ok(RuntimePattern::Entity(entity.body().to_owned())),
        Pattern::Tuple(items) => items
            .iter()
            .map(|item| lower_runtime_pattern_with_policy(item, policy))
            .collect::<Result<Vec<_>, _>>()
            .map(RuntimePattern::Tuple),
        Pattern::Record { path, fields, rest } => Ok(RuntimePattern::Record {
            path: path.clone(),
            fields: fields
                .iter()
                .map(|field| {
                    Ok(RuntimeRecordPatternField {
                        name: field.name().to_owned(),
                        pattern: lower_runtime_pattern_with_policy(field.pattern(), policy)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
            rest: *rest,
        }),
        Pattern::BracketSeq { items, rest } => Ok(RuntimePattern::BracketSeq {
            items: items
                .iter()
                .map(|item| lower_runtime_pattern_with_policy(item, policy))
                .collect::<Result<Vec<_>, _>>()?,
            rest: rest.clone(),
        }),
        Pattern::Variant {
            path,
            name,
            payload,
        } => Ok(RuntimePattern::Variant {
            path: path.clone(),
            name: name.clone(),
            payload: payload
                .as_ref()
                .map(|payload| lower_runtime_variant_payload(payload, policy).map(Box::new))
                .transpose()?,
        }),
        Pattern::Whole { name, pattern } => Ok(RuntimePattern::Whole {
            name: name.clone(),
            pattern: Box::new(lower_runtime_pattern_with_policy(pattern, policy)?),
        }),
        Pattern::Typed { name, ty } => Ok(RuntimePattern::Typed {
            name: name.clone(),
            ty: format!("{ty:?}"),
        }),
        Pattern::Raw(raw) => match policy {
            PatternLoweringPolicy::Lossy => {
                Ok(RuntimePattern::Literal(RuntimeValue::String(raw.clone())))
            }
            PatternLoweringPolicy::Checked => {
                Err(format!("raw recovery pattern `{raw}` is not executable"))
            }
        },
    }
}

fn lower_literal_pattern(
    expr: &arcweft_lang_syntax::expr::Expr,
    policy: PatternLoweringPolicy,
) -> Result<RuntimePattern, String> {
    let lowered = match policy {
        PatternLoweringPolicy::Lossy => lower_runtime_expr(expr),
        PatternLoweringPolicy::Checked => lower_runtime_expr_strict(expr)?,
    };
    match lowered {
        RuntimeExpr::Value(value) => Ok(RuntimePattern::Literal(value)),
        RuntimeExpr::EntityRef(entity) => Ok(RuntimePattern::Entity(entity)),
        _ if matches!(policy, PatternLoweringPolicy::Lossy) => Ok(RuntimePattern::Literal(
            RuntimeValue::String(expr_label(expr)),
        )),
        _ => Err(format!(
            "literal pattern `{}` did not lower to a runtime literal or entity",
            expr_label(expr)
        )),
    }
}

fn lower_runtime_variant_payload(
    payload: &VariantPatternPayload,
    policy: PatternLoweringPolicy,
) -> Result<RuntimePattern, String> {
    match payload {
        VariantPatternPayload::Tuple(items) => items
            .iter()
            .map(|item| lower_runtime_pattern_with_policy(item, policy))
            .collect::<Result<Vec<_>, _>>()
            .map(RuntimePattern::Tuple),
        VariantPatternPayload::Record { fields, rest } => Ok(RuntimePattern::Record {
            path: None,
            fields: fields
                .iter()
                .map(|field| {
                    Ok(RuntimeRecordPatternField {
                        name: field.name().to_owned(),
                        pattern: lower_runtime_pattern_with_policy(field.pattern(), policy)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
            rest: *rest,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_pattern_lowering_rejects_raw_recovery_syntax() {
        let error = lower_runtime_pattern_checked(&Pattern::Raw("broken pattern".to_owned()))
            .expect_err("raw patterns are not executable");

        assert!(error.contains("raw recovery pattern"));
    }
}
