use super::*;
use crate::{
    entry::RuntimeCallableId,
    plan::{
        RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFunctionInputBindingSeed,
        RuntimeFunctionInputOwnershipRequirement, RuntimeFunctionInputSource,
        RuntimeLocalDeclarationSeed, RuntimePatternSeed, RuntimePatternSeedKind,
        RuntimePlanBuilder, RuntimePlanTypeSeed, RuntimePureProgramBindingSeed,
    },
};

fn fixture() -> (Arc<RuntimePlan>, RuntimePureProgramId, RuntimeCallTarget) {
    let semantic = RuntimeSemanticTypeId::from_bytes([37; 32]);
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                semantic,
                RuntimePlanTypeProjection::Bool,
            )],
            [],
        )
        .unwrap();
    let target = RuntimeCallTarget::callable(RuntimeCallableId::from_checked_digest([44; 32]));
    let site = builder
        .push_function_site_seed(
            crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([41; 32]),
            crate::plan::RuntimeFunctionSemanticRole::Ordinary,
            [],
            RuntimeExprSeed::new(
                semantic,
                RuntimeExprSeedKind::Call {
                    callee: target.clone(),
                    args: Box::new([]),
                },
            ),
        )
        .unwrap();
    let program = RuntimePureProgramId::from_checked_digest([38; 32]);
    builder
        .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed { program, site })
        .unwrap();
    (Arc::new(builder.finish().unwrap()), program, target)
}

struct External {
    plan: Arc<RuntimePlan>,
    target: RuntimeCallTarget,
    value: RuntimeValue,
    calls: usize,
}
impl RuntimeExternalCallBackend for External {
    fn call_external(
        &mut self,
        context: &RuntimeExternalCallContext,
        target: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        assert_eq!(target, &self.target);
        assert!(args.is_empty());
        let RuntimeProgramOwner::Plan(plan) = context.program_owner().unwrap() else {
            panic!("selected Plan")
        };
        assert!(Arc::ptr_eq(plan, &self.plan));
        assert_eq!(
            context.result_type(),
            Some(RuntimeSemanticTypeId::from_bytes([37; 32]))
        );
        self.calls += 1;
        Some(Ok(self.value.clone()))
    }
}

#[test]
fn pure_program_external_calls_keep_selected_owner_and_validate_results() {
    let (plan, program, target) = fixture();
    let mut absent = VmRuntimePureCallBackend::default();
    assert!(evaluate_pure_program_with_backend(&plan, program, &[], &mut absent).is_err());
    let mut backend = absent.with_external_calls(External {
        plan: Arc::clone(&plan),
        target,
        value: RuntimeValue::Bool(true),
        calls: 0,
    });
    assert_eq!(
        evaluate_pure_program_with_backend(&plan, program, &[], &mut backend).unwrap(),
        RuntimeValue::Bool(true)
    );
    assert_eq!(backend.external.calls, 1);
    backend.external.value = RuntimeValue::String("wrong result".to_owned());
    assert!(evaluate_pure_program_with_backend(&plan, program, &[], &mut backend).is_err());
    assert!(
        evaluate_pure_program_with_backend(
            &plan,
            RuntimePureProgramId::from_checked_digest([0; 32]),
            &[],
            &mut backend
        )
        .is_err()
    );
    assert!(
        evaluate_pure_program_with_backend(
            &plan,
            program,
            &[RuntimeValue::Bool(false)],
            &mut backend
        )
        .is_err()
    );
    assert_eq!(
        backend.external.calls, 2,
        "bad bindings and arity do not reach the external implementation"
    );
}

