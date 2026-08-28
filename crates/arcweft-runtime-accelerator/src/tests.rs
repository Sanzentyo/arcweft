use std::sync::Arc;

use super::*;
#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
use arcweft_core::value::runtime_sequence_dense_u32;
use arcweft_core::{
    engine::{Engine, FlowExit, FlowFiberStatus},
    entry::RuntimeNominalTypeId,
    pattern::{RuntimeSemanticTypeId, RuntimeVariantIdentity},
    plan::{
        FlowRuntimeId, RuntimeCallArgumentSeed, RuntimeExprSeed, RuntimeExprSeedKind,
        RuntimeFlowOpSeed, RuntimeFlowSchema, RuntimeFlowSeed, RuntimeLocalDeclarationSeed,
        RuntimeLocalSeedId, RuntimePlan, RuntimePlanBuilder, RuntimePlanSequenceKind,
        RuntimePlanTypeProjection, RuntimePlanTypeSeed, RuntimePureHelperOrigin,
        RuntimePureHelperSeed,
    },
    pure::{PureFunctionRequest, RuntimePureHelperRef},
    step::{RuntimeStepInput, RuntimeStepOptions},
};
use arcweft_core::{
    entry::RuntimeCallableId,
    plan::RuntimePureHelperId,
    value::{
        RuntimeBinaryOp, RuntimeCallArgumentMode, RuntimeCallTarget, RuntimeISizeValue, RuntimeSeq,
        RuntimeSignedIntWidth, RuntimeStandardMapFamily, RuntimeStandardMapOperandOrder,
        RuntimeUSizeValue, RuntimeUnsignedIntWidth,
    },
};

fn flow_id(value: &str) -> FlowRuntimeId {
    FlowRuntimeId::from_runtime_target_value(value).expect("test flow ID is valid")
}

fn callable_target(value: &str) -> RuntimeCallTarget {
    RuntimeCallTarget::callable(
        RuntimeCallableId::try_new(value).expect("test callable identity is valid"),
    )
}

struct AdmittedHelper {
    request: PureFunctionRequest,
}

impl AdmittedHelper {
    fn helper_ref(&self) -> RuntimePureHelperRef<'_> {
        self.request
            .helper_ref()
            .expect("admitted helper is resolved through its plan-qualified request")
    }

    fn plan(&self) -> &Arc<RuntimePlan> {
        self.request.plan()
    }

    fn from_plan(plan: Arc<RuntimePlan>, id: RuntimePureHelperId) -> Self {
        let args = plan.pure_helpers()[id.0]
            .input_types
            .iter()
            .copied()
            .map(default_helper_input)
            .collect::<Vec<_>>();
        let request = PureFunctionRequest::try_new(plan, id, args)
            .expect("admitted helper request has well-typed default inputs");
        Self { request }
    }
}

fn empty_runtime_plan() -> Arc<RuntimePlan> {
    Arc::new(
        RuntimePlanBuilder::new()
            .finish()
            .expect("empty adapter plan is sealed"),
    )
}

fn empty_plan_accelerator(config: RuntimePureAcceleratorConfig) -> RuntimePureAccelerator {
    let plan = empty_runtime_plan();
    RuntimePureAccelerator::with_config(config, &plan)
}

fn empty_plan_accelerator_with_mode(mode: RuntimePureBackendMode) -> RuntimePureAccelerator {
    let plan = empty_runtime_plan();
    RuntimePureAccelerator::new(mode, &plan)
}

fn default_helper_input(input: RuntimePureInputType) -> RuntimeValue {
    match input {
        RuntimePureInputType::I8 => RuntimeValue::i8(0),
        RuntimePureInputType::I16 => RuntimeValue::i16(0),
        RuntimePureInputType::I32 => RuntimeValue::i32(0),
        RuntimePureInputType::I64 => RuntimeValue::i64(0),
        RuntimePureInputType::I128 => RuntimeValue::i128(0),
        RuntimePureInputType::ISize => RuntimeValue::isize(0),
        RuntimePureInputType::U8 => RuntimeValue::u8(0),
        RuntimePureInputType::U16 => RuntimeValue::u16(0),
        RuntimePureInputType::U32 => RuntimeValue::u32(0),
        RuntimePureInputType::U64 => RuntimeValue::u64(0),
        RuntimePureInputType::U128 => RuntimeValue::u128(0),
        RuntimePureInputType::USize => RuntimeValue::usize(0),
        RuntimePureInputType::F32 => RuntimeValue::f32(0.0),
        RuntimePureInputType::F64 => RuntimeValue::f64(0.0),
        RuntimePureInputType::Value => RuntimeValue::String(String::new()),
    }
}

fn helper_type_identity(abi: RuntimePureInputType) -> RuntimeSemanticTypeId {
    let marker = match abi {
        RuntimePureInputType::I8 => 1,
        RuntimePureInputType::I16 => 2,
        RuntimePureInputType::I32 => 3,
        RuntimePureInputType::I64 => 4,
        RuntimePureInputType::I128 => 5,
        RuntimePureInputType::ISize => 6,
        RuntimePureInputType::U8 => 7,
        RuntimePureInputType::U16 => 8,
        RuntimePureInputType::U32 => 9,
        RuntimePureInputType::U64 => 10,
        RuntimePureInputType::U128 => 11,
        RuntimePureInputType::USize => 12,
        RuntimePureInputType::F32 => 13,
        RuntimePureInputType::F64 => 14,
        RuntimePureInputType::Value => 15,
    };
    RuntimeSemanticTypeId::from_bytes([marker; 32])
}

fn helper_type_seed(abi: RuntimePureInputType) -> RuntimePlanTypeSeed {
    let projection = match abi {
        RuntimePureInputType::I8 => RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I8),
        RuntimePureInputType::I16 => RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I16),
        RuntimePureInputType::I32 => RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I32),
        RuntimePureInputType::I64 => RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64),
        RuntimePureInputType::I128 => {
            RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I128)
        }
        RuntimePureInputType::ISize => {
            RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::ISize)
        }
        RuntimePureInputType::U8 => {
            RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U8)
        }
        RuntimePureInputType::U16 => {
            RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U16)
        }
        RuntimePureInputType::U32 => {
            RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U32)
        }
        RuntimePureInputType::U64 => {
            RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U64)
        }
        RuntimePureInputType::U128 => {
            RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U128)
        }
        RuntimePureInputType::USize => {
            RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::USize)
        }
        RuntimePureInputType::F32 => RuntimePlanTypeProjection::F32,
        RuntimePureInputType::F64 => RuntimePlanTypeProjection::F64,
        RuntimePureInputType::Value => RuntimePlanTypeProjection::String,
    };
    RuntimePlanTypeSeed::new(helper_type_identity(abi), projection)
}

fn output_input_abi(abi: RuntimePureOutputType) -> RuntimePureInputType {
    match abi {
        RuntimePureOutputType::Bool | RuntimePureOutputType::Value => RuntimePureInputType::Value,
        RuntimePureOutputType::I8 => RuntimePureInputType::I8,
        RuntimePureOutputType::I16 => RuntimePureInputType::I16,
        RuntimePureOutputType::I32 => RuntimePureInputType::I32,
        RuntimePureOutputType::I64 => RuntimePureInputType::I64,
        RuntimePureOutputType::I128 => RuntimePureInputType::I128,
        RuntimePureOutputType::ISize => RuntimePureInputType::ISize,
        RuntimePureOutputType::U8 => RuntimePureInputType::U8,
        RuntimePureOutputType::U16 => RuntimePureInputType::U16,
        RuntimePureOutputType::U32 => RuntimePureInputType::U32,
        RuntimePureOutputType::U64 => RuntimePureInputType::U64,
        RuntimePureOutputType::U128 => RuntimePureInputType::U128,
        RuntimePureOutputType::USize => RuntimePureInputType::USize,
        RuntimePureOutputType::F32 => RuntimePureInputType::F32,
        RuntimePureOutputType::F64 => RuntimePureInputType::F64,
    }
}

fn admit_helper(
    name: &str,
    input_abi: Vec<RuntimePureInputType>,
    output_abi: RuntimePureOutputType,
    scalar_eval_supported: bool,
    origin: RuntimePureHelperOrigin,
    body: impl FnOnce(&[RuntimeLocalSeedId], RuntimeSemanticTypeId) -> RuntimeExprSeed,
) -> AdmittedHelper {
    let output_input = output_input_abi(output_abi);
    let mut type_seeds = input_abi
        .iter()
        .copied()
        .map(helper_type_seed)
        .collect::<Vec<_>>();
    let output_seed = helper_type_seed(output_input);
    if !type_seeds
        .iter()
        .any(|seed| seed.semantic_identity() == output_seed.semantic_identity())
    {
        type_seeds.push(output_seed);
    }
    type_seeds.push(RuntimePlanTypeSeed::new(
        RuntimeSemanticTypeId::from_bytes([16; 32]),
        RuntimePlanTypeProjection::Bool,
    ));
    let mut builder = RuntimePlanBuilder::new();
    let input_types = input_abi
        .iter()
        .copied()
        .map(helper_type_identity)
        .collect::<Vec<_>>();
    let admission = builder
        .admit_semantic_batch(
            type_seeds,
            input_types
                .iter()
                .copied()
                .map(RuntimeLocalDeclarationSeed::new),
            [],
            [],
        )
        .expect("test helper semantic inputs are admitted");
    builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            name: name.to_owned(),
            inputs: admission.local_ids().to_vec().into_boxed_slice(),
            input_abi,
            output_abi,
            body: body(admission.local_ids(), helper_type_identity(output_input)),
            scalar_eval_supported,
            origin,
        })
        .expect("test helper is admitted");
    let plan = Arc::new(builder.finish().expect("test helper plan is sealed"));
    AdmittedHelper::from_plan(Arc::clone(&plan), plan.pure_helpers()[0].id)
}

fn local_expr(ty: RuntimeSemanticTypeId, local: RuntimeLocalSeedId) -> RuntimeExprSeed {
    RuntimeExprSeed::new(ty, RuntimeExprSeedKind::Local(local))
}

fn value_expr(ty: RuntimeSemanticTypeId, value: RuntimeValue) -> RuntimeExprSeed {
    RuntimeExprSeed::new(ty, RuntimeExprSeedKind::Value(value))
}

fn binary_expr(
    ty: RuntimeSemanticTypeId,
    lhs: RuntimeExprSeed,
    op: RuntimeBinaryOp,
    rhs: RuntimeExprSeed,
) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        ty,
        RuntimeExprSeedKind::Binary {
            lhs: Box::new(lhs),
            op,
            rhs: Box::new(rhs),
        },
    )
}

fn mul_add_helper(
    name: &str,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
    constant: RuntimeValue,
    origin: RuntimePureHelperOrigin,
) -> AdmittedHelper {
    let ty = helper_type_identity(input_type);
    admit_helper(
        name,
        vec![input_type, input_type],
        output_type,
        true,
        origin,
        move |inputs, output_ty| {
            binary_expr(
                output_ty,
                local_expr(ty, inputs[0].clone()),
                RuntimeBinaryOp::Mul,
                binary_expr(
                    output_ty,
                    local_expr(ty, inputs[1].clone()),
                    RuntimeBinaryOp::Add,
                    value_expr(output_ty, constant),
                ),
            )
        },
    )
}

fn add_helper(
    name: &str,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
) -> AdmittedHelper {
    admit_add_helpers(&[(name, input_type, output_type)])
        .pop()
        .expect("one requested helper is admitted")
}

