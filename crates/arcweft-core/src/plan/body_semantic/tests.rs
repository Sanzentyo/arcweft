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

fn producer_expression_plan(
    padding: bool,
    value: bool,
    role: crate::plan::RuntimeFunctionSemanticRole,
) -> (
    RuntimePlan,
    crate::plan::construction::task_coordinates::RuntimeTaskPlanCoordinateOwner,
    crate::runtime_id::RuntimeFunctionSiteId,
) {
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
                crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([99; 32]),
                role,
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
            role,
            [],
            crate::plan::RuntimeExprSeed::new(
                boolean,
                crate::plan::RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(value)),
            ),
        )
        .unwrap();
    let plan = builder.finish().unwrap();
    let id = crate::runtime_id::RuntimeFunctionSiteId::from_accepted_ordinal(
        NonZeroU32::new(if padding { 2 } else { 1 }).unwrap(),
    );
    (plan, owner, id)
}

#[test]
fn producer_function_digest_commits_actual_body_for_every_function_role() {
    for role in crate::plan::RuntimeFunctionSemanticRole::ALL {
        let first = producer_expression_plan(false, true, *role);
        let padded = producer_expression_plan(true, true, *role);
        let changed = producer_expression_plan(false, false, *role);
        let hash = |(plan, owner, function): &(RuntimePlan, _, _)| {
            let mut meter = TaskSemanticMeter::new(10_000, 100_000);
            RuntimeBodySemanticContext::new(plan)
                .producer_function_digest(
                    &mut meter,
                    *function,
                    owner,
                    &mut |_| panic!("no task edge"),
                    crate::plan::RuntimeTaskPlanSealLimits::default(),
                )
                .unwrap()
        };
        assert_eq!(hash(&first), hash(&padded));
        assert_ne!(hash(&first), hash(&changed));
    }
}

#[test]
fn retained_transfer_encoding_never_conflates_external_formal_and_language_moves() {
    use crate::plan::{RuntimeFunctionCaptureMode as C, RuntimeFunctionInputTransfer as T};
    let kinds = [
        T::Transferred(C::Copy),
        T::Transferred(C::SnapshotClone),
        T::Transferred(C::Move),
        T::ExternalBinding,
        T::Formal,
    ];
    let mut hashes = std::collections::HashSet::new();
    for kind in kinds {
        let mut meter = TaskSemanticMeter::new(10, 10);
        let mut encoder = TaskSemanticEncoder::new(b"t", &mut meter);
        kind.encode_semantic_transfer(&mut encoder);
        hashes.insert(encoder.finish().unwrap());
    }
    assert_eq!(hashes.len(), 5);
}

