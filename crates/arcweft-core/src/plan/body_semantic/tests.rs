use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeLocalDeclarationSeed, RuntimeLocalOrigin, RuntimePlanBuilder, RuntimePlanTypeProjection,
    RuntimePlanTypeSeed,
};
use crate::task::semantic::TaskSemanticMeter;
use crate::value::RuntimeDisplacedField;
use std::num::NonZeroU32;

fn plan(padding: bool) -> (RuntimePlan, RuntimeLocalDeclarationId) {
    let mut builder = RuntimePlanBuilder::new();
    let semantic = RuntimeSemanticTypeId::from_bytes([11; 32]);
    let mut locals = Vec::new();
    if padding {
        locals.push(RuntimeLocalDeclarationSeed::new(
            RuntimeLocalOrigin::Binding([9; 32]),
            semantic,
        ));
    }
    locals.push(RuntimeLocalDeclarationSeed::new(
        RuntimeLocalOrigin::Binding([12; 32]),
        semantic,
    ));
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                semantic,
                RuntimePlanTypeProjection::Bool,
            )],
            locals,
        )
        .unwrap();
    (
        builder.finish().unwrap(),
        RuntimeLocalDeclarationId::from_accepted_ordinal(
            NonZeroU32::new(if padding { 2 } else { 1 }).unwrap(),
        ),
    )
}

fn assignment_digest(plan: &RuntimePlan, assignment: &RuntimeAssignment) -> blake3::Hash {
    let mut meter = TaskSemanticMeter::new(100, 1000);
    let mut encoder = TaskSemanticEncoder::new(b"body-test.v1\0", &mut meter);
    RuntimeBodySemanticContext::new(plan)
        .write_assignment(&mut encoder, assignment)
        .unwrap();
    encoder.finish().unwrap()
}

#[test]
fn local_arena_padding_does_not_change_the_resolved_body_place_transcript() {
    let (first, a) = plan(false);
    let (padded, b) = plan(true);
    assert_ne!(a, b);
    let write = |local| {
        RuntimeAssignment::new(
            RuntimeMutablePlace::Local(local),
            RuntimePlaceDisplacement::Reachable {
                initialization: RuntimePlaceInitialization::Initialized,
                fields: Box::new([]),
            },
        )
    };
    assert_eq!(
        assignment_digest(&first, &write(a)),
        assignment_digest(&padded, &write(b))
    );
}

#[test]
fn assignment_cleanup_contour_and_field_source_order_change_body_transcript() {
    let (plan, local) = plan(false);
    let field = |ordinal| RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap();
    let write = |state, reverse| {
        RuntimeAssignment::new(
            RuntimeMutablePlace::Local(local),
            RuntimePlaceDisplacement::Reachable {
                initialization: state,
                fields: if reverse {
                    Box::new([
                        RuntimeDisplacedField {
                            fields: Box::new([field(1)]),
                            initialization: RuntimePlaceInitialization::Uninitialized,
                        },
                        RuntimeDisplacedField {
                            fields: Box::new([field(0)]),
                            initialization: RuntimePlaceInitialization::Initialized,
                        },
                    ])
                } else {
                    Box::new([
                        RuntimeDisplacedField {
                            fields: Box::new([field(0)]),
                            initialization: RuntimePlaceInitialization::Initialized,
                        },
                        RuntimeDisplacedField {
                            fields: Box::new([field(1)]),
                            initialization: RuntimePlaceInitialization::Uninitialized,
                        },
                    ])
                },
            },
        )
    };
    let initial = assignment_digest(
        &plan,
        &write(RuntimePlaceInitialization::Initialized, false),
    );
    assert_ne!(
        initial,
        assignment_digest(
            &plan,
            &write(RuntimePlaceInitialization::Conditional, false)
        )
    );
    assert_ne!(
        initial,
        assignment_digest(&plan, &write(RuntimePlaceInitialization::Initialized, true))
    );
}

