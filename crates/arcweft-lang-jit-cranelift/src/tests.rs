use std::sync::Arc;

use super::*;
use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::plan::{
    RuntimeCallArgumentSeed, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeLocalDeclarationSeed,
    RuntimeLocalReadSeed, RuntimeLocalSeedId, RuntimePlan, RuntimePlanBuilder,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed, RuntimePureHelperId, RuntimePureHelperOrigin,
    RuntimePureHelperSeed, RuntimePureInputType, RuntimePureOutputType,
};
use arcweft_core::pure::{
    PureFunctionBackendKind, PureFunctionRequest, VmPureFunctionBackend,
    compare_pure_function_backend,
};
use arcweft_core::runtime_id::RuntimeLocalDeclarationId;
use arcweft_core::value::{
    RuntimeBinaryOp, RuntimeCallArgumentMode, RuntimeCallTarget, RuntimeExprKind, RuntimeIntrinsic,
    RuntimeLocalReadMode, RuntimeSignedIntWidth, RuntimeUnaryOp, RuntimeUnsignedIntWidth,
    RuntimeValue,
};

const BOOL_MARKER: u8 = 1;

#[derive(Clone, Copy)]
enum Scalar {
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
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
            Self::ISize => 15,
            Self::U8 => 7,
            Self::U16 => 8,
            Self::U32 => 9,
            Self::U64 => 10,
            Self::U128 => 11,
            Self::USize => 16,
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
            Self::ISize => RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::ISize),
            Self::U8 => RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U8),
            Self::U16 => RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U16),
            Self::U32 => RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U32),
            Self::U64 => RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U64),
            Self::U128 => RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U128),
            Self::USize => RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::USize),
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
            Self::ISize => RuntimePureInputType::ISize,
            Self::U8 => RuntimePureInputType::U8,
            Self::U16 => RuntimePureInputType::U16,
            Self::U32 => RuntimePureInputType::U32,
            Self::U64 => RuntimePureInputType::U64,
            Self::U128 => RuntimePureInputType::U128,
            Self::USize => RuntimePureInputType::USize,
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
            Self::ISize => RuntimePureOutputType::ISize,
            Self::U8 => RuntimePureOutputType::U8,
            Self::U16 => RuntimePureOutputType::U16,
            Self::U32 => RuntimePureOutputType::U32,
            Self::U64 => RuntimePureOutputType::U64,
            Self::U128 => RuntimePureOutputType::U128,
            Self::USize => RuntimePureOutputType::USize,
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
    expr(
        scalar,
        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(local, RuntimeLocalReadMode::Copy)),
    )
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
                .enumerate()
                .map(|(index, value)| {
                    RuntimeCallArgumentSeed::new(
                        value,
                        RuntimeCallArgumentMode::Value,
                        u32::try_from(index).expect("test call ABI position"),
                    )
                })
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
    fn input_locals(&self) -> Vec<RuntimeLocalDeclarationId> {
        self.plan.pure_helpers()[self.helper.0]
            .inputs
            .iter()
            .map(|input| input.local())
            .collect()
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
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(bool_ty(), RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(scalar.ty(), scalar.projection()),
            ],
            (0..locals).map(|source_ordinal| {
                RuntimeLocalDeclarationSeed::new(
                    manual_local_source(&format!(
                        "arcweft-lang-jit-cranelift.fixture.tests.helper.input[{source_ordinal}]"
                    )),
                    scalar.ty(),
                )
            }),
        )
        .expect("semantic admission");
    builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            definition:
                arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [51; 32],
                ),
            name: name.to_owned(),
            inputs: admission.local_ids()[..inputs]
                .to_vec()
                .into_boxed_slice()
                .into_iter()
                .zip(vec![scalar.input_abi(); inputs])
                .map(
                    |(local, abi)| arcweft_core::plan::RuntimeCallableParameterSeed {
                        identity: arcweft_core::plan::RuntimeFunctionParameterIdentity::from_accepted_identity([91; 32]),
                        local,
                        passing: arcweft_core::plan::RuntimeFunctionParameterPassing::Value,
                        abi,
                    },
                )
                .collect(),
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
                        Scalar::ISize => RuntimeValue::isize(2),
                        Scalar::U8 => RuntimeValue::u8(2),
                        Scalar::U16 => RuntimeValue::u16(2),
                        Scalar::U32 => RuntimeValue::u32(2),
                        Scalar::U64 => RuntimeValue::u64(2),
                        Scalar::U128 => RuntimeValue::u128(2),
                        Scalar::USize => RuntimeValue::usize(2),
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
            .function_ref()
            .expect("helper reference")
            .expression()
            .expect("expression recipe")
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
    assert_eq!(caller.call(&[3, 4]).expect("checked ABI call"), Some(18));
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
            .call()
            .expect("checked zero-input call"),
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
    let compiled = CraneliftPureFunctionBackend
        .compile_i64_with_inputs(&unary_request, unary.input_locals().iter().copied())
        .expect("the signed division shares the checked native outcome");
    assert_eq!(compiled.call(&[21, 9, 3]).unwrap(), -4);
    assert!(matches!(
        compiled.call(&[21, 9, 0]),
        Err(CraneliftCodegenError::Execution {
            error: RuntimeEvalError::RecoverableExpression(
                arcweft_core::value::RuntimeExpressionFailure::DivisionByZero
            ),
            completed_rows: 0,
        })
    ));
    let values = [
        RuntimeValue::i64(21),
        RuntimeValue::i64(9),
        RuntimeValue::i64(3),
    ];
    assert_eq!(
        aot_scalar_body_result(&unary_request, &values).unwrap(),
        RuntimeValue::i64(-4)
    );
    assert_eq!(
        arcweft_core::pure::VmPureFunctionScratch::default()
            .evaluate_values(
                unary_request.plan(),
                unary_request.function_id(),
                Vec::from(values)
            )
            .unwrap(),
        RuntimeValue::i64(-4)
    );
    let pure_unary = ordinary_scope_request(Scalar::I64, |ids| {
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
        )
    });
    let locals = pure_unary
        .function_ref()
        .unwrap()
        .inputs
        .iter()
        .map(arcweft_core::pure::RuntimePureFunctionInputRef::local)
        .collect::<Vec<_>>();
    assert_eq!(
        CraneliftPureFunctionBackend
            .compile_i64_with_inputs(&pure_unary, locals)
            .unwrap()
            .call(&[21, 9, 3])
            .unwrap(),
        -12
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

fn manual_local_source(declaration: &str) -> arcweft_core::plan::RuntimeLocalDeclarationSource {
    // This fixture declares a semantic binding name independent of its value,
    // type, source offset, and builder-issued local ordinal.
    let mut identity = blake3::Hasher::new();
    identity.update(b"arcweft.manual-fixture-binding.v1\0");
    identity.update(declaration.as_bytes());
    arcweft_core::plan::RuntimeLocalDeclarationSource::Binding {
        identity: *identity.finalize().as_bytes(),
        declaration: arcweft_core::plan::RuntimeLocalBindingDeclaration::new(
            arcweft_core::plan::RuntimeLocalBindingKind::PatternBinding,
            false,
            arcweft_core::plan::RuntimeLocalBindingStorage::Derived,
        ),
    }
}

fn scoped(ty: RuntimeSemanticTypeId, name: &str, body: RuntimeExprSeed) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        ty,
        RuntimeExprSeedKind::Scope {
            identity: arcweft_core::scope::RuntimeScopeIdentity::Named(
                name.parse().expect("declared lexical scope name"),
            ),
            body: Box::new(body),
        },
    )
}

