use crate::awbc_lower::frame::FrameBuilder;
use crate::awbc_lower::inventory::{AwbcInventory, AwbcLowerDiagnostic};
use arcweft_core::awbc::schema::{
    AwbcAgentTypeShape, AwbcPattern, AwbcPatternId, AwbcPatternRest, AwbcRecordPatternField,
    AwbcRuntimeTypeShape, AwbcSignedIntKind, AwbcTypeId, AwbcUnsignedIntKind, AwbcVariantCase,
    AwbcVariantIdentity,
};
use arcweft_core::pattern::{
    RuntimeBuiltinVariantIdentity, RuntimeCheckedType, RuntimePattern, RuntimePatternKind,
    RuntimePatternRest, RuntimeRecordPatternField, RuntimeVariantIdentity,
};
use arcweft_core::plan::{RuntimeAgentTypeProjection, RuntimePlan, RuntimePlanTypeProjection};
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
            let ty = admitted_plan_type(inventory, plan, pattern.ty());
            let register = frame.local(binding.local(), ty);
            inventory.intern_pattern(AwbcPattern::Bind {
                target: register,
                mutable: *mutable,
                expected: Some(ty),
            })
        }
        RuntimePatternKind::Typed { binding } => {
            let ty = admitted_plan_type(inventory, plan, pattern.ty());
            let register = frame.local(binding.local(), ty);
            inventory.intern_pattern(AwbcPattern::Bind {
                target: register,
                mutable: false,
                expected: Some(ty),
            })
        }
        RuntimePatternKind::Discard => inventory.intern_pattern(AwbcPattern::Discard),
        RuntimePatternKind::Literal(value) => {
            let ty = admitted_plan_type(inventory, plan, pattern.ty());
            let constant = inventory.constant_runtime_value_typed(value, ty);
            inventory.intern_pattern(AwbcPattern::Literal(constant))
        }
        RuntimePatternKind::Entity(value) => {
            inventory.intern_pattern(AwbcPattern::Entity(value.clone()))
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
            let ty = admitted_plan_type(inventory, plan, pattern.ty());
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
            let ty = admitted_plan_type(inventory, plan, pattern.ty());
            let payload = payload
                .as_deref()
                .map(|payload| lower_pattern(inventory, plan, frame, payload));
            let case_name = admitted_variant_case_name(inventory, plan, pattern.ty(), *ordinal);
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
            let ty = admitted_plan_type(inventory, plan, pattern.ty());
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
            let ty =
                admitted_plan_type(inventory, plan, pattern_binding_type(plan, binding.local()));
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

/// Admits the complete RuntimePlan type graph before executable lowering.
/// Missing projections and conflicting semantic owners fail before any Flow
/// instruction can observe a placeholder type.
pub(crate) fn preflight_plan_types(
    inventory: &mut AwbcInventory,
    plan: &RuntimePlan,
) -> Result<(), Vec<AwbcLowerDiagnostic>> {
    let types = plan
        .type_table()
        .declarations_with_ids()
        .map(|(ty, declaration)| (ty, declaration.semantic_identity()))
        .collect::<Vec<_>>();
    let mut errors = Vec::new();
    for (ty, semantic_identity) in &types {
        if let Err(error) = inventory.reserve_plan_type(*ty, *semantic_identity) {
            errors.push(error);
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    for (ty, _) in types {
        match plan_type_shape(inventory, plan, ty)
            .and_then(|shape| inventory.define_plan_type(ty, shape))
        {
            Ok(()) => {}
            Err(error) => errors.push(error),
        }
    }
    if errors.is_empty()
        && let Err(error) = inventory.commit_plan_types()
    {
        errors.push(error);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the preflight exhaustively maps the closed RuntimePlan type algebra"
)]
fn plan_type_shape(
    inventory: &mut AwbcInventory,
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
) -> Result<AwbcRuntimeTypeShape, AwbcLowerDiagnostic> {
    let declaration = plan.type_table().get(ty).ok_or_else(|| {
        AwbcLowerDiagnostic::error(format!("type.{ty}"), "RuntimePlan type is absent")
    })?;
    let shape = match declaration.projection() {
        RuntimePlanTypeProjection::Never => AwbcRuntimeTypeShape::Never,
        RuntimePlanTypeProjection::Unit => AwbcRuntimeTypeShape::Unit,
        RuntimePlanTypeProjection::Bool => AwbcRuntimeTypeShape::Bool,
        RuntimePlanTypeProjection::Signed(width) => AwbcRuntimeTypeShape::Int(match width {
            arcweft_core::value::RuntimeSignedIntWidth::I8 => AwbcSignedIntKind::I8,
            arcweft_core::value::RuntimeSignedIntWidth::I16 => AwbcSignedIntKind::I16,
            arcweft_core::value::RuntimeSignedIntWidth::I32 => AwbcSignedIntKind::I32,
            arcweft_core::value::RuntimeSignedIntWidth::I64 => AwbcSignedIntKind::I64,
            arcweft_core::value::RuntimeSignedIntWidth::I128 => AwbcSignedIntKind::I128,
            arcweft_core::value::RuntimeSignedIntWidth::ISize => AwbcSignedIntKind::ISize,
        }),
        RuntimePlanTypeProjection::Unsigned(width) => AwbcRuntimeTypeShape::UInt(match width {
            arcweft_core::value::RuntimeUnsignedIntWidth::U8 => AwbcUnsignedIntKind::U8,
            arcweft_core::value::RuntimeUnsignedIntWidth::U16 => AwbcUnsignedIntKind::U16,
            arcweft_core::value::RuntimeUnsignedIntWidth::U32 => AwbcUnsignedIntKind::U32,
            arcweft_core::value::RuntimeUnsignedIntWidth::U64 => AwbcUnsignedIntKind::U64,
            arcweft_core::value::RuntimeUnsignedIntWidth::U128 => AwbcUnsignedIntKind::U128,
            arcweft_core::value::RuntimeUnsignedIntWidth::USize => AwbcUnsignedIntKind::USize,
        }),
        RuntimePlanTypeProjection::F32 => AwbcRuntimeTypeShape::F32,
        RuntimePlanTypeProjection::F64 => AwbcRuntimeTypeShape::F64,
        RuntimePlanTypeProjection::String => AwbcRuntimeTypeShape::String,
        RuntimePlanTypeProjection::Char => AwbcRuntimeTypeShape::Char,
        RuntimePlanTypeProjection::Bytes => AwbcRuntimeTypeShape::Bytes,
        RuntimePlanTypeProjection::Duration => AwbcRuntimeTypeShape::Duration,
        RuntimePlanTypeProjection::Progress => AwbcRuntimeTypeShape::Progress,
        RuntimePlanTypeProjection::EntityReference => AwbcRuntimeTypeShape::EntityRef,
        RuntimePlanTypeProjection::AgentValue => AwbcRuntimeTypeShape::AgentValue,
        RuntimePlanTypeProjection::Range(item) => {
            AwbcRuntimeTypeShape::Range(reserved_plan_type(inventory, *item)?)
        }
        RuntimePlanTypeProjection::Iterator(item) => {
            AwbcRuntimeTypeShape::Iterator(reserved_plan_type(inventory, *item)?)
        }
        RuntimePlanTypeProjection::Sequence { item, .. } => {
            AwbcRuntimeTypeShape::Sequence(reserved_plan_type(inventory, *item)?)
        }
        RuntimePlanTypeProjection::Array { item, length } => AwbcRuntimeTypeShape::Array {
            item: reserved_plan_type(inventory, *item)?,
            length: *length,
        },
        RuntimePlanTypeProjection::Map { key, value } => AwbcRuntimeTypeShape::Map {
            key: reserved_plan_type(inventory, *key)?,
            value: reserved_plan_type(inventory, *value)?,
        },
        RuntimePlanTypeProjection::Need(value) => {
            AwbcRuntimeTypeShape::Need(reserved_plan_type(inventory, *value)?)
        }
        RuntimePlanTypeProjection::Stream { item, error } => AwbcRuntimeTypeShape::Stream {
            item: reserved_plan_type(inventory, *item)?,
            error: reserved_plan_type(inventory, *error)?,
        },
        RuntimePlanTypeProjection::Result { value, error } => AwbcRuntimeTypeShape::Variant {
            owner: AwbcVariantIdentity::Builtin(
                arcweft_core::pattern::RuntimeBuiltinVariantIdentity::Result,
            ),
            arguments: Vec::new(),
            cases: vec![
                AwbcVariantCase {
                    name: inventory.intern_string("Ok"),
                    payload: Some(reserved_plan_type(inventory, *value)?),
                },
                AwbcVariantCase {
                    name: inventory.intern_string("Err"),
                    payload: Some(reserved_plan_type(inventory, *error)?),
                },
            ],
        },
        RuntimePlanTypeProjection::Option(item) => AwbcRuntimeTypeShape::Variant {
            owner: AwbcVariantIdentity::Builtin(
                arcweft_core::pattern::RuntimeBuiltinVariantIdentity::Option,
            ),
            arguments: Vec::new(),
            cases: vec![
                AwbcVariantCase {
                    name: inventory.intern_string("Some"),
                    payload: Some(reserved_plan_type(inventory, *item)?),
                },
                AwbcVariantCase {
                    name: inventory.intern_string("None"),
                    payload: None,
                },
            ],
        },
        RuntimePlanTypeProjection::BuiltinVariant { owner, cases } => {
            AwbcRuntimeTypeShape::Variant {
                owner: AwbcVariantIdentity::Builtin(*owner),
                arguments: Vec::new(),
                cases: owner
                    .cases()
                    .iter()
                    .zip(cases)
                    .map(|(schema, payload)| {
                        Ok(AwbcVariantCase {
                            name: inventory.intern_string(schema.name()),
                            payload: payload
                                .map(|payload| reserved_plan_type(inventory, payload))
                                .transpose()?,
                        })
                    })
                    .collect::<Result<Vec<_>, AwbcLowerDiagnostic>>()?,
            }
        }
        RuntimePlanTypeProjection::ThreadHandle(result) => {
            AwbcRuntimeTypeShape::Task(reserved_plan_type(inventory, *result)?)
        }
        RuntimePlanTypeProjection::Shared(value) => {
            AwbcRuntimeTypeShape::Shared(reserved_plan_type(inventory, *value)?)
        }
        RuntimePlanTypeProjection::Reference(value) => {
            AwbcRuntimeTypeShape::Reference(reserved_plan_type(inventory, *value)?)
        }
        RuntimePlanTypeProjection::Function { parameters, result } => {
            AwbcRuntimeTypeShape::Function {
                parameters: parameters
                    .iter()
                    .map(|parameter| reserved_plan_type(inventory, *parameter))
                    .collect::<Result<Vec<_>, _>>()?,
                result: reserved_plan_type(inventory, *result)?,
            }
        }
        RuntimePlanTypeProjection::ProjectNominal {
            nominal,
            layout,
            arguments,
        } => nominal_plan_shape(inventory, plan, ty, nominal, *layout, arguments)?,
        RuntimePlanTypeProjection::Tuple(items) => AwbcRuntimeTypeShape::Tuple(
            items
                .iter()
                .map(|item| reserved_plan_type(inventory, *item))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        RuntimePlanTypeProjection::Choice(items) => AwbcRuntimeTypeShape::Choice(
            items
                .iter()
                .map(|item| reserved_plan_type(inventory, *item))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        RuntimePlanTypeProjection::Opaque {
            producer,
            admission,
            value_class,
            persistence,
            arguments,
        } => AwbcRuntimeTypeShape::Opaque {
            producer: inventory.intern_string(producer.as_str()),
            admission: *admission,
            value_class: *value_class,
            persistence: *persistence,
            arguments: arguments
                .iter()
                .map(|argument| reserved_plan_type(inventory, *argument))
                .collect::<Result<Vec<_>, _>>()?,
        },
        RuntimePlanTypeProjection::Agent(agent) => AwbcRuntimeTypeShape::Agent(match agent {
            RuntimeAgentTypeProjection::Probe(value) => {
                AwbcAgentTypeShape::Probe(reserved_plan_type(inventory, *value)?)
            }
            _ => AwbcAgentTypeShape::Leaf(agent.operational_type()),
        }),
    };
    Ok(shape)
}

fn reserved_plan_type(
    inventory: &AwbcInventory,
    ty: RuntimePlanTypeId,
) -> Result<AwbcTypeId, AwbcLowerDiagnostic> {
    inventory.plan_type(ty).ok_or_else(|| {
        AwbcLowerDiagnostic::error(
            format!("type.{ty}"),
            "RuntimePlan child type was not reserved by AWBC preflight",
        )
    })
}

fn nominal_plan_shape(
    inventory: &mut AwbcInventory,
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    nominal: &arcweft_core::entry::RuntimeNominalTypeId,
    layout: arcweft_core::entry::TypeLayoutHash,
    arguments: &[RuntimePlanTypeId],
) -> Result<AwbcRuntimeTypeShape, AwbcLowerDiagnostic> {
    let public_id = inventory.intern_string(nominal.as_str());
    let arguments = arguments
        .iter()
        .map(|argument| reserved_plan_type(inventory, *argument))
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(domain) = plan.nominal_record_domains().get(ty) {
        let fields = domain
            .fields()
            .iter()
            .map(|field| {
                Ok(arcweft_core::awbc::schema::AwbcRecordField {
                    name: inventory.intern_string(field.name()),
                    ty: reserved_plan_type(inventory, field.ty())?,
                })
            })
            .collect::<Result<Vec<_>, AwbcLowerDiagnostic>>()?;
        return Ok(AwbcRuntimeTypeShape::NominalRecord {
            public_id,
            layout: *layout.as_bytes(),
            arguments,
            fields,
        });
    }
    if let Some(domain) = plan.variant_domains().get(ty) {
        let cases = domain
            .cases()
            .iter()
            .map(|case| {
                Ok(AwbcVariantCase {
                    name: inventory.intern_string(case.name()),
                    payload: case
                        .payload()
                        .map(|payload| reserved_plan_type(inventory, payload))
                        .transpose()?,
                })
            })
            .collect::<Result<Vec<_>, AwbcLowerDiagnostic>>()?;
        return Ok(AwbcRuntimeTypeShape::Variant {
            owner: AwbcVariantIdentity::Nominal { public_id },
            arguments,
            cases,
        });
    }
    Ok(AwbcRuntimeTypeShape::Nominal {
        public_id,
        layout: *layout.as_bytes(),
        arguments,
    })
}

pub(crate) fn plan_type(
    inventory: &mut AwbcInventory,
    _plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
) -> Result<AwbcTypeId, AwbcLowerDiagnostic> {
    inventory.plan_type(ty).ok_or_else(|| {
        AwbcLowerDiagnostic::error(
            format!("type.{ty}"),
            "RuntimePlan type was not admitted by the AWBC type preflight",
        )
    })
}

pub(crate) fn admitted_plan_type(
    inventory: &mut AwbcInventory,
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
) -> AwbcTypeId {
    plan_type(inventory, plan, ty)
        .expect("AWBC type preflight admits every RuntimePlan type before lowering")
}

pub(crate) fn admitted_local_type(
    inventory: &mut AwbcInventory,
    plan: &RuntimePlan,
    local: RuntimeLocalDeclarationId,
) -> AwbcTypeId {
    let Some(declaration) = plan.local_declarations().get(local) else {
        inventory.diagnostic(AwbcLowerDiagnostic::error(
            format!("local.{local}"),
            "RuntimePlan local declaration is absent during AWBC lowering",
        ));
        return inventory.dynamic_ty();
    };
    admitted_plan_type(inventory, plan, declaration.ty())
}

pub(crate) fn variant_case_name(
    inventory: &mut AwbcInventory,
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    ordinal: u32,
) -> Result<arcweft_core::awbc::schema::AwbcStringId, AwbcLowerDiagnostic> {
    let name = if let Some(domain) = plan.variant_domains().get(ty) {
        domain.case(ordinal).map(|case| case.name().to_owned())
    } else {
        plan.type_table()
            .get(ty)
            .and_then(|declaration| match declaration.projection() {
                RuntimePlanTypeProjection::Option(_) => match ordinal {
                    0 => Some("Some".to_owned()),
                    1 => Some("None".to_owned()),
                    _ => None,
                },
                RuntimePlanTypeProjection::Result { .. } => match ordinal {
                    0 => Some("Ok".to_owned()),
                    1 => Some("Err".to_owned()),
                    _ => None,
                },
                _ => None,
            })
    };
    name.map(|name| inventory.intern_string(&name))
        .ok_or_else(|| {
            AwbcLowerDiagnostic::error(
                format!("type.{ty}"),
                format!("variant ordinal {ordinal} is absent from RuntimePlan type {ty}"),
            )
        })
}

pub(crate) fn admitted_variant_case_name(
    inventory: &mut AwbcInventory,
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    ordinal: u32,
) -> arcweft_core::awbc::schema::AwbcStringId {
    variant_case_name(inventory, plan, ty, ordinal)
        .expect("AWBC type preflight retains every checked variant case")
}

pub(crate) fn intern_runtime_type(
    inventory: &mut AwbcInventory,
    ty: &RuntimeCheckedType,
) -> AwbcTypeId {
    let projected = match ty {
        RuntimeCheckedType::Never => AwbcRuntimeTypeShape::Never,
        RuntimeCheckedType::Unit => AwbcRuntimeTypeShape::Unit,
        RuntimeCheckedType::Bool => AwbcRuntimeTypeShape::Bool,
        RuntimeCheckedType::Signed(width) => AwbcRuntimeTypeShape::Int(match width {
            arcweft_core::value::RuntimeSignedIntWidth::I8 => AwbcSignedIntKind::I8,
            arcweft_core::value::RuntimeSignedIntWidth::I16 => AwbcSignedIntKind::I16,
            arcweft_core::value::RuntimeSignedIntWidth::I32 => AwbcSignedIntKind::I32,
            arcweft_core::value::RuntimeSignedIntWidth::I64 => AwbcSignedIntKind::I64,
            arcweft_core::value::RuntimeSignedIntWidth::I128 => AwbcSignedIntKind::I128,
            arcweft_core::value::RuntimeSignedIntWidth::ISize => AwbcSignedIntKind::ISize,
        }),
        RuntimeCheckedType::Unsigned(width) => AwbcRuntimeTypeShape::UInt(match width {
            arcweft_core::value::RuntimeUnsignedIntWidth::U8 => AwbcUnsignedIntKind::U8,
            arcweft_core::value::RuntimeUnsignedIntWidth::U16 => AwbcUnsignedIntKind::U16,
            arcweft_core::value::RuntimeUnsignedIntWidth::U32 => AwbcUnsignedIntKind::U32,
            arcweft_core::value::RuntimeUnsignedIntWidth::U64 => AwbcUnsignedIntKind::U64,
            arcweft_core::value::RuntimeUnsignedIntWidth::U128 => AwbcUnsignedIntKind::U128,
            arcweft_core::value::RuntimeUnsignedIntWidth::USize => AwbcUnsignedIntKind::USize,
        }),
        RuntimeCheckedType::F32 => AwbcRuntimeTypeShape::F32,
        RuntimeCheckedType::F64 => AwbcRuntimeTypeShape::F64,
        RuntimeCheckedType::String => AwbcRuntimeTypeShape::String,
        RuntimeCheckedType::Char => AwbcRuntimeTypeShape::Char,
        RuntimeCheckedType::Duration => AwbcRuntimeTypeShape::Duration,
        RuntimeCheckedType::Progress => AwbcRuntimeTypeShape::Progress,
        RuntimeCheckedType::EntityReference => AwbcRuntimeTypeShape::EntityRef,
        RuntimeCheckedType::AgentValue => AwbcRuntimeTypeShape::AgentValue,
        RuntimeCheckedType::Bytes => AwbcRuntimeTypeShape::Bytes,
        RuntimeCheckedType::Sequence(item) => {
            AwbcRuntimeTypeShape::Sequence(intern_runtime_type(inventory, item))
        }
        RuntimeCheckedType::Tuple(items) => AwbcRuntimeTypeShape::Tuple(
            items
                .iter()
                .map(|item| intern_runtime_type(inventory, item))
                .collect(),
        ),
        RuntimeCheckedType::Choice(alternatives) => AwbcRuntimeTypeShape::Choice(
            alternatives
                .iter()
                .map(|item| intern_runtime_type(inventory, item))
                .collect(),
        ),
        RuntimeCheckedType::Nominal {
            nominal,
            layout,
            arguments,
            ..
        } => AwbcRuntimeTypeShape::Nominal {
            public_id: inventory.intern_string(nominal.as_str()),
            layout: *layout.as_bytes(),
            arguments: arguments
                .iter()
                .map(|argument| intern_runtime_type(inventory, argument))
                .collect(),
        },
        RuntimeCheckedType::Opaque { owner } => AwbcRuntimeTypeShape::Opaque {
            producer: inventory.intern_string(owner.producer().as_str()),
            admission: owner.admission(),
            value_class: owner.value_class(),
            persistence: owner.persistence(),
            arguments: Vec::new(),
        },
        RuntimeCheckedType::Variant {
            owner,
            arguments,
            cases,
        } => intern_variant_type(inventory, owner, arguments, cases),
        RuntimeCheckedType::Result { ok, error } => AwbcRuntimeTypeShape::Variant {
            owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Result),
            arguments: Vec::new(),
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
        RuntimeCheckedType::Option(item) => AwbcRuntimeTypeShape::Variant {
            owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Option),
            arguments: Vec::new(),
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
        RuntimeCheckedType::Agent(agent) => {
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(*agent))
        }
    };
    let semantic_identity = match ty {
        RuntimeCheckedType::Nominal {
            semantic_identity, ..
        }
        | RuntimeCheckedType::Variant {
            owner:
                RuntimeVariantIdentity::Nominal {
                    semantic_identity, ..
                },
            ..
        } => *semantic_identity,
        RuntimeCheckedType::Opaque { owner } => owner.semantic_identity(),
        _ => ty.semantic_identity_digest(),
    };
    inventory
        .intern_semantic_type(semantic_identity, projected)
        .expect("one checked runtime type has one canonical AWBC shape")
}

fn intern_variant_type(
    inventory: &mut AwbcInventory,
    owner: &RuntimeVariantIdentity,
    arguments: &[RuntimeCheckedType],
    cases: &[arcweft_core::pattern::RuntimeCheckedVariantCase],
) -> AwbcRuntimeTypeShape {
    AwbcRuntimeTypeShape::Variant {
        owner: match owner {
            RuntimeVariantIdentity::Nominal { nominal, .. } => AwbcVariantIdentity::Nominal {
                public_id: inventory.intern_string(nominal.as_str()),
            },
            RuntimeVariantIdentity::Builtin(owner) => AwbcVariantIdentity::Builtin(*owner),
        },
        arguments: arguments
            .iter()
            .map(|argument| intern_runtime_type(inventory, argument))
            .collect(),
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
