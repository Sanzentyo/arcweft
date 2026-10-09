use super::*;
use crate::engine::{Engine, FlowFiberStatus};
use crate::entry::{
    FlowParameterCoordinate, RuntimeFlowExecutableParameter, RuntimeFlowParameterMode,
    RuntimeNominalDeclarationId, RuntimeNominalRecordShape, RuntimeNominalSchemaBody,
    RuntimeNominalSchemaDefinition, RuntimeNominalSchemaField, RuntimeNominalSchemaGraph,
    RuntimeNominalSchemaIdentity, RuntimeNominalTypeId, RuntimeSchemaLimits, RuntimeTypeSchema,
};
use crate::pattern::{RuntimeBuiltinVariantIdentity, RuntimeSemanticTypeId};
use crate::plan::{
    FlowRuntimeId, RuntimeEffectSet, RuntimeExecutableBodySeed, RuntimeFunctionDefinitionIdentity,
    RuntimeFunctionInputBindingSeed, RuntimeFunctionInputOrigin,
    RuntimeFunctionInputOwnershipRequirement, RuntimeFunctionInputTransfer,
    RuntimeFunctionParameterIdentity, RuntimeFunctionParameterPassing, RuntimeFunctionSemanticRole,
    RuntimeFunctionSiteDeclarationSeed, RuntimeLocalDeclarationSeed, RuntimeLocalReadSeed,
    RuntimeNominalRecordDomainFieldSeed, RuntimeNominalRecordDomainSeed, RuntimePlanTypeSeed,
};
use crate::pure::VmPureFunctionScratch;
use crate::step::{RuntimeStepInput, RuntimeStepOptions};
use crate::value::{RuntimeFieldTarget, RuntimeLocalReadMode, RuntimeNominalRecordValue};
use std::sync::Arc;

fn identity(marker: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([marker; 32])
}

fn record_fixture() -> (RuntimePlanBuilder, Vec<RuntimeLocalSeedId>) {
    let nominal = RuntimeNominalTypeId::try_new("fixture.AffineInspectedRecord").unwrap();
    let nominal_identity = RuntimeNominalSchemaIdentity::new(nominal.clone(), identity(3));
    let field = |ordinal| RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap();
    let schemas = RuntimeNominalSchemaGraph::try_new(
        vec![RuntimeNominalSchemaDefinition::new(
            RuntimeNominalDeclarationId::from_bytes(
                *nominal_identity.semantic_identity().as_bytes(),
            ),
            nominal_identity,
            Vec::new(),
            RuntimeNominalSchemaBody::Record {
                shape: RuntimeNominalRecordShape::Record,
                fields: Box::new([
                    RuntimeNominalSchemaField::new(
                        field(0),
                        Some("uri".into()),
                        RuntimeTypeSchema::String,
                    ),
                    RuntimeNominalSchemaField::new(
                        field(1),
                        Some("body".into()),
                        RuntimeTypeSchema::ExecutableRef(identity(2)),
                    ),
                ]),
            },
        )],
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap();
    let layout = schemas.try_layout_hash(identity(3)).unwrap();
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(identity(1), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(identity(2), RuntimePlanTypeProjection::Need(identity(1))),
                RuntimePlanTypeSeed::new(
                    identity(3),
                    RuntimePlanTypeProjection::Nominal {
                        nominal,
                        layout,
                        arguments: Box::new([]),
                    },
                ),
                RuntimePlanTypeSeed::new(
                    identity(4),
                    RuntimePlanTypeProjection::Tuple(Box::new([
                        identity(1),
                        identity(1),
                        identity(3),
                    ])),
                ),
            ],
            [
                ("flow_owner", identity(3)),
                ("first_uri", identity(1)),
                ("second_uri", identity(1)),
                ("function_owner", identity(3)),
            ]
            .map(|(name, ty)| {
                RuntimeLocalDeclarationSeed::new(
                    manual_local_source(&format!("arcweft-core.fixture.field_inspection.{name}")),
                    ty,
                )
            }),
            [RuntimeNominalRecordDomainSeed::new(
                identity(3),
                RuntimeNominalRecordShape::Record,
                [
                    RuntimeNominalRecordDomainFieldSeed::new(
                        field(0),
                        Some("uri".into()),
                        identity(1),
                    ),
                    RuntimeNominalRecordDomainFieldSeed::new(
                        field(1),
                        Some("body".into()),
                        identity(2),
                    ),
                ],
            )],
            [],
            &schemas,
        )
        .expect("record and exact local declaration graph admits");
    (builder, admission.local_ids().to_vec())
}