#[test]
fn meter_rejection_precedes_local_resolution_and_prevents_digest() {
    let (plan, _) = plan(false);
    let unknown = RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::new(99).unwrap());
    let mut meter = TaskSemanticMeter::new(0, 100);
    let mut encoder = TaskSemanticEncoder::new(b"body-test.v1\0", &mut meter);
    encoder.tag(0);
    assert!(matches!(
        RuntimeBodySemanticContext::new(&plan).write_local(&mut encoder, unknown),
        Err(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    assert_eq!(
        encoder.finish(),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
}

#[test]
fn actual_local_expression_resolves_origin_before_hashing() {
    let (a_plan, a) = plan(false);
    let (b_plan, b) = plan(true);
    let expression = |local| {
        crate::value::RuntimeExpr::from_admitted_parts(
            crate::runtime_id::RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN),
            crate::value::RuntimeExprKind::Local(
                crate::value::RuntimeLocalRead::from_admitted_parts(
                    local,
                    crate::value::RuntimeLocalReadMode::Copy,
                ),
            ),
        )
    };
    let hash = |plan: &RuntimePlan, expr: &crate::value::RuntimeExpr| {
        let mut meter = TaskSemanticMeter::new(100, 1000);
        let mut encoder = TaskSemanticEncoder::new(b"expr-test.v1\0", &mut meter);
        RuntimeBodySemanticContext::new(plan)
            .write_expression(&mut encoder, expr)
            .unwrap();
        encoder.finish().unwrap()
    };
    assert_eq!(hash(&a_plan, &expression(a)), hash(&b_plan, &expression(b)));
}

#[test]
fn pattern_literals_and_guard_branch_metadata_enter_the_actual_body_transcript() {
    let (plan, _) = plan(false);
    let ty = crate::runtime_id::RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN);
    let expression = |literal| {
        crate::value::RuntimeExpr::from_admitted_parts(
            ty,
            crate::value::RuntimeExprKind::IfLet {
                pattern: crate::pattern::RuntimePattern::from_admitted_parts(
                    ty,
                    crate::pattern::RuntimePatternKind::Literal(crate::value::RuntimeValue::Bool(
                        literal,
                    )),
                ),
                expr: Box::new(crate::value::RuntimeExpr::from_admitted_parts(
                    ty,
                    crate::value::RuntimeExprKind::Value(crate::value::RuntimeValue::Bool(true)),
                )),
                guard: None,
                then_expr: Box::new(crate::value::RuntimeExpr::from_admitted_parts(
                    ty,
                    crate::value::RuntimeExprKind::Value(crate::value::RuntimeValue::Bool(true)),
                )),
                else_expr: Box::new(crate::value::RuntimeExpr::from_admitted_parts(
                    ty,
                    crate::value::RuntimeExprKind::Value(crate::value::RuntimeValue::Bool(false)),
                )),
            },
        )
    };
    let digest = |expr| {
        let mut meter = TaskSemanticMeter::new(1000, 10000);
        let mut encoder = TaskSemanticEncoder::new(b"pattern-body.v1\0", &mut meter);
        RuntimeBodySemanticContext::new(&plan)
            .write_expression(&mut encoder, &expr)
            .unwrap();
        encoder.finish().unwrap()
    };
    assert_ne!(digest(expression(true)), digest(expression(false)));
}

#[test]
fn template_transcript_commits_owner_digest_and_delayed_effect_duration() {
    use crate::plan::{
        RuntimeDialogueContentEffectSlotSeed, RuntimeDialogueContentEffectTrigger,
        RuntimeDialogueContentTemplateManifestSeed,
    };
    let make = |delay| {
        let mut builder = RuntimePlanBuilder::new();
        let template =
            crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0).unwrap();
        builder
            .register_dialogue_content_template_seed(RuntimeDialogueContentTemplateManifestSeed {
                id: template,
                digest: crate::entry::RuntimeDialogueContentTemplateDigest::from_bytes([22; 32]),
                slots: Box::new([]),
                effects: Box::new([RuntimeDialogueContentEffectSlotSeed {
                    site: crate::runtime_id::RuntimeDialogueEffectSiteId::from_zero_based(0)
                        .unwrap(),
                    trigger: RuntimeDialogueContentEffectTrigger::Delay {
                        duration: crate::time::LogicalDuration::from_nanos(delay),
                    },
                    capture_types: Box::new([]),
                }]),
            })
            .unwrap();
        (builder.finish().unwrap(), template)
    };
    let hash = |plan: &RuntimePlan, template| {
        let mut meter = TaskSemanticMeter::new(100, 1000);
        let mut encoder = TaskSemanticEncoder::new(b"template-test.v1\0", &mut meter);
        RuntimeBodySemanticContext::new(plan)
            .write_content_template(&mut encoder, template)
            .unwrap();
        encoder.finish().unwrap()
    };
    let (first, a) = make(10);
    let (second, b) = make(11);
    assert_ne!(hash(&first, a), hash(&second, b));
}

