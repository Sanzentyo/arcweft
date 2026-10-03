//! Issued-resource fixtures shared by the two owned program boundaries.

use std::num::{NonZeroU32, NonZeroU64};
use std::sync::Arc;

use crate::line_task::{
    RuntimeHandleLeaseState, RuntimeHandleOwnerSlot, RuntimeHandleResource,
    RuntimeLineHandleLedger, RuntimeStageActorLease,
};
use crate::pattern::{RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimeSemanticTypeId};
use crate::plan::{
    RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFunctionInputBindingSeed,
    RuntimeFunctionInputSource, RuntimeLocalDeclarationSeed, RuntimeLocalReadSeed,
    RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlan, RuntimePlanBuilder,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed, RuntimePureProgramBindingSeed,
};
use crate::runtime_id::{
    DialogueActivationId, ExecutionInstanceId, RuntimeDialogueContentPlanId,
    RuntimeLineHandleSiteId, RuntimeLineHandleToken, RuntimeLocalSlotId, RuntimePersistentFiberId,
};
use crate::value::ownership::RuntimeOwnedSlotId;
use crate::value::{
    RuntimeHandleKind, RuntimeLocalReadMode, RuntimeOpaquePersistence, RuntimeOpaqueValueClass,
    RuntimeValue,
};
use arcweft_id::runtime_program::RuntimePureProgramId;

pub(crate) struct IssuedProgramHandle {
    pub plan: Arc<RuntimePlan>,
    pub program: RuntimePureProgramId,
    pub ledger: RuntimeLineHandleLedger,
    pub value: RuntimeValue,
    pub token: RuntimeLineHandleToken,
}

pub(crate) fn issued_program_handle() -> IssuedProgramHandle {
    let ty = RuntimeSemanticTypeId::from_bytes([0x51; 32]);
    let program = RuntimePureProgramId::from_checked_digest([92; 32]);
    let producer = RuntimeHandleKind::StageActor.try_producer().unwrap();
    let opaque_owner = RuntimeOpaqueTypeOwner::exact_with(
        producer.clone(),
        ty,
        RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::StageActor),
        RuntimeOpaquePersistence::SnapshotOnly,
    );
    let mut builder = RuntimePlanBuilder::new();
    let locals = builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                ty,
                RuntimePlanTypeProjection::Opaque {
                    producer,
                    admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                    value_class: opaque_owner.value_class(),
                    persistence: opaque_owner.persistence(),
                    arguments: Box::new([]),
                },
            )],
            [RuntimeLocalDeclarationSeed::new(ty)],
        )
        .unwrap();
    let local = locals.local_ids()[0].clone();
    let site = builder
        .push_function_site_seed(
            [RuntimeFunctionInputBindingSeed {
                source: RuntimeFunctionInputSource::Parameter { position: 0 },
                input_local: local.clone(),
                pattern: RuntimePatternSeed::new(
                    ty,
                    RuntimePatternSeedKind::Bind {
                        mutable: false,
                        local: local.clone(),
                    },
                ),
                ownership: Default::default(),
                unrestricted_bindings: Box::new([]),
            }],
            RuntimeExprSeed::new(
                ty,
                RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                    local,
                    RuntimeLocalReadMode::Move,
                )),
            ),
        )
        .unwrap();
    builder
        .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed { program, site })
        .unwrap();
    let plan = Arc::new(builder.finish().unwrap());
    let activation = DialogueActivationId::new(
        crate::effect::RuntimeArtifactFingerprint::try_from_bytes([93; 32]).unwrap(),
        RuntimePersistentFiberId::from_allocated(1),
        RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::MIN),
        0,
    );
    let source = RuntimeOwnedSlotId::EnvironmentLocal {
        execution: ExecutionInstanceId::from_allocated(NonZeroU64::MIN),
        local: RuntimeLocalSlotId::from_allocated(NonZeroU64::MIN),
    };
    let mut ledger = RuntimeLineHandleLedger::default();
    let value = ledger
        .issue_exact(
            &activation,
            RuntimeLineHandleSiteId::from_zero_based(0),
            RuntimeHandleKind::StageActor,
            &opaque_owner,
            RuntimeHandleResource::StageActor(RuntimeStageActorLease::new(
                arcweft_character::id::CharacterId::try_new("character.fixture").unwrap(),
            )),
            RuntimeHandleOwnerSlot::ParentFiber(source),
        )
        .unwrap();
    let value = RuntimeValue::Opaque(value);
    let token = RuntimeLineHandleLedger::token_from_value(&value).unwrap();
    ledger
        .set_state(
            &token,
            RuntimeHandleLeaseState::Allocating,
            RuntimeHandleLeaseState::Active,
        )
        .unwrap();
    IssuedProgramHandle {
        plan,
        program,
        ledger,
        value,
        token,
    }
}

