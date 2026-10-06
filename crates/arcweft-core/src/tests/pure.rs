use std::sync::Arc;

use crate::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
use crate::plan::{
    RuntimeCallArgumentSeed, RuntimeDialogueContentSlotSeed,
    RuntimeDialogueContentTemplateManifestSeed, RuntimeDialogueValueRole, RuntimeExprSeed,
    RuntimeExprSeedKind, RuntimeFormatContentOperandSeed, RuntimeFunctionInputBindingSeed,
    RuntimeFunctionInputSource, RuntimeFunctionSiteSeedId, RuntimeLocalDeclarationSeed,
    RuntimeLocalReadSeed, RuntimeLocalSeedId, RuntimePatternSeed, RuntimePatternSeedKind,
    RuntimePlan, RuntimePlanBuilder, RuntimePlanSequenceKind, RuntimePlanTypeProjection,
    RuntimePlanTypeSeed, RuntimePureHelperId, RuntimePureHelperOrigin, RuntimePureHelperSeed,
    RuntimePureInputType, RuntimePureOutputType, RuntimeReceiverMode, RuntimeTraitMethodId,
    RuntimeTraitMethodIdentity, RuntimeTraitMethodSeed,
};
use crate::pure::{
    AotPureFunctionBackend, PureFunctionBackend, PureFunctionBackendKind, PureFunctionRequest,
    RuntimeI64Args, RuntimePureCallBackend, RuntimePureHelperRef, VmPureFunctionBackend,
    VmPureFunctionScratch, VmRuntimePureCallBackend, compare_pure_function_backend,
};
use crate::scope::RuntimeScopeIdentity;
use crate::value::{
    RuntimeBinaryOp, RuntimeCallArgumentMode, RuntimeCallTarget, RuntimeDialogueContentValue,
    RuntimeDialogueFormattedOutcome, RuntimeDialogueFormattedSuccess, RuntimeDialogueOpaqueRole,
    RuntimeEvalError, RuntimeExprKind, RuntimeFmtParameterId, RuntimeFormatContext,
    RuntimeLocalReadMode, RuntimeSeq, RuntimeSignedIntWidth, RuntimeStandardMapFamily,
    RuntimeStandardMapOperandOrder, RuntimeValue,
};
use arcweft_id::{DeclarationName, LocaleTag};

#[test]
fn pure_value_backend_moves_an_affine_need_argument_into_its_result() {
    let unit = RuntimeCheckedType::Unit.semantic_identity_digest();
    let need = RuntimeSemanticTypeId::from_bytes([0x91; 32]);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(need, RuntimePlanTypeProjection::Need(unit)),
            ],
            [RuntimeLocalDeclarationSeed::new(need)],
        )
        .expect("affine pure input type");
    let input = admission.local_ids()[0].clone();
    builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            name: "move_need".to_owned(),
            inputs: Box::new([input.clone()]),
            input_abi: vec![RuntimePureInputType::Value],
            output_abi: RuntimePureOutputType::Value,
            body: RuntimeExprSeed::new(
                need,
                RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                    input,
                    RuntimeLocalReadMode::Move,
                )),
            ),
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .expect("move-only pure helper");
    let plan = Arc::new(builder.finish().expect("sealed pure helper"));
    let helper =
        RuntimePureHelperRef::resolve(&plan, plan.pure_helpers()[0].id).expect("helper reference");
    let value = RuntimeValue::NeedHandle(crate::tests::reusable_need("need.pure.affine"));
    assert!(!value.ownership().permits_copy());
    let mut backend = VmRuntimePureCallBackend::default();
    assert_eq!(
        backend
            .call_values(helper, vec![value])
            .expect("affine argument transfers into pure result"),
        RuntimeValue::NeedHandle(crate::tests::reusable_need("need.pure.affine"))
    );
}

#[test]
fn pure_collect_intrinsic_moves_affine_sequence_items() {
    let unit = RuntimeCheckedType::Unit.semantic_identity_digest();
    let need = RuntimeSemanticTypeId::from_bytes([0x92; 32]);
    let vector = RuntimeSemanticTypeId::from_bytes([0x93; 32]);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(need, RuntimePlanTypeProjection::Need(unit)),
                RuntimePlanTypeSeed::new(
                    vector,
                    RuntimePlanTypeProjection::Sequence {
                        kind: RuntimePlanSequenceKind::Vec,
                        item: need,
                    },
                ),
            ],
            [RuntimeLocalDeclarationSeed::new(vector)],
        )
        .expect("affine sequence input type");
    let input = admission.local_ids()[0].clone();
    builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            name: "collect_affine".to_owned(),
            inputs: Box::new([input.clone()]),
            input_abi: vec![RuntimePureInputType::Value],
            output_abi: RuntimePureOutputType::Value,
            body: RuntimeExprSeed::new(
                vector,
                RuntimeExprSeedKind::Call {
                    callee: RuntimeCallTarget::intrinsic(
                        crate::value::RuntimeIntrinsic::CoreIterCollect,
                    ),
                    args: Box::new([RuntimeCallArgumentSeed::new(
                        RuntimeExprSeed::new(
                            vector,
                            RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                                input,
                                RuntimeLocalReadMode::Move,
                            )),
                        ),
                        RuntimeCallArgumentMode::Value,
                        0,
                    )]),
                },
            ),
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .expect("collect helper admits");
    let plan = Arc::new(builder.finish().expect("collect helper seals"));
    let helper = RuntimePureHelperRef::resolve(&plan, plan.pure_helpers()[0].id)
        .expect("collect helper reference");
    let item = RuntimeValue::NeedHandle(crate::tests::reusable_need("need.pure.collect"));
    let original = match &item {
        RuntimeValue::NeedHandle(handle) => handle.spec() as *const crate::task::TaskSpec,
        _ => unreachable!(),
    };
    let input = RuntimeValue::Seq(RuntimeSeq::Values(vec![item]));
    let result = VmRuntimePureCallBackend::default()
        .call_values(helper, vec![input])
        .expect("pure intrinsic consumes sequence");
    let RuntimeValue::Seq(RuntimeSeq::Values(items)) = result else {
        panic!("collect returns its affine sequence")
    };
    let [RuntimeValue::NeedHandle(item)] = items.as_slice() else {
        panic!("collect retains one Need")
    };
    assert_eq!(item.spec() as *const crate::task::TaskSpec, original);
}

