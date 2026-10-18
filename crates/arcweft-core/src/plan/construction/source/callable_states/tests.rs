use super::super::pattern::tests::{declarations, frames, meter, resources};
use super::*;
use crate::pattern::RuntimeSemanticTypeId as TypeId;
use crate::plan::RuntimeCallArgumentSeed;
use crate::plan::{
    RuntimeCallableAttachedContract as Attached, RuntimeCallableDefault as Default,
    RuntimeCallableInputSource as Input, RuntimeCallableParameterCoordinate as Coordinate,
    RuntimeCallableParameterInput as Parameter, RuntimeCallableParameterKind as Kind,
    RuntimeCallablePosition as Position, RuntimeCallableRetainedInput as Retained,
    RuntimeCallableRetainedRole as Role, RuntimeCallableStateDefinition as State,
    RuntimeCallableTransition as Transition, RuntimeEffectSet, RuntimeExprSeed,
    RuntimeExprSeedKind as Expr, RuntimeFunctionDefinitionIdentity,
    RuntimeFunctionInputBindingSeed, RuntimeFunctionInputOrigin, RuntimeFunctionInputSource,
    RuntimeFunctionInputTransfer, RuntimeFunctionParameterIdentity,
    RuntimeFunctionParameterPassing, RuntimeFunctionSemanticRole, RuntimeFunctionSiteBodyKind,
    RuntimeFunctionSiteDeclarationSeed, RuntimeFunctionTypeContract,
    RuntimeLocalBindingDeclaration, RuntimeLocalBindingKind, RuntimeLocalBindingStorage,
    RuntimeLocalDeclarationSeed, RuntimeLocalDeclarationSource, RuntimePatternSeed,
    RuntimePatternSeedKind, RuntimePlan, RuntimePlanBuilder, RuntimePlanSequenceKind,
    RuntimePlanTypeProjection as P, RuntimePlanTypeSeed, RuntimeTaskPlanSealLimits,
    RuntimeTypeBinder, RuntimeTypeScope, RuntimeValueOwnershipRequirement,
};
use crate::value::{RuntimeCallArgumentMode, RuntimeValue};

const BOOL: TypeId = TypeId::from_bytes([121; 32]);
const UNARY: TypeId = TypeId::from_bytes([122; 32]);
const PAIR: TypeId = TypeId::from_bytes([123; 32]);
const FIRST: TypeId = TypeId::from_bytes([124; 32]);
const ZERO: TypeId = TypeId::from_bytes([125; 32]);
const PAYLOAD: TypeId = TypeId::from_bytes([126; 32]);
const OPTION: TypeId = TypeId::from_bytes([127; 32]);
const VECTOR: TypeId = TypeId::from_bytes([128; 32]);
const BOUND: TypeId = TypeId::from_bytes([129; 32]);
const GENERIC: TypeId = TypeId::from_bytes([130; 32]);