fn admit_add_helpers(
    helpers: &[(&str, RuntimePureInputType, RuntimePureOutputType)],
) -> Vec<AdmittedHelper> {
    let mut type_seeds = vec![RuntimePlanTypeSeed::new(
        RuntimeSemanticTypeId::from_bytes([16; 32]),
        RuntimePlanTypeProjection::Bool,
    )];
    for (_, input_type, output_type) in helpers {
        for input in [*input_type, output_input_abi(*output_type)] {
            let seed = helper_type_seed(input);
            if !type_seeds
                .iter()
                .any(|existing| existing.semantic_identity() == seed.semantic_identity())
            {
                type_seeds.push(seed);
            }
        }
    }
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_semantic_batch(
            type_seeds,
            helpers.iter().flat_map(|(_, input_type, _)| {
                std::iter::repeat_n(
                    RuntimeLocalDeclarationSeed::new(helper_type_identity(*input_type)),
                    2,
                )
            }),
            [],
            [],
        )
        .expect("test helper semantic inputs are admitted");
    let mut next_local = 0;
    let ids = helpers
        .iter()
        .map(|(name, input_type, output_type)| {
            let inputs = &admission.local_ids()[next_local..next_local + 2];
            next_local += 2;
            let input_ty = helper_type_identity(*input_type);
            builder
                .push_pure_helper_seed(RuntimePureHelperSeed {
                    name: (*name).to_owned(),
                    inputs: inputs.to_vec().into_boxed_slice(),
                    input_abi: vec![*input_type; 2],
                    output_abi: *output_type,
                    body: binary_expr(
                        helper_type_identity(output_input_abi(*output_type)),
                        local_expr(input_ty, inputs[0].clone()),
                        RuntimeBinaryOp::Add,
                        local_expr(input_ty, inputs[1].clone()),
                    ),
                    scalar_eval_supported: true,
                    origin: RuntimePureHelperOrigin::Annotated,
                })
                .expect("test helper is admitted")
        })
        .collect::<Vec<_>>();
    let plan = Arc::new(builder.finish().expect("test helper plan is sealed"));
    ids.into_iter()
        .enumerate()
        .map(|(index, _)| AdmittedHelper::from_plan(Arc::clone(&plan), RuntimePureHelperId(index)))
        .collect()
}

fn conditional_div_helper(
    name: &str,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
    threshold: RuntimeValue,
    one: RuntimeValue,
    zero: RuntimeValue,
) -> AdmittedHelper {
    let ty = helper_type_identity(input_type);
    let bool_ty = RuntimeSemanticTypeId::from_bytes([16; 32]);
    admit_helper(
        name,
        vec![input_type, input_type],
        output_type,
        true,
        RuntimePureHelperOrigin::Annotated,
        move |inputs, output_ty| {
            RuntimeExprSeed::new(
                output_ty,
                RuntimeExprSeedKind::If {
                    condition: Box::new(binary_expr(
                        bool_ty,
                        local_expr(ty, inputs[0].clone()),
                        RuntimeBinaryOp::Ge,
                        value_expr(ty, threshold),
                    )),
                    then_expr: Box::new(binary_expr(
                        output_ty,
                        local_expr(ty, inputs[0].clone()),
                        RuntimeBinaryOp::Div,
                        binary_expr(
                            output_ty,
                            local_expr(ty, inputs[1].clone()),
                            RuntimeBinaryOp::Add,
                            value_expr(output_ty, one),
                        ),
                    )),
                    else_expr: Box::new(value_expr(output_ty, zero)),
                },
            )
        },
    )
}

fn data_format_value(format: DataFormat) -> RuntimeValue {
    let ordinal = DataFormat::ALL
        .iter()
        .position(|candidate| candidate == &format)
        .and_then(|ordinal| u32::try_from(ordinal).ok())
        .expect("DataFormat inventory fits the runtime ordinal");
    RuntimeValue::Variant {
        owner: RuntimeVariantIdentity::Nominal {
            nominal: RuntimeNominalTypeId::try_new("DataFormat")
                .expect("DataFormat runtime nominal identity is valid"),
            // The accelerator consumes the checked owner identity rather than
            // rebuilding semantic type facts. A distinctive test identity
            // proves it does not fall back to a source path.
            semantic_identity: RuntimeSemanticTypeId::from_bytes([0xDA; 32]),
        },
        ordinal,
        name: format.variant_name().to_owned(),
        payload: None,
    }
}

#[test]
fn data_external_call_encodes_and_decodes_json_with_format_enum() {
    let mut accelerator = empty_plan_accelerator(RuntimePureAcceleratorConfig::default());
    let value = RuntimeValue::Seq(RuntimeSeq::Values(vec![RuntimeValue::String(
        "hello".to_owned(),
    )]));
    let format = data_format_value(DataFormat::Json);

    let encoded = accelerator
        .call_external(
            &callable_target("data.encode"),
            &[value.clone(), format.clone()],
        )
        .expect("data encode is handled")
        .expect("data encode succeeds");
    let RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Bytes(bytes))) = &encoded else {
        panic!("expected encoded bytes");
    };
    assert_eq!(bytes.as_slice(), br#"["hello"]"#);

    let decoded = accelerator
        .call_external(&callable_target("data.decode"), &[encoded, format])
        .expect("data decode is handled")
        .expect("data decode succeeds");

    assert_eq!(decoded, value);
}

#[test]
fn data_external_call_rejects_wrong_data_format_owner_and_ordinal() {
    let mut accelerator = empty_plan_accelerator(RuntimePureAcceleratorConfig::default());
    let value = RuntimeValue::String("hello".to_owned());
    let wrong_owner = RuntimeValue::Variant {
        owner: RuntimeVariantIdentity::Nominal {
            nominal: RuntimeNominalTypeId::try_new("OtherFormat")
                .expect("test runtime nominal identity is valid"),
            semantic_identity: RuntimeSemanticTypeId::from_bytes([0xDA; 32]),
        },
        ordinal: 0,
        name: "Json".to_owned(),
        payload: None,
    };
    let wrong_ordinal = RuntimeValue::Variant {
        owner: RuntimeVariantIdentity::Nominal {
            nominal: RuntimeNominalTypeId::try_new("DataFormat")
                .expect("DataFormat runtime nominal identity is valid"),
            semantic_identity: RuntimeSemanticTypeId::from_bytes([0xDA; 32]),
        },
        ordinal: 1,
        name: "Json".to_owned(),
        payload: None,
    };

    for format in [wrong_owner, wrong_ordinal] {
        let error = accelerator
            .call_external(&callable_target("data.encode"), &[value.clone(), format])
            .expect("data encode is handled")
            .expect_err("forged DataFormat identity is rejected");
        let RuntimeEvalError::UnsupportedPure { reason, .. } = error else {
            panic!("expected UnsupportedPure for forged DataFormat identity");
        };
        assert!(reason.contains("DataFormat"));
    }
}

#[test]
fn data_external_call_round_trips_dynamic_avro() {
    let mut accelerator = empty_plan_accelerator(RuntimePureAcceleratorConfig::default());
    let value = RuntimeValue::try_record(vec![(
        "speaker".to_owned(),
        RuntimeValue::String("alice".to_owned()),
    )])
    .expect("test record fields are unique");
    let format = data_format_value(DataFormat::Avro);

    let encoded = accelerator
        .call_external(
            &callable_target("data.encode"),
            &[value.clone(), format.clone()],
        )
        .expect("data encode is handled")
        .expect("data encode succeeds");
    let decoded = accelerator
        .call_external(&callable_target("data.decode"), &[encoded, format])
        .expect("data decode is handled")
        .expect("data decode succeeds");

    assert_eq!(decoded, value);
}

#[test]
fn data_external_call_encodes_shape_required_formats_and_rejects_dynamic_decode() {
    for variant in ["Csv", "ArrowIpc", "Parquet", "ArcweftBinary"] {
        let mut accelerator = empty_plan_accelerator(RuntimePureAcceleratorConfig::default());
        let value = RuntimeValue::Seq(RuntimeSeq::Values(vec![
            RuntimeValue::try_record(vec![
                ("line".to_owned(), RuntimeValue::String("hello".to_owned())),
                (
                    "speaker".to_owned(),
                    RuntimeValue::String("alice".to_owned()),
                ),
            ])
            .expect("test record fields are unique"),
        ]));
        let format = data_format_value(
            DataFormat::from_variant_name(variant).expect("tested data format is registered"),
        );

        let encoded = accelerator
            .call_external(
                &callable_target("data.encode"),
                &[value.clone(), format.clone()],
            )
            .unwrap_or_else(|| panic!("{variant} data encode is handled"))
            .unwrap_or_else(|error| panic!("{variant} data encode succeeds: {error}"));
        let error = accelerator
            .call_external(&callable_target("data.decode"), &[encoded, format])
            .unwrap_or_else(|| panic!("{variant} data decode is handled"))
            .expect_err("shape-required formats need an explicit decode shape");

        let RuntimeEvalError::UnsupportedPure { reason, .. } = error else {
            panic!("expected UnsupportedPure for {variant} dynamic decode");
        };
        assert!(
            reason.contains("requires an explicit TypeShape"),
            "{variant} should explain why dynamic decode is unavailable: {reason}"
        );
    }
}

#[test]
fn data_external_call_decodes_shape_required_formats_with_explicit_shape() {
    for variant in ["Csv", "ArrowIpc", "Parquet", "ArcweftBinary"] {
        let mut accelerator = empty_plan_accelerator(RuntimePureAcceleratorConfig::default());
        let value = RuntimeValue::Seq(RuntimeSeq::Values(vec![
            RuntimeValue::try_record(vec![
                ("line".to_owned(), RuntimeValue::String("hello".to_owned())),
                (
                    "speaker".to_owned(),
                    RuntimeValue::String("alice".to_owned()),
                ),
            ])
            .expect("test record fields are unique"),
        ]));
        let format = data_format_value(
            DataFormat::from_variant_name(variant).expect("tested data format is registered"),
        );
        let shape = accelerator
            .call_external(&callable_target("data.shape"), std::slice::from_ref(&value))
            .unwrap_or_else(|| panic!("{variant} data shape is handled"))
            .unwrap_or_else(|error| panic!("{variant} data shape succeeds: {error}"));
        let encoded = accelerator
            .call_external(
                &callable_target("data.encode"),
                &[value.clone(), format.clone()],
            )
            .unwrap_or_else(|| panic!("{variant} data encode is handled"))
            .unwrap_or_else(|error| panic!("{variant} data encode succeeds: {error}"));
        let decoded = accelerator
            .call_external(&callable_target("data.decode"), &[encoded, format, shape])
            .unwrap_or_else(|| panic!("{variant} data decode is handled"))
            .unwrap_or_else(|error| panic!("{variant} data decode succeeds: {error}"));

        assert_eq!(decoded, value, "{variant} explicit shape decode roundtrip");
    }
}

