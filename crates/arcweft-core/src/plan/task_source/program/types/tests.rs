//! Verified, permuted backend rows against the existing complete Source fixture.

use super::*;
use crate::awbc::schema::{AwbcRuntimeType, AwbcSignedIntKind, AwbcUnsignedIntKind};
use crate::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext};
use crate::plan::{RuntimePlanBuilder, RuntimePlanTypeSeed};
use crate::value::RuntimeRecordFieldId;

fn original() -> RuntimePlan {
    let table = crate::plan::type_table::source_codec::tests::full_table();
    let nominal = table
        .declarations()
        .find(|row| matches!(row.projection(), RuntimePlanTypeProjection::Nominal { .. }))
        .unwrap();
    let RuntimePlanTypeProjection::Nominal { nominal: name, .. } = nominal.projection() else {
        unreachable!()
    };
    let unit = table
        .declarations()
        .find(|row| matches!(row.projection(), RuntimePlanTypeProjection::Unit))
        .unwrap()
        .semantic_identity();
    let schema = crate::entry::RuntimeNominalSchemaGraph::try_new(
        vec![crate::entry::RuntimeNominalSchemaDefinition::new(
            crate::entry::RuntimeNominalDeclarationId::from_bytes(
                *nominal.semantic_identity().as_bytes(),
            ),
            crate::entry::RuntimeNominalSchemaIdentity::new(
                name.clone(),
                nominal.semantic_identity(),
            ),
            vec![crate::entry::RuntimeTypeSchema::Unit],
            crate::entry::RuntimeNominalSchemaBody::Record {
                shape: crate::entry::RuntimeNominalRecordShape::Record,
                fields: Box::new([crate::entry::RuntimeNominalSchemaField::new(
                    RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                    Some("payload".into()),
                    crate::entry::RuntimeTypeSchema::Unit,
                )]),
            },
        )],
        crate::entry::RuntimeSchemaLimits::engine_default(),
    )
    .unwrap();
    let seeds = table
        .declarations()
        .map(|row| {
            let mut projection = row
                .projection()
                .clone()
                .try_map(|ty| {
                    Ok::<_, std::convert::Infallible>(table.get(ty).unwrap().semantic_identity())
                })
                .unwrap();
            if let RuntimePlanTypeProjection::Nominal { layout, .. } = &mut projection {
                *layout = schema.try_layout_hash(nominal.semantic_identity()).unwrap();
            }
            RuntimePlanTypeSeed::new(row.semantic_identity(), projection)
                .with_scope(row.scope().clone())
        })
        .collect::<Vec<_>>();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            seeds,
            [],
            [crate::plan::RuntimeNominalRecordDomainSeed::new(
                nominal.semantic_identity(),
                crate::entry::RuntimeNominalRecordShape::Record,
                [crate::plan::RuntimeNominalRecordDomainFieldSeed::new(
                    RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                    Some("payload".into()),
                    unit,
                )],
            )],
            [],
            &schema,
        )
        .unwrap();
    builder.finish().unwrap()
}

fn id(plan: &RuntimePlan, source: RuntimePlanTypeId) -> AwbcTypeId {
    // Reverse every dense Source row: equality of local indices cannot pass.
    AwbcTypeId(u32::try_from(plan.type_table().len()).unwrap() - source.get().get())
}

fn string(program: &mut AwbcProgram, value: &str) -> AwbcStringId {
    let id = AwbcStringId(u32::try_from(program.strings.len()).unwrap());
    program.strings.push(value.to_owned());
    id
}

fn builtin(
    program: &mut AwbcProgram,
    owner: crate::pattern::RuntimeBuiltinVariantIdentity,
    payloads: &[Option<AwbcTypeId>],
) -> AwbcRuntimeTypeShape {
    let cases = owner
        .cases()
        .iter()
        .zip(payloads)
        .map(|(case, payload)| AwbcVariantCase {
            name: string(program, case.name()),
            payload: *payload,
        })
        .collect();
    AwbcRuntimeTypeShape::Variant {
        owner: AwbcVariantIdentity::Builtin(owner),
        arguments: vec![],
        cases,
    }
}