#[test]
fn producer_digest_meter_is_shared_and_poisoned_on_body_failure() {
    let (plan, owner, function) = producer_expression_plan(
        false,
        true,
        crate::plan::RuntimeFunctionSemanticRole::Ordinary,
    );
    let mut meter = TaskSemanticMeter::new(1, 10000);
    assert!(matches!(
        RuntimeBodySemanticContext::new(&plan).producer_function_digest(
            &mut meter,
            function,
            &owner,
            &mut |_| panic!("no task edge"),
            crate::plan::RuntimeTaskPlanSealLimits::default()
        ),
        Err(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    let encoder = TaskSemanticEncoder::new(b"later", &mut meter);
    assert_eq!(
        encoder.finish(),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
}

fn producer_host_plan(
    in_then: bool,
) -> (
    RuntimePlan,
    crate::plan::construction::task_coordinates::RuntimeTaskPlanCoordinateOwner,
    crate::runtime_id::RuntimeFunctionSiteId,
) {
    producer_host_plan_with_arguments(in_then, vec![])
}

fn producer_host_plan_with_arguments(
    in_then: bool,
    args: Vec<crate::plan::RuntimeHostArgumentSeed>,
) -> (
    RuntimePlan,
    crate::plan::construction::task_coordinates::RuntimeTaskPlanCoordinateOwner,
    crate::runtime_id::RuntimeFunctionSiteId,
) {
    producer_host_plan_with_setup(in_then, |_| (args, Box::new([])))
}

fn producer_host_plan_with_setup(
    in_then: bool,
    setup: impl FnOnce(
        &mut RuntimePlanBuilder,
    ) -> (
        Vec<crate::plan::RuntimeHostArgumentSeed>,
        Box<[crate::plan::RuntimeFunctionInputBindingSeed]>,
    ),
) -> (
    RuntimePlan,
    crate::plan::construction::task_coordinates::RuntimeTaskPlanCoordinateOwner,
    crate::runtime_id::RuntimeFunctionSiteId,
) {
    try_producer_request_plan_with_setup(in_then, None, setup).unwrap()
}

fn try_producer_request_plan_with_setup(
    in_then: bool,
    start_plan: Option<crate::task::NeedProducerTaskPlan>,
    setup: impl FnOnce(
        &mut RuntimePlanBuilder,
    ) -> (
        Vec<crate::plan::RuntimeHostArgumentSeed>,
        Box<[crate::plan::RuntimeFunctionInputBindingSeed]>,
    ),
) -> Result<
    (
        RuntimePlan,
        crate::plan::construction::task_coordinates::RuntimeTaskPlanCoordinateOwner,
        crate::runtime_id::RuntimeFunctionSiteId,
    ),
    crate::plan::RuntimePlanBuildError,
> {
    use crate::plan::*;
    let mut builder = RuntimePlanBuilder::new();
    let unit = RuntimeSemanticTypeId::from_bytes([62; 32]);
    let boolean = RuntimeSemanticTypeId::from_bytes([61; 32]);
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
            ],
            [],
        )
        .unwrap();
    let need = RuntimeSemanticTypeId::from_bytes([63; 32]);
    if start_plan.is_some() {
        builder
            .admit_type_batch(
                [RuntimePlanTypeSeed::new(
                    need,
                    RuntimePlanTypeProjection::Need(unit),
                )],
                [],
            )
            .unwrap();
    }
    let (args, inputs) = setup(&mut builder);
    let effects = RuntimeEffectSet::empty();
    let site = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([72; 32]),
            role: RuntimeFunctionSemanticRole::Ordinary,
            function_type: None,
            inputs,
            result: unit,
            body_kind: RuntimeFunctionSiteBodyKind::Executable,
            effects: effects.clone(),
        })
        .unwrap();
    let host = if let Some(plan) = start_plan {
        RuntimeFlowOpSeed::StartNeedProducer {
            binding: RuntimePatternSeed::new(need, RuntimePatternSeedKind::Discard),
            target: RuntimeNeedProducerStartTargetSeed {
                plan,
                arguments: args,
            },
        }
    } else {
        RuntimeFlowOpSeed::HostCall {
            binding: None,
            target: RuntimeHostCallTargetSeed {
                producer: crate::task::HostCallProducerDefinition {
                    contract: crate::task::NeedProducerContractDigest::from_bytes([1; 32]),
                    plan: crate::task::TaskPlanSemanticDigest::from_bytes([2; 32]),
                    site: crate::task::NeedProducerSiteDigest::from_bytes([3; 32]),
                },
                public_id: "test.notify".to_owned(),
                capability: "test".to_owned(),
                operation: "notify".to_owned(),
                contract: None,
                args,
                result: unit,
                mode: crate::step::RuntimeHostCallMode::Suspend,
                deterministic: false,
            },
        }
    };
    builder.define_function_site_seed(
        &site,
        RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
            effects,
            ops: Box::new([
                RuntimeFlowOpSeed::If {
                    condition: RuntimeExprSeed::new(
                        boolean,
                        RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(true)),
                    ),
                    then_ops: if in_then { vec![host.clone()] } else { vec![] },
                    else_ops: if in_then { vec![] } else { vec![host] },
                },
                RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                    unit,
                    RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Unit),
                )),
            ]),
        }),
    )?;
    let owner = builder.task_coordinate_owner(1);
    Ok((
        builder.finish()?,
        owner,
        crate::runtime_id::RuntimeFunctionSiteId::from_accepted_ordinal(NonZeroU32::MIN),
    ))
}

#[test]
fn producer_endpoint_branch_path_and_role_quota_enter_the_same_transcript() {
    let first = producer_host_plan(true);
    let second = producer_host_plan(false);
    let hash = |(plan, owner, function): &(RuntimePlan, _, _)| {
        let mut meter = TaskSemanticMeter::new(10_000, 100_000);
        RuntimeBodySemanticContext::new(plan)
            .producer_function_digest(
                &mut meter,
                *function,
                owner,
                &mut |_| Ok(owner.resolve(0).unwrap()),
                crate::plan::RuntimeTaskPlanSealLimits::default(),
            )
            .unwrap()
    };
    assert_ne!(hash(&first), hash(&second));
    let mut meter = TaskSemanticMeter::new(10_000, 100_000);
    assert!(matches!(
        RuntimeBodySemanticContext::new(&first.0).producer_function_digest(
            &mut meter,
            first.2,
            &first.1,
            &mut |_| panic!("quota must precede resolver"),
            crate::plan::RuntimeTaskPlanSealLimits {
                max_function_roles: 0,
                ..Default::default()
            }
        ),
        Err(RuntimeBodySemanticError::FunctionRoles {
            actual: 1,
            maximum: 0
        })
    ));
}

