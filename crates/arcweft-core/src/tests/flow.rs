//! Native-flow behavior exercised through the sole typed runtime-plan builder.

mod function_call;

use crate::scope::RuntimeScopeIdentity;
use crate::{
    engine::{Engine, FlowFiberStatus},
    entry::RuntimeNominalTypeId,
    pattern::RuntimeSemanticTypeId,
    plan::{
        FlowEvent, RuntimeAwaitPendingObserverSeed, RuntimeAwaitTargetSeed,
        RuntimeCallableAttachedContract, RuntimeCallableDefault, RuntimeCallableInputSource,
        RuntimeCallableParameterCoordinate, RuntimeCallableParameterInput,
        RuntimeCallableParameterKind, RuntimeCallablePosition, RuntimeCallableStateDefinition,
        RuntimeCallableStateSeedId, RuntimeCallableTransition, RuntimeEffectSet,
        RuntimeExecutableBodySeed, RuntimeExprSeed, RuntimeExprSeedKind,
        RuntimeFieldProjectionSeed, RuntimeFlowOpSeed, RuntimeFlowSchema, RuntimeFlowSeed,
        RuntimeFunctionInputBindingSeed, RuntimeFunctionInputSource, RuntimeFunctionSiteBodyKind,
        RuntimeFunctionSiteBodySeed, RuntimeFunctionSiteDeclarationSeed,
        RuntimeHostTaskRequestTemplateSeed, RuntimeLocalDeclarationSeed,
        RuntimeNominalRecordDomainFieldSeed, RuntimeNominalRecordDomainSeed,
        RuntimeNominalRecordFieldSeed, RuntimePatternSeed, RuntimePatternSeedKind,
        RuntimePlanBuilder, RuntimePlanSequenceKind, RuntimePlanTypeProjection,
        RuntimePlanTypeSeed, RuntimeProjectCallAttachedMaterializationSeed,
        RuntimeProjectCallAttachedPresenceSeed, RuntimeProjectCallOperandSeed,
        RuntimeProjectCallOrdinaryMaterializationSeed, RuntimeProjectCallPlanSeed,
        RuntimeProjectCallRestMaterializationSeed, RuntimeRecordFieldSeedId,
    },
    step::{RuntimeStepInput, RuntimeStepOptions},
    task::{
        HostCapabilityId, LogicalEpoch, NeedId, TaskEvent, TaskEventKind, TaskId,
        TaskOutcomeContract, TaskSequence,
    },
    value::{RuntimeBinaryOp, RuntimeUnsignedIntWidth, RuntimeValue},
};
use arcweft_id::DeclarationName;
use arcweft_need::Progress;

const STRING_TYPE_MARKER: u8 = 1;

fn string_type() -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([STRING_TYPE_MARKER; 32])
}

fn string_value(value: &str) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        string_type(),
        RuntimeExprSeedKind::Value(RuntimeValue::String(value.to_owned())),
    )
}

fn unit_type() -> RuntimeSemanticTypeId {
    crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest()
}

fn unit_value() -> RuntimeExprSeed {
    RuntimeExprSeed::new(unit_type(), RuntimeExprSeedKind::Value(RuntimeValue::Unit))
}

fn callable_type(marker: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([marker; 32])
}

fn define_invoke_callable(
    builder: &mut RuntimePlanBuilder,
    function_type: RuntimeSemanticTypeId,
    result: RuntimeSemanticTypeId,
    parameters: &[(
        RuntimeSemanticTypeId,
        RuntimeSemanticTypeId,
        RuntimeCallableParameterKind,
    )],
    attached: RuntimeCallableAttachedContract<
        RuntimeSemanticTypeId,
        crate::plan::RuntimeFunctionSiteSeedId,
    >,
    function: crate::plan::RuntimeFunctionSiteSeedId,
) -> (RuntimeCallableStateSeedId, RuntimeExprSeed) {
    let state = builder
        .reserve_callable_state_seed()
        .expect("callable state reserves");
    let arguments = (0..parameters.len())
        .map(|position| RuntimeCallableInputSource::Argument {
            position: u32::try_from(position).expect("test parameter ordinal"),
        })
        .chain(
            (!matches!(&attached, RuntimeCallableAttachedContract::None))
                .then_some(RuntimeCallableInputSource::Attached),
        )
        .collect();
    builder
        .define_callable_state_seed(
            &state,
            RuntimeCallableStateDefinition {
                function_type,
                origin: state.clone(),
                position: RuntimeCallablePosition::Unapplied,
                retained: Box::new([]),
                parameters: parameters
                    .iter()
                    .enumerate()
                    .map(
                        |(parameter, (abi_ty, binding_ty, kind))| RuntimeCallableParameterInput {
                            coordinate: RuntimeCallableParameterCoordinate {
                                group: 0,
                                parameter: u32::try_from(parameter).expect("test formal ordinal"),
                            },
                            kind: *kind,
                            abi_ty: *abi_ty,
                            binding_ty: *binding_ty,
                        },
                    )
                    .collect(),
                result,
                attached,
                transition: RuntimeCallableTransition::Invoke {
                    function,
                    captures: Box::new([]),
                    arguments,
                },
                partials: Box::new([]),
            },
        )
        .expect("callable state defines");
    let callee = RuntimeExprSeed::new(
        function_type,
        RuntimeExprSeedKind::MakeCallable {
            state: state.clone(),
            captures: Box::new([]),
        },
    );
    (state, callee)
}