fn executable(plan: &RuntimePlan) -> AwbcProgram {
    use AwbcRuntimeTypeShape as A;
    use RuntimePlanTypeProjection as P;
    let mut program = AwbcProgram {
        runtime_types: vec![],
        ..AwbcProgram::default()
    };
    for (source, row) in plan
        .type_table()
        .declarations_with_ids()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        let projection = row
            .projection()
            .clone()
            .try_map(|ty| Ok::<_, std::convert::Infallible>(id(plan, ty)))
            .unwrap();
        let shape = match projection {
            P::BoundType(reference) => A::BoundType(reference),
            P::Never => A::Never,
            P::Unit => A::Unit,
            P::Bool => A::Bool,
            P::F32 => A::F32,
            P::F64 => A::F64,
            P::String => A::String,
            P::Color => A::Color,
            P::Char => A::Char,
            P::Bytes => A::Bytes,
            P::Duration => A::Duration,
            P::Progress => A::Progress,
            P::EntityReference => A::EntityRef,
            P::AgentValue => A::AgentValue,
            P::Signed(width) => A::Int(
                *AwbcSignedIntKind::ALL
                    .iter()
                    .find(|kind| crate::value::RuntimeSignedIntWidth::from(**kind) == width)
                    .unwrap(),
            ),
            P::Unsigned(width) => A::UInt(
                *AwbcUnsignedIntKind::ALL
                    .iter()
                    .find(|kind| crate::value::RuntimeUnsignedIntWidth::from(**kind) == width)
                    .unwrap(),
            ),
            P::DenseNumeric(dense) => dense.into(),
            P::Range(item) => A::Range(item),
            P::Iterator(item) => A::Iterator(item),
            P::Need(item) => A::Need(item),
            P::ThreadHandle(item) => A::Task(item),
            P::Shared(item) => A::Shared(item),
            P::Reference(item) => A::Reference(item),
            P::Sequence { kind, item } => A::Sequence { kind, item },
            P::Array { item, length } => A::Array { item, length },
            P::Map { kind, key, value } => A::Map { kind, key, value },
            P::Traversal { source, mapping } => A::Traversal { source, mapping },
            P::Stream { item, error } => A::Stream { item, error },
            P::Tuple(items) => A::Tuple(items.into_vec()),
            P::Choice(items) => A::Choice(items.into_vec()),
            P::Function {
                contract,
                parameters,
                result,
            } => A::Function {
                contract,
                parameters: parameters.into_vec(),
                result,
            },
            P::Record(fields) => A::Record {
                public_id: None,
                fields: fields
                    .iter()
                    .enumerate()
                    .map(|(index, field)| AwbcRecordField {
                        field: RuntimeRecordFieldId::try_from_zero_based_ordinal(index).unwrap(),
                        name: Some(string(&mut program, field.diagnostic_name())),
                        ty: *field.ty(),
                    })
                    .collect(),
            },
            P::Nominal {
                nominal,
                layout,
                arguments,
            } => {
                let domain = plan.nominal_record_domains().get(source).unwrap();
                A::NominalRecord {
                    public_id: string(&mut program, nominal.as_str()),
                    layout: *layout.as_bytes(),
                    arguments: arguments.into_vec(),
                    shape: domain.shape(),
                    fields: domain
                        .fields()
                        .iter()
                        .map(|field| AwbcRecordField {
                            field: field.field(),
                            name: field.name().map(|name| string(&mut program, name)),
                            ty: id(plan, field.ty()),
                        })
                        .collect(),
                }
            }
            P::Opaque {
                producer,
                admission,
                value_class,
                persistence,
                arguments,
            } => A::Opaque {
                producer: string(&mut program, producer.as_str()),
                admission,
                value_class,
                persistence,
                arguments: arguments.into_vec(),
            },
            P::Agent(agent) => A::Agent(match agent {
                RuntimeAgentTypeProjection::Probe(item) => AwbcAgentTypeShape::Probe(item),
                RuntimeAgentTypeProjection::DataShape(item) => AwbcAgentTypeShape::DataShape(item),
                other => AwbcAgentTypeShape::Leaf(other.operational_type()),
            }),
            P::Result {
                value_payload,
                error_payload,
                ..
            } => builtin(
                &mut program,
                crate::pattern::RuntimeBuiltinVariantIdentity::Result,
                &[Some(value_payload), Some(error_payload)],
            ),
            P::Option { some_payload, .. } => builtin(
                &mut program,
                crate::pattern::RuntimeBuiltinVariantIdentity::Option,
                &[Some(some_payload), None],
            ),
            P::BuiltinVariant { owner, cases } => builtin(&mut program, owner, &cases),
        };
        let mut ty =
            AwbcRuntimeType::new(row.semantic_identity(), shape).with_scope(row.scope().clone());
        if let Some(declaration) = row.nominal_declaration() {
            ty = ty.with_nominal_declaration(declaration);
        }
        if let Some(codec) = plan
            .nominal_record_domains()
            .get(source)
            .and_then(|domain| domain.data_codec())
        {
            ty = ty
                .with_data_codec(codec.body.clone())
                .with_data_codec_arguments(codec.arguments.to_vec());
        } else if let Some(codec) = row.data_codec() {
            ty = ty.with_data_codec(codec.clone());
        }
        program.runtime_types.push(ty);
    }
    program.canonicalize_string_table();
    program
        .verify(AwbcVerifyBudget::default(), context())
        .unwrap();
    program
}

