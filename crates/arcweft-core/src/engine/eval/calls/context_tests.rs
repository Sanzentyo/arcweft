use std::{cell::Cell, rc::Rc, sync::Arc};

use crate::{
    effect::RuntimeArtifactFingerprint,
    engine::{Engine, RuntimeEvalError},
    entry::RuntimeDialogueContentTemplateDigest,
    pattern::{RuntimeOpaqueTypeOwner, RuntimeSemanticTypeId, runtime_standard_opaque_type},
    plan::{
        RuntimeCallableAttachedContract, RuntimeCallablePosition, RuntimeCallableStateSeed,
        RuntimeCallableTransition, RuntimeDialogueContentSlotSeed,
        RuntimeDialogueContentTemplateManifestSeed, RuntimeDialogueValueRole, RuntimeEffectSet,
        RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFunctionSiteBodyKind,
        RuntimeFunctionSiteBodySeed, RuntimeFunctionSiteDeclarationSeed, RuntimePlanBuilder,
        RuntimePlanTypeProjection, RuntimePlanTypeSeed,
    },
    pure::{RuntimeExternalCallBackend, VmRuntimePureCallBackend},
    runtime_id::{
        RuntimeCallableStateId, RuntimeDialogueContentTemplateId, RuntimeDialogueValueSlotId,
    },
    task::RuntimeProgramOwner,
    value::{
        RuntimeArcError, RuntimeArcErrorSource, RuntimeCallArgument, RuntimeCallArgumentMode,
        RuntimeCallTarget, RuntimeDialogueContentValue, RuntimeDialogueOpaqueRole,
        RuntimeDialoguePlainTextContextTemplateProof, RuntimeDialoguePlainTextContextTemplateRef,
        RuntimeExpr, RuntimeExprKind, RuntimeIntrinsic, RuntimeValue,
    },
};

const STRING: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([0x11; 32]);
const STRING_TUPLE: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([0x12; 32]);
const RESULT_STRING_ERROR: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([0x13; 32]);
const RESULT_ARC_ERROR: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([0x14; 32]);
const OPTION_STRING: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([0x15; 32]);
const CALLBACK: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([0x16; 32]);

#[derive(Clone, Copy)]
struct ContextTypes {
    content: RuntimeSemanticTypeId,
    result_string_error: RuntimeSemanticTypeId,
    result_arc_error: RuntimeSemanticTypeId,
    option_string: RuntimeSemanticTypeId,
    callback: RuntimeSemanticTypeId,
}

struct ContextPlan {
    plan: crate::plan::RuntimePlan,
    types: ContextTypes,
    artifact: RuntimeArtifactFingerprint,
    proof: RuntimeDialoguePlainTextContextTemplateProof,
    callback_target: RuntimeCallTarget,
    has_callback: bool,
}

fn opaque_projection(
    owner: &RuntimeOpaqueTypeOwner,
) -> RuntimePlanTypeProjection<RuntimeSemanticTypeId> {
    RuntimePlanTypeProjection::Opaque {
        producer: owner.producer().clone(),
        admission: owner.admission(),
        value_class: owner.value_class(),
        persistence: owner.persistence(),
        arguments: Box::new([]),
    }
}

