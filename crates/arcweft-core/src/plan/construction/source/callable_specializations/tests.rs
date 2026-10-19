use super::super::callable_states::tests as graph;
use super::super::pattern::tests::meter;
use super::*;

#[test]
fn original_expression_sources_replay_specialization_with_the_original_generic_frame_and_state_relations()
 {
    use super::super::expression::{RuntimeExpressionSourceHandles, tests::replay_functions};
    let plan = fixture();
    let bytes = encode(&plan);
    let (mut source, functions, states) = construction(&plan);
    let specializations = source
        .preflight_specialization_sources(&bytes, limits(&plan))
        .unwrap()
        .materialize(&mut source, &states)
        .unwrap();
    let handles = RuntimeExpressionSourceHandles {
        states: &states,
        specializations: &specializations,
        helpers: &[],
        methods: &[],
        attempts: &[],
    };
    replay_functions(&plan, &mut source, &functions, &handles);
    let seal = RuntimeTaskPlanSealLimits::default();
    let restored = source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan();
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(
        restored.callable_specializations(),
        plan.callable_specializations()
    );
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
}
use crate::effect_row::{DecisionControl, DecisionWork, EffectFormula, EffectPredicate};
use crate::pattern::RuntimeSemanticTypeId as Ty;
use crate::plan::{
    RuntimeArrayLength, RuntimeCallableAttachedContract, RuntimeCallableParameterCoordinate,
    RuntimeCallableParameterInput, RuntimeCallableParameterKind, RuntimeCallablePosition,
    RuntimeCallableSpecializationState, RuntimeCallableStateDefinition, RuntimeCallableTransition,
    RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFunctionSiteSeedId,
    RuntimeFunctionSpecializationArguments, RuntimePlanBuilder, RuntimePlanTypeProjection as P,
    RuntimePlanTypeSeed, RuntimeTaskPlanSealLimits, RuntimeTypeBinder, RuntimeTypeScope,
};