fn context() -> AwbcVerifyContext<'static> {
    AwbcVerifyContext {
        require_entrypoint: false,
        ..AwbcVerifyContext::default()
    }
}
fn meter() -> TaskSemanticMeter {
    TaskSemanticMeter::new(10_000_000, 10_000_000)
}

#[test]
fn original_awbc_type_relation_accepts_the_complete_algebra_in_a_different_row_order() {
    let plan = original();
    let code = executable(&plan);
    let mut work = meter();
    let mut relation = RuntimeAwbcSourceTypeRelation::new(
        &plan,
        &code,
        RuntimeTaskPlanSealLimits::default(),
        &mut work,
    );
    let mut kinds = [false; 38];
    for (ty, row) in plan.type_table().declarations_with_ids() {
        kinds[row.projection().executable_semantic_kind() as usize] = true;
        relation.admit(ty, id(&plan, ty)).unwrap();
    }
    assert!(kinds.into_iter().all(|kind| kind));
    assert_eq!(relation.seen.len(), plan.type_table().len());
}

#[test]
fn traversal_type_survives_awbc_wire_round_trip_and_rejects_foreign_shape_edges() {
    let plan = original();
    let code = executable(&plan);
    let bytes = code.encode_canonical().unwrap();
    let decoded =
        AwbcProgram::decode_canonical(&bytes, crate::awbc::codec::AwbcDecodeBudget::default())
            .unwrap();
    decoded
        .verify(AwbcVerifyBudget::default(), context())
        .unwrap();
    assert_eq!(decoded.encode_canonical().unwrap(), bytes);
    let (traversal, _) = plan
        .type_table()
        .declarations_with_ids()
        .find(|(_, row)| {
            matches!(
                row.projection(),
                RuntimePlanTypeProjection::Traversal { .. }
            )
        })
        .unwrap();
    let traversal_id = id(&plan, traversal);
    let row = &decoded.runtime_types[traversal_id.index()];
    let AwbcRuntimeTypeShape::Traversal { source, mapping } = row.shape() else {
        panic!("wire codec must retain the traversal head")
    };
    let source = *source;
    let mapping = *mapping;
    assert_ne!(source, mapping);
    assert!(
        RuntimeAwbcSourceTypeRelation::new(
            &plan,
            &decoded,
            RuntimeTaskPlanSealLimits::default(),
            &mut meter()
        )
        .admit(traversal, traversal_id)
        .is_ok()
    );
    for (source, mapping) in [(mapping, mapping), (source, source)] {
        let mut candidate = decoded.clone();
        candidate.runtime_types[traversal_id.index()] = AwbcRuntimeType::new(
            row.semantic_identity(),
            AwbcRuntimeTypeShape::Traversal { source, mapping },
        )
        .with_scope(row.scope().clone());
        assert!(
            candidate
                .verify(AwbcVerifyBudget::default(), context())
                .is_err()
        );
    }
}