fn flow_id(value: &str) -> crate::plan::FlowRuntimeId {
    crate::plan::FlowRuntimeId::from_runtime_target_value(value).expect("valid test flow id")
}

fn flow_schema(flow: &crate::plan::FlowRuntimeId) -> RuntimeFlowSchema {
    RuntimeFlowSchema {
        flow: flow.clone(),
        parameters: Vec::new(),
    }
}

fn finish_plan(flows: impl IntoIterator<Item = RuntimeFlowSeed>) -> crate::plan::RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                string_type(),
                RuntimePlanTypeProjection::String,
            )],
            [],
        )
        .expect("typed scalar admission");
    for flow in flows {
        builder
            .push_flow_schema(flow_schema(flow.id()))
            .expect("typed flow schema admission");
        builder.push_flow_seed(flow).expect("typed flow admission");
    }
    builder.finish().expect("valid typed runtime plan")
}

fn step(engine: &mut Engine) -> crate::step::RuntimeStepOutput {
    engine
        .step(RuntimeStepInput::default(), RuntimeStepOptions::default())
        .output
}

fn drain(engine: &mut Engine) -> crate::step::RuntimeStepOutput {
    let mut output = crate::step::RuntimeStepOutput::default();
    for _ in 0..8 {
        let next = step(engine);
        output.flow_events.extend(next.flow_events);
        output.diagnostics.extend(next.diagnostics);
        if matches!(
            engine.fiber().status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        ) {
            break;
        }
    }
    output
}

#[test]
fn native_flow_returns_a_typed_scalar_value() {
    let entry = flow_id("flow.return");
    let plan = finish_plan([RuntimeFlowSeed::new(
        entry.clone(),
        [],
        crate::plan::RuntimeEffectSet::empty(),
        vec![RuntimeFlowOpSeed::ReturnExpr(string_value("ready"))],
    )]);
    let mut engine = Engine::for_flow(plan, &entry).expect("entry flow exists");

    let output = drain(&mut engine);

    assert_eq!(
        output.flow_events,
        vec![FlowEvent::Return {
            value: "ready".to_owned(),
        }]
    );
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Done(_)));
}

#[test]
fn named_flow_scope_binds_its_result_in_the_parent_and_keeps_aot_identity() {
    let string = string_type();
    let entry = flow_id("flow.named_scope");
    let identity =
        RuntimeScopeIdentity::Named(DeclarationName::try_new("window").expect("valid scope name"));
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                string,
                RuntimePlanTypeProjection::String,
            )],
            [RuntimeLocalDeclarationSeed::new(string)],
        )
        .expect("named scope result local admits");
    let result = admission.local_ids()[0].clone();
    builder
        .push_flow_schema(flow_schema(&entry))
        .expect("flow schema admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            entry.clone(),
            [],
            RuntimeEffectSet::empty(),
            vec![
                RuntimeFlowOpSeed::EnterScope {
                    identity: identity.clone(),
                },
                RuntimeFlowOpSeed::ExitScopeBind {
                    pattern: RuntimePatternSeed::new(
                        string,
                        RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local: result.clone(),
                        },
                    ),
                    expr: string_value("scope result"),
                },
                RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                    string,
                    RuntimeExprSeedKind::Local(result),
                )),
            ],
        ))
        .expect("named flow body admits");
    let plan = builder.finish().expect("named flow plan seals");

    let aot = crate::aot::AotProgram::from_runtime_plan(&plan);
    assert!(matches!(
        aot.flow_block(0).and_then(|block| block.linear_op(0)),
        Some(crate::aot::AotLinearOp::EnterScope { identity: projected })
            if projected == &identity
    ));
    let mut engine = Engine::for_flow(plan, &entry).expect("named scope flow starts");

    let output = drain(&mut engine);

    assert_eq!(
        output.flow_events,
        vec![FlowEvent::Return {
            value: "scope result".to_owned(),
        }]
    );
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Done(_)));
}

#[test]
fn native_flow_evaluates_a_named_expression_scope_before_its_outer_return() {
    let entry = flow_id("flow.named_expression_scope");
    let plan = finish_plan([RuntimeFlowSeed::new(
        entry.clone(),
        [],
        RuntimeEffectSet::empty(),
        vec![RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
            string_type(),
            RuntimeExprSeedKind::Scope {
                identity: RuntimeScopeIdentity::Named(
                    DeclarationName::try_new("window").expect("valid scope name"),
                ),
                body: Box::new(string_value("expression scope")),
            },
        ))],
    )]);
    let mut engine = Engine::for_flow(plan, &entry).expect("named expression scope starts");

    let output = drain(&mut engine);

    assert_eq!(
        output.flow_events,
        vec![FlowEvent::Return {
            value: "expression scope".to_owned(),
        }]
    );
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Done(_)));
}

