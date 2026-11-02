use super::*;
use crate::{
    effect::RuntimeArtifactFingerprint,
    plan::task_source::RuntimeTaskSourceProgram,
    presentation::{
        RuntimePresentationResourceLayout,
        factory_test_support::{RESOURCE, fixture as factory_fixture},
        resources::{
            PresentationBanksSaveSnapshot, PresentationLedgerLimits, PresentationScopeOwner,
        },
    },
    runtime_id::{
        DialogueActivationId, ExecutionInstanceId, RuntimeDialogueContentPlanId,
        RuntimePersistentFiberId, RuntimePresentationBankId,
    },
    task::RuntimeProgramOwner,
    value::{RuntimeEnv, RuntimeRetainedValueAdmission},
};
use arcweft_interaction_model::presentation::{
    PresentationHandleMethodId, PresentationResourceKind,
};
use std::{
    num::{NonZeroU32, NonZeroU64},
    sync::Arc,
};

struct Fixture {
    program: RuntimeProgramOwner,
    source: RuntimeTaskSourceProgram,
    layout: Arc<crate::presentation::RuntimePresentationFactoryCall>,
    artifact: RuntimeArtifactFingerprint,
    banks: PresentationBanks<PresentationBody<RuntimePayload>>,
}

impl Fixture {
    fn new() -> Self {
        let (builder, layout) = factory_fixture();
        let mut plan = builder.finish().unwrap();
        let artifact = RuntimeArtifactFingerprint::try_from_bytes([0x39; 32]).unwrap();
        plan.bind_artifact(artifact).unwrap();
        let plan = Arc::new(plan);
        let source = RuntimeTaskSourceProgram::native(Arc::clone(&plan));
        let program = RuntimeProgramOwner::Plan(plan);
        let banks = PresentationBanks::new(
            program.clone(),
            artifact,
            PresentationLedgerLimits::for_schema(RuntimeSchemaLimits::engine_default()),
        );
        Self {
            program,
            source,
            layout,
            artifact,
            banks,
        }
    }

    fn register(
        &mut self,
        bank: RuntimePresentationBankId,
        env: &mut RuntimeEnv,
        value: bool,
    ) -> (RuntimePresentationHandleToken, PresentationScopeOwner) {
        let body = RuntimeRetainedValueAdmission::for_task_source(
            &self.source,
            RuntimeSchemaLimits::engine_default(),
        )
        .construct_presentation_resource(
            self.source.clone(),
            RuntimePresentationResourceLayout::Factory(Arc::clone(&self.layout)),
            None,
            RuntimePayload::new(RuntimeValue::Tuple(vec![RuntimeValue::Bool(value)])),
        )
        .unwrap();
        let scope = self
            .banks
            .ledger_mut(bank)
            .unwrap()
            .bind_scope(
                &self.program,
                ExecutionInstanceId::from_allocated(NonZeroU64::MIN),
                RuntimePersistentFiberId::from_allocated(1),
                env.presentation_scope_target().unwrap(),
            )
            .unwrap();
        let token = self
            .banks
            .register_and_retire(
                &self.program,
                scope,
                PresentationRegistrationSite::Independent(PresentationResourceKind::Image),
                PresentationBody {
                    expected: RESOURCE,
                    binding: None,
                    value: RuntimePayload::new(body),
                },
            )
            .unwrap();
        (token, scope)
    }

    fn frame(&self) -> RuntimePresentationFrame<'_> {
        RuntimePresentationFrame::try_new(Some(&self.banks), RuntimeSchemaLimits::engine_default())
            .unwrap()
    }
}

#[test]
fn empty_frame_needs_no_bank_or_resource_capacity() {
    let frame = RuntimePresentationFrame::try_new(
        None,
        RuntimeSchemaLimits {
            max_sequence_items: 0,
            ..RuntimeSchemaLimits::engine_default()
        },
    )
    .unwrap();
    assert!(frame.is_empty());
    assert_eq!(frame.len(), 0);
    assert_eq!(frame.resources().count(), 0);
}