#[test]
fn original_awbc_type_relation_rejects_an_incorrect_shape_for_every_original_type() {
    let plan = original();
    let code = executable(&plan);
    for (ty, source) in plan.type_table().declarations_with_ids() {
        let mut changed = code.clone();
        let shape = if matches!(source.projection(), RuntimePlanTypeProjection::String) {
            AwbcRuntimeTypeShape::Unit
        } else {
            AwbcRuntimeTypeShape::String
        };
        changed.runtime_types[id(&plan, ty).index()] =
            AwbcRuntimeType::new(source.semantic_identity(), shape)
                .with_scope(source.scope().clone());
        assert!(
            RuntimeAwbcSourceTypeRelation::new(
                &plan,
                &changed,
                RuntimeTaskPlanSealLimits::default(),
                &mut meter()
            )
            .admit(ty, id(&plan, ty))
            .is_err(),
            "{:?}",
            source.projection()
        );
    }
}

#[test]
fn original_awbc_type_relation_keeps_distinct_tails_after_an_already_seen_prefix() {
    let plan = original();
    let mut code = executable(&plan);
    let (unit, _) = plan
        .type_table()
        .declarations_with_ids()
        .find(|(_, row)| matches!(row.projection(), RuntimePlanTypeProjection::Unit))
        .unwrap();
    let (boolean, _) = plan
        .type_table()
        .declarations_with_ids()
        .find(|(_, row)| matches!(row.projection(), RuntimePlanTypeProjection::Bool))
        .unwrap();
    let (choice, _) = plan
        .type_table()
        .declarations_with_ids()
        .find(|(_, row)| matches!(row.projection(), RuntimePlanTypeProjection::Choice(_)))
        .unwrap();
    let (payload, row) = plan
        .type_table()
        .declarations_with_ids()
        .find(|(_, row)| matches!(row.projection(), RuntimePlanTypeProjection::Tuple(_)))
        .unwrap();
    code.runtime_types[id(&plan, payload).index()] = AwbcRuntimeType::new(
        row.semantic_identity(),
        AwbcRuntimeTypeShape::Tuple(vec![id(&plan, boolean)]),
    );
    code.verify(AwbcVerifyBudget::default(), context()).unwrap();
    let mut work = meter();
    let mut relation = RuntimeAwbcSourceTypeRelation::new(
        &plan,
        &code,
        RuntimeTaskPlanSealLimits::default(),
        &mut work,
    );
    relation.admit(unit, id(&plan, unit)).unwrap();
    assert!(matches!(
        relation.admit(choice, id(&plan, choice)),
        Err(RuntimeAwbcSourceTypeError::Mismatch {
            field: RuntimeAwbcSourceTypeField::Identity,
            ..
        })
    ));
    assert!(matches!(
        relation.admit(choice, id(&plan, choice)),
        Err(RuntimeAwbcSourceTypeError::Encoding(
            TaskSemanticEncodingError::OwnerRejected
        ))
    ));
}

#[test]
fn original_awbc_type_relation_shares_exact_work_and_keeps_a_short_limit_failure() {
    let plan = original();
    let code = executable(&plan);
    let run = |work: &mut TaskSemanticMeter| {
        let mut relation = RuntimeAwbcSourceTypeRelation::new(
            &plan,
            &code,
            RuntimeTaskPlanSealLimits::default(),
            work,
        );
        for (ty, _) in plan.type_table().declarations_with_ids() {
            relation.admit(ty, id(&plan, ty))?;
        }
        Ok::<_, RuntimeAwbcSourceTypeError>(())
    };
    let mut measured = meter();
    run(&mut measured).unwrap();
    let total = measured.totals();
    assert!(total.0 > plan.type_table().len() as u64);
    let mut exact = TaskSemanticMeter::new(total.0, total.1);
    run(&mut exact).unwrap();
    assert_eq!(exact.totals(), total);
    let mut short = TaskSemanticMeter::new(total.0 - 1, total.1);
    for _ in 0..2 {
        assert!(matches!(
            run(&mut short),
            Err(RuntimeAwbcSourceTypeError::Encoding(
                TaskSemanticEncodingError::SemanticWork
            ))
        ));
        assert_eq!(short.status(), Err(TaskSemanticEncodingError::SemanticWork));
    }
}