#[test]
fn external_inference_call_sequence_uses_adapter_boundary() {
    let image = DenseTensorF32::new(
        vec![1, 1, 4, 4],
        vec![
            8.0, 1.0, 1.0, 1.0, 1.0, 4.0, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0,
        ],
    )
    .expect("image tensor shape is valid");
    let kernel = DenseTensorF32::new(vec![1, 1, 2, 2], vec![1.0, 0.0, 0.0, -1.0])
        .expect("kernel tensor shape is valid");
    let dense = DenseTensorF32::new(vec![4, 2], vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0])
        .expect("dense tensor shape is valid");
    let bias = DenseTensorF32::new(vec![2], vec![0.0, 0.0]).expect("bias tensor shape is valid");
    let conv_target = callable_target("conv2d.valid_f32");
    assert!(matches!(&conv_target, RuntimeCallTarget::Callable(_)));
    let mut accelerator = empty_plan_accelerator_with_mode(RuntimePureBackendMode::Auto);
    let conv = accelerator
        .call_external(
            &conv_target,
            &[
                RuntimeValue::tensor_f32(image),
                RuntimeValue::tensor_f32(kernel),
                RuntimeValue::usize(1),
                RuntimeValue::usize(1),
            ],
        )
        .expect("conv2d call is handled")
        .expect("conv2d call succeeds");
    let relu = accelerator
        .call_external(&callable_target("infer.relu_f32"), &[conv])
        .expect("relu call is handled")
        .expect("relu call succeeds");
    let pooled = accelerator
        .call_external(
            &callable_target("infer.max_pool2d_f32"),
            &[
                relu,
                RuntimeValue::usize(2),
                RuntimeValue::usize(2),
                RuntimeValue::usize(1),
                RuntimeValue::usize(1),
            ],
        )
        .expect("max-pool call is handled")
        .expect("max-pool call succeeds");
    let flattened = accelerator
        .call_external(&callable_target("infer.flatten_outer_f32"), &[pooled])
        .expect("flatten call is handled")
        .expect("flatten call succeeds");
    let logits = accelerator
        .call_external(
            &callable_target("infer.matmul_bias_add_f32"),
            &[
                flattened,
                RuntimeValue::tensor_f32(dense),
                RuntimeValue::tensor_f32(bias),
            ],
        )
        .expect("matmul-bias call is handled")
        .expect("matmul-bias call succeeds");
    let classified = accelerator
        .call_external(&callable_target("infer.argmax_last_dim_f32"), &[logits])
        .expect("argmax call is handled")
        .expect("argmax call succeeds");

    assert_eq!(accelerator.stats().math_calls, 6);
    assert!(matches!(
        classified,
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::USize(values)))
            if values.as_slice()[0].get() == 1
    ));
}

#[cfg(feature = "math-glam")]
#[test]
fn math_intrinsic_uses_adapter_math_accelerator() {
    let lhs = DenseMatrixF32::new(
        4,
        4,
        vec![
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ],
    )
    .expect("matrix shape is valid");
    let rhs = DenseMatrixF32::new(
        4,
        4,
        vec![
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        ],
    )
    .expect("matrix shape is valid");
    let mut accelerator = empty_plan_accelerator_with_mode(RuntimePureBackendMode::Auto);
    let result = RuntimeMathCallBackend::call_math_matmul_f32(&mut accelerator, &lhs, &rhs)
        .expect("f32 matrix multiplication succeeds");

    assert_eq!(result.rows(), 4);
    assert_eq!(result.cols(), 4);
    assert_eq!(accelerator.stats().math_calls, 1);
    assert_eq!(accelerator.stats().math_accelerated_calls, 1);
    assert_eq!(
        accelerator.math_stats().last_backend,
        Some(math::RuntimeMathBackend::Glam)
    );
}