#[test]
fn native_goto_selects_the_targeted_typed_flow() {
    let opening = flow_id("flow.opening");
    let ending = flow_id("flow.ending");
    let plan = finish_plan([
        RuntimeFlowSeed::new(
            opening.clone(),
            [],
            crate::plan::RuntimeEffectSet::empty(),
            vec![RuntimeFlowOpSeed::Goto(ending.clone())],
        ),
        RuntimeFlowSeed::new(
            ending.clone(),
            [],
            crate::plan::RuntimeEffectSet::empty(),
            vec![RuntimeFlowOpSeed::ReturnExpr(string_value("finished"))],
        ),
    ]);
    let mut engine = Engine::for_flow(plan, &opening).expect("opening flow exists");

    let output = drain(&mut engine);

    assert_eq!(
        output.flow_events,
        vec![
            FlowEvent::Goto { target: ending },
            FlowEvent::Return {
                value: "finished".to_owned(),
            },
        ]
    );
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Done(_)));
}

#[test]
fn native_project_call_direct_continue_publishes_one_catalog_site() {
    let unit = unit_type();
    let function = callable_type(0x77);
    let outer = callable_type(0x76);
    let entry = flow_id("flow.project_call_continue");
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(
                    function,
                    RuntimePlanTypeProjection::Function {
                        parameters: Box::new([]),
                        result: unit,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    outer,
                    RuntimePlanTypeProjection::Function {
                        parameters: Box::new([]),
                        result: function,
                    },
                ),
            ],
            [],
        )
        .expect("project-call continuation types admit");
    let target_site = builder
        .push_function_site_seed([], unit_value())
        .expect("continued callable target admits");
    let initial = builder
        .reserve_callable_state_seed()
        .expect("initial state");
    let continuation = builder
        .reserve_callable_state_seed()
        .expect("continuation state");
    builder
        .define_callable_state_seed(
            &initial,
            RuntimeCallableStateDefinition {
                function_type: outer,
                origin: initial.clone(),
                position: RuntimeCallablePosition::Unapplied,
                retained: Box::new([]),
                parameters: Box::new([]),
                result: function,
                attached: RuntimeCallableAttachedContract::None,
                transition: RuntimeCallableTransition::Retain {
                    state: continuation.clone(),
                    values: Box::new([]),
                },
                partials: Box::new([]),
            },
        )
        .expect("initial state defines");
    builder
        .define_callable_state_seed(
            &continuation,
            RuntimeCallableStateDefinition {
                function_type: function,
                origin: initial.clone(),
                position: RuntimeCallablePosition::AfterGroup { completed: 0 },
                retained: Box::new([]),
                parameters: Box::new([]),
                result: unit,
                attached: RuntimeCallableAttachedContract::None,
                transition: RuntimeCallableTransition::Invoke {
                    function: target_site,
                    captures: Box::new([]),
                    arguments: Box::new([]),
                },
                partials: Box::new([]),
            },
        )
        .expect("continuation state defines");
    builder
        .push_flow_schema(flow_schema(&entry))
        .expect("project-call flow schema admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            entry.clone(),
            [],
            crate::plan::RuntimeEffectSet::empty(),
            vec![
                RuntimeFlowOpSeed::ProjectCall {
                    plan: RuntimeProjectCallPlanSeed {
                        callee: RuntimeExprSeed::new(
                            outer,
                            RuntimeExprSeedKind::MakeCallable {
                                state: initial.clone(),
                                captures: Box::new([]),
                            },
                        ),
                        state: initial,
                        completed_group: 0,
                        operands: Box::new([]),
                        ordinary: Box::new([]),
                        attached: None,
                    },
                    result: RuntimePatternSeed::new(function, RuntimePatternSeedKind::Discard),
                },
                RuntimeFlowOpSeed::Return("continued".to_owned()),
            ],
        ))
        .expect("direct project-call flow admits");
    let plan = builder.finish().expect("direct project-call plan seals");
    assert_eq!(plan.project_call_sites().len(), 1);
    let mut engine = Engine::for_flow(plan, &entry).expect("project-call flow exists");

    let output = drain(&mut engine);

    assert_eq!(
        output.flow_events,
        vec![FlowEvent::Return {
            value: "continued".to_owned(),
        }]
    );
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Done(_)));
}