fn callable_plan(
    padding: bool,
    definition: u8,
) -> (RuntimePlan, crate::runtime_id::RuntimeCallableStateId) {
    use crate::plan::*;
    let mut builder = RuntimePlanBuilder::new();
    let unit = RuntimeSemanticTypeId::from_bytes([31; 32]);
    let arrow = RuntimeSemanticTypeId::from_bytes([32; 32]);
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(
                    arrow,
                    RuntimePlanTypeProjection::Function {
                        contract: RuntimeFunctionTypeContract::new(
                            RuntimeTypeBinder::new(0, 0, 0),
                            crate::effect_row::EffectPredicate::unconstrained(),
                            crate::effect_row::EffectFormula::empty(),
                        ),
                        parameters: Box::new([]),
                        result: unit,
                    },
                ),
            ],
            [],
        )
        .unwrap();
    let push_function = |builder: &mut RuntimePlanBuilder, identity| {
        builder
            .push_function_site_seed(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([identity; 32]),
                RuntimeFunctionSemanticRole::Closure,
                [],
                RuntimeExprSeed::new(
                    unit,
                    RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Unit),
                ),
            )
            .unwrap()
    };
    if padding {
        let function = push_function(&mut builder, 99);
        let state = builder.reserve_callable_state_seed().unwrap();
        builder
            .define_callable_state_seed(
                &state,
                RuntimeCallableStateSeed {
                    function_type: arrow,
                    origin: state.clone(),
                    position: RuntimeCallablePosition::Unapplied,
                    retained: Box::new([]),
                    parameters: Box::new([]),
                    result: unit,
                    attached: RuntimeCallableAttachedContract::None,
                    transition: RuntimeCallableTransition::Invoke {
                        function,
                        captures: Box::new([]),
                        arguments: Box::new([]),
                    },
                    partials: Box::new([]),
                },
            )
            .unwrap();
    }
    let function = push_function(&mut builder, definition);
    let state = builder.reserve_callable_state_seed().unwrap();
    builder
        .define_callable_state_seed(
            &state,
            RuntimeCallableStateSeed {
                function_type: arrow,
                origin: state.clone(),
                position: RuntimeCallablePosition::Unapplied,
                retained: Box::new([]),
                parameters: Box::new([]),
                result: unit,
                attached: RuntimeCallableAttachedContract::None,
                transition: RuntimeCallableTransition::Invoke {
                    function,
                    captures: Box::new([]),
                    arguments: Box::new([]),
                },
                partials: Box::new([]),
            },
        )
        .unwrap();
    let plan = builder.finish().unwrap();
    let state = plan.callable_states().iter_with_ids().last().unwrap().0;
    (plan, state)
}

#[test]
fn callable_semantics_resolve_definition_and_ignore_state_and_function_arena_padding() {
    let (first, a) = callable_plan(false, 41);
    let (padded, b) = callable_plan(true, 41);
    let (changed, c) = callable_plan(false, 42);
    assert_ne!(a, b);
    let digest = |plan: &RuntimePlan, state| {
        let mut meter = TaskSemanticMeter::new(1000, 10000);
        let mut encoder = TaskSemanticEncoder::new(b"callable-body.v1\0", &mut meter);
        RuntimeBodySemanticContext::new(plan)
            .write_callable_state(&mut encoder, state)
            .unwrap();
        encoder.finish().unwrap()
    };
    assert_eq!(digest(&first, a), digest(&padded, b));
    assert_ne!(digest(&first, a), digest(&changed, c));
}

