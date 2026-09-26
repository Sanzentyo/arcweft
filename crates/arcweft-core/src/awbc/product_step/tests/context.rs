use std::{cell::Cell, rc::Rc, sync::Arc};

use crate::{
    awbc::{
        product_step::execution::ProductVmHost,
        schema::{
            AwbcBlock, AwbcBlockId, AwbcDialogueContentSlot, AwbcDialogueContentTemplate,
            AwbcDialogueValueRole, AwbcFrameLayout, AwbcFrameLayoutId, AwbcFrameSlot,
            AwbcFrameSlotRole, AwbcFunction, AwbcFunctionId, AwbcFunctionKind, AwbcInstruction,
            AwbcIntrinsic, AwbcIntrinsicId, AwbcProgram, AwbcRegisterId, AwbcRuntimeType,
            AwbcRuntimeTypeShape, AwbcSafePointKind, AwbcSignature, AwbcSignatureId, AwbcStringId,
            AwbcTableRange, AwbcTerminator, AwbcTypeId,
        },
        vm::{VmError, VmExecutionContext, VmHost},
    },
    effect::RuntimeArtifactFingerprint,
    entry::{RuntimeCallableId, RuntimeDialogueContentTemplateDigest},
    pattern::{RuntimeCheckedType, RuntimeSemanticTypeId},
    plan::{
        RuntimeCallableAttachedContract, RuntimeCallablePosition, RuntimeCallableStateDefinition,
        RuntimeCallableTransition, RuntimeFunctionTypeContract,
    },
    pure::{
        RuntimeCallBackend, RuntimeExternalCallBackend, RuntimeExternalCallContext,
        VmRuntimePureCallBackend,
    },
    runtime_id::{
        RuntimeCallableStateId, RuntimeDialogueContentTemplateId, RuntimeDialogueValueSlotId,
    },
    task::RuntimeProgramOwner,
    value::{
        RuntimeArcError, RuntimeArcErrorSource, RuntimeCallTarget, RuntimeDialogueContentValue,
        RuntimeDialogueOpaqueRole, RuntimeDialoguePlainTextContextTemplateProof,
        RuntimeDialoguePlainTextContextTemplateRef, RuntimeEvalError, RuntimeIntrinsic,
        RuntimeValue,
    },
};

const STRING: AwbcTypeId = AwbcTypeId(1);
const CALLBACK: AwbcTypeId = AwbcTypeId(2);
const CONTENT: AwbcTypeId = AwbcTypeId(3);

struct AwbcContextFixture {
    program: Arc<AwbcProgram>,
    artifact: RuntimeArtifactFingerprint,
    proof: RuntimeDialoguePlainTextContextTemplateProof,
    callback_target: RuntimeCallTarget,
}