fn scalar_number(scalar: Scalar, number: u8) -> RuntimeValue {
    match scalar {
        Scalar::I8 => RuntimeValue::i8(i8::try_from(number).unwrap()),
        Scalar::I16 => RuntimeValue::i16(i16::from(number)),
        Scalar::I32 => RuntimeValue::i32(i32::from(number)),
        Scalar::I64 => RuntimeValue::i64(i64::from(number)),
        Scalar::I128 => RuntimeValue::i128(i128::from(number)),
        Scalar::ISize => RuntimeValue::isize(i64::from(number)),
        Scalar::U8 => RuntimeValue::u8(number),
        Scalar::U16 => RuntimeValue::u16(u16::from(number)),
        Scalar::U32 => RuntimeValue::u32(u32::from(number)),
        Scalar::U64 => RuntimeValue::u64(u64::from(number)),
        Scalar::U128 => RuntimeValue::u128(u128::from(number)),
        Scalar::USize => RuntimeValue::usize(u64::from(number)),
        Scalar::F32 => RuntimeValue::F32(f32::from(number)),
        Scalar::F64 => RuntimeValue::F64(f64::from(number)),
        Scalar::String => unreachable!("numeric scope fixture"),
    }
}

fn ordinary_scope_request(
    scalar: Scalar,
    body: impl FnOnce(&[RuntimeLocalSeedId]) -> RuntimeExprSeed,
) -> PureFunctionRequest {
    ordinary_body_request(
        scalar,
        arcweft_core::plan::RuntimeFunctionSiteBodyKind::Expression,
        |ids| arcweft_core::plan::RuntimeFunctionSiteBodySeed::Expression(body(ids)),
    )
}

fn ordinary_executable_request(
    scalar: Scalar,
    body: impl FnOnce(&[RuntimeLocalSeedId]) -> Vec<arcweft_core::plan::RuntimeFlowOpSeed>,
) -> PureFunctionRequest {
    ordinary_body_request(
        scalar,
        arcweft_core::plan::RuntimeFunctionSiteBodyKind::Executable,
        |ids| {
            arcweft_core::plan::RuntimeFunctionSiteBodySeed::Executable(
                arcweft_core::plan::RuntimeExecutableBodySeed {
                    effects: arcweft_core::plan::RuntimeEffectSet::empty(),
                    ops: body(ids).into(),
                },
            )
        },
    )
}