#[test]
fn invalid_body_reference_poison_prevents_ignored_error_digest_publication() {
    let (plan, _) = plan(false);
    let mut meter = TaskSemanticMeter::new(100, 1000);
    let mut encoder = TaskSemanticEncoder::new(b"invalid-body.v1\0", &mut meter);
    let unknown = RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::new(99).unwrap());
    assert!(matches!(
        RuntimeBodySemanticContext::new(&plan).write_local(&mut encoder, unknown),
        Err(RuntimeBodySemanticError::UnknownLocal { .. })
    ));
    encoder.tag(0);
    assert_eq!(
        encoder.finish(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
}

fn stream_plan(reverse: bool, branch: bool) -> RuntimePlan {
    use crate::plan::{
        RuntimeExprSeed, RuntimeExprSeedKind, RuntimeStreamOpSeed as Op, RuntimeStreamPlanSeed,
    };
    let mut builder = RuntimePlanBuilder::new();
    let boolean = RuntimeSemanticTypeId::from_bytes([61; 32]);
    let unit = RuntimeSemanticTypeId::from_bytes([62; 32]);
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
            ],
            [],
        )
        .unwrap();
    let expr = |value| {
        RuntimeExprSeed::new(
            boolean,
            RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(value)),
        )
    };
    let mut ops = vec![
        Op::Yield { expr: expr(true) },
        Op::Yield { expr: expr(false) },
    ];
    if reverse {
        ops.reverse();
    }
    builder
        .push_stream_plan_seed(RuntimeStreamPlanSeed {
            id: crate::stream::StreamRuntimeId::canonical("items").unwrap(),
            item_ty: boolean,
            error_ty: unit,
            ops: vec![Op::If {
                condition: expr(true),
                then_ops: if branch { ops.clone() } else { vec![] },
                else_ops: if branch { vec![] } else { ops },
            }],
        })
        .unwrap();
    builder.finish().unwrap()
}

#[test]
fn stream_body_order_and_empty_branch_role_are_semantic() {
    let first = stream_plan(false, true);
    let reordered = stream_plan(true, true);
    let branch = stream_plan(false, false);
    let digest = |plan: &RuntimePlan| {
        let mut meter = TaskSemanticMeter::new(1000, 10000);
        let mut encoder = TaskSemanticEncoder::new(b"stream-body.v1\0", &mut meter);
        RuntimeBodySemanticContext::new(plan)
            .write_stream(&mut encoder, &plan.stream_plans()[0])
            .unwrap();
        encoder.finish().unwrap()
    };
    assert_ne!(digest(&first), digest(&reordered));
    assert_ne!(digest(&first), digest(&branch));
}

#[test]
fn stream_body_work_quota_poison_prevents_digest_publication() {
    let plan = stream_plan(false, true);
    let mut meter = TaskSemanticMeter::new(3, 10000);
    let mut encoder = TaskSemanticEncoder::new(b"stream-body.v1\0", &mut meter);
    assert!(matches!(
        RuntimeBodySemanticContext::new(&plan).write_stream(&mut encoder, &plan.stream_plans()[0]),
        Err(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    assert_eq!(
        encoder.finish(),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
}

fn flow_plan(reverse: bool, branch: bool) -> RuntimePlan {
    use crate::plan::{
        RuntimeEffectSet, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFlowOpSeed as Op,
        RuntimeFlowSeed, RuntimeFunctionDefinitionIdentity,
    };
    let mut builder = RuntimePlanBuilder::new();
    let boolean = RuntimeSemanticTypeId::from_bytes([61; 32]);
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                boolean,
                RuntimePlanTypeProjection::Bool,
            )],
            [],
        )
        .unwrap();
    let mut body = vec![
        Op::ReturnExpr(RuntimeExprSeed::new(
            boolean,
            RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(true)),
        )),
        Op::Noop,
    ];
    if reverse {
        body.reverse();
    }
    builder
        .push_flow_schema(crate::entry::RuntimeFlowSchema {
            flow: crate::plan::FlowRuntimeId::canonical("body").unwrap(),
            parameters: vec![],
        })
        .unwrap();
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            RuntimeFunctionDefinitionIdentity::from_accepted_identity([70; 32]),
            crate::plan::FlowRuntimeId::canonical("body").unwrap(),
            [],
            RuntimeEffectSet::empty(),
            vec![Op::If {
                condition: RuntimeExprSeed::new(
                    boolean,
                    RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(true)),
                ),
                then_ops: if branch { body.clone() } else { vec![] },
                else_ops: if branch { vec![] } else { body },
            }],
        ))
        .unwrap();
    builder.finish().unwrap()
}

#[test]
fn flow_body_operation_order_empty_branch_and_static_literal_are_committed() {
    let owner = RuntimePlanBuilder::new().task_coordinate_owner(0);
    let hash = |plan: &RuntimePlan| {
        let mut meter = TaskSemanticMeter::new(1000, 10000);
        let mut encoder = TaskSemanticEncoder::new(b"flow-body.v1\0", &mut meter);
        RuntimeBodySemanticContext::new(plan)
            .write_flow(
                &mut encoder,
                plan.flows()[0].body().ops(),
                &owner,
                &mut |_| panic!("no task edge in this body"),
            )
            .unwrap();
        encoder.finish().unwrap()
    };
    assert_ne!(hash(&flow_plan(false, true)), hash(&flow_plan(true, true)));
    assert_ne!(
        hash(&flow_plan(false, true)),
        hash(&flow_plan(false, false))
    );
}