#[test]
fn pure_format_content_uses_selected_ambient_locale() {
    let int_type =
        RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64).semantic_identity_digest();
    let string_type = RuntimeCheckedType::String.semantic_identity_digest();
    let content_owner = RuntimeDialogueOpaqueRole::Content.exact_owner();
    let content_type = content_owner.semantic_identity();
    let template = crate::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0).unwrap();
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(
                    int_type,
                    RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64),
                ),
                RuntimePlanTypeSeed::new(string_type, RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(
                    content_type,
                    RuntimePlanTypeProjection::Opaque {
                        producer: content_owner.producer().clone(),
                        admission: content_owner.admission(),
                        value_class: content_owner.value_class(),
                        persistence: content_owner.persistence(),
                        arguments: Box::new([]),
                    },
                ),
            ],
            [RuntimeLocalDeclarationSeed::new(int_type)],
        )
        .unwrap();
    let receiver_local = admission.local_ids()[0].clone();
    builder
        .register_plain_text_context_template_seed(RuntimeDialogueContentTemplateManifestSeed {
            id: template,
            digest: crate::entry::RuntimeDialogueContentTemplateDigest::from_bytes([0x32; 32]),
            slots: vec![RuntimeDialogueContentSlotSeed {
                slot: crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0).unwrap(),
                role: RuntimeDialogueValueRole::Formatted,
                semantic_type: content_type,
            }]
            .into_boxed_slice(),
            effects: Box::new([]),
        })
        .unwrap();
    let body = RuntimeExprSeed::format_content(
        content_type,
        template,
        None,
        None,
        false,
        [
            RuntimeFormatContentOperandSeed::new(
                RuntimeFmtParameterId::Value,
                RuntimeExprSeed::new(
                    int_type,
                    RuntimeExprSeedKind::Value(RuntimeValue::i64(12345)),
                ),
            ),
            RuntimeFormatContentOperandSeed::new(
                RuntimeFmtParameterId::Style,
                RuntimeExprSeed::new(
                    string_type,
                    RuntimeExprSeedKind::Value(RuntimeValue::String("number".to_owned())),
                ),
            ),
        ],
    );
    let method = builder
        .push_trait_method_seed(RuntimeTraitMethodSeed {
            identity: RuntimeTraitMethodIdentity {
                impl_id: 0,
                trait_id: Some(0),
                witness: Some(0),
                trait_name: Some("DisplayText".to_owned()),
                self_type: "I64".to_owned(),
                method_name: "display_text".to_owned(),
                monomorph_label: "I64::display_text".to_owned(),
            },
            receiver: RuntimeReceiverMode::Owned,
            inputs: Box::new([receiver_local.clone()]),
            input_abi: vec![RuntimePureInputType::I64],
            output_abi: RuntimePureOutputType::Value,
            body: RuntimeExprSeed::format_content(
                content_type,
                template,
                None,
                None,
                false,
                [
                    RuntimeFormatContentOperandSeed::new(
                        RuntimeFmtParameterId::Value,
                        RuntimeExprSeed::new(
                            int_type,
                            RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                                receiver_local,
                                RuntimeLocalReadMode::Copy,
                            )),
                        ),
                    ),
                    RuntimeFormatContentOperandSeed::new(
                        RuntimeFmtParameterId::Style,
                        RuntimeExprSeed::new(
                            string_type,
                            RuntimeExprSeedKind::Value(RuntimeValue::String("number".to_owned())),
                        ),
                    ),
                ],
            ),
        })
        .unwrap();
    builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            name: "localized_number".to_owned(),
            inputs: Box::new([]),
            input_abi: vec![],
            output_abi: RuntimePureOutputType::Value,
            body,
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .unwrap();
    builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            name: "localized_number_via_trait".to_owned(),
            inputs: Box::new([]),
            input_abi: vec![],
            output_abi: RuntimePureOutputType::Value,
            body: RuntimeExprSeed::new(
                content_type,
                RuntimeExprSeedKind::TraitCall {
                    callable: method,
                    receiver: Box::new(RuntimeExprSeed::new(
                        int_type,
                        RuntimeExprSeedKind::Value(RuntimeValue::i64(12345)),
                    )),
                    args: Box::new([]),
                },
            ),
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .unwrap();
    let mut plan = builder.finish().unwrap();
    plan.bind_artifact(
        crate::effect::RuntimeArtifactFingerprint::try_from_bytes([0x4a; 32]).unwrap(),
    )
    .unwrap();
    let plan = Arc::new(plan);
    let helper = plan.pure_helpers()[0].id;
    let context = RuntimeFormatContext::new(LocaleTag::try_new("de-DE").unwrap());
    let request = PureFunctionRequest::try_new(Arc::clone(&plan), helper, [])
        .unwrap()
        .with_format_context(context.clone());
    let evaluated = VmPureFunctionBackend.evaluate(&request).unwrap();
    let trait_request =
        PureFunctionRequest::try_new(Arc::clone(&plan), plan.pure_helpers()[1].id, [])
            .unwrap()
            .with_format_context(context.clone());
    let trait_evaluated = VmPureFunctionBackend.evaluate(&trait_request).unwrap();
    assert_eq!(trait_evaluated.value, evaluated.value);
    let content = RuntimeDialogueContentValue::try_from_runtime_value(&evaluated.value).unwrap();
    let value = content
        .binding(crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0).unwrap())
        .and_then(|binding| binding.formatted())
        .unwrap();
    assert_eq!(
        value.outcome(),
        &RuntimeDialogueFormattedOutcome::Success {
            value: RuntimeDialogueFormattedSuccess::Text("12.345".to_owned()),
            color: None,
        }
    );
    let mut scratch = VmPureFunctionScratch::default();
    scratch.set_format_context(context);
    assert_eq!(
        scratch.evaluate_values(&plan, helper, vec![]).unwrap(),
        evaluated.value
    );
}

const I64_SEMANTIC_MARKER: u8 = 1;
const BOOL_SEMANTIC_MARKER: u8 = 2;
const FUNCTION_SEMANTIC_MARKER: u8 = 3;

fn semantic_type(marker: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([marker; 32])
}

fn i64_semantic_type() -> RuntimeSemanticTypeId {
    semantic_type(I64_SEMANTIC_MARKER)
}

fn bool_semantic_type() -> RuntimeSemanticTypeId {
    semantic_type(BOOL_SEMANTIC_MARKER)
}

fn scalar_type_seeds() -> [RuntimePlanTypeSeed; 2] {
    [
        RuntimePlanTypeSeed::new(
            i64_semantic_type(),
            RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64),
        ),
        RuntimePlanTypeSeed::new(bool_semantic_type(), RuntimePlanTypeProjection::Bool),
    ]
}

fn i64_value(value: i64) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        i64_semantic_type(),
        RuntimeExprSeedKind::Value(RuntimeValue::i64(value)),
    )
}

fn i64_local(local: RuntimeLocalSeedId) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        i64_semantic_type(),
        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(local, RuntimeLocalReadMode::Copy)),
    )
}

fn i64_binary(lhs: RuntimeExprSeed, op: RuntimeBinaryOp, rhs: RuntimeExprSeed) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        i64_semantic_type(),
        RuntimeExprSeedKind::Binary {
            lhs: Box::new(lhs),
            op,
            rhs: Box::new(rhs),
        },
    )
}

fn bool_binary(lhs: RuntimeExprSeed, op: RuntimeBinaryOp, rhs: RuntimeExprSeed) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        bool_semantic_type(),
        RuntimeExprSeedKind::Binary {
            lhs: Box::new(lhs),
            op,
            rhs: Box::new(rhs),
        },
    )
}

fn standard_map_seed(
    family: RuntimeStandardMapFamily,
    order: RuntimeStandardMapOperandOrder,
    function_ty: RuntimeSemanticTypeId,
    source_ty: RuntimeSemanticTypeId,
    result_ty: RuntimeSemanticTypeId,
    site: RuntimeFunctionSiteSeedId,
    source: RuntimeValue,
) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        result_ty,
        RuntimeExprSeedKind::StandardMap {
            family,
            order,
            mapping: Box::new(RuntimeExprSeed::new(
                function_ty,
                RuntimeExprSeedKind::Function {
                    site,
                    captures: Box::new([]),
                },
            )),
            source: Box::new(RuntimeExprSeed::new(
                source_ty,
                RuntimeExprSeedKind::Value(source),
            )),
        },
    )
}

struct StandardMapPureCase {
    helper: RuntimePureHelperId,
    expected: RuntimeValue,
    callback_count: usize,
}

fn standard_map_source() -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::values(vec![
        RuntimeValue::i64(1),
        RuntimeValue::i64(2),
        RuntimeValue::i64(3),
    ]))
}