#[cfg(feature = "math-ndarray")]
#[test]
fn f64_math_intrinsic_uses_width_preserving_adapter_backend() {
    let lhs = DenseMatrixF64::new(2, 2, vec![1.5, 2.0, 3.25, 4.5]).expect("matrix shape is valid");
    let rhs = DenseMatrixF64::new(2, 2, vec![5.0, 6.5, 7.0, 8.25]).expect("matrix shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Ndarray,
                ..math::RuntimeMathAcceleratorConfig::default()
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &empty_runtime_plan(),
    );

    let result = RuntimeMathCallBackend::call_math_matmul_f64(&mut accelerator, &lhs, &rhs)
        .expect("f64 matrix multiplication succeeds");

    assert_eq!(result.rows(), 2);
    assert_eq!(result.cols(), 2);
    assert_eq!(accelerator.stats().math_calls, 1);
    assert_eq!(accelerator.stats().math_accelerated_calls, 1);
    assert_eq!(
        accelerator.stats().arg_bytes_borrowed,
        8 * std::mem::size_of::<f64>()
    );
    assert_eq!(
        accelerator.math_stats().last_backend,
        Some(math::RuntimeMathBackend::Ndarray)
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_wgpu_math_cache_reuses_prepared_matmul_buffers_across_counter_reset() {
    let lhs = DenseMatrixF32::new(16, 16, vec![1.0; 256]).expect("matrix shape is valid");
    let rhs = DenseMatrixF32::new(16, 16, vec![2.0; 256]).expect("matrix shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Wgpu,
                ..math::RuntimeMathAcceleratorConfig::default()
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &empty_runtime_plan(),
    );

    let Ok(first) = RuntimeMathCallBackend::call_math_matmul_f32(&mut accelerator, &lhs, &rhs)
    else {
        return;
    };
    assert_eq!(first.rows(), 16);
    assert_eq!(first.cols(), 16);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 4);

    accelerator.reset_runtime_counters();
    let second = RuntimeMathCallBackend::call_math_matmul_f32(&mut accelerator, &lhs, &rhs)
        .expect("prepared runtime math matmul cache is reusable");

    assert_eq!(second.values(), first.values());
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 4);
    assert_eq!(accelerator.math_stats().gpu_reused_dispatches, 1);
    assert_eq!(accelerator.math_stats().bytes_uploaded, 0);
    assert_eq!(
        accelerator.math_stats().bytes_downloaded,
        std::mem::size_of_val(first.values())
    );
    assert_eq!(
        accelerator.stats().arg_bytes_borrowed,
        (lhs.values().len() + rhs.values().len()) * std::mem::size_of::<f32>()
    );
    assert_eq!(
        accelerator.stats().result_bytes_copied,
        std::mem::size_of_val(second.values())
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_wgpu_math_cache_updates_prepared_matmul_inputs_for_same_shape() {
    let lhs = DenseMatrixF32::new(16, 16, vec![1.0; 256]).expect("matrix shape is valid");
    let rhs = DenseMatrixF32::new(16, 16, vec![2.0; 256]).expect("matrix shape is valid");
    let changed_lhs = DenseMatrixF32::new(16, 16, vec![3.0; 256]).expect("matrix shape is valid");
    let changed_rhs = DenseMatrixF32::new(16, 16, vec![0.5; 256]).expect("matrix shape is valid");
    let expected = changed_lhs
        .matmul_scalar(&changed_rhs)
        .expect("scalar matmul succeeds");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Wgpu,
                ..math::RuntimeMathAcceleratorConfig::default()
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &empty_runtime_plan(),
    );

    if RuntimeMathCallBackend::call_math_matmul_f32(&mut accelerator, &lhs, &rhs).is_err() {
        return;
    }

    accelerator.reset_runtime_counters();
    let second =
        RuntimeMathCallBackend::call_math_matmul_f32(&mut accelerator, &changed_lhs, &changed_rhs)
            .expect("prepared runtime math matmul cache updates same-shape inputs");

    assert_eq!(second.values(), expected.values());
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 7);
    assert_eq!(
        accelerator.math_stats().bytes_uploaded,
        (changed_lhs.values().len() + changed_rhs.values().len()) * std::mem::size_of::<f32>()
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_external_infer_matmul_bias_add_reuses_prepared_wgpu_buffers() {
    let lhs = DenseTensorF32::new(vec![16, 16], vec![1.0; 256]).expect("tensor shape is valid");
    let rhs = DenseTensorF32::new(vec![16, 16], vec![2.0; 256]).expect("tensor shape is valid");
    let bias = DenseTensorF32::new(vec![16], vec![0.25; 16]).expect("bias shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Wgpu,
                ..math::RuntimeMathAcceleratorConfig::default()
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &empty_runtime_plan(),
    );
    let target = callable_target("infer.matmul_bias_add_f32");

    let Some(Ok(first)) = RuntimeExternalCallBackend::call_external(
        &mut accelerator,
        &target,
        &[
            RuntimeValue::tensor_f32(lhs.clone()),
            RuntimeValue::tensor_f32(rhs.clone()),
            RuntimeValue::tensor_f32(bias.clone()),
        ],
    ) else {
        return;
    };
    let RuntimeValue::TensorF32(first) = first else {
        panic!("matmul-bias external call returns a tensor");
    };
    assert_eq!(first.shape().dims(), &[16, 16]);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 7);

    accelerator.reset_runtime_counters();
    let expected_arg_bytes = (lhs.values().len() + rhs.values().len() + bias.values().len())
        * std::mem::size_of::<f32>();
    let Some(Ok(second)) = RuntimeExternalCallBackend::call_external(
        &mut accelerator,
        &target,
        &[
            RuntimeValue::tensor_f32(lhs),
            RuntimeValue::tensor_f32(rhs),
            RuntimeValue::tensor_f32(bias),
        ],
    ) else {
        panic!("prepared matmul-bias external call cache is reusable");
    };
    let RuntimeValue::TensorF32(second) = second else {
        panic!("matmul-bias external call returns a tensor");
    };

    assert_eq!(second.values(), first.values());
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().fused_matmul_bias_add_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 7);
    assert_eq!(accelerator.math_stats().gpu_reused_dispatches, 1);
    assert_eq!(accelerator.math_stats().bytes_uploaded, 0);
    assert_eq!(
        accelerator.math_stats().bytes_downloaded,
        std::mem::size_of_val(second.values())
    );
    assert_eq!(accelerator.stats().arg_bytes_borrowed, expected_arg_bytes);
    assert_eq!(
        accelerator.stats().result_bytes_copied,
        std::mem::size_of_val(second.values())
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_auto_wgpu_matmul_uses_prepared_cache_when_threshold_selects_gpu() {
    let lhs = DenseMatrixF32::new(8, 8, vec![1.0; 64]).expect("matrix shape is valid");
    let rhs = DenseMatrixF32::new(8, 8, vec![2.0; 64]).expect("matrix shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Auto,
                wgpu_min_elements: 1,
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &empty_runtime_plan(),
    );

    let Ok(first) = RuntimeMathCallBackend::call_math_matmul_f32(&mut accelerator, &lhs, &rhs)
    else {
        return;
    };
    assert_eq!(
        accelerator.math_stats().last_auto_reason,
        Some(math::RuntimeMathAutoSelectionReason::MatmulWgpuWorkThreshold)
    );

    accelerator.reset_runtime_counters();
    let second = RuntimeMathCallBackend::call_math_matmul_f32(&mut accelerator, &lhs, &rhs)
        .expect("auto-selected wgpu matmul reuses prepared runtime cache");

    assert_eq!(second.values(), first.values());
    assert_eq!(
        accelerator.math_stats().last_auto_reason,
        Some(math::RuntimeMathAutoSelectionReason::MatmulWgpuWorkThreshold)
    );
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 4);
    assert_eq!(accelerator.math_stats().bytes_uploaded, 0);
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_auto_wgpu_matmul_reuses_capacity_cache_for_smaller_shape() {
    let lhs = DenseMatrixF32::new(16, 16, vec![1.0; 256]).expect("matrix shape is valid");
    let rhs = DenseMatrixF32::new(16, 16, vec![2.0; 256]).expect("matrix shape is valid");
    let smaller_lhs = DenseMatrixF32::new(8, 8, vec![3.0; 64]).expect("matrix shape is valid");
    let smaller_rhs = DenseMatrixF32::new(8, 8, vec![0.5; 64]).expect("matrix shape is valid");
    let expected = smaller_lhs
        .matmul_scalar(&smaller_rhs)
        .expect("scalar matmul succeeds");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Auto,
                wgpu_min_elements: 1,
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &empty_runtime_plan(),
    );

    if RuntimeMathCallBackend::call_math_matmul_f32(&mut accelerator, &lhs, &rhs).is_err() {
        return;
    }

    accelerator.reset_runtime_counters();
    let second =
        RuntimeMathCallBackend::call_math_matmul_f32(&mut accelerator, &smaller_lhs, &smaller_rhs)
            .expect("auto-selected wgpu matmul reuses capacity-prepared runtime cache");

    assert_eq!(second.values(), expected.values());
    assert_eq!(
        accelerator.math_stats().last_auto_reason,
        Some(math::RuntimeMathAutoSelectionReason::MatmulWgpuWorkThreshold)
    );
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 7);
    assert_eq!(
        accelerator.math_stats().bytes_uploaded,
        (smaller_lhs.values().len() + smaller_rhs.values().len()) * std::mem::size_of::<f32>()
    );
    assert_eq!(
        accelerator.math_stats().bytes_downloaded,
        std::mem::size_of_val(second.values())
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_wgpu_math_cache_reuses_prepared_tensor_add_buffers() {
    let lhs = DenseTensorF32::new(vec![32], vec![1.0; 32]).expect("tensor shape is valid");
    let rhs = DenseTensorF32::new(vec![32], vec![2.0; 32]).expect("tensor shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Wgpu,
                ..math::RuntimeMathAcceleratorConfig::default()
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &empty_runtime_plan(),
    );

    let Ok(first) = RuntimeMathCallBackend::call_math_tensor_add_f32(&mut accelerator, &lhs, &rhs)
    else {
        return;
    };
    assert_eq!(first.values(), vec![3.0; 32].as_slice());

    accelerator.reset_runtime_counters();
    let second = RuntimeMathCallBackend::call_math_tensor_add_f32(&mut accelerator, &lhs, &rhs)
        .expect("prepared runtime tensor add cache is reusable");

    assert_eq!(second.values(), vec![3.0; 32].as_slice());
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 4);
    assert_eq!(accelerator.math_stats().bytes_uploaded, 0);
    assert_eq!(
        accelerator.math_stats().bytes_downloaded,
        std::mem::size_of_val(second.values())
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_auto_wgpu_matrix_add_uses_prepared_cache_when_threshold_selects_gpu() {
    let lhs = DenseMatrixF32::new(8, 8, vec![1.0; 64]).expect("matrix shape is valid");
    let rhs = DenseMatrixF32::new(8, 8, vec![2.0; 64]).expect("matrix shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Auto,
                wgpu_min_elements: 1,
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &empty_runtime_plan(),
    );

    let Ok(first) = RuntimeMathCallBackend::call_math_matrix_add_f32(&mut accelerator, &lhs, &rhs)
    else {
        return;
    };
    assert_eq!(
        accelerator.math_stats().last_auto_reason,
        Some(math::RuntimeMathAutoSelectionReason::ElementwiseWgpuWorkThreshold)
    );

    accelerator.reset_runtime_counters();
    let second = RuntimeMathCallBackend::call_math_matrix_add_f32(&mut accelerator, &lhs, &rhs)
        .expect("auto-selected wgpu matrix add reuses prepared runtime cache");

    assert_eq!(second.values(), first.values());
    assert_eq!(
        accelerator.math_stats().last_auto_reason,
        Some(math::RuntimeMathAutoSelectionReason::ElementwiseWgpuWorkThreshold)
    );
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 4);
    assert_eq!(accelerator.math_stats().bytes_uploaded, 0);
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_auto_wgpu_matrix_add_reuses_capacity_cache_for_smaller_shape() {
    let lhs = DenseMatrixF32::new(16, 16, vec![1.0; 256]).expect("matrix shape is valid");
    let rhs = DenseMatrixF32::new(16, 16, vec![2.0; 256]).expect("matrix shape is valid");
    let smaller_lhs = DenseMatrixF32::new(8, 8, vec![4.0; 64]).expect("matrix shape is valid");
    let smaller_rhs = DenseMatrixF32::new(8, 8, vec![5.0; 64]).expect("matrix shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Auto,
                wgpu_min_elements: 1,
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &empty_runtime_plan(),
    );

    if RuntimeMathCallBackend::call_math_matrix_add_f32(&mut accelerator, &lhs, &rhs).is_err() {
        return;
    }

    accelerator.reset_runtime_counters();
    let second = RuntimeMathCallBackend::call_math_matrix_add_f32(
        &mut accelerator,
        &smaller_lhs,
        &smaller_rhs,
    )
    .expect("auto-selected wgpu matrix add reuses capacity-prepared runtime cache");

    assert_eq!(second.values(), vec![9.0; 64].as_slice());
    assert_eq!(
        accelerator.math_stats().last_auto_reason,
        Some(math::RuntimeMathAutoSelectionReason::ElementwiseWgpuWorkThreshold)
    );
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 7);
    assert_eq!(
        accelerator.math_stats().bytes_uploaded,
        (smaller_lhs.values().len() + smaller_rhs.values().len()) * std::mem::size_of::<f32>()
    );
    assert_eq!(
        accelerator.math_stats().bytes_downloaded,
        std::mem::size_of_val(second.values())
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_wgpu_math_cache_updates_prepared_matrix_add_inputs_for_same_shape() {
    let lhs = DenseMatrixF32::new(8, 8, vec![1.0; 64]).expect("matrix shape is valid");
    let rhs = DenseMatrixF32::new(8, 8, vec![2.0; 64]).expect("matrix shape is valid");
    let changed_lhs = DenseMatrixF32::new(8, 8, vec![4.0; 64]).expect("matrix shape is valid");
    let changed_rhs = DenseMatrixF32::new(8, 8, vec![5.0; 64]).expect("matrix shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Wgpu,
                ..math::RuntimeMathAcceleratorConfig::default()
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &empty_runtime_plan(),
    );

    if RuntimeMathCallBackend::call_math_matrix_add_f32(&mut accelerator, &lhs, &rhs).is_err() {
        return;
    }

    accelerator.reset_runtime_counters();
    let second = RuntimeMathCallBackend::call_math_matrix_add_f32(
        &mut accelerator,
        &changed_lhs,
        &changed_rhs,
    )
    .expect("prepared runtime matrix add cache updates same-shape inputs");

    assert_eq!(second.values(), vec![9.0; 64].as_slice());
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 7);
    assert_eq!(
        accelerator.math_stats().bytes_uploaded,
        (changed_lhs.values().len() + changed_rhs.values().len()) * std::mem::size_of::<f32>()
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_auto_wgpu_tensor_add_uses_prepared_cache_when_threshold_selects_gpu() {
    let lhs = DenseTensorF32::new(vec![64], vec![1.0; 64]).expect("tensor shape is valid");
    let rhs = DenseTensorF32::new(vec![64], vec![2.0; 64]).expect("tensor shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Auto,
                wgpu_min_elements: 1,
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &empty_runtime_plan(),
    );

    let Ok(first) = RuntimeMathCallBackend::call_math_tensor_add_f32(&mut accelerator, &lhs, &rhs)
    else {
        return;
    };
    assert_eq!(
        accelerator.math_stats().last_auto_reason,
        Some(math::RuntimeMathAutoSelectionReason::ElementwiseWgpuWorkThreshold)
    );

    accelerator.reset_runtime_counters();
    let second = RuntimeMathCallBackend::call_math_tensor_add_f32(&mut accelerator, &lhs, &rhs)
        .expect("auto-selected wgpu tensor add reuses prepared runtime cache");

    assert_eq!(second.values(), first.values());
    assert_eq!(
        accelerator.math_stats().last_auto_reason,
        Some(math::RuntimeMathAutoSelectionReason::ElementwiseWgpuWorkThreshold)
    );
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 4);
    assert_eq!(accelerator.math_stats().bytes_uploaded, 0);
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_auto_wgpu_tensor_add_reuses_capacity_cache_for_smaller_len() {
    let lhs = DenseTensorF32::new(vec![128], vec![1.0; 128]).expect("tensor shape is valid");
    let rhs = DenseTensorF32::new(vec![128], vec![2.0; 128]).expect("tensor shape is valid");
    let smaller_lhs = DenseTensorF32::new(vec![64], vec![6.0; 64]).expect("tensor shape is valid");
    let smaller_rhs = DenseTensorF32::new(vec![64], vec![7.0; 64]).expect("tensor shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Auto,
                wgpu_min_elements: 1,
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &empty_runtime_plan(),
    );

    if RuntimeMathCallBackend::call_math_tensor_add_f32(&mut accelerator, &lhs, &rhs).is_err() {
        return;
    }

    accelerator.reset_runtime_counters();
    let second = RuntimeMathCallBackend::call_math_tensor_add_f32(
        &mut accelerator,
        &smaller_lhs,
        &smaller_rhs,
    )
    .expect("auto-selected wgpu tensor add reuses capacity-prepared runtime cache");

    assert_eq!(second.values(), vec![13.0; 64].as_slice());
    assert_eq!(
        accelerator.math_stats().last_auto_reason,
        Some(math::RuntimeMathAutoSelectionReason::ElementwiseWgpuWorkThreshold)
    );
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 7);
    assert_eq!(
        accelerator.math_stats().bytes_uploaded,
        (smaller_lhs.values().len() + smaller_rhs.values().len()) * std::mem::size_of::<f32>()
    );
    assert_eq!(
        accelerator.math_stats().bytes_downloaded,
        std::mem::size_of_val(second.values())
    );
}

#[cfg(all(feature = "math-wgpu", not(target_arch = "wasm32")))]
#[test]
fn runtime_wgpu_math_cache_updates_prepared_tensor_add_inputs_for_same_shape() {
    let lhs = DenseTensorF32::new(vec![64], vec![1.0; 64]).expect("tensor shape is valid");
    let rhs = DenseTensorF32::new(vec![64], vec![2.0; 64]).expect("tensor shape is valid");
    let changed_lhs = DenseTensorF32::new(vec![64], vec![6.0; 64]).expect("tensor shape is valid");
    let changed_rhs = DenseTensorF32::new(vec![64], vec![7.0; 64]).expect("tensor shape is valid");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            math: math::RuntimeMathAcceleratorConfig {
                backend: math::RuntimeMathBackend::Wgpu,
                ..math::RuntimeMathAcceleratorConfig::default()
            },
            ..RuntimePureAcceleratorConfig::default()
        },
        &empty_runtime_plan(),
    );

    if RuntimeMathCallBackend::call_math_tensor_add_f32(&mut accelerator, &lhs, &rhs).is_err() {
        return;
    }

    accelerator.reset_runtime_counters();
    let second = RuntimeMathCallBackend::call_math_tensor_add_f32(
        &mut accelerator,
        &changed_lhs,
        &changed_rhs,
    )
    .expect("prepared runtime tensor add cache updates same-shape inputs");

    assert_eq!(second.values(), vec![13.0; 64].as_slice());
    assert_eq!(accelerator.math_stats().wgpu_calls, 1);
    assert_eq!(accelerator.math_stats().gpu_buffer_creations, 0);
    assert_eq!(accelerator.math_stats().gpu_buffer_reuse_hits, 7);
    assert_eq!(
        accelerator.math_stats().bytes_uploaded,
        (changed_lhs.values().len() + changed_rhs.values().len()) * std::mem::size_of::<f32>()
    );
}

#[test]
fn auto_accelerator_uses_aot_for_cold_scalar_calls_without_value_vec_allocation() {
    let helper = mul_add_helper(
        "score",
        RuntimePureInputType::I64,
        RuntimePureOutputType::I64,
        RuntimeValue::i64(2),
        RuntimePureHelperOrigin::Annotated,
    );
    let mut accelerator = RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, helper.plan());

    let value = accelerator
        .call_i64(helper.helper_ref(), RuntimeI64Args::new([3, 4, 0, 0], 2))
        .expect("accelerated call succeeds");

    assert_eq!(value, Some(18));
    assert_eq!(accelerator.stats().pure_calls, 1);
    assert_eq!(accelerator.stats().arg_stack_packs, 1);
    assert_eq!(accelerator.stats().arg_vec_allocations, 0);
    assert_eq!(
        accelerator.stats().arg_bytes_copied,
        2 * std::mem::size_of::<i64>()
    );
    assert_eq!(accelerator.stats().result_bytes_copied, 0);
    assert!(accelerator.resolved_worker_count() >= 1);
    assert!(!accelerator.has_worker_pool());
    assert_eq!(accelerator.summary().aot, 1);
    assert_eq!(accelerator.summary().jit, 0);
    assert_eq!(accelerator.compile_stats().auto_aot_selected, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().object_attempts, 0);
    assert_eq!(accelerator.compile_stats().object_bytes, 0);
}

#[test]
fn aot_scalar_preserves_i32_and_f32_without_vm_fallback() {
    let i32_helper = add_helper(
        "i32_score",
        RuntimePureInputType::I32,
        RuntimePureOutputType::I32,
    );
    let f32_helper = mul_add_helper(
        "f32_score",
        RuntimePureInputType::F32,
        RuntimePureOutputType::F32,
        RuntimeValue::f32(0.0),
        RuntimePureHelperOrigin::Annotated,
    );
    let config = RuntimePureAcceleratorConfig {
        backend: RuntimePureBackendMode::Aot,
        workers: RuntimePureWorkerCount::Fixed(1),
        batch_min_len: 1024,
        emit_object_artifacts: true,
        ..RuntimePureAcceleratorConfig::default()
    };
    let mut i32_accelerator = RuntimePureAccelerator::with_config(config, i32_helper.plan());
    let mut f32_accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Aot,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            emit_object_artifacts: true,
            ..RuntimePureAcceleratorConfig::default()
        },
        f32_helper.plan(),
    );

    let i32_value = i32_accelerator
        .call_i32_slice(i32_helper.helper_ref(), &[7, 9])
        .expect("i32 AOT scalar succeeds");
    let f32_value = f32_accelerator
        .call_f32_slice(f32_helper.helper_ref(), &[3.5, 2.0])
        .expect("f32 AOT scalar succeeds");
    let mut i32_out = [0; 3];
    i32_accelerator
        .call_i32_flat_batch(
            i32_helper.helper_ref(),
            &[1, 2, 3, 4, 5, 6],
            2,
            &mut i32_out,
        )
        .expect("i32 AOT flat batch succeeds");
    let i32_sum = i32_accelerator
        .call_i32_flat_batch_sum(i32_helper.helper_ref(), &[1, 2, 3, 4, 5, 6], 2, 3)
        .expect("i32 AOT flat batch sum succeeds");

    assert_eq!(i32_value, Some(16));
    assert_eq!(f32_value, Some(7.0));
    assert_eq!(i32_out, [3, 7, 11]);
    assert_eq!(i32_sum, 21);
    assert_eq!(
        i32_accelerator.stats().aot_calls + f32_accelerator.stats().aot_calls,
        8
    );
    assert_eq!(
        i32_accelerator.stats().vm_calls + f32_accelerator.stats().vm_calls,
        0
    );
    assert_eq!(
        i32_accelerator.stats().fallbacks + f32_accelerator.stats().fallbacks,
        0
    );
    assert_eq!(
        i32_accelerator.summary().aot + f32_accelerator.summary().aot,
        2
    );
    #[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
    {
        let i32_compile = i32_accelerator.compile_stats();
        let f32_compile = f32_accelerator.compile_stats();
        assert_eq!(i32_compile.object_attempts + f32_compile.object_attempts, 2);
        assert_eq!(
            i32_compile.object_successes + f32_compile.object_successes,
            2
        );
        assert_eq!(i32_compile.object_failures + f32_compile.object_failures, 0);
        assert!(i32_compile.object_bytes + f32_compile.object_bytes > 0);
    }
    #[cfg(not(all(feature = "native-jit", not(target_arch = "wasm32"))))]
    {
        assert_eq!(i32_accelerator.compile_stats().object_attempts, 0);
        assert_eq!(f32_accelerator.compile_stats().object_attempts, 0);
    }
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn explicit_jit_uses_native_i16_for_slice_and_flat_batch() {
    let helper = mul_add_helper(
        "i16_score",
        RuntimePureInputType::I16,
        RuntimePureOutputType::I16,
        RuntimeValue::i16(2),
        RuntimePureHelperOrigin::Annotated,
    );
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Jit,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        helper.plan(),
    );
    let value = accelerator
        .call_i16_slice(helper.helper_ref(), &[30, 4])
        .expect("native i16 JIT slice call succeeds");
    let mut out = [0; 3];
    accelerator
        .call_i16_flat_batch(helper.helper_ref(), &[30, 4, -20, 1, 70, 1], 2, &mut out)
        .expect("native i16 JIT flat batch succeeds");
    let sum = accelerator
        .call_i16_flat_batch_sum(helper.helper_ref(), &[30, 4, -20, 1, 70, 1], 2, 3)
        .expect("native i16 JIT flat batch sum succeeds");

    assert_eq!(value, Some(180));
    assert_eq!(out, [180, -60, 210]);
    assert_eq!(sum, 330);
    assert_eq!(accelerator.stats().jit_calls, 7);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.stats().fallbacks, 0);
    assert_eq!(accelerator.compile_stats().jit_attempts, 1);
    assert_eq!(accelerator.compile_stats().jit_failures, 0);
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn explicit_jit_uses_native_i32_for_slice_and_flat_batch() {
    let helper = mul_add_helper(
        "i32_score_jit",
        RuntimePureInputType::I32,
        RuntimePureOutputType::I32,
        RuntimeValue::i32(2),
        RuntimePureHelperOrigin::Annotated,
    );
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Jit,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        helper.plan(),
    );

    let value = accelerator
        .call_i32_slice(helper.helper_ref(), &[3, 4])
        .expect("native i32 JIT slice call succeeds");
    let mut out = [0; 3];
    accelerator
        .call_i32_flat_batch(helper.helper_ref(), &[3, 4, 2, 99, 7, 1], 2, &mut out)
        .expect("native i32 JIT flat batch succeeds");
    let sum = accelerator
        .call_i32_flat_batch_sum(helper.helper_ref(), &[3, 4, 2, 99, 7, 1], 2, 3)
        .expect("native i32 JIT flat batch sum succeeds");

    assert_eq!(value, Some(18));
    assert_eq!(out, [18, 202, 21]);
    assert_eq!(sum, 241);
    assert_eq!(accelerator.stats().jit_calls, 7);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn explicit_jit_uses_native_u32_for_slice_and_flat_batch() {
    let helper = conditional_div_helper(
        "u32_score_jit",
        RuntimePureInputType::U32,
        RuntimePureOutputType::U32,
        RuntimeValue::u32(u32::MAX - 4),
        RuntimeValue::u32(1),
        RuntimeValue::u32(0),
    );
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Jit,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        helper.plan(),
    );

    let value = accelerator
        .call_u32_slice(helper.helper_ref(), &[u32::MAX - 1, 1])
        .expect("native u32 JIT slice call succeeds");
    let mut out = [0; 3];
    accelerator
        .call_u32_flat_batch(
            helper.helper_ref(),
            &[u32::MAX - 1, 1, 3, 99, u32::MAX, 4],
            2,
            &mut out,
        )
        .expect("native u32 JIT flat batch succeeds");
    let sum = accelerator
        .call_u32_flat_batch_sum(
            helper.helper_ref(),
            &[u32::MAX - 1, 1, 3, 99, u32::MAX, 4],
            2,
            3,
        )
        .expect("native u32 JIT flat batch sum succeeds");

    assert_eq!(value, Some((u32::MAX - 1) / 2));
    assert_eq!(out, [(u32::MAX - 1) / 2, 0, u32::MAX / 5]);
    assert_eq!(sum, i64::from((u32::MAX - 1) / 2) + i64::from(u32::MAX / 5));
    assert_eq!(accelerator.stats().jit_calls, 7);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn explicit_jit_uses_native_u64_for_slice_and_flat_batch() {
    let helper = conditional_div_helper(
        "u64_score_jit",
        RuntimePureInputType::U64,
        RuntimePureOutputType::U64,
        RuntimeValue::u64(5),
        RuntimeValue::u64(1),
        RuntimeValue::u64(0),
    );
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Jit,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        helper.plan(),
    );

    let value = accelerator
        .call_u64_slice(helper.helper_ref(), &[8, 1])
        .expect("native u64 JIT slice call succeeds");
    let mut out = [0; 3];
    accelerator
        .call_u64_flat_batch(helper.helper_ref(), &[8, 1, 3, 99, 10, 4], 2, &mut out)
        .expect("native u64 JIT flat batch succeeds");
    let sum = accelerator
        .call_u64_flat_batch_sum(helper.helper_ref(), &[8, 1, 3, 99, 10, 4], 2, 3)
        .expect("native u64 JIT flat batch sum succeeds");

    assert_eq!(value, Some(4));
    assert_eq!(out, [4, 0, 2]);
    assert_eq!(sum, 6);
    assert_eq!(accelerator.stats().jit_calls, 7);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn explicit_jit_uses_native_f32_for_slice_and_flat_batch() {
    let helper = mul_add_helper(
        "f32_score_jit",
        RuntimePureInputType::F32,
        RuntimePureOutputType::F32,
        RuntimeValue::f32(2.0),
        RuntimePureHelperOrigin::Annotated,
    );
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Jit,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        helper.plan(),
    );

    let value = accelerator
        .call_f32_slice(helper.helper_ref(), &[3.0, 4.0])
        .expect("native f32 JIT slice call succeeds");
    let mut out = [0.0; 3];
    accelerator
        .call_f32_flat_batch(
            helper.helper_ref(),
            &[3.0, 4.0, 2.0, 99.0, 7.0, 1.0],
            2,
            &mut out,
        )
        .expect("native f32 JIT flat batch succeeds");

    assert_eq!(value.map(f32::to_bits), Some(18.0f32.to_bits()));
    assert_eq!(
        out.map(f32::to_bits),
        [18.0f32, 202.0, 21.0].map(f32::to_bits)
    );
    assert_eq!(accelerator.stats().jit_calls, 4);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(
        accelerator.stats().flat_batch_bytes_borrowed,
        6 * std::mem::size_of::<f32>()
    );
    assert_eq!(
        accelerator.stats().result_bytes_copied,
        3 * std::mem::size_of::<f32>()
    );
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn explicit_jit_uses_native_f64_for_slice_and_flat_batch() {
    let helper = mul_add_helper(
        "f64_score_jit",
        RuntimePureInputType::F64,
        RuntimePureOutputType::F64,
        RuntimeValue::f64(2.0),
        RuntimePureHelperOrigin::Annotated,
    );
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Jit,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        helper.plan(),
    );

    let value = accelerator
        .call_f64_slice(helper.helper_ref(), &[3.0, 4.0])
        .expect("native f64 JIT slice call succeeds");
    let mut out = [0.0; 3];
    accelerator
        .call_f64_flat_batch(
            helper.helper_ref(),
            &[3.0, 4.0, 2.0, 99.0, 7.0, 1.0],
            2,
            &mut out,
        )
        .expect("native f64 JIT flat batch succeeds");

    assert_eq!(value.map(f64::to_bits), Some(18.0f64.to_bits()));
    assert_eq!(
        out.map(f64::to_bits),
        [18.0f64, 202.0, 21.0].map(f64::to_bits)
    );
    assert_eq!(accelerator.stats().jit_calls, 4);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(
        accelerator.stats().flat_batch_bytes_borrowed,
        6 * std::mem::size_of::<f64>()
    );
    assert_eq!(
        accelerator.stats().result_bytes_copied,
        3 * std::mem::size_of::<f64>()
    );
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn auto_promotes_large_i32_flat_batch_to_native_jit() {
    let helper = add_helper(
        "i32_score_auto_jit",
        RuntimePureInputType::I32,
        RuntimePureOutputType::I32,
    );
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Auto,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        helper.plan(),
    );
    let flat_inputs = (0..128).flat_map(|value| [value, 1]).collect::<Vec<i32>>();
    let mut out = [0; 128];

    accelerator
        .call_i32_flat_batch(helper.helper_ref(), &flat_inputs, 2, &mut out)
        .expect("auto promotes large i32 flat batch");

    assert_eq!(out[0], 1);
    assert_eq!(out[127], 128);
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn auto_promotes_large_u32_flat_batch_to_native_jit() {
    let helper = add_helper(
        "u32_score_auto_jit",
        RuntimePureInputType::U32,
        RuntimePureOutputType::U32,
    );
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Auto,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        helper.plan(),
    );
    let flat_inputs = (0..128).flat_map(|value| [value, 1]).collect::<Vec<u32>>();
    let mut out = [0; 128];

    accelerator
        .call_u32_flat_batch(helper.helper_ref(), &flat_inputs, 2, &mut out)
        .expect("auto promotes large u32 flat batch");

    assert_eq!(out[0], 1);
    assert_eq!(out[127], 128);
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
fn exact_int_add_helper(
    name: &str,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
    one: RuntimeValue,
) -> AdmittedHelper {
    scalar_add_helper(name, input_type, output_type, one)
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
fn scalar_add_helper(
    name: &str,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
    one: RuntimeValue,
) -> AdmittedHelper {
    let ty = helper_type_identity(input_type);
    admit_helper(
        name,
        vec![input_type, input_type],
        output_type,
        true,
        RuntimePureHelperOrigin::Annotated,
        move |inputs, output_ty| {
            binary_expr(
                output_ty,
                local_expr(ty, inputs[0].clone()),
                RuntimeBinaryOp::Add,
                binary_expr(
                    output_ty,
                    local_expr(ty, inputs[1].clone()),
                    RuntimeBinaryOp::Add,
                    value_expr(output_ty, one),
                ),
            )
        },
    )
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
fn assert_exact_int_scalar_jit<T>(name: &str, args: &[T], one: T, expected: T)
where
    T: RuntimePureScalarInteger + PartialEq + std::fmt::Debug,
{
    let helper = exact_int_add_helper(
        name,
        T::INPUT_TYPE,
        T::OUTPUT_TYPE,
        one.into_runtime_value(),
    );
    let mut accelerator = RuntimePureAccelerator::new(RuntimePureBackendMode::Jit, helper.plan());

    let value = accelerator
        .call_exact_int_slice::<T>(helper.helper_ref(), args)
        .expect("generic exact-int call succeeds");

    assert_eq!(value, Some(expected));
    assert_eq!(accelerator.stats().jit_calls, 1);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.stats().fallbacks, 0);
    assert_eq!(accelerator.stats().arg_vec_allocations, 0);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn generic_exact_int_scalar_call_recognizes_width_specific_jit_entry() {
    assert_exact_int_scalar_jit("i8_generic_jit", &[3_i8, 5_i8], 1_i8, 9_i8);
    assert_exact_int_scalar_jit("i16_generic_jit", &[7_i16, 11_i16], 1_i16, 19_i16);
    assert_exact_int_scalar_jit("i32_generic_jit", &[19_i32, 23_i32], 1_i32, 43_i32);
    assert_exact_int_scalar_jit("u16_generic_jit", &[13_u16, 17_u16], 1_u16, 31_u16);
    assert_exact_int_scalar_jit("u8_generic_jit", &[5_u8, 7_u8], 1_u8, 13_u8);
    assert_exact_int_scalar_jit("u32_generic_jit", &[29_u32, 31_u32], 1_u32, 61_u32);
    assert_exact_int_scalar_jit("u64_generic_jit", &[41_u64, 43_u64], 1_u64, 85_u64);
    assert_exact_int_scalar_jit("i128_generic_jit", &[53_i128, 59_i128], 1_i128, 113_i128);
    assert_exact_int_scalar_jit(
        "isize_generic_jit",
        &[RuntimeISizeValue::new(37), RuntimeISizeValue::new(41)],
        RuntimeISizeValue::new(1),
        RuntimeISizeValue::new(79),
    );
    assert_exact_int_scalar_jit(
        "usize_generic_jit",
        &[RuntimeUSizeValue::new(43), RuntimeUSizeValue::new(47)],
        RuntimeUSizeValue::new(1),
        RuntimeUSizeValue::new(91),
    );
    assert_exact_int_scalar_jit("u128_generic_jit", &[61_u128, 67_u128], 1_u128, 129_u128);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn auto_promotes_hot_scalar_exact_int_calls_to_native_jit() {
    let helper = scalar_add_helper(
        "i128_hot_scalar_auto_jit",
        RuntimePureInputType::I128,
        RuntimePureOutputType::I128,
        RuntimeValue::i128(1),
    );
    let mut accelerator = RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, helper.plan());

    for value in 0..160 {
        let actual = accelerator
            .call_exact_int_slice::<i128>(helper.helper_ref(), &[value, 11])
            .expect("hot scalar i128 call succeeds");
        assert_eq!(actual, Some(value + 12));
    }

    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
    assert!(accelerator.stats().aot_calls > 0);
    assert!(accelerator.stats().jit_calls > 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.stats().fallbacks, 0);
    assert_eq!(accelerator.stats().arg_vec_allocations, 0);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn auto_promotes_hot_scalar_float_calls_to_native_jit() {
    let helper = scalar_add_helper(
        "f32_hot_scalar_auto_jit",
        RuntimePureInputType::F32,
        RuntimePureOutputType::F32,
        RuntimeValue::f32(1.0),
    );
    let mut accelerator = RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, helper.plan());

    for value in 0_u16..160 {
        let base = f32::from(value);
        let actual = accelerator
            .call_f32_slice(helper.helper_ref(), &[base, 11.0])
            .expect("hot scalar f32 call succeeds");
        assert_eq!(actual, Some(base + 12.0));
    }

    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
    assert!(accelerator.stats().aot_calls > 0);
    assert!(accelerator.stats().jit_calls > 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.stats().fallbacks, 0);
    assert_eq!(accelerator.stats().arg_vec_allocations, 0);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn auto_promotes_small_integer_flat_batches_to_native_jit() {
    let helper = exact_int_add_helper(
        "i8_auto_jit",
        RuntimePureInputType::I8,
        RuntimePureOutputType::I8,
        RuntimeValue::i8(1),
    );
    let mut accelerator = RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, helper.plan());
    let flat_inputs = (0..64).flat_map(|value| [value, 1]).collect::<Vec<i8>>();
    let mut out = [0; 64];
    accelerator
        .call_i8_flat_batch(helper.helper_ref(), &flat_inputs, 2, &mut out)
        .expect("auto promotes large i8 flat batch");
    assert_eq!(out[0], 2);
    assert_eq!(out[63], 65);
    assert_eq!(accelerator.stats().jit_calls, 64);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);

    let helper = exact_int_add_helper(
        "i16_auto_jit",
        RuntimePureInputType::I16,
        RuntimePureOutputType::I16,
        RuntimeValue::i16(1),
    );
    let mut accelerator = RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, helper.plan());
    let flat_inputs = (0..128).flat_map(|value| [value, 1]).collect::<Vec<i16>>();
    let mut out = [0; 128];
    accelerator
        .call_i16_flat_batch(helper.helper_ref(), &flat_inputs, 2, &mut out)
        .expect("auto promotes large i16 flat batch");
    assert_eq!(out[0], 2);
    assert_eq!(out[127], 129);
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);

    let helper = exact_int_add_helper(
        "u8_auto_jit",
        RuntimePureInputType::U8,
        RuntimePureOutputType::U8,
        RuntimeValue::u8(1),
    );
    let mut accelerator = RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, helper.plan());
    let flat_inputs = (0..128).flat_map(|value| [value, 1]).collect::<Vec<u8>>();
    let mut out = [0; 128];
    accelerator
        .call_u8_flat_batch(helper.helper_ref(), &flat_inputs, 2, &mut out)
        .expect("auto promotes large u8 flat batch");
    assert_eq!(out[0], 2);
    assert_eq!(out[127], 129);
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);

    let helper = exact_int_add_helper(
        "u16_auto_jit",
        RuntimePureInputType::U16,
        RuntimePureOutputType::U16,
        RuntimeValue::u16(1),
    );
    let mut accelerator = RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, helper.plan());
    let flat_inputs = (0..128).flat_map(|value| [value, 1]).collect::<Vec<u16>>();
    let mut out = [0; 128];
    accelerator
        .call_u16_flat_batch(helper.helper_ref(), &flat_inputs, 2, &mut out)
        .expect("auto promotes large u16 flat batch");
    assert_eq!(out[0], 2);
    assert_eq!(out[127], 129);
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn auto_promotes_target_size_integer_flat_batches_to_native_jit() {
    let helper = exact_int_add_helper(
        "isize_auto_jit",
        RuntimePureInputType::ISize,
        RuntimePureOutputType::ISize,
        RuntimeValue::isize(1),
    );
    let mut accelerator = RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, helper.plan());
    let flat_inputs = (0..128_i64)
        .flat_map(|value| [RuntimeISizeValue::new(value), RuntimeISizeValue::new(1)])
        .collect::<Vec<_>>();
    let mut out = [RuntimeISizeValue::new(0); 128];
    accelerator
        .call_exact_int_flat_batch(helper.helper_ref(), &flat_inputs, 2, &mut out)
        .expect("auto promotes large isize flat batch");
    let sum = accelerator
        .call_exact_int_flat_batch_sum(helper.helper_ref(), &flat_inputs, 2, 128)
        .expect("native isize flat batch sum succeeds");
    assert_eq!(out[0], RuntimeISizeValue::new(2));
    assert_eq!(out[127], RuntimeISizeValue::new(129));
    assert_eq!(sum, 8_384);
    assert_eq!(accelerator.stats().jit_calls, 256);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.stats().fallbacks, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);

    let helper = exact_int_add_helper(
        "usize_auto_jit",
        RuntimePureInputType::USize,
        RuntimePureOutputType::USize,
        RuntimeValue::usize(1),
    );
    let mut accelerator = RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, helper.plan());
    let flat_inputs = (0..128_u64)
        .flat_map(|value| [RuntimeUSizeValue::new(value), RuntimeUSizeValue::new(1)])
        .collect::<Vec<_>>();
    let mut out = [RuntimeUSizeValue::new(0); 128];
    accelerator
        .call_exact_int_flat_batch(helper.helper_ref(), &flat_inputs, 2, &mut out)
        .expect("auto promotes large usize flat batch");
    let sum = accelerator
        .call_exact_int_flat_batch_sum(helper.helper_ref(), &flat_inputs, 2, 128)
        .expect("native usize flat batch sum succeeds");
    assert_eq!(out[0], RuntimeUSizeValue::new(2));
    assert_eq!(out[127], RuntimeUSizeValue::new(129));
    assert_eq!(sum, 8_384);
    assert_eq!(accelerator.stats().jit_calls, 256);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.stats().fallbacks, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn auto_promotes_wide_integer_flat_batches_to_native_jit() {
    let helper = exact_int_add_helper(
        "i128_auto_jit",
        RuntimePureInputType::I128,
        RuntimePureOutputType::I128,
        RuntimeValue::i128(1),
    );
    let mut accelerator = RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, helper.plan());
    let flat_inputs = (0..128)
        .flat_map(|value| [i128::from(value), 1])
        .collect::<Vec<i128>>();
    let mut out = [0; 128];
    accelerator
        .call_i128_flat_batch(helper.helper_ref(), &flat_inputs, 2, &mut out)
        .expect("auto promotes large i128 flat batch");
    assert_eq!(out[0], 2);
    assert_eq!(out[127], 129);
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);

    let helper = exact_int_add_helper(
        "u128_auto_jit",
        RuntimePureInputType::U128,
        RuntimePureOutputType::U128,
        RuntimeValue::u128(1),
    );
    let mut accelerator = RuntimePureAccelerator::new(RuntimePureBackendMode::Auto, helper.plan());
    let flat_inputs = (0..128)
        .flat_map(|value: u16| [u128::from(value), 1])
        .collect::<Vec<u128>>();
    let mut out = [0; 128];
    accelerator
        .call_u128_flat_batch(helper.helper_ref(), &flat_inputs, 2, &mut out)
        .expect("auto promotes large u128 flat batch");
    assert_eq!(out[0], 2);
    assert_eq!(out[127], 129);
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_deferred, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
fn dense_u32_map_sum_plan() -> Arc<RuntimePlan> {
    let u32_ty = helper_type_identity(RuntimePureInputType::U32);
    let u32_seq_ty = RuntimeSemanticTypeId::from_bytes([17; 32]);
    let u32_mapping_ty = RuntimeSemanticTypeId::from_bytes([18; 32]);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_semantic_batch(
            [
                helper_type_seed(RuntimePureInputType::U32),
                RuntimePlanTypeSeed::new(
                    u32_seq_ty,
                    RuntimePlanTypeProjection::Sequence {
                        kind: RuntimePlanSequenceKind::Seq,
                        item: u32_ty,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    u32_mapping_ty,
                    RuntimePlanTypeProjection::Function {
                        parameters: Box::new([u32_ty]),
                        result: u32_ty,
                    },
                ),
            ],
            (0..3).map(|_| RuntimeLocalDeclarationSeed::new(u32_ty)),
            [],
            [],
        )
        .expect("u32 flow semantic inputs are admitted");
    let locals = admission.local_ids();
    let helper = builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            name: "u32_flow_score".to_owned(),
            inputs: vec![locals[1].clone(), locals[2].clone()].into_boxed_slice(),
            input_abi: vec![RuntimePureInputType::U32, RuntimePureInputType::U32],
            output_abi: RuntimePureOutputType::U32,
            body: binary_expr(
                u32_ty,
                local_expr(u32_ty, locals[1].clone()),
                RuntimeBinaryOp::Add,
                local_expr(u32_ty, locals[2].clone()),
            ),
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .expect("u32 flow helper is admitted");
    let mapping = builder
        .push_function_site_seed(
            [locals[0].clone()],
            [],
            RuntimeExprSeed::new(
                u32_ty,
                RuntimeExprSeedKind::PureCall {
                    helper,
                    args: Box::new([
                        RuntimeCallArgumentSeed::new(
                            local_expr(u32_ty, locals[0].clone()),
                            RuntimeCallArgumentMode::Value,
                        ),
                        RuntimeCallArgumentSeed::new(
                            value_expr(u32_ty, RuntimeValue::u32(1)),
                            RuntimeCallArgumentMode::Value,
                        ),
                    ]),
                },
            ),
        )
        .expect("u32 map callback is admitted");
    let flow = flow_id("flow.u32");
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: Vec::new(),
        })
        .expect("u32 flow schema is admitted");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow,
            [],
            vec![RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                u32_ty,
                RuntimeExprSeedKind::Sum {
                    source: Box::new(RuntimeExprSeed::new(
                        u32_seq_ty,
                        RuntimeExprSeedKind::StandardMap {
                            family: RuntimeStandardMapFamily::Seq,
                            order: RuntimeStandardMapOperandOrder::MappingThenReceiver,
                            mapping: Box::new(RuntimeExprSeed::new(
                                u32_mapping_ty,
                                RuntimeExprSeedKind::Function(mapping),
                            )),
                            source: Box::new(value_expr(
                                u32_seq_ty,
                                runtime_sequence_dense_u32((0..128).collect()),
                            )),
                        },
                    )),
                },
            ))],
        ))
        .expect("u32 flow is admitted");
    Arc::new(builder.finish().expect("u32 flow plan is sealed"))
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn runtime_flow_dense_u32_standard_map_applies_typed_callback() {
    let plan = dense_u32_map_sum_plan();
    let mut engine = Engine::for_flow(plan.as_ref().clone(), &flow_id("flow.u32"))
        .expect("test flow starts explicitly");
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Auto,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        &plan,
    );

    let result = engine.step_with_pure_backend(
        RuntimeStepInput::default(),
        RuntimeStepOptions::default(),
        &mut accelerator,
    );

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "8256"
    ));
    assert_eq!(result.stats.pure.pure_calls, 128);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn auto_promotes_large_f32_flat_batch_to_native_jit() {
    let helper = mul_add_helper(
        "f32_score_auto_jit",
        RuntimePureInputType::F32,
        RuntimePureOutputType::F32,
        RuntimeValue::f32(2.0),
        RuntimePureHelperOrigin::Annotated,
    );
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Auto,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        helper.plan(),
    );
    let flat_inputs = (1..=128)
        .flat_map(|value: u16| [f32::from(value), 2.0])
        .collect::<Vec<f32>>();
    let mut out = [0.0; 128];

    accelerator
        .call_f32_flat_batch(helper.helper_ref(), &flat_inputs, 2, &mut out)
        .expect("auto promotes large f32 flat batch");

    assert_eq!(out[0].to_bits(), 4.0f32.to_bits());
    assert_eq!(out[127].to_bits(), 512.0f32.to_bits());
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn auto_promotes_large_f64_flat_batch_to_native_jit() {
    let helper = mul_add_helper(
        "f64_score_auto_jit",
        RuntimePureInputType::F64,
        RuntimePureOutputType::F64,
        RuntimeValue::f64(2.0),
        RuntimePureHelperOrigin::Annotated,
    );
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Auto,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        helper.plan(),
    );
    let flat_inputs = (1..=128)
        .flat_map(|value: u16| [f64::from(value), 2.0])
        .collect::<Vec<f64>>();
    let mut out = [0.0; 128];

    accelerator
        .call_f64_flat_batch(helper.helper_ref(), &flat_inputs, 2, &mut out)
        .expect("auto promotes large f64 flat batch");

    assert_eq!(out[0].to_bits(), 4.0f64.to_bits());
    assert_eq!(out[127].to_bits(), 512.0f64.to_bits());
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn auto_accelerator_promotes_large_flat_batches_to_jit() {
    let helper = mul_add_helper(
        "score",
        RuntimePureInputType::I64,
        RuntimePureOutputType::I64,
        RuntimeValue::i64(2),
        RuntimePureHelperOrigin::Annotated,
    );
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Auto,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 1024,
            ..RuntimePureAcceleratorConfig::default()
        },
        helper.plan(),
    );
    let mut flat_inputs = Vec::new();
    for value in 1..=128 {
        flat_inputs.extend([value, 2]);
    }
    let mut out = [0; 128];

    accelerator
        .call_i64_flat_batch(helper.helper_ref(), &flat_inputs, 2, &mut out)
        .expect("large auto flat batch succeeds");

    assert_eq!(out[0], 4);
    assert_eq!(out[127], 512);
    assert_eq!(accelerator.stats().jit_calls, 128);
    assert_eq!(accelerator.stats().aot_calls, 0);
    assert_eq!(accelerator.stats().arg_vec_allocations, 0);
    assert_eq!(accelerator.stats().flatten_materializations, 0);
    assert_eq!(accelerator.stats().flatten_bytes_copied, 0);
    assert_eq!(
        accelerator.stats().flat_batch_bytes_borrowed,
        flat_inputs.len() * std::mem::size_of::<i64>()
    );
    assert_eq!(accelerator.summary().jit, 1);
    assert_eq!(accelerator.compile_stats().auto_aot_selected, 1);
    assert_eq!(accelerator.compile_stats().auto_jit_promotions, 1);
}