#[test]
fn request_template_commits_static_role_order_and_paths_under_actual_producer_endpoint() {
    use super::request::*;
    let (plan, owner, function) = producer_host_plan(true);
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let producer = RuntimeBodySemanticContext::new(&plan)
        .producer_function(
            &mut meter,
            function,
            &owner,
            &mut |_| Ok(owner.resolve(0).unwrap()),
            crate::plan::RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    assert!(producer.endpoint(1).is_none());
    let ty = crate::runtime_id::RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN);
    let hash = |reverse, path| {
        let mut meter = TaskSemanticMeter::new(10_000, 100_000);
        let argument = |ordinal| RuntimeRequestArgument {
            role: RuntimeRequestArgumentRole::Positional,
            identity: None,
            ty,
            source: RuntimeRequestValueSource::Literal,
            path: Box::new([RuntimeRequestPathStep::Operand(ordinal)]),
        };
        let arguments = if reverse {
            [argument(1), argument(path)]
        } else {
            [argument(path), argument(1)]
        };
        RuntimeBodySemanticContext::new(&plan)
            .request_template_digest(
                &mut meter,
                producer.endpoint(0).unwrap(),
                &arguments,
                &[],
                crate::plan::RuntimeTaskPlanSealLimits::default(),
            )
            .unwrap()
    };
    assert_ne!(hash(false, 0), hash(true, 0));
    assert_ne!(hash(false, 0), hash(false, 2));
}

