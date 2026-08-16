use std::sync::Arc;

use super::*;
use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::plan::{
    RuntimeCallArgumentSeed, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeLocalDeclarationSeed,
    RuntimeLocalSeedId, RuntimePlan, RuntimePlanBuilder, RuntimePlanTypeProjection,
    RuntimePlanTypeSeed, RuntimePureHelperId, RuntimePureHelperOrigin, RuntimePureHelperSeed,
    RuntimePureInputType, RuntimePureOutputType,
};
use arcweft_core::pure::{
    PureFunctionBackendKind, PureFunctionRequest, VmPureFunctionBackend,
    compare_pure_function_backend,
};
use arcweft_core::runtime_id::RuntimeLocalDeclarationId;
use arcweft_core::value::{
    RuntimeBinaryOp, RuntimeCallArgumentMode, RuntimeCallTarget, RuntimeExprKind, RuntimeIntrinsic,
    RuntimeSignedIntWidth, RuntimeUnaryOp, RuntimeUnsignedIntWidth, RuntimeValue,
};

const BOOL_MARKER: u8 = 1;

#[derive(Clone, Copy)]
enum Scalar {
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
    F32,
    F64,
    String,
}

impl Scalar {
    const fn marker(self) -> u8 {
        match self {
            Self::I8 => 2,
            Self::I16 => 3,
            Self::I32 => 4,
            Self::I64 => 5,
            Self::I128 => 6,
            Self::U8 => 7,
            Self::U16 => 8,
            Self::U32 => 9,
            Self::U64 => 10,
            Self::U128 => 11,
            Self::F32 => 12,
            Self::F64 => 13,
            Self::String => 14,
        }
    }
    const fn ty(self) -> RuntimeSemanticTypeId {
        RuntimeSemanticTypeId::from_bytes([self.marker(); 32])
    }
    const fn projection(self) -> RuntimePlanTypeProjection<RuntimeSemanticTypeId> {
        match self {
            Self::I8 => RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I8),
            Self::I16 => RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I16),
            Self::I32 => RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I32),
            Self::I64 => RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64),
            Self::I128 => RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I128),
            Self::U8 => RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U8),
            Self::U16 => RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U16),
            Self::U32 => RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U32),
            Self::U64 => RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U64),
            Self::U128 => RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U128),
            Self::F32 => RuntimePlanTypeProjection::F32,
            Self::F64 => RuntimePlanTypeProjection::F64,
            Self::String => RuntimePlanTypeProjection::String,
        }
    }
    const fn input_abi(self) -> RuntimePureInputType {
        match self {
            Self::I8 => RuntimePureInputType::I8,
            Self::I16 => RuntimePureInputType::I16,
            Self::I32 => RuntimePureInputType::I32,
            Self::I64 => RuntimePureInputType::I64,
            Self::I128 => RuntimePureInputType::I128,
            Self::U8 => RuntimePureInputType::U8,
            Self::U16 => RuntimePureInputType::U16,
            Self::U32 => RuntimePureInputType::U32,
            Self::U64 => RuntimePureInputType::U64,
            Self::U128 => RuntimePureInputType::U128,
            Self::F32 => RuntimePureInputType::F32,
            Self::F64 => RuntimePureInputType::F64,
            Self::String => RuntimePureInputType::Value,
        }
    }
    const fn output_abi(self) -> RuntimePureOutputType {
        match self {
            Self::I8 => RuntimePureOutputType::I8,
            Self::I16 => RuntimePureOutputType::I16,
            Self::I32 => RuntimePureOutputType::I32,
            Self::I64 => RuntimePureOutputType::I64,
            Self::I128 => RuntimePureOutputType::I128,
            Self::U8 => RuntimePureOutputType::U8,
            Self::U16 => RuntimePureOutputType::U16,
            Self::U32 => RuntimePureOutputType::U32,
            Self::U64 => RuntimePureOutputType::U64,
            Self::U128 => RuntimePureOutputType::U128,
            Self::F32 => RuntimePureOutputType::F32,
            Self::F64 => RuntimePureOutputType::F64,
            Self::String => RuntimePureOutputType::Value,
        }
    }
}