fn context_plan(with_proof: bool, with_callback: bool) -> ContextPlan {
    let arc_error_owner = runtime_standard_opaque_type(&["ArcError"])
        .and_then(|spec| spec.monomorphic_owner())
        .expect("standard ArcError owner exists");
    let content_owner = RuntimeDialogueOpaqueRole::Content.exact_owner();
    let arc_error = arc_error_owner.semantic_identity();
    let content = content_owner.semantic_identity();
    let error_arc_tuple = RuntimeSemanticTypeId::from_bytes([0x18; 32]);

    let template_id = RuntimeDialogueContentTemplateId::from_zero_based(0)
        .expect("first context template identity exists");
    let template_digest = RuntimeDialogueContentTemplateDigest::from_bytes([0x22; 32]);
    let template_ref = RuntimeDialoguePlainTextContextTemplateRef::from_encoded_identity(
        template_id,
        template_digest,
    );
    let proof = RuntimeDialoguePlainTextContextTemplateProof::try_from_validated_ref(
        template_ref,
        template_digest,
    )
    .expect("Core test proof issues for the producer-validated identity");
    let artifact =
        RuntimeArtifactFingerprint::try_from_bytes([0x4a; 32]).expect("nonzero artifact identity");
    let callback_target = RuntimeCallTarget::callable(
        crate::entry::RuntimeCallableId::from_checked_digest([0x49; 32]),
    );

    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(STRING, RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(
                    STRING_TUPLE,
                    RuntimePlanTypeProjection::Tuple(Box::new([STRING])),
                ),
                RuntimePlanTypeSeed::new(
                    error_arc_tuple,
                    RuntimePlanTypeProjection::Tuple(Box::new([arc_error])),
                ),
                RuntimePlanTypeSeed::new(arc_error, opaque_projection(&arc_error_owner)),
                RuntimePlanTypeSeed::new(content, opaque_projection(&content_owner)),
                RuntimePlanTypeSeed::new(
                    RESULT_STRING_ERROR,
                    RuntimePlanTypeProjection::Result {
                        value: STRING,
                        error: STRING,
                        value_payload: STRING_TUPLE,
                        error_payload: STRING_TUPLE,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    RESULT_ARC_ERROR,
                    RuntimePlanTypeProjection::Result {
                        value: STRING,
                        error: arc_error,
                        value_payload: STRING_TUPLE,
                        error_payload: error_arc_tuple,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    OPTION_STRING,
                    RuntimePlanTypeProjection::Option {
                        item: STRING,
                        some_payload: STRING_TUPLE,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    CALLBACK,
                    RuntimePlanTypeProjection::Function {
                        contract: Default::default(),
                        parameters: Box::new([]),
                        result: STRING,
                    },
                ),
            ],
            [],
        )
        .expect("context operand types admit");
    if with_proof {
        let registered = builder
            .register_plain_text_context_template_seed(RuntimeDialogueContentTemplateManifestSeed {
                id: template_id,
                digest: template_digest,
                slots: vec![RuntimeDialogueContentSlotSeed {
                    slot: RuntimeDialogueValueSlotId::from_zero_based(0)
                        .expect("first context slot identity exists"),
                    role: RuntimeDialogueValueRole::Formatted,
                    semantic_type: content,
                }]
                .into_boxed_slice(),
                effects: Box::new([]),
            })
            .expect("context template schema registers");
        assert_eq!(registered, template_ref);
    }

    if with_callback {
        let site = builder
            .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
                inputs: Box::new([]),
                result: STRING,
                body_kind: RuntimeFunctionSiteBodyKind::Expression,
                effects: RuntimeEffectSet::empty(),
            })
            .expect("callback function site reserves");
        builder
            .define_function_site_seed(
                &site,
                RuntimeFunctionSiteBodySeed::Expression(RuntimeExprSeed::new(
                    STRING,
                    RuntimeExprSeedKind::Call {
                        callee: callback_target.clone(),
                        args: Box::new([]),
                    },
                )),
            )
            .expect("callback function site defines");
        let state = builder
            .reserve_callable_state_seed()
            .expect("callback state reserves");
        builder
            .define_callable_state_seed(
                &state,
                RuntimeCallableStateSeed {
                    function_type: CALLBACK,
                    origin: state.clone(),
                    position: RuntimeCallablePosition::Unapplied,
                    retained: Box::new([]),
                    parameters: Box::new([]),
                    result: STRING,
                    attached: RuntimeCallableAttachedContract::None,
                    transition: RuntimeCallableTransition::Invoke {
                        function: site,
                        captures: Box::new([]),
                        arguments: Box::new([]),
                    },
                    partials: Box::new([]),
                },
            )
            .expect("callback state defines");
    }

    let mut plan = builder.finish().expect("context RuntimePlan seals");
    plan.bind_artifact(artifact)
        .expect("context plan binds its artifact");
    if with_proof {
        plan.accept_plain_text_context_template_proof(proof)
            .expect("plan accepts its canonical context proof");
    }
    ContextPlan {
        plan,
        types: ContextTypes {
            content,
            result_string_error: RESULT_STRING_ERROR,
            result_arc_error: RESULT_ARC_ERROR,
            option_string: OPTION_STRING,
            callback: CALLBACK,
        },
        artifact,
        proof,
        callback_target,
        has_callback: with_callback,
    }
}

fn value_expression(
    plan: &crate::plan::RuntimePlan,
    ty: RuntimeSemanticTypeId,
    value: RuntimeValue,
) -> RuntimeExpr {
    RuntimeExpr::from_admitted_parts(
        plan.type_table()
            .id_for_semantic(ty)
            .expect("semantic type is in the test plan"),
        RuntimeExprKind::Value(value),
    )
}

