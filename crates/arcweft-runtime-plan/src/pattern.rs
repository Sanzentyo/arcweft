//! Runtime pattern lowering.

use crate::expr::lower_runtime_expr;
use crate::labels::expr_label;
use arcweft_core::pattern::{RuntimePattern, RuntimeRecordPatternField};
use arcweft_core::value::{RuntimeExpr, RuntimeValue};
use arcweft_lang_hir::syntax::ast::pattern::{Pattern, VariantPatternPayload};

/// Converts parser/HIR patterns into the Sans I/O runtime pattern model.
pub(crate) fn lower_runtime_pattern(pattern: &Pattern) -> RuntimePattern {
    match pattern {
        Pattern::Ident(name) => RuntimePattern::Ident(name.clone()),
        Pattern::MutIdent(name) => RuntimePattern::MutIdent(name.clone()),
        Pattern::Discard => RuntimePattern::Discard,
        Pattern::Literal(expr) => match lower_runtime_expr(expr) {
            RuntimeExpr::Value(value) => RuntimePattern::Literal(value),
            RuntimeExpr::EntityRef(entity) => RuntimePattern::Entity(entity),
            _ => RuntimePattern::Literal(RuntimeValue::String(expr_label(expr))),
        },
        Pattern::Entity(entity) => RuntimePattern::Entity(entity.body().to_owned()),
        Pattern::Tuple(items) => {
            RuntimePattern::Tuple(items.iter().map(lower_runtime_pattern).collect())
        }
        Pattern::Record { path, fields, rest } => RuntimePattern::Record {
            path: path.clone(),
            fields: fields
                .iter()
                .map(|field| RuntimeRecordPatternField {
                    name: field.name().to_owned(),
                    pattern: lower_runtime_pattern(field.pattern()),
                })
                .collect(),
            rest: *rest,
        },
        Pattern::BracketSeq { items, rest } => RuntimePattern::BracketSeq {
            items: items.iter().map(lower_runtime_pattern).collect(),
            rest: rest.clone(),
        },
        Pattern::Variant {
            path,
            name,
            payload,
        } => RuntimePattern::Variant {
            path: path.clone(),
            name: name.clone(),
            payload: payload
                .as_ref()
                .map(|payload| Box::new(lower_runtime_variant_payload(payload))),
        },
        Pattern::Whole { name, pattern } => RuntimePattern::Whole {
            name: name.clone(),
            pattern: Box::new(lower_runtime_pattern(pattern)),
        },
        Pattern::Typed { name, ty } => RuntimePattern::Typed {
            name: name.clone(),
            ty: format!("{ty:?}"),
        },
        Pattern::Raw(raw) => RuntimePattern::Literal(RuntimeValue::String(raw.clone())),
    }
}

fn lower_runtime_variant_payload(payload: &VariantPatternPayload) -> RuntimePattern {
    match payload {
        VariantPatternPayload::Tuple(items) => {
            RuntimePattern::Tuple(items.iter().map(lower_runtime_pattern).collect())
        }
        VariantPatternPayload::Record { fields, rest } => RuntimePattern::Record {
            path: None,
            fields: fields
                .iter()
                .map(|field| RuntimeRecordPatternField {
                    name: field.name().to_owned(),
                    pattern: lower_runtime_pattern(field.pattern()),
                })
                .collect(),
            rest: *rest,
        },
    }
}