fn function_type(id: TypeId, parameters: Box<[TypeId]>, result: TypeId) -> RuntimePlanTypeSeed {
    RuntimePlanTypeSeed::new(
        id,
        P::Function {
            contract: RuntimeFunctionTypeContract::new(
                RuntimeTypeBinder::new(0, 0, 0),
                crate::effect_row::EffectPredicate::unconstrained(),
                crate::effect_row::EffectFormula::empty(),
            ),
            parameters,
            result,
        },
    )
}
fn reserve_function(
    builder: &mut RuntimePlanBuilder,
    index: u8,
    inputs: &[TypeId],
    captures: usize,
    result: TypeId,
) -> RuntimeFunctionSiteSeedId {
    let locals = inputs
        .iter()
        .enumerate()
        .map(|(position, &ty)| {
            let mut identity = [index; 32];
            identity[1] = position as u8;
            let source = if position < captures {
                RuntimeLocalDeclarationSource::Binding {
                    identity,
                    declaration: RuntimeLocalBindingDeclaration::new(
                        RuntimeLocalBindingKind::PatternBinding,
                        false,
                        RuntimeLocalBindingStorage::Derived,
                    ),
                }
            } else {
                RuntimeLocalDeclarationSource::Parameter(
                    RuntimeFunctionParameterIdentity::from_accepted_identity(identity),
                )
            };
            RuntimeLocalDeclarationSeed::new(source, ty)
        })
        .collect::<Vec<_>>();
    let admitted = builder.admit_type_batch([], locals.clone()).unwrap();
    let inputs = admitted
        .local_ids()
        .iter()
        .zip(locals)
        .enumerate()
        .map(
            |(position, (local, definition))| RuntimeFunctionInputBindingSeed {
                transfer: if position < captures {
                    RuntimeFunctionInputTransfer::ExternalBinding
                } else {
                    RuntimeFunctionInputTransfer::Formal
                },
                origin: match definition.source() {
                    RuntimeLocalDeclarationSource::Binding { identity, .. } => {
                        RuntimeFunctionInputOrigin::Binding(identity)
                    }
                    RuntimeLocalDeclarationSource::Parameter(parameter) => {
                        RuntimeFunctionInputOrigin::Parameter(parameter)
                    }
                    _ => unreachable!(),
                },
                source: if position < captures {
                    RuntimeFunctionInputSource::Capture {
                        position: position as u32,
                    }
                } else {
                    RuntimeFunctionInputSource::Parameter {
                        position: (position - captures) as u32,
                        passing: RuntimeFunctionParameterPassing::Value,
                    }
                },
                input_local: local.clone(),
                pattern: RuntimePatternSeed::new(
                    definition.ty(),
                    RuntimePatternSeedKind::Bind {
                        mutable: false,
                        local: local.clone(),
                    },
                ),
                ownership: RuntimeValueOwnershipRequirement::Owned,
                unrestricted_bindings: Box::new([]),
            },
        )
        .collect();
    builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([index; 32]),
            role: RuntimeFunctionSemanticRole::Ordinary,
            function_type: None,
            inputs,
            result,
            body_kind: RuntimeFunctionSiteBodyKind::Expression,
            effects: RuntimeEffectSet::empty(),
        })
        .unwrap()
}
fn define_bodies(
    builder: &mut RuntimePlanBuilder,
    functions: &[RuntimeFunctionSiteSeedId],
    states: &[RuntimeCallableStateSeedId],
) {
    for function in &functions[..7] {
        builder
            .define_function_site_seed(
                function,
                RuntimeExprSeed::new(BOOL, Expr::Value(RuntimeValue::Bool(true))),
            )
            .unwrap();
    }
    let callable = RuntimeExprSeed::new(
        PAIR,
        Expr::MakeCallable {
            state: states[0].clone(),
            captures: Box::new([]),
        },
    );
    builder
        .define_function_site_seed(
            &functions[7],
            RuntimeExprSeed::new(
                UNARY,
                Expr::Apply {
                    callee: Box::new(callable),
                    args: Box::new([RuntimeCallArgumentSeed::new(
                        RuntimeExprSeed::new(BOOL, Expr::Value(RuntimeValue::Bool(true))),
                        RuntimeCallArgumentMode::Value,
                        0,
                    )]),
                },
            ),
        )
        .unwrap();
}
fn parameter(
    group: u32,
    parameter: u32,
    kind: Kind,
    abi_ty: TypeId,
    binding_ty: TypeId,
) -> Parameter<TypeId> {
    Parameter {
        coordinate: Coordinate { group, parameter },
        kind,
        abi_ty,
        binding_ty,
    }
}
fn retained(role: Role) -> Retained<TypeId> {
    Retained { role, ty: BOOL }
}
fn fixture_builder() -> (
    RuntimePlanBuilder,
    [RuntimeFunctionSiteSeedId; 8],
    Vec<RuntimeCallableStateSeedId>,
) {
    let mut builder = RuntimePlanBuilder::new();
    let binder = RuntimeTypeBinder::new(1, 0, 0);
    let scope = RuntimeTypeScope::root().enter(binder).unwrap();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(BOOL, P::Bool),
                function_type(UNARY, Box::new([BOOL]), BOOL),
                function_type(PAIR, Box::new([BOOL, BOOL]), BOOL),
                function_type(FIRST, Box::new([BOOL]), UNARY),
                function_type(ZERO, Box::new([]), BOOL),
                RuntimePlanTypeSeed::new(PAYLOAD, P::Tuple(Box::new([BOOL]))),
                RuntimePlanTypeSeed::new(
                    OPTION,
                    P::Option {
                        item: BOOL,
                        some_payload: PAYLOAD,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    VECTOR,
                    P::Sequence {
                        kind: RuntimePlanSequenceKind::Vec,
                        item: BOOL,
                    },
                ),
                RuntimePlanTypeSeed::new(BOUND, P::BoundType(scope.bound_type(0, 0).unwrap()))
                    .with_scope(scope),
                RuntimePlanTypeSeed::new(
                    GENERIC,
                    P::Function {
                        contract: RuntimeFunctionTypeContract::new(
                            binder,
                            crate::effect_row::EffectPredicate::unconstrained(),
                            crate::effect_row::EffectFormula::empty(),
                        ),
                        parameters: Box::new([BOUND]),
                        result: BOOL,
                    },
                ),
            ],
            [],
        )
        .unwrap();
    let functions = [
        reserve_function(&mut builder, 141, &[BOOL, BOOL], 0, BOOL),
        reserve_function(&mut builder, 142, &[], 0, BOOL),
        reserve_function(&mut builder, 143, &[BOOL], 0, BOOL),
        reserve_function(&mut builder, 144, &[OPTION], 0, BOOL),
        reserve_function(&mut builder, 145, &[BOOL], 1, BOOL),
        reserve_function(&mut builder, 146, &[BOOL, BOOL], 1, BOOL),
        reserve_function(&mut builder, 147, &[VECTOR], 0, BOOL),
        reserve_function(&mut builder, 148, &[], 0, UNARY),
    ];
    let states = (0..10)
        .map(|_| builder.reserve_callable_state_seed().unwrap())
        .collect::<Vec<_>>();
    for index in 0..10 {
        let origin = if index == 1 {
            0
        } else if index == 3 {
            2
        } else {
            index
        };
        let (function_type, position, retained, parameters, result, attached, transition) =
            match index {
                0 => (
                    PAIR,
                    Position::Unapplied,
                    vec![],
                    vec![
                        parameter(0, 0, Kind::Fixed, BOOL, BOOL),
                        parameter(0, 1, Kind::Fixed, BOOL, BOOL),
                    ],
                    BOOL,
                    Attached::None,
                    Transition::Invoke {
                        function: functions[0].clone(),
                        captures: Box::new([]),
                        arguments: Box::new([
                            Input::Argument { position: 0 },
                            Input::Argument { position: 1 },
                        ]),
                    },
                ),
                1 => (
                    UNARY,
                    Position::WithinGroup {
                        group: 0,
                        bound: Box::new([Coordinate {
                            group: 0,
                            parameter: 0,
                        }]),
                    },
                    vec![retained(Role::Parameter(Coordinate {
                        group: 0,
                        parameter: 0,
                    }))],
                    vec![parameter(0, 1, Kind::Fixed, BOOL, BOOL)],
                    BOOL,
                    Attached::None,
                    Transition::Invoke {
                        function: functions[0].clone(),
                        captures: Box::new([]),
                        arguments: Box::new([
                            Input::Retained { position: 0 },
                            Input::Argument { position: 0 },
                        ]),
                    },
                ),
                2 => (
                    FIRST,
                    Position::Unapplied,
                    vec![],
                    vec![parameter(0, 0, Kind::Fixed, BOOL, BOOL)],
                    UNARY,
                    Attached::None,
                    Transition::Retain {
                        state: states[3].clone(),
                        values: Box::new([Input::Argument { position: 0 }]),
                    },
                ),
                3 => (
                    UNARY,
                    Position::AfterGroup { completed: 0 },
                    vec![retained(Role::Parameter(Coordinate {
                        group: 0,
                        parameter: 0,
                    }))],
                    vec![parameter(1, 0, Kind::Fixed, BOOL, BOOL)],
                    BOOL,
                    Attached::None,
                    Transition::Invoke {
                        function: functions[0].clone(),
                        captures: Box::new([]),
                        arguments: Box::new([
                            Input::Retained { position: 0 },
                            Input::Argument { position: 0 },
                        ]),
                    },
                ),
                4 => (
                    ZERO,
                    Position::Unapplied,
                    vec![],
                    vec![],
                    BOOL,
                    Attached::Required { ty: BOOL },
                    Transition::Invoke {
                        function: functions[2].clone(),
                        captures: Box::new([]),
                        arguments: Box::new([Input::Attached]),
                    },
                ),
                5 => (
                    ZERO,
                    Position::Unapplied,
                    vec![],
                    vec![],
                    BOOL,
                    Attached::Optional {
                        value: BOOL,
                        binding: OPTION,
                    },
                    Transition::Invoke {
                        function: functions[3].clone(),
                        captures: Box::new([]),
                        arguments: Box::new([Input::Attached]),
                    },
                ),
                6 => (
                    ZERO,
                    Position::Unapplied,
                    vec![],
                    vec![],
                    BOOL,
                    Attached::Defaulted {
                        ty: BOOL,
                        default: Default::Body {
                            function: functions[1].clone(),
                            captures: Box::new([]),
                        },
                    },
                    Transition::Invoke {
                        function: functions[2].clone(),
                        captures: Box::new([]),
                        arguments: Box::new([Input::Attached]),
                    },
                ),
                7 => (
                    ZERO,
                    Position::Unapplied,
                    vec![retained(Role::Capture { position: 0 })],
                    vec![],
                    BOOL,
                    Attached::Defaulted {
                        ty: BOOL,
                        default: Default::Body {
                            function: functions[4].clone(),
                            captures: Box::new([Input::Retained { position: 0 }]),
                        },
                    },
                    Transition::Invoke {
                        function: functions[5].clone(),
                        captures: Box::new([Input::Retained { position: 0 }]),
                        arguments: Box::new([Input::Attached]),
                    },
                ),
                8 => (
                    GENERIC,
                    Position::Unapplied,
                    vec![],
                    vec![parameter(0, 0, Kind::Fixed, BOUND, BOUND)],
                    BOOL,
                    Attached::Defaulted {
                        ty: BOOL,
                        default: Default::RequiresSpecialization,
                    },
                    Transition::RequiresSpecialization,
                ),
                9 => (
                    UNARY,
                    Position::Unapplied,
                    vec![],
                    vec![parameter(0, 0, Kind::Rest, BOOL, VECTOR)],
                    BOOL,
                    Attached::None,
                    Transition::Invoke {
                        function: functions[6].clone(),
                        captures: Box::new([]),
                        arguments: Box::new([Input::Argument { position: 0 }]),
                    },
                ),
                _ => unreachable!(),
            };
        builder
            .define_callable_state_seed(
                &states[index],
                State {
                    function_type,
                    origin: states[origin].clone(),
                    position,
                    retained: retained.into_boxed_slice(),
                    parameters: parameters.into_boxed_slice(),
                    result,
                    attached,
                    transition,
                    partials: Box::new([]),
                },
            )
            .unwrap();
    }
    (builder, functions, states)
}
fn fixture() -> RuntimePlan {
    let (mut builder, functions, states) = fixture_builder();
    define_bodies(&mut builder, &functions, &states);
    builder.finish().unwrap()
}
fn limits(plan: &RuntimePlan) -> RuntimeCallableSourceLimits {
    RuntimeCallableSourceLimits {
        rows: 100,
        elements: 100_000,
        encoded_bytes: 1 << 22,
        decoded_bytes: 1 << 24,
        type_count: plan.type_table().len(),
        function_count: plan.function_sites().len(),
    }
}
fn encode(plan: &RuntimePlan) -> Vec<u8> {
    let prepared = plan
        .callable_states()
        .prepare_sources_encoding(limits(plan), &mut meter())
        .unwrap();
    let mut bytes = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut bytes).unwrap();
    bytes
}
fn preparation<'a>(
    plan: &RuntimePlan,
    bytes: &'a [u8],
) -> (
    RuntimePlanSourceConstruction,
    Box<[RuntimeFunctionSiteSeedId]>,
    PreparedRuntimeCallableSources<'a>,
) {
    let declaration_bytes = frames(plan);
    let mut bank = meter();
    let declarations = declarations(&declaration_bytes, &mut bank);
    let prepared_functions = plan
        .function_sites()
        .prepare_declaration_sources_encoding(
            super::super::functions::RuntimeFunctionSourceLimits {
                rows: 100,
                elements: 100_000,
                pattern_nodes: 10_000,
                encoded_bytes: 1 << 22,
                decoded_bytes: 1 << 24,
                type_count: plan.type_table().len(),
                local_count: plan.local_declarations().len(),
            },
            &resources(),
            &mut meter(),
        )
        .unwrap();
    let mut function_bytes = Vec::with_capacity(prepared_functions.encoded_bytes());
    prepared_functions.append_to(&mut function_bytes).unwrap();
    let functions = crate::plan::RuntimeFunctionSiteTable::preflight_declaration_sources(
        &function_bytes,
        super::super::functions::RuntimeFunctionSourceLimits {
            rows: 100,
            elements: 100_000,
            pattern_nodes: 10_000,
            encoded_bytes: 1 << 22,
            decoded_bytes: 1 << 24,
            type_count: plan.type_table().len(),
            local_count: plan.local_declarations().len(),
        },
        &resources(),
        &mut bank,
    )
    .unwrap();
    let states =
        RuntimeCallableStateTable::preflight_sources(bytes, limits(plan), &mut bank).unwrap();
    let mut source = declarations.into_construction(bank).unwrap();
    let functions = functions.reserve(&mut source, &mut resources()).unwrap();
    (source, functions, states)
}
fn source(
    plan: &RuntimePlan,
    bytes: &[u8],
) -> (
    RuntimePlanSourceConstruction,
    Box<[RuntimeFunctionSiteSeedId]>,
    Box<[RuntimeCallableStateSeedId]>,
) {
    let (mut source, functions, prepared) = preparation(plan, bytes);
    let states = prepared.materialize(&mut source, &functions).unwrap();
    (source, functions, states)
}
fn finish(
    mut source: RuntimePlanSourceConstruction,
    functions: &[RuntimeFunctionSiteSeedId],
    states: &[RuntimeCallableStateSeedId],
) -> RuntimePlan {
    define_bodies(&mut source.builder, functions, states);
    let seal = RuntimeTaskPlanSealLimits::default();
    source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan()
}