fn context_expression(
    plan: &crate::plan::RuntimePlan,
    result_type: RuntimeSemanticTypeId,
    receiver_type: RuntimeSemanticTypeId,
    receiver: RuntimeValue,
    message_type: RuntimeSemanticTypeId,
    message: RuntimeValue,
    intrinsic: RuntimeIntrinsic,
) -> RuntimeExpr {
    let receiver = value_expression(plan, receiver_type, receiver);
    let message = value_expression(plan, message_type, message);
    RuntimeExpr::from_admitted_parts(
        plan.type_table()
            .id_for_semantic(result_type)
            .expect("context result type is in the test plan"),
        RuntimeExprKind::Call {
            callee: RuntimeCallTarget::intrinsic(intrinsic),
            args: vec![
                RuntimeCallArgument::from_admitted_parts(
                    receiver,
                    RuntimeCallArgumentMode::Value,
                    0,
                ),
                RuntimeCallArgument::from_admitted_parts(
                    message,
                    RuntimeCallArgumentMode::Value,
                    1,
                ),
            ],
        },
    )
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

struct CountLazyMessage {
    calls: Rc<Cell<usize>>,
    target: RuntimeCallTarget,
}

impl RuntimeExternalCallBackend for CountLazyMessage {
    fn call_external(
        &mut self,
        _context: &crate::pure::RuntimeExternalCallContext,
        callee: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        assert_eq!(callee, &self.target);
        assert!(args.is_empty());
        self.calls.set(self.calls.get() + 1);
        Some(Ok(RuntimeValue::String("lazy context".to_owned())))
    }
}

fn expected_content(
    artifact: RuntimeArtifactFingerprint,
    proof: RuntimeDialoguePlainTextContextTemplateProof,
    text: &str,
) -> RuntimeDialogueContentValue {
    RuntimeDialogueContentValue::try_new_plain_text(artifact, proof, text)
        .expect("proof admits a canonical Content message")
}

#[test]
fn native_context_intrinsics_preserve_carriers_and_build_content_errors() {
    let context = context_plan(true, false);
    let expected = expected_content(context.artifact, context.proof, "opening failed");
    let mut engine = Engine::new(context.plan);
    let mut backend = VmRuntimePureCallBackend::default();

    let result_error = engine
        .evaluate_expr_with_backend(
            &context_expression(
                &engine.plan,
                context.types.result_arc_error,
                context.types.result_string_error,
                RuntimeValue::result_err(RuntimeValue::String("asset missing".to_owned())),
                STRING,
                RuntimeValue::String("opening failed".to_owned()),
                RuntimeIntrinsic::StdResultContext,
            ),
            &mut backend,
        )
        .expect("Result::Err context executes");
    let error = context_error(result_error);
    assert_eq!(error.message(), &expected);
    assert!(matches!(
        error.source(),
        Some(RuntimeArcErrorSource::TypedValue(RuntimeValue::String(cause)))
            if cause == "asset missing"
    ));
    assert_eq!(error.trace().frames()[0].message(), Some(&expected));

    let result_ok = engine
        .evaluate_expr_with_backend(
            &context_expression(
                &engine.plan,
                context.types.result_arc_error,
                context.types.result_string_error,
                RuntimeValue::result_ok(RuntimeValue::String("ready".to_owned())),
                STRING,
                RuntimeValue::String("unused".to_owned()),
                RuntimeIntrinsic::StdResultContext,
            ),
            &mut backend,
        )
        .expect("Result::Ok passes through");
    assert_eq!(
        result_ok.try_into_builtin_variant_case(),
        Ok((
            crate::pattern::RuntimeBuiltinVariantCaseIdentity::ResultOk,
            Some(RuntimeValue::String("ready".to_owned()))
        ))
    );

    let option_none = engine
        .evaluate_expr_with_backend(
            &context_expression(
                &engine.plan,
                context.types.result_arc_error,
                context.types.option_string,
                RuntimeValue::option_none(),
                STRING,
                RuntimeValue::String("route missing".to_owned()),
                RuntimeIntrinsic::StdOptionContext,
            ),
            &mut backend,
        )
        .expect("Option::None context executes");
    let error = context_error(option_none);
    assert_eq!(error.kind().as_str(), "MissingValue");
    assert!(error.source().is_none());
    assert_eq!(
        error.message(),
        &expected_content(context.artifact, context.proof, "route missing")
    );

    let option_some = engine
        .evaluate_expr_with_backend(
            &context_expression(
                &engine.plan,
                context.types.result_arc_error,
                context.types.option_string,
                RuntimeValue::option_some(RuntimeValue::String("route.main".to_owned())),
                STRING,
                RuntimeValue::String("unused".to_owned()),
                RuntimeIntrinsic::StdOptionContext,
            ),
            &mut backend,
        )
        .expect("Option::Some becomes Result::Ok");
    assert_eq!(
        option_some.try_into_builtin_variant_case(),
        Ok((
            crate::pattern::RuntimeBuiltinVariantCaseIdentity::ResultOk,
            Some(RuntimeValue::String("route.main".to_owned()))
        ))
    );
}

#[test]
fn native_with_context_calls_the_callback_once_only_for_err_or_none() {
    let context = context_plan(true, true);
    assert!(context.has_callback);
    let mut engine = Engine::new(context.plan);
    let calls = Rc::new(Cell::new(0));
    let mut backend = VmRuntimePureCallBackend::default().with_external_calls(CountLazyMessage {
        calls: Rc::clone(&calls),
        target: context.callback_target.clone(),
    });
    let callback = crate::value::RuntimeCallableValue::try_new(
        RuntimeProgramOwner::Plan(Arc::clone(&engine.plan)),
        RuntimeCallableStateId::from_zero_based(0).expect("first callback state"),
        [],
    )
    .expect("callback state is plan-owned");

    let result_error = engine
        .evaluate_expr_with_backend(
            &context_expression(
                &engine.plan,
                context.types.result_arc_error,
                context.types.result_string_error,
                RuntimeValue::result_err(RuntimeValue::String("failure".to_owned())),
                context.types.callback,
                RuntimeValue::Callable(callback.clone()),
                RuntimeIntrinsic::StdResultWithContext,
            ),
            &mut backend,
        )
        .expect("Result::Err invokes lazy context");
    assert_eq!(calls.get(), 1);
    assert_eq!(
        context_error(result_error).message(),
        &expected_content(context.artifact, context.proof, "lazy context")
    );

    let option_none = engine
        .evaluate_expr_with_backend(
            &context_expression(
                &engine.plan,
                context.types.result_arc_error,
                context.types.option_string,
                RuntimeValue::option_none(),
                context.types.callback,
                RuntimeValue::Callable(callback.clone()),
                RuntimeIntrinsic::StdOptionWithContext,
            ),
            &mut backend,
        )
        .expect("Option::None invokes lazy context");
    assert_eq!(calls.get(), 2);
    assert_eq!(
        context_error(option_none).message(),
        &expected_content(context.artifact, context.proof, "lazy context")
    );

    let result_ok = engine
        .evaluate_expr_with_backend(
            &context_expression(
                &engine.plan,
                context.types.result_arc_error,
                context.types.result_string_error,
                RuntimeValue::result_ok(RuntimeValue::String("ready".to_owned())),
                context.types.callback,
                RuntimeValue::Callable(callback.clone()),
                RuntimeIntrinsic::StdResultWithContext,
            ),
            &mut backend,
        )
        .expect("Result::Ok skips the callback");
    assert_eq!(calls.get(), 2);
    assert_eq!(
        result_ok.try_into_builtin_variant_case(),
        Ok((
            crate::pattern::RuntimeBuiltinVariantCaseIdentity::ResultOk,
            Some(RuntimeValue::String("ready".to_owned()))
        ))
    );

    let option_some = engine
        .evaluate_expr_with_backend(
            &context_expression(
                &engine.plan,
                context.types.result_arc_error,
                context.types.option_string,
                RuntimeValue::option_some(RuntimeValue::String("route".to_owned())),
                context.types.callback,
                RuntimeValue::Callable(callback),
                RuntimeIntrinsic::StdOptionWithContext,
            ),
            &mut backend,
        )
        .expect("Option::Some skips the callback");
    assert_eq!(calls.get(), 2);
    assert_eq!(
        option_some.try_into_builtin_variant_case(),
        Ok((
            crate::pattern::RuntimeBuiltinVariantCaseIdentity::ResultOk,
            Some(RuntimeValue::String("route".to_owned()))
        ))
    );
}

#[test]
fn native_standalone_context_requires_proof_for_string_but_accepts_content() {
    let context = context_plan(false, false);
    let mut engine = Engine::new(context.plan);
    let mut backend = VmRuntimePureCallBackend::default();

    let missing_proof = engine.evaluate_expr_with_backend(
        &context_expression(
            &engine.plan,
            context.types.result_arc_error,
            context.types.result_string_error,
            RuntimeValue::result_err(RuntimeValue::String("cause".to_owned())),
            STRING,
            RuntimeValue::String("plain text".to_owned()),
            RuntimeIntrinsic::StdResultContext,
        ),
        &mut backend,
    );
    assert!(matches!(
        missing_proof,
        Err(RuntimeEvalError::DialogueContentConstruction(_))
    ));

    let direct_content = expected_content(context.artifact, context.proof, "already Content");
    let result = engine
        .evaluate_expr_with_backend(
            &context_expression(
                &engine.plan,
                context.types.result_arc_error,
                context.types.result_string_error,
                RuntimeValue::result_err(RuntimeValue::String("cause".to_owned())),
                context.types.content,
                direct_content.clone().into_runtime_value(),
                RuntimeIntrinsic::StdResultContext,
            ),
            &mut backend,
        )
        .expect("typed Content passes without an execution proof");
    assert_eq!(context_error(result).message(), &direct_content);
}