fn standard_map_pure_plan() -> (Arc<RuntimePlan>, Vec<StandardMapPureCase>) {
    let item_ty = i64_semantic_type();
    let error_ty = semantic_type(11);
    let function_ty = semantic_type(12);
    let vec_ty = semantic_type(13);
    let seq_ty = semantic_type(14);
    let array_ty = semantic_type(15);
    let slice_ty = semantic_type(16);
    let option_ty = semantic_type(17);
    let result_ty = semantic_type(18);
    let item_payload_ty = semantic_type(19);
    let error_payload_ty = semantic_type(20);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(
                    item_ty,
                    RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64),
                ),
                RuntimePlanTypeSeed::new(error_ty, RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(
                    function_ty,
                    RuntimePlanTypeProjection::Function {
                        contract: Default::default(),
                        parameters: Box::new([item_ty]),
                        result: item_ty,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    vec_ty,
                    RuntimePlanTypeProjection::Sequence {
                        kind: RuntimePlanSequenceKind::Vec,
                        item: item_ty,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    seq_ty,
                    RuntimePlanTypeProjection::Sequence {
                        kind: RuntimePlanSequenceKind::Seq,
                        item: item_ty,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    array_ty,
                    RuntimePlanTypeProjection::Array {
                        item: item_ty,
                        length: 3.into(),
                    },
                ),
                RuntimePlanTypeSeed::new(
                    slice_ty,
                    RuntimePlanTypeProjection::Sequence {
                        kind: RuntimePlanSequenceKind::Slice,
                        item: item_ty,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    item_payload_ty,
                    RuntimePlanTypeProjection::Tuple(Box::new([item_ty])),
                ),
                RuntimePlanTypeSeed::new(
                    error_payload_ty,
                    RuntimePlanTypeProjection::Tuple(Box::new([error_ty])),
                ),
                RuntimePlanTypeSeed::new(
                    option_ty,
                    RuntimePlanTypeProjection::Option {
                        item: item_ty,
                        some_payload: item_payload_ty,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    result_ty,
                    RuntimePlanTypeProjection::Result {
                        value: item_ty,
                        error: error_ty,
                        value_payload: item_payload_ty,
                        error_payload: error_payload_ty,
                    },
                ),
            ],
            (0..6).map(|_| RuntimeLocalDeclarationSeed::new(item_ty)),
        )
        .expect("standard map type graph");

    let callback_body = |local: RuntimeLocalSeedId| {
        i64_binary(i64_local(local), RuntimeBinaryOp::Add, i64_value(1))
    };
    let callback_sites = admission
        .local_ids()
        .iter()
        .cloned()
        .map(|local| {
            builder
                .push_function_site_seed(
                    crate::plan::RuntimeFunctionSemanticRole::Closure,
                    [RuntimeFunctionInputBindingSeed {
                        ownership: Default::default(),
                        unrestricted_bindings: Box::new([]),
                        source: RuntimeFunctionInputSource::Parameter {
                            position: 0,
                            passing: crate::plan::RuntimeFunctionParameterPassing::Value,
                        },
                        input_local: local.clone(),
                        pattern: RuntimePatternSeed::new(
                            i64_semantic_type(),
                            RuntimePatternSeedKind::Bind {
                                mutable: false,
                                local: local.clone(),
                            },
                        ),
                    }],
                    callback_body(local),
                )
                .expect("standard map callback site")
        })
        .collect::<Vec<_>>();

    let cases = [
        (
            RuntimeStandardMapFamily::Vec,
            RuntimeStandardMapOperandOrder::MappingThenReceiver,
            vec_ty,
            vec_ty,
            standard_map_source(),
            RuntimeValue::Seq(RuntimeSeq::values(vec![
                RuntimeValue::i64(2),
                RuntimeValue::i64(3),
                RuntimeValue::i64(4),
            ])),
            3,
        ),
        (
            RuntimeStandardMapFamily::Seq,
            RuntimeStandardMapOperandOrder::ReceiverThenMapping,
            seq_ty,
            seq_ty,
            standard_map_source(),
            RuntimeValue::Seq(RuntimeSeq::values(vec![
                RuntimeValue::i64(2),
                RuntimeValue::i64(3),
                RuntimeValue::i64(4),
            ])),
            3,
        ),
        (
            RuntimeStandardMapFamily::Array,
            RuntimeStandardMapOperandOrder::MappingThenReceiver,
            array_ty,
            array_ty,
            standard_map_source(),
            RuntimeValue::Seq(RuntimeSeq::values(vec![
                RuntimeValue::i64(2),
                RuntimeValue::i64(3),
                RuntimeValue::i64(4),
            ])),
            3,
        ),
        (
            RuntimeStandardMapFamily::Slice,
            RuntimeStandardMapOperandOrder::ReceiverThenMapping,
            slice_ty,
            vec_ty,
            standard_map_source(),
            RuntimeValue::Seq(RuntimeSeq::values(vec![
                RuntimeValue::i64(2),
                RuntimeValue::i64(3),
                RuntimeValue::i64(4),
            ])),
            3,
        ),
        (
            RuntimeStandardMapFamily::Option,
            RuntimeStandardMapOperandOrder::MappingThenReceiver,
            option_ty,
            option_ty,
            RuntimeValue::option_some(RuntimeValue::i64(7)),
            RuntimeValue::option_some(RuntimeValue::i64(8)),
            1,
        ),
        (
            RuntimeStandardMapFamily::Option,
            RuntimeStandardMapOperandOrder::ReceiverThenMapping,
            option_ty,
            option_ty,
            RuntimeValue::option_none(),
            RuntimeValue::option_none(),
            0,
        ),
        (
            RuntimeStandardMapFamily::Result,
            RuntimeStandardMapOperandOrder::MappingThenReceiver,
            result_ty,
            result_ty,
            RuntimeValue::result_ok(RuntimeValue::i64(9)),
            RuntimeValue::result_ok(RuntimeValue::i64(10)),
            1,
        ),
        (
            RuntimeStandardMapFamily::Result,
            RuntimeStandardMapOperandOrder::ReceiverThenMapping,
            result_ty,
            result_ty,
            RuntimeValue::result_err(RuntimeValue::String("preserve".to_owned())),
            RuntimeValue::result_err(RuntimeValue::String("preserve".to_owned())),
            0,
        ),
    ];

    let mut expectations = Vec::with_capacity(cases.len());
    for (index, (family, order, source_ty, result_ty, source, expected, callback_count)) in
        cases.into_iter().enumerate()
    {
        builder
            .push_pure_helper_seed(RuntimePureHelperSeed {
                name: format!("standard_map_{index}"),
                inputs: Box::new([]),
                input_abi: Vec::new(),
                output_abi: RuntimePureOutputType::Value,
                body: standard_map_seed(
                    family,
                    order,
                    function_ty,
                    source_ty,
                    result_ty,
                    callback_sites[index % callback_sites.len()].clone(),
                    source,
                ),
                scalar_eval_supported: false,
                origin: RuntimePureHelperOrigin::Annotated,
            })
            .expect("standard map helper");
        expectations.push(StandardMapPureCase {
            helper: RuntimePureHelperId(index),
            expected,
            callback_count,
        });
    }

    (
        Arc::new(builder.finish().expect("standard map pure plan")),
        expectations,
    )
}

struct AdmittedHelper {
    plan: Arc<RuntimePlan>,
    helper: RuntimePureHelperId,
}

impl AdmittedHelper {
    fn helper_ref(&self) -> RuntimePureHelperRef<'_> {
        RuntimePureHelperRef::resolve(&self.plan, self.helper).expect("admitted helper")
    }

    fn request(&self, args: impl IntoIterator<Item = RuntimeValue>) -> PureFunctionRequest {
        PureFunctionRequest::try_new(Arc::clone(&self.plan), self.helper, args)
            .expect("well-typed helper request")
    }
}

fn admit_i64_helper(
    name: &str,
    arity: usize,
    body: impl FnOnce(&[RuntimeLocalSeedId]) -> RuntimeExprSeed,
) -> AdmittedHelper {
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            scalar_type_seeds(),
            (0..arity).map(|_| RuntimeLocalDeclarationSeed::new(i64_semantic_type())),
        )
        .expect("semantic helper inputs");
    builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            name: name.to_owned(),
            inputs: admission.local_ids().to_vec().into_boxed_slice(),
            input_abi: vec![RuntimePureInputType::I64; arity],
            output_abi: RuntimePureOutputType::I64,
            body: body(admission.local_ids()),
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .expect("typed helper admission");
    let plan = Arc::new(builder.finish().expect("sealed helper plan"));
    let helper = plan.pure_helpers()[0].id;
    AdmittedHelper { plan, helper }
}

fn admitted_add_helper() -> AdmittedHelper {
    admit_i64_helper("add", 2, |inputs| {
        i64_binary(
            i64_local(inputs[0].clone()),
            RuntimeBinaryOp::Add,
            i64_local(inputs[1].clone()),
        )
    })
}

#[test]
fn pure_request_is_qualified_by_the_admitted_plan_and_helper() {
    let helper = admitted_add_helper();
    let request = helper.request([RuntimeValue::i64(3), RuntimeValue::i64(4)]);

    assert!(Arc::ptr_eq(request.plan(), &helper.plan));
    assert_eq!(request.helper_id(), helper.helper);
    assert_eq!(
        request
            .bindings()
            .iter()
            .map(|binding| binding.local)
            .collect::<Vec<_>>(),
        helper.plan.pure_helpers()[0].input_locals.as_ref()
    );

    let result = VmPureFunctionBackend
        .evaluate(&request)
        .expect("plan-qualified VM evaluation");
    assert_eq!(result.backend, PureFunctionBackendKind::Vm);
    assert_eq!(result.value, RuntimeValue::i64(7));
    assert_eq!(result.stats.evaluated_binary_ops, 1);
}

#[test]
fn pure_request_rejects_a_value_outside_the_input_local_type() {
    let helper = admitted_add_helper();
    let input = helper.plan.pure_helpers()[0].input_locals[0];
    let input_ty = helper
        .plan
        .local_declarations()
        .get(input)
        .expect("input declaration")
        .ty();

    assert_eq!(
        PureFunctionRequest::try_new(
            Arc::clone(&helper.plan),
            helper.helper,
            [
                RuntimeValue::String("wrong".to_owned()),
                RuntimeValue::i64(1)
            ],
        ),
        Err(RuntimeEvalError::InvalidExpressionType(input_ty))
    );
}

#[test]
fn runtime_backend_accepts_only_a_plan_qualified_helper_handle() {
    let helper = admitted_add_helper();
    let mut backend = VmRuntimePureCallBackend::default();

    let value = backend
        .call_i64(helper.helper_ref(), RuntimeI64Args::new([9, 4, 0, 0], 2))
        .expect("runtime helper call");

    assert_eq!(value, Some(13));
    assert_eq!(backend.stats().pure_calls, 1);
    assert_eq!(backend.stats().vm_calls, 1);
    assert_eq!(backend.stats().arg_stack_packs, 1);
}

#[test]
fn runtime_backend_flat_batch_reuses_the_same_plan_qualified_helper() {
    let helper = admit_i64_helper("multiply", 2, |inputs| {
        i64_binary(
            i64_local(inputs[0].clone()),
            RuntimeBinaryOp::Mul,
            i64_local(inputs[1].clone()),
        )
    });
    let mut backend = VmRuntimePureCallBackend::default();
    let mut output = [0; 3];

    backend
        .call_i64_flat_batch(helper.helper_ref(), &[2, 3, 4, 5, 6, 7], 2, &mut output)
        .expect("typed flat batch");

    assert_eq!(output, [6, 20, 42]);
    assert_eq!(backend.stats().flat_batch_calls, 1);
    assert_eq!(backend.stats().flat_batch_items, 3);
}

#[test]
fn vm_scratch_rebinds_plan_local_inputs_between_calls() {
    let helper = admitted_add_helper();
    let mut scratch = VmPureFunctionScratch::default();

    assert_eq!(
        scratch
            .evaluate_i64_slice(&helper.plan, helper.helper, &[1, 2])
            .expect("first evaluation"),
        RuntimeValue::i64(3)
    );
    assert_eq!(
        scratch
            .evaluate_i64_slice(&helper.plan, helper.helper, &[10, 20])
            .expect("second evaluation"),
        RuntimeValue::i64(30)
    );
}

#[test]
fn aot_plan_uses_the_helpers_plan_local_input_coordinates() {
    let helper = admitted_add_helper();
    let request = helper.request([RuntimeValue::i64(0), RuntimeValue::i64(0)]);
    let input_locals = helper.plan.pure_helpers()[0].input_locals.clone();
    let plan = AotPureFunctionBackend::new()
        .compile_i64_with_inputs(&request, input_locals.iter().copied())
        .expect("typed AOT compilation");

    let (value, stats) = plan
        .call_with_inputs(&[12, 30])
        .expect("typed AOT invocation");

    assert_eq!(value, 42);
    assert_eq!(stats.evaluated_binary_ops, 1);
}

#[test]
fn aot_rejects_a_consuming_local_read_that_vm_executes() {
    let helper = admit_i64_helper("consuming_input", 1, |inputs| {
        RuntimeExprSeed::new(
            i64_semantic_type(),
            RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                inputs[0].clone(),
                RuntimeLocalReadMode::Move,
            )),
        )
    });
    let request = helper.request([RuntimeValue::i64(9)]);
    let input_locals = helper.plan.pure_helpers()[0].input_locals.clone();

    assert!(
        AotPureFunctionBackend::new()
            .compile_i64_with_inputs(&request, input_locals.iter().copied())
            .is_err()
    );
    assert_eq!(
        VmPureFunctionBackend
            .evaluate(&request)
            .expect("VM consumes the local")
            .value,
        RuntimeValue::i64(9)
    );
}