fn ordinary_body_request(
    scalar: Scalar,
    body_kind: arcweft_core::plan::RuntimeFunctionSiteBodyKind,
    body: impl FnOnce(&[RuntimeLocalSeedId]) -> arcweft_core::plan::RuntimeFunctionSiteBodySeed,
) -> PureFunctionRequest {
    use arcweft_core::plan::{
        RuntimeEffectSet, RuntimeFunctionDefinitionIdentity, RuntimeFunctionInputBindingSeed,
        RuntimeFunctionInputOrigin, RuntimeFunctionInputSource, RuntimeFunctionInputTransfer,
        RuntimeFunctionParameterIdentity, RuntimeFunctionParameterPassing,
        RuntimeFunctionSemanticRole, RuntimeFunctionSiteDeclarationSeed,
        RuntimeLocalDeclarationSource, RuntimePatternSeed, RuntimePatternSeedKind,
    };
    let parameters = [0x81, 0x82, 0x83]
        .map(|marker| RuntimeFunctionParameterIdentity::from_accepted_identity([marker; 32]));
    let mut builder = RuntimePlanBuilder::new();
    let sources = parameters
        .map(RuntimeLocalDeclarationSource::Parameter)
        .into_iter()
        .chain([manual_local_source("ordinary_scope.temp")]);
    let admission = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(bool_ty(), RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(scalar.ty(), scalar.projection()),
            ],
            sources.map(|source| RuntimeLocalDeclarationSeed::new(source, scalar.ty())),
        )
        .unwrap();
    let inputs = parameters
        .into_iter()
        .zip(admission.local_ids().iter().take(3).cloned())
        .enumerate()
        .map(
            |(position, (parameter, local))| RuntimeFunctionInputBindingSeed {
                transfer: RuntimeFunctionInputTransfer::Formal,
                origin: RuntimeFunctionInputOrigin::Parameter(parameter),
                source: RuntimeFunctionInputSource::Parameter {
                    position: u32::try_from(position).unwrap(),
                    passing: RuntimeFunctionParameterPassing::Value,
                },
                input_local: local.clone(),
                pattern: RuntimePatternSeed::new(
                    scalar.ty(),
                    RuntimePatternSeedKind::Bind {
                        mutable: false,
                        local,
                    },
                ),
                ownership: Default::default(),
                unrestricted_bindings: Box::new([]),
            },
        )
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let site = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([0x91; 32]),
            role: RuntimeFunctionSemanticRole::Ordinary,
            function_type: None,
            inputs,
            result: scalar.ty(),
            body_kind,
            effects: RuntimeEffectSet::empty(),
        })
        .unwrap();
    builder
        .define_function_site_seed(&site, body(admission.local_ids()))
        .unwrap();
    let plan = Arc::new(builder.finish().unwrap());
    assert!(
        plan.pure_helpers().is_empty(),
        "ordinary body is not copied into a recipe"
    );
    let site = plan.function_sites().iter_with_ids().next().unwrap().0;
    PureFunctionRequest::try_new(plan, site, (0..3).map(|_| scalar_number(scalar, 0))).unwrap()
}

fn numeric_scope_request(scalar: Scalar) -> PureFunctionRequest {
    ordinary_scope_request(scalar, |ids| {
        scoped(
            scalar.ty(),
            "outer",
            expr(
                scalar,
                RuntimeExprSeedKind::Let {
                    binding: ids[3].clone(),
                    expr: Box::new(binary(
                        scalar,
                        local(scalar, ids[0].clone()),
                        RuntimeBinaryOp::Add,
                        local(scalar, ids[1].clone()),
                    )),
                    body: Box::new(scoped(
                        scalar.ty(),
                        "inner",
                        if_expr(
                            scalar,
                            scoped(
                                bool_ty(),
                                "condition",
                                compare(
                                    local(scalar, ids[3].clone()),
                                    RuntimeBinaryOp::Lt,
                                    value(scalar, scalar_number(scalar, 10)),
                                ),
                            ),
                            scoped(
                                scalar.ty(),
                                "then_value",
                                binary(
                                    scalar,
                                    local(scalar, ids[3].clone()),
                                    RuntimeBinaryOp::Mul,
                                    value(scalar, scalar_number(scalar, 2)),
                                ),
                            ),
                            scoped(
                                scalar.ty(),
                                "else_value",
                                binary(
                                    scalar,
                                    local(scalar, ids[3].clone()),
                                    RuntimeBinaryOp::Add,
                                    value(scalar, scalar_number(scalar, 1)),
                                ),
                            ),
                        ),
                    )),
                },
            ),
        )
    })
}

