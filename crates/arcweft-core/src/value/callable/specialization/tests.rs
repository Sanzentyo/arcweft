use std::sync::Arc;

use crate::effect_row::{EffectFormula, EffectPredicate};
use crate::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
use crate::plan::*;
use crate::runtime_id::{RuntimeCallableSpecializationId, RuntimeCallableStateId};
use crate::task::RuntimeProgramOwner;
use crate::value::{
    RuntimeCallableApplication, RuntimeCallableValue, RuntimeCallableValueError,
    RuntimeSignedIntWidth, RuntimeValue,
};

fn id(marker: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([marker; 32])
}

fn input(ty: RuntimeSemanticTypeId) -> RuntimeCallableParameterInput<RuntimeSemanticTypeId> {
    RuntimeCallableParameterInput {
        coordinate: RuntimeCallableParameterCoordinate {
            group: 0,
            parameter: 0,
        },
        kind: RuntimeCallableParameterKind::Fixed,
        abi_ty: ty,
        binding_ty: ty,
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the fixture admits one complete scheme and two independently executable specializations"
)]
fn plan() -> RuntimePlan {
    let integer = RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64).semantic_identity_digest();
    let boolean = RuntimeCheckedType::Bool.semantic_identity_digest();
    let string = RuntimeCheckedType::String.semantic_identity_digest();
    let binder = RuntimeTypeBinder::new(1, 0, 0);
    let scope = RuntimeTypeScope::root().enter(binder).unwrap();
    let mut builder = RuntimePlanBuilder::new();
    let seeds = vec![
        RuntimePlanTypeSeed::new(
            integer,
            RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64),
        ),
        RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
        RuntimePlanTypeSeed::new(string, RuntimePlanTypeProjection::String),
        RuntimePlanTypeSeed::new(
            id(1),
            RuntimePlanTypeProjection::BoundType(scope.bound_type(0, 0).unwrap()),
        )
        .with_scope(scope),
        RuntimePlanTypeSeed::new(
            id(2),
            RuntimePlanTypeProjection::Function {
                contract: RuntimeFunctionTypeContract::new(
                    binder,
                    EffectPredicate::unconstrained(),
                    EffectFormula::empty(),
                ),
                parameters: Box::new([id(1)]),
                result: integer,
            },
        ),
        RuntimePlanTypeSeed::new(
            id(3),
            RuntimePlanTypeProjection::Function {
                contract: RuntimeFunctionTypeContract::default(),
                parameters: Box::new([boolean]),
                result: integer,
            },
        ),
        RuntimePlanTypeSeed::new(
            id(4),
            RuntimePlanTypeProjection::Function {
                contract: RuntimeFunctionTypeContract::default(),
                parameters: Box::new([string]),
                result: integer,
            },
        ),
    ];
    let locals = builder
        .admit_type_batch(
            seeds,
            [integer, boolean, string].map(RuntimeLocalDeclarationSeed::new),
        )
        .unwrap();
    let source = builder.reserve_callable_state_seed().unwrap();
    let retained = Box::new([RuntimeCallableRetainedInput {
        role: RuntimeCallableRetainedRole::Capture { position: 0 },
        ty: integer,
    }]);
    builder
        .define_callable_state_seed(
            &source,
            RuntimeCallableStateDefinition {
                function_type: id(2),
                origin: source.clone(),
                position: RuntimeCallablePosition::Unapplied,
                retained: retained.clone(),
                parameters: Box::new([input(id(1))]),
                result: integer,
                attached: RuntimeCallableAttachedContract::None,
                transition: RuntimeCallableTransition::RequiresSpecialization,
                partials: Box::new([]),
            },
        )
        .unwrap();
    for (index, (argument, function_type)) in
        [(boolean, id(3)), (string, id(4))].into_iter().enumerate()
    {
        let site = builder
            .push_function_site_seed(
                [
                    RuntimeFunctionInputBindingSeed {
                        source: RuntimeFunctionInputSource::Capture { position: 0 },
                        input_local: locals.local_ids()[0].clone(),
                        pattern: RuntimePatternSeed::new(integer, RuntimePatternSeedKind::Discard),
                    },
                    RuntimeFunctionInputBindingSeed {
                        source: RuntimeFunctionInputSource::Parameter { position: 0 },
                        input_local: locals.local_ids()[index + 1].clone(),
                        pattern: RuntimePatternSeed::new(argument, RuntimePatternSeedKind::Discard),
                    },
                ],
                RuntimeExprSeed::new(
                    integer,
                    RuntimeExprSeedKind::Local(locals.local_ids()[0].clone()),
                ),
            )
            .unwrap();
        let target = builder.reserve_callable_state_seed().unwrap();
        builder
            .define_callable_state_seed(
                &target,
                RuntimeCallableStateDefinition {
                    function_type,
                    origin: source.clone(),
                    position: RuntimeCallablePosition::Unapplied,
                    retained: retained.clone(),
                    parameters: Box::new([input(argument)]),
                    result: integer,
                    attached: RuntimeCallableAttachedContract::None,
                    transition: RuntimeCallableTransition::Invoke {
                        function: site,
                        captures: Box::new([RuntimeCallableInputSource::Retained { position: 0 }]),
                        arguments: Box::new([RuntimeCallableInputSource::Argument { position: 0 }]),
                    },
                    partials: Box::new([]),
                },
            )
            .unwrap();
        builder
            .push_callable_specialization_seed(RuntimeCallableSpecializationDefinition {
                source_type: id(2),
                target_type: function_type,
                arguments: RuntimeFunctionSpecializationArguments {
                    types: Box::new([argument]),
                    const_lengths: Box::new([]),
                    effects: Box::new([]),
                },
                states: Box::new([RuntimeCallableSpecializationState {
                    source: source.clone(),
                    target,
                }]),
            })
            .unwrap();
    }
    builder.finish().unwrap()
}