fn bool_ty() -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([BOOL_MARKER; 32])
}
fn expr(scalar: Scalar, kind: RuntimeExprSeedKind) -> RuntimeExprSeed {
    RuntimeExprSeed::new(scalar.ty(), kind)
}
fn value(scalar: Scalar, value: RuntimeValue) -> RuntimeExprSeed {
    expr(scalar, RuntimeExprSeedKind::Value(value))
}
fn local(scalar: Scalar, local: RuntimeLocalSeedId) -> RuntimeExprSeed {
    expr(scalar, RuntimeExprSeedKind::Local(local))
}
fn binary(
    scalar: Scalar,
    lhs: RuntimeExprSeed,
    op: RuntimeBinaryOp,
    rhs: RuntimeExprSeed,
) -> RuntimeExprSeed {
    expr(
        scalar,
        RuntimeExprSeedKind::Binary {
            lhs: Box::new(lhs),
            op,
            rhs: Box::new(rhs),
        },
    )
}
fn compare(lhs: RuntimeExprSeed, op: RuntimeBinaryOp, rhs: RuntimeExprSeed) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        bool_ty(),
        RuntimeExprSeedKind::Binary {
            lhs: Box::new(lhs),
            op,
            rhs: Box::new(rhs),
        },
    )
}
fn if_expr(
    scalar: Scalar,
    condition: RuntimeExprSeed,
    then_expr: RuntimeExprSeed,
    else_expr: RuntimeExprSeed,
) -> RuntimeExprSeed {
    expr(
        scalar,
        RuntimeExprSeedKind::If {
            condition: Box::new(condition),
            then_expr: Box::new(then_expr),
            else_expr: Box::new(else_expr),
        },
    )
}
fn call(
    scalar: Scalar,
    intrinsic: RuntimeIntrinsic,
    args: impl IntoIterator<Item = RuntimeExprSeed>,
) -> RuntimeExprSeed {
    expr(
        scalar,
        RuntimeExprSeedKind::Call {
            callee: RuntimeCallTarget::intrinsic(intrinsic),
            args: args
                .into_iter()
                .map(|value| RuntimeCallArgumentSeed::new(value, RuntimeCallArgumentMode::Value))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        },
    )
}

struct AdmittedHelper {
    plan: Arc<RuntimePlan>,
    helper: RuntimePureHelperId,
}
impl AdmittedHelper {
    fn request(&self, args: impl IntoIterator<Item = RuntimeValue>) -> PureFunctionRequest {
        PureFunctionRequest::try_new(Arc::clone(&self.plan), self.helper, args)
            .expect("well-typed helper request")
    }
    fn input_locals(&self) -> &[RuntimeLocalDeclarationId] {
        self.plan.pure_helpers()[self.helper.0]
            .input_locals
            .as_ref()
    }
}

fn admit(
    scalar: Scalar,
    name: &str,
    inputs: usize,
    locals: usize,
    body: impl FnOnce(&[RuntimeLocalSeedId]) -> RuntimeExprSeed,
) -> AdmittedHelper {
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(bool_ty(), RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(scalar.ty(), scalar.projection()),
            ],
            (0..locals).map(|_| RuntimeLocalDeclarationSeed::new(scalar.ty())),
            [],
            [],
        )
        .expect("semantic admission");
    builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            name: name.to_owned(),
            inputs: admission.local_ids()[..inputs].to_vec().into_boxed_slice(),
            input_abi: vec![scalar.input_abi(); inputs],
            output_abi: scalar.output_abi(),
            body: body(admission.local_ids()),
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .expect("typed helper admission");
    let plan = Arc::new(builder.finish().expect("sealed helper plan"));
    AdmittedHelper {
        helper: plan.pure_helpers()[0].id,
        plan,
    }
}

fn two_input_product(scalar: Scalar, name: &str) -> AdmittedHelper {
    admit(scalar, name, 2, 2, |ids| {
        binary(
            scalar,
            local(scalar, ids[0].clone()),
            RuntimeBinaryOp::Mul,
            binary(
                scalar,
                local(scalar, ids[1].clone()),
                RuntimeBinaryOp::Add,
                value(
                    scalar,
                    match scalar {
                        Scalar::I8 => RuntimeValue::i8(2),
                        Scalar::I16 => RuntimeValue::i16(2),
                        Scalar::I32 => RuntimeValue::i32(2),
                        Scalar::I64 => RuntimeValue::i64(2),
                        Scalar::I128 => RuntimeValue::i128(2),
                        Scalar::U8 => RuntimeValue::u8(2),
                        Scalar::U16 => RuntimeValue::u16(2),
                        Scalar::U32 => RuntimeValue::u32(2),
                        Scalar::U64 => RuntimeValue::u64(2),
                        Scalar::U128 => RuntimeValue::u128(2),
                        Scalar::F32 => RuntimeValue::F32(0.5),
                        Scalar::F64 => RuntimeValue::F64(0.5),
                        Scalar::String => unreachable!("not arithmetic"),
                    },
                ),
            ),
        )
    })
}