/// Publishes the issued handle and moves the complete ledger without cloning it.
pub(crate) fn publish_program_handle(
    mut ledger: RuntimeLineHandleLedger,
    value: RuntimeValue,
    destination: RuntimeOwnedSlotId,
) -> (
    RuntimeValue,
    crate::line_task::RuntimePublishedDialogueRegistry,
) {
    let token = RuntimeLineHandleLedger::token_from_value(&value).unwrap();
    let source = ledger.lease(&token).unwrap().owner().clone();
    ledger
        .transfer(
            &token,
            &source,
            RuntimeHandleOwnerSlot::ParentFiber(destination),
        )
        .unwrap();
    let activation = token.activation().clone();
    let mut registry = crate::line_task::RuntimeDialogueActivationRegistry::<(), ()>::default();
    registry.begin(activation.clone(), ()).unwrap();
    let mut transaction = registry.begin_transaction(&activation).unwrap();
    transaction.line_mut().commit_ledger(ledger);
    transaction.line_mut().commit_result((), value).unwrap();
    transaction.line_mut().begin_result_publication().unwrap();
    let proof = registry.inspect_published(&transaction).unwrap();
    let (_, value) = transaction.line_mut().finish_result_publication().unwrap();
    transaction.line_mut().release_frame().unwrap();
    registry.commit_published_prepared(transaction, proof);
    (value, registry.into_published().unwrap())
}

#[test]
fn published_custody_rejects_active_and_inflight_registries_without_losing_owner() {
    let fixture = issued_program_handle();
    let activation = fixture.token.activation().clone();
    let mut registry = crate::line_task::RuntimeDialogueActivationRegistry::<(), ()>::default();
    registry.begin(activation.clone(), ()).unwrap();
    let (mut registry, reason) = registry.into_published().unwrap_err();
    assert_eq!(
        reason,
        crate::line_task::LineRuntimeError::ParentHandleBeforePublication
    );
    let transaction = registry.begin_transaction(&activation).unwrap();
    let (mut registry, reason) = registry.into_published().unwrap_err();
    assert_eq!(
        reason,
        crate::line_task::LineRuntimeError::ParentHandleBeforePublication
    );
    registry.restore_transaction(transaction).unwrap();
    assert!(registry.begin_transaction(&activation).is_ok());
}