const SOURCE_T: Ty = Ty::from_bytes([201; 32]);
const SOURCE_ARRAY: Ty = Ty::from_bytes([202; 32]);
const SOURCE: Ty = Ty::from_bytes([203; 32]);
const TARGET_T: Ty = Ty::from_bytes([204; 32]);
const TARGET_ARRAY: Ty = Ty::from_bytes([205; 32]);
const TARGET: Ty = Ty::from_bytes([206; 32]);
struct Algebra;
impl DecisionControl for Algebra {
    type Error = std::convert::Infallible;
    fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
        Ok(())
    }
}
fn define_specialized_body(
    builder: &mut RuntimePlanBuilder,
    function: &RuntimeFunctionSiteSeedId,
    states: &[RuntimeCallableStateSeedId],
    specializations: &[RuntimeCallableSpecializationSeedId],
) {
    builder
        .define_function_site_seed(
            function,
            RuntimeExprSeed::new(
                TARGET,
                RuntimeExprSeedKind::SpecializeCallable {
                    value: Box::new(RuntimeExprSeed::new(
                        SOURCE,
                        RuntimeExprSeedKind::MakeCallable {
                            state: states[10].clone(),
                            captures: Box::new([]),
                        },
                    )),
                    specialization: specializations[1].clone(),
                },
            ),
        )
        .unwrap();
}
pub(in crate::plan::construction::source) fn fixture() -> RuntimePlan {
    let (mut builder, functions, mut states) = graph::fixture_builder();
    let source_binder = RuntimeTypeBinder::new(1, 1, 1);
    let target_binder = RuntimeTypeBinder::new(1, 0, 1);
    let source_scope = RuntimeTypeScope::root().enter(source_binder).unwrap();
    let target_scope = RuntimeTypeScope::root().enter(target_binder).unwrap();
    let source_effect =
        EffectFormula::variable(source_scope.bound_effect(0, 0).unwrap(), &mut Algebra).unwrap();
    let target_effect =
        EffectFormula::variable(target_scope.bound_effect(0, 0).unwrap(), &mut Algebra).unwrap();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(
                    SOURCE_T,
                    P::BoundType(source_scope.bound_type(0, 0).unwrap()),
                )
                .with_scope(source_scope.clone()),
                RuntimePlanTypeSeed::new(
                    SOURCE_ARRAY,
                    P::Array {
                        item: SOURCE_T,
                        length: RuntimeArrayLength::Bound(source_scope.bound_const(0, 0).unwrap()),
                    },
                )
                .with_scope(source_scope.clone()),
                RuntimePlanTypeSeed::new(
                    SOURCE,
                    P::Function {
                        contract: RuntimeFunctionTypeContract::new(
                            source_binder,
                            EffectPredicate::unconstrained(),
                            source_effect.clone(),
                        ),
                        parameters: Box::new([SOURCE_ARRAY]),
                        result: SOURCE_ARRAY,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    TARGET_T,
                    P::BoundType(target_scope.bound_type(0, 0).unwrap()),
                )
                .with_scope(target_scope.clone()),
                RuntimePlanTypeSeed::new(
                    TARGET_ARRAY,
                    P::Array {
                        item: TARGET_T,
                        length: RuntimeArrayLength::Constant(7),
                    },
                )
                .with_scope(target_scope),
                RuntimePlanTypeSeed::new(
                    TARGET,
                    P::Function {
                        contract: RuntimeFunctionTypeContract::new(
                            target_binder,
                            EffectPredicate::unconstrained(),
                            target_effect.clone(),
                        ),
                        parameters: Box::new([TARGET_ARRAY]),
                        result: TARGET_ARRAY,
                    },
                ),
            ],
            [],
        )
        .unwrap();
    states.push(builder.reserve_callable_state_seed().unwrap());
    states.push(builder.reserve_callable_state_seed().unwrap());
    for (index, function_type, array) in [(10, SOURCE, SOURCE_ARRAY), (11, TARGET, TARGET_ARRAY)] {
        builder
            .define_callable_state_seed(
                &states[index],
                RuntimeCallableStateDefinition {
                    function_type,
                    origin: states[10].clone(),
                    position: RuntimeCallablePosition::Unapplied,
                    retained: Box::new([]),
                    parameters: Box::new([RuntimeCallableParameterInput {
                        coordinate: RuntimeCallableParameterCoordinate {
                            group: 0,
                            parameter: 0,
                        },
                        kind: RuntimeCallableParameterKind::Fixed,
                        abi_ty: array,
                        binding_ty: array,
                    }]),
                    result: array,
                    attached: RuntimeCallableAttachedContract::None,
                    transition: RuntimeCallableTransition::RequiresSpecialization,
                    partials: Box::new([]),
                },
            )
            .unwrap();
    }
    let specializations = [
        builder
            .push_callable_specialization_seed(RuntimeCallableSpecializationDefinition {
                source_type: SOURCE,
                target_type: SOURCE,
                arguments: RuntimeFunctionSpecializationArguments {
                    types: Box::new([SOURCE_T]),
                    const_lengths: Box::new([RuntimeArrayLength::Bound(
                        source_scope.bound_const(0, 0).unwrap(),
                    )]),
                    effects: Box::new([source_effect]),
                },
                states: Box::new([RuntimeCallableSpecializationState {
                    source: states[10].clone(),
                    target: states[10].clone(),
                }]),
            })
            .unwrap(),
        builder
            .push_callable_specialization_seed(RuntimeCallableSpecializationDefinition {
                source_type: SOURCE,
                target_type: TARGET,
                arguments: RuntimeFunctionSpecializationArguments {
                    types: Box::new([TARGET_T]),
                    const_lengths: Box::new([RuntimeArrayLength::Constant(7)]),
                    effects: Box::new([target_effect]),
                },
                states: Box::new([RuntimeCallableSpecializationState {
                    source: states[10].clone(),
                    target: states[11].clone(),
                }]),
            })
            .unwrap(),
    ];
    let specialized = graph::reserve_function(&mut builder, 211, &[], 0, TARGET);
    graph::define_bodies(&mut builder, &functions, &states);
    define_specialized_body(&mut builder, &specialized, &states, &specializations);
    builder.finish().unwrap()
}
fn limits(plan: &RuntimePlan) -> RuntimeSpecializationSourceLimits {
    RuntimeSpecializationSourceLimits {
        rows: 100,
        elements: 100_000,
        encoded_bytes: 1 << 22,
        decoded_bytes: 1 << 24,
        type_count: plan.type_table().len(),
        state_count: plan.callable_states().len(),
    }
}
fn encode(plan: &RuntimePlan) -> Vec<u8> {
    let prepared = plan
        .prepare_specialization_sources_encoding(limits(plan), &mut meter())
        .unwrap();
    let mut bytes = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut bytes).unwrap();
    bytes
}
fn construction(
    plan: &RuntimePlan,
) -> (
    RuntimePlanSourceConstruction,
    Box<[RuntimeFunctionSiteSeedId]>,
    Box<[RuntimeCallableStateSeedId]>,
) {
    graph::source(plan, &graph::encode(plan))
}
fn define_bodies(
    source: &mut RuntimePlanSourceConstruction,
    functions: &[RuntimeFunctionSiteSeedId],
    states: &[RuntimeCallableStateSeedId],
    specializations: &[RuntimeCallableSpecializationSeedId],
) {
    graph::define_bodies(&mut source.builder, functions, states);
    define_specialized_body(&mut source.builder, &functions[8], states, specializations);
}

