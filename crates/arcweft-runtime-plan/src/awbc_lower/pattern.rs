use crate::awbc_lower::frame::FrameBuilder;
use crate::awbc_lower::inventory::{AwbcInventory, AwbcLowerDiagnostic};
use arcweft_core::awbc::schema::{
    AwbcPattern, AwbcPatternId, AwbcPatternRest, AwbcRecordPatternField, AwbcRuntimeType,
    AwbcSignedIntKind, AwbcTypeId, AwbcUnsignedIntKind, AwbcVariantCase, AwbcVariantIdentity,
};
use arcweft_core::pattern::{
    RuntimeCheckedType, RuntimePattern, RuntimePatternKind, RuntimePatternRest,
    RuntimeRecordPatternField,
};
use arcweft_core::plan::{RuntimePlan, RuntimePlanTypeProjection};
use arcweft_core::runtime_id::{RuntimeLocalDeclarationId, RuntimePlanTypeId};

/// Lowers runtime patterns into executable AWBC pattern graph nodes.
pub(crate) fn lower_pattern(
    inventory: &mut AwbcInventory,
    plan: &RuntimePlan,
    frame: &mut FrameBuilder,
    pattern: &RuntimePattern,
) -> AwbcPatternId {
    match pattern.kind() {
        RuntimePatternKind::Bind { mutable, binding } => {
            let ty = plan_type(inventory, plan, pattern.ty());
            let register = frame.local(binding.local(), ty);
            inventory.intern_pattern(AwbcPattern::Bind {
                target: register,
                mutable: *mutable,
                expected: Some(ty),
            })
        }
        RuntimePatternKind::Typed { binding } => {
            let ty = plan_type(inventory, plan, pattern.ty());
            let register = frame.local(binding.local(), ty);
            inventory.intern_pattern(AwbcPattern::Bind {
                target: register,
                mutable: false,
                expected: Some(ty),
            })
        }
        RuntimePatternKind::Discard => inventory.intern_pattern(AwbcPattern::Discard),
        RuntimePatternKind::Literal(value) => {
            let constant = inventory.constant_runtime_value(value);
            inventory.intern_pattern(AwbcPattern::Literal(constant))
        }
        RuntimePatternKind::Entity(value) => {
            let label = value.runtime_label();
            let entity = inventory.intern_string(&label);
            inventory.intern_pattern(AwbcPattern::Entity(entity))
        }
        RuntimePatternKind::Tuple(items) => {
            let items = items
                .iter()
                .map(|item| lower_pattern(inventory, plan, frame, item))
                .collect();
            inventory.intern_pattern(AwbcPattern::Tuple(items))
        }
        RuntimePatternKind::Record { fields, rest } => {
            let fields = fields
                .iter()
                .map(|field| {
                    record_field(inventory, plan, frame, field.field().zero_based(), field)
                })
                .collect();
            let ty = plan_type(inventory, plan, pattern.ty());
            let rest = lower_rest(inventory, plan, frame, rest);
            inventory.intern_pattern(AwbcPattern::Record {
                ty: Some(ty),
                fields,
                rest,
            })
        }
        RuntimePatternKind::Sequence { items, rest } => {
            let items = items
                .iter()
                .map(|item| lower_pattern(inventory, plan, frame, item))
                .collect();
            let rest = lower_rest(inventory, plan, frame, rest);
            inventory.intern_pattern(AwbcPattern::Sequence { items, rest })
        }
        RuntimePatternKind::Variant { ordinal, payload } => {
            let ty = plan_type(inventory, plan, pattern.ty());
            let payload = payload
                .as_deref()
                .map(|payload| lower_pattern(inventory, plan, frame, payload));
            let case_name = variant_case_name(inventory, plan, pattern.ty(), *ordinal);
            inventory.intern_pattern(AwbcPattern::Variant {
                ty,
                case: *ordinal,
                case_name,
                payload,
            })
        }
        RuntimePatternKind::Whole {
            binding,
            pattern: inner,
        } => {
            let inner = lower_pattern(inventory, plan, frame, inner);
            let ty = plan_type(inventory, plan, pattern.ty());
            let target = frame.local(binding.local(), ty);
            inventory.intern_pattern(AwbcPattern::Whole { target, inner })
        }
    }
}

fn lower_rest(
    inventory: &mut AwbcInventory,
    plan: &RuntimePlan,
    frame: &mut FrameBuilder,
    rest: &RuntimePatternRest,
) -> AwbcPatternRest {
    match rest {
        RuntimePatternRest::Exact => AwbcPatternRest::Exact,
        RuntimePatternRest::Ignore => AwbcPatternRest::Ignore,
        RuntimePatternRest::Bind(binding) => {
            let ty = plan_type(inventory, plan, pattern_binding_type(plan, binding.local()));
            AwbcPatternRest::Bind(frame.local(binding.local(), ty))
        }
    }
}