#[test]
fn aot_and_vm_compare_the_same_admitted_helper() {
    let helper = admit_i64_helper("conditional", 2, |inputs| {
        RuntimeExprSeed::new(
            i64_semantic_type(),
            RuntimeExprSeedKind::If {
                condition: Box::new(bool_binary(
                    i64_local(inputs[0].clone()),
                    RuntimeBinaryOp::Ge,
                    i64_local(inputs[1].clone()),
                )),
                then_expr: Box::new(i64_binary(
                    i64_local(inputs[0].clone()),
                    RuntimeBinaryOp::Mul,
                    i64_value(2),
                )),
                else_expr: Box::new(i64_local(inputs[1].clone())),
            },
        )
    });
    let request = helper.request([RuntimeValue::i64(7), RuntimeValue::i64(4)]);

    let comparison = compare_pure_function_backend(
        &VmPureFunctionBackend,
        &AotPureFunctionBackend::new(),
        &request,
    )
    .expect("VM/AOT comparison");

    assert!(comparison.matches_vm);
    assert_eq!(comparison.vm.value, RuntimeValue::i64(14));
    assert_eq!(comparison.candidate.value, RuntimeValue::i64(14));
}

#[test]
fn named_scope_is_preserved_by_the_vm_and_aot_pure_backends() {
    let helper = admit_i64_helper("named_scope", 1, |inputs| {
        RuntimeExprSeed::new(
            i64_semantic_type(),
            RuntimeExprSeedKind::Scope {
                identity: RuntimeScopeIdentity::Named(
                    DeclarationName::try_new("window").expect("valid scope name"),
                ),
                body: Box::new(i64_binary(
                    i64_local(inputs[0].clone()),
                    RuntimeBinaryOp::Add,
                    i64_value(1),
                )),
            },
        )
    });
    assert!(matches!(
        helper.plan.pure_helpers()[0].expr.kind(),
        RuntimeExprKind::Scope { identity, .. }
            if identity.name().is_some_and(|name| name.as_str() == "window")
    ));
    let request = helper.request([RuntimeValue::i64(41)]);

    let comparison = compare_pure_function_backend(
        &VmPureFunctionBackend,
        &AotPureFunctionBackend::new(),
        &request,
    )
    .expect("both pure evaluators retain the named lexical frame");

    assert!(comparison.matches_vm);
    assert_eq!(comparison.vm.value, RuntimeValue::i64(42));
    assert_eq!(comparison.candidate.value, RuntimeValue::i64(42));
}

