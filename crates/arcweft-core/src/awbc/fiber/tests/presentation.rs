use super::*;
use crate::effect::RuntimeArtifactFingerprint;
use crate::presentation::RuntimePresentationError;
use crate::presentation::resources::{
    PresentationBody, PresentationLedger, PresentationLedgerLimits, PresentationRegistrationSite,
};
use crate::value::{AwbcRuntimeValueSnapshotError, RuntimePayload, RuntimeRetainedValueAdmission};
use arcweft_interaction_model::presentation::PresentationResourceKind;
use std::{num::NonZeroU64, sync::Arc};

struct Fixture {
    owner: RuntimeProgramOwner,
    artifact: RuntimeArtifactFingerprint,
    fiber: FiberState,
    ledger: PresentationLedger<PresentationBody<RuntimePayload>>,
}

impl Fixture {
    fn new(limit: usize) -> Self {
        let mut program = zero_parameter_entry_program();
        program.runtime_types.push(AwbcRuntimeType::new(
            crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest(),
            crate::awbc::schema::AwbcRuntimeTypeShape::Bool,
        ));
        program.frame_layouts[0].scopes = vec![
            crate::awbc::schema::AwbcScopeDefinition {
                kind: crate::scope::RuntimeScopeFrameKind::EmittedLexical,
                parent: None,
                identity: crate::scope::RuntimeScopeIdentity::Anonymous,
            },
            crate::awbc::schema::AwbcScopeDefinition {
                kind: crate::scope::RuntimeScopeFrameKind::EmittedLexical,
                parent: Some(crate::awbc::schema::AwbcScopeId(0)),
                identity: crate::scope::RuntimeScopeIdentity::Anonymous,
            },
        ];
        program.frame_layouts[0].max_scope_depth = 2;
        program.instructions = [
            (true, 0),
            (true, 1),
            (false, 1),
            (true, 1),
            (false, 1),
            (false, 0),
            (true, 0),
            (false, 0),
        ]
        .map(|(enter, id)| {
            let scope = crate::awbc::schema::AwbcScopeId(id);
            if enter {
                crate::awbc::schema::AwbcInstruction::EnterScope { scope }
            } else {
                crate::awbc::schema::AwbcInstruction::ExitScope { scope }
            }
        })
        .into();
        program.blocks[0].instructions = AwbcTableRange::new(0, 8);
        program.blocks[0].safe_point = AwbcSafePointKind::FlowEntry;
        program.flow_executables.push(AwbcFlowExecutable {
            metadata: crate::entry::RuntimeFlowExecutable {
                flow: program.flow_bindings[0].flow.clone(),
                contract: crate::entry::FlowContractHash::from_bytes([17; 32]),
                controller: None,
            },
            function: AwbcFunctionId(0),
        });
        program
            .verify(
                crate::awbc::verify::AwbcVerifyBudget::default(),
                crate::awbc::verify::AwbcVerifyContext::default(),
            )
            .expect("scope fixture is an admitted executable");
        let owner = RuntimeProgramOwner::Awbc(Arc::new(program));
        let RuntimeProgramOwner::Awbc(program) = &owner else {
            unreachable!()
        };
        let fiber = FiberState::for_entry(program, AwbcEntryId(0), 0, 64).unwrap();
        let artifact = RuntimeArtifactFingerprint::try_from_bytes([17; 32]).unwrap();
        let ledger = PresentationLedger::new(
            owner.clone(),
            artifact,
            PresentationLedgerLimits {
                handles: limit,
                scopes: limit,
            },
        );
        Self {
            owner,
            artifact,
            fiber,
            ledger,
        }
    }

    fn poll_one(&mut self) {
        let RuntimeProgramOwner::Awbc(program) = &self.owner else {
            unreachable!()
        };
        let output = crate::awbc::vm::step(
            program,
            &mut self.fiber,
            crate::awbc::vm::VmStepOptions {
                max_instructions: 1,
            },
        )
        .unwrap();
        assert_eq!(output.executed, 1);
        if self.fiber.status == FiberStatus::Suspended {
            self.fiber.resume_budget_yield(program).unwrap();
        }
    }

    fn bind(&mut self) -> crate::presentation::resources::PresentationScopeOwner {
        self.fiber
            .bind_presentation_scope(&self.owner, &mut self.ledger)
            .unwrap()
    }

    fn register(&mut self, value: bool) -> crate::runtime_id::RuntimePresentationHandleToken {
        let scope = self.bind();
        self.ledger
            .register(
                &self.owner,
                scope,
                PresentationRegistrationSite::Independent(PresentationResourceKind::Image),
                PresentationBody {
                    expected: crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest(),
                    binding: None,
                    value: RuntimePayload(RuntimeValue::Bool(value)),
                },
            )
            .unwrap()
            .0
    }