#[test]
fn aot_accelerates_exact_width_scalar_calls_without_i64_widening() {
    let helpers = admit_add_helpers(&[
        (
            "i32_add",
            RuntimePureInputType::I32,
            RuntimePureOutputType::I32,
        ),
        (
            "u32_add",
            RuntimePureInputType::U32,
            RuntimePureOutputType::U32,
        ),
        (
            "f32_add",
            RuntimePureInputType::F32,
            RuntimePureOutputType::F32,
        ),
        (
            "f64_add",
            RuntimePureInputType::F64,
            RuntimePureOutputType::F64,
        ),
        (
            "isize_add",
            RuntimePureInputType::ISize,
            RuntimePureOutputType::ISize,
        ),
        (
            "usize_add",
            RuntimePureInputType::USize,
            RuntimePureOutputType::USize,
        ),
    ]);
    let mut accelerator =
        RuntimePureAccelerator::new(RuntimePureBackendMode::Aot, helpers[0].plan());

    let i32_value = accelerator
        .call_i32_slice(helpers[0].helper_ref(), &[7, 11])
        .expect("i32 AOT call succeeds");
    let u32_value = accelerator
        .call_exact_int_slice::<u32>(helpers[1].helper_ref(), &[13, 17])
        .expect("u32 AOT call succeeds");
    let f32_value = accelerator
        .call_f32_slice(helpers[2].helper_ref(), &[1.25, 2.5])
        .expect("f32 AOT call succeeds");
    let f64_value = accelerator
        .call_f64_slice(helpers[3].helper_ref(), &[3.0, 4.5])
        .expect("f64 AOT call succeeds");
    let isize_value = accelerator
        .call_exact_int_slice::<RuntimeISizeValue>(
            helpers[4].helper_ref(),
            &[RuntimeISizeValue::new(19), RuntimeISizeValue::new(23)],
        )
        .expect("isize AOT call succeeds");
    let usize_value = accelerator
        .call_exact_int_slice::<RuntimeUSizeValue>(
            helpers[5].helper_ref(),
            &[RuntimeUSizeValue::new(29), RuntimeUSizeValue::new(31)],
        )
        .expect("usize AOT call succeeds");

    assert_eq!(i32_value, Some(18));
    assert_eq!(u32_value, Some(30));
    assert_eq!(f32_value, Some(3.75));
    assert_eq!(f64_value, Some(7.5));
    assert_eq!(isize_value, Some(RuntimeISizeValue::new(42)));
    assert_eq!(usize_value, Some(RuntimeUSizeValue::new(60)));
    assert_eq!(accelerator.stats().aot_calls, 6);
    assert_eq!(accelerator.stats().vm_calls, 0);
    assert_eq!(accelerator.stats().fallbacks, 0);
    assert_eq!(accelerator.summary().aot, 6);
}