#[test]
fn structured_closure_captures_the_exact_owning_plan() {
    let function_semantic_type = semantic_type(FUNCTION_SEMANTIC_MARKER);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                scalar_type_seeds()[0].clone(),
                RuntimePlanTypeSeed::new(
                    function_semantic_type,
                    RuntimePlanTypeProjection::Function {
                        contract: Default::default(),
                        parameters: Box::new([i64_semantic_type()]),
                        result: i64_semantic_type(),
                    },
                ),
            ],
            [
                RuntimeLocalDeclarationSeed::new(i64_semantic_type()),
                RuntimeLocalDeclarationSeed::new(function_semantic_type),
                RuntimeLocalDeclarationSeed::new(i64_semantic_type()),
                RuntimeLocalDeclarationSeed::new(i64_semantic_type()),
                RuntimeLocalDeclarationSeed::new(i64_semantic_type()),
            ],
        )
        .expect("closure type graph");
    let captured = admission.local_ids()[0].clone();
    let closure_binding = admission.local_ids()[1].clone();
    let parameter = admission.local_ids()[2].clone();
    let capture_input = admission.local_ids()[3].clone();
    let parameter_input = admission.local_ids()[4].clone();
    let site = builder
        .push_function_site_seed(
            crate::plan::RuntimeFunctionSemanticRole::Closure,
            [
                RuntimeFunctionInputBindingSeed {
                    ownership: Default::default(),
                    unrestricted_bindings: Box::new([]),
                    source: RuntimeFunctionInputSource::Capture { position: 0 },
                    input_local: capture_input,
                    pattern: RuntimePatternSeed::new(
                        i64_semantic_type(),
                        RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local: captured.clone(),
                        },
                    ),
                },
                RuntimeFunctionInputBindingSeed {
                    ownership: Default::default(),
                    unrestricted_bindings: Box::new([]),
                    source: RuntimeFunctionInputSource::Parameter {
                        position: 0,
                        passing: crate::plan::RuntimeFunctionParameterPassing::Value,
                    },
                    input_local: parameter_input,
                    pattern: RuntimePatternSeed::new(
                        i64_semantic_type(),
                        RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local: parameter.clone(),
                        },
                    ),
                },
            ],
            i64_binary(
                i64_local(parameter),
                RuntimeBinaryOp::Add,
                i64_local(captured.clone()),
            ),
        )
        .expect("typed closure site");
    let closure = RuntimeExprSeed::new(
        function_semantic_type,
        RuntimeExprSeedKind::Function {
            site,
            captures: Box::new([i64_local(captured.clone())]),
        },
    );
    let apply = RuntimeExprSeed::new(
        i64_semantic_type(),
        RuntimeExprSeedKind::Apply {
            callee: Box::new(RuntimeExprSeed::new(
                function_semantic_type,
                RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                    closure_binding.clone(),
                    RuntimeLocalReadMode::Copy,
                )),
            )),
            args: Box::new([RuntimeCallArgumentSeed::new(
                i64_value(3),
                RuntimeCallArgumentMode::Value,
                0,
            )]),
        },
    );
    builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            name: "captured_add".to_owned(),
            inputs: Box::new([captured]),
            input_abi: vec![RuntimePureInputType::I64],
            output_abi: RuntimePureOutputType::I64,
            body: RuntimeExprSeed::new(
                i64_semantic_type(),
                RuntimeExprSeedKind::Let {
                    binding: closure_binding,
                    expr: Box::new(closure),
                    body: Box::new(apply),
                },
            ),
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .expect("closure helper admission");
    let plan = Arc::new(builder.finish().expect("sealed closure plan"));
    let helper = plan.pure_helpers()[0].id;

    let value = VmPureFunctionBackend
        .evaluate(
            &PureFunctionRequest::try_new(Arc::clone(&plan), helper, [RuntimeValue::i64(4)])
                .expect("closure request"),
        )
        .expect("closure evaluation")
        .value;

    assert_eq!(value, RuntimeValue::i64(7));
}

#[test]
fn structured_function_input_tuple_pattern_binds_body_locals() {
    let tuple_semantic_type = semantic_type(30);
    let function_semantic_type = semantic_type(31);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                scalar_type_seeds()[0].clone(),
                RuntimePlanTypeSeed::new(
                    tuple_semantic_type,
                    RuntimePlanTypeProjection::Tuple(Box::new([
                        i64_semantic_type(),
                        i64_semantic_type(),
                    ])),
                ),
                RuntimePlanTypeSeed::new(
                    function_semantic_type,
                    RuntimePlanTypeProjection::Function {
                        contract: Default::default(),
                        parameters: Box::new([tuple_semantic_type]),
                        result: i64_semantic_type(),
                    },
                ),
            ],
            [
                RuntimeLocalDeclarationSeed::new(function_semantic_type),
                RuntimeLocalDeclarationSeed::new(tuple_semantic_type),
                RuntimeLocalDeclarationSeed::new(i64_semantic_type()),
                RuntimeLocalDeclarationSeed::new(i64_semantic_type()),
            ],
        )
        .expect("tuple-pattern type graph");
    let closure_binding = admission.local_ids()[0].clone();
    let input_local = admission.local_ids()[1].clone();
    let first = admission.local_ids()[2].clone();
    let second = admission.local_ids()[3].clone();
    let site = builder
        .push_function_site_seed(
            crate::plan::RuntimeFunctionSemanticRole::Closure,
            [RuntimeFunctionInputBindingSeed {
                ownership: Default::default(),
                unrestricted_bindings: Box::new([]),
                source: RuntimeFunctionInputSource::Parameter {
                    position: 0,
                    passing: crate::plan::RuntimeFunctionParameterPassing::Value,
                },
                input_local,
                pattern: RuntimePatternSeed::new(
                    tuple_semantic_type,
                    RuntimePatternSeedKind::Tuple(Box::new([
                        RuntimePatternSeed::new(
                            i64_semantic_type(),
                            RuntimePatternSeedKind::Bind {
                                mutable: false,
                                local: first.clone(),
                            },
                        ),
                        RuntimePatternSeed::new(
                            i64_semantic_type(),
                            RuntimePatternSeedKind::Bind {
                                mutable: false,
                                local: second.clone(),
                            },
                        ),
                    ])),
                ),
            }],
            i64_binary(i64_local(first), RuntimeBinaryOp::Add, i64_local(second)),
        )
        .expect("tuple-pattern function site");
    let closure = RuntimeExprSeed::new(
        function_semantic_type,
        RuntimeExprSeedKind::Function {
            site,
            captures: Box::new([]),
        },
    );
    let apply = RuntimeExprSeed::new(
        i64_semantic_type(),
        RuntimeExprSeedKind::Apply {
            callee: Box::new(RuntimeExprSeed::new(
                function_semantic_type,
                RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                    closure_binding.clone(),
                    RuntimeLocalReadMode::Copy,
                )),
            )),
            args: Box::new([RuntimeCallArgumentSeed::new(
                RuntimeExprSeed::new(
                    tuple_semantic_type,
                    RuntimeExprSeedKind::Value(RuntimeValue::Tuple(vec![
                        RuntimeValue::i64(2),
                        RuntimeValue::i64(5),
                    ])),
                ),
                RuntimeCallArgumentMode::Value,
                0,
            )]),
        },
    );
    builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            name: "tuple_pattern_add".to_owned(),
            inputs: Box::new([]),
            input_abi: vec![],
            output_abi: RuntimePureOutputType::I64,
            body: RuntimeExprSeed::new(
                i64_semantic_type(),
                RuntimeExprSeedKind::Let {
                    binding: closure_binding,
                    expr: Box::new(closure),
                    body: Box::new(apply),
                },
            ),
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .expect("tuple-pattern helper admission");
    let plan = Arc::new(builder.finish().expect("sealed tuple-pattern plan"));
    let result = VmPureFunctionBackend
        .evaluate(
            &PureFunctionRequest::try_new(Arc::clone(&plan), plan.pure_helpers()[0].id, [])
                .expect("tuple-pattern request"),
        )
        .expect("tuple-pattern evaluation");
    assert_eq!(result.value, RuntimeValue::i64(7));
}