#[test]
fn native_project_call_defaulted_omitted_rejoins_target_through_catalog_site() {
    let unit = unit_type();
    let function = callable_type(0x79);
    let tuple =
        crate::pattern::RuntimeCheckedType::Tuple(vec![crate::pattern::RuntimeCheckedType::Unit])
            .semantic_identity_digest();
    let option = crate::pattern::RuntimeCheckedType::Option(Box::new(
        crate::pattern::RuntimeCheckedType::Unit,
    ))
    .semantic_identity_digest();
    let entry = flow_id("flow.project_call_default");
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(tuple, RuntimePlanTypeProjection::Tuple(Box::new([unit]))),
                RuntimePlanTypeSeed::new(
                    option,
                    RuntimePlanTypeProjection::Option {
                        item: unit,
                        some_payload: tuple,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    function,
                    RuntimePlanTypeProjection::Function {
                        parameters: Box::new([option]),
                        result: unit,
                    },
                ),
            ],
            [RuntimeLocalDeclarationSeed::new(unit)],
        )
        .expect("project-call attached types admit");
    let target_input = admission
        .local_ids()
        .first()
        .cloned()
        .expect("target parameter local admits");
    let default_site = builder
        .push_function_site_seed([], unit_value())
        .expect("default function-site admits");
    let target_site = builder
        .push_function_site_seed(
            [RuntimeFunctionInputBindingSeed {
                source: RuntimeFunctionInputSource::Parameter { position: 0 },
                input_local: target_input,
                pattern: RuntimePatternSeed::new(unit, RuntimePatternSeedKind::Discard),
            }],
            unit_value(),
        )
        .expect("target function-site admits");
    let (state, callee) = define_invoke_callable(
        &mut builder,
        function,
        unit,
        &[],
        RuntimeCallableAttachedContract::Defaulted {
            ty: unit,
            default: RuntimeCallableDefault {
                function: default_site,
                captures: Box::new([]),
            },
        },
        target_site,
    );
    builder
        .push_flow_schema(flow_schema(&entry))
        .expect("defaulted project-call flow schema admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            entry.clone(),
            [],
            crate::plan::RuntimeEffectSet::empty(),
            vec![
                RuntimeFlowOpSeed::ProjectCall {
                    plan: RuntimeProjectCallPlanSeed {
                        callee,
                        state,
                        completed_group: 0,
                        operands: Box::new([]),
                        ordinary: Box::new([]),
                        attached: Some(RuntimeProjectCallAttachedMaterializationSeed {
                            abi_ty: option,
                            binding_ty: unit,
                            source_index: None,
                            presence: RuntimeProjectCallAttachedPresenceSeed::DefaultedOmitted,
                        }),
                    },
                    result: RuntimePatternSeed::new(unit, RuntimePatternSeedKind::Discard),
                },
                RuntimeFlowOpSeed::Return("defaulted".to_owned()),
            ],
        ))
        .expect("defaulted project-call flow admits");
    let plan = builder.finish().expect("defaulted project-call plan seals");
    assert_eq!(plan.project_call_sites().len(), 1);
    let mut engine = Engine::for_flow(plan, &entry).expect("defaulted project-call flow exists");

    let output = drain(&mut engine);

    assert_eq!(
        output.flow_events,
        vec![FlowEvent::Return {
            value: "defaulted".to_owned(),
        }]
    );
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Done(_)));
}