#[test]
fn original_awbc_type_relation_checks_real_codec_policy_and_its_exact_byte_allowance() {
    use crate::entry::schema::RuntimeBytesFormat;
    let semantic = crate::pattern::RuntimeCheckedType::Bytes.semantic_identity_digest();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(semantic, RuntimePlanTypeProjection::Bytes)
                    .with_data_codec(RuntimeCodecUse::Bytes {
                        format: RuntimeBytesFormat::Hex,
                    }),
            ],
            [],
        )
        .unwrap();
    let plan = builder.finish().unwrap();
    let code = executable(&plan);
    let ty = plan.type_table().id_for_semantic(semantic).unwrap();
    let mut measured = meter();
    RuntimeAwbcSourceTypeRelation::new(
        &plan,
        &code,
        RuntimeTaskPlanSealLimits::default(),
        &mut measured,
    )
    .admit(ty, id(&plan, ty))
    .unwrap();
    let total = measured.totals();
    assert!(total.1 > 0);
    let mut exact = TaskSemanticMeter::new(total.0, total.1);
    RuntimeAwbcSourceTypeRelation::new(
        &plan,
        &code,
        RuntimeTaskPlanSealLimits::default(),
        &mut exact,
    )
    .admit(ty, id(&plan, ty))
    .unwrap();
    assert_eq!(exact.totals(), total);
    let mut short = TaskSemanticMeter::new(total.0, total.1 - 1);
    for _ in 0..2 {
        let error = RuntimeAwbcSourceTypeRelation::new(
            &plan,
            &code,
            RuntimeTaskPlanSealLimits::default(),
            &mut short,
        )
        .admit(ty, id(&plan, ty))
        .unwrap_err();
        assert!(matches!(
            error,
            RuntimeAwbcSourceTypeError::Encoding(TaskSemanticEncodingError::TranscriptBytes)
                | RuntimeAwbcSourceTypeError::Policy(RuntimeBodySemanticError::Encoding(
                    TaskSemanticEncodingError::TranscriptBytes
                ))
        ));
        assert_eq!(
            short.status(),
            Err(TaskSemanticEncodingError::TranscriptBytes)
        );
    }
    let mut changed = code.clone();
    let index = id(&plan, ty).index();
    changed.runtime_types[index] =
        changed.runtime_types[index]
            .clone()
            .with_data_codec(RuntimeCodecUse::Bytes {
                format: RuntimeBytesFormat::Base64,
            });
    changed
        .verify(AwbcVerifyBudget::default(), context())
        .unwrap();
    assert!(matches!(
        RuntimeAwbcSourceTypeRelation::new(
            &plan,
            &changed,
            RuntimeTaskPlanSealLimits::default(),
            &mut meter()
        )
        .admit(ty, id(&plan, ty)),
        Err(RuntimeAwbcSourceTypeError::Mismatch {
            field: RuntimeAwbcSourceTypeField::Codec,
            ..
        })
    ));
}

#[test]
fn original_awbc_type_relation_allows_a_leaf_with_zero_children_and_bounds_real_children() {
    let plan = original();
    let code = executable(&plan);
    let unit = plan
        .type_table()
        .declarations_with_ids()
        .find(|(_, row)| matches!(row.projection(), RuntimePlanTypeProjection::Unit))
        .unwrap()
        .0;
    let tuple = plan
        .type_table()
        .declarations_with_ids()
        .find(|(_, row)| matches!(row.projection(), RuntimePlanTypeProjection::Tuple(_)))
        .unwrap()
        .0;
    let limits = RuntimeTaskPlanSealLimits {
        max_children_per_row: 0,
        ..RuntimeTaskPlanSealLimits::default()
    };
    let mut work = meter();
    let mut relation = RuntimeAwbcSourceTypeRelation::new(&plan, &code, limits, &mut work);
    relation.admit(unit, id(&plan, unit)).unwrap();
    assert!(matches!(
        relation.admit(tuple, id(&plan, tuple)),
        Err(RuntimeAwbcSourceTypeError::Children {
            actual: 1,
            maximum: 0
        })
    ));
    assert!(matches!(
        relation.admit(unit, id(&plan, unit)),
        Err(RuntimeAwbcSourceTypeError::Encoding(
            TaskSemanticEncodingError::OwnerRejected
        ))
    ));
}