#[test]
fn structured_function_input_sequence_rest_binds_one_logical_tail() {
    let sequence_semantic_type = semantic_type(32);
    let function_semantic_type = semantic_type(33);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                scalar_type_seeds()[0].clone(),
                RuntimePlanTypeSeed::new(
                    sequence_semantic_type,
                    RuntimePlanTypeProjection::Sequence {
                        kind: RuntimePlanSequenceKind::Seq,
                        item: i64_semantic_type(),
                    },
                ),
                RuntimePlanTypeSeed::new(
                    function_semantic_type,
                    RuntimePlanTypeProjection::Function {
                        contract: Default::default(),
                        parameters: Box::new([sequence_semantic_type]),
                        result: i64_semantic_type(),
                    },
                ),
            ],
            [
                RuntimeLocalDeclarationSeed::new(function_semantic_type),
                RuntimeLocalDeclarationSeed::new(sequence_semantic_type),
                RuntimeLocalDeclarationSeed::new(i64_semantic_type()),
                RuntimeLocalDeclarationSeed::new(sequence_semantic_type),
            ],
        )
        .expect("sequence-rest type graph");
    let closure_binding = admission.local_ids()[0].clone();
    let input_local = admission.local_ids()[1].clone();
    let selected = admission.local_ids()[2].clone();
    let tail = admission.local_ids()[3].clone();
    let site = builder
        .push_function_site_seed(
            crate::plan::RuntimeFunctionSemanticRole::Closure,
            [RuntimeFunctionInputBindingSeed {
                ownership: Default::default(),
                unrestricted_bindings: Box::new([]),
                source: RuntimeFunctionInputSource::Parameter {
                    position: 0,
                    passing: crate::plan::RuntimeFunctionParameterPassing::Value,
                },
                input_local,
                pattern: RuntimePatternSeed::new(
                    sequence_semantic_type,
                    RuntimePatternSeedKind::Sequence {
                        items: Box::new([
                            RuntimePatternSeed::new(
                                i64_semantic_type(),
                                RuntimePatternSeedKind::Discard,
                            ),
                            RuntimePatternSeed::new(
                                i64_semantic_type(),
                                RuntimePatternSeedKind::Bind {
                                    mutable: false,
                                    local: selected.clone(),
                                },
                            ),
                        ]),
                        rest: crate::plan::RuntimePatternRestSeed::Bind(tail),
                    },
                ),
            }],
            i64_local(selected),
        )
        .expect("sequence-rest function site");
    let closure = RuntimeExprSeed::new(
        function_semantic_type,
        RuntimeExprSeedKind::Function {
            site,
            captures: Box::new([]),
        },
    );
    let apply = RuntimeExprSeed::new(
        i64_semantic_type(),
        RuntimeExprSeedKind::Apply {
            callee: Box::new(RuntimeExprSeed::new(
                function_semantic_type,
                RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                    closure_binding.clone(),
                    RuntimeLocalReadMode::Copy,
                )),
            )),
            args: Box::new([RuntimeCallArgumentSeed::new(
                RuntimeExprSeed::new(
                    sequence_semantic_type,
                    RuntimeExprSeedKind::Value(RuntimeValue::Seq(RuntimeSeq::values(vec![
                        RuntimeValue::i64(1),
                        RuntimeValue::i64(7),
                        RuntimeValue::i64(9),
                    ]))),
                ),
                RuntimeCallArgumentMode::Value,
                0,
            )]),
        },
    );
    builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            name: "sequence_rest_select".to_owned(),
            inputs: Box::new([]),
            input_abi: vec![],
            output_abi: RuntimePureOutputType::I64,
            body: RuntimeExprSeed::new(
                i64_semantic_type(),
                RuntimeExprSeedKind::Let {
                    binding: closure_binding,
                    expr: Box::new(closure),
                    body: Box::new(apply),
                },
            ),
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .expect("sequence-rest helper admission");
    let plan = Arc::new(builder.finish().expect("sealed sequence-rest plan"));
    let result = VmPureFunctionBackend
        .evaluate(
            &PureFunctionRequest::try_new(Arc::clone(&plan), plan.pure_helpers()[0].id, [])
                .expect("sequence-rest request"),
        )
        .expect("sequence-rest evaluation");
    assert_eq!(result.value, RuntimeValue::i64(7));
}

#[test]
fn structured_function_input_record_pattern_binds_by_declared_field_coordinate() {
    let record_semantic_type = semantic_type(34);
    let function_semantic_type = semantic_type(35);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                scalar_type_seeds()[0].clone(),
                RuntimePlanTypeSeed::new(
                    record_semantic_type,
                    RuntimePlanTypeProjection::Record(Box::new([
                        crate::plan::RuntimePlanRecordField::new("first", i64_semantic_type()),
                        crate::plan::RuntimePlanRecordField::new("second", i64_semantic_type()),
                    ])),
                ),
                RuntimePlanTypeSeed::new(
                    function_semantic_type,
                    RuntimePlanTypeProjection::Function {
                        contract: Default::default(),
                        parameters: Box::new([record_semantic_type]),
                        result: i64_semantic_type(),
                    },
                ),
            ],
            [
                RuntimeLocalDeclarationSeed::new(function_semantic_type),
                RuntimeLocalDeclarationSeed::new(record_semantic_type),
                RuntimeLocalDeclarationSeed::new(i64_semantic_type()),
            ],
        )
        .expect("record-pattern type graph");
    let closure_binding = admission.local_ids()[0].clone();
    let input_local = admission.local_ids()[1].clone();
    let selected = admission.local_ids()[2].clone();
    let site = builder
        .push_function_site_seed(
            crate::plan::RuntimeFunctionSemanticRole::Closure,
            [RuntimeFunctionInputBindingSeed {
                ownership: Default::default(),
                unrestricted_bindings: Box::new([]),
                source: RuntimeFunctionInputSource::Parameter {
                    position: 0,
                    passing: crate::plan::RuntimeFunctionParameterPassing::Value,
                },
                input_local,
                pattern: RuntimePatternSeed::new(
                    record_semantic_type,
                    RuntimePatternSeedKind::Record {
                        fields: Box::new([crate::plan::RuntimeRecordPatternFieldSeed::new(
                            crate::plan::RuntimeRecordFieldSeedId::from_zero_based(1),
                            RuntimePatternSeed::new(
                                i64_semantic_type(),
                                RuntimePatternSeedKind::Bind {
                                    mutable: false,
                                    local: selected.clone(),
                                },
                            ),
                        )]),
                        rest: crate::plan::RuntimePatternRestSeed::Ignore,
                    },
                ),
            }],
            i64_local(selected),
        )
        .expect("record-pattern function site");
    let closure = RuntimeExprSeed::new(
        function_semantic_type,
        RuntimeExprSeedKind::Function {
            site,
            captures: Box::new([]),
        },
    );
    let record = RuntimeValue::try_record(vec![
        ("first".to_owned(), RuntimeValue::i64(3)),
        ("second".to_owned(), RuntimeValue::i64(8)),
    ])
    .expect("record value");
    let apply = RuntimeExprSeed::new(
        i64_semantic_type(),
        RuntimeExprSeedKind::Apply {
            callee: Box::new(RuntimeExprSeed::new(
                function_semantic_type,
                RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                    closure_binding.clone(),
                    RuntimeLocalReadMode::Copy,
                )),
            )),
            args: Box::new([RuntimeCallArgumentSeed::new(
                RuntimeExprSeed::new(record_semantic_type, RuntimeExprSeedKind::Value(record)),
                RuntimeCallArgumentMode::Value,
                0,
            )]),
        },
    );
    builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            name: "record_pattern_select".to_owned(),
            inputs: Box::new([]),
            input_abi: vec![],
            output_abi: RuntimePureOutputType::I64,
            body: RuntimeExprSeed::new(
                i64_semantic_type(),
                RuntimeExprSeedKind::Let {
                    binding: closure_binding,
                    expr: Box::new(closure),
                    body: Box::new(apply),
                },
            ),
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .expect("record-pattern helper admission");
    let plan = Arc::new(builder.finish().expect("sealed record-pattern plan"));
    let result = VmPureFunctionBackend
        .evaluate(
            &PureFunctionRequest::try_new(Arc::clone(&plan), plan.pure_helpers()[0].id, [])
                .expect("record-pattern request"),
        )
        .expect("record-pattern evaluation");
    assert_eq!(result.value, RuntimeValue::i64(8));
}