#[test]
fn callable_definition_snapshots_share_metadata_across_slot_growth_and_release_with_the_last_owner()
{
    use std::sync::Arc;

    let (mut builder, _, _) = fixture_builder();
    let snapshot = builder.callable_states.borrow().states[0]
        .as_ref()
        .unwrap()
        .clone();
    assert_eq!(snapshot.parameters.len(), 2);
    let parameters = snapshot.parameters.as_ptr();
    let weak = Arc::downgrade(&snapshot);
    let aliases = (0..128).map(|_| Arc::clone(&snapshot)).collect::<Vec<_>>();
    let capacity = builder.callable_states.borrow().states.capacity();
    for _ in 0..=capacity {
        builder.reserve_callable_state_seed().unwrap();
    }
    let rows = builder.callable_states.borrow();
    assert!(rows.states.capacity() > capacity);
    assert!(Arc::ptr_eq(rows.states[0].as_ref().unwrap(), &snapshot));
    assert!(
        aliases.iter().all(|alias| {
            Arc::ptr_eq(alias, &snapshot) && alias.parameters.as_ptr() == parameters
        })
    );
    drop(rows);
    drop(builder);
    assert_eq!(snapshot.parameters[1].coordinate.parameter, 1);
    drop(snapshot);
    assert!(weak.upgrade().is_some());
    drop(aliases);
    assert!(weak.upgrade().is_none());
}