#[test]
fn value_fallback_reuses_vm_scratch_without_value_vec_allocation() {
    let helper = admit_helper(
        "echo",
        vec![RuntimePureInputType::Value],
        RuntimePureOutputType::Value,
        false,
        RuntimePureHelperOrigin::Annotated,
        |inputs, output_ty| local_expr(output_ty, inputs[0].clone()),
    );
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Vm,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 2,
            ..RuntimePureAcceleratorConfig::default()
        },
        helper.plan(),
    );

    let value = accelerator
        .call_values(
            helper.helper_ref(),
            &[RuntimeValue::String("ready".to_owned())],
        )
        .expect("VM value fallback succeeds");

    assert_eq!(value, RuntimeValue::String("ready".to_owned()));
    assert_eq!(accelerator.stats().pure_calls, 1);
    assert_eq!(accelerator.stats().vm_calls, 1);
    assert_eq!(accelerator.stats().fallbacks, 1);
    assert_eq!(accelerator.stats().arg_vec_allocations, 0);
    assert_eq!(
        accelerator.stats().arg_bytes_borrowed,
        std::mem::size_of_val(&[RuntimeValue::String("ready".to_owned())])
    );
}

#[test]
fn aot_batch_matches_scalar_results_and_records_parallel_stats() {
    let helper = mul_add_helper(
        "score",
        RuntimePureInputType::I64,
        RuntimePureOutputType::I64,
        RuntimeValue::i64(2),
        RuntimePureHelperOrigin::Inferred,
    );
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Aot,
            workers: RuntimePureWorkerCount::Fixed(2),
            batch_min_len: 1,
            ..RuntimePureAcceleratorConfig::default()
        },
        helper.plan(),
    );
    let rows = [
        RuntimeI64Args::new([3, 4, 0, 0], 2),
        RuntimeI64Args::new([5, 1, 0, 0], 2),
        RuntimeI64Args::new([2, 8, 0, 0], 2),
        RuntimeI64Args::new([7, 0, 0, 0], 2),
    ];
    let mut out = [0; 4];

    accelerator
        .call_i64_batch(helper.helper_ref(), &rows, &mut out)
        .expect("batch succeeds");

    assert_eq!(out, [18, 15, 20, 14]);
    assert_eq!(accelerator.stats().batch_calls, 1);
    assert_eq!(accelerator.stats().batch_items, 4);
    assert_eq!(accelerator.stats().aot_calls, 4);
    assert_eq!(accelerator.stats().arg_vec_allocations, 0);
    assert_eq!(accelerator.resolved_worker_count(), 2);
    assert!(accelerator.has_worker_pool());
    assert_eq!(accelerator.stats().parallel_policy_checks, 1);
    assert_eq!(accelerator.stats().parallel_batches, 1);
    assert_eq!(accelerator.stats().parallel_skipped_small, 0);
    assert_eq!(accelerator.stats().parallel_skipped_backend, 0);
    assert!(accelerator.stats().parallel_work_units > rows.len());
    assert!(accelerator.stats().thread_pool_jobs > 0);
}