fn numeric_executable_request(scalar: Scalar) -> PureFunctionRequest {
    use arcweft_core::plan::{RuntimeFlowOpSeed as Op, RuntimePatternSeed, RuntimePatternSeedKind};
    ordinary_executable_request(scalar, |ids| {
        vec![
            Op::Scope {
                identity: arcweft_core::scope::RuntimeScopeIdentity::Named(
                    "outer".parse().unwrap(),
                ),
                body: vec![
                    Op::Let {
                        pattern: RuntimePatternSeed::new(
                            scalar.ty(),
                            RuntimePatternSeedKind::Bind {
                                local: ids[3].clone(),
                                mutable: false,
                            },
                        ),
                        expr: binary(
                            scalar,
                            local(scalar, ids[0].clone()),
                            RuntimeBinaryOp::Add,
                            local(scalar, ids[1].clone()),
                        ),
                    },
                    Op::If {
                        condition: compare(
                            local(scalar, ids[3].clone()),
                            RuntimeBinaryOp::Lt,
                            value(scalar, scalar_number(scalar, 10)),
                        ),
                        then_ops: vec![
                            Op::EnterScope {
                                identity: arcweft_core::scope::RuntimeScopeIdentity::Named(
                                    "inner".parse().unwrap(),
                                ),
                            },
                            Op::ReturnExpr(binary(
                                scalar,
                                local(scalar, ids[3].clone()),
                                RuntimeBinaryOp::Mul,
                                value(scalar, scalar_number(scalar, 2)),
                            )),
                            // Return unwinds this still-entered scope; no synthetic
                            // ExitScope is needed by the admitted runtime body.
                        ],
                        else_ops: vec![Op::ReturnExpr(binary(
                            scalar,
                            local(scalar, ids[3].clone()),
                            RuntimeBinaryOp::Add,
                            value(scalar, scalar_number(scalar, 1)),
                        ))],
                    },
                ],
            },
            // Both branches return from the function, so this value cannot escape.
            Op::ReturnExpr(value(scalar, scalar_number(scalar, 99))),
        ]
    })
}