#[test]
fn callable_sources_preserve_the_complete_graph_defaults_forward_edges_partials_and_common_seal() {
    let plan = fixture();
    let bytes = encode(&plan);
    let prepared = plan
        .callable_states()
        .prepare_sources_encoding(limits(&plan), &mut meter())
        .unwrap();
    let decoded =
        RuntimeCallableStateTable::preflight_sources(&bytes, limits(&plan), &mut meter()).unwrap();
    assert_eq!(prepared.storage_bytes(), decoded.storage_bytes());
    assert_eq!(prepared.elements(), decoded.elements());
    assert_eq!(decoded.row_count(), 10);
    let (source, functions, states) = source(&plan, &bytes);
    let restored = finish(source, &functions, &states);
    assert_eq!(restored.callable_states(), plan.callable_states());
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    assert_eq!(encode(&restored), bytes);
}

#[test]
fn callable_source_exact_limits_and_output_capacity_cover_all_rows_and_lists() {
    let plan = fixture();
    let bytes = encode(&plan);
    let prepared =
        RuntimeCallableStateTable::preflight_sources(&bytes, limits(&plan), &mut meter()).unwrap();
    let exact = RuntimeCallableSourceLimits {
        rows: 10,
        elements: prepared.elements() as u32,
        encoded_bytes: bytes.len(),
        decoded_bytes: prepared.storage_bytes(),
        ..limits(&plan)
    };
    RuntimeCallableStateTable::preflight_sources(&bytes, exact, &mut meter()).unwrap();
    plan.callable_states()
        .prepare_sources_encoding(exact, &mut meter())
        .unwrap();
    for bounded in [
        RuntimeCallableSourceLimits { rows: 9, ..exact },
        RuntimeCallableSourceLimits {
            elements: exact.elements - 1,
            ..exact
        },
        RuntimeCallableSourceLimits {
            encoded_bytes: exact.encoded_bytes - 1,
            ..exact
        },
        RuntimeCallableSourceLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
    ] {
        let mut bank = meter();
        assert!(RuntimeCallableStateTable::preflight_sources(&bytes, bounded, &mut bank).is_err());
        assert!(bank.status().is_err());
    }
    let prepared = plan
        .callable_states()
        .prepare_sources_encoding(exact, &mut meter())
        .unwrap();
    let mut output = Vec::with_capacity(prepared.encoded_bytes() - 1);
    output.push(74);
    assert!(matches!(
        prepared.append_to(&mut output),
        Err(Error::OutputCapacity { .. })
    ));
    assert_eq!(output, [74]);
}