fn score_i64(name: &str) -> AdmittedHelper {
    admit(Scalar::I64, name, 2, 2, |ids| {
        if_expr(
            Scalar::I64,
            compare(
                local(Scalar::I64, ids[0].clone()),
                RuntimeBinaryOp::Ge,
                value(Scalar::I64, RuntimeValue::i64(3)),
            ),
            binary(
                Scalar::I64,
                local(Scalar::I64, ids[0].clone()),
                RuntimeBinaryOp::Mul,
                call(
                    Scalar::I64,
                    RuntimeIntrinsic::Add,
                    [
                        local(Scalar::I64, ids[1].clone()),
                        value(Scalar::I64, RuntimeValue::i64(2)),
                    ],
                ),
            ),
            value(Scalar::I64, RuntimeValue::i64(0)),
        )
    })
}

fn has_symbol(symbols: &[&str], expected: &str) -> bool {
    symbols
        .iter()
        .any(|symbol| *symbol == expected || symbol.strip_prefix('_') == Some(expected))
}
fn assert_object_symbols(object: &ObjectPureInputs) {
    use cranelift_object::object::{Object, ObjectSymbol};
    let parsed = cranelift_object::object::File::parse(object.object_bytes.as_slice())
        .expect("object parses");
    let symbols = parsed
        .symbols()
        .filter_map(|symbol| symbol.name().ok())
        .collect::<Vec<_>>();
    assert!(has_symbol(&symbols, &object.entry_symbol));
    assert!(has_symbol(&symbols, &object.batch_symbol));
}
fn assert_batch_symbols(object: &ObjectPureBatchInputs) {
    use cranelift_object::object::{Object, ObjectSymbol};
    let parsed = cranelift_object::object::File::parse(object.object_bytes.as_slice())
        .expect("batch object parses");
    let symbols = parsed
        .symbols()
        .filter_map(|symbol| symbol.name().ok())
        .collect::<Vec<_>>();
    assert!(has_symbol(&symbols, &object.batch_symbol));
    assert!(has_symbol(&symbols, &object.batch_sum_symbol));
}

#[test]
fn cranelift_plan_qualified_i64_helper_matches_vm() {
    let helper = score_i64("score");
    let request = helper.request([RuntimeValue::i64(3), RuntimeValue::i64(4)]);
    assert!(Arc::ptr_eq(request.plan(), &helper.plan));
    assert!(matches!(
        request
            .helper_ref()
            .expect("helper reference")
            .declaration()
            .expr
            .kind(),
        RuntimeExprKind::If { .. }
    ));
    let result = compare_pure_function_backend(
        &VmPureFunctionBackend,
        &CraneliftPureFunctionBackend,
        &request,
    )
    .expect("backends agree");
    assert!(result.matches_vm);
    assert_eq!(result.candidate.backend, PureFunctionBackendKind::Jit);
    assert_eq!(result.candidate.value, RuntimeValue::i64(18));
}