#[test]
fn aot_worker_pool_is_created_only_for_parallel_batches() {
    let helper = mul_add_helper(
        "score",
        RuntimePureInputType::I64,
        RuntimePureOutputType::I64,
        RuntimeValue::i64(2),
        RuntimePureHelperOrigin::Annotated,
    );
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Aot,
            workers: RuntimePureWorkerCount::Fixed(2),
            batch_min_len: 2,
            ..RuntimePureAcceleratorConfig::default()
        },
        helper.plan(),
    );
    let small_rows = [
        RuntimeI64Args::new([3, 4, 0, 0], 2),
        RuntimeI64Args::new([5, 1, 0, 0], 2),
    ];
    let mut small_out = [0; 2];

    accelerator
        .call_i64_batch(helper.helper_ref(), &small_rows, &mut small_out)
        .expect("small AOT batch succeeds without pool");

    assert_eq!(small_out, [18, 15]);
    assert!(!accelerator.has_worker_pool());
    assert_eq!(accelerator.stats().parallel_policy_checks, 1);
    assert_eq!(accelerator.stats().parallel_skipped_small, 1);
    assert_eq!(accelerator.stats().thread_pool_jobs, 0);

    let mut small_flat_out = [0; 2];
    accelerator
        .call_i64_flat_batch(helper.helper_ref(), &[3, 4, 5, 1], 2, &mut small_flat_out)
        .expect("small flat AOT batch reuses sequential scratch without pool");

    assert_eq!(small_flat_out, [18, 15]);
    assert!(!accelerator.has_worker_pool());
    assert_eq!(accelerator.stats().parallel_policy_checks, 2);
    assert_eq!(accelerator.stats().parallel_skipped_small, 2);
    assert_eq!(accelerator.stats().thread_pool_jobs, 0);

    let large_rows = [
        RuntimeI64Args::new([3, 4, 0, 0], 2),
        RuntimeI64Args::new([5, 1, 0, 0], 2),
        RuntimeI64Args::new([2, 8, 0, 0], 2),
        RuntimeI64Args::new([7, 0, 0, 0], 2),
        RuntimeI64Args::new([9, 1, 0, 0], 2),
    ];
    let mut large_out = [0; 5];

    accelerator
        .call_i64_batch(helper.helper_ref(), &large_rows, &mut large_out)
        .expect("large AOT batch creates pool");

    assert_eq!(large_out, [18, 15, 20, 14, 27]);
    assert!(accelerator.has_worker_pool());
    assert_eq!(accelerator.stats().parallel_policy_checks, 3);
    assert_eq!(accelerator.stats().parallel_batches, 1);
    assert_eq!(accelerator.stats().parallel_skipped_small, 2);
    assert_eq!(accelerator.stats().thread_pool_jobs, 2);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn jit_batch_matches_scalar_results_without_value_vec_allocation() {
    let helper = mul_add_helper(
        "score",
        RuntimePureInputType::I64,
        RuntimePureOutputType::I64,
        RuntimeValue::i64(2),
        RuntimePureHelperOrigin::Annotated,
    );
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Jit,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 2,
            ..RuntimePureAcceleratorConfig::default()
        },
        helper.plan(),
    );
    let rows = [
        RuntimeI64Args::new([3, 4, 0, 0], 2),
        RuntimeI64Args::new([5, 1, 0, 0], 2),
        RuntimeI64Args::new([2, 8, 0, 0], 2),
    ];
    let mut out = [0; 3];

    RuntimePureCallBackend::call_i64_batch(&mut accelerator, helper.helper_ref(), &rows, &mut out)
        .expect("JIT batch succeeds");

    assert_eq!(out, [18, 15, 20]);
    assert_eq!(accelerator.stats().batch_calls, 1);
    assert_eq!(accelerator.stats().batch_items, 3);
    assert_eq!(accelerator.stats().jit_calls, 3);
    assert_eq!(accelerator.stats().arg_stack_packs, 3);
    assert_eq!(accelerator.stats().arg_vec_allocations, 0);
    assert_eq!(accelerator.stats().flat_batch_calls, 0);
    assert_eq!(accelerator.stats().flat_batch_items, 0);
    assert_eq!(accelerator.stats().flatten_materializations, 1);
    assert_eq!(accelerator.stats().parallel_policy_checks, 1);
    assert_eq!(accelerator.stats().parallel_skipped_backend, 1);
    assert_eq!(accelerator.stats().parallel_batches, 0);
    assert_eq!(
        accelerator.stats().flatten_bytes_copied,
        6 * std::mem::size_of::<i64>()
    );
    assert_eq!(accelerator.summary().jit, 1);
}