#[test]
fn callable_sources_reject_truncation_bad_domains_tags_counts_references_and_trailing_bytes() {
    let plan = fixture();
    let bytes = encode(&plan);
    let header = DOMAIN.len() + 5;
    let mut invalid = (0..bytes.len())
        .map(|end| bytes[..end].to_vec())
        .collect::<Vec<_>>();
    for (offset, value) in [
        (0, 0),
        (DOMAIN.len(), 2),
        (header, 255),
        (header + 4, 0),
        (header + 8, 255),
        (header + 12, 255),
    ] {
        let mut bad = bytes.clone();
        bad[offset] = value;
        invalid.push(bad);
    }
    let mut bad = bytes.clone();
    bad[DOMAIN.len() + 1..header].copy_from_slice(&u32::MAX.to_le_bytes());
    invalid.push(bad);
    let mut bad = bytes.clone();
    bad.push(0);
    invalid.push(bad);
    for bad in invalid {
        let mut bank = meter();
        assert!(
            RuntimeCallableStateTable::preflight_sources(&bad, limits(&plan), &mut bank).is_err()
        );
        assert!(matches!(
            RuntimeCallableStateTable::preflight_sources(&bytes, limits(&plan), &mut bank),
            Err(Error::Resource(_))
        ));
    }
}