#[test]
fn borrowed_program_inputs_reject_nested_affine_values_before_execution() {
    use crate::pattern::{RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId};
    use crate::task::NeedId;

    let boolean = RuntimeSemanticTypeId::from_bytes([37; 32]);
    let owner = RuntimeOpaqueTypeOwner::exact(
        RuntimeOpaqueTypeProducerId::try_new("fixture.program-input").unwrap(),
        RuntimeSemanticTypeId::from_bytes([39; 32]),
    );
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    owner.semantic_identity(),
                    RuntimePlanTypeProjection::Opaque {
                        producer: owner.producer().clone(),
                        admission: owner.admission(),
                        value_class: owner.value_class(),
                        persistence: owner.persistence(),
                        arguments: Box::new([]),
                    },
                ),
            ],
            [
                RuntimeLocalDeclarationSeed::new(boolean),
                RuntimeLocalDeclarationSeed::new(owner.semantic_identity()),
            ],
        )
        .unwrap();
    let target = RuntimeCallTarget::callable(RuntimeCallableId::from_checked_digest([44; 32]));
    let inputs = admission
        .local_ids()
        .iter()
        .cloned()
        .zip([boolean, owner.semantic_identity()])
        .enumerate()
        .map(|(position, (local, ty))| RuntimeFunctionInputBindingSeed {
            transfer: crate::plan::RuntimeFunctionInputTransfer::ExternalBinding,
            origin: crate::plan::RuntimeFunctionInputOrigin::Binding([81; 32]),
            source: RuntimeFunctionInputSource::Capture {
                position: position as u32,
            },
            input_local: local.clone(),
            pattern: RuntimePatternSeed::new(
                ty,
                RuntimePatternSeedKind::Bind {
                    mutable: false,
                    local,
                },
            ),
            ownership: RuntimeFunctionInputOwnershipRequirement::Owned,
            unrestricted_bindings: Box::new([]),
        });
    let site = builder
        .push_function_site_seed(
            crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([41; 32]),
            crate::plan::RuntimeFunctionSemanticRole::Ordinary,
            inputs,
            RuntimeExprSeed::new(
                boolean,
                RuntimeExprSeedKind::Call {
                    callee: target.clone(),
                    args: Box::new([]),
                },
            ),
        )
        .unwrap();
    let program = RuntimePureProgramId::from_checked_digest([40; 32]);
    builder
        .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed { program, site })
        .unwrap();
    let plan = Arc::new(builder.finish().unwrap());
    let mut backend = VmRuntimePureCallBackend::default().with_external_calls(External {
        plan: Arc::clone(&plan),
        target,
        value: RuntimeValue::Bool(true),
        calls: 0,
    });
    let args = [
        RuntimeValue::Bool(false),
        owner
            .try_wrap(RuntimeValue::Tuple(vec![RuntimeValue::NeedHandle(
                crate::tests::reusable_need("need.borrowed-input"),
            )]))
            .unwrap(),
    ];
    let binding = &plan.pure_programs()[0];
    let local = plan.function_sites().get(binding.site()).unwrap().inputs()[1].input_local();
    assert_eq!(
        evaluate_pure_program_with_backend(&plan, program, &args, &mut backend),
        Err(RuntimeEvalError::AffineLocalCopy(local))
    );
    let RuntimeValue::Opaque(value) = &args[1] else {
        panic!("caller retains the original opaque input")
    };
    assert_eq!(
        value.payload(),
        &RuntimeValue::Tuple(vec![RuntimeValue::NeedHandle(crate::tests::reusable_need(
            "need.borrowed-input"
        ))])
    );
    assert_eq!(backend.external.calls, 0);

    let copy_args = [
        RuntimeValue::Bool(false),
        owner
            .try_wrap(RuntimeValue::Tuple(vec![RuntimeValue::Bool(true)]))
            .unwrap(),
    ];
    assert_eq!(
        evaluate_pure_program_with_backend(&plan, program, &copy_args, &mut backend).unwrap(),
        RuntimeValue::Bool(true)
    );
    assert_eq!(backend.external.calls, 1);
    assert!(matches!(copy_args[1], RuntimeValue::Opaque(_)));
}

struct DialogueExternal {
    plan: Arc<RuntimePlan>,
    value: RuntimeValue,
    calls: usize,
}

impl RuntimeExternalCallBackend for DialogueExternal {
    fn call_external(
        &mut self,
        _context: &RuntimeExternalCallContext,
        _callee: &RuntimeCallTarget,
        _args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        None
    }