#[test]
fn engine_only_flow_continuation_is_rejected_and_poisoned() {
    let (plan, _) = plan(false);
    let owner = RuntimePlanBuilder::new().task_coordinate_owner(0);
    let mut meter = TaskSemanticMeter::new(1000, 10000);
    let mut encoder = TaskSemanticEncoder::new(b"flow-body.v1\0", &mut meter);
    assert!(matches!(
        RuntimeBodySemanticContext::new(&plan).write_flow(
            &mut encoder,
            &[crate::plan::FlowOp::Bind(vec![])],
            &owner,
            &mut |_| panic!("no task edge")
        ),
        Err(RuntimeBodySemanticError::RuntimeFlowContinuation)
    ));
    assert_eq!(
        encoder.finish(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
}

#[test]
fn task_coordinates_share_the_aggregate_issuer_and_reject_foreign_same_ordinal() {
    let builder = RuntimePlanBuilder::new();
    let owner = builder.task_coordinate_owner(2);
    let same = builder.task_coordinate_owner(2);
    let foreign = RuntimePlanBuilder::new().task_coordinate_owner(2);
    let coordinate = owner.resolve(1).unwrap();
    assert_eq!(coordinate.ordinal(), 1);
    assert!(same.contains(&coordinate));
    assert!(!foreign.contains(&coordinate));
    assert!(owner.resolve(2).is_none());
}

#[test]
fn flow_effect_metadata_changes_the_body_transcript() {
    let (plan, _) = plan(false);
    let owner = RuntimePlanBuilder::new().task_coordinate_owner(0);
    let hash = |delay| {
        let mut meter = TaskSemanticMeter::new(1000, 10000);
        let mut encoder = TaskSemanticEncoder::new(b"flow-body.v1\0", &mut meter);
        RuntimeBodySemanticContext::new(&plan)
            .write_flow(
                &mut encoder,
                &[crate::plan::FlowOp::Effect(
                    crate::effect::LineEffectRequest::Wait(
                        crate::effect::RuntimeWaitTarget::Duration(
                            crate::time::LogicalDuration::from_nanos(delay),
                        ),
                    ),
                )],
                &owner,
                &mut |_| panic!("no task edge"),
            )
            .unwrap();
        encoder.finish().unwrap()
    };
    assert_ne!(hash(10), hash(11));
}

#[test]
fn flow_host_edge_rejects_foreign_coordinate_and_never_reads_completed_plan_digest() {
    use crate::plan::body_semantic::flow::RuntimeBodyTaskSource;
    let (plan, _) = plan(false);
    let ty = crate::runtime_id::RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN);
    let owner = RuntimePlanBuilder::new().task_coordinate_owner(1);
    let foreign = RuntimePlanBuilder::new().task_coordinate_owner(1);
    let target = |digest| crate::plan::RuntimeHostCallTarget {
        producer: crate::task::HostCallProducerDefinition {
            contract: crate::task::NeedProducerContractDigest::from_bytes([1; 32]),
            plan: crate::task::TaskPlanSemanticDigest::from_bytes([digest; 32]),
            site: crate::task::NeedProducerSiteDigest::from_bytes([3; 32]),
        },
        public_id: "test.notify".to_owned(),
        capability: "test".to_owned(),
        operation: "notify".to_owned(),
        contract: None,
        args: vec![],
        result: ty,
        mode: crate::step::RuntimeHostCallMode::Suspend,
        deterministic: false,
    };
    let hash = |digest| {
        let mut meter = TaskSemanticMeter::new(1000, 10000);
        let mut encoder = TaskSemanticEncoder::new(b"host-coordinate.v1\0", &mut meter);
        RuntimeBodySemanticContext::new(&plan)
            .write_flow(
                &mut encoder,
                &[crate::plan::FlowOp::HostCall {
                    binding: None,
                    target: target(digest),
                }],
                &owner,
                &mut |source| {
                    assert!(matches!(source, RuntimeBodyTaskSource::Host(_)));
                    Ok(owner.resolve(0).unwrap())
                },
            )
            .unwrap();
        encoder.finish().unwrap()
    };
    assert_eq!(hash(2), hash(9));
    let mut meter = TaskSemanticMeter::new(1000, 10000);
    let mut encoder = TaskSemanticEncoder::new(b"host-coordinate.v1\0", &mut meter);
    assert!(matches!(
        RuntimeBodySemanticContext::new(&plan).write_flow(
            &mut encoder,
            &[crate::plan::FlowOp::HostCall {
                binding: None,
                target: target(2)
            }],
            &owner,
            &mut |_| Ok(foreign.resolve(0).unwrap())
        ),
        Err(RuntimeBodySemanticError::ForeignTaskCoordinate)
    ));
    assert_eq!(
        encoder.finish(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
}

#[test]
fn audio_loop_and_microphone_constraints_enter_static_effect_metadata() {
    let hash = |command: crate::audio::RuntimeAudioCommand| {
        let mut meter = TaskSemanticMeter::new(1000, 10000);
        let mut encoder = TaskSemanticEncoder::new(b"audio-metadata.v1\0", &mut meter);
        crate::effect::LineEffectRequest::Audio(Box::new(command))
            .encode_body_metadata(&mut encoder);
        encoder.finish().unwrap()
    };
    let expr = || {
        crate::value::RuntimeExpr::from_admitted_parts(
            crate::runtime_id::RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN),
            crate::value::RuntimeExprKind::Value(crate::value::RuntimeValue::Bool(true)),
        )
    };
    let microphone = |channels| crate::audio::RuntimeAudioCommand::RequestMicrophone {
        capture: expr(),
        constraints: arcweft_interaction_model::audio::MicrophoneConstraints {
            channels,
            preferred_sample_rate_hz: Some(48_000),
            echo_cancellation: false,
            noise_suppression: true,
            auto_gain_control: false,
        },
    };
    assert_ne!(hash(microphone(1)), hash(microphone(2)));
    let play = |end| crate::audio::RuntimeAudioCommand::Play {
        voice: expr(),
        resource: expr(),
        bus: expr(),
        gain_db_milli: expr(),
        pan_milli: expr(),
        loop_mode: arcweft_interaction_model::audio::AudioLoopMode::Region {
            start_frame: 1,
            end_frame: end,
        },
        start_frame: expr(),
        fade_in_millis: expr(),
    };
    assert_ne!(hash(play(10)), hash(play(11)));
}

#[test]
fn actual_function_row_commits_body_value_but_ignores_function_arena_padding() {
    let make = |padding, value| {
        let mut builder = RuntimePlanBuilder::new();
        let boolean = RuntimeSemanticTypeId::from_bytes([61; 32]);
        builder
            .admit_type_batch(
                [RuntimePlanTypeSeed::new(
                    boolean,
                    RuntimePlanTypeProjection::Bool,
                )],
                [],
            )
            .unwrap();
        let owner = builder.task_coordinate_owner(0);
        if padding {
            builder
                .push_function_site_seed(
                    crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                        [99; 32],
                    ),
                    crate::plan::RuntimeFunctionSemanticRole::Ordinary,
                    [],
                    crate::plan::RuntimeExprSeed::new(
                        boolean,
                        crate::plan::RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(
                            false,
                        )),
                    ),
                )
                .unwrap();
        }
        builder
            .push_function_site_seed(
                crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([71; 32]),
                crate::plan::RuntimeFunctionSemanticRole::Ordinary,
                [],
                crate::plan::RuntimeExprSeed::new(
                    boolean,
                    crate::plan::RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(
                        value,
                    )),
                ),
            )
            .unwrap();
        let plan = builder.finish().unwrap();
        let id = crate::runtime_id::RuntimeFunctionSiteId::from_accepted_ordinal(
            NonZeroU32::new(if padding { 2 } else { 1 }).unwrap(),
        );
        (plan, owner, id)
    };
    let first = make(false, true);
    let padding = make(true, true);
    let changed = make(false, false);
    let hash = |(plan, owner, id): &(RuntimePlan, _, _)| {
        let mut meter = TaskSemanticMeter::new(1000, 10000);
        RuntimeBodySemanticContext::new(plan)
            .function_row_digest(&mut meter, *id, owner, &mut |_| panic!("no task edge"))
            .unwrap()
    };
    assert_eq!(hash(&first), hash(&padding));
    assert_ne!(hash(&first), hash(&changed));
}