#[test]
fn request_role_quota_poison_precedes_type_resolution() {
    use super::request::*;
    let (plan, owner, function) = producer_host_plan(true);
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let producer = RuntimeBodySemanticContext::new(&plan)
        .producer_function(
            &mut meter,
            function,
            &owner,
            &mut |_| Ok(owner.resolve(0).unwrap()),
            crate::plan::RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    let unknown =
        crate::runtime_id::RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::new(99).unwrap());
    let args = [RuntimeRequestArgument {
        role: RuntimeRequestArgumentRole::Named,
        identity: Some(RuntimeRequestRoleIdentity::from_accepted_identity([9; 32])),
        ty: unknown,
        source: RuntimeRequestValueSource::Local,
        path: Box::new([]),
    }];
    let mut meter = TaskSemanticMeter::new(1000, 10000);
    assert!(matches!(
        RuntimeBodySemanticContext::new(&plan).request_template_digest(
            &mut meter,
            producer.endpoint(0).unwrap(),
            &args,
            &[],
            crate::plan::RuntimeTaskPlanSealLimits {
                max_request_roles: 0,
                ..Default::default()
            }
        ),
        Err(RuntimeBodySemanticError::RequestRoles {
            actual: 1,
            maximum: 0
        })
    ));
    assert_eq!(
        TaskSemanticEncoder::new(b"later", &mut meter).finish(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
}

#[test]
fn request_template_field_identity_roles_and_shared_budget_affect_acceptance() {
    use super::request::*;
    let (plan, owner, function) = producer_host_plan(true);
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let producer = RuntimeBodySemanticContext::new(&plan)
        .producer_function(
            &mut meter,
            function,
            &owner,
            &mut |_| Ok(owner.resolve(0).unwrap()),
            crate::plan::RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    let ty = crate::runtime_id::RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN);
    let field = |identity, role| RuntimeRequestField {
        identity: RuntimeRequestRoleIdentity::from_accepted_identity([identity; 32]),
        role,
        ty,
        path: Box::new([RuntimeRequestPathStep::Operand(0)]),
    };
    let digest = |identity, role| {
        let mut meter = TaskSemanticMeter::new(10_000, 100_000);
        RuntimeBodySemanticContext::new(&plan)
            .request_template_digest(
                &mut meter,
                producer.endpoint(0).unwrap(),
                &[],
                &[field(identity, role)],
                crate::plan::RuntimeTaskPlanSealLimits::default(),
            )
            .unwrap()
    };
    assert_ne!(
        digest(1, RuntimeRequestFieldRole::Required),
        digest(2, RuntimeRequestFieldRole::Required)
    );
    assert_ne!(
        digest(1, RuntimeRequestFieldRole::Required),
        digest(1, RuntimeRequestFieldRole::Optional)
    );
    let mut meter = TaskSemanticMeter::new(1, 100_000);
    assert!(matches!(
        RuntimeBodySemanticContext::new(&plan).request_template_digest(
            &mut meter,
            producer.endpoint(0).unwrap(),
            &[],
            &[field(1, RuntimeRequestFieldRole::Required)],
            crate::plan::RuntimeTaskPlanSealLimits::default(),
        ),
        Err(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    assert_eq!(
        TaskSemanticEncoder::new(b"later", &mut meter).finish(),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
}

#[test]
fn actual_host_request_uses_admitted_roles_and_excludes_display_names() {
    use crate::plan::{RuntimeExprSeed, RuntimeExprSeedKind, RuntimeHostArgumentSeed};
    use crate::task::{NamedHostArg, RuntimeRequestRoleIdentity};
    let make = |label: &str, identity| {
        producer_host_plan_with_arguments(
            true,
            vec![RuntimeHostArgumentSeed::Named(
                RuntimeRequestRoleIdentity::from_accepted_identity([identity; 32]),
                NamedHostArg {
                    name: label.to_owned(),
                    value: RuntimeExprSeed::new(
                        RuntimeSemanticTypeId::from_bytes([61; 32]),
                        RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(true)),
                    ),
                },
            )],
        )
    };
    let hash = |label: &str, identity| {
        let (plan, owner, function) = make(label, identity);
        let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
        let context = RuntimeBodySemanticContext::new(&plan);
        let producer = context
            .producer_function(
                &mut meter,
                function,
                &owner,
                &mut |_| Ok(owner.resolve(0).unwrap()),
                crate::plan::RuntimeTaskPlanSealLimits::default(),
            )
            .unwrap();
        let target = producer.endpoint(0).unwrap().host_target().unwrap();
        assert_eq!(target.args[0].identity().as_bytes(), &[identity; 32]);
        context
            .host_request_template_digest(
                &mut meter,
                producer.endpoint(0).unwrap(),
                Default::default(),
            )
            .unwrap()
    };
    assert_eq!(hash("first", 42), hash("renamed", 42));
    assert_ne!(hash("first", 42), hash("first", 43));

    let (plan, owner, function) = make("first", 42);
    let clone = plan.clone();
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let context = RuntimeBodySemanticContext::new(&plan);
    let producer = context
        .producer_function(
            &mut meter,
            function,
            &owner,
            &mut |_| Ok(owner.resolve(0).unwrap()),
            Default::default(),
        )
        .unwrap();
    assert!(matches!(
        RuntimeBodySemanticContext::new(&clone).host_request_template_digest(
            &mut meter,
            producer.endpoint(0).unwrap(),
            Default::default()
        ),
        Err(RuntimeBodySemanticError::InvalidHostRequestEndpoint)
    ));
    assert_eq!(
        TaskSemanticEncoder::new(b"later", &mut meter).finish(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
}

#[test]
fn actual_host_request_quota_precedes_source_projection() {
    use crate::plan::{RuntimeExprSeed, RuntimeExprSeedKind, RuntimeHostArgumentSeed};
    let (plan, owner, function) = producer_host_plan_with_arguments(
        true,
        vec![RuntimeHostArgumentSeed::Positional(
            crate::task::RuntimeRequestRoleIdentity::from_accepted_identity([42; 32]),
            RuntimeExprSeed::new(
                RuntimeSemanticTypeId::from_bytes([61; 32]),
                RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(true)),
            ),
        )],
    );
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let context = RuntimeBodySemanticContext::new(&plan);
    let producer = context
        .producer_function(
            &mut meter,
            function,
            &owner,
            &mut |_| Ok(owner.resolve(0).unwrap()),
            Default::default(),
        )
        .unwrap();
    let mut meter = TaskSemanticMeter::new(0, 0);
    assert!(matches!(
        context.host_request_template_digest(
            &mut meter,
            producer.endpoint(0).unwrap(),
            crate::plan::RuntimeTaskPlanSealLimits {
                max_request_roles: 0,
                ..Default::default()
            }
        ),
        Err(RuntimeBodySemanticError::RequestRoles {
            actual: 1,
            maximum: 0
        })
    ));
    assert_eq!(
        TaskSemanticEncoder::new(b"later", &mut meter).finish(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
}

#[test]
fn actual_host_request_recognizes_typed_capture_prologue_bindings() {
    use super::request::*;
    use crate::plan::*;
    let (plan, owner, function) = producer_host_plan_with_setup(true, |builder| {
        let boolean = RuntimeSemanticTypeId::from_bytes([61; 32]);
        let locals = builder
            .admit_type_batch(
                [],
                [
                    RuntimeLocalDeclarationSeed::new(
                        RuntimeLocalOrigin::Binding([31; 32]),
                        boolean,
                    ),
                    RuntimeLocalDeclarationSeed::new(
                        RuntimeLocalOrigin::Binding([32; 32]),
                        boolean,
                    ),
                ],
            )
            .unwrap();
        let input = locals.local_ids()[0].clone();
        let local = locals.local_ids()[1].clone();
        let expression = RuntimeExprSeed::new(
            boolean,
            RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                local.clone(),
                crate::value::RuntimeLocalReadMode::Copy,
            )),
        );
        (
            vec![RuntimeHostArgumentSeed::Positional(
                RuntimeRequestRoleIdentity::from_accepted_identity([42; 32]),
                expression,
            )],
            Box::new([RuntimeFunctionInputBindingSeed {
                transfer: RuntimeFunctionInputTransfer::Transferred(
                    RuntimeFunctionCaptureMode::Copy,
                ),
                origin: RuntimeFunctionInputOrigin::Binding([31; 32]),
                source: RuntimeFunctionInputSource::Capture { position: 0 },
                input_local: input,
                pattern: RuntimePatternSeed::new(boolean, RuntimePatternSeedKind::Typed { local }),
                ownership: RuntimeFunctionInputOwnershipRequirement::Owned,
                unrestricted_bindings: Box::new([]),
            }]),
        )
    });
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let context = RuntimeBodySemanticContext::new(&plan);
    let producer = context
        .producer_function(
            &mut meter,
            function,
            &owner,
            &mut |_| Ok(owner.resolve(0).unwrap()),
            Default::default(),
        )
        .unwrap();
    let endpoint = producer.endpoint(0).unwrap();
    let argument = |source| RuntimeRequestArgument {
        role: RuntimeRequestArgumentRole::Positional,
        identity: None,
        ty: endpoint.host_target().unwrap().args[0].value().ty(),
        source,
        path: Box::new([RuntimeRequestPathStep::Operand(0)]),
    };
    let actual = context
        .host_request_template_digest(&mut meter, endpoint, Default::default())
        .unwrap();
    let expected = context
        .request_template_digest(
            &mut meter,
            endpoint,
            &[argument(RuntimeRequestValueSource::Capture)],
            &[],
            Default::default(),
        )
        .unwrap();
    let wrong = context
        .request_template_digest(
            &mut meter,
            endpoint,
            &[argument(RuntimeRequestValueSource::Local)],
            &[],
            Default::default(),
        )
        .unwrap();
    assert_eq!(actual, expected);
    assert_ne!(actual, wrong);
}

/// Private image fixture probes graph termination independently of structural
/// admission: all references use actual plan rows, including deliberate cycles.
fn callable_graph_fixture(
    depth: usize,
    edges: usize,
) -> (RuntimePlan, crate::runtime_id::RuntimeCallableStateId) {
    use crate::plan::*;
    use crate::runtime_id::RuntimeCallableStateId as State;
    let (mut plan, origin) = callable_plan(false, 41);
    let prototype = plan.callable_states().get(origin).unwrap().clone();
    let mut rows = vec![prototype.clone()];
    for index in 1..depth {
        let mut row = prototype.clone();
        row.partials = (0..edges)
            .map(|parameter| RuntimeCallablePartialTransition {
                parameters: Box::new([RuntimeCallableParameterCoordinate {
                    group: 0,
                    parameter: u32::try_from(parameter).unwrap(),
                }]),
                state: State::for_index(index - 1).unwrap(),
                values: Box::new([]),
            })
            .collect();
        rows.push(row);
    }
    plan.callable_states = RuntimeCallableStateTable::from_admitted(rows);
    (plan, State::for_index(depth - 1).unwrap())
}

#[test]
fn callable_shared_dag_finishes_with_linear_work_and_exact_budget() {
    let (plan, root) = callable_graph_fixture(32, 2);
    let digest = |work, bytes| {
        let mut meter = TaskSemanticMeter::new(work, bytes);
        let mut encoder = TaskSemanticEncoder::new(b"callable-body.v1\0", &mut meter);
        let result =
            RuntimeBodySemanticContext::new(&plan).write_callable_state(&mut encoder, root);
        match result {
            Ok(()) => (Ok(encoder.finish().unwrap()), meter.totals()),
            Err(_) => (encoder.finish(), meter.totals()),
        }
    };
    let (first, (work, bytes)) = digest(10_000, 100_000);
    assert!(first.is_ok());
    assert!(
        work < 3_000,
        "32 shared levels must not expand as a binary tree"
    );
    assert_eq!(digest(work, bytes).0, first);
    assert_eq!(
        digest(work - 1, bytes).0,
        Err(TaskSemanticEncodingError::SemanticWork)
    );
    assert_eq!(
        digest(work, bytes - 1).0,
        Err(TaskSemanticEncodingError::TranscriptBytes)
    );
}

#[test]
fn callable_memo_keeps_source_edge_order_and_definition_changes() {
    use crate::plan::*;
    let (plan, root) = callable_graph_fixture(3, 2);
    let mut reordered = plan.clone();
    let mut rows = reordered
        .callable_states()
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    rows[root.index()].partials.reverse();
    reordered.callable_states = RuntimeCallableStateTable::from_admitted(rows);
    let digest = |plan: &RuntimePlan| {
        let mut meter = TaskSemanticMeter::new(10_000, 100_000);
        let mut encoder = TaskSemanticEncoder::new(b"callable-body.v1\0", &mut meter);
        RuntimeBodySemanticContext::new(plan)
            .write_callable_state(&mut encoder, root)
            .unwrap();
        encoder.finish().unwrap()
    };
    assert_ne!(digest(&plan), digest(&reordered));
    assert_eq!(digest(&plan), digest(&plan.clone()));
}

#[test]
fn callable_graph_rejects_definition_and_origin_cycles_with_sticky_failure() {
    use crate::plan::*;
    for origin_cycle in [false, true] {
        let (mut plan, root) = callable_graph_fixture(2, 1);
        let mut rows = plan.callable_states().iter().cloned().collect::<Vec<_>>();
        if origin_cycle {
            rows[0].transition = RuntimeCallableTransition::Retain {
                state: crate::runtime_id::RuntimeCallableStateId::for_index(0).unwrap(),
                values: Box::new([]),
            };
        } else {
            rows[root.index()].partials[0].state = root;
        }
        plan.callable_states = RuntimeCallableStateTable::from_admitted(rows);
        let mut meter = TaskSemanticMeter::new(10_000, 100_000);
        let mut encoder = TaskSemanticEncoder::new(b"callable-body.v1\0", &mut meter);
        assert!(matches!(
            RuntimeBodySemanticContext::new(&plan).write_callable_state(&mut encoder, root),
            Err(RuntimeBodySemanticError::CallableCycle { .. })
        ));
        encoder.tag(0);
        assert_eq!(
            encoder.finish(),
            Err(TaskSemanticEncodingError::OwnerRejected)
        );
    }
}

#[test]
fn callable_deep_acyclic_graph_uses_an_iterative_stack() {
    let (plan, root) = callable_graph_fixture(20_000, 1);
    let mut meter = TaskSemanticMeter::new(4_194_304, 67_108_864);
    let mut encoder = TaskSemanticEncoder::new(b"callable-body.v1\0", &mut meter);
    RuntimeBodySemanticContext::new(&plan)
        .write_callable_state(&mut encoder, root)
        .unwrap();
    encoder.finish().unwrap();
    assert!(meter.totals().0 < 1_500_000);
}

fn actual_line_plan(
    value: bool,
    cleanup_value: bool,
    trigger: &str,
    mark_name: &str,
) -> (RuntimePlan, crate::plan::RuntimeTaskPlanCoordinateOwner) {
    use crate::plan::*;
    use crate::runtime_id::*;
    let mut builder = RuntimePlanBuilder::new();
    let unit = RuntimeSemanticTypeId::from_bytes([81; 32]);
    let boolean = RuntimeSemanticTypeId::from_bytes([82; 32]);
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
            ],
            [],
        )
        .unwrap();
    let operation = |value| RuntimeFlowOpSeed::Let {
        pattern: RuntimePatternSeed::new(boolean, RuntimePatternSeedKind::Discard),
        expr: RuntimeExprSeed::new(
            boolean,
            RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(value)),
        ),
    };
    let content = builder
        .push_dialogue_content_seed(RuntimeDialogueContentPlanSeed {
            line: RuntimeLineId::from_source_entity_body("line.semantic_test").unwrap(),
            template: RuntimeDialogueContentTemplateManifestSeed {
                id: RuntimeDialogueContentTemplateId::from_zero_based(0).unwrap(),
                digest: crate::entry::RuntimeDialogueContentTemplateDigest::from_bytes([83; 32]),
                slots: Box::new([]),
                effects: Box::new([]),
            },
            values: Box::new([]),
            effect_sites: Box::new([]),
            marks: Box::new([mark_name.to_owned()]),
            effect_site_count: RuntimeDialogueEffectSiteCount::try_from_len(0).unwrap(),
        })
        .unwrap();
    let group = builder
        .push_line_task_group_seed(RuntimeLineTaskGroupSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([84; 32]),
            activation_ops: vec![],
            result_type: unit,
            handle_sites: Box::new([]),
            root: RuntimeLineTaskNodeSeed::Sequence(vec![
                RuntimeLineTaskNodeSeed::Action(vec![operation(value)]),
                RuntimeLineTaskNodeSeed::Action(vec![RuntimeFlowOpSeed::Noop]),
            ]),
            cancel_rules: Box::new([RuntimeLineTaskCancelRuleSeed {
                trigger: arcweft_interaction_model::input::InputActionId::new(trigger).unwrap(),
                action: vec![operation(true)],
            }]),
            cleanup_completed: vec![operation(cleanup_value)],
            cleanup_cancelled: vec![],
            cleanup_failed: vec![],
            cleanup_policy: crate::line_task::LineCleanupPolicy::default(),
        })
        .unwrap();
    builder
        .attach_line_task_group_seed(&content, &group)
        .unwrap();
    let owner = builder.task_coordinate_owner(0);
    (builder.finish().unwrap(), owner)
}

#[test]
fn actual_line_digest_commits_action_cancel_cleanup_and_order_but_not_mark_spelling() {
    use crate::plan::*;
    let digest = |plan: &RuntimePlan, owner: &RuntimeTaskPlanCoordinateOwner| {
        let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
        RuntimeBodySemanticContext::new(plan)
            .line_row_digest(
                &mut meter,
                crate::runtime_id::RuntimeLineTaskGroupId::from_zero_based(0).unwrap(),
                owner,
                &mut |_| panic!("no task references"),
                RuntimeTaskPlanSealLimits::default(),
            )
            .unwrap()
    };
    let first = actual_line_plan(true, true, "cancel", "mark");
    let renamed = actual_line_plan(true, true, "cancel", "renamed");
    assert_eq!(digest(&first.0, &first.1), digest(&renamed.0, &renamed.1));
    for changed in [
        actual_line_plan(false, true, "cancel", "mark"),
        actual_line_plan(true, false, "cancel", "mark"),
        actual_line_plan(true, true, "another", "mark"),
    ] {
        assert_ne!(digest(&first.0, &first.1), digest(&changed.0, &changed.1));
    }
    let mut reordered = first.0.clone();
    let old = &reordered.line_task_groups[0];
    let mut nodes = old.nodes().to_vec();
    if let crate::line_task::LineTaskNode::Sequence(children) = &mut nodes[0] {
        children.reverse();
    }
    reordered.line_task_groups[0] = crate::line_task::LineTaskGroup::new(
        old.definition(),
        old.captures().into(),
        old.activation_exports().into(),
        old.activation_ops().into(),
        old.result_type(),
        old.handle_sites().into(),
        old.root(),
        nodes.into(),
        old.cancel_rules().into(),
        old.cleanup().clone(),
    );
    assert_ne!(digest(&first.0, &first.1), digest(&reordered, &first.1));
    let mut changed_policy = first.0.clone();
    let old = &changed_policy.line_task_groups[0];
    let mut policy = old.cleanup().policy().clone();
    policy.audio = crate::line_task::AudioCleanup::KeepRegistered;
    let cleanup = crate::line_task::LineTaskCleanup::new(
        old.cleanup()
            .actions(crate::line_task::ScopeExit::Completed)
            .into(),
        old.cleanup()
            .actions(crate::line_task::ScopeExit::Cancelled)
            .into(),
        old.cleanup()
            .actions(crate::line_task::ScopeExit::Failed)
            .into(),
        policy,
    );
    changed_policy.line_task_groups[0] = crate::line_task::LineTaskGroup::new(
        old.definition(),
        old.captures().into(),
        old.activation_exports().into(),
        old.activation_ops().into(),
        old.result_type(),
        old.handle_sites().into(),
        old.root(),
        old.nodes().into(),
        old.cancel_rules().into(),
        cleanup,
    );
    assert_ne!(
        digest(&first.0, &first.1),
        digest(&changed_policy, &first.1)
    );
}

#[test]
fn actual_line_digest_preflights_children_and_uses_one_exact_meter() {
    let (plan, owner) = actual_line_plan(true, true, "cancel", "mark");
    let group = crate::runtime_id::RuntimeLineTaskGroupId::from_zero_based(0).unwrap();
    let mut meter = TaskSemanticMeter::new(0, 0);
    assert!(matches!(
        RuntimeBodySemanticContext::new(&plan).line_row_digest(
            &mut meter,
            group,
            &owner,
            &mut |_| panic!("no task refs"),
            crate::plan::RuntimeTaskPlanSealLimits {
                max_children_per_row: 0,
                ..Default::default()
            }
        ),
        Err(RuntimeBodySemanticError::LineChildren { .. })
    ));
    assert_eq!(
        TaskSemanticEncoder::new(b"later", &mut meter).finish(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
    let digest = |work, bytes| {
        let mut meter = TaskSemanticMeter::new(work, bytes);
        let result = RuntimeBodySemanticContext::new(&plan).line_row_digest(
            &mut meter,
            group,
            &owner,
            &mut |_| panic!("no task refs"),
            crate::plan::RuntimeTaskPlanSealLimits::default(),
        );
        (result, meter.totals())
    };
    let (first, (work, bytes)) = digest(100_000, 1_000_000);
    assert!(first.is_ok());
    assert_eq!(digest(work, bytes).0.unwrap(), first.unwrap());
    assert!(matches!(
        digest(work - 1, bytes).0,
        Err(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    assert!(matches!(
        digest(work, bytes - 1).0,
        Err(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::TranscriptBytes
        ))
    ));
}

fn make_need_request_plan(name: Option<&str>) -> crate::task::NeedProducerTaskPlan {
    use crate::task::*;
    NeedProducerTaskPlan::try_new(
        NeedProducerContractDigest::from_bytes([11; 32]),
        NeedProducerSiteDigest::from_bytes([12; 32]),
        NeedProducerRequestProjection::ExternCapability {
            capability: HostCapabilityId("test".into()),
            operation: "notify".into(),
            contract: crate::step::HostCallContractDigest::from_bytes([11; 32]),
            argument_names: Box::new([name.map(str::to_owned)]),
        },
        Box::new([RuntimeSemanticTypeId::from_bytes([61; 32])]),
        RuntimeSemanticTypeId::from_bytes([62; 32]),
        TaskPolicy::AlwaysStart,
        crate::task::HostRestartPolicy::Restartable,
        TaskClass::Io,
        TaskPriority(0),
        CancelScopeId("test".into()),
    )
    .unwrap()
}

#[test]
fn make_need_request_uses_actual_endpoint_and_keeps_checked_role_identity() {
    use crate::plan::*;
    use crate::task::{NamedHostArg, RuntimeRequestRoleIdentity};
    let make = |name: &str, identity| {
        try_producer_request_plan_with_setup(true, Some(make_need_request_plan(Some(name))), |_| {
            (
                vec![RuntimeHostArgumentSeed::Named(
                    RuntimeRequestRoleIdentity::from_accepted_identity([identity; 32]),
                    NamedHostArg {
                        name: name.into(),
                        value: RuntimeExprSeed::new(
                            RuntimeSemanticTypeId::from_bytes([61; 32]),
                            RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(true)),
                        ),
                    },
                )],
                Box::new([]),
            )
        })
        .unwrap()
    };
    let hash = |name: &str, identity| {
        let (plan, owner, function) = make(name, identity);
        let context = RuntimeBodySemanticContext::new(&plan);
        let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
        let producer = context
            .producer_function(
                &mut meter,
                function,
                &owner,
                &mut |_| Ok(owner.resolve(0).unwrap()),
                RuntimeTaskPlanSealLimits::default(),
            )
            .unwrap();
        let endpoint = producer.endpoint(0).unwrap();
        assert_eq!(endpoint.kind(), super::function::EndpointKind::MakeNeed);
        assert_eq!(
            endpoint.host_arguments().unwrap()[0].identity().as_bytes(),
            &[identity; 32]
        );
        context
            .host_request_template_digest(
                &mut meter,
                endpoint,
                RuntimeTaskPlanSealLimits::default(),
            )
            .unwrap()
    };
    assert_eq!(hash("field", 42), hash("renamed", 42));
    assert_ne!(hash("field", 42), hash("field", 43));
}

#[test]
fn make_need_builder_rejects_extra_arguments_before_zip_can_drop_them() {
    use crate::plan::*;
    let args = vec![
        RuntimeHostArgumentSeed::Positional(
            crate::task::RuntimeRequestRoleIdentity::from_accepted_identity([42; 32]),
            RuntimeExprSeed::new(
                RuntimeSemanticTypeId::from_bytes([61; 32]),
                RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(true))
            )
        );
        2
    ];
    assert!(matches!(
        try_producer_request_plan_with_setup(true, Some(make_need_request_plan(None)), |_| (
            args,
            Box::new([])
        )),
        Err(RuntimePlanBuildError::NeedProducerStartTarget(
            RuntimeNeedProducerStartTargetError::ArgumentCountMismatch
        ))
    ));
}

#[test]
fn make_need_target_rejects_binding_mismatch_and_spread() {
    use crate::task::{
        NamedHostArg, RuntimeHostArgumentTemplate as Argument, RuntimeRequestRoleIdentity,
    };
    let (plan, _, _) =
        try_producer_request_plan_with_setup(true, Some(make_need_request_plan(None)), |_| {
            (
                vec![crate::plan::RuntimeHostArgumentSeed::Positional(
                    RuntimeRequestRoleIdentity::from_accepted_identity([42; 32]),
                    crate::plan::RuntimeExprSeed::new(
                        RuntimeSemanticTypeId::from_bytes([61; 32]),
                        crate::plan::RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(
                            true,
                        )),
                    ),
                )],
                Box::new([]),
            )
        })
        .unwrap();
    let value = crate::value::RuntimeExpr::from_admitted_parts(
        plan.type_table()
            .id_for_semantic(RuntimeSemanticTypeId::from_bytes([61; 32]))
            .unwrap(),
        crate::value::RuntimeExprKind::Value(crate::value::RuntimeValue::Bool(true)),
    );
    let role = RuntimeRequestRoleIdentity::from_accepted_identity([42; 32]);
    for argument in [
        Argument::Positional(role, value.clone()),
        Argument::Named(
            role,
            NamedHostArg {
                name: "wrong".into(),
                value: value.clone(),
            },
        ),
        Argument::Spread(role, value),
    ] {
        assert!(matches!(
            crate::plan::RuntimeNeedProducerStartTarget::try_new(
                make_need_request_plan(Some("field")),
                vec![argument]
            ),
            Err(
                crate::plan::RuntimeNeedProducerStartTargetError::ArgumentBindingMismatch {
                    ordinal: 0
                }
            )
        ));
    }
}