fn pattern_binding_type(plan: &RuntimePlan, local: RuntimeLocalDeclarationId) -> RuntimePlanTypeId {
    plan.local_declarations().get(local).map_or_else(
        || panic!("admitted pattern binding local {local} is absent from its RuntimePlan"),
        arcweft_core::plan::RuntimeLocalDeclaration::ty,
    )
}

fn record_field(
    inventory: &mut AwbcInventory,
    plan: &RuntimePlan,
    frame: &mut FrameBuilder,
    ordinal: u32,
    field: &RuntimeRecordPatternField,
) -> AwbcRecordPatternField {
    AwbcRecordPatternField {
        field: ordinal,
        pattern: lower_pattern(inventory, plan, frame, field.pattern()),
    }
}

pub(crate) fn plan_type(
    inventory: &mut AwbcInventory,
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
) -> AwbcTypeId {
    if let Some(declaration) = plan.type_table().get(ty)
        && let RuntimePlanTypeProjection::Opaque {
            producer,
            admission,
            arguments,
        } = declaration.projection()
    {
        let producer = inventory.intern_string(producer.as_str());
        let arguments = arguments
            .iter()
            .map(|argument| plan_type(inventory, plan, *argument))
            .collect();
        return inventory.intern_type(AwbcRuntimeType::Opaque {
            producer,
            semantic_identity: *declaration.semantic_identity().as_bytes(),
            admission: *admission,
            arguments,
        });
    }
    match plan.checked_type(ty) {
        Ok(Some(checked)) => intern_runtime_type(inventory, &checked),
        Ok(None) => inventory.dynamic_ty(),
        Err(error) => {
            inventory.diagnostic(AwbcLowerDiagnostic::error(
                format!("type.{ty}"),
                format!("RuntimePlan type {ty} cannot be projected into AWBC: {error}"),
            ));
            inventory.dynamic_ty()
        }
    }
}

pub(crate) fn variant_case_name(
    inventory: &mut AwbcInventory,
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    ordinal: u32,
) -> arcweft_core::awbc::schema::AwbcStringId {
    let name = plan
        .checked_type(ty)
        .ok()
        .flatten()
        .and_then(|checked| match checked {
            RuntimeCheckedType::Variant { cases, .. } => cases
                .get(usize::try_from(ordinal).ok()?)
                .map(|case| case.name.clone()),
            RuntimeCheckedType::Option(_) => match ordinal {
                0 => Some("Some".to_owned()),
                1 => Some("None".to_owned()),
                _ => None,
            },
            RuntimeCheckedType::Result { .. } => match ordinal {
                0 => Some("Ok".to_owned()),
                1 => Some("Err".to_owned()),
                _ => None,
            },
            _ => None,
        });
    if let Some(name) = name {
        inventory.intern_string(&name)
    } else {
        inventory.diagnostic(AwbcLowerDiagnostic::error(
            format!("type.{ty}"),
            format!("variant ordinal {ordinal} is absent from RuntimePlan type {ty}"),
        ));
        inventory.intern_string("<invalid-variant>")
    }
}

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
        RuntimeCheckedType::Progress => AwbcRuntimeType::Progress,
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
                .map(|item| intern_runtime_type(inventory, item))
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
            arguments: Vec::new(),
        },
        RuntimeCheckedType::Variant {
            nominal,
            semantic_identity,
            cases,
        } => intern_variant_type(inventory, nominal, *semantic_identity, cases),
        RuntimeCheckedType::Result { ok, error } => AwbcRuntimeType::Variant {
            owner: AwbcVariantIdentity::Result,
            cases: vec![
                AwbcVariantCase {
                    name: inventory.intern_string("Ok"),
                    payload: Some(intern_runtime_type(inventory, ok)),
                },
                AwbcVariantCase {
                    name: inventory.intern_string("Err"),
                    payload: Some(intern_runtime_type(inventory, error)),
                },
            ],
        },
        RuntimeCheckedType::Option(item) => AwbcRuntimeType::Variant {
            owner: AwbcVariantIdentity::Option,
            cases: vec![
                AwbcVariantCase {
                    name: inventory.intern_string("Some"),
                    payload: Some(intern_runtime_type(inventory, item)),
                },
                AwbcVariantCase {
                    name: inventory.intern_string("None"),
                    payload: None,
                },
            ],
        },
        RuntimeCheckedType::Agent(agent) => AwbcRuntimeType::Agent(*agent),
    };
    inventory.intern_type(projected)
}

fn intern_variant_type(
    inventory: &mut AwbcInventory,
    nominal: &arcweft_core::entry::RuntimeNominalTypeId,
    semantic_identity: arcweft_core::pattern::RuntimeSemanticTypeId,
    cases: &[arcweft_core::pattern::RuntimeCheckedVariantCase],
) -> AwbcRuntimeType {
    AwbcRuntimeType::Variant {
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
    }
}