    fn admission(&self) -> RuntimeRetainedValueAdmission {
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            self.owner.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        let proof = self
            .ledger
            .admit_ownership(&self.owner, self.artifact, &mut admission)
            .unwrap();
        admission.bind_presentation_custody(proof).unwrap();
        self.ledger.admit_bodies(&mut admission).unwrap();
        admission
    }

    fn image(&self) -> AwbcFiberStateSnapshot {
        let mut admission = self.admission();
        let image = AwbcFiberStateSnapshot::from_live(&self.fiber, &mut admission).unwrap();
        admission.finish_presentation_custody().unwrap();
        image
    }

    fn admit_image(
        &self,
        image: &AwbcFiberStateSnapshot,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        let mut admission = self.admission();
        image.admit_retained(&mut admission)?;
        admission.finish_presentation_custody()
    }

    fn retire(&mut self) -> Vec<RuntimeValue> {
        self.fiber
            .release_presentation_scope(&self.owner, &mut self.ledger)
            .unwrap()
            .into_iter()
            .map(|retired| retired.body.value.0)
            .collect()
    }
}

#[test]
fn awbc_scope_grants_follow_actual_enter_exit_and_reentry_without_aliasing() {
    let mut fixture = Fixture::new(8);
    let root = fixture.bind();
    assert_eq!(root, fixture.bind());
    fixture.poll_one();
    let outer = fixture.bind();
    assert_ne!(root, outer);
    fixture.register(false);
    fixture.poll_one();
    let inner = fixture.bind();
    assert_ne!(outer, inner);
    let old = fixture.register(true);
    let old_image = fixture.image();
    assert_eq!(fixture.retire(), vec![RuntimeValue::Bool(true)]);
    fixture.poll_one();
    assert_eq!(fixture.bind(), outer);
    fixture.poll_one();
    let reentered = fixture.bind();
    assert_ne!(reentered, inner);
    fixture.register(false);
    assert_eq!(
        fixture.ledger.release(&fixture.owner, &old, Some(inner)),
        Err(RuntimePresentationError::UnknownHandle)
    );
    let current = fixture.image();
    let mut stale = current.clone();
    stale.frames[0].scopes[1].presentation_scope =
        old_image.frames[0].scopes[1].presentation_scope.clone();
    assert!(matches!(
        fixture.admit_image(&stale),
        Err(AwbcRuntimeValueSnapshotError::PresentationScopeCustodyRequired { .. })
    ));
    assert_eq!(fixture.image(), current);
    assert_eq!(fixture.retire(), vec![RuntimeValue::Bool(false)]);
    fixture.poll_one();
    assert_eq!(fixture.retire(), vec![RuntimeValue::Bool(false)]);
    fixture.poll_one();
    assert_eq!(fixture.bind(), root);
    assert!(fixture.retire().is_empty());
}

#[test]
fn awbc_scope_wire_requires_custody_and_rejects_exchanged_or_forged_headers() {
    let mut fixture = Fixture::new(8);
    fixture.register(true);
    fixture.poll_one();
    fixture.register(false);
    let original = fixture.image();
    let bytes = serde_json::to_vec(&original).unwrap();
    let decoded: AwbcFiberStateSnapshot = serde_json::from_slice(&bytes).unwrap();
    assert!(matches!(
        decoded.clone().into_live_for_program(&fixture.owner),
        Err(AwbcRuntimeValueSnapshotError::PresentationScopeCustodyRequired { .. })
    ));
    fixture.admit_image(&decoded).unwrap();
    let mut restored = decoded
        .into_admitted_live_for_program(&fixture.owner)
        .unwrap();
    assert_eq!(
        restored
            .bind_presentation_scope(&fixture.owner, &mut fixture.ledger)
            .unwrap(),
        fixture.bind()
    );

    let mut exchanged = original.clone();
    let frame = &mut exchanged.frames[0];
    let root = frame.root_presentation_scope.take();
    frame.root_presentation_scope = frame.scopes[0].presentation_scope.take();
    frame.scopes[0].presentation_scope = root;
    assert_eq!(
        fixture.admit_image(&exchanged),
        Err(RuntimePresentationError::Owner.into())
    );

    let original_json = serde_json::to_value(&original).unwrap();
    for field in ["frame", "execution", "fiber", "scope", "lexical_scope"] {
        let mut json = original_json.clone();
        let header = &mut json["frames"][0]["root_presentation_scope"];
        match field {
            "frame" => {
                header["anchor"]["frame"] = serde_json::to_value(
                    RuntimeFrameInstanceId::from_allocated(NonZeroU64::new(99).unwrap()),
                )
                .unwrap()
            }
            "execution" => {
                header["owner"]["execution"] =
                    serde_json::to_value(crate::runtime_id::ExecutionInstanceId::from_allocated(
                        NonZeroU64::new(99).unwrap(),
                    ))
                    .unwrap()
            }
            "fiber" => {
                header["owner"]["fiber"] = serde_json::to_value(
                    crate::runtime_id::RuntimePersistentFiberId::from_allocated(99),
                )
                .unwrap()
            }
            "scope" => {
                header["owner"]["scope"] =
                    serde_json::to_value(crate::runtime_id::RuntimeScopeInstanceId::from_allocated(
                        NonZeroU64::new(99).unwrap(),
                    ))
                    .unwrap()
            }
            "lexical_scope" => {
                header["anchor"]["scope"] =
                    serde_json::to_value(crate::awbc::schema::AwbcScopeId(0)).unwrap()
            }
            _ => unreachable!(),
        }
        let candidate: AwbcFiberStateSnapshot = serde_json::from_value(json).unwrap();
        let error = fixture.admit_image(&candidate).unwrap_err();
        if field == "scope" {
            assert!(matches!(
                error,
                AwbcRuntimeValueSnapshotError::PresentationScopeCustodyRequired { .. }
            ));
        } else {
            assert_eq!(error, RuntimePresentationError::Owner.into(), "{field}");
        }
        assert_eq!(
            fixture.image(),
            original,
            "{field} must preserve the original grant and body"
        );
    }
}