fn context_program() -> AwbcContextFixture {
    let artifact =
        RuntimeArtifactFingerprint::try_from_bytes([0x4a; 32]).expect("nonzero artifact identity");
    let template_id = RuntimeDialogueContentTemplateId::from_zero_based(0)
        .expect("first context template identity exists");
    let digest = RuntimeDialogueContentTemplateDigest::from_bytes([0x62; 32]);
    let template_ref =
        RuntimeDialoguePlainTextContextTemplateRef::from_encoded_identity(template_id, digest);
    let proof =
        RuntimeDialoguePlainTextContextTemplateProof::try_from_validated_ref(template_ref, digest)
            .expect("test proof issues for the producer-validated template identity");
    let content_owner = RuntimeDialogueOpaqueRole::Content.exact_owner();
    let callback_target =
        RuntimeCallTarget::callable(RuntimeCallableId::from_checked_digest([0x64; 32]));
    let callback_state =
        RuntimeCallableStateId::from_zero_based(0).expect("first callback state exists");
    let callback_function = AwbcFunctionId(0);
    let context_intrinsics = [
        RuntimeIntrinsic::StdResultContext,
        RuntimeIntrinsic::StdResultWithContext,
        RuntimeIntrinsic::StdOptionContext,
        RuntimeIntrinsic::StdOptionWithContext,
    ]
    .into_iter()
    .map(|intrinsic| AwbcIntrinsic {
        identity: RuntimeCallTarget::intrinsic(intrinsic),
        signature: AwbcSignatureId(0),
        revision: 1,
    });
    let mut intrinsics = context_intrinsics.collect::<Vec<_>>();
    intrinsics.push(AwbcIntrinsic {
        identity: callback_target.clone(),
        signature: AwbcSignatureId(0),
        revision: 1,
    });

    let program = AwbcProgram {
        strings: vec![
            "context.callback".to_owned(),
            "std.dialogue.content".to_owned(),
        ],
        runtime_types: vec![
            AwbcRuntimeType::unit(),
            AwbcRuntimeType::new(
                RuntimeCheckedType::String.semantic_identity_digest(),
                AwbcRuntimeTypeShape::String,
            ),
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([0x63; 32]),
                AwbcRuntimeTypeShape::Function {
                    contract: RuntimeFunctionTypeContract::default(),
                    parameters: Vec::new(),
                    result: STRING,
                },
            ),
            AwbcRuntimeType::new(
                content_owner.semantic_identity(),
                AwbcRuntimeTypeShape::Opaque {
                    producer: AwbcStringId(1),
                    admission: content_owner.admission(),
                    value_class: content_owner.value_class(),
                    persistence: content_owner.persistence(),
                    arguments: Vec::new(),
                },
            ),
        ],
        signatures: vec![AwbcSignature {
            params: Vec::new(),
            result: Some(STRING),
            effects: crate::awbc::schema::AwbcEffectSetId(0),
        }],
        frame_layouts: vec![AwbcFrameLayout {
            slots: vec![AwbcFrameSlot {
                name: None,
                ty: STRING,
                role: AwbcFrameSlotRole::ReturnValue,
                scope_depth: 0,
            }],
            scopes: Vec::new(),
            max_scope_depth: 0,
        }],
        instructions: vec![AwbcInstruction::CallIntrinsic {
            dst: Some(AwbcRegisterId(0)),
            intrinsic: AwbcIntrinsicId(4),
            args: Vec::new(),
        }],
        blocks: vec![AwbcBlock {
            owner: callback_function,
            instructions: AwbcTableRange::new(0, 1),
            terminator: AwbcTerminator::Return {
                value: Some(AwbcRegisterId(0)),
            },
            safe_point: AwbcSafePointKind::Return,
            source_map: None,
        }],
        functions: vec![AwbcFunction {
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::Ordinary,
            signature: AwbcSignatureId(0),
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 1),
            entry_block: AwbcBlockId(0),
            flags: Default::default(),
        }],
        intrinsics,
        callable_states: vec![RuntimeCallableStateDefinition {
            function_type: CALLBACK,
            origin: callback_state,
            position: RuntimeCallablePosition::Unapplied,
            retained: Box::new([]),
            parameters: Box::new([]),
            result: STRING,
            attached: RuntimeCallableAttachedContract::None,
            transition: RuntimeCallableTransition::Invoke {
                function: callback_function,
                captures: Box::new([]),
                arguments: Box::new([]),
            },
            partials: Box::new([]),
        }],
        content_templates: vec![AwbcDialogueContentTemplate {
            id: template_id,
            digest,
            slots: vec![AwbcDialogueContentSlot {
                slot: RuntimeDialogueValueSlotId::from_zero_based(0)
                    .expect("first context slot identity exists"),
                role: AwbcDialogueValueRole::Formatted,
                semantic_type: CONTENT,
            }],
            effects: Vec::new(),
        }],
        plain_text_context_template: Some(template_ref),
        ..AwbcProgram::default()
    };
    AwbcContextFixture {
        program: Arc::new(program),
        artifact,
        proof,
        callback_target,
    }
}

struct CountContextCallback {
    target: RuntimeCallTarget,
    calls: Rc<Cell<usize>>,
}

impl RuntimeExternalCallBackend for CountContextCallback {
    fn call_external(
        &mut self,
        _context: &RuntimeExternalCallContext,
        callee: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        assert_eq!(callee, &self.target);
        assert!(args.is_empty());
        self.calls.set(self.calls.get() + 1);
        Some(Ok(RuntimeValue::String("lazy context".to_owned())))
    }
}

fn evaluate_context(
    fixture: &AwbcContextFixture,
    proof: Option<RuntimeDialoguePlainTextContextTemplateProof>,
    intrinsic: RuntimeIntrinsic,
    receiver: RuntimeValue,
    message: RuntimeValue,
    backend: &mut impl RuntimeCallBackend,
) -> Result<RuntimeValue, VmError> {
    let context = if let Some(proof) = proof {
        VmExecutionContext::for_program_with_plain_text_context_proof(
            fixture.artifact,
            Arc::clone(&fixture.program),
            proof,
        )
    } else {
        VmExecutionContext::for_program(fixture.artifact, Arc::clone(&fixture.program))
    };
    let program_owner = RuntimeProgramOwner::Awbc(Arc::clone(&fixture.program));
    let mut fallback_stats = crate::step::RuntimePureCallStats::default();
    let mut host = ProductVmHost {
        backend,
        fallback_stats: &mut fallback_stats,
        context,
        program_owner,
    };
    let intrinsic_id = fixture
        .program
        .intrinsics
        .iter()
        .position(|record| record.identity.as_intrinsic() == Some(intrinsic))
        .and_then(|index| u32::try_from(index).ok())
        .map(AwbcIntrinsicId)
        .expect("context intrinsic is registered in the fixture program");
    host.call_intrinsic(&fixture.program, intrinsic_id, &[receiver, message])?
        .ok_or_else(|| VmError::Runtime("context intrinsic returned no value".to_owned()))
}