#[test]
fn cranelift_i64_entry_batch_and_benchmark_use_local_ids() {
    let helper = score_i64("score inputs");
    let request = helper.request([RuntimeValue::i64(3), RuntimeValue::i64(4)]);
    let ids = helper.input_locals().to_vec();
    let compiled = CraneliftPureFunctionBackend
        .compile_i64_with_inputs(&request, ids.clone())
        .expect("compiles");
    assert_eq!(compiled.input_locals(), ids);
    assert_eq!(compiled.call(&[3, 4]).expect("call"), 18);
    let mut out = [0; 3];
    compiled
        .call_flat_batch(&[3, 4, 2, 99, 7, 1], &mut out)
        .expect("batch");
    assert_eq!(out, [18, 0, 21]);
    let mut module = jit_module().expect("JIT module");
    let defined = define_i64_with_inputs(
        &mut module,
        "arcweft_test_defined_i64",
        &request,
        helper.input_locals().iter().copied(),
    )
    .expect("defined");
    module.finalize_definitions().expect("finalizes");
    let caller =
        native_call::I64InputCaller::from_code(module.get_finalized_function(defined.entry), 2)
            .expect("ABI");
    assert_eq!(caller.call(&[3, 4]), Some(18));
    let mut module = object_module().expect("object module");
    let benchmark = define_i64_benchmark_batch(
        &mut module,
        "arcweft_test_i64_benchmark_batch",
        &request,
        helper.input_locals().iter().copied(),
    )
    .expect("benchmark");
    assert_eq!(benchmark.input_locals.len(), 2);
    assert!(!emit_object_bytes(module).expect("object").is_empty());

    let constant = admit(Scalar::I64, "constant", 0, 0, |_| {
        binary(
            Scalar::I64,
            value(Scalar::I64, RuntimeValue::i64(21)),
            RuntimeBinaryOp::Add,
            value(Scalar::I64, RuntimeValue::i64(21)),
        )
    });
    assert_eq!(
        CraneliftPureFunctionBackend
            .compile_i64(&constant.request([]))
            .expect("zero-input helper compiles")
            .call(),
        42
    );
    assert_eq!(
        CraneliftPureFunctionBackend
            .compile_i64_batch(&request, helper.input_locals().iter().copied())
            .expect("benchmark helper compiles")
            .call(7, 0, 8)
            .expect("batch call"),
        136
    );
}

#[test]
fn cranelift_emits_objects_from_typed_helpers() {
    let backend = CraneliftPureFunctionBackend;
    let i64 = two_input_product(Scalar::I64, "object i64");
    let i64_request = i64.request([RuntimeValue::i64(0), RuntimeValue::i64(0)]);
    assert_object_symbols(
        &backend
            .emit_object_i64_with_inputs(&i64_request, i64.input_locals().iter().copied())
            .expect("i64 object"),
    );
    let i32 = two_input_product(Scalar::I32, "object i32");
    let i32_request = i32.request([RuntimeValue::i32(0), RuntimeValue::i32(0)]);
    assert_object_symbols(
        &backend
            .emit_object_i32_with_inputs(&i32_request, i32.input_locals().iter().copied())
            .expect("i32 object"),
    );
    let u32 = two_input_product(Scalar::U32, "object u32");
    let u32_request = u32.request([RuntimeValue::u32(0), RuntimeValue::u32(0)]);
    assert_object_symbols(
        &backend
            .emit_object_u32_with_inputs(&u32_request, u32.input_locals().iter().copied())
            .expect("u32 object"),
    );
    let f32 = two_input_product(Scalar::F32, "object f32");
    let f32_request = f32.request([RuntimeValue::F32(0.0), RuntimeValue::F32(0.0)]);
    assert_object_symbols(
        &backend
            .emit_object_f32_with_inputs(&f32_request, f32.input_locals().iter().copied())
            .expect("f32 object"),
    );
    let f64 = two_input_product(Scalar::F64, "object f64");
    let f64_request = f64.request([RuntimeValue::F64(0.0), RuntimeValue::F64(0.0)]);
    assert_object_symbols(
        &backend
            .emit_object_f64_with_inputs(&f64_request, f64.input_locals().iter().copied())
            .expect("f64 object"),
    );
    let i128 = admit(Scalar::I128, "object i128", 2, 2, |ids| {
        binary(
            Scalar::I128,
            local(Scalar::I128, ids[0].clone()),
            RuntimeBinaryOp::Add,
            local(Scalar::I128, ids[1].clone()),
        )
    });
    let i128_request = i128.request([RuntimeValue::i128(0), RuntimeValue::i128(0)]);
    assert_batch_symbols(
        &backend
            .emit_object_i128_batch_with_inputs(&i128_request, i128.input_locals().iter().copied())
            .expect("i128 object"),
    );
    let u128 = admit(Scalar::U128, "object u128", 2, 2, |ids| {
        binary(
            Scalar::U128,
            local(Scalar::U128, ids[0].clone()),
            RuntimeBinaryOp::Add,
            local(Scalar::U128, ids[1].clone()),
        )
    });
    let u128_request = u128.request([RuntimeValue::u128(0), RuntimeValue::u128(0)]);
    assert_batch_symbols(
        &backend
            .emit_object_u128_batch_with_inputs(&u128_request, u128.input_locals().iter().copied())
            .expect("u128 object"),
    );
}