fn record_value(
    builder: &RuntimePlanBuilder,
    need: crate::task::RuntimeNeedHandle,
) -> RuntimeValue {
    let ty = builder
        .resolve_seed_type("fixture nominal", identity(3))
        .unwrap();
    let RuntimePlanTypeProjection::Nominal {
        nominal, layout, ..
    } = builder.types.get(ty).unwrap().projection()
    else {
        unreachable!("the fixture admits an exact nominal owner");
    };
    RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
        nominal.clone(),
        identity(3),
        *layout,
        vec![
            RuntimeValue::String("resource://one".into()),
            RuntimeValue::NeedHandle(need),
        ],
    ))
}

fn inspection(local: &RuntimeLocalSeedId, ordinal: u32, result: u8) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        identity(result),
        RuntimeExprSeedKind::Field {
            target: super::super::RuntimeFieldTargetSeed::Inspect(RuntimeMutablePlaceSeed::Local(
                local.clone(),
            )),
            field: RuntimeFieldProjectionSeed::Nominal {
                owner: identity(3),
                field: RuntimeRecordFieldSeedId::from_zero_based(ordinal),
            },
        },
    )
}

#[test]
fn field_inspection_admits_copy_result_from_an_affine_record() {
    let (builder, locals) = record_fixture();
    let expression = builder
        .lower_expression(inspection(&locals[0], 0, 1))
        .unwrap();
    let (owner, owner_ty) = locals[0].resolve(&builder.issuer).unwrap();
    assert_eq!(
        expression.ty(),
        builder
            .resolve_seed_type("fixture result", identity(1))
            .unwrap()
    );
    assert!(matches!(expression.kind(), RuntimeExprKind::Field {
        target: RuntimeFieldTarget::Inspect { place: RuntimeMutablePlace::Local(local), ty },
        field: RuntimeFieldProjection::Nominal(field),
    } if *local == owner && *ty == owner_ty && field.zero_based() == 0));
    let (_registry, need) = crate::tests::pending_need(identity(1));
    let value = record_value(&builder, need);
    assert!(
        !value.ownership().permits_copy(),
        "the receiver contains a real affine Need"
    );
}

#[test]
fn field_inspection_rejects_a_need_result_at_construction() {
    let (builder, locals) = record_fixture();
    let need_ty = builder
        .resolve_seed_type("fixture Need", identity(2))
        .unwrap();
    assert_eq!(
        builder.lower_expression(inspection(&locals[0], 1, 2)),
        Err(RuntimePlanBuildError::InvalidTypeProjection {
            context: "Copy field inspection result",
            ty: need_ty,
        })
    );
    assert!(
        builder
            .lower_expression(inspection(&locals[0], 0, 1))
            .is_ok(),
        "rejecting an affine child does not reject the owner's Copy child"
    );
}

#[test]
fn field_inspection_rejects_resource_body_at_construction() {
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(identity(1), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(
                    identity(2),
                    RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::Resource),
                ),
                RuntimePlanTypeSeed::new(identity(3), RuntimePlanTypeProjection::AgentValue),
                RuntimePlanTypeSeed::new(
                    identity(4),
                    RuntimePlanTypeProjection::Agent(
                        RuntimeAgentTypeProjection::BinaryResourceBody,
                    ),
                ),
                RuntimePlanTypeSeed::new(
                    identity(5),
                    RuntimePlanTypeProjection::Tuple(Box::new([identity(3)])),
                ),
                RuntimePlanTypeSeed::new(
                    identity(6),
                    RuntimePlanTypeProjection::Tuple(Box::new([identity(1)])),
                ),
                RuntimePlanTypeSeed::new(
                    identity(7),
                    RuntimePlanTypeProjection::Tuple(Box::new([identity(4)])),
                ),
                RuntimePlanTypeSeed::new(
                    identity(8),
                    RuntimePlanTypeProjection::BuiltinVariant {
                        owner: RuntimeBuiltinVariantIdentity::AgentResourceBody,
                        cases: Box::new([Some(identity(5)), Some(identity(6)), Some(identity(7))]),
                    },
                ),
            ],
            [RuntimeLocalDeclarationSeed::new(
                manual_local_source("arcweft-core.fixture.field_inspection.resource"),
                identity(2),
            )],
        )
        .unwrap();
    let field = |result, projection| {
        RuntimeExprSeed::new(
            identity(result),
            RuntimeExprSeedKind::Field {
                target: super::super::RuntimeFieldTargetSeed::Inspect(
                    RuntimeMutablePlaceSeed::Local(admission.local_ids()[0].clone()),
                ),
                field: RuntimeFieldProjectionSeed::Agent(projection),
            },
        )
    };
    let body_ty = builder
        .resolve_seed_type("fixture ResourceBody", identity(8))
        .unwrap();
    assert_eq!(
        builder.lower_expression(field(8, crate::value::RuntimeAgentField::ResourceBody)),
        Err(RuntimePlanBuildError::InvalidTypeProjection {
            context: "Copy field inspection result",
            ty: body_ty,
        })
    );
    builder
        .lower_expression(field(1, crate::value::RuntimeAgentField::ResourceUri))
        .expect("the same affine Resource still permits its fixed String field");
}