#[test]
fn callable_source_data_cannot_publish_an_invalid_projection_or_invent_missing_partial_states() {
    let plan = fixture();
    let bytes = encode(&plan);
    let (mut source, functions, states) = source(&plan, &bytes);
    assert!(matches!(
        source.builder.reserve_callable_state_seed(),
        Err(RuntimePlanBuildError::CallableSourceGraphClosed)
    ));
    assert_eq!(source.builder.callable_states.borrow().states.len(), 10);
    // The declared prefix is reused by actual body lowering; no duplicate is synthesized.
    let restored = finish(source, &functions, &states);
    assert_eq!(restored.callable_states().len(), 10);
    let (source, _, _) = super::tests::source(&plan, &bytes);
    let pair = source.builder.types.id_for_semantic(PAIR).unwrap();
    let unary = source.builder.types.id_for_semantic(UNARY).unwrap();
    let other = source.builder.types.id_for_semantic(ZERO).unwrap();
    assert!(
        source
            .builder
            .callable_states
            .borrow_mut()
            .request_partial(pair, unary, 1)
            .is_ok()
    );
    assert!(matches!(
        source
            .builder
            .callable_states
            .borrow_mut()
            .request_partial(pair, other, 1),
        Err(RuntimePlanBuildError::MissingSourceCallablePartial { .. })
    ));
    // Whole-graph source validation still belongs to the actual common seal.
    let mut bad = bytes.clone();
    let header = DOMAIN.len() + 5;
    bad[header + 8..header + 12].copy_from_slice(&1u32.to_le_bytes());
    let (source, functions, states) = super::tests::source(&plan, &bad);
    let mut source = source;
    define_bodies(&mut source.builder, &functions, &states);
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}