#[test]
fn cranelift_emits_bundle_from_plan_qualified_requests() {
    let i32 = two_input_product(Scalar::I32, "bundle i32");
    let i32_request = i32.request([RuntimeValue::i32(0), RuntimeValue::i32(0)]);
    let f32 = two_input_product(Scalar::F32, "bundle f32");
    let f32_request = f32.request([RuntimeValue::F32(0.0), RuntimeValue::F32(0.0)]);
    let u128 = admit(Scalar::U128, "bundle u128", 2, 2, |ids| {
        binary(
            Scalar::U128,
            local(Scalar::U128, ids[0].clone()),
            RuntimeBinaryOp::Add,
            local(Scalar::U128, ids[1].clone()),
        )
    });
    let u128_request = u128.request([RuntimeValue::u128(0), RuntimeValue::u128(0)]);
    let bundle = CraneliftPureFunctionBackend
        .emit_object_bundle([
            PureObjectBundleRequest::new(
                &i32_request,
                PureObjectInputKind::I32,
                i32.input_locals().iter().copied(),
            ),
            PureObjectBundleRequest::new(
                &f32_request,
                PureObjectInputKind::F32,
                f32.input_locals().iter().copied(),
            ),
            PureObjectBundleRequest::new(
                &u128_request,
                PureObjectInputKind::U128,
                u128.input_locals().iter().copied(),
            ),
        ])
        .expect("object bundle emits");
    assert_eq!(bundle.helpers.len(), 3);
    assert!(bundle.helpers[0].entrypoints.entry_symbol().is_some());
    assert!(bundle.helpers[1].entrypoints.batch_sum_symbol().is_none());
    assert!(bundle.helpers[2].entrypoints.entry_symbol().is_none());
}

#[test]
fn cranelift_compiles_each_scalar_abi_from_local_ids() {
    let backend = CraneliftPureFunctionBackend;
    let i8 = two_input_product(Scalar::I8, "i8");
    let i8_request = i8.request([RuntimeValue::i8(0), RuntimeValue::i8(0)]);
    assert_eq!(
        backend
            .compile_i8_with_inputs(&i8_request, i8.input_locals().iter().copied())
            .expect("i8")
            .call(&[3, 4])
            .expect("call"),
        18
    );
    let i16 = two_input_product(Scalar::I16, "i16");
    let i16_request = i16.request([RuntimeValue::i16(0), RuntimeValue::i16(0)]);
    assert_eq!(
        backend
            .compile_i16_with_inputs(&i16_request, i16.input_locals().iter().copied())
            .expect("i16")
            .call(&[30, 4])
            .expect("call"),
        180
    );
    let i32 = two_input_product(Scalar::I32, "i32");
    let i32_request = i32.request([RuntimeValue::i32(0), RuntimeValue::i32(0)]);
    assert_eq!(
        backend
            .compile_i32_with_inputs(&i32_request, i32.input_locals().iter().copied())
            .expect("i32")
            .call(&[3, 4])
            .expect("call"),
        18
    );
    let u8 = two_input_product(Scalar::U8, "u8");
    let u8_request = u8.request([RuntimeValue::u8(0), RuntimeValue::u8(0)]);
    assert_eq!(
        backend
            .compile_u8_with_inputs(&u8_request, u8.input_locals().iter().copied())
            .expect("u8")
            .call(&[3, 4])
            .expect("call"),
        18
    );
    let u16 = two_input_product(Scalar::U16, "u16");
    let u16_request = u16.request([RuntimeValue::u16(0), RuntimeValue::u16(0)]);
    assert_eq!(
        backend
            .compile_u16_with_inputs(&u16_request, u16.input_locals().iter().copied())
            .expect("u16")
            .call(&[30, 4])
            .expect("call"),
        180
    );
    let u32 = two_input_product(Scalar::U32, "u32");
    let u32_request = u32.request([RuntimeValue::u32(0), RuntimeValue::u32(0)]);
    assert_eq!(
        backend
            .compile_u32_with_inputs(&u32_request, u32.input_locals().iter().copied())
            .expect("u32")
            .call(&[3, 4])
            .expect("call"),
        18
    );
    let u64 = two_input_product(Scalar::U64, "u64");
    let u64_request = u64.request([RuntimeValue::u64(0), RuntimeValue::u64(0)]);
    assert_eq!(
        backend
            .compile_u64_with_inputs(&u64_request, u64.input_locals().iter().copied())
            .expect("u64")
            .call(&[3, 4])
            .expect("call"),
        18
    );
}