#[test]
fn one_callable_can_be_specialized_repeatedly_without_replaying_its_captures() {
    let owner = RuntimeProgramOwner::Plan(Arc::new(plan()));
    let source = RuntimeCallableStateId::from_zero_based(0).unwrap();
    let value =
        RuntimeCallableValue::try_new(owner.clone(), source, [RuntimeValue::i64(42)]).unwrap();
    for (index, argument) in [
        RuntimeValue::Bool(true),
        RuntimeValue::String("text".to_owned()),
    ]
    .into_iter()
    .enumerate()
    {
        let specialization = RuntimeCallableSpecializationId::from_zero_based(index).unwrap();
        let specialized = value.clone().specialize(&owner, specialization).unwrap();
        assert_eq!(value.state(), source);
        assert_eq!(specialized.retained(), value.retained());
        assert_eq!(specialized.function_type().unwrap(), id(index as u8 + 3));
        let RuntimeCallableApplication::Invoke(invocation) = specialized
            .prepare_group(&[argument.clone()], None)
            .unwrap()
        else {
            panic!("specialized callable must select its admitted body");
        };
        assert_eq!(invocation.captures, [RuntimeValue::i64(42)]);
        assert_eq!(invocation.arguments, [argument]);
        assert!(matches!(
            specialized.specialize(&owner, specialization),
            Err(RuntimeCallableValueError::SpecializationSource { .. })
        ));
    }
}

#[test]
fn specialization_requires_the_exact_program_lease_and_an_admitted_relation() {
    let plan = Arc::new(plan());
    let owner = RuntimeProgramOwner::Plan(Arc::clone(&plan));
    let value = RuntimeCallableValue::try_new(
        owner.clone(),
        RuntimeCallableStateId::from_zero_based(0).unwrap(),
        [RuntimeValue::i64(7)],
    )
    .unwrap();
    let foreign = RuntimeProgramOwner::Plan(Arc::new((*plan).clone()));
    assert!(matches!(
        value.clone().specialize(
            &foreign,
            RuntimeCallableSpecializationId::from_zero_based(0).unwrap()
        ),
        Err(RuntimeCallableValueError::ForeignProgram)
    ));
    assert!(matches!(
        value.specialize(
            &owner,
            RuntimeCallableSpecializationId::from_zero_based(9).unwrap()
        ),
        Err(RuntimeCallableValueError::MissingSpecialization { .. })
    ));
}