fn formal(
    local: RuntimeLocalSeedId,
    parameter: RuntimeFunctionParameterIdentity,
) -> RuntimeFunctionInputBindingSeed {
    RuntimeFunctionInputBindingSeed {
        transfer: RuntimeFunctionInputTransfer::Formal,
        origin: RuntimeFunctionInputOrigin::Parameter(parameter),
        source: RuntimeFunctionInputSource::Parameter {
            position: 0,
            passing: RuntimeFunctionParameterPassing::Affine,
        },
        input_local: local.clone(),
        pattern: RuntimePatternSeed::new(
            identity(3),
            RuntimePatternSeedKind::Bind {
                mutable: false,
                local,
            },
        ),
        ownership: RuntimeFunctionInputOwnershipRequirement::Owned,
        unrestricted_bindings: Box::new([]),
    }
}

#[test]
fn field_inspection_native_and_pure_execution_preserve_an_affine_receiver_until_move() {
    let (mut builder, locals) = record_fixture();
    let flow = FlowRuntimeId::canonical("field.inspect").unwrap();
    let flow_parameter = RuntimeFunctionParameterIdentity::from_accepted_identity([92; 32]);
    let function_parameter = RuntimeFunctionParameterIdentity::from_accepted_identity([93; 32]);
    let local_ids = locals
        .iter()
        .map(|local| local.resolve(&builder.issuer).unwrap().0)
        .collect::<Vec<_>>();
    let function = builder
        .push_function_site_seed(
            RuntimeFunctionDefinitionIdentity::from_accepted_identity([41; 32]),
            RuntimeFunctionSemanticRole::Ordinary,
            [formal(locals[3].clone(), function_parameter)],
            RuntimeExprSeed::new(
                identity(4),
                RuntimeExprSeedKind::Tuple(Box::new([
                    inspection(&locals[3], 0, 1),
                    inspection(&locals[3], 0, 1),
                    RuntimeExprSeed::new(
                        identity(3),
                        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                            locals[3].clone(),
                            RuntimeLocalReadMode::Move,
                        )),
                    ),
                ])),
            ),
        )
        .unwrap();
    let function_id = function.resolve(&builder.issuer).unwrap().0;
    builder
        .push_flow_schema(crate::plan::RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: vec![RuntimeFlowExecutableParameter {
                identity: flow_parameter,
                coordinate: FlowParameterCoordinate::from_position(0),
                name: "owner".into(),
                mode: RuntimeFlowParameterMode::Owned,
                passing: RuntimeFunctionParameterPassing::Affine,
                semantic_identity: identity(3),
            }],
        })
        .unwrap();
    builder
        .push_flow_seed(crate::plan::RuntimeFlowSeed::new(
            flow.clone(),
            RuntimeFunctionSiteDeclarationSeed::flow(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([61; 32]),
                None,
                Box::new([formal(locals[0].clone(), flow_parameter)]),
                identity(3),
                RuntimeEffectSet::empty(),
            ),
            RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([
                    RuntimeFlowOpSeed::Let {
                        pattern: RuntimePatternSeed::new(
                            identity(1),
                            RuntimePatternSeedKind::Bind {
                                mutable: false,
                                local: locals[1].clone(),
                            },
                        ),
                        expr: inspection(&locals[0], 0, 1),
                    },
                    RuntimeFlowOpSeed::Let {
                        pattern: RuntimePatternSeed::new(
                            identity(1),
                            RuntimePatternSeedKind::Bind {
                                mutable: false,
                                local: locals[2].clone(),
                            },
                        ),
                        expr: inspection(&locals[0], 0, 1),
                    },
                    RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                        identity(3),
                        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                            locals[0].clone(),
                            RuntimeLocalReadMode::Move,
                        )),
                    )),
                ]),
            },
        ))
        .unwrap();
    let (registry, need) = crate::tests::pending_need(identity(1));
    let value = record_value(&builder, need);
    let plan = builder.finish().unwrap();
    let pure_plan = Arc::new(plan.clone());
    assert!(!value.ownership().permits_copy());
    let mut scratch = VmPureFunctionScratch::default();
    assert_eq!(
        scratch
            .evaluate_function_site(&pure_plan, function_id, vec![value.clone()])
            .unwrap(),
        RuntimeValue::Tuple(vec![
            RuntimeValue::String("resource://one".into()),
            RuntimeValue::String("resource://one".into()),
            value.clone()
        ])
    );

    let invocation = plan
        .seal_flow_invocation(
            flow,
            [crate::value::RuntimeFlowParameterBinding {
                parameter: FlowParameterCoordinate::from_position(0),
                value: value.clone(),
            }],
        )
        .unwrap();
    let mut engine = Engine::for_flow_invocation_with_need_context(
        invocation,
        crate::task::GenerationId::new(0),
        registry,
    )
    .unwrap();
    let mut options = RuntimeStepOptions::default();
    options.budget.max_ops = 1;
    for inspected in [local_ids[1], local_ids[2]] {
        let step = engine.step(RuntimeStepInput::default(), options);
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
        assert_eq!(engine.fiber().env.get(local_ids[0]), Some(&value));
        assert_eq!(
            engine.fiber().env.get(inspected),
            Some(&RuntimeValue::String("resource://one".into()))
        );
        assert!(matches!(engine.fiber().status, FlowFiberStatus::Running));
    }
    let completed = engine.step(RuntimeStepInput::default(), options);
    assert!(
        completed.output.diagnostics.is_empty(),
        "{:?}",
        completed.output.diagnostics
    );
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Done(_)));
    assert!(
        engine.fiber().env.get(local_ids[0]).is_none(),
        "only the final Move consumes the receiver"
    );
}