#[test]
fn native_project_call_rest_materialization_accepts_empty_and_source_ordered_values() {
    let unit = unit_type();
    let function = callable_type(0x7a);
    let sequence = crate::pattern::RuntimeCheckedType::Sequence(Box::new(
        crate::pattern::RuntimeCheckedType::Unit,
    ))
    .semantic_identity_digest();

    for source_count in [0_usize, 2] {
        let entry = flow_id(if source_count == 0 {
            "flow.project_call_rest_empty"
        } else {
            "flow.project_call_rest_values"
        });
        let mut builder = RuntimePlanBuilder::new();
        let admission = builder
            .admit_type_batch(
                [
                    RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                    RuntimePlanTypeSeed::new(
                        sequence,
                        RuntimePlanTypeProjection::Sequence {
                            kind: RuntimePlanSequenceKind::Vec,
                            item: unit,
                        },
                    ),
                    RuntimePlanTypeSeed::new(
                        function,
                        RuntimePlanTypeProjection::Function {
                            parameters: Box::new([unit]),
                            result: unit,
                        },
                    ),
                ],
                [RuntimeLocalDeclarationSeed::new(sequence)],
            )
            .expect("rest project-call types admit");
        let target_input = admission
            .local_ids()
            .first()
            .cloned()
            .expect("rest target parameter local admits");
        let target_site = builder
            .push_function_site_seed(
                [RuntimeFunctionInputBindingSeed {
                    source: RuntimeFunctionInputSource::Parameter { position: 0 },
                    input_local: target_input,
                    pattern: RuntimePatternSeed::new(sequence, RuntimePatternSeedKind::Discard),
                }],
                unit_value(),
            )
            .expect("rest target function-site admits");
        let (callable_state, callee) = define_invoke_callable(
            &mut builder,
            function,
            unit,
            &[(unit, sequence, RuntimeCallableParameterKind::Rest)],
            RuntimeCallableAttachedContract::None,
            target_site,
        );
        builder
            .push_flow_schema(flow_schema(&entry))
            .expect("rest flow schema admits");
        let operands = (0..source_count)
            .map(|position| RuntimeProjectCallOperandSeed {
                value: unit_value(),
                mode: crate::value::RuntimeCallArgumentMode::Value,
                abi_position: u32::try_from(position).expect("test source count fits u32"),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let source_indices = (0..source_count)
            .map(|position| u32::try_from(position).expect("test source count fits u32"))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        builder
            .push_flow_seed(RuntimeFlowSeed::new(
                entry.clone(),
                [],
                crate::plan::RuntimeEffectSet::empty(),
                vec![
                    RuntimeFlowOpSeed::ProjectCall {
                        plan: RuntimeProjectCallPlanSeed {
                            callee,
                            state: callable_state,
                            completed_group: 0,
                            operands,
                            ordinary: Box::new([
                                RuntimeProjectCallOrdinaryMaterializationSeed::Rest(
                                    RuntimeProjectCallRestMaterializationSeed {
                                        parameter: 0,
                                        abi_ty: unit,
                                        binding_ty: sequence,
                                        source_indices,
                                    },
                                ),
                            ]),
                            attached: None,
                        },
                        result: RuntimePatternSeed::new(unit, RuntimePatternSeedKind::Discard),
                    },
                    RuntimeFlowOpSeed::Return("rest".to_owned()),
                ],
            ))
            .expect("rest project-call flow admits");
        let plan = builder.finish().expect("rest project-call plan seals");
        assert_eq!(plan.project_call_sites().len(), 1);
        let mut engine = Engine::for_flow(plan, &entry).expect("rest flow exists");
        let output = drain(&mut engine);
        assert_eq!(
            output.flow_events,
            vec![FlowEvent::Return {
                value: "rest".to_owned(),
            }]
        );
        assert!(matches!(engine.fiber().status, FlowFiberStatus::Done(_)));
    }
}

#[test]
fn native_project_call_evaluates_rest_operands_once_in_source_order() {
    use crate::entry::{
        RuntimeNominalSchemaBody, RuntimeNominalSchemaDefinition, RuntimeNominalSchemaField,
        RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity, RuntimeSchemaLimits,
        RuntimeTypeSchema,
    };
    let unit = unit_type();
    let u32_ty = crate::pattern::RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U32)
        .semantic_identity_digest();
    let sequence = crate::pattern::RuntimeCheckedType::Sequence(Box::new(
        crate::pattern::RuntimeCheckedType::Unsigned(RuntimeUnsignedIntWidth::U32),
    ))
    .semantic_identity_digest();
    let function = callable_type(0x7b);
    let state_ty = RuntimeSemanticTypeId::from_bytes([0x91; 32]);
    let state_nominal =
        RuntimeNominalTypeId::try_new("test.ProjectCallState").expect("state nominal identity");
    let schema = RuntimeNominalSchemaGraph::try_new(
        vec![RuntimeNominalSchemaDefinition::new(
            RuntimeNominalSchemaIdentity::new(state_nominal.clone(), state_ty),
            vec![],
            RuntimeNominalSchemaBody::Record {
                shape: crate::entry::RuntimeNominalRecordShape::Record,
                fields: vec![RuntimeNominalSchemaField::new(
                    crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                    Some("value".to_owned()),
                    RuntimeTypeSchema::U32,
                )]
                .into_boxed_slice(),
            },
        )],
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap();
    let state_layout = schema.try_layout_hash(state_ty).unwrap();
    let field = RuntimeRecordFieldSeedId::from_zero_based(0);
    let entry = flow_id("flow.project_call_source_once");
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(
                    u32_ty,
                    RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U32),
                ),
                RuntimePlanTypeSeed::new(
                    sequence,
                    RuntimePlanTypeProjection::Sequence {
                        kind: RuntimePlanSequenceKind::Vec,
                        item: u32_ty,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    function,
                    RuntimePlanTypeProjection::Function {
                        parameters: Box::new([u32_ty]),
                        result: unit,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    state_ty,
                    RuntimePlanTypeProjection::Nominal {
                        nominal: state_nominal,
                        layout: state_layout,
                        arguments: Box::new([]),
                    },
                ),
            ],
            [
                RuntimeLocalDeclarationSeed::new(state_ty),
                RuntimeLocalDeclarationSeed::new(sequence),
            ],
            [RuntimeNominalRecordDomainSeed::new(
                state_ty,
                crate::entry::RuntimeNominalRecordShape::Record,
                [RuntimeNominalRecordDomainFieldSeed::new(
                    crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                    Some("value".to_owned()),
                    u32_ty,
                )],
            )],
            [],
            &schema,
        )
        .expect("source-once types and state domain admit");
    let state = admission.local_ids()[0].clone();
    let target_input = admission.local_ids()[1].clone();
    let target_site = builder
        .push_function_site_seed(
            [RuntimeFunctionInputBindingSeed {
                source: RuntimeFunctionInputSource::Parameter { position: 0 },
                input_local: target_input,
                pattern: RuntimePatternSeed::new(sequence, RuntimePatternSeedKind::Discard),
            }],
            unit_value(),
        )
        .expect("source-once target function-site admits");
    let (callable_state, callee) = define_invoke_callable(
        &mut builder,
        function,
        unit,
        &[(u32_ty, sequence, RuntimeCallableParameterKind::Rest)],
        RuntimeCallableAttachedContract::None,
        target_site,
    );

    let local_state = |local| RuntimeExprSeed::new(state_ty, RuntimeExprSeedKind::Local(local));
    let state_value = |local| {
        RuntimeExprSeed::new(
            u32_ty,
            RuntimeExprSeedKind::Field {
                target: Box::new(local_state(local)),
                field: RuntimeFieldProjectionSeed::Nominal {
                    owner: state_ty,
                    field,
                },
            },
        )
    };
    let bump_state = |local: crate::plan::RuntimeLocalSeedId, amount: u32| {
        RuntimeExprSeed::new(
            u32_ty,
            RuntimeExprSeedKind::AssignNominalField {
                base: local.clone(),
                owner: state_ty,
                field,
                expr: Box::new(RuntimeExprSeed::new(
                    u32_ty,
                    RuntimeExprSeedKind::Binary {
                        lhs: Box::new(RuntimeExprSeed::new(
                            u32_ty,
                            RuntimeExprSeedKind::Binary {
                                lhs: Box::new(state_value(local.clone())),
                                op: RuntimeBinaryOp::Mul,
                                rhs: Box::new(RuntimeExprSeed::new(
                                    u32_ty,
                                    RuntimeExprSeedKind::Value(RuntimeValue::u32(10)),
                                )),
                            },
                        )),
                        op: RuntimeBinaryOp::Add,
                        rhs: Box::new(RuntimeExprSeed::new(
                            u32_ty,
                            RuntimeExprSeedKind::Value(RuntimeValue::u32(amount)),
                        )),
                    },
                )),
                body: Box::new(state_value(local)),
            },
        )
    };
    let initial_state = RuntimeExprSeed::new(
        state_ty,
        RuntimeExprSeedKind::NominalRecord(
            [RuntimeNominalRecordFieldSeed::new(
                field,
                RuntimeExprSeed::new(u32_ty, RuntimeExprSeedKind::Value(RuntimeValue::u32(0))),
            )]
            .into(),
        ),
    );
    builder
        .push_flow_schema(flow_schema(&entry))
        .expect("source-once flow schema admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            entry.clone(),
            [],
            crate::plan::RuntimeEffectSet::empty(),
            vec![
                RuntimeFlowOpSeed::Let {
                    pattern: RuntimePatternSeed::new(
                        state_ty,
                        RuntimePatternSeedKind::Bind {
                            mutable: true,
                            local: state.clone(),
                        },
                    ),
                    expr: initial_state,
                },
                RuntimeFlowOpSeed::ProjectCall {
                    plan: RuntimeProjectCallPlanSeed {
                        callee,
                        state: callable_state,
                        completed_group: 0,
                        operands: Box::new([
                            RuntimeProjectCallOperandSeed {
                                value: bump_state(state.clone(), 1),
                                mode: crate::value::RuntimeCallArgumentMode::Value,
                                abi_position: 0,
                            },
                            RuntimeProjectCallOperandSeed {
                                value: bump_state(state.clone(), 2),
                                mode: crate::value::RuntimeCallArgumentMode::Value,
                                abi_position: 1,
                            },
                        ]),
                        ordinary: Box::new([RuntimeProjectCallOrdinaryMaterializationSeed::Rest(
                            RuntimeProjectCallRestMaterializationSeed {
                                parameter: 0,
                                abi_ty: u32_ty,
                                binding_ty: sequence,
                                source_indices: Box::new([0, 1]),
                            },
                        )]),
                        attached: None,
                    },
                    result: RuntimePatternSeed::new(unit, RuntimePatternSeedKind::Discard),
                },
                RuntimeFlowOpSeed::ReturnExpr(state_value(state)),
            ],
        ))
        .expect("source-once project-call flow admits");
    let plan = builder.finish().expect("source-once plan seals");
    let mut engine = Engine::for_flow(plan, &entry).expect("source-once flow exists");

    let output = drain(&mut engine);

    assert_eq!(
        output.flow_events,
        vec![FlowEvent::Return {
            value: "12".to_owned(),
        }],
    );
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Done(_)));
}

