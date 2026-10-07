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
