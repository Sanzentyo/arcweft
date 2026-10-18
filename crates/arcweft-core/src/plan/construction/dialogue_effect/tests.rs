use crate::entry::RuntimeDialogueContentTemplateDigest;
use crate::pattern::{RuntimeOpaqueTypeAdmission, RuntimeSemanticTypeId as Ty};
use crate::plan::*;
use crate::runtime_id::*;
use crate::task::RuntimeProgramOwner;
use crate::value::{
    RuntimeCallableValue, RuntimeDialogueOpaqueRole, RuntimeOpaquePersistence,
    RuntimeOpaqueValueClass, RuntimeValue,
};
use std::sync::Arc;

const UNIT: Ty = Ty::from_bytes([231; 32]);
const BOOL: Ty = Ty::from_bytes([232; 32]);
const CALLBACK: Ty = Ty::from_bytes([233; 32]);
const FIRST: Ty = Ty::from_bytes([234; 32]);

fn manifest() -> RuntimeDialogueContentTemplateManifestSeed {
    RuntimeDialogueContentTemplateManifestSeed {
        id: RuntimeDialogueContentTemplateId::from_zero_based(0).unwrap(),
        digest: RuntimeDialogueContentTemplateDigest::from_bytes([235; 32]),
        slots: Box::new([]),
        effects: Box::new([RuntimeDialogueContentEffectSlotSeed {
            site: RuntimeDialogueEffectSiteId::from_zero_based(0).unwrap(),
            trigger: RuntimeDialogueContentEffectTrigger::Content,
            capture_types: Box::new([BOOL]),
        }]),
    }
}
fn executable() -> RuntimeFunctionSiteBodySeed {
    RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
        effects: RuntimeEffectSet::empty(),
        ops: Box::new([]),
    })
}
fn content(state: RuntimeCallableStateSeedId) -> RuntimeExprSeed {
    RuntimeExprSeed::dialogue_content_with_effects(
        RuntimeDialogueOpaqueRole::Content.semantic_identity(),
        RuntimeDialogueContentTemplateId::from_zero_based(0).unwrap(),
        [],
        [RuntimeDialogueEffectSiteSeed {
            site: RuntimeDialogueEffectSiteId::from_zero_based(0).unwrap(),
            state,
            captures: Box::new([RuntimeExprSeed::new(
                BOOL,
                RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
            )]),
        }],
    )
}
fn fixture_builder() -> (
    RuntimePlanBuilder,
    [RuntimeFunctionSiteSeedId; 2],
    [RuntimeCallableStateSeedId; 2],
) {
    let mut builder = RuntimePlanBuilder::new();
    let parameter = RuntimeFunctionParameterIdentity::from_accepted_identity([236; 32]);
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(UNIT, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(BOOL, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    CALLBACK,
                    RuntimePlanTypeProjection::Function {
                        contract: Default::default(),
                        parameters: Box::new([]),
                        result: UNIT,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    FIRST,
                    RuntimePlanTypeProjection::Function {
                        contract: Default::default(),
                        parameters: Box::new([BOOL]),
                        result: CALLBACK,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    RuntimeDialogueOpaqueRole::Content.semantic_identity(),
                    RuntimePlanTypeProjection::Opaque {
                        producer: RuntimeDialogueOpaqueRole::Content.producer(),
                        admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                        value_class: RuntimeOpaqueValueClass::Plain,
                        persistence: RuntimeOpaquePersistence::SnapshotOnly,
                        arguments: Box::new([]),
                    },
                ),
            ],
            [RuntimeLocalDeclarationSeed::new(
                RuntimeLocalDeclarationSource::Parameter(parameter),
                BOOL,
            )],
        )
        .unwrap();
    let local = admitted.local_ids()[0].clone();
    let body = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([237; 32]),
            role: RuntimeFunctionSemanticRole::Ordinary,
            function_type: None,
            inputs: Box::new([RuntimeFunctionInputBindingSeed {
                transfer: RuntimeFunctionInputTransfer::Formal,
                origin: RuntimeFunctionInputOrigin::Parameter(parameter),
                source: RuntimeFunctionInputSource::Parameter {
                    position: 0,
                    passing: RuntimeFunctionParameterPassing::Value,
                },
                input_local: local.clone(),
                pattern: RuntimePatternSeed::new(
                    BOOL,
                    RuntimePatternSeedKind::Bind {
                        mutable: false,
                        local,
                    },
                ),
                ownership: RuntimeValueOwnershipRequirement::Owned,
                unrestricted_bindings: Box::new([]),
            }]),
            result: UNIT,
            body_kind: RuntimeFunctionSiteBodyKind::Executable,
            effects: RuntimeEffectSet::empty(),
        })
        .unwrap();
    let factory = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([238; 32]),
            role: RuntimeFunctionSemanticRole::Ordinary,
            function_type: None,
            inputs: Box::new([]),
            result: RuntimeDialogueOpaqueRole::Content.semantic_identity(),
            body_kind: RuntimeFunctionSiteBodyKind::Expression,
            effects: RuntimeEffectSet::empty(),
        })
        .unwrap();
    let origin = builder.reserve_callable_state_seed().unwrap();
    let callback = builder.reserve_callable_state_seed().unwrap();
    let coordinate = RuntimeCallableParameterCoordinate {
        group: 0,
        parameter: 0,
    };
    builder
        .define_callable_state_seed(
            &origin,
            RuntimeCallableStateDefinition {
                function_type: FIRST,
                origin: origin.clone(),
                position: RuntimeCallablePosition::Unapplied,
                retained: Box::new([]),
                parameters: Box::new([RuntimeCallableParameterInput {
                    coordinate,
                    kind: RuntimeCallableParameterKind::Fixed,
                    abi_ty: BOOL,
                    binding_ty: BOOL,
                }]),
                result: CALLBACK,
                attached: RuntimeCallableAttachedContract::None,
                transition: RuntimeCallableTransition::Retain {
                    state: callback.clone(),
                    values: Box::new([RuntimeCallableInputSource::Argument { position: 0 }]),
                },
                partials: Box::new([]),
            },
        )
        .unwrap();
    builder
        .define_callable_state_seed(
            &callback,
            RuntimeCallableStateDefinition {
                function_type: CALLBACK,
                origin: origin.clone(),
                position: RuntimeCallablePosition::AfterGroup { completed: 0 },
                retained: Box::new([RuntimeCallableRetainedInput {
                    role: RuntimeCallableRetainedRole::Parameter(coordinate),
                    ty: BOOL,
                }]),
                parameters: Box::new([]),
                result: UNIT,
                attached: RuntimeCallableAttachedContract::None,
                transition: RuntimeCallableTransition::Invoke {
                    function: body.clone(),
                    captures: Box::new([]),
                    arguments: Box::new([RuntimeCallableInputSource::Retained { position: 0 }]),
                },
                partials: Box::new([]),
            },
        )
        .unwrap();
    builder
        .register_dialogue_content_template_seed(manifest())
        .unwrap();
    (builder, [body, factory], [origin, callback])
}
fn fixture() -> RuntimePlan {
    let (mut builder, functions, states) = fixture_builder();
    builder
        .define_function_site_seed(&functions[0], executable())
        .unwrap();
    builder
        .define_function_site_seed(&functions[1], content(states[1].clone()))
        .unwrap();
    builder.finish().unwrap()
}