struct PartialMoveInspectionFixture {
    plan: Arc<crate::plan::RuntimePlan>,
    function: crate::runtime_id::RuntimeFunctionSiteId,
    flow: FlowRuntimeId,
    owner: crate::runtime_id::RuntimeLocalDeclarationId,
    body: crate::runtime_id::RuntimeLocalDeclarationId,
    uris: [crate::runtime_id::RuntimeLocalDeclarationId; 2],
    value: RuntimeValue,
    need: RuntimeValue,
    registry: crate::task::NeedProducerRegistry,
}

fn moved_child(local: &RuntimeLocalSeedId, ordinal: u32, result: u8) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        identity(result),
        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new_place(
            local.clone(),
            RuntimeLocalReadMode::Move,
            Box::new([RuntimeRecordFieldSeedId::from_zero_based(ordinal)]),
        )),
    )
}

fn partial_move_inspection_fixture() -> PartialMoveInspectionFixture {
    let (mut builder, locals) = record_fixture();
    let additional = builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                identity(5),
                RuntimePlanTypeProjection::Tuple(Box::new([identity(2), identity(1), identity(1)])),
            )],
            [RuntimeLocalDeclarationSeed::new(
                manual_local_source("arcweft-core.fixture.field_inspection.moved_body"),
                identity(2),
            )],
        )
        .unwrap();
    let body = additional.local_ids()[0].clone();
    let flow = FlowRuntimeId::canonical("field.partial_inspect").unwrap();
    let flow_parameter = RuntimeFunctionParameterIdentity::from_accepted_identity([94; 32]);
    let function_parameter = RuntimeFunctionParameterIdentity::from_accepted_identity([95; 32]);
    let function = builder
        .push_function_site_seed(
            RuntimeFunctionDefinitionIdentity::from_accepted_identity([42; 32]),
            RuntimeFunctionSemanticRole::Ordinary,
            [formal(locals[3].clone(), function_parameter)],
            RuntimeExprSeed::new(
                identity(5),
                RuntimeExprSeedKind::Tuple(Box::new([
                    moved_child(&locals[3], 1, 2),
                    inspection(&locals[3], 0, 1),
                    inspection(&locals[3], 0, 1),
                ])),
            ),
        )
        .unwrap()
        .resolve(&builder.issuer)
        .unwrap()
        .0;
    builder
        .push_flow_schema(crate::plan::RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: vec![RuntimeFlowExecutableParameter {
                identity: flow_parameter,
                coordinate: FlowParameterCoordinate::from_position(0),
                name: "owner".into(),
                mode: RuntimeFlowParameterMode::Owned,
                passing: RuntimeFunctionParameterPassing::Affine,
                semantic_identity: identity(3),
            }],
        })
        .unwrap();
    let bind = |ty, local| {
        RuntimePatternSeed::new(
            identity(ty),
            RuntimePatternSeedKind::Bind {
                mutable: false,
                local,
            },
        )
    };
    let read = |ty, local, mode| {
        RuntimeExprSeed::new(
            identity(ty),
            RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(local, mode)),
        )
    };
    builder
        .push_flow_seed(crate::plan::RuntimeFlowSeed::new(
            flow.clone(),
            RuntimeFunctionSiteDeclarationSeed::flow(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([62; 32]),
                None,
                Box::new([formal(locals[0].clone(), flow_parameter)]),
                identity(5),
                RuntimeEffectSet::empty(),
            ),
            RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([
                    RuntimeFlowOpSeed::Let {
                        pattern: bind(2, body.clone()),
                        expr: moved_child(&locals[0], 1, 2),
                    },
                    RuntimeFlowOpSeed::Let {
                        pattern: bind(1, locals[1].clone()),
                        expr: inspection(&locals[0], 0, 1),
                    },
                    RuntimeFlowOpSeed::Let {
                        pattern: bind(1, locals[2].clone()),
                        expr: inspection(&locals[0], 0, 1),
                    },
                    RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                        identity(5),
                        RuntimeExprSeedKind::Tuple(Box::new([
                            read(2, body.clone(), RuntimeLocalReadMode::Move),
                            read(1, locals[1].clone(), RuntimeLocalReadMode::Copy),
                            read(1, locals[2].clone(), RuntimeLocalReadMode::Copy),
                        ])),
                    )),
                ]),
            },
        ))
        .unwrap();
    let owner = locals[0].resolve(&builder.issuer).unwrap().0;
    let body_id = body.resolve(&builder.issuer).unwrap().0;
    let uris = [
        locals[1].resolve(&builder.issuer).unwrap().0,
        locals[2].resolve(&builder.issuer).unwrap().0,
    ];
    let (registry, need) = crate::tests::pending_need(identity(1));
    let value = record_value(&builder, need.clone());
    PartialMoveInspectionFixture {
        plan: Arc::new(builder.finish().unwrap()),
        function,
        flow,
        owner,
        body: body_id,
        uris,
        value,
        need: RuntimeValue::NeedHandle(need),
        registry,
    }
}