fn context_error(value: RuntimeValue) -> RuntimeArcError {
    let (case, Some(error)) = value
        .try_into_builtin_variant_case()
        .expect("context returns a builtin carrier")
    else {
        panic!("context failure returns Result::Err");
    };
    assert_eq!(
        case,
        crate::pattern::RuntimeBuiltinVariantCaseIdentity::ResultErr
    );
    RuntimeArcError::try_from_runtime_value(&error).expect("context payload is ArcError")
}

fn expected_content(fixture: &AwbcContextFixture, text: &str) -> RuntimeDialogueContentValue {
    RuntimeDialogueContentValue::try_new_plain_text(fixture.artifact, fixture.proof, text)
        .expect("test proof admits the canonical Content message")
}

#[test]
fn awbc_context_callbacks_match_native_laziness_and_preserve_error_cause() {
    let fixture = context_program();
    let calls = Rc::new(Cell::new(0));
    let mut backend =
        VmRuntimePureCallBackend::default().with_external_calls(CountContextCallback {
            target: fixture.callback_target.clone(),
            calls: Rc::clone(&calls),
        });
    let callback = crate::value::RuntimeCallableValue::try_new(
        RuntimeProgramOwner::Awbc(Arc::clone(&fixture.program)),
        RuntimeCallableStateId::from_zero_based(0).expect("first callback state"),
        [],
    )
    .expect("callback state is AWBC-owned");

    let error = evaluate_context(
        &fixture,
        Some(fixture.proof),
        RuntimeIntrinsic::StdResultWithContext,
        RuntimeValue::result_err(RuntimeValue::String("asset missing".to_owned())),
        RuntimeValue::Callable(callback.clone()),
        &mut backend,
    )
    .expect("Result::Err invokes lazy context");
    assert_eq!(calls.get(), 1);
    let error = context_error(error);
    assert_eq!(error.message(), &expected_content(&fixture, "lazy context"));
    assert!(matches!(
        error.source(),
        Some(RuntimeArcErrorSource::TypedValue(RuntimeValue::String(cause)))
            if cause == "asset missing"
    ));

    let none = evaluate_context(
        &fixture,
        Some(fixture.proof),
        RuntimeIntrinsic::StdOptionWithContext,
        RuntimeValue::option_none(),
        RuntimeValue::Callable(callback.clone()),
        &mut backend,
    )
    .expect("Option::None invokes lazy context");
    assert_eq!(calls.get(), 2);
    assert_eq!(
        context_error(none).message(),
        &expected_content(&fixture, "lazy context")
    );

    let ok = evaluate_context(
        &fixture,
        Some(fixture.proof),
        RuntimeIntrinsic::StdResultWithContext,
        RuntimeValue::result_ok(RuntimeValue::String("ready".to_owned())),
        RuntimeValue::Callable(callback.clone()),
        &mut backend,
    )
    .expect("Result::Ok bypasses the callback");
    assert_eq!(calls.get(), 2);
    assert_eq!(
        ok.try_into_builtin_variant_case(),
        Ok((
            crate::pattern::RuntimeBuiltinVariantCaseIdentity::ResultOk,
            Some(RuntimeValue::String("ready".to_owned()))
        ))
    );

    let some = evaluate_context(
        &fixture,
        Some(fixture.proof),
        RuntimeIntrinsic::StdOptionWithContext,
        RuntimeValue::option_some(RuntimeValue::String("route.main".to_owned())),
        RuntimeValue::Callable(callback),
        &mut backend,
    )
    .expect("Option::Some bypasses the callback");
    assert_eq!(calls.get(), 2);
    assert_eq!(
        some.try_into_builtin_variant_case(),
        Ok((
            crate::pattern::RuntimeBuiltinVariantCaseIdentity::ResultOk,
            Some(RuntimeValue::String("route.main".to_owned()))
        ))
    );
}

#[test]
fn awbc_standalone_context_requires_string_proof_but_accepts_direct_content() {
    let fixture = context_program();
    let mut backend = VmRuntimePureCallBackend::default();

    let missing_proof = evaluate_context(
        &fixture,
        None,
        RuntimeIntrinsic::StdResultContext,
        RuntimeValue::result_err(RuntimeValue::String("cause".to_owned())),
        RuntimeValue::String("plain text".to_owned()),
        &mut backend,
    );
    assert!(missing_proof.is_err());

    let direct_content = expected_content(&fixture, "already Content");
    let result = evaluate_context(
        &fixture,
        None,
        RuntimeIntrinsic::StdResultContext,
        RuntimeValue::result_err(RuntimeValue::String("cause".to_owned())),
        direct_content.clone().into_runtime_value(),
        &mut backend,
    )
    .expect("already typed Content does not need a String-conversion proof");
    assert_eq!(context_error(result).message(), &direct_content);
}