    fn produce_character_dialogue(
        &mut self,
        owner: &RuntimeProgramOwner,
        operation: CharacterDialogueOperation,
        target: RuntimeValue,
        fields: &[CharacterDialoguePatchField<RuntimeValue>],
        result_type: RuntimeSemanticTypeId,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        assert!(owner.same_program(&RuntimeProgramOwner::Plan(Arc::clone(&self.plan))));
        assert_eq!(operation, CharacterDialogueOperation::Factory);
        assert_eq!(result_type, RuntimeSemanticTypeId::from_bytes([41; 32]));
        assert!(matches!(target, RuntimeValue::EntityRef(_)));
        assert_eq!(fields.len(), 2);
        assert!(matches!(
            &fields[0].operation,
            CharacterDialoguePatchOperation::Set(RuntimeValue::Bool(true))
        ));
        assert!(matches!(
            fields[1].operation,
            CharacterDialoguePatchOperation::Clear
        ));
        self.calls += 1;
        Ok(self.value.clone())
    }
}

#[test]
fn pure_character_dialogue_uses_the_exact_plan_and_ordered_source_row() {
    use crate::pattern::{RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner};
    use crate::value::{
        RuntimeCharacterDialogueProducerId, RuntimeEntityReference, RuntimeOpaquePersistence,
        RuntimeOpaqueValueClass,
    };
    use arcweft_character::id::CharacterId;
    use arcweft_id::DeclarationIdentityFamily;
    use arcweft_interaction_model::dialogue::CharacterDialogueFieldCoordinate;

    let result_type = RuntimeSemanticTypeId::from_bytes([41; 32]);
    let target_type = RuntimeSemanticTypeId::from_bytes([42; 32]);
    let bool_type = RuntimeSemanticTypeId::from_bytes([43; 32]);
    let producer = RuntimeCharacterDialogueProducerId::get();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(
                    result_type,
                    RuntimePlanTypeProjection::Opaque {
                        producer: producer.clone(),
                        admission: RuntimeOpaqueTypeAdmission::ProducerWide,
                        value_class: RuntimeOpaqueValueClass::Plain,
                        persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
                        arguments: Box::new([]),
                    },
                ),
                RuntimePlanTypeSeed::new(target_type, RuntimePlanTypeProjection::EntityReference),
                RuntimePlanTypeSeed::new(bool_type, RuntimePlanTypeProjection::Bool),
            ],
            [],
        )
        .expect("typed dialogue expression graph");
    let character = CharacterId::try_new("character.alice").unwrap();
    let site = builder
        .push_function_site_seed(
            crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([41; 32]),
            crate::plan::RuntimeFunctionSemanticRole::Ordinary,
            [],
            RuntimeExprSeed::new(
                result_type,
                RuntimeExprSeedKind::CharacterDialogue {
                    operation: CharacterDialogueOperation::Factory,
                    target: Box::new(RuntimeExprSeed::new(
                        target_type,
                        RuntimeExprSeedKind::EntityRef(RuntimeEntityReference::Project {
                            family: DeclarationIdentityFamily::Character,
                            public_id: character.as_public_id(),
                        }),
                    )),
                    fields: vec![
                        CharacterDialoguePatchField {
                            coordinate: CharacterDialogueFieldCoordinate::Voice,
                            operation: CharacterDialoguePatchOperation::Set(RuntimeExprSeed::new(
                                bool_type,
                                RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
                            )),
                        },
                        CharacterDialoguePatchField {
                            coordinate: CharacterDialogueFieldCoordinate::SourceLocale,
                            operation: CharacterDialoguePatchOperation::Clear,
                        },
                    ]
                    .into_boxed_slice(),
                },
            ),
        )
        .expect("program function site");
    let program = RuntimePureProgramId::from_checked_digest([44; 32]);
    builder
        .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed { program, site })
        .expect("pure program binding");
    let plan = Arc::new(builder.finish().expect("sealed plan"));
    let value =
        RuntimeOpaqueTypeOwner::exact(producer, RuntimeSemanticTypeId::from_bytes([45; 32]))
            .try_wrap(RuntimeValue::Unit)
            .expect("exact opaque value");
    let mut backend = VmRuntimePureCallBackend::default().with_external_calls(DialogueExternal {
        plan: Arc::clone(&plan),
        value: value.clone(),
        calls: 0,
    });
    assert_eq!(
        evaluate_pure_program_with_backend(&plan, program, &[], &mut backend)
            .expect("generation backend produced dialogue"),
        value
    );
    assert_eq!(backend.external.calls, 1);
}