#[test]
fn nominal_field_inspection_pure_reads_the_live_child_after_an_affine_sibling_move() {
    let fixture = partial_move_inspection_fixture();
    let mut scratch = VmPureFunctionScratch::default();
    assert_eq!(
        scratch.evaluate_function_site(&fixture.plan, fixture.function, vec![fixture.value]),
        Ok(RuntimeValue::Tuple(vec![
            fixture.need,
            RuntimeValue::String("resource://one".into()),
            RuntimeValue::String("resource://one".into()),
        ])),
    );
}

#[test]
fn nominal_field_inspection_native_reads_the_live_child_after_an_affine_sibling_move() {
    let fixture = partial_move_inspection_fixture();
    let invocation = Arc::try_unwrap(fixture.plan)
        .expect("native fixture owns its exact immutable plan")
        .seal_flow_invocation(
            fixture.flow,
            [crate::value::RuntimeFlowParameterBinding {
                parameter: FlowParameterCoordinate::from_position(0),
                value: fixture.value,
            }],
        )
        .unwrap();
    let mut engine = Engine::for_flow_invocation_with_need_context(
        invocation,
        crate::task::GenerationId::new(0),
        fixture.registry,
    )
    .unwrap();
    let mut options = RuntimeStepOptions::default();
    options.budget.max_ops = 1;
    let moved = engine.step(RuntimeStepInput::default(), options);
    assert!(
        moved.output.diagnostics.is_empty(),
        "{:?}",
        moved.output.diagnostics
    );
    assert!(
        engine.fiber().env.get(fixture.owner).is_none(),
        "the whole partial owner cannot be read"
    );
    assert_eq!(engine.fiber().env.get(fixture.body), Some(&fixture.need));
    for uri in fixture.uris {
        let inspected = engine.step(RuntimeStepInput::default(), options);
        assert!(
            inspected.output.diagnostics.is_empty(),
            "{:?}",
            inspected.output.diagnostics
        );
        assert_eq!(
            engine.fiber().env.get(uri),
            Some(&RuntimeValue::String("resource://one".into()))
        );
        assert_eq!(engine.fiber().env.get(fixture.body), Some(&fixture.need));
        assert!(engine.fiber().env.get(fixture.owner).is_none());
    }
    let completed = engine.step(RuntimeStepInput::default(), options);
    assert!(
        completed.output.diagnostics.is_empty(),
        "{:?}",
        completed.output.diagnostics
    );
    let FlowFiberStatus::Done(crate::engine::FlowExit::Return(label)) = &engine.fiber().status
    else {
        panic!(
            "the moved Need and two inspected URIs must return: {:?}",
            engine.fiber().status
        );
    };
    assert_eq!(
        label,
        &crate::value::runtime_value_label(&RuntimeValue::Tuple(vec![
            fixture.need,
            RuntimeValue::String("resource://one".into()),
            RuntimeValue::String("resource://one".into()),
        ]))
    );
    assert!(
        engine.fiber().env.get(fixture.body).is_none(),
        "the Need moves once into the return"
    );
}

