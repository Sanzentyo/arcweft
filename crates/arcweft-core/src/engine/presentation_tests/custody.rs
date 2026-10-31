use super::*;
use crate::pattern::{
    RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId,
};
use crate::plan::{
    RuntimeLocalDeclarationSeed, RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
};
use crate::value::RuntimeStoredValue;
use crate::value::{RuntimeOpaquePersistence, RuntimeOpaqueValueClass};

struct Fixture {
    engine: Engine,
    body_type: crate::pattern::RuntimeSemanticTypeId,
    tuple_type: crate::pattern::RuntimeSemanticTypeId,
    handle_owner: RuntimeOpaqueTypeOwner,
    locals: [crate::runtime_id::RuntimeLocalDeclarationId; 2],
}

impl Fixture {
    fn new() -> Self {
        let body_type = crate::pattern::RuntimeSemanticTypeId::from_bytes([101; 32]);
        let handle_type = crate::pattern::RuntimeSemanticTypeId::from_bytes([102; 32]);
        let tuple_type = crate::pattern::RuntimeSemanticTypeId::from_bytes([103; 32]);
        let producer = RuntimeOpaqueTypeProducerId::try_new("test.presentation.handle").unwrap();
        let handle_owner = RuntimeOpaqueTypeOwner::exact_with(
            producer.clone(),
            handle_type,
            RuntimeOpaqueValueClass::PresentationHandle,
            RuntimeOpaquePersistence::SnapshotOnly,
        );
        let mut builder = RuntimePlanBuilder::new();
        let admitted = builder
            .admit_type_batch(
                [
                    RuntimePlanTypeSeed::new(body_type, RuntimePlanTypeProjection::Bool),
                    RuntimePlanTypeSeed::new(
                        handle_type,
                        RuntimePlanTypeProjection::Opaque {
                            producer,
                            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                            value_class: RuntimeOpaqueValueClass::PresentationHandle,
                            persistence: RuntimeOpaquePersistence::SnapshotOnly,
                            arguments: Box::new([]),
                        },
                    ),
                    RuntimePlanTypeSeed::new(
                        tuple_type,
                        RuntimePlanTypeProjection::Tuple(Box::new([handle_type])),
                    ),
                ],
                ["first", "second"].map(|name| {
                    RuntimeLocalDeclarationSeed::new(
                        manual_local_source(&format!(
                            "arcweft-core.fixture.presentation.custody.{name}"
                        )),
                        tuple_type,
                    )
                }),
            )
            .unwrap();
        let locals = admitted
            .local_ids()
            .iter()
            .map(|local| {
                builder
                    .lower_pattern_seed_for_test(crate::plan::RuntimePatternSeed::new(
                        tuple_type,
                        crate::plan::RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local: local.clone(),
                        },
                    ))
                    .unwrap()
                    .binding_declarations()
                    .next()
                    .unwrap()
                    .local()
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        builder
            .declare_test_input_locals(admitted.local_ids())
            .unwrap();
        let mut plan = builder.finish().unwrap();
        plan.bind_artifact(
            crate::effect::RuntimeArtifactFingerprint::try_from_bytes([41; 32]).unwrap(),
        )
        .unwrap();
        Self {
            engine: Engine::new(plan),
            body_type,
            tuple_type,
            handle_owner,
            locals,
        }
    }

    fn register(&mut self) -> crate::runtime_id::RuntimePresentationHandleToken {
        self.engine
            .register_presentation_body(
                view_site(),
                self.body_type,
                RuntimePayload(RuntimeValue::Bool(true)),
            )
            .unwrap()
            .0
    }

    fn bind(&mut self, index: usize, token: &crate::runtime_id::RuntimePresentationHandleToken) {
        self.engine.fiber.frame.env.set(
            self.locals[index],
            RuntimeValue::Tuple(vec![
                self.handle_owner.try_wrap(token.encode_payload()).unwrap(),
            ]),
        );
    }
}

#[test]
fn native_presentation_handle_custody_roundtrips_the_actual_frame_and_rejects_duplicate_owners() {
    let mut fixture = Fixture::new();
    let token = fixture.register();
    fixture.bind(0, &token);
    fixture
        .engine
        .push_scope_frame(crate::scope::RuntimeScopeIdentity::Anonymous);
    fixture.register();
    fixture
        .engine
        .exit_emitted_scope(
            &mut RuntimeStepOutput::default(),
            &mut crate::pure::VmRuntimePureCallBackend::default(),
        )
        .unwrap();
    let owner = fixture.engine.fiber.frame.program.clone();
    let image = fixture.engine.inert_rollback_image().unwrap();
    let restored = Engine::from_rollback_image(image.clone(), &owner).unwrap();
    assert_eq!(restored.inert_rollback_image().unwrap(), image);
    let mut duplicate = image.clone();
    let value = RuntimeValue::Tuple(vec![
        fixture
            .handle_owner
            .try_wrap(token.encode_payload())
            .unwrap(),
    ]);
    duplicate.presentation.as_mut().unwrap().retirements_mut()[0]
        .body
        .expected = fixture.tuple_type;
    duplicate.presentation.as_mut().unwrap().retirements_mut()[0]
        .body
        .value = crate::value::AwbcRuntimeValueSnapshot::from_standalone_value(&value).unwrap();
    let error = Engine::from_rollback_image(duplicate, &owner).unwrap_err();
    assert!(error.contains("invalid retained custody"), "{error}");
    assert!(error.contains("duplicate: true"), "{error}");
    assert_eq!(fixture.engine.inert_rollback_image().unwrap(), image);
    fixture.bind(1, &token);
    assert!(fixture.engine.inert_rollback_image().is_err());
}

#[test]
fn native_presentation_handle_requires_the_issuing_ledger_and_current_lease() {
    let mut fixture = Fixture::new();
    let token = fixture.register();
    fixture.bind(0, &token);
    let mut image = fixture.engine.inert_rollback_image().unwrap();
    let owner = fixture.engine.fiber.frame.program.clone();
    image.presentation = None;
    assert!(Engine::from_rollback_image(image, &owner).is_err());
    let bound = fixture
        .engine
        .fiber
        .bind_presentation_scope(
            fixture
                .engine
                .presentation
                .as_mut()
                .unwrap()
                .ledger_mut(crate::runtime_id::RuntimePresentationBankId::Root)
                .unwrap(),
        )
        .unwrap();
    fixture
        .engine
        .presentation
        .as_mut()
        .unwrap()
        .ledger_mut(crate::runtime_id::RuntimePresentationBankId::Root)
        .unwrap()
        .release(&owner, &token, Some(bound))
        .unwrap();
    let error = fixture.engine.inert_rollback_image().unwrap_err();
    assert!(error.contains("invalid retained custody"), "{error}");
}

#[test]
fn native_presentation_custody_bank_does_not_refresh_between_metadata_and_owned_values() {
    let mut fixture = Fixture::new();
    let token = fixture.register();
    let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
        fixture.engine.fiber.frame.program.clone(),
        crate::entry::RuntimeSchemaLimits::engine_default(),
    );
    fixture
        .engine
        .admit_presentation_state(&mut admission)
        .unwrap();
    let value = fixture
        .handle_owner
        .try_wrap(token.encode_payload())
        .unwrap();
    let mut copy_admission = crate::value::RuntimeRetainedValueAdmission::for_program(
        fixture.engine.fiber.frame.program.clone(),
        crate::entry::RuntimeSchemaLimits::engine_default(),
    );
    fixture
        .engine
        .admit_presentation_state(&mut copy_admission)
        .unwrap();
    assert_eq!(
        copy_admission.validate(&value),
        Err(crate::value::AwbcRuntimeValueSnapshotError::AffineValue)
    );
    admission
        .validate_owned_borrowed(value.borrowed_value())
        .unwrap();
    let state = admission.try_into_state().ok().unwrap();
    let mut resumed = state.resume();
    let first = resumed
        .validate_owned_borrowed(value.borrowed_value())
        .unwrap_err();
    assert!(matches!(
        first,
        crate::value::AwbcRuntimeValueSnapshotError::Resource(
            crate::entry::RuntimeSchemaError::PresentationHandleCustody {
                duplicate: true,
                ..
            }
        )
    ));
    assert_eq!(resumed.validate_metadata_string("later"), Err(first));
}