#[test]
fn content_effect_source_reuses_original_state_with_retained_parameter_and_body_argument() {
    use super::super::source::callable_states::tests as graph;
    let plan = fixture();
    let (mut source, functions, states) = graph::source(&plan, &graph::encode(&plan));
    source
        .builder
        .register_dialogue_content_template_seed(manifest())
        .unwrap();
    source
        .builder
        .define_function_site_seed(&functions[0], executable())
        .unwrap();
    source
        .builder
        .define_function_site_seed(&functions[1], content(states[1].clone()))
        .unwrap();
    let seal = RuntimeTaskPlanSealLimits::default();
    let restored = source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan();
    assert_eq!(restored.callable_states(), plan.callable_states());
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    let plan = Arc::new(restored);
    let callback = RuntimeCallableValue::try_new(
        RuntimeProgramOwner::Plan(Arc::clone(&plan)),
        RuntimeCallableStateId::for_index(1).unwrap(),
        vec![RuntimeValue::Bool(true)],
    )
    .unwrap();
    assert!(callback.is_structured_executable_callback());
    let (proof, captures, arguments) = callback.inspect_zero_arg_plan_inputs().unwrap();
    assert!(captures.is_empty());
    assert_eq!(arguments, [&RuntimeValue::Bool(true)]);
    let invocation = callback.commit_zero_arg_invocation(proof);
    assert!(invocation.captures.is_empty());
    assert_eq!(invocation.arguments, [RuntimeValue::Bool(true)]);
}

#[test]
fn content_effect_binding_rejects_foreign_states_and_bad_retained_values_before_publication() {
    let (mut builder, functions, _) = fixture_builder();
    let (_, _, foreign) = fixture_builder();
    assert!(matches!(
        builder.define_function_site_seed(&functions[1], content(foreign[1].clone())),
        Err(RuntimePlanBuildError::ForeignCallableStateSeed)
    ));
    assert!(builder.finish().is_err());
    let (mut builder, functions, states) = fixture_builder();
    let bad = RuntimeExprSeed::dialogue_content_with_effects(
        RuntimeDialogueOpaqueRole::Content.semantic_identity(),
        RuntimeDialogueContentTemplateId::from_zero_based(0).unwrap(),
        [],
        [RuntimeDialogueEffectSiteSeed {
            site: RuntimeDialogueEffectSiteId::from_zero_based(0).unwrap(),
            state: states[1].clone(),
            captures: Box::new([]),
        }],
    );
    assert!(matches!(
        builder.define_function_site_seed(&functions[1], bad),
        Err(RuntimePlanBuildError::DialogueEffectCaptureCountMismatch { .. })
    ));
    assert!(builder.finish().is_err());
}

#[test]
fn function_state_authoring_factory_checks_actual_arrow_and_issuer_without_duplicating_states() {
    let (mut builder, functions, _) = fixture_builder();
    assert!(matches!(
        builder.function_callable_state_seed(CALLBACK, &functions[0]),
        Err(RuntimePlanBuildError::InvalidTypeProjection { .. })
    ));
    let (_, foreign, _) = fixture_builder();
    assert!(matches!(
        builder.function_callable_state_seed(CALLBACK, &foreign[0]),
        Err(RuntimePlanBuildError::ForeignFunctionSiteSeed)
    ));
    let flat = Ty::from_bytes([240; 32]);
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                flat,
                RuntimePlanTypeProjection::Function {
                    contract: Default::default(),
                    parameters: Box::new([BOOL]),
                    result: UNIT,
                },
            )],
            [],
        )
        .unwrap();
    let first = builder
        .function_callable_state_seed(flat, &functions[0])
        .unwrap();
    let second = builder
        .function_callable_state_seed(flat, &functions[0])
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(builder.callable_states.borrow().states.len(), 3);
}
