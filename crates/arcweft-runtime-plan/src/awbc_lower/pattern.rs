use crate::awbc_lower::frame::FrameBuilder;
use crate::awbc_lower::inventory::AwbcInventory;
use crate::awbc_lower::table_index;
use arcweft_core::awbc::schema::{AwbcPattern, AwbcPatternId, AwbcRecordPatternField, AwbcTypeId};
use arcweft_core::pattern::{RuntimePattern, RuntimeRecordPatternField};

/// Lowers runtime patterns into executable AWBC pattern graph nodes.
pub(crate) fn lower_pattern(
    inventory: &mut AwbcInventory,
    frame: &mut FrameBuilder,
    pattern: &RuntimePattern,
) -> AwbcPatternId {
    match pattern {
        RuntimePattern::Ident(name) | RuntimePattern::MutIdent(name) => {
            let name_id = inventory.intern_string(name);
            let register = frame.local(name, name_id, inventory.dynamic_ty());
            inventory.intern_pattern(AwbcPattern::Bind {
                target: register,
                mutable: matches!(pattern, RuntimePattern::MutIdent(_)),
                expected: None,
            })
        }
        RuntimePattern::Typed { name, ty } => {
            let name_id = inventory.intern_string(name);
            let register = frame.local(name, name_id, inventory.dynamic_ty());
            let expected = Some(type_label(inventory, ty));
            inventory.intern_pattern(AwbcPattern::Bind {
                target: register,
                mutable: false,
                expected,
            })
        }
        RuntimePattern::Discard => inventory.intern_pattern(AwbcPattern::Discard),
        RuntimePattern::Literal(value) => {
            let constant = inventory.constant_runtime_value(value);
            inventory.intern_pattern(AwbcPattern::Literal(constant))
        }
        RuntimePattern::Entity(value) => {
            let entity = inventory.intern_string(value);
            inventory.intern_pattern(AwbcPattern::Entity(entity))
        }
        RuntimePattern::Tuple(items) => {
            let items = items
                .iter()
                .map(|item| lower_pattern(inventory, frame, item))
                .collect();
            inventory.intern_pattern(AwbcPattern::Tuple(items))
        }
        RuntimePattern::Record { fields, rest, .. } => {
            let fields = fields
                .iter()
                .enumerate()
                .map(|(index, field)| record_field(inventory, frame, index, field))
                .collect();
            inventory.intern_pattern(AwbcPattern::Record {
                ty: None,
                fields,
                rest: *rest,
            })
        }
        RuntimePattern::BracketSeq { items, rest } => {
            let rest = rest.as_ref().map(|name| {
                let name_id = inventory.intern_string(name);
                frame.local(name, name_id, inventory.dynamic_ty())
            });
            let items = items
                .iter()
                .map(|item| lower_pattern(inventory, frame, item))
                .collect();
            inventory.intern_pattern(AwbcPattern::Sequence { items, rest })
        }
        RuntimePattern::Variant { name, payload, .. } => {
            let payload = payload
                .as_deref()
                .map(|payload| lower_pattern(inventory, frame, payload));
            let case_name = inventory.intern_string(name);
            inventory.intern_pattern(AwbcPattern::Variant {
                ty: None,
                case: stable_case(name),
                case_name,
                payload,
            })
        }
        RuntimePattern::Whole { name, pattern } => {
            let name_id = inventory.intern_string(name);
            let register = frame.local(name, name_id, inventory.dynamic_ty());
            let inner = lower_pattern(inventory, frame, pattern);
            inventory.intern_pattern(AwbcPattern::Whole {
                target: register,
                inner,
            })
        }
    }
}

pub(crate) fn pattern_binding_names(pattern: &RuntimePattern) -> Vec<String> {
    match pattern {
        RuntimePattern::Ident(name)
        | RuntimePattern::MutIdent(name)
        | RuntimePattern::Typed { name, .. } => vec![name.clone()],
        RuntimePattern::Whole { name, pattern } => {
            let mut names = vec![name.clone()];
            names.extend(pattern_binding_names(pattern));
            names
        }
        RuntimePattern::Tuple(patterns) => {
            patterns.iter().flat_map(pattern_binding_names).collect()
        }
        RuntimePattern::Record { fields, .. } => fields
            .iter()
            .flat_map(|field| pattern_binding_names(&field.pattern))
            .collect(),
        RuntimePattern::BracketSeq { items, rest } => {
            let mut names = items
                .iter()
                .flat_map(pattern_binding_names)
                .collect::<Vec<_>>();
            if let Some(rest) = rest {
                names.push(rest.clone());
            }
            names
        }
        RuntimePattern::Variant { payload, .. } => payload
            .as_deref()
            .map_or_else(Vec::new, pattern_binding_names),
        RuntimePattern::Discard | RuntimePattern::Literal(_) | RuntimePattern::Entity(_) => {
            Vec::new()
        }
    }
}

fn record_field(
    inventory: &mut AwbcInventory,
    frame: &mut FrameBuilder,
    index: usize,
    field: &RuntimeRecordPatternField,
) -> AwbcRecordPatternField {
    let _ = inventory.intern_string(&field.name);
    AwbcRecordPatternField {
        field: table_index(index),
        pattern: lower_pattern(inventory, frame, &field.pattern),
    }
}

fn type_label(inventory: &mut AwbcInventory, ty: &str) -> AwbcTypeId {
    match ty {
        "bool" => inventory.bool_ty(),
        "i64" => inventory.i64_ty(),
        "String" => inventory.string_ty(),
        _ => inventory.dynamic_ty(),
    }
}

fn stable_case(value: &str) -> u32 {
    value.bytes().fold(2_166_136_261_u32, |acc, byte| {
        acc.wrapping_mul(16_777_619) ^ u32::from(byte)
    })
}