#[test]
fn specialization_sources_preserve_types_constant_and_bound_lengths_effects_states_and_actual_body_handles()
 {
    let plan = fixture();
    let bytes = encode(&plan);
    let encoding = plan
        .prepare_specialization_sources_encoding(limits(&plan), &mut meter())
        .unwrap();
    let (mut source, functions, states) = construction(&plan);
    let prepared = source
        .preflight_specialization_sources(&bytes, limits(&plan))
        .unwrap();
    assert_eq!(prepared.storage_bytes(), encoding.storage_bytes());
    assert_eq!(prepared.elements(), encoding.elements());
    let handles = prepared.materialize(&mut source, &states).unwrap();
    assert_eq!(handles.len(), 2);
    define_bodies(&mut source, &functions, &states, &handles);
    let seal = RuntimeTaskPlanSealLimits::default();
    let restored = source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan();
    assert_eq!(
        restored.callable_specializations(),
        plan.callable_specializations()
    );
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    assert_eq!(encode(&restored), bytes);
}

#[test]
fn specialization_source_combined_limits_and_prefix_capacity_are_exact() {
    let plan = fixture();
    let bytes = encode(&plan);
    let (mut source, _, states) = construction(&plan);
    let prepared = source
        .preflight_specialization_sources(&bytes, limits(&plan))
        .unwrap();
    let exact = RuntimeSpecializationSourceLimits {
        rows: 2,
        elements: prepared.elements() as u32,
        encoded_bytes: bytes.len(),
        decoded_bytes: prepared.storage_bytes(),
        ..limits(&plan)
    };
    source
        .preflight_specialization_sources(&bytes, exact)
        .unwrap()
        .materialize(&mut source, &states)
        .unwrap();
    plan.prepare_specialization_sources_encoding(exact, &mut meter())
        .unwrap();
    for short in [
        RuntimeSpecializationSourceLimits { rows: 1, ..exact },
        RuntimeSpecializationSourceLimits {
            elements: exact.elements - 1,
            ..exact
        },
        RuntimeSpecializationSourceLimits {
            encoded_bytes: exact.encoded_bytes - 1,
            ..exact
        },
        RuntimeSpecializationSourceLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
    ] {
        let (mut source, _, _) = construction(&plan);
        assert!(
            source
                .preflight_specialization_sources(&bytes, short)
                .is_err()
        );
        assert!(source.meter.status().is_err());
        assert!(source.builder.callable_specializations.is_empty());
        assert!(
            plan.prepare_specialization_sources_encoding(short, &mut meter())
                .is_err()
        );
    }
    let mut output = Vec::with_capacity(bytes.len() - 1);
    output.push(71);
    assert!(matches!(
        plan.prepare_specialization_sources_encoding(exact, &mut meter())
            .unwrap()
            .append_to(&mut output),
        Err(Error::OutputCapacity { .. })
    ));
    assert_eq!(output, [71]);
}