#[test]
fn callable_sources_require_actual_namespace_issuers_before_reserving_slots_and_keep_work_failures_sticky()
 {
    let plan = fixture();
    let bytes = encode(&plan);
    let (mut first, _, prepared) = preparation(&plan, &bytes);
    let (_, foreign, _) = preparation(&plan, &bytes);
    assert!(matches!(
        prepared.materialize(&mut first, &foreign),
        Err(Error::Construction(
            RuntimePlanBuildError::ForeignFunctionSiteSeed
        ))
    ));
    assert_eq!(
        first.meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
    assert!(first.builder.callable_states.borrow().states.is_empty());
    let (mut source, functions, _) = preparation(&plan, &bytes);
    let bad = RuntimeCallableSourceLimits {
        type_count: plan.type_table().len() + 1,
        ..limits(&plan)
    };
    let prepared =
        RuntimeCallableStateTable::preflight_sources(&bytes, bad, &mut source.meter).unwrap();
    assert!(matches!(
        prepared.materialize(&mut source, &functions),
        Err(Error::Namespace { kind: "type", .. })
    ));
    assert!(source.builder.callable_states.borrow().states.is_empty());
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
    let empty = RuntimePlanBuilder::new().finish().unwrap();
    let bytes = encode(&empty);
    let exact = RuntimeCallableSourceLimits {
        rows: 0,
        elements: 0,
        encoded_bytes: bytes.len(),
        decoded_bytes: 0,
        ..limits(&empty)
    };
    RuntimeCallableStateTable::preflight_sources(&bytes, exact, &mut meter()).unwrap();
    assert!(
        empty
            .callable_states()
            .prepare_sources_encoding(
                RuntimeCallableSourceLimits {
                    encoded_bytes: bytes.len() - 1,
                    ..exact
                },
                &mut meter()
            )
            .is_err()
    );
    let mut bank = TaskSemanticMeter::new(0, 0);
    let first = RuntimeCallableStateTable::preflight_sources(&bytes, exact, &mut bank)
        .err()
        .unwrap();
    let second = RuntimeCallableStateTable::preflight_sources(&bytes, exact, &mut bank)
        .err()
        .unwrap();
    assert_eq!(first, second);
}