#[test]
fn structured_apply_reorders_source_arguments_to_the_checked_abi() {
    let function_semantic_type = semantic_type(36);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                scalar_type_seeds()[0].clone(),
                RuntimePlanTypeSeed::new(
                    function_semantic_type,
                    RuntimePlanTypeProjection::Function {
                        contract: Default::default(),
                        parameters: Box::new([i64_semantic_type(), i64_semantic_type()]),
                        result: i64_semantic_type(),
                    },
                ),
            ],
            [
                RuntimeLocalDeclarationSeed::new(function_semantic_type),
                RuntimeLocalDeclarationSeed::new(i64_semantic_type()),
                RuntimeLocalDeclarationSeed::new(i64_semantic_type()),
                RuntimeLocalDeclarationSeed::new(i64_semantic_type()),
                RuntimeLocalDeclarationSeed::new(i64_semantic_type()),
            ],
        )
        .expect("positioned-call type graph");
    let closure_binding = admission.local_ids()[0].clone();
    let input_zero = admission.local_ids()[1].clone();
    let input_one = admission.local_ids()[2].clone();
    let parameter_zero = admission.local_ids()[3].clone();
    let parameter_one = admission.local_ids()[4].clone();
    let site = builder
        .push_function_site_seed(
            crate::plan::RuntimeFunctionSemanticRole::Closure,
            [
                RuntimeFunctionInputBindingSeed {
                    ownership: Default::default(),
                    unrestricted_bindings: Box::new([]),
                    source: RuntimeFunctionInputSource::Parameter {
                        position: 0,
                        passing: crate::plan::RuntimeFunctionParameterPassing::Value,
                    },
                    input_local: input_zero,
                    pattern: RuntimePatternSeed::new(
                        i64_semantic_type(),
                        RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local: parameter_zero.clone(),
                        },
                    ),
                },
                RuntimeFunctionInputBindingSeed {
                    ownership: Default::default(),
                    unrestricted_bindings: Box::new([]),
                    source: RuntimeFunctionInputSource::Parameter {
                        position: 1,
                        passing: crate::plan::RuntimeFunctionParameterPassing::Value,
                    },
                    input_local: input_one,
                    pattern: RuntimePatternSeed::new(
                        i64_semantic_type(),
                        RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local: parameter_one.clone(),
                        },
                    ),
                },
            ],
            RuntimeExprSeed::new(
                i64_semantic_type(),
                RuntimeExprSeedKind::Binary {
                    lhs: Box::new(i64_local(parameter_zero)),
                    op: RuntimeBinaryOp::Sub,
                    rhs: Box::new(i64_local(parameter_one)),
                },
            ),
        )
        .expect("positioned-call function site");
    let closure = RuntimeExprSeed::new(
        function_semantic_type,
        RuntimeExprSeedKind::Function {
            site,
            captures: Box::new([]),
        },
    );
    let apply = RuntimeExprSeed::new(
        i64_semantic_type(),
        RuntimeExprSeedKind::Apply {
            callee: Box::new(RuntimeExprSeed::new(
                function_semantic_type,
                RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                    closure_binding.clone(),
                    RuntimeLocalReadMode::Copy,
                )),
            )),
            args: Box::new([
                RuntimeCallArgumentSeed::new(i64_value(10), RuntimeCallArgumentMode::Value, 1),
                RuntimeCallArgumentSeed::new(i64_value(1), RuntimeCallArgumentMode::Value, 0),
            ]),
        },
    );
    builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            name: "positioned_apply".to_owned(),
            inputs: Box::new([]),
            input_abi: vec![],
            output_abi: RuntimePureOutputType::I64,
            body: RuntimeExprSeed::new(
                i64_semantic_type(),
                RuntimeExprSeedKind::Let {
                    binding: closure_binding,
                    expr: Box::new(closure),
                    body: Box::new(apply),
                },
            ),
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .expect("positioned-call helper admission");
    let plan = Arc::new(builder.finish().expect("sealed positioned-call plan"));
    let result = VmPureFunctionBackend
        .evaluate(
            &PureFunctionRequest::try_new(Arc::clone(&plan), plan.pure_helpers()[0].id, [])
                .expect("positioned-call request"),
        )
        .expect("positioned-call evaluation");
    assert_eq!(result.value, RuntimeValue::i64(-9));
}

#[test]
fn owned_pure_trait_call_evaluates_receiver_and_source_arguments_once() {
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [scalar_type_seeds()[0].clone()],
            [
                RuntimeLocalDeclarationSeed::new(i64_semantic_type()),
                RuntimeLocalDeclarationSeed::new(i64_semantic_type()),
                RuntimeLocalDeclarationSeed::new(i64_semantic_type()),
            ],
        )
        .expect("trait call input types");
    let receiver = admission.local_ids()[0].clone();
    let first = admission.local_ids()[1].clone();
    let second = admission.local_ids()[2].clone();
    let method = builder
        .push_trait_method_seed(RuntimeTraitMethodSeed {
            identity: RuntimeTraitMethodIdentity {
                impl_id: 0,
                trait_id: Some(0),
                witness: Some(0),
                trait_name: Some("DisplayText".to_owned()),
                self_type: "I64".to_owned(),
                method_name: "display_text".to_owned(),
                monomorph_label: "I64::display_text".to_owned(),
            },
            receiver: RuntimeReceiverMode::Owned,
            inputs: vec![receiver.clone(), first.clone(), second.clone()].into_boxed_slice(),
            input_abi: vec![RuntimePureInputType::I64; 3],
            output_abi: RuntimePureOutputType::I64,
            body: i64_binary(
                i64_local(receiver),
                RuntimeBinaryOp::Sub,
                i64_binary(i64_local(first), RuntimeBinaryOp::Mul, i64_local(second)),
            ),
        })
        .expect("owned method body");
    builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            name: "owned_trait_call".to_owned(),
            inputs: Box::new([]),
            input_abi: vec![],
            output_abi: RuntimePureOutputType::I64,
            body: RuntimeExprSeed::new(
                i64_semantic_type(),
                RuntimeExprSeedKind::TraitCall {
                    callable: method,
                    receiver: Box::new(i64_binary(
                        i64_value(20),
                        RuntimeBinaryOp::Add,
                        i64_value(1),
                    )),
                    args: Box::new([
                        RuntimeCallArgumentSeed::new(
                            i64_binary(i64_value(2), RuntimeBinaryOp::Add, i64_value(1)),
                            RuntimeCallArgumentMode::Value,
                            1,
                        ),
                        RuntimeCallArgumentSeed::new(
                            i64_value(4),
                            RuntimeCallArgumentMode::Value,
                            0,
                        ),
                    ]),
                },
            ),
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .expect("owned trait call helper");
    let plan = Arc::new(builder.finish().expect("owned trait call plan"));
    let request = PureFunctionRequest::try_new(Arc::clone(&plan), plan.pure_helpers()[0].id, [])
        .expect("owned trait call request");
    let result = VmPureFunctionBackend
        .evaluate(&request)
        .expect("owned trait call evaluation");
    assert_eq!(result.value, RuntimeValue::i64(9));
    assert_eq!(result.stats.evaluated_binary_ops, 4);
    assert_eq!(result.stats.evaluated_calls, 1);
}