#[test]
fn specialization_sources_reject_truncation_tags_ordinals_namespaces_refs_and_foreign_state_issuers()
 {
    let plan = fixture();
    let bytes = encode(&plan);
    let header = DOMAIN.len() + 5;
    for end in 0..bytes.len() {
        let (mut source, _, _) = construction(&plan);
        assert!(
            source
                .preflight_specialization_sources(&bytes[..end], limits(&plan))
                .is_err(),
            "{end}"
        );
        assert!(source.builder.callable_specializations.is_empty());
    }
    for (offset, value) in [
        (0, 0),
        (DOMAIN.len(), 2),
        (header, 255),
        (header + 4, 0),
        (header + 24, 255),
    ] {
        let mut bad = bytes.clone();
        bad[offset] = value;
        let (mut source, _, _) = construction(&plan);
        assert!(
            source
                .preflight_specialization_sources(&bad, limits(&plan))
                .is_err(),
            "{offset}"
        );
    }
    let (mut source, _, states) = construction(&plan);
    let prepared = source
        .preflight_specialization_sources(&bytes, limits(&plan))
        .unwrap();
    let (_, _, foreign) = construction(&plan);
    assert!(matches!(
        prepared.materialize(&mut source, &foreign),
        Err(Error::Construction(
            RuntimePlanBuildError::ForeignCallableStateSeed
        ))
    ));
    assert!(source.builder.callable_specializations.is_empty());
    assert_eq!(
        source.meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
    let (mut source, _, _) = construction(&plan);
    assert!(matches!(
        source.preflight_specialization_sources(
            &bytes,
            RuntimeSpecializationSourceLimits {
                state_count: states.len() + 1,
                ..limits(&plan)
            }
        ),
        Err(Error::Namespace { .. })
    ));
    let mut bad = bytes;
    bad.push(0);
    let (mut source, _, _) = construction(&plan);
    assert!(matches!(
        source.preflight_specialization_sources(&bad, limits(&plan)),
        Err(Error::Trailing)
    ));
}

#[test]
fn late_effect_owner_failure_poisons_the_original_context_after_an_earlier_real_row() {
    let plan = fixture();
    let mut bytes = encode(&plan);
    let row = &plan.callable_specializations()[1];
    let (target, scope) = Types::Program(plan.type_table())
        .target(row.target_type)
        .unwrap();
    let prepared = target
        .prepare_argument_source_in_frame(
            &row.arguments.effects[0],
            scope,
            RuntimeFunctionTypeSourceLimits {
                encoded_bytes: 1000,
                decoded_bytes: 10000,
                decision_nodes: 100,
                overrides: 100,
            },
        )
        .unwrap();
    let mut frame = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut frame).unwrap();
    let offsets = bytes
        .windows(frame.len())
        .enumerate()
        .filter_map(|(index, part)| (part == frame).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(offsets.len(), 2);
    let offset = offsets[1];
    let infinite = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0];
    bytes[offset - 4..offset].copy_from_slice(&(infinite.len() as u32).to_le_bytes());
    bytes.splice(offset..offset + frame.len(), infinite);
    let (mut source, _, states) = construction(&plan);
    let prepared = source
        .preflight_specialization_sources(&bytes, limits(&plan))
        .unwrap();
    assert!(matches!(
        prepared.materialize(&mut source, &states),
        Err(Error::Effect(_))
    ));
    assert_eq!(source.builder.callable_specializations.len(), 1);
    assert_eq!(
        source.meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}

#[test]
fn specialization_source_reference_bounds_and_state_correspondence_stay_on_the_actual_owners() {
    let plan = fixture();
    let bytes = encode(&plan);
    let mut outside = bytes.clone();
    let end = outside.len();
    outside[end - 4..].copy_from_slice(&u32::MAX.to_le_bytes());
    let (mut source, _, _) = construction(&plan);
    assert!(matches!(
        source.preflight_specialization_sources(&outside, limits(&plan)),
        Err(Error::Reference { kind: "state", .. })
    ));
    assert!(source.builder.callable_specializations.is_empty());
    let mut unrelated = bytes;
    unrelated[end - 4..].copy_from_slice(&0u32.to_le_bytes());
    let (mut source, functions, states) = construction(&plan);
    let prepared = source
        .preflight_specialization_sources(&unrelated, limits(&plan))
        .unwrap();
    let handles = prepared.materialize(&mut source, &states).unwrap();
    // All references are real and in bounds. Only the common relation owner
    // can reject a state from an unrelated callable origin/type.
    define_bodies(&mut source, &functions, &states, &handles);
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}

#[test]
fn specialization_empty_source_and_original_work_failure_do_not_allocate_relations_or_replace_the_bank()
 {
    let empty = RuntimePlanBuilder::new().finish().unwrap();
    let bytes = encode(&empty);
    let exact = RuntimeSpecializationSourceLimits {
        rows: 0,
        elements: 0,
        encoded_bytes: bytes.len(),
        decoded_bytes: 0,
        ..limits(&empty)
    };
    let (mut source, _, states) = construction(&empty);
    let prepared = source
        .preflight_specialization_sources(&bytes, exact)
        .unwrap();
    assert_eq!(prepared.storage_bytes(), 0);
    assert_eq!(prepared.elements(), 0);
    assert!(
        prepared
            .materialize(&mut source, &states)
            .unwrap()
            .is_empty()
    );
    let seal = RuntimeTaskPlanSealLimits::default();
    assert_eq!(
        source
            .prepare_image(seal)
            .unwrap()
            .seal(seal, None)
            .unwrap()
            .into_plan()
            .executable_semantic_digest(),
        empty.executable_semantic_digest()
    );
    assert!(
        empty
            .prepare_specialization_sources_encoding(
                RuntimeSpecializationSourceLimits {
                    encoded_bytes: bytes.len() - 1,
                    ..exact
                },
                &mut meter()
            )
            .is_err()
    );
    let plan = fixture();
    let bytes = encode(&plan);
    let (source, _, _) = construction(&plan);
    let (prelude_work, _) = source.meter.totals();
    let (mut source, _, _) = graph::source_with_meter(
        &plan,
        &graph::encode(&plan),
        TaskSemanticMeter::new(prelude_work + 3 * bytes.len() as u64 - 1, u64::MAX),
    );
    assert!(source.meter.status().is_ok());
    let first = source
        .preflight_specialization_sources(&bytes, limits(&plan))
        .err()
        .unwrap();
    assert_eq!(
        first,
        Error::Resource(TaskSemanticEncodingError::SemanticWork)
    );
    assert_eq!(
        source
            .preflight_specialization_sources(&[], limits(&plan))
            .err()
            .unwrap(),
        first
    );
    assert!(source.builder.callable_specializations.is_empty());
}