#[test]
fn native_project_call_executable_target_explicit_return_rejoins_catalog_site() {
    let string = string_type();
    let function = callable_type(0x7c);
    let entry = flow_id("flow.project_call_explicit_return");
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(string, RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(
                    function,
                    RuntimePlanTypeProjection::Function {
                        parameters: Box::new([]),
                        result: string,
                    },
                ),
            ],
            [],
        )
        .expect("project-call return type admits");
    let target_site = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            inputs: Box::new([]),
            result: string,
            body_kind: RuntimeFunctionSiteBodyKind::Executable,
            effects: RuntimeEffectSet::empty(),
        })
        .expect("executable target site reserves");
    builder
        .define_function_site_seed(
            &target_site,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([RuntimeFlowOpSeed::Return("target".to_owned())]),
            }),
        )
        .expect("executable target site defines");
    let (state, callee) = define_invoke_callable(
        &mut builder,
        function,
        string,
        &[],
        RuntimeCallableAttachedContract::None,
        target_site,
    );
    builder
        .push_flow_schema(flow_schema(&entry))
        .expect("explicit-return flow schema admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            entry.clone(),
            [],
            crate::plan::RuntimeEffectSet::empty(),
            vec![
                RuntimeFlowOpSeed::ProjectCall {
                    plan: RuntimeProjectCallPlanSeed {
                        callee,
                        state,
                        completed_group: 0,
                        operands: Box::new([]),
                        ordinary: Box::new([]),
                        attached: None,
                    },
                    result: RuntimePatternSeed::new(string, RuntimePatternSeedKind::Discard),
                },
                RuntimeFlowOpSeed::Return("done".to_owned()),
            ],
        ))
        .expect("explicit-return project-call flow admits");
    let plan = builder.finish().expect("explicit-return plan seals");
    let mut engine = Engine::for_flow(plan, &entry).expect("explicit-return flow exists");

    let output = drain(&mut engine);

    assert_eq!(
        output.flow_events,
        vec![FlowEvent::Return {
            value: "done".to_owned(),
        }]
    );
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Done(_)));
}