#[test]
fn frame_borrows_original_bodies_in_bank_and_issuance_order() {
    let mut fixture = Fixture::new();
    let dialogue = RuntimePresentationBankId::Dialogue {
        activation: DialogueActivationId::new(
            fixture.artifact,
            RuntimePersistentFiberId::from_allocated(1),
            RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::MIN),
            0,
        ),
    };
    let mut root_env = RuntimeEnv::default();
    let mut dialogue_env = RuntimeEnv::default();
    let (later_bank, _) = fixture.register(dialogue, &mut dialogue_env, false);
    let (root, _) = fixture.register(RuntimePresentationBankId::Root, &mut root_env, true);
    let frame = fixture.frame();
    assert_eq!(frame.len(), 2);
    let refs = frame.resources().collect::<Vec<_>>();
    assert_eq!(refs[0].token(), &root);
    assert_eq!(refs[1].token(), &later_bank);
    assert_eq!(
        refs[0].value().body(),
        &RuntimeValue::Tuple(vec![RuntimeValue::Bool(true)])
    );
    assert_eq!(
        refs[1].value().body(),
        &RuntimeValue::Tuple(vec![RuntimeValue::Bool(false)])
    );
    for (resource, lease) in refs.iter().zip(fixture.banks.active_leases()) {
        let RuntimeValue::PresentationResource(original) =
            lease.body.as_ref().unwrap().value.value()
        else {
            panic!("real resource body")
        };
        assert!(std::ptr::eq(resource.value(), original.as_ref()));
        assert!(
            resource
                .value()
                .source()
                .program_owner()
                .same_program(&fixture.program)
        );
        assert!(resource.value().type_instantiation().is_none());
        assert!(resource.slot().is_none());
    }
}

#[test]
fn visibility_and_retirement_come_from_the_original_lease() {
    let mut fixture = Fixture::new();
    let mut env = RuntimeEnv::default();
    let (first, scope) = fixture.register(RuntimePresentationBankId::Root, &mut env, true);
    let (second, _) = fixture.register(RuntimePresentationBankId::Root, &mut env, false);
    for (method, visible) in [
        (PresentationHandleMethodId::Hide, false),
        (PresentationHandleMethodId::Show, true),
    ] {
        fixture
            .banks
            .apply_handle_method(&fixture.program, &first, Some(scope), method)
            .unwrap();
        let resource = fixture.frame().resources().next().unwrap();
        assert_eq!(resource.token(), &first);
        assert_eq!(resource.visibility_override(), Some(visible));
        assert_eq!(
            resource.value().body(),
            &RuntimeValue::Tuple(vec![RuntimeValue::Bool(true)])
        );
    }
    fixture
        .banks
        .apply_handle_method(
            &fixture.program,
            &first,
            Some(scope),
            PresentationHandleMethodId::Release,
        )
        .unwrap();
    let frame = fixture.frame();
    assert_eq!(frame.len(), 1);
    assert_eq!(frame.resources().next().unwrap().token(), &second);
    assert_eq!(fixture.banks.retirements.len(), 1);
}

#[test]
fn complete_frame_capacity_rejects_before_references_are_published() {
    let mut fixture = Fixture::new();
    let mut env = RuntimeEnv::default();
    fixture.register(RuntimePresentationBankId::Root, &mut env, true);
    fixture.register(RuntimePresentationBankId::Root, &mut env, false);
    let before = fixture.banks.snapshot_admitted().unwrap();
    let limits = RuntimeSchemaLimits {
        max_sequence_items: 1,
        ..RuntimeSchemaLimits::engine_default()
    };
    assert_eq!(
        RuntimePresentationFrame::try_new(Some(&fixture.banks), limits).unwrap_err(),
        RuntimePresentationFrameError::Resources {
            actual: 2,
            maximum: 1
        }
    );
    assert_eq!(fixture.banks.snapshot_admitted().unwrap(), before);
    assert_eq!(
        RuntimePresentationFrame::try_new(
            Some(&fixture.banks),
            RuntimeSchemaLimits {
                max_sequence_items: 2,
                ..limits
            }
        )
        .unwrap()
        .len(),
        2
    );
}