#[test]
fn cranelift_wide_batches_preserve_full_width_values() {
    let i128 = admit(Scalar::I128, "wide i128", 2, 2, |ids| {
        binary(
            Scalar::I128,
            local(Scalar::I128, ids[0].clone()),
            RuntimeBinaryOp::Add,
            local(Scalar::I128, ids[1].clone()),
        )
    });
    let i128_request = i128.request([RuntimeValue::i128(0), RuntimeValue::i128(0)]);
    let i128_compiled = CraneliftPureFunctionBackend
        .compile_i128_batch_with_inputs(&i128_request, i128.input_locals().iter().copied())
        .expect("i128");
    let mut i128_out = [0; 2];
    i128_compiled
        .call_flat_batch(&[i128::MAX - 5, 3, i128::MIN + 9, -4], &mut i128_out)
        .expect("batch");
    assert_eq!(i128_out, [i128::MAX - 2, i128::MIN + 5]);
    let u128 = admit(Scalar::U128, "wide u128", 2, 2, |ids| {
        binary(
            Scalar::U128,
            local(Scalar::U128, ids[0].clone()),
            RuntimeBinaryOp::Add,
            local(Scalar::U128, ids[1].clone()),
        )
    });
    let u128_request = u128.request([RuntimeValue::u128(0), RuntimeValue::u128(0)]);
    let u128_compiled = CraneliftPureFunctionBackend
        .compile_u128_batch_with_inputs(&u128_request, u128.input_locals().iter().copied())
        .expect("u128");
    let mut u128_out = [0; 2];
    u128_compiled
        .call_flat_batch(&[u128::MAX - 7, 2, 1_u128 << 100, 5], &mut u128_out)
        .expect("batch");
    assert_eq!(u128_out, [u128::MAX - 5, (1_u128 << 100) + 5]);
}