#[test]
fn nominal_field_inspection_pure_rejects_moved_selected_children_and_whole_partial_owners() {
    for selected_child in [true, false] {
        let (mut builder, locals) = record_fixture();
        let local = locals[3].clone();
        let (first_ty, second_ty) = if selected_child {
            (identity(1), identity(1))
        } else {
            (identity(2), identity(3))
        };
        builder
            .admit_type_batch(
                [RuntimePlanTypeSeed::new(
                    identity(5),
                    RuntimePlanTypeProjection::Tuple(Box::new([first_ty, second_ty])),
                )],
                [],
            )
            .unwrap();
        let (first, second) = if selected_child {
            (moved_child(&local, 0, 1), inspection(&local, 0, 1))
        } else {
            (
                moved_child(&local, 1, 2),
                RuntimeExprSeed::new(
                    identity(3),
                    RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                        local.clone(),
                        RuntimeLocalReadMode::Move,
                    )),
                ),
            )
        };
        let site = builder
            .push_function_site_seed(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([43; 32]),
                RuntimeFunctionSemanticRole::Ordinary,
                [formal(
                    local.clone(),
                    RuntimeFunctionParameterIdentity::from_accepted_identity([96; 32]),
                )],
                RuntimeExprSeed::new(
                    identity(5),
                    RuntimeExprSeedKind::Tuple(Box::new([first, second])),
                ),
            )
            .unwrap()
            .resolve(&builder.issuer)
            .unwrap()
            .0;
        let expected = crate::value::RuntimeEvalError::UninitializedLocal(
            local.resolve(&builder.issuer).unwrap().0,
        );
        let (_registry, need) = crate::tests::pending_need(identity(1));
        let value = record_value(&builder, need);
        let plan = Arc::new(builder.finish().unwrap());
        assert_eq!(
            VmPureFunctionScratch::default().evaluate_function_site(&plan, site, vec![value]),
            Err(expected),
            "selected_child = {selected_child}"
        );
    }
}