#[test]
fn resource_frame_rejects_a_nonresource_stored_body() {
    let mut fixture = Fixture::new();
    let mut env = RuntimeEnv::default();
    let scope = fixture
        .banks
        .ledger_mut(RuntimePresentationBankId::Root)
        .unwrap()
        .bind_scope(
            &fixture.program,
            ExecutionInstanceId::from_allocated(NonZeroU64::MIN),
            RuntimePersistentFiberId::from_allocated(1),
            env.presentation_scope_target().unwrap(),
        )
        .unwrap();
    fixture
        .banks
        .register_and_retire(
            &fixture.program,
            scope,
            PresentationRegistrationSite::Independent(PresentationResourceKind::Image),
            PresentationBody {
                expected: crate::presentation::factory_test_support::BOOL,
                binding: None,
                value: RuntimePayload::new(RuntimeValue::Bool(true)),
            },
        )
        .unwrap();
    assert_eq!(
        RuntimePresentationFrame::try_new(
            Some(&fixture.banks),
            RuntimeSchemaLimits::engine_default()
        )
        .unwrap_err(),
        RuntimePresentationFrameError::Resource { ordinal: 0 }
    );
}

#[test]
fn admitted_restore_preserves_projected_identity_body_and_visibility() {
    let mut fixture = Fixture::new();
    let mut env = RuntimeEnv::default();
    let (token, scope) = fixture.register(RuntimePresentationBankId::Root, &mut env, true);
    fixture
        .banks
        .apply_handle_method(
            &fixture.program,
            &token,
            Some(scope),
            PresentationHandleMethodId::Hide,
        )
        .unwrap();
    let mut admission = RuntimeRetainedValueAdmission::for_program(
        fixture.program.clone(),
        RuntimeSchemaLimits::engine_default(),
    );
    fixture
        .banks
        .admit_retained(&fixture.program, fixture.artifact, &mut admission)
        .unwrap();
    env.admit_retained(&mut admission).unwrap();
    admission.finish_presentation_custody().unwrap();
    let snapshot = fixture
        .banks
        .snapshot_admitted()
        .unwrap()
        .inert_state_admitted();
    let decoded: PresentationBanksSaveSnapshot =
        serde_json::from_slice(&serde_json::to_vec(&snapshot).unwrap()).unwrap();
    let retained = decoded.retain_program(
        &fixture.program,
        PresentationLedgerLimits::for_schema(RuntimeSchemaLimits::engine_default()),
    );
    let mut admission = RuntimeRetainedValueAdmission::for_task_source(
        &fixture.source,
        RuntimeSchemaLimits::engine_default(),
    );
    retained
        .admit_retained(&fixture.program, fixture.artifact, &mut admission)
        .unwrap();
    env.admit_retained(&mut admission).unwrap();
    admission.finish_presentation_custody().unwrap();
    let context = crate::value::RuntimeValueRestoreContext::new(
        &fixture.program,
        Some(crate::task::TaskSourceRestore {
            program: &fixture.source,
            admission: &admission,
        }),
    );
    fixture.banks = retained.into_admitted_live(context).unwrap();
    let resource = fixture.frame().resources().next().unwrap();
    assert_eq!(resource.token(), &token);
    assert_eq!(resource.visibility_override(), Some(false));
    assert_eq!(
        resource.value().body(),
        &RuntimeValue::Tuple(vec![RuntimeValue::Bool(true)])
    );
    assert!(
        resource
            .value()
            .source()
            .program_owner()
            .same_program(&fixture.program)
    );
}
