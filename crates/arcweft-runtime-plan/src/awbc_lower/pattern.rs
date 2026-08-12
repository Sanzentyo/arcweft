use crate::awbc_lower::frame::FrameBuilder;
use crate::awbc_lower::inventory::AwbcInventory;
use crate::awbc_lower::table_index;
use arcweft_core::awbc::schema::{
    AwbcPattern, AwbcPatternId, AwbcRecordField, AwbcRecordPatternField, AwbcRuntimeType,
    AwbcSignedIntKind, AwbcTypeId, AwbcUnsignedIntKind, AwbcVariantCase, AwbcVariantIdentity,
};
use arcweft_core::pattern::{RuntimeCheckedType, RuntimePattern, RuntimeRecordPatternField};
use arcweft_core::value::RuntimeNominalRecordLayout;

/// Lowers runtime patterns into executable AWBC pattern graph nodes.
#[allow(
    clippy::too_many_lines,
    reason = "pattern lowering exhaustively mirrors the closed RuntimePattern family"
)]
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
            let expected = Some(intern_runtime_type(inventory, ty));
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
        RuntimePattern::Record {
            nominal_layout,
            fields,
            rest,
        } => {
            let fields = fields
                .iter()
                .enumerate()
                .map(|(index, field)| {
                    let ordinal = nominal_layout.as_ref().map_or_else(
                        || table_index(index),
                        |layout| {
                            layout
                                .field_by_name(&field.name)
                                .expect("checked nominal record pattern field must exist in layout")
                                .0
                                .zero_based()
                        },
                    );
                    record_field(inventory, frame, ordinal, field)
                })
                .collect();
            let ty = nominal_layout
                .as_deref()
                .map(|layout| intern_nominal_record_type(inventory, layout));
            inventory.intern_pattern(AwbcPattern::Record {
                ty,
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
        RuntimePattern::Variant {
            owner,
            ordinal,
            name,
            payload,
        } => {
            let payload = payload
                .as_deref()
                .map(|payload| lower_pattern(inventory, frame, payload));
            let case_name = inventory.intern_string(name);
            let ty = intern_runtime_type(inventory, owner);
            inventory.intern_pattern(AwbcPattern::Variant {
                ty,
                case: *ordinal,
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
    ordinal: u32,
    field: &RuntimeRecordPatternField,
) -> AwbcRecordPatternField {
    let _ = inventory.intern_string(&field.name);
    AwbcRecordPatternField {
        field: ordinal,
        pattern: lower_pattern(inventory, frame, &field.pattern),
    }
}

pub(crate) fn intern_nominal_record_type(
    inventory: &mut AwbcInventory,
    layout: &RuntimeNominalRecordLayout,
) -> AwbcTypeId {
    let fields = layout
        .fields()
        .iter()
        .map(|field| AwbcRecordField {
            name: inventory.intern_string(field.name()),
            ty: intern_runtime_type(inventory, field.checked_type()),
        })
        .collect();
    let public_id = inventory.intern_string(layout.nominal().as_str());
    inventory.intern_type(AwbcRuntimeType::NominalRecord {
        public_id,
        semantic_identity: *layout.semantic_identity().as_bytes(),
        layout: *layout.layout().as_bytes(),
        fields,
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "the exhaustive checked-type to AWBC schema projection is one closed recursive type-family matrix"
)]
pub(crate) fn intern_runtime_type(
    inventory: &mut AwbcInventory,
    ty: &RuntimeCheckedType,
) -> AwbcTypeId {
    let projected = match ty {
        RuntimeCheckedType::Never => AwbcRuntimeType::Never,
        RuntimeCheckedType::Unit => AwbcRuntimeType::Unit,
        RuntimeCheckedType::Bool => AwbcRuntimeType::Bool,
        RuntimeCheckedType::Signed(width) => AwbcRuntimeType::Int(match width {
            arcweft_core::value::RuntimeSignedIntWidth::I8 => AwbcSignedIntKind::I8,
            arcweft_core::value::RuntimeSignedIntWidth::I16 => AwbcSignedIntKind::I16,
            arcweft_core::value::RuntimeSignedIntWidth::I32 => AwbcSignedIntKind::I32,
            arcweft_core::value::RuntimeSignedIntWidth::I64 => AwbcSignedIntKind::I64,
            arcweft_core::value::RuntimeSignedIntWidth::I128 => AwbcSignedIntKind::I128,
            arcweft_core::value::RuntimeSignedIntWidth::ISize => AwbcSignedIntKind::ISize,
        }),
        RuntimeCheckedType::Unsigned(width) => AwbcRuntimeType::UInt(match width {
            arcweft_core::value::RuntimeUnsignedIntWidth::U8 => AwbcUnsignedIntKind::U8,
            arcweft_core::value::RuntimeUnsignedIntWidth::U16 => AwbcUnsignedIntKind::U16,
            arcweft_core::value::RuntimeUnsignedIntWidth::U32 => AwbcUnsignedIntKind::U32,
            arcweft_core::value::RuntimeUnsignedIntWidth::U64 => AwbcUnsignedIntKind::U64,
            arcweft_core::value::RuntimeUnsignedIntWidth::U128 => AwbcUnsignedIntKind::U128,
            arcweft_core::value::RuntimeUnsignedIntWidth::USize => AwbcUnsignedIntKind::USize,
        }),
        RuntimeCheckedType::F32 => AwbcRuntimeType::F32,
        RuntimeCheckedType::F64 => AwbcRuntimeType::F64,
        RuntimeCheckedType::String => AwbcRuntimeType::String,
        RuntimeCheckedType::Char => AwbcRuntimeType::Char,
        RuntimeCheckedType::Duration => AwbcRuntimeType::Duration,
        RuntimeCheckedType::EntityReference => AwbcRuntimeType::EntityRef,
        RuntimeCheckedType::Bytes => AwbcRuntimeType::Bytes,
        RuntimeCheckedType::Sequence(item) => {
            AwbcRuntimeType::Sequence(intern_runtime_type(inventory, item))
        }
        RuntimeCheckedType::Tuple(items) => AwbcRuntimeType::Tuple(
            items
                .iter()
                .map(|item| intern_runtime_type(inventory, item))
                .collect(),
        ),
        RuntimeCheckedType::Choice(alternatives) => AwbcRuntimeType::Choice(
            alternatives
                .iter()
                .map(|alternative| intern_runtime_type(inventory, alternative))
                .collect(),
        ),
        RuntimeCheckedType::Nominal {
            nominal,
            semantic_identity,
            layout,
        } => AwbcRuntimeType::Nominal {
            public_id: inventory.intern_string(nominal.as_str()),
            semantic_identity: *semantic_identity.as_bytes(),
            layout: *layout.as_bytes(),
        },
        RuntimeCheckedType::Opaque { owner } => AwbcRuntimeType::Opaque {
            producer: inventory.intern_string(owner.producer().as_str()),
            semantic_identity: *owner.semantic_identity().as_bytes(),
            admission: owner.admission(),
        },
        RuntimeCheckedType::Variant {
            nominal,
            semantic_identity,
            cases,
        } => AwbcRuntimeType::Variant {
            owner: AwbcVariantIdentity::Nominal {
                public_id: inventory.intern_string(nominal.as_str()),
                semantic_identity: *semantic_identity.as_bytes(),
            },
            cases: cases
                .iter()
                .map(|case| AwbcVariantCase {
                    name: inventory.intern_string(&case.name),
                    payload: case
                        .payload
                        .as_deref()
                        .map(|payload| intern_runtime_type(inventory, payload)),
                })
                .collect(),
        },
        RuntimeCheckedType::Result { ok, error } => {
            let ok = intern_runtime_type(inventory, ok);
            let error = intern_runtime_type(inventory, error);
            let ok_name = inventory.intern_string("Ok");
            let error_name = inventory.intern_string("Err");
            AwbcRuntimeType::Variant {
                owner: AwbcVariantIdentity::Result,
                cases: vec![
                    AwbcVariantCase {
                        name: ok_name,
                        payload: Some(ok),
                    },
                    AwbcVariantCase {
                        name: error_name,
                        payload: Some(error),
                    },
                ],
            }
        }
        RuntimeCheckedType::Option(item) => {
            let item = intern_runtime_type(inventory, item);
            let some_name = inventory.intern_string("Some");
            let none_name = inventory.intern_string("None");
            AwbcRuntimeType::Variant {
                owner: AwbcVariantIdentity::Option,
                cases: vec![
                    AwbcVariantCase {
                        name: some_name,
                        payload: Some(item),
                    },
                    AwbcVariantCase {
                        name: none_name,
                        payload: None,
                    },
                ],
            }
        }
    };
    inventory.intern_type(projected)
}