#[test]
fn entity_reference_field_coordinates_are_closed_and_normalize_source_seeds() {
    use crate::value::{RuntimeAgentField, RuntimeEntityReferenceField};
    for field in RuntimeEntityReferenceField::ALL {
        assert_eq!(
            RuntimeEntityReferenceField::from_label(field.as_label()),
            Some(field)
        );
        let agent = RuntimeAgentField::from(field);
        assert_eq!(agent.as_label(), field.as_label());
        assert_eq!(RuntimeEntityReferenceField::try_from(agent), Ok(field));
    }
    for name in ["", "Id", "target", "parent_id", "id.extra"] {
        assert_eq!(
            RuntimeEntityReferenceField::from_label(name),
            None,
            "{name}"
        );
    }
    assert_eq!(
        RuntimeEntityReferenceField::try_from(RuntimeAgentField::ResourceUri),
        Err(RuntimeAgentField::ResourceUri)
    );
    let (builder, locals) = entity_reference_field_fixture();
    for field in RuntimeEntityReferenceField::ALL {
        for projection in [
            RuntimeFieldProjectionSeed::Agent(field.into()),
            RuntimeFieldProjectionSeed::EntityReference(field),
        ] {
            let expr = builder
                .lower_expression(RuntimeExprSeed::new(
                    identity(2),
                    RuntimeExprSeedKind::Field {
                        target: super::super::RuntimeFieldTargetSeed::Inspect(
                            RuntimeMutablePlaceSeed::Local(locals[0].clone()),
                        ),
                        field: projection,
                    },
                ))
                .unwrap();
            assert!(matches!(expr.kind(), RuntimeExprKind::Field {
                field: RuntimeFieldProjection::EntityReference(actual), ..
            } if *actual == field));
        }
    }
}

fn entity_reference_field_fixture() -> (RuntimePlanBuilder, Vec<RuntimeLocalSeedId>) {
    let mut builder = RuntimePlanBuilder::new();
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(identity(1), RuntimePlanTypeProjection::EntityReference),
                RuntimePlanTypeSeed::new(identity(2), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(identity(3), RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    identity(4),
                    RuntimePlanTypeProjection::Reference(identity(1)),
                ),
            ],
            [
                ("entity", identity(1)),
                ("borrowed", identity(4)),
                ("boolean", identity(3)),
            ]
            .map(|(name, ty)| {
                RuntimeLocalDeclarationSeed::new(
                    manual_local_source(&format!("arcweft-core.fixture.reference_field.{name}")),
                    ty,
                )
            }),
        )
        .unwrap();
    (builder, admitted.local_ids().to_vec())
}

#[test]
fn entity_reference_fields_reject_borrowed_aliases_wrong_owners_and_nonstring_results() {
    use crate::value::RuntimeEntityReferenceField;
    let (builder, locals) = entity_reference_field_fixture();
    for projection in [
        RuntimeFieldProjectionSeed::Agent(RuntimeEntityReferenceField::Id.into()),
        RuntimeFieldProjectionSeed::EntityReference(RuntimeEntityReferenceField::Id),
    ] {
        let agent = matches!(projection, RuntimeFieldProjectionSeed::Agent(_));
        for (local, marker) in [(&locals[1], 4), (&locals[2], 3)] {
            let ty = builder
                .resolve_seed_type("wrong reference owner", identity(marker))
                .unwrap();
            assert_eq!(
                builder.lower_expression(RuntimeExprSeed::new(
                    identity(2),
                    RuntimeExprSeedKind::Field {
                        target: super::super::RuntimeFieldTargetSeed::Inspect(
                            RuntimeMutablePlaceSeed::Local(local.clone())
                        ),
                        field: projection.clone(),
                    }
                )),
                Err(RuntimePlanBuildError::InvalidTypeProjection {
                    context: if agent {
                        "Agent field owner"
                    } else {
                        "entity-reference field target"
                    },
                    ty,
                })
            );
        }
        let result = builder
            .resolve_seed_type("wrong reference result", identity(3))
            .unwrap();
        assert_eq!(
            builder.lower_expression(RuntimeExprSeed::new(
                identity(3),
                RuntimeExprSeedKind::Field {
                    target: super::super::RuntimeFieldTargetSeed::Inspect(
                        RuntimeMutablePlaceSeed::Local(locals[0].clone())
                    ),
                    field: projection,
                }
            )),
            Err(RuntimePlanBuildError::InvalidTypeProjection {
                context: if agent {
                    "Agent field result"
                } else {
                    "entity-reference field result"
                },
                ty: result,
            })
        );
    }
}