pub(crate) fn awbc_handle_program(
    id: RuntimePureProgramId,
) -> Arc<crate::awbc::schema::AwbcProgram> {
    use crate::awbc::schema::*;
    let ty = RuntimeSemanticTypeId::from_bytes([0x51; 32]);
    let mut program = AwbcProgram::default();
    program.strings = vec![
        RuntimeHandleKind::StageActor
            .try_producer()
            .unwrap()
            .as_str()
            .to_owned(),
    ];
    program.runtime_types = vec![AwbcRuntimeType::new(
        ty,
        AwbcRuntimeTypeShape::Opaque {
            producer: AwbcStringId(0),
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::StageActor),
            persistence: RuntimeOpaquePersistence::SnapshotOnly,
            arguments: Vec::new(),
        },
    )];
    program.signatures = vec![AwbcSignature {
        params: vec![AwbcTypeId(0)],
        result: Some(AwbcTypeId(0)),
        effects: AwbcEffectSetId(0),
    }];
    program.frame_layouts = vec![AwbcFrameLayout {
        scopes: Vec::new(),
        slots: vec![AwbcFrameSlot {
            name: None,
            ty: AwbcTypeId(0),
            role: AwbcFrameSlotRole::Parameter,
            scope_depth: 0,
        }],
        max_scope_depth: 0,
    }];
    program.functions = vec![AwbcFunction {
        public_id: None,
        kind: AwbcFunctionKind::Ordinary,
        signature: AwbcSignatureId(0),
        input_ownership: vec![AwbcFunctionInputOwnership::default()],
        frame_layout: AwbcFrameLayoutId(0),
        blocks: AwbcTableRange::new(0, 1),
        entry_block: AwbcBlockId(0),
        flags: AwbcFunctionFlags::default(),
    }];
    program.blocks = vec![AwbcBlock {
        owner: AwbcFunctionId(0),
        instructions: AwbcTableRange::default(),
        terminator: AwbcTerminator::Return {
            value: Some(AwbcRegisterId(0)),
        },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    }];
    program.pure_programs = vec![AwbcPureProgramBinding {
        program: id,
        function: AwbcFunctionId(0),
        input_types: vec![ty],
        result_type: ty,
    }];
    program
        .verify(
            crate::awbc::verify::AwbcVerifyBudget::default(),
            crate::awbc::verify::AwbcVerifyContext {
                require_entrypoint: false,
                ..Default::default()
            },
        )
        .unwrap();
    Arc::new(program)
}

#[test]
fn detached_input_rejects_issued_handle_before_consuming_awbc_owner() {
    let fixture = issued_program_handle();
    let program = awbc_handle_program(fixture.program);
    crate::awbc::fiber::validate_function_argument_values(
        &program,
        crate::awbc::schema::AwbcFunctionId(0),
        std::slice::from_ref(&fixture.value),
    )
    .unwrap();
    let before = fixture.ledger.clone();
    let error = crate::awbc::product_step::AwbcProductStepExecutor::for_program_invocation(
        program,
        fixture.program,
        vec![fixture.value],
        crate::task::GenerationId::new(0),
        64,
    )
    .unwrap_err();
    let (reason, retained) = error.into_parts();
    assert!(matches!(
        reason,
        crate::awbc::product_step::AwbcProductStepBuildError::ProgramInputCustody {
            position: 0,
            ..
        }
    ));
    assert_eq!(fixture.ledger, before);
    assert_eq!(
        retained[0].affine_line_handles().unwrap()[0].token(),
        &fixture.token
    );
}

#[test]
fn detached_input_rejects_issued_handle_before_consuming_native_owner() {
    let fixture = issued_program_handle();
    let site = fixture.plan.pure_programs()[0].site();
    fixture
        .plan
        .validate_function_site_input_refs(site, &[], &[&fixture.value])
        .unwrap();
    let before = fixture.ledger.clone();
    let error = crate::engine::Engine::for_program_invocation(
        fixture.plan,
        fixture.program,
        vec![fixture.value],
    )
    .unwrap_err();
    let (reason, retained) = error.into_parts();
    assert!(matches!(
        reason,
        crate::value::RuntimeEvalError::ProgramInputCustody { position: 0, .. }
    ));
    assert_eq!(fixture.ledger, before);
    let handle = retained[0].affine_line_handles().unwrap().pop().unwrap();
    assert_eq!(handle.token(), &fixture.token);
    assert_eq!(
        fixture.ledger.lease(&fixture.token).unwrap().state(),
        RuntimeHandleLeaseState::Active
    );
}

#[test]
fn detached_nested_handle_reports_its_original_path_and_preserves_ledger() {
    let fixture = issued_program_handle();
    let before = fixture.ledger.clone();
    let value = RuntimeValue::Tuple(vec![RuntimeValue::Bool(true), fixture.value]);
    let handle = value.affine_line_handles().unwrap().pop().unwrap();
    let error = value.validate_detached_custody().unwrap_err();
    assert_eq!(
        error,
        crate::value::ownership::RuntimeDetachedValueError::LineHandleCustodyRequired {
            token: fixture.token,
            path: handle.path().clone(),
        }
    );
    assert_eq!(fixture.ledger, before);
}