#[test]
fn native_project_call_target_goto_unwinds_the_catalog_return_boundary() {
    let unit = unit_type();
    let function = callable_type(0x7d);
    let entry = flow_id("flow.project_call_goto");
    let target = flow_id("flow.project_call_goto_target");
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(
                    function,
                    RuntimePlanTypeProjection::Function {
                        parameters: Box::new([]),
                        result: unit,
                    },
                ),
            ],
            [],
        )
        .expect("goto project-call type admits");
    let target_site = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            inputs: Box::new([]),
            result: unit,
            body_kind: RuntimeFunctionSiteBodyKind::Executable,
            effects: RuntimeEffectSet::empty(),
        })
        .expect("goto target site reserves");
    builder
        .define_function_site_seed(
            &target_site,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([RuntimeFlowOpSeed::Goto(target.clone())]),
            }),
        )
        .expect("goto target site defines");
    let (state, callee) = define_invoke_callable(
        &mut builder,
        function,
        unit,
        &[],
        RuntimeCallableAttachedContract::None,
        target_site,
    );
    builder
        .push_flow_schema(flow_schema(&entry))
        .expect("goto entry schema admits");
    builder
        .push_flow_schema(flow_schema(&target))
        .expect("goto target schema admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            entry.clone(),
            [],
            crate::plan::RuntimeEffectSet::empty(),
            vec![RuntimeFlowOpSeed::ProjectCall {
                plan: RuntimeProjectCallPlanSeed {
                    callee,
                    state,
                    completed_group: 0,
                    operands: Box::new([]),
                    ordinary: Box::new([]),
                    attached: None,
                },
                result: RuntimePatternSeed::new(unit, RuntimePatternSeedKind::Discard),
            }],
        ))
        .expect("goto project-call entry admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            target.clone(),
            [],
            crate::plan::RuntimeEffectSet::empty(),
            vec![RuntimeFlowOpSeed::Return("goto-done".to_owned())],
        ))
        .expect("goto target flow admits");
    let mut engine = Engine::for_flow(
        builder.finish().expect("goto project-call plan seals"),
        &entry,
    )
    .expect("goto project-call flow exists");

    let output = drain(&mut engine);

    assert_eq!(
        output.flow_events,
        vec![
            FlowEvent::Goto {
                target: target.clone(),
            },
            FlowEvent::Return {
                value: "goto-done".to_owned(),
            },
        ]
    );
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Done(_)));
}