#[test]
fn awbc_scope_capacity_and_missing_frame_rejection_preserve_issuance_and_values() {
    let mut fixture = Fixture::new(2);
    let frame = fixture.fiber.frames.pop().unwrap();
    assert_eq!(
        fixture
            .fiber
            .bind_presentation_scope(&fixture.owner, &mut fixture.ledger),
        Err(RuntimePresentationError::MissingScope)
    );
    fixture.fiber.frames.push(frame);
    fixture.register(true);
    assert_eq!(
        fixture
            .fiber
            .active_frame()
            .unwrap()
            .presentation_scope_binding()
            .unwrap()
            .scope_key()
            .2
            .get(),
        NonZeroU64::MIN
    );
    fixture.poll_one();
    fixture.register(false);
    fixture.poll_one();
    let before = fixture.image();
    assert_eq!(
        fixture
            .fiber
            .bind_presentation_scope(&fixture.owner, &mut fixture.ledger),
        Err(RuntimePresentationError::ScopeCapacity)
    );
    assert_eq!(fixture.image(), before);
    assert!(
        fixture
            .fiber
            .active_frame()
            .unwrap()
            .presentation_scope_binding()
            .is_none()
    );
    fixture.poll_one();
    assert_eq!(fixture.retire(), vec![RuntimeValue::Bool(false)]);
    fixture.poll_one();
    fixture.bind();
    assert_eq!(
        fixture
            .fiber
            .active_frame()
            .unwrap()
            .presentation_scope_binding()
            .unwrap()
            .scope_key()
            .2
            .get()
            .get(),
        3
    );
    fixture.image();
}

#[test]
fn awbc_cancel_without_the_original_presentation_bank_is_atomic() {
    let mut fixture = Fixture::new(4);
    fixture.register(true);
    let before = fixture.image();
    let RuntimeProgramOwner::Awbc(program) = &fixture.owner else {
        unreachable!()
    };
    assert!(matches!(
        crate::awbc::vm::cancel_fiber(program, &mut fixture.fiber),
        Err(crate::awbc::vm::VmError::Fiber(FiberStateError::Snapshot(
            AwbcRuntimeValueSnapshotError::Presentation(RuntimePresentationError::MissingLedger)
        )))
    ));
    assert_eq!(fixture.image(), before);
    assert_eq!(fixture.retire(), vec![RuntimeValue::Bool(true)]);
}

#[test]
fn live_and_inert_scope_headers_share_one_sticky_custody_contour() {
    let mut fixture = Fixture::new(4);
    fixture.register(true);
    let image = fixture.image();
    let mut admission = fixture.admission();
    fixture.fiber.admit_retained(&mut admission).unwrap();
    let error = image.admit_retained(&mut admission).unwrap_err();
    assert!(matches!(
        error,
        AwbcRuntimeValueSnapshotError::PresentationScopeCustodyRequired { .. }
    ));
    assert_eq!(admission.finish_presentation_custody(), Err(error));
    assert_eq!(fixture.image(), image);
    assert_eq!(fixture.retire(), vec![RuntimeValue::Bool(true)]);
}