fn simple_trait_call_plan(
    receiver_mode: RuntimeReceiverMode,
    host_call_body: bool,
) -> Arc<RuntimePlan> {
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [scalar_type_seeds()[0].clone()],
            [RuntimeLocalDeclarationSeed::new(i64_semantic_type())],
        )
        .expect("trait method receiver type");
    let receiver = admission.local_ids()[0].clone();
    let body = if host_call_body {
        RuntimeExprSeed::new(
            i64_semantic_type(),
            RuntimeExprSeedKind::Call {
                callee: RuntimeCallTarget::try_from_label("host_call")
                    .expect("valid callable identity"),
                args: Box::new([]),
            },
        )
    } else {
        i64_local(receiver.clone())
    };
    let method = builder
        .push_trait_method_seed(RuntimeTraitMethodSeed {
            identity: RuntimeTraitMethodIdentity {
                impl_id: 0,
                trait_id: None,
                witness: None,
                trait_name: None,
                self_type: "I64".to_owned(),
                method_name: "render".to_owned(),
                monomorph_label: "I64::render".to_owned(),
            },
            receiver: receiver_mode,
            inputs: Box::new([receiver]),
            input_abi: vec![RuntimePureInputType::I64],
            output_abi: RuntimePureOutputType::I64,
            body,
        })
        .expect("trait method");
    builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            name: "trait_call".to_owned(),
            inputs: Box::new([]),
            input_abi: vec![],
            output_abi: RuntimePureOutputType::I64,
            body: RuntimeExprSeed::new(
                i64_semantic_type(),
                RuntimeExprSeedKind::TraitCall {
                    callable: method,
                    receiver: Box::new(i64_value(42)),
                    args: Box::new([]),
                },
            ),
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .expect("trait call helper");
    Arc::new(builder.finish().expect("trait call plan"))
}

#[test]
fn pure_trait_call_rejects_borrowed_receivers_and_host_calls() {
    for mode in [RuntimeReceiverMode::SharedRef, RuntimeReceiverMode::MutRef] {
        let plan = simple_trait_call_plan(mode, false);
        let request =
            PureFunctionRequest::try_new(Arc::clone(&plan), plan.pure_helpers()[0].id, [])
                .expect("trait call request");
        assert!(matches!(
            VmPureFunctionBackend.evaluate(&request),
            Err(RuntimeEvalError::UnsupportedPure { .. })
        ));
    }
    let plan = simple_trait_call_plan(RuntimeReceiverMode::Owned, true);
    let request = PureFunctionRequest::try_new(Arc::clone(&plan), plan.pure_helpers()[0].id, [])
        .expect("host call request");
    assert!(matches!(
        VmPureFunctionBackend.evaluate(&request),
        Err(RuntimeEvalError::UnsupportedPure { reason, .. }) if reason.contains("host calls")
    ));
}

#[test]
fn pure_trait_call_checks_selected_method_and_sealed_abi() {
    let base = simple_trait_call_plan(RuntimeReceiverMode::Owned, false);
    let mut wrong_id = Arc::new((*base).clone());
    Arc::get_mut(&mut wrong_id).unwrap().trait_methods[0].id = RuntimeTraitMethodId(1);
    let request =
        PureFunctionRequest::try_new(Arc::clone(&wrong_id), wrong_id.pure_helpers()[0].id, [])
            .expect("wrong method id request");
    assert_eq!(
        VmPureFunctionBackend.evaluate(&request),
        Err(RuntimeEvalError::UnknownTraitMethod(0))
    );

    let mut wrong_abi = Arc::new((*base).clone());
    Arc::get_mut(&mut wrong_abi).unwrap().trait_methods[0].input_types[0] =
        RuntimePureInputType::F64;
    let request =
        PureFunctionRequest::try_new(Arc::clone(&wrong_abi), wrong_abi.pure_helpers()[0].id, [])
            .expect("wrong method ABI request");
    assert!(matches!(
        VmPureFunctionBackend.evaluate(&request),
        Err(RuntimeEvalError::InvalidExpressionType(_))
    ));

    let mut wrong_result = Arc::new((*base).clone());
    Arc::get_mut(&mut wrong_result).unwrap().trait_methods[0].output_type =
        RuntimePureOutputType::F64;
    let request = PureFunctionRequest::try_new(
        Arc::clone(&wrong_result),
        wrong_result.pure_helpers()[0].id,
        [],
    )
    .expect("wrong method result request");
    assert!(matches!(
        VmPureFunctionBackend.evaluate(&request),
        Err(RuntimeEvalError::InvalidExpressionType(_))
    ));
}

#[test]
fn structured_pure_standard_map_covers_all_published_families() {
    let (plan, cases) = standard_map_pure_plan();

    for case in cases {
        let result = VmPureFunctionBackend
            .evaluate(
                &PureFunctionRequest::try_new(Arc::clone(&plan), case.helper, [])
                    .expect("standard map request"),
            )
            .expect("standard map pure evaluation");

        assert_eq!(result.value, case.expected);
        assert_eq!(
            result.stats.evaluated_binary_ops, case.callback_count,
            "callback was applied exactly once per selected source item"
        );
    }

    let array_result = VmPureFunctionBackend
        .evaluate(
            &PureFunctionRequest::try_new(Arc::clone(&plan), RuntimePureHelperId(2), [])
                .expect("array map request"),
        )
        .expect("array map pure evaluation");
    let RuntimeValue::Seq(array) = array_result.value else {
        panic!("array map must retain sequence representation");
    };
    assert_eq!(array.len(), 3, "array map preserves its admitted length");

    let option_none_result = VmPureFunctionBackend
        .evaluate(
            &PureFunctionRequest::try_new(Arc::clone(&plan), RuntimePureHelperId(5), [])
                .expect("Option::None map request"),
        )
        .expect("Option::None map pure evaluation");
    assert_eq!(option_none_result.value, RuntimeValue::option_none());
    assert_eq!(option_none_result.stats.evaluated_binary_ops, 0);

    let result_err = VmPureFunctionBackend
        .evaluate(
            &PureFunctionRequest::try_new(Arc::clone(&plan), RuntimePureHelperId(7), [])
                .expect("Result::Err map request"),
        )
        .expect("Result::Err map pure evaluation");
    assert_eq!(
        result_err.value,
        RuntimeValue::result_err(RuntimeValue::String("preserve".to_owned()))
    );
    assert_eq!(result_err.stats.evaluated_binary_ops, 0);
}