#[test]
fn native_project_call_executable_target_fallthrough_fails_closed() {
    let unit = unit_type();
    let function = callable_type(0x7e);
    let entry = flow_id("flow.project_call_fallthrough");
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(
                    function,
                    RuntimePlanTypeProjection::Function {
                        parameters: Box::new([]),
                        result: unit,
                    },
                ),
            ],
            [],
        )
        .expect("fallthrough type admits");
    let target_site = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            inputs: Box::new([]),
            result: unit,
            body_kind: RuntimeFunctionSiteBodyKind::Executable,
            effects: RuntimeEffectSet::empty(),
        })
        .expect("fallthrough target site reserves");
    builder
        .define_function_site_seed(
            &target_site,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([]),
            }),
        )
        .expect("fallthrough target site defines");
    let (state, callee) = define_invoke_callable(
        &mut builder,
        function,
        unit,
        &[],
        RuntimeCallableAttachedContract::None,
        target_site,
    );
    builder
        .push_flow_schema(flow_schema(&entry))
        .expect("fallthrough flow schema admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            entry.clone(),
            [],
            crate::plan::RuntimeEffectSet::empty(),
            vec![RuntimeFlowOpSeed::ProjectCall {
                plan: RuntimeProjectCallPlanSeed {
                    callee,
                    state,
                    completed_group: 0,
                    operands: Box::new([]),
                    ordinary: Box::new([]),
                    attached: None,
                },
                result: RuntimePatternSeed::new(unit, RuntimePatternSeedKind::Discard),
            }],
        ))
        .expect("fallthrough project-call flow admits");
    let mut engine = Engine::for_flow(builder.finish().expect("fallthrough plan seals"), &entry)
        .expect("fallthrough flow exists");

    let output = drain(&mut engine);

    assert!(matches!(engine.fiber().status, FlowFiberStatus::Failed(_)));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("exhausted without a typed return")
    }));
}

#[test]
fn native_if_uses_the_admitted_bool_condition() {
    let bool_type = RuntimeSemanticTypeId::from_bytes([2; 32]);
    let entry = flow_id("flow.branch");
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(string_type(), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(bool_type, RuntimePlanTypeProjection::Bool),
            ],
            [],
        )
        .expect("typed scalar admission");
    builder
        .push_flow_schema(flow_schema(&entry))
        .expect("typed branch flow schema admission");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            entry.clone(),
            [],
            crate::plan::RuntimeEffectSet::empty(),
            vec![RuntimeFlowOpSeed::If {
                condition: RuntimeExprSeed::new(
                    bool_type,
                    RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
                ),
                then_ops: vec![RuntimeFlowOpSeed::ReturnExpr(string_value("then"))],
                else_ops: vec![RuntimeFlowOpSeed::ReturnExpr(string_value("else"))],
            }],
        ))
        .expect("typed if flow admission");
    let plan = builder.finish().expect("valid typed branch plan");
    let mut engine = Engine::for_flow(plan, &entry).expect("branch flow exists");

    let output = drain(&mut engine);

    assert_eq!(
        output.flow_events,
        vec![FlowEvent::Return {
            value: "then".to_owned(),
        }]
    );
}

#[test]
fn await_progress_runs_only_the_first_matching_observer() {
    let progress_type = RuntimeSemanticTypeId::from_bytes([3; 32]);
    let entry = flow_id("flow.await_observer");
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(string_type(), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(progress_type, RuntimePlanTypeProjection::Progress),
            ],
            [],
        )
        .expect("Await observer types admit");
    builder
        .push_flow_schema(flow_schema(&entry))
        .expect("Await observer flow schema admission");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            entry.clone(),
            [],
            crate::plan::RuntimeEffectSet::empty(),
            vec![RuntimeFlowOpSeed::Await {
                binding: None,
                target: RuntimeAwaitTargetSeed {
                    need: NeedId("need.observe".to_owned()),
                    task: TaskId("task.observe".to_owned()),
                    outcome: TaskOutcomeContract::new(crate::pattern::RuntimeCheckedType::String),
                    request: RuntimeHostTaskRequestTemplateSeed {
                        capability: HostCapabilityId("test".to_owned()),
                        operation: "observe".to_owned(),
                        args: Vec::new(),
                    },
                },
                observers: vec![
                    RuntimeAwaitPendingObserverSeed {
                        pattern: RuntimePatternSeed::new(
                            progress_type,
                            RuntimePatternSeedKind::Discard,
                        ),
                        ops: vec![RuntimeFlowOpSeed::Return("first".to_owned())],
                    },
                    RuntimeAwaitPendingObserverSeed {
                        pattern: RuntimePatternSeed::new(
                            progress_type,
                            RuntimePatternSeedKind::Discard,
                        ),
                        ops: vec![RuntimeFlowOpSeed::Return("second".to_owned())],
                    },
                ],
            }],
        ))
        .expect("Await observer flow admits");
    let plan = builder.finish().expect("valid Await observer plan");
    let mut engine = Engine::for_flow(plan, &entry).expect("Await observer flow exists");
    let _started = step(&mut engine);

    let result = engine.step(
        RuntimeStepInput {
            task_events: vec![TaskEvent {
                logical_epoch: LogicalEpoch(1),
                task_id: TaskId("task.observe".to_owned()),
                sequence: TaskSequence(1),
                kind: TaskEventKind::Progress(
                    Progress::new(0.5).expect("fixture Progress is valid"),
                ),
            }],
            ..RuntimeStepInput::default()
        },
        RuntimeStepOptions::default(),
    );

    assert_eq!(
        result
            .output
            .flow_events
            .iter()
            .filter(|event| matches!(event, FlowEvent::AwaitProgress { .. }))
            .count(),
        1
    );
    let output = drain(&mut engine);
    assert_eq!(
        output.flow_events,
        vec![FlowEvent::Return {
            value: "first".to_owned(),
        }]
    );
}