fn aot_scalar_body_result(
    request: &PureFunctionRequest,
    values: &[RuntimeValue],
) -> Result<RuntimeValue, RuntimeEvalError> {
    use arcweft_core::plan::RuntimePureOutputType;
    use arcweft_core::pure::{AotPureFunctionBackend, RuntimePureFunctionInputRef};
    use arcweft_core::value::{RuntimeExactInteger, RuntimeISizeValue, RuntimeUSizeValue};
    let function = request.function_ref()?;
    if function.output_type == RuntimePureOutputType::I64 {
        let plan = AotPureFunctionBackend.compile_i64_with_inputs(
            request,
            function
                .inputs
                .iter()
                .map(RuntimePureFunctionInputRef::local),
        )?;
        let values = values
            .iter()
            .map(|value| match value {
                RuntimeValue::Int(value) => value.exact_i64().expect("exact i64 fixture input"),
                _ => panic!("exact i64 fixture input"),
            })
            .collect::<Vec<_>>();
        return plan
            .call_with_inputs_scratch(&values, &mut Vec::new())
            .map(|(value, _)| RuntimeValue::i64(value));
    }
    let input = function.inputs.first().unwrap().abi();
    let plan = AotPureFunctionBackend.compile_scalar_with_inputs(
        request,
        function
            .inputs
            .iter()
            .map(RuntimePureFunctionInputRef::local),
        input,
        function.output_type,
    )?;
    let mut slots = Vec::new();
    macro_rules! exact {
        ($ty:ty) => {{
            let inputs = values
                .iter()
                .cloned()
                .map(|value| {
                    <$ty as RuntimeExactInteger>::try_from_runtime_value(function.name, value)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let (value, _) = plan.call_exact_int_with_inputs_scratch(&inputs, &mut slots)?;
            Ok(value.into_runtime_value())
        }};
    }
    match function.output_type {
        RuntimePureOutputType::I8 => exact!(i8),
        RuntimePureOutputType::I16 => exact!(i16),
        RuntimePureOutputType::I32 => exact!(i32),
        RuntimePureOutputType::I64 => unreachable!("i64 uses its dedicated admitted AOT ABI"),
        RuntimePureOutputType::I128 => exact!(i128),
        RuntimePureOutputType::ISize => exact!(RuntimeISizeValue),
        RuntimePureOutputType::U8 => exact!(u8),
        RuntimePureOutputType::U16 => exact!(u16),
        RuntimePureOutputType::U32 => exact!(u32),
        RuntimePureOutputType::U64 => exact!(u64),
        RuntimePureOutputType::U128 => exact!(u128),
        RuntimePureOutputType::USize => exact!(RuntimeUSizeValue),
        RuntimePureOutputType::F32 => {
            let values = values
                .iter()
                .map(|value| match value {
                    RuntimeValue::F32(value) => *value,
                    _ => panic!("exact f32 fixture input"),
                })
                .collect::<Vec<_>>();
            plan.call_f32_with_inputs_scratch(&values, &mut slots)
                .map(|(value, _)| RuntimeValue::F32(value))
        }
        RuntimePureOutputType::F64 => {
            let values = values
                .iter()
                .map(|value| match value {
                    RuntimeValue::F64(value) => *value,
                    _ => panic!("exact f64 fixture input"),
                })
                .collect::<Vec<_>>();
            plan.call_f64_with_inputs_scratch(&values, &mut slots)
                .map(|(value, _)| RuntimeValue::F64(value))
        }
        RuntimePureOutputType::Bool | RuntimePureOutputType::Value => {
            panic!("numeric fixture output")
        }
    }
}

fn vm_executable_result(
    request: &PureFunctionRequest,
    values: Vec<RuntimeValue>,
) -> Result<RuntimeValue, RuntimeEvalError> {
    let invocation =
        PureFunctionRequest::try_new(Arc::clone(request.plan()), request.function_id(), values)?;
    VmPureFunctionBackend
        .evaluate_invocation(
            &invocation,
            arcweft_core::step::RuntimeStepBudget { max_ops: 64 },
        )
        .map(|result| result.value)
}

#[test]
fn cranelift_executable_unsupported_child_retains_vm_and_aot_decline() {
    let request = ordinary_executable_request(Scalar::I64, |ids| {
        vec![arcweft_core::plan::RuntimeFlowOpSeed::Scope {
            identity: arcweft_core::scope::RuntimeScopeIdentity::Named("outer".parse().unwrap()),
            body: vec![arcweft_core::plan::RuntimeFlowOpSeed::ReturnExpr(expr(
                Scalar::I64,
                RuntimeExprSeedKind::IfLet {
                    pattern: arcweft_core::plan::RuntimePatternSeed::new(
                        Scalar::I64.ty(),
                        arcweft_core::plan::RuntimePatternSeedKind::Literal(RuntimeValue::i64(0)),
                    ),
                    expr: Box::new(local(Scalar::I64, ids[0].clone())),
                    guard: None,
                    then_expr: Box::new(value(Scalar::I64, RuntimeValue::i64(7))),
                    else_expr: Box::new(value(Scalar::I64, RuntimeValue::i64(9))),
                },
            ))],
        }]
    });
    let function = request.function_ref().unwrap();
    assert!(function.body.is_executable());
    assert_eq!(
        VmPureFunctionBackend
            .evaluate_invocation(
                &request,
                arcweft_core::step::RuntimeStepBudget { max_ops: 64 }
            )
            .unwrap()
            .value,
        RuntimeValue::i64(7)
    );
    let locals = function
        .inputs
        .iter()
        .map(arcweft_core::pure::RuntimePureFunctionInputRef::local)
        .collect::<Vec<_>>();
    assert!(matches!(
        CraneliftPureFunctionBackend.compile_i64_with_inputs(&request, locals),
        Err(CraneliftCodegenError::UnsupportedExpr(_))
    ));
    assert!(matches!(
        aot_scalar_body_result(
            &request,
            &request
                .bindings()
                .iter()
                .map(|binding| binding.value.clone())
                .collect::<Vec<_>>()
        ),
        Err(RuntimeEvalError::UnsupportedPure { .. })
    ));
}

#[test]
fn cranelift_signed_division_returns_checked_faults_and_wrapping_core_results() {
    use arcweft_core::pure::AotPureFunctionBackend;
    use arcweft_core::value::RuntimeExpressionFailure;
    let request = ordinary_executable_request(Scalar::I64, |ids| {
        vec![arcweft_core::plan::RuntimeFlowOpSeed::ReturnExpr(binary(
            Scalar::I64,
            local(Scalar::I64, ids[0].clone()),
            RuntimeBinaryOp::Div,
            local(Scalar::I64, ids[1].clone()),
        ))]
    });
    let function = request.function_ref().unwrap();
    let locals = function
        .inputs
        .iter()
        .map(arcweft_core::pure::RuntimePureFunctionInputRef::local)
        .collect::<Vec<_>>();
    let compiled = CraneliftPureFunctionBackend
        .compile_i64_with_inputs(&request, locals.iter().copied())
        .expect("checked signed division does not execute machine traps");
    assert!(matches!(
        compiled.call(&[12, 0, 99]),
        Err(CraneliftCodegenError::Execution {
            error: RuntimeEvalError::RecoverableExpression(
                RuntimeExpressionFailure::DivisionByZero
            ),
            completed_rows: 0,
        })
    ));
    assert_eq!(compiled.call(&[i64::MIN, -1, 99]).unwrap(), i64::MIN);
    assert_eq!(compiled.call(&[21, 3, 99]).unwrap(), 7);
    let aot = AotPureFunctionBackend
        .compile_i64_with_inputs(&request, locals)
        .unwrap();
    let mut vm = arcweft_core::pure::VmPureFunctionScratch::default();
    let expression_request = ordinary_scope_request(Scalar::I64, |ids| {
        binary(
            Scalar::I64,
            local(Scalar::I64, ids[0].clone()),
            RuntimeBinaryOp::Div,
            local(Scalar::I64, ids[1].clone()),
        )
    });
    let expression = expression_request.function_ref().unwrap();
    let zero = [
        RuntimeValue::i64(12),
        RuntimeValue::i64(0),
        RuntimeValue::i64(99),
    ];
    assert!(matches!(
        vm.evaluate_values(expression_request.plan(), expression.id(), Vec::from(zero)),
        Err(RuntimeEvalError::RecoverableExpression(
            RuntimeExpressionFailure::DivisionByZero
        ))
    ));
    assert!(matches!(
        aot.call_with_inputs(&[12, 0, 99]),
        Err(RuntimeEvalError::RecoverableExpression(
            RuntimeExpressionFailure::DivisionByZero
        ))
    ));
    // RuntimeDeterministicNumeric in Core owns wrapping division when rhs !=0.
    assert_eq!(
        vm_executable_result(
            &request,
            vec![
                RuntimeValue::i64(i64::MIN),
                RuntimeValue::i64(-1),
                RuntimeValue::i64(99)
            ]
        )
        .unwrap(),
        RuntimeValue::i64(i64::MIN)
    );
    assert_eq!(
        aot.call_with_inputs(&[i64::MIN, -1, 99]).unwrap().0,
        i64::MIN
    );
    assert_eq!(
        vm_executable_result(
            &request,
            vec![
                RuntimeValue::i64(21),
                RuntimeValue::i64(3),
                RuntimeValue::i64(99)
            ]
        )
        .unwrap(),
        RuntimeValue::i64(7)
    );
    assert_eq!(aot.call_with_inputs(&[21, 3, 99]).unwrap().0, 7);
}

#[test]
fn cranelift_ordinary_scopes_preserve_numeric_abis_and_full_formals() {
    macro_rules! check {
        ($scalar:ident, $compile:ident, $rows:expr, $wrap:expr) => {{
            let request = numeric_scope_request(Scalar::$scalar);
            let function = request.function_ref().unwrap();
            let locals = function
                .inputs
                .iter()
                .map(arcweft_core::pure::RuntimePureFunctionInputRef::local)
                .collect::<Vec<_>>();
            assert_eq!(locals.len(), 3, "unused formal remains in the admitted ABI");
            assert!(
                PureFunctionRequest::try_new(
                    Arc::clone(request.plan()),
                    function.id(),
                    (0..2).map(|_| scalar_number(Scalar::$scalar, 0))
                )
                .is_err()
            );
            let compiled = CraneliftPureFunctionBackend
                .$compile(&request, locals.iter().copied())
                .unwrap();
            let mut vm = arcweft_core::pure::VmPureFunctionScratch::default();
            for (arguments, expected) in $rows {
                assert!(
                    compiled.call(&arguments[..2]).is_err(),
                    "compiled full formal arity is enforced"
                );
                assert_eq!(compiled.call(&arguments).unwrap(), expected);
                let expected = $wrap(expected);
                let actual = vm
                    .evaluate_values(
                        request.plan(),
                        function.id(),
                        arguments.into_iter().map($wrap).collect(),
                    )
                    .unwrap();
                assert_eq!(actual, expected);
            }
        }};
    }
    check!(
        I8,
        compile_i8_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::i8
    );
    check!(
        I16,
        compile_i16_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::i16
    );
    check!(
        I32,
        compile_i32_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::i32
    );
    check!(
        I64,
        compile_i64_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::i64
    );
    check!(
        I128,
        compile_i128_batch_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::i128
    );
    check!(
        ISize,
        compile_i64_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::isize
    );
    check!(
        U8,
        compile_u8_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::u8
    );
    check!(
        U16,
        compile_u16_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::u16
    );
    check!(
        U32,
        compile_u32_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::u32
    );
    check!(
        U64,
        compile_u64_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::u64
    );
    check!(
        U128,
        compile_u128_batch_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::u128
    );
    check!(
        USize,
        compile_u64_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::usize
    );
    check!(
        F32,
        compile_f32_with_inputs,
        [([3.0, 4.0, 99.0], 14.0), ([11.0, 0.0, 77.0], 12.0)],
        RuntimeValue::F32
    );
    check!(
        F64,
        compile_f64_with_inputs,
        [([3.0, 4.0, 99.0], 14.0), ([11.0, 0.0, 77.0], 12.0)],
        RuntimeValue::F64
    );
}

#[test]
fn cranelift_scope_keeps_unsupported_child_decline() {
    let request = ordinary_scope_request(Scalar::I64, |ids| {
        scoped(
            Scalar::I64.ty(),
            "outer",
            expr(
                Scalar::I64,
                RuntimeExprSeedKind::IfLet {
                    pattern: arcweft_core::plan::RuntimePatternSeed::new(
                        Scalar::I64.ty(),
                        arcweft_core::plan::RuntimePatternSeedKind::Literal(RuntimeValue::i64(0)),
                    ),
                    expr: Box::new(local(Scalar::I64, ids[0].clone())),
                    guard: None,
                    then_expr: Box::new(value(Scalar::I64, RuntimeValue::i64(7))),
                    else_expr: Box::new(value(Scalar::I64, RuntimeValue::i64(9))),
                },
            ),
        )
    });
    assert_eq!(
        arcweft_core::pure::VmPureFunctionScratch::default()
            .evaluate_values(
                request.plan(),
                request.function_ref().unwrap().id(),
                vec![
                    RuntimeValue::i64(0),
                    RuntimeValue::i64(0),
                    RuntimeValue::i64(99)
                ]
            )
            .unwrap(),
        RuntimeValue::i64(7)
    );
    let locals = request
        .function_ref()
        .unwrap()
        .inputs
        .iter()
        .map(arcweft_core::pure::RuntimePureFunctionInputRef::local)
        .collect::<Vec<_>>();
    assert!(matches!(
        CraneliftPureFunctionBackend.compile_i64_with_inputs(&request, locals),
        Err(CraneliftCodegenError::UnsupportedExpr(_))
    ));
}

#[test]
fn cranelift_executable_returns_preserve_numeric_abis_batches_and_full_formals() {
    macro_rules! check {
        ($scalar:ident, $compile:ident, $rows:expr, $wrap:expr) => {{
            let request = numeric_executable_request(Scalar::$scalar);
            let function = request.function_ref().unwrap();
            let locals = function
                .inputs
                .iter()
                .map(arcweft_core::pure::RuntimePureFunctionInputRef::local)
                .collect::<Vec<_>>();
            assert_eq!(locals.len(), 3, "unused formal remains in the admitted ABI");
            assert!(
                PureFunctionRequest::try_new(
                    Arc::clone(request.plan()),
                    function.id(),
                    (0..2).map(|_| scalar_number(Scalar::$scalar, 0))
                )
                .is_err()
            );
            let compiled = CraneliftPureFunctionBackend
                .$compile(&request, locals.iter().copied())
                .unwrap();
            for (arguments, expected) in $rows {
                assert!(
                    compiled.call(&arguments[..2]).is_err(),
                    "compiled full formal arity is enforced"
                );
                assert_eq!(compiled.call(&arguments).unwrap(), expected);
                let mut batch = [expected; 2];
                let flat = arguments.into_iter().chain(arguments).collect::<Vec<_>>();
                compiled.call_flat_batch(&flat, &mut batch).unwrap();
                assert_eq!(batch, [expected; 2]);
                let values = arguments.into_iter().map($wrap).collect::<Vec<_>>();
                assert_eq!(
                    aot_scalar_body_result(&request, &values).unwrap(),
                    $wrap(expected)
                );
                let expected = $wrap(expected);
                let actual =
                    vm_executable_result(&request, arguments.into_iter().map($wrap).collect())
                        .unwrap();
                assert_eq!(actual, expected);
            }
        }};
    }
    check!(
        I8,
        compile_i8_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::i8
    );
    check!(
        I16,
        compile_i16_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::i16
    );
    check!(
        I32,
        compile_i32_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::i32
    );
    check!(
        I64,
        compile_i64_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::i64
    );
    check!(
        I128,
        compile_i128_batch_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::i128
    );
    check!(
        ISize,
        compile_i64_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::isize
    );
    check!(
        U8,
        compile_u8_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::u8
    );
    check!(
        U16,
        compile_u16_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::u16
    );
    check!(
        U32,
        compile_u32_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::u32
    );
    check!(
        U64,
        compile_u64_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::u64
    );
    check!(
        U128,
        compile_u128_batch_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::u128
    );
    check!(
        USize,
        compile_u64_with_inputs,
        [([3, 4, 99], 14), ([11, 0, 77], 12)],
        RuntimeValue::usize
    );
    check!(
        F32,
        compile_f32_with_inputs,
        [([3.0, 4.0, 99.0], 14.0), ([11.0, 0.0, 77.0], 12.0)],
        RuntimeValue::F32
    );
    check!(
        F64,
        compile_f64_with_inputs,
        [([3.0, 4.0, 99.0], 14.0), ([11.0, 0.0, 77.0], 12.0)],
        RuntimeValue::F64
    );
}

#[test]
fn checked_unsigned_native_outcomes_keep_fault_prefix_and_rejected_sum_values() {
    use arcweft_core::value::RuntimeExpressionFailure;
    let helper = admit(Scalar::U64, "checked_unsigned", 2, 2, |ids| {
        binary(
            Scalar::U64,
            local(Scalar::U64, ids[0].clone()),
            RuntimeBinaryOp::Div,
            local(Scalar::U64, ids[1].clone()),
        )
    });
    let request = helper.request([RuntimeValue::u64(12), RuntimeValue::u64(2)]);
    let compiled = CraneliftPureFunctionBackend
        .compile_u64_with_inputs(&request, helper.input_locals())
        .expect("checked native division compiles");
    assert_eq!(compiled.call(&[12, 2]).unwrap(), 6);
    assert!(matches!(
        compiled.call(&[12, 0]),
        Err(CraneliftCodegenError::Execution {
            error: RuntimeEvalError::RecoverableExpression(
                RuntimeExpressionFailure::DivisionByZero
            ),
            completed_rows: 0,
        })
    ));
    let mut out = [77; 3];
    assert!(matches!(
        compiled.call_flat_batch(&[12, 2, 9, 0, 20, 2], &mut out),
        Err(CraneliftCodegenError::Execution {
            error: RuntimeEvalError::RecoverableExpression(
                RuntimeExpressionFailure::DivisionByZero
            ),
            completed_rows: 1,
        })
    ));
    assert_eq!(out, [6, 77, 77], "failed row and suffix publish no output");
    assert!(matches!(
        compiled.call_flat_batch_sum(&[12, 2, 9, 0, 20, 2], 3),
        Err(CraneliftCodegenError::Execution {
            error: RuntimeEvalError::RecoverableExpression(
                RuntimeExpressionFailure::DivisionByZero
            ),
            completed_rows: 1,
        })
    ));
    assert!(
        matches!(compiled.call_flat_batch_sum(&[12,2,u64::MAX,1,20,2],3),
        Err(CraneliftCodegenError::Execution {
            error:RuntimeEvalError::UnsupportedPure {reason,..},completed_rows:1,
        }) if reason.contains(&u64::MAX.to_string()) && reason.contains("i64 sum"))
    );
    assert_eq!(compiled.call(&[u64::MAX, 1]).unwrap(), u64::MAX);
    assert_eq!(compiled.call_flat_batch_sum(&[], 0).unwrap(), 0);
    assert_eq!(NATIVE_PURE_CALL_ABI_VERSION, 1);
    assert_eq!(NativePureCallOutcome::from_code(77), None);
    assert!(matches!(
        native_abi::check_outcome(77, 0, 1),
        Err(CraneliftCodegenError::InvalidNativeOutcome { code: 77, .. })
    ));
    assert!(matches!(
        native_abi::check_outcome(0, 0, 1),
        Err(CraneliftCodegenError::InvalidNativeOutcome { code: 0, .. })
    ));
    assert!(matches!(
        native_abi::check_outcome(1, 1, 1),
        Err(CraneliftCodegenError::InvalidNativeOutcome { code: 1, .. })
    ));
}

#[test]
fn native_wide_sum_refusal_uses_the_exact_typed_conversion_without_narrowing() {
    let signed = admit(Scalar::I128, "wide_signed_sum", 1, 1, |ids| {
        local(Scalar::I128, ids[0].clone())
    });
    let request = signed.request([RuntimeValue::i128(1)]);
    let compiled = CraneliftPureFunctionBackend
        .compile_i128_batch_with_inputs(&request, signed.input_locals())
        .unwrap();
    for value in [
        i128::MIN,
        i128::from(i64::MIN) - 1,
        i128::from(i64::MAX) + 1,
        i128::MAX,
    ] {
        assert!(matches!(compiled.call_flat_batch_sum(&[1,value,2],3),
            Err(CraneliftCodegenError::Execution {error:RuntimeEvalError::UnsupportedPure {reason,..},completed_rows:1})
            if reason.contains(&value.to_string())));
    }
    assert_eq!(
        compiled
            .call_flat_batch_sum(&[i128::from(i64::MIN), i128::from(i64::MAX)], 2)
            .unwrap(),
        -1
    );
    let unsigned = admit(Scalar::U128, "wide_unsigned_sum", 1, 1, |ids| {
        local(Scalar::U128, ids[0].clone())
    });
    let request = unsigned.request([RuntimeValue::u128(1)]);
    let compiled = CraneliftPureFunctionBackend
        .compile_u128_batch_with_inputs(&request, unsigned.input_locals())
        .unwrap();
    for value in [(i64::MAX as u128) + 1, u128::MAX] {
        assert!(matches!(compiled.call_flat_batch_sum(&[1,value,2],3),
            Err(CraneliftCodegenError::Execution {error:RuntimeEvalError::UnsupportedPure {reason,..},completed_rows:1})
            if reason.contains(&value.to_string())));
    }
    assert_eq!(compiled.call_flat_batch_sum(&[1, 2, 3], 3).unwrap(), 6);
}

#[path = "tests/signed_division.rs"]
mod signed_division;