#[test]
fn cranelift_floats_intrinsics_and_lexical_let_use_seeded_expressions() {
    let f32 = two_input_product(Scalar::F32, "f32");
    let f32_request = f32.request([RuntimeValue::F32(0.0), RuntimeValue::F32(0.0)]);
    assert_eq!(
        CraneliftPureFunctionBackend
            .compile_f32_with_inputs(&f32_request, f32.input_locals().iter().copied())
            .expect("f32")
            .call(&[3.0, 1.5])
            .expect("call"),
        6.0
    );
    let f64 = two_input_product(Scalar::F64, "f64");
    let f64_request = f64.request([RuntimeValue::F64(0.0), RuntimeValue::F64(0.0)]);
    assert_eq!(
        CraneliftPureFunctionBackend
            .compile_f64_with_inputs(&f64_request, f64.input_locals().iter().copied())
            .expect("f64")
            .call(&[3.0, 1.5])
            .expect("call"),
        6.0
    );
    let intrinsic = admit(Scalar::F32, "intrinsic", 3, 3, |ids| {
        call(
            Scalar::F32,
            RuntimeIntrinsic::StdF32MulAdd,
            [
                call(
                    Scalar::F32,
                    RuntimeIntrinsic::StdF32Sqrt,
                    [local(Scalar::F32, ids[0].clone())],
                ),
                call(
                    Scalar::F32,
                    RuntimeIntrinsic::StdF32Abs,
                    [local(Scalar::F32, ids[1].clone())],
                ),
                call(
                    Scalar::F32,
                    RuntimeIntrinsic::StdF32Fract,
                    [local(Scalar::F32, ids[2].clone())],
                ),
            ],
        )
    });
    let intrinsic_request = intrinsic.request([
        RuntimeValue::F32(0.0),
        RuntimeValue::F32(0.0),
        RuntimeValue::F32(0.0),
    ]);
    assert_eq!(
        CraneliftPureFunctionBackend
            .compile_f32_with_inputs(&intrinsic_request, intrinsic.input_locals().iter().copied())
            .expect("intrinsic")
            .call(&[9.0, -2.0, 1.25])
            .expect("call")
            .to_bits(),
        6.25_f32.to_bits()
    );
    let f64_intrinsic = admit(Scalar::F64, "f64 intrinsic", 3, 3, |ids| {
        call(
            Scalar::F64,
            RuntimeIntrinsic::StdF64MulAdd,
            [
                call(
                    Scalar::F64,
                    RuntimeIntrinsic::StdF64Sqrt,
                    [local(Scalar::F64, ids[0].clone())],
                ),
                call(
                    Scalar::F64,
                    RuntimeIntrinsic::StdF64Ceil,
                    [local(Scalar::F64, ids[1].clone())],
                ),
                call(
                    Scalar::F64,
                    RuntimeIntrinsic::StdF64Fract,
                    [local(Scalar::F64, ids[2].clone())],
                ),
            ],
        )
    });
    let f64_intrinsic_request = f64_intrinsic.request([
        RuntimeValue::F64(0.0),
        RuntimeValue::F64(0.0),
        RuntimeValue::F64(0.0),
    ]);
    assert_eq!(
        CraneliftPureFunctionBackend
            .compile_f64_with_inputs(
                &f64_intrinsic_request,
                f64_intrinsic.input_locals().iter().copied(),
            )
            .expect("f64 intrinsic")
            .call(&[25.0, 1.2, 3.5])
            .expect("call")
            .to_bits(),
        10.5_f64.to_bits()
    );
    let lexical = admit(Scalar::I64, "lexical", 2, 3, |ids| {
        expr(
            Scalar::I64,
            RuntimeExprSeedKind::Let {
                binding: ids[2].clone(),
                expr: Box::new(call(
                    Scalar::I64,
                    RuntimeIntrinsic::Add,
                    [
                        local(Scalar::I64, ids[1].clone()),
                        value(Scalar::I64, RuntimeValue::i64(2)),
                    ],
                )),
                body: Box::new(binary(
                    Scalar::I64,
                    local(Scalar::I64, ids[0].clone()),
                    RuntimeBinaryOp::Mul,
                    local(Scalar::I64, ids[2].clone()),
                )),
            },
        )
    });
    let lexical_request = lexical.request([RuntimeValue::i64(0), RuntimeValue::i64(0)]);
    assert_eq!(
        CraneliftPureFunctionBackend
            .compile_i64_with_inputs(&lexical_request, lexical.input_locals().iter().copied())
            .expect("let")
            .call(&[3, 4])
            .expect("call"),
        18
    );
}

#[test]
fn cranelift_unary_and_unsupported_typed_values_have_deterministic_boundaries() {
    let unary = admit(Scalar::I64, "normalized", 3, 3, |ids| {
        binary(
            Scalar::I64,
            expr(
                Scalar::I64,
                RuntimeExprSeedKind::Unary {
                    op: RuntimeUnaryOp::Neg,
                    expr: Box::new(binary(
                        Scalar::I64,
                        local(Scalar::I64, ids[0].clone()),
                        RuntimeBinaryOp::Sub,
                        local(Scalar::I64, ids[1].clone()),
                    )),
                },
            ),
            RuntimeBinaryOp::Div,
            local(Scalar::I64, ids[2].clone()),
        )
    });
    let unary_request = unary.request([
        RuntimeValue::i64(0),
        RuntimeValue::i64(0),
        RuntimeValue::i64(1),
    ]);
    assert_eq!(
        CraneliftPureFunctionBackend
            .compile_i64_with_inputs(&unary_request, unary.input_locals().iter().copied())
            .expect("unary")
            .call(&[21, 9, 3])
            .expect("call"),
        -4
    );
    let string = admit(Scalar::String, "string", 0, 0, |_| {
        value(Scalar::String, RuntimeValue::String("x".to_owned()))
    });
    let string_request = string.request([]);
    assert!(matches!(
        CraneliftPureFunctionBackend.evaluate_jit(&string_request),
        Err(CraneliftCodegenError::UnsupportedExpr(_))
    ));
}