#[cfg(all(feature = "native-jit", not(target_arch = "wasm32")))]
#[test]
fn jit_flat_batch_sum_avoids_output_copy() {
    let helper = mul_add_helper(
        "score",
        RuntimePureInputType::I64,
        RuntimePureOutputType::I64,
        RuntimeValue::i64(2),
        RuntimePureHelperOrigin::Annotated,
    );
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Jit,
            workers: RuntimePureWorkerCount::Fixed(1),
            batch_min_len: 2,
            ..RuntimePureAcceleratorConfig::default()
        },
        helper.plan(),
    );

    let sum = accelerator
        .call_i64_flat_batch_sum(helper.helper_ref(), &[3, 4, 5, 1, 2, 8], 2, 3)
        .expect("JIT flat batch sum succeeds");

    assert_eq!(sum, 53);
    assert_eq!(accelerator.stats().batch_calls, 1);
    assert_eq!(accelerator.stats().batch_items, 3);
    assert_eq!(accelerator.stats().flat_batch_calls, 1);
    assert_eq!(accelerator.stats().flat_batch_items, 3);
    assert_eq!(
        accelerator.stats().flat_batch_bytes_borrowed,
        6 * std::mem::size_of::<i64>()
    );
    assert_eq!(accelerator.stats().jit_calls, 3);
    assert_eq!(accelerator.stats().arg_stack_packs, 0);
    assert_eq!(accelerator.stats().arg_vec_allocations, 0);
    assert_eq!(accelerator.stats().flatten_materializations, 0);
    assert_eq!(accelerator.stats().flatten_bytes_copied, 0);
    assert_eq!(accelerator.stats().result_bytes_copied, 0);
    assert_eq!(accelerator.stats().parallel_policy_checks, 1);
    assert_eq!(accelerator.stats().parallel_skipped_backend, 1);
    assert_eq!(accelerator.stats().parallel_batches, 0);
}

#[test]
fn vm_batch_uses_i64_args_without_value_vec_allocation() {
    let helper = mul_add_helper(
        "score",
        RuntimePureInputType::I64,
        RuntimePureOutputType::I64,
        RuntimeValue::i64(2),
        RuntimePureHelperOrigin::Annotated,
    );
    let mut accelerator = RuntimePureAccelerator::with_config(
        RuntimePureAcceleratorConfig {
            backend: RuntimePureBackendMode::Vm,
            workers: RuntimePureWorkerCount::Fixed(2),
            batch_min_len: 1,
            ..RuntimePureAcceleratorConfig::default()
        },
        helper.plan(),
    );
    let rows = [
        RuntimeI64Args::new([3, 4, 0, 0], 2),
        RuntimeI64Args::new([5, 1, 0, 0], 2),
        RuntimeI64Args::new([2, 8, 0, 0], 2),
    ];
    let mut out = [0; 3];

    accelerator
        .call_i64_batch(helper.helper_ref(), &rows, &mut out)
        .expect("VM batch succeeds");

    assert_eq!(out, [18, 15, 20]);
    assert_eq!(accelerator.stats().batch_calls, 1);
    assert_eq!(accelerator.stats().batch_items, 3);
    assert_eq!(accelerator.stats().vm_calls, 3);
    assert_eq!(accelerator.stats().fallbacks, 3);
    assert_eq!(accelerator.stats().arg_stack_packs, 3);
    assert_eq!(accelerator.stats().arg_vec_allocations, 0);
    assert_eq!(accelerator.stats().parallel_policy_checks, 1);
    assert_eq!(accelerator.stats().parallel_batches, 1);
    assert_eq!(accelerator.stats().thread_pool_jobs, 2);
}