#[test]
fn native_presentation_registration_moves_handle_captures_and_retires_the_owned_graph() {
    let mut fixture = Fixture::new();
    let captured = fixture.register();
    let value = RuntimeValue::Tuple(vec![
        fixture
            .handle_owner
            .try_wrap(captured.encode_payload())
            .unwrap(),
    ]);
    let (outer, _) = fixture
        .engine
        .register_presentation_body(view_site(), fixture.tuple_type, RuntimePayload(value))
        .unwrap();
    let owner = fixture.engine.fiber.frame.program.clone();
    let image = fixture.engine.inert_rollback_image().unwrap();
    let mut restored = Engine::from_rollback_image(image.clone(), &owner).unwrap();
    assert_eq!(restored.inert_rollback_image().unwrap(), image);
    restored.finish(
        &mut RuntimeStepOutput::default(),
        &mut crate::pure::VmRuntimePureCallBackend::default(),
    );
    assert_eq!(
        restored.presentation.as_ref().unwrap().retirements().len(),
        2
    );
    assert_eq!(
        restored.presentation.as_ref().unwrap().retirements()[0].token,
        captured
    );
    assert_eq!(
        restored.presentation.as_ref().unwrap().retirements()[1].token,
        outer
    );
    let retired_image = restored.inert_rollback_image().unwrap();
    assert_eq!(
        Engine::from_rollback_image(retired_image.clone(), &owner)
            .unwrap()
            .inert_rollback_image()
            .unwrap(),
        retired_image
    );

    let mut duplicate = Fixture::new();
    let token = duplicate.register();
    duplicate.bind(0, &token);
    let before = duplicate.engine.inert_rollback_image().unwrap();
    let value = RuntimeValue::Tuple(vec![
        duplicate
            .handle_owner
            .try_wrap(token.encode_payload())
            .unwrap(),
    ]);
    let error = duplicate
        .engine
        .register_presentation_body(view_site(), duplicate.tuple_type, RuntimePayload(value))
        .unwrap_err();
    assert!(matches!(
        error.cause,
        crate::value::AwbcRuntimeValueSnapshotError::Resource(
            crate::entry::RuntimeSchemaError::PresentationHandleCustody {
                duplicate: true,
                ..
            }
        )
    ));
    assert_eq!(error.body.value().affine_handles().unwrap().len(), 1);
    assert_eq!(duplicate.engine.inert_rollback_image().unwrap(), before);
}
