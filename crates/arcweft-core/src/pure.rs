use crate::math::{DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64};
use crate::pattern::{
    RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimePattern, RuntimeVariantIdentity,
    match_runtime_pattern,
};
use crate::plan::{
    RuntimePlan, RuntimePlanTypeDeclaration, RuntimePlanTypeProjection, RuntimePureHelper,
    RuntimePureHelperId, RuntimePureInputType, RuntimePureOutputType,
};
use crate::runtime_id::{RuntimeFunctionSiteId, RuntimeLocalDeclarationId};
use crate::step::RuntimePureCallStats;
use crate::value::{
    RuntimeAgentExpr, RuntimeAgentValue, RuntimeBinaryOp, RuntimeCallArgument,
    RuntimeCallArgumentMode, RuntimeCallTarget, RuntimeEntityReferenceField, RuntimeEnv,
    RuntimeEvalError, RuntimeExactInteger, RuntimeExpr, RuntimeExprKind, RuntimeExprMatchArm,
    RuntimeFieldProjection, RuntimeFunctionApplyError, RuntimeFunctionValue, RuntimeISizeValue,
    RuntimeIntrinsic, RuntimeIterator, RuntimeLocalBinding, RuntimeNominalRecordExpr,
    RuntimeReductionValue, RuntimeSeq, RuntimeSignedIntWidth, RuntimeUSizeValue, RuntimeUnaryOp,
    RuntimeUnsignedIntWidth, RuntimeValue, evaluate_binary, evaluate_core_iter_collect_intrinsic,
    evaluate_core_iter_into_iter_intrinsic, evaluate_core_iter_next_intrinsic,
    evaluate_core_option_is_some_intrinsic, evaluate_core_option_unwrap_intrinsic,
    evaluate_core_range_intrinsic, evaluate_index_intrinsic, evaluate_numeric_op,
    evaluate_std_float_intrinsic, evaluate_string_intrinsic, evaluate_unary,
    runtime_sequence_values, runtime_value_into_sequence_values, runtime_value_label,
    sum_i64_sequence_ref,
};
use std::ops::Deref;
use std::sync::Arc;

mod aot;
mod runtime_backend;

/// Request for evaluating a deterministic pure helper expression.
#[derive(Clone, Debug, PartialEq)]
pub struct PureFunctionRequest {
    plan: Arc<RuntimePlan>,
    helper: RuntimePureHelperId,
    bindings: Box<[RuntimeLocalBinding]>,
}

/// Result of one pure helper backend evaluation.
#[derive(Clone, Debug, PartialEq)]
pub struct PureFunctionResult {
    pub backend: PureFunctionBackendKind,
    pub value: RuntimeValue,
    pub stats: PureFunctionStats,
}

/// Backend family used for pure helper evaluation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PureFunctionBackendKind {
    Vm,
    Aot,
    Jit,
}

/// Deterministic counters for pure helper evaluation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PureFunctionStats {
    pub evaluated_exprs: usize,
    pub evaluated_calls: usize,
    pub evaluated_binary_ops: usize,
}

/// Fixed-size scalar argument pack for runtime pure helper fast paths.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuntimeFixedArgs<T> {
    len: usize,
    values: [T; 4],
}

pub type RuntimeI32Args = RuntimeFixedArgs<i32>;
pub type RuntimeI64Args = RuntimeFixedArgs<i64>;
pub type RuntimeFloat32Args = RuntimeFixedArgs<f32>;
pub type RuntimeFloat64Args = RuntimeFixedArgs<f64>;

/// Exact integer scalar that preserves the helper ABI width during VM pure evaluation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RuntimePureScalar {
    Bool(bool),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    ISize(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    USize(u64),
    F32(f32),
    F64(f64),
}

/// Integer widths that can stay typed across pure helper VM fast paths.
pub trait RuntimePureScalarInteger: RuntimeExactInteger {
    fn into_pure_scalar(self) -> RuntimePureScalar;
}

macro_rules! impl_runtime_pure_scalar_integer {
    ($ty:ty, $variant:ident) => {
        impl RuntimePureScalarInteger for $ty {
            fn into_pure_scalar(self) -> RuntimePureScalar {
                RuntimePureScalar::$variant(self)
            }
        }
    };
}

impl_runtime_pure_scalar_integer!(i8, I8);
impl_runtime_pure_scalar_integer!(i16, I16);
impl_runtime_pure_scalar_integer!(i32, I32);
impl_runtime_pure_scalar_integer!(i128, I128);
impl_runtime_pure_scalar_integer!(u8, U8);
impl_runtime_pure_scalar_integer!(u16, U16);
impl_runtime_pure_scalar_integer!(u32, U32);
impl_runtime_pure_scalar_integer!(u64, U64);
impl_runtime_pure_scalar_integer!(u128, U128);

impl RuntimePureScalarInteger for RuntimeISizeValue {
    fn into_pure_scalar(self) -> RuntimePureScalar {
        RuntimePureScalar::ISize(self.get())
    }
}

impl RuntimePureScalarInteger for RuntimeUSizeValue {
    fn into_pure_scalar(self) -> RuntimePureScalar {
        RuntimePureScalar::USize(self.get())
    }
}

/// Runtime-facing backend for deterministic built-in math calls.
pub trait RuntimeMathCallBackend {
    fn call_math_matmul_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeEvalError>;

    fn call_math_matrix_add_f32(
        &mut self,
        lhs: &DenseMatrixF32,
        rhs: &DenseMatrixF32,
    ) -> Result<DenseMatrixF32, RuntimeEvalError>;

    fn call_math_tensor_add_f32(
        &mut self,
        lhs: &DenseTensorF32,
        rhs: &DenseTensorF32,
    ) -> Result<DenseTensorF32, RuntimeEvalError>;

    fn call_math_matmul_f64(
        &mut self,
        lhs: &DenseMatrixF64,
        rhs: &DenseMatrixF64,
    ) -> Result<DenseMatrixF64, RuntimeEvalError>;

    fn call_math_matrix_add_f64(
        &mut self,
        lhs: &DenseMatrixF64,
        rhs: &DenseMatrixF64,
    ) -> Result<DenseMatrixF64, RuntimeEvalError>;

    fn call_math_tensor_add_f64(
        &mut self,
        lhs: &DenseTensorF64,
        rhs: &DenseTensorF64,
    ) -> Result<DenseTensorF64, RuntimeEvalError>;
}

/// Adapter boundary for runtime calls that are not Arcweft Core intrinsics.
///
/// This mirrors an FFI boundary: Core evaluates argument expressions and keeps
/// their typed `RuntimeValue` shape, while adapter crates decide which named
/// calls they own and how to execute them.
pub trait RuntimeExternalCallBackend {
    fn call_external(
        &mut self,
        callee: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>>;
}

/// Stable compact-AWBC pure helper identity presented to runtime backends.
///
/// Compact product execution cannot reconstruct the structured helper expression,
/// so backends receive the canonical helper identity and ABI shape directly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCompactPureHelper {
    pub id: u32,
    pub name: String,
    pub arity: usize,
    pub scalar_eval_supported: bool,
}

/// Borrowed capability for one helper admitted by an exact runtime plan.
///
/// Backends receive this handle rather than a detached helper row, preserving
/// the plan authority needed by typed expressions, local declarations, and
/// nominal domains.
#[derive(Clone, Copy, Debug)]
pub struct RuntimePureHelperRef<'a> {
    plan: &'a Arc<RuntimePlan>,
    id: RuntimePureHelperId,
    declaration: &'a RuntimePureHelper,
}

impl<'a> RuntimePureHelperRef<'a> {
    fn new(
        plan: &'a Arc<RuntimePlan>,
        id: RuntimePureHelperId,
        declaration: &'a RuntimePureHelper,
    ) -> Self {
        Self {
            plan,
            id,
            declaration,
        }
    }

    /// Resolves one helper through its exact admitted plan.
    ///
    /// This is the request-free capability boundary used by eager backends
    /// that compile helper bodies before any concrete call arguments exist.
    pub fn resolve(
        plan: &'a Arc<RuntimePlan>,
        id: RuntimePureHelperId,
    ) -> Result<Self, RuntimeEvalError> {
        let declaration = resolve_pure_helper(plan, id)?;
        validate_pure_helper_contract(plan, declaration)?;
        Ok(Self::new(plan, id, declaration))
    }

    #[must_use]
    pub const fn plan(self) -> &'a Arc<RuntimePlan> {
        self.plan
    }

    #[must_use]
    pub const fn id(self) -> RuntimePureHelperId {
        self.id
    }

    #[must_use]
    pub const fn declaration(self) -> &'a RuntimePureHelper {
        self.declaration
    }
}

impl Deref for RuntimePureHelperRef<'_> {
    type Target = RuntimePureHelper;

    fn deref(&self) -> &Self::Target {
        self.declaration
    }
}

/// Runtime-facing backend for deterministic pure helper calls.
pub trait RuntimePureCallBackend {
    fn call_i8_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[i8],
    ) -> Result<Option<i8>, RuntimeEvalError>;

    fn call_i8_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i8],
        arity: usize,
        out: &mut [i8],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i8_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i8],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_i16_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[i16],
    ) -> Result<Option<i16>, RuntimeEvalError>;

    fn call_i16_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i16],
        arity: usize,
        out: &mut [i16],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i16_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i16],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_i128_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i128],
        arity: usize,
        out: &mut [i128],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i128_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i128],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_i32(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: RuntimeI32Args,
    ) -> Result<Option<i32>, RuntimeEvalError>;

    fn call_i32_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[i32],
    ) -> Result<Option<i32>, RuntimeEvalError>;

    fn call_i32_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i32],
        arity: usize,
        out: &mut [i32],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i32_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i32],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_u32_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[u32],
    ) -> Result<Option<u32>, RuntimeEvalError>;

    fn call_u8_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[u8],
    ) -> Result<Option<u8>, RuntimeEvalError>;

    fn call_u8_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u8],
        arity: usize,
        out: &mut [u8],
    ) -> Result<(), RuntimeEvalError>;

    fn call_u8_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u8],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_u16_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[u16],
    ) -> Result<Option<u16>, RuntimeEvalError>;

    fn call_u16_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u16],
        arity: usize,
        out: &mut [u16],
    ) -> Result<(), RuntimeEvalError>;

    fn call_u16_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u16],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_u128_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u128],
        arity: usize,
        out: &mut [u128],
    ) -> Result<(), RuntimeEvalError>;

    fn call_u128_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u128],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_u32_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u32],
        arity: usize,
        out: &mut [u32],
    ) -> Result<(), RuntimeEvalError>;

    fn call_u32_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u32],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_u64_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[u64],
    ) -> Result<Option<u64>, RuntimeEvalError>;

    fn call_u64_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u64],
        arity: usize,
        out: &mut [u64],
    ) -> Result<(), RuntimeEvalError>;

    fn call_u64_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[u64],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_exact_int_flat_batch_sum<T: RuntimePureScalarInteger>(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[T],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_exact_int_slice<T: RuntimePureScalarInteger>(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[T],
    ) -> Result<Option<T>, RuntimeEvalError>;

    fn call_exact_int_flat_batch<T: RuntimePureScalarInteger>(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[T],
        arity: usize,
        out: &mut [T],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i64(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: RuntimeI64Args,
    ) -> Result<Option<i64>, RuntimeEvalError>;

    fn call_i64_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[i64],
    ) -> Result<Option<i64>, RuntimeEvalError>;

    fn call_i64_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        rows: &[RuntimeI64Args],
        out: &mut [i64],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i64_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i64],
        arity: usize,
        out: &mut [i64],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i64_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[i64],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_i64_repeated_flat_batch_sum(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        row: &[i64],
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_f32_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[f32],
    ) -> Result<Option<f32>, RuntimeEvalError>;

    fn call_f32_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[f32],
        arity: usize,
        out: &mut [f32],
    ) -> Result<(), RuntimeEvalError>;

    fn call_f64_slice(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[f64],
    ) -> Result<Option<f64>, RuntimeEvalError>;

    fn call_f64_flat_batch(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        flat_inputs: &[f64],
        arity: usize,
        out: &mut [f64],
    ) -> Result<(), RuntimeEvalError>;

    fn call_values(
        &mut self,
        helper: RuntimePureHelperRef<'_>,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, RuntimeEvalError>;

    /// Optionally evaluates a canonical compact-AWBC helper directly.
    ///
    /// Returning `None` selects the verified compact VM fallback owned by the
    /// product executor. Returning `Some` preserves backend selection and
    /// deterministic success/failure at the shared runtime boundary.
    fn call_compact_values(
        &mut self,
        helper: &RuntimeCompactPureHelper,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        let _ = (helper, args);
        None
    }

    fn stats(&self) -> RuntimePureCallStats;
}

/// Backend accepted by runtime expression evaluation.
pub trait RuntimeCallBackend:
    RuntimePureCallBackend + RuntimeMathCallBackend + RuntimeExternalCallBackend
{
}

impl<T> RuntimeCallBackend for T where
    T: RuntimePureCallBackend + RuntimeMathCallBackend + RuntimeExternalCallBackend
{
}

/// Backend contract for pure deterministic helper evaluation.
pub trait PureFunctionBackend {
    fn kind(&self) -> PureFunctionBackendKind;

    fn evaluate(
        &self,
        request: &PureFunctionRequest,
    ) -> Result<PureFunctionResult, RuntimeEvalError>;
}

/// VM fallback backend for pure helpers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VmPureFunctionBackend;

/// Reusable VM fallback storage for repeated `i64` pure-helper evaluation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VmPureFunctionScratch {
    env: RuntimeEnv,
}

/// AOT backend for deterministic pure helpers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AotPureFunctionBackend;

/// VM runtime backend used when no external pure accelerator is provided.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VmRuntimePureCallBackend {
    stats: RuntimePureCallStats,
    scratch: VmPureFunctionScratch,
}

/// Compiled AOT plan for the current deterministic `i64` pure-helper subset.
#[derive(Clone, Debug, PartialEq)]
pub struct AotPureI64Plan {
    plan: Arc<RuntimePlan>,
    helper: RuntimePureHelperId,
    expr: aot::AotI64Expr,
    initial_slots: Vec<i64>,
    input_slots: Vec<usize>,
    slot_count: usize,
}

/// Compiled AOT plan for exact-width scalar helpers that are not widened to `i64`.
#[derive(Clone, Debug, PartialEq)]
pub struct AotPureScalarPlan {
    plan: Arc<RuntimePlan>,
    helper: RuntimePureHelperId,
    expr: aot::AotScalarExpr,
    initial_slots: Vec<RuntimePureScalar>,
    input_slots: Vec<usize>,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
    slot_count: usize,
}

/// VM/JIT conformance result for deterministic helper execution.
#[derive(Clone, Debug, PartialEq)]
pub struct PureFunctionConformance {
    pub vm: PureFunctionResult,
    pub candidate: PureFunctionResult,
    pub matches_vm: bool,
}

impl PureFunctionRequest {
    /// Creates a request for one helper admitted by the exact owning plan.
    ///
    /// Input values are assigned only through the helper's plan-local input
    /// coordinates. Source names and detached expression trees are not runtime
    /// evaluation authority.
    pub fn try_new(
        plan: Arc<RuntimePlan>,
        helper: RuntimePureHelperId,
        args: impl IntoIterator<Item = RuntimeValue>,
    ) -> Result<Self, RuntimeEvalError> {
        let args = args.into_iter().collect::<Vec<_>>();
        let declaration = resolve_pure_helper(&plan, helper)?;
        validate_pure_helper_contract(&plan, declaration)?;
        if args.len() != declaration.input_locals.len() {
            return Err(RuntimeEvalError::TooManyPureArgs {
                helper: declaration.name.clone(),
                max: declaration.input_locals.len(),
                found: args.len(),
            });
        }
        let mut bindings = Vec::with_capacity(args.len());
        for (&local, value) in declaration.input_locals.iter().zip(args) {
            let local_declaration = plan
                .local_declarations()
                .get(local)
                .ok_or(RuntimeEvalError::UnknownLocal(local))?;
            if !plan.value_matches_type(local_declaration.ty(), &value)? {
                return Err(RuntimeEvalError::InvalidExpressionType(
                    local_declaration.ty(),
                ));
            }
            bindings.push(RuntimeLocalBinding { local, value });
        }
        Ok(Self {
            plan,
            helper,
            bindings: bindings.into_boxed_slice(),
        })
    }

    #[must_use]
    pub fn plan(&self) -> &Arc<RuntimePlan> {
        &self.plan
    }

    pub fn helper_ref(&self) -> Result<RuntimePureHelperRef<'_>, RuntimeEvalError> {
        RuntimePureHelperRef::resolve(&self.plan, self.helper)
    }

    #[must_use]
    pub const fn helper_id(&self) -> RuntimePureHelperId {
        self.helper
    }

    #[must_use]
    pub fn bindings(&self) -> &[RuntimeLocalBinding] {
        &self.bindings
    }
}

fn resolve_pure_helper(
    plan: &RuntimePlan,
    helper: RuntimePureHelperId,
) -> Result<&RuntimePureHelper, RuntimeEvalError> {
    plan.pure_helpers()
        .get(helper.0)
        .filter(|candidate| candidate.id == helper)
        .ok_or(RuntimeEvalError::UnknownPureHelper(helper.0))
}

fn resolve_validated_pure_helper(
    plan: &Arc<RuntimePlan>,
    helper: RuntimePureHelperId,
) -> Result<&RuntimePureHelper, RuntimeEvalError> {
    Ok(RuntimePureHelperRef::resolve(plan, helper)?.declaration())
}

fn validate_pure_helper_contract(
    plan: &RuntimePlan,
    helper: &RuntimePureHelper,
) -> Result<(), RuntimeEvalError> {
    if helper.input_locals.len() != helper.input_types.len() {
        return Err(RuntimeEvalError::UnsupportedPure {
            name: helper.name.clone(),
            reason: "input local and ABI type counts differ".to_owned(),
        });
    }
    for (&local, &input_type) in helper.input_locals.iter().zip(&helper.input_types) {
        let declaration = plan
            .local_declarations()
            .get(local)
            .ok_or(RuntimeEvalError::UnknownLocal(local))?;
        if input_type != RuntimePureInputType::Value
            && pure_scalar_projection(plan, declaration.ty()) != Some(input_as_output(input_type))
        {
            return Err(RuntimeEvalError::InvalidExpressionType(declaration.ty()));
        }
    }
    if helper.output_type != RuntimePureOutputType::Value
        && pure_scalar_projection(plan, helper.expr.ty()) != Some(helper.output_type)
    {
        return Err(RuntimeEvalError::InvalidExpressionType(helper.expr.ty()));
    }
    Ok(())
}

fn pure_scalar_projection(
    plan: &RuntimePlan,
    ty: crate::runtime_id::RuntimePlanTypeId,
) -> Option<RuntimePureOutputType> {
    let declaration = plan.type_table().get(ty)?;
    Some(match declaration.projection() {
        RuntimePlanTypeProjection::Bool => RuntimePureOutputType::Bool,
        RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I8) => RuntimePureOutputType::I8,
        RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I16) => RuntimePureOutputType::I16,
        RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I32) => RuntimePureOutputType::I32,
        RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64) => RuntimePureOutputType::I64,
        RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I128) => {
            RuntimePureOutputType::I128
        }
        RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::ISize) => {
            RuntimePureOutputType::ISize
        }
        RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U8) => {
            RuntimePureOutputType::U8
        }
        RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U16) => {
            RuntimePureOutputType::U16
        }
        RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U32) => {
            RuntimePureOutputType::U32
        }
        RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U64) => {
            RuntimePureOutputType::U64
        }
        RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U128) => {
            RuntimePureOutputType::U128
        }
        RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::USize) => {
            RuntimePureOutputType::USize
        }
        RuntimePlanTypeProjection::F32 => RuntimePureOutputType::F32,
        RuntimePlanTypeProjection::F64 => RuntimePureOutputType::F64,
        _ => return None,
    })
}

const fn input_as_output(input: RuntimePureInputType) -> RuntimePureOutputType {
    match input {
        RuntimePureInputType::I8 => RuntimePureOutputType::I8,
        RuntimePureInputType::I16 => RuntimePureOutputType::I16,
        RuntimePureInputType::I32 => RuntimePureOutputType::I32,
        RuntimePureInputType::I64 => RuntimePureOutputType::I64,
        RuntimePureInputType::I128 => RuntimePureOutputType::I128,
        RuntimePureInputType::ISize => RuntimePureOutputType::ISize,
        RuntimePureInputType::U8 => RuntimePureOutputType::U8,
        RuntimePureInputType::U16 => RuntimePureOutputType::U16,
        RuntimePureInputType::U32 => RuntimePureOutputType::U32,
        RuntimePureInputType::U64 => RuntimePureOutputType::U64,
        RuntimePureInputType::U128 => RuntimePureOutputType::U128,
        RuntimePureInputType::USize => RuntimePureOutputType::USize,
        RuntimePureInputType::F32 => RuntimePureOutputType::F32,
        RuntimePureInputType::F64 => RuntimePureOutputType::F64,
        RuntimePureInputType::Value => RuntimePureOutputType::Value,
    }
}

impl PureFunctionBackend for VmPureFunctionBackend {
    fn kind(&self) -> PureFunctionBackendKind {
        PureFunctionBackendKind::Vm
    }

    fn evaluate(
        &self,
        request: &PureFunctionRequest,
    ) -> Result<PureFunctionResult, RuntimeEvalError> {
        let helper = request.helper_ref()?.declaration();
        let mut evaluator = PureEvaluator::new_ref(&request.plan, &request.bindings);
        let value = evaluator.evaluate_expr(&helper.expr)?;
        Ok(PureFunctionResult {
            backend: self.kind(),
            value,
            stats: evaluator.stats,
        })
    }
}

impl VmPureFunctionBackend {
    pub fn evaluate_i32_args(
        &self,
        plan: &Arc<RuntimePlan>,
        helper: RuntimePureHelperId,
        args: RuntimeI32Args,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.evaluate_i32_slice(plan, helper, args.as_slice())
    }

    pub fn evaluate_i32_slice(
        &self,
        plan: &Arc<RuntimePlan>,
        helper: RuntimePureHelperId,
        args: &[i32],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let mut scratch = VmPureFunctionScratch::default();
        scratch.evaluate_i32_slice(plan, helper, args)
    }

    pub fn evaluate_i64_args(
        &self,
        plan: &Arc<RuntimePlan>,
        helper: RuntimePureHelperId,
        args: RuntimeI64Args,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.evaluate_i64_slice(plan, helper, args.as_slice())
    }

    pub fn evaluate_i64_slice(
        &self,
        plan: &Arc<RuntimePlan>,
        helper: RuntimePureHelperId,
        args: &[i64],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let mut scratch = VmPureFunctionScratch::default();
        scratch.evaluate_i64_slice(plan, helper, args)
    }

    pub fn evaluate_f32_slice(
        &self,
        plan: &Arc<RuntimePlan>,
        helper: RuntimePureHelperId,
        args: &[f32],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let mut scratch = VmPureFunctionScratch::default();
        scratch.evaluate_f32_slice(plan, helper, args)
    }

    pub fn evaluate_f64_slice(
        &self,
        plan: &Arc<RuntimePlan>,
        helper: RuntimePureHelperId,
        args: &[f64],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let mut scratch = VmPureFunctionScratch::default();
        scratch.evaluate_f64_slice(plan, helper, args)
    }
}

impl VmPureFunctionScratch {
    pub fn evaluate_i32_args(
        &mut self,
        plan: &Arc<RuntimePlan>,
        helper: RuntimePureHelperId,
        args: RuntimeI32Args,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.evaluate_i32_slice(plan, helper, args.as_slice())
    }

    pub fn evaluate_i32_slice(
        &mut self,
        plan: &Arc<RuntimePlan>,
        helper: RuntimePureHelperId,
        args: &[i32],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let helper = resolve_validated_pure_helper(plan, helper)?;
        let bindings =
            prepare_helper_bindings(plan, helper, args.iter().copied().map(RuntimeValue::i32))?;
        self.env.replace_scopes_with_bindings([bindings]);
        let mut evaluator = PureEvaluator::with_env(plan, std::mem::take(&mut self.env));
        let result = validate_helper_result(
            plan,
            helper,
            if helper.scalar_eval_supported {
                evaluator
                    .evaluate_scalar_expr(&helper.expr)
                    .map(RuntimePureScalar::into_runtime_value)
            } else {
                evaluator.evaluate_expr(&helper.expr)
            },
        );
        self.env = evaluator.into_env();
        result
    }

    pub fn evaluate_exact_int_slice<T: RuntimePureScalarInteger>(
        &mut self,
        plan: &Arc<RuntimePlan>,
        helper: RuntimePureHelperId,
        args: &[T],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let helper = resolve_validated_pure_helper(plan, helper)?;
        let bindings = prepare_helper_bindings(
            plan,
            helper,
            args.iter()
                .copied()
                .map(RuntimePureScalarInteger::into_pure_scalar)
                .map(RuntimePureScalar::into_runtime_value),
        )?;
        if helper.scalar_eval_supported {
            let mut evaluator = PureScalarEvaluator::new_exact(&helper.input_locals, args);
            return validate_helper_result(
                plan,
                helper,
                evaluator
                    .evaluate(&helper.expr)
                    .map(RuntimePureScalar::into_runtime_value),
            );
        }
        self.env.replace_scopes_with_bindings([bindings]);
        let mut evaluator = PureEvaluator::with_env(plan, std::mem::take(&mut self.env));
        let result = validate_helper_result(
            plan,
            helper,
            if helper.scalar_eval_supported {
                evaluator
                    .evaluate_scalar_expr(&helper.expr)
                    .map(RuntimePureScalar::into_runtime_value)
            } else {
                evaluator.evaluate_expr(&helper.expr)
            },
        );
        self.env = evaluator.into_env();
        result
    }

    pub fn evaluate_i64_args(
        &mut self,
        plan: &Arc<RuntimePlan>,
        helper: RuntimePureHelperId,
        args: RuntimeI64Args,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.evaluate_i64_slice(plan, helper, args.as_slice())
    }

    pub fn evaluate_i64_slice(
        &mut self,
        plan: &Arc<RuntimePlan>,
        helper: RuntimePureHelperId,
        args: &[i64],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let helper = resolve_validated_pure_helper(plan, helper)?;
        let bindings =
            prepare_helper_bindings(plan, helper, args.iter().copied().map(RuntimeValue::i64))?;
        self.env.replace_scopes_with_bindings([bindings]);
        let mut evaluator = PureEvaluator::with_env(plan, std::mem::take(&mut self.env));
        let result = validate_helper_result(
            plan,
            helper,
            if helper.scalar_eval_supported {
                evaluator
                    .evaluate_scalar_expr(&helper.expr)
                    .map(RuntimePureScalar::into_runtime_value)
            } else {
                evaluator.evaluate_expr(&helper.expr)
            },
        );
        self.env = evaluator.into_env();
        result
    }

    pub fn evaluate_f32_slice(
        &mut self,
        plan: &Arc<RuntimePlan>,
        helper: RuntimePureHelperId,
        args: &[f32],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let helper = resolve_validated_pure_helper(plan, helper)?;
        let bindings =
            prepare_helper_bindings(plan, helper, args.iter().copied().map(RuntimeValue::F32))?;
        self.env.replace_scopes_with_bindings([bindings]);
        let mut evaluator = PureEvaluator::with_env(plan, std::mem::take(&mut self.env));
        let result = validate_helper_result(
            plan,
            helper,
            if helper.scalar_eval_supported {
                evaluator
                    .evaluate_scalar_expr(&helper.expr)
                    .map(RuntimePureScalar::into_runtime_value)
            } else {
                evaluator.evaluate_expr(&helper.expr)
            },
        );
        self.env = evaluator.into_env();
        result
    }

    pub fn evaluate_f64_slice(
        &mut self,
        plan: &Arc<RuntimePlan>,
        helper: RuntimePureHelperId,
        args: &[f64],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let helper = resolve_validated_pure_helper(plan, helper)?;
        let bindings =
            prepare_helper_bindings(plan, helper, args.iter().copied().map(RuntimeValue::F64))?;
        self.env.replace_scopes_with_bindings([bindings]);
        let mut evaluator = PureEvaluator::with_env(plan, std::mem::take(&mut self.env));
        let result = validate_helper_result(
            plan,
            helper,
            if helper.scalar_eval_supported {
                evaluator
                    .evaluate_scalar_expr(&helper.expr)
                    .map(RuntimePureScalar::into_runtime_value)
            } else {
                evaluator.evaluate_expr(&helper.expr)
            },
        );
        self.env = evaluator.into_env();
        result
    }

    pub fn evaluate_values(
        &mut self,
        plan: &Arc<RuntimePlan>,
        helper: RuntimePureHelperId,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let helper = resolve_validated_pure_helper(plan, helper)?;
        let bindings = prepare_helper_bindings(plan, helper, args.iter().cloned())?;
        self.env.replace_scopes_with_bindings([bindings]);
        let mut evaluator = PureEvaluator::with_env(plan, std::mem::take(&mut self.env));
        let result = validate_helper_result(
            plan,
            helper,
            if helper.scalar_eval_supported {
                evaluator
                    .evaluate_scalar_expr(&helper.expr)
                    .map(RuntimePureScalar::into_runtime_value)
            } else {
                evaluator.evaluate_expr(&helper.expr)
            },
        );
        self.env = evaluator.into_env();
        result
    }
}

fn prepare_helper_bindings(
    plan: &RuntimePlan,
    helper: &RuntimePureHelper,
    values: impl IntoIterator<Item = RuntimeValue>,
) -> Result<Vec<RuntimeLocalBinding>, RuntimeEvalError> {
    let values = values.into_iter().collect::<Vec<_>>();
    if values.len() != helper.input_locals.len() {
        return Err(RuntimeEvalError::TooManyPureArgs {
            helper: helper.name.clone(),
            max: helper.input_locals.len(),
            found: values.len(),
        });
    }
    helper
        .input_locals
        .iter()
        .copied()
        .zip(values)
        .map(|(local, value)| {
            let declaration = plan
                .local_declarations()
                .get(local)
                .ok_or(RuntimeEvalError::UnknownLocal(local))?;
            if !plan.value_matches_type(declaration.ty(), &value)? {
                return Err(RuntimeEvalError::InvalidExpressionType(declaration.ty()));
            }
            Ok(RuntimeLocalBinding { local, value })
        })
        .collect()
}

fn validate_helper_result(
    plan: &RuntimePlan,
    helper: &RuntimePureHelper,
    result: Result<RuntimeValue, RuntimeEvalError>,
) -> Result<RuntimeValue, RuntimeEvalError> {
    let value = result?;
    if !plan.value_matches_type(helper.expr.ty(), &value)? {
        return Err(RuntimeEvalError::InvalidExpressionType(helper.expr.ty()));
    }
    Ok(value)
}

impl<T> RuntimeFixedArgs<T> {
    pub const MAX: usize = 4;

    pub const fn new(values: [T; 4], len: usize) -> Self {
        Self { len, values }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn as_slice(&self) -> &[T] {
        &self.values[..self.len]
    }

    pub fn into_parts(self) -> ([T; 4], usize) {
        (self.values, self.len)
    }
}

pub fn compare_pure_function_backend(
    vm: &impl PureFunctionBackend,
    candidate: &impl PureFunctionBackend,
    request: &PureFunctionRequest,
) -> Result<PureFunctionConformance, RuntimeEvalError> {
    let vm = vm.evaluate(request)?;
    let candidate = candidate.evaluate(request)?;
    let matches_vm = candidate.value == vm.value;
    Ok(PureFunctionConformance {
        vm,
        candidate,
        matches_vm,
    })
}

struct PureEvaluator<'a> {
    plan: &'a Arc<RuntimePlan>,
    env: RuntimeEnv,
    stats: PureFunctionStats,
}

impl RuntimePureScalar {
    fn default_for_output(output_type: RuntimePureOutputType) -> Result<Self, RuntimeEvalError> {
        match output_type {
            RuntimePureOutputType::Bool => Ok(Self::Bool(false)),
            RuntimePureOutputType::I8 => Ok(Self::I8(0)),
            RuntimePureOutputType::I16 => Ok(Self::I16(0)),
            RuntimePureOutputType::I32 => Ok(Self::I32(0)),
            RuntimePureOutputType::I64 => Ok(Self::I64(0)),
            RuntimePureOutputType::I128 => Ok(Self::I128(0)),
            RuntimePureOutputType::ISize => Ok(Self::ISize(0)),
            RuntimePureOutputType::U8 => Ok(Self::U8(0)),
            RuntimePureOutputType::U16 => Ok(Self::U16(0)),
            RuntimePureOutputType::U32 => Ok(Self::U32(0)),
            RuntimePureOutputType::U64 => Ok(Self::U64(0)),
            RuntimePureOutputType::U128 => Ok(Self::U128(0)),
            RuntimePureOutputType::USize => Ok(Self::USize(0)),
            RuntimePureOutputType::F32 => Ok(Self::F32(0.0)),
            RuntimePureOutputType::F64 => Ok(Self::F64(0.0)),
            RuntimePureOutputType::Value => Err(RuntimeEvalError::UnsupportedPure {
                name: "pure".to_owned(),
                reason: "AOT scalar slots require a concrete scalar output type".to_owned(),
            }),
        }
    }

    const fn into_runtime_value(self) -> RuntimeValue {
        match self {
            Self::Bool(value) => RuntimeValue::Bool(value),
            Self::I8(value) => RuntimeValue::i8(value),
            Self::I16(value) => RuntimeValue::i16(value),
            Self::I32(value) => RuntimeValue::i32(value),
            Self::I64(value) => RuntimeValue::i64(value),
            Self::I128(value) => RuntimeValue::i128(value),
            Self::ISize(value) => RuntimeValue::isize(value),
            Self::U8(value) => RuntimeValue::u8(value),
            Self::U16(value) => RuntimeValue::u16(value),
            Self::U32(value) => RuntimeValue::u32(value),
            Self::U64(value) => RuntimeValue::u64(value),
            Self::U128(value) => RuntimeValue::u128(value),
            Self::USize(value) => RuntimeValue::usize(value),
            Self::F32(value) => RuntimeValue::F32(value),
            Self::F64(value) => RuntimeValue::F64(value),
        }
    }

    fn label(self) -> String {
        match self {
            Self::Bool(value) => value.to_string(),
            Self::I8(value) => value.to_string(),
            Self::I16(value) => value.to_string(),
            Self::I32(value) => value.to_string(),
            Self::I64(value) | Self::ISize(value) => value.to_string(),
            Self::I128(value) => value.to_string(),
            Self::U8(value) => value.to_string(),
            Self::U16(value) => value.to_string(),
            Self::U32(value) => value.to_string(),
            Self::U64(value) | Self::USize(value) => value.to_string(),
            Self::U128(value) => value.to_string(),
            Self::F32(value) => value.to_string(),
            Self::F64(value) => value.to_string(),
        }
    }
}

fn runtime_value_as_scalar(value: &RuntimeValue) -> Option<RuntimePureScalar> {
    match value {
        RuntimeValue::Bool(value) => Some(RuntimePureScalar::Bool(*value)),
        RuntimeValue::Int(value) => Some(runtime_int_as_scalar(*value)),
        RuntimeValue::UInt(value) => Some(runtime_uint_as_scalar(*value)),
        RuntimeValue::F32(value) => Some(RuntimePureScalar::F32(*value)),
        RuntimeValue::F64(value) => Some(RuntimePureScalar::F64(*value)),
        _ => None,
    }
}

fn runtime_value_into_scalar(value: RuntimeValue) -> Result<RuntimePureScalar, RuntimeEvalError> {
    match value {
        RuntimeValue::Bool(value) => Ok(RuntimePureScalar::Bool(value)),
        RuntimeValue::Int(value) => Ok(runtime_int_as_scalar(value)),
        RuntimeValue::UInt(value) => Ok(runtime_uint_as_scalar(value)),
        RuntimeValue::F32(value) => Ok(RuntimePureScalar::F32(value)),
        RuntimeValue::F64(value) => Ok(RuntimePureScalar::F64(value)),
        value => Err(RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
    }
}

fn runtime_int_as_scalar(value: crate::value::RuntimeInt) -> RuntimePureScalar {
    match value {
        crate::value::RuntimeInt::I8(value) => RuntimePureScalar::I8(value),
        crate::value::RuntimeInt::I16(value) => RuntimePureScalar::I16(value),
        crate::value::RuntimeInt::I32(value) => RuntimePureScalar::I32(value),
        crate::value::RuntimeInt::I64(value) => RuntimePureScalar::I64(value),
        crate::value::RuntimeInt::I128(value) => RuntimePureScalar::I128(value),
        crate::value::RuntimeInt::ISize(value) => RuntimePureScalar::ISize(value),
    }
}

fn runtime_uint_as_scalar(value: crate::value::RuntimeUInt) -> RuntimePureScalar {
    match value {
        crate::value::RuntimeUInt::U8(value) => RuntimePureScalar::U8(value),
        crate::value::RuntimeUInt::U16(value) => RuntimePureScalar::U16(value),
        crate::value::RuntimeUInt::U32(value) => RuntimePureScalar::U32(value),
        crate::value::RuntimeUInt::U64(value) => RuntimePureScalar::U64(value),
        crate::value::RuntimeUInt::U128(value) => RuntimePureScalar::U128(value),
        crate::value::RuntimeUInt::USize(value) => RuntimePureScalar::USize(value),
    }
}

fn evaluate_scalar_unary(
    op: RuntimeUnaryOp,
    value: RuntimePureScalar,
) -> Result<RuntimePureScalar, RuntimeEvalError> {
    match (op, value) {
        (RuntimeUnaryOp::Not, RuntimePureScalar::Bool(value)) => {
            Ok(RuntimePureScalar::Bool(!value))
        }
        (RuntimeUnaryOp::Neg, RuntimePureScalar::I8(value)) => {
            Ok(RuntimePureScalar::I8(value.wrapping_neg()))
        }
        (RuntimeUnaryOp::Neg, RuntimePureScalar::I16(value)) => {
            Ok(RuntimePureScalar::I16(value.wrapping_neg()))
        }
        (RuntimeUnaryOp::Neg, RuntimePureScalar::I32(value)) => {
            Ok(RuntimePureScalar::I32(value.wrapping_neg()))
        }
        (RuntimeUnaryOp::Neg, RuntimePureScalar::I64(value)) => {
            Ok(RuntimePureScalar::I64(value.wrapping_neg()))
        }
        (RuntimeUnaryOp::Neg, RuntimePureScalar::I128(value)) => {
            Ok(RuntimePureScalar::I128(value.wrapping_neg()))
        }
        (RuntimeUnaryOp::Neg, RuntimePureScalar::ISize(value)) => {
            Ok(RuntimePureScalar::ISize(value.wrapping_neg()))
        }
        (RuntimeUnaryOp::Neg, RuntimePureScalar::F32(value)) => Ok(RuntimePureScalar::F32(-value)),
        (RuntimeUnaryOp::Neg, RuntimePureScalar::F64(value)) => Ok(RuntimePureScalar::F64(-value)),
        (
            RuntimeUnaryOp::Neg,
            value @ (RuntimePureScalar::U8(_)
            | RuntimePureScalar::U16(_)
            | RuntimePureScalar::U32(_)
            | RuntimePureScalar::U64(_)
            | RuntimePureScalar::U128(_)
            | RuntimePureScalar::USize(_)),
        ) => Err(RuntimeEvalError::UnsupportedUnary {
            op: op.as_label(),
            value: value.label(),
        }),
        (op, value) => Err(RuntimeEvalError::UnsupportedUnary {
            op: op.as_label(),
            value: value.label(),
        }),
    }
}

fn evaluate_scalar_binary(
    lhs: RuntimePureScalar,
    op: RuntimeBinaryOp,
    rhs: RuntimePureScalar,
) -> Result<RuntimePureScalar, RuntimeEvalError> {
    match op {
        RuntimeBinaryOp::Eq => Ok(RuntimePureScalar::Bool(lhs == rhs)),
        RuntimeBinaryOp::Ne => Ok(RuntimePureScalar::Bool(lhs != rhs)),
        RuntimeBinaryOp::And => match (lhs, rhs) {
            (RuntimePureScalar::Bool(lhs), RuntimePureScalar::Bool(rhs)) => {
                Ok(RuntimePureScalar::Bool(lhs && rhs))
            }
            (lhs, rhs) => Err(RuntimeEvalError::UnsupportedBinary {
                op: op.as_label(),
                lhs: lhs.label(),
                rhs: rhs.label(),
            }),
        },
        RuntimeBinaryOp::Or => match (lhs, rhs) {
            (RuntimePureScalar::Bool(lhs), RuntimePureScalar::Bool(rhs)) => {
                Ok(RuntimePureScalar::Bool(lhs || rhs))
            }
            (lhs, rhs) => Err(RuntimeEvalError::UnsupportedBinary {
                op: op.as_label(),
                lhs: lhs.label(),
                rhs: rhs.label(),
            }),
        },
        RuntimeBinaryOp::Lt | RuntimeBinaryOp::Le | RuntimeBinaryOp::Gt | RuntimeBinaryOp::Ge => {
            evaluate_scalar_comparison(lhs, op, rhs)
        }
        RuntimeBinaryOp::Add
        | RuntimeBinaryOp::Sub
        | RuntimeBinaryOp::Mul
        | RuntimeBinaryOp::Div => evaluate_scalar_arithmetic(lhs, op, rhs),
    }
}

fn evaluate_scalar_comparison(
    lhs: RuntimePureScalar,
    op: RuntimeBinaryOp,
    rhs: RuntimePureScalar,
) -> Result<RuntimePureScalar, RuntimeEvalError> {
    match (lhs, rhs) {
        (RuntimePureScalar::I8(lhs), RuntimePureScalar::I8(rhs)) => Ok(RuntimePureScalar::Bool(
            compare_scalar_ordered(&lhs, op, &rhs),
        )),
        (RuntimePureScalar::I16(lhs), RuntimePureScalar::I16(rhs)) => Ok(RuntimePureScalar::Bool(
            compare_scalar_ordered(&lhs, op, &rhs),
        )),
        (RuntimePureScalar::I32(lhs), RuntimePureScalar::I32(rhs)) => Ok(RuntimePureScalar::Bool(
            compare_scalar_ordered(&lhs, op, &rhs),
        )),
        (RuntimePureScalar::I64(lhs), RuntimePureScalar::I64(rhs))
        | (RuntimePureScalar::ISize(lhs), RuntimePureScalar::ISize(rhs)) => Ok(
            RuntimePureScalar::Bool(compare_scalar_ordered(&lhs, op, &rhs)),
        ),
        (RuntimePureScalar::I128(lhs), RuntimePureScalar::I128(rhs)) => Ok(
            RuntimePureScalar::Bool(compare_scalar_ordered(&lhs, op, &rhs)),
        ),
        (RuntimePureScalar::U8(lhs), RuntimePureScalar::U8(rhs)) => Ok(RuntimePureScalar::Bool(
            compare_scalar_ordered(&lhs, op, &rhs),
        )),
        (RuntimePureScalar::U16(lhs), RuntimePureScalar::U16(rhs)) => Ok(RuntimePureScalar::Bool(
            compare_scalar_ordered(&lhs, op, &rhs),
        )),
        (RuntimePureScalar::U32(lhs), RuntimePureScalar::U32(rhs)) => Ok(RuntimePureScalar::Bool(
            compare_scalar_ordered(&lhs, op, &rhs),
        )),
        (RuntimePureScalar::U64(lhs), RuntimePureScalar::U64(rhs))
        | (RuntimePureScalar::USize(lhs), RuntimePureScalar::USize(rhs)) => Ok(
            RuntimePureScalar::Bool(compare_scalar_ordered(&lhs, op, &rhs)),
        ),
        (RuntimePureScalar::U128(lhs), RuntimePureScalar::U128(rhs)) => Ok(
            RuntimePureScalar::Bool(compare_scalar_ordered(&lhs, op, &rhs)),
        ),
        (RuntimePureScalar::F32(lhs), RuntimePureScalar::F32(rhs)) => Ok(RuntimePureScalar::Bool(
            compare_scalar_float(&lhs, op, &rhs),
        )),
        (RuntimePureScalar::F64(lhs), RuntimePureScalar::F64(rhs)) => Ok(RuntimePureScalar::Bool(
            compare_scalar_float(&lhs, op, &rhs),
        )),
        (lhs, rhs) => Err(RuntimeEvalError::UnsupportedBinary {
            op: op.as_label(),
            lhs: lhs.label(),
            rhs: rhs.label(),
        }),
    }
}

fn evaluate_scalar_arithmetic(
    lhs: RuntimePureScalar,
    op: RuntimeBinaryOp,
    rhs: RuntimePureScalar,
) -> Result<RuntimePureScalar, RuntimeEvalError> {
    match (lhs, rhs) {
        (RuntimePureScalar::I8(lhs), RuntimePureScalar::I8(rhs)) => {
            Ok(RuntimePureScalar::I8(evaluate_scalar_numeric(lhs, op, rhs)))
        }
        (RuntimePureScalar::I16(lhs), RuntimePureScalar::I16(rhs)) => Ok(RuntimePureScalar::I16(
            evaluate_scalar_numeric(lhs, op, rhs),
        )),
        (RuntimePureScalar::I32(lhs), RuntimePureScalar::I32(rhs)) => Ok(RuntimePureScalar::I32(
            evaluate_scalar_numeric(lhs, op, rhs),
        )),
        (RuntimePureScalar::I64(lhs), RuntimePureScalar::I64(rhs)) => Ok(RuntimePureScalar::I64(
            evaluate_scalar_numeric(lhs, op, rhs),
        )),
        (RuntimePureScalar::I128(lhs), RuntimePureScalar::I128(rhs)) => Ok(
            RuntimePureScalar::I128(evaluate_scalar_numeric(lhs, op, rhs)),
        ),
        (RuntimePureScalar::ISize(lhs), RuntimePureScalar::ISize(rhs)) => Ok(
            RuntimePureScalar::ISize(evaluate_scalar_numeric(lhs, op, rhs)),
        ),
        (RuntimePureScalar::U8(lhs), RuntimePureScalar::U8(rhs)) => {
            Ok(RuntimePureScalar::U8(evaluate_scalar_numeric(lhs, op, rhs)))
        }
        (RuntimePureScalar::U16(lhs), RuntimePureScalar::U16(rhs)) => Ok(RuntimePureScalar::U16(
            evaluate_scalar_numeric(lhs, op, rhs),
        )),
        (RuntimePureScalar::U32(lhs), RuntimePureScalar::U32(rhs)) => Ok(RuntimePureScalar::U32(
            evaluate_scalar_numeric(lhs, op, rhs),
        )),
        (RuntimePureScalar::U64(lhs), RuntimePureScalar::U64(rhs)) => Ok(RuntimePureScalar::U64(
            evaluate_scalar_numeric(lhs, op, rhs),
        )),
        (RuntimePureScalar::U128(lhs), RuntimePureScalar::U128(rhs)) => Ok(
            RuntimePureScalar::U128(evaluate_scalar_numeric(lhs, op, rhs)),
        ),
        (RuntimePureScalar::USize(lhs), RuntimePureScalar::USize(rhs)) => Ok(
            RuntimePureScalar::USize(evaluate_scalar_numeric(lhs, op, rhs)),
        ),
        (RuntimePureScalar::F32(lhs), RuntimePureScalar::F32(rhs)) => Ok(RuntimePureScalar::F32(
            evaluate_scalar_numeric(lhs, op, rhs),
        )),
        (RuntimePureScalar::F64(lhs), RuntimePureScalar::F64(rhs)) => Ok(RuntimePureScalar::F64(
            evaluate_scalar_numeric(lhs, op, rhs),
        )),
        (lhs, rhs) => Err(RuntimeEvalError::UnsupportedBinary {
            op: op.as_label(),
            lhs: lhs.label(),
            rhs: rhs.label(),
        }),
    }
}

fn compare_scalar_ordered<T: Ord>(lhs: &T, op: RuntimeBinaryOp, rhs: &T) -> bool {
    match op {
        RuntimeBinaryOp::Lt => lhs < rhs,
        RuntimeBinaryOp::Le => lhs <= rhs,
        RuntimeBinaryOp::Gt => lhs > rhs,
        RuntimeBinaryOp::Ge => lhs >= rhs,
        _ => unreachable!(),
    }
}

fn compare_scalar_float<T: PartialOrd>(lhs: &T, op: RuntimeBinaryOp, rhs: &T) -> bool {
    match op {
        RuntimeBinaryOp::Lt => lhs < rhs,
        RuntimeBinaryOp::Le => lhs <= rhs,
        RuntimeBinaryOp::Gt => lhs > rhs,
        RuntimeBinaryOp::Ge => lhs >= rhs,
        _ => unreachable!(),
    }
}

fn evaluate_scalar_numeric<T: crate::value::RuntimeDeterministicNumeric>(
    lhs: T,
    op: RuntimeBinaryOp,
    rhs: T,
) -> T {
    evaluate_numeric_op(lhs, op, rhs)
}

struct PureScalarEvaluator<'a, T> {
    input_locals: &'a [RuntimeLocalDeclarationId],
    args: &'a [T],
    locals: Vec<(RuntimeLocalDeclarationId, RuntimePureScalar)>,
}

impl<'a, T: RuntimePureScalarInteger> PureScalarEvaluator<'a, T> {
    fn new_exact(input_locals: &'a [RuntimeLocalDeclarationId], args: &'a [T]) -> Self {
        Self {
            input_locals,
            args,
            locals: Vec::new(),
        }
    }

    fn evaluate(&mut self, expr: &RuntimeExpr) -> Result<RuntimePureScalar, RuntimeEvalError> {
        match expr.kind() {
            RuntimeExprKind::Value(value) => runtime_value_as_scalar(value)
                .ok_or_else(|| RuntimeEvalError::ExpectedInt(runtime_value_label(value))),
            RuntimeExprKind::Local(local) => self
                .get(*local)
                .ok_or(RuntimeEvalError::UnknownLocal(*local)),
            RuntimeExprKind::Let {
                binding,
                expr,
                body,
            } => {
                let value = self.evaluate(expr)?;
                self.locals.push((*binding, value));
                let result = self.evaluate(body);
                self.locals.pop();
                result
            }
            RuntimeExprKind::Unary { op, expr } => evaluate_scalar_unary(*op, self.evaluate(expr)?),
            RuntimeExprKind::Binary { lhs, op, rhs } => {
                let lhs = self.evaluate(lhs)?;
                let rhs = self.evaluate(rhs)?;
                evaluate_scalar_binary(lhs, *op, rhs)
            }
            RuntimeExprKind::If {
                condition,
                then_expr,
                else_expr,
            } => {
                if self.evaluate_bool(condition)? {
                    self.evaluate(then_expr)
                } else {
                    self.evaluate(else_expr)
                }
            }
            _ => Err(RuntimeEvalError::UnsupportedPure {
                name: "scalar pure".to_owned(),
                reason: format!(
                    "expression with plan type {} is not in the exact scalar subset",
                    expr.ty()
                ),
            }),
        }
    }

    fn evaluate_bool(&mut self, expr: &RuntimeExpr) -> Result<bool, RuntimeEvalError> {
        match self.evaluate(expr)? {
            RuntimePureScalar::Bool(value) => Ok(value),
            value => Err(RuntimeEvalError::ExpectedBool(value.label())),
        }
    }

    fn get(&self, local: RuntimeLocalDeclarationId) -> Option<RuntimePureScalar> {
        self.locals
            .iter()
            .rev()
            .find_map(|(candidate, value)| (*candidate == local).then_some(*value))
            .or_else(|| {
                self.input_locals
                    .iter()
                    .zip(self.args.iter().copied())
                    .find_map(|(input_local, value)| {
                        (*input_local == local).then_some(value.into_pure_scalar())
                    })
            })
    }
}

impl<'a> PureEvaluator<'a> {
    fn new_ref(plan: &'a Arc<RuntimePlan>, bindings: &[RuntimeLocalBinding]) -> Self {
        let mut env = RuntimeEnv::default();
        env.bind_all_ref(bindings);
        Self {
            plan,
            env,
            stats: PureFunctionStats::default(),
        }
    }

    fn with_env(plan: &'a Arc<RuntimePlan>, env: RuntimeEnv) -> Self {
        Self {
            plan,
            env,
            stats: PureFunctionStats::default(),
        }
    }

    fn into_env(self) -> RuntimeEnv {
        self.env
    }

    fn evaluate_expr(&mut self, expr: &RuntimeExpr) -> Result<RuntimeValue, RuntimeEvalError> {
        self.stats.evaluated_exprs += 1;
        let value = match expr.kind() {
            RuntimeExprKind::Value(value) => Ok(value.clone()),
            RuntimeExprKind::Agent(agent) => self.evaluate_agent_expr(agent),
            RuntimeExprKind::Local(local) => self.evaluate_local(*local),
            RuntimeExprKind::EntityRef(target) => {
                Ok(RuntimeValue::EntityRef(target.runtime_label()))
            }
            RuntimeExprKind::Let {
                binding,
                expr,
                body,
            } => self.evaluate_let_expr(*binding, expr, body),
            RuntimeExprKind::Tuple(items) => self.evaluate_items(items, RuntimeValue::Tuple),
            RuntimeExprKind::BracketSeq(items) => {
                self.evaluate_items(items, runtime_sequence_values)
            }
            RuntimeExprKind::RepeatSeq { value, len } => self.evaluate_repeat_seq_expr(value, *len),
            RuntimeExprKind::Range {
                start,
                end,
                inclusive,
            } => self.evaluate_range_expr(start.as_deref(), end.as_deref(), *inclusive),
            RuntimeExprKind::NominalRecord(record) => {
                self.evaluate_nominal_record_expr(expr.ty(), record)
            }
            RuntimeExprKind::Variant { ordinal, payload } => {
                self.evaluate_variant_expr(expr.ty(), *ordinal, payload.as_deref())
            }
            RuntimeExprKind::Field { target, field } => self.evaluate_field_expr(target, *field),
            RuntimeExprKind::ProjectTuple { target, ordinal } => {
                self.evaluate_project_tuple_expr(target, *ordinal)
            }
            RuntimeExprKind::ProjectRecord { target, ordinal } => {
                self.evaluate_project_record_expr(target, *ordinal)
            }
            RuntimeExprKind::AssignNominalField {
                base,
                field,
                expr,
                body,
            } => self.evaluate_assign_field_expr(*base, *field, expr, body),
            RuntimeExprKind::Call { callee, args } => self.evaluate_call_expr(callee, args),
            RuntimeExprKind::Function(site) => self.evaluate_function_expr(*site),
            RuntimeExprKind::Apply { callee, args } => self.evaluate_apply_expr(callee, args),
            RuntimeExprKind::TraitCall { .. } => Self::unsupported_flow_runtime_expr(),
            RuntimeExprKind::PureCall { helper, args } => {
                self.evaluate_nested_pure_call(*helper, args)
            }
            RuntimeExprKind::Map {
                source,
                param,
                body,
            } => self.evaluate_map_expr(source, *param, body),
            RuntimeExprKind::Filter {
                source,
                param,
                body,
            } => self.evaluate_filter_expr(source, *param, body),
            RuntimeExprKind::Sum { source } => self.evaluate_sum_expr(source),
            RuntimeExprKind::Unary { op, expr } => {
                let value = self.evaluate_expr(expr)?;
                evaluate_unary(*op, value)
            }
            RuntimeExprKind::Binary { lhs, op, rhs } => {
                self.stats.evaluated_binary_ops += 1;
                let lhs = self.evaluate_expr(lhs)?;
                let rhs = self.evaluate_expr(rhs)?;
                evaluate_binary(lhs, *op, rhs)
            }
            RuntimeExprKind::If {
                condition,
                then_expr,
                else_expr,
            } => self.evaluate_if_expr(condition, then_expr, else_expr),
            RuntimeExprKind::IfLet {
                pattern,
                expr,
                guard,
                then_expr,
                else_expr,
            } => self.evaluate_if_let_expr(pattern, expr, guard.as_deref(), then_expr, else_expr),
            RuntimeExprKind::Match { scrutinee, arms } => self.evaluate_match_expr(scrutinee, arms),
            RuntimeExprKind::ReductionUnchanged { state } => {
                self.evaluate_reduction_unchanged(expr.ty(), state)
            }
        }?;
        if !self.plan.value_matches_type(expr.ty(), &value)? {
            return Err(RuntimeEvalError::InvalidExpressionType(expr.ty()));
        }
        Ok(value)
    }

    fn evaluate_items(
        &mut self,
        items: &[RuntimeExpr],
        collect: impl FnOnce(Vec<RuntimeValue>) -> RuntimeValue,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        items
            .iter()
            .map(|item| self.evaluate_expr(item))
            .collect::<Result<Vec<_>, _>>()
            .map(collect)
    }

    fn evaluate_local(
        &self,
        local: RuntimeLocalDeclarationId,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.env
            .get(local)
            .cloned()
            .ok_or(RuntimeEvalError::UnknownLocal(local))
    }

    fn evaluate_agent_expr(
        &mut self,
        agent: &RuntimeAgentExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let mut operands = Vec::new();
        if let Some(choice) = agent.choice() {
            operands.push(RuntimeValue::EntityRef(choice.as_str().to_owned()));
        }
        for operand in agent.operands() {
            operands.push(self.evaluate_expr(operand)?);
        }
        RuntimeAgentValue::try_construct(agent.constructor(), operands)
            .map(RuntimeValue::Agent)
            .map_err(|error| RuntimeEvalError::AgentConstruction(error.to_string()))
    }

    fn evaluate_nominal_record_expr(
        &mut self,
        ty: crate::runtime_id::RuntimePlanTypeId,
        record: &RuntimeNominalRecordExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let plan = Arc::clone(self.plan);
        let declaration = plan
            .type_table()
            .get(ty)
            .ok_or(RuntimeEvalError::UnknownPlanType(ty))?;
        let RuntimePlanTypeProjection::ProjectNominal {
            nominal, layout, ..
        } = declaration.projection()
        else {
            return Err(RuntimeEvalError::InvalidExpressionType(ty));
        };
        let domain = plan
            .nominal_record_domains()
            .get(ty)
            .ok_or(RuntimeEvalError::MissingNominalRecordDomain(ty))?;
        let mut fields = std::iter::repeat_with(|| None)
            .take(domain.fields().len())
            .collect::<Vec<_>>();
        for initializer in record.initializers() {
            let value = self.evaluate_expr(initializer.value())?;
            let ordinal = usize::try_from(initializer.field().zero_based())
                .map_err(|_| RuntimeEvalError::InvalidExpressionType(ty))?;
            let field = domain
                .fields()
                .get(ordinal)
                .ok_or(RuntimeEvalError::InvalidExpressionType(ty))?;
            if !plan.value_matches_type(field.ty(), &value)? {
                return Err(RuntimeEvalError::InvalidExpressionType(
                    initializer.value().ty(),
                ));
            }
            if fields[ordinal].replace(value).is_some() {
                return Err(RuntimeEvalError::InvalidExpressionType(ty));
            }
        }
        let fields = fields
            .into_iter()
            .enumerate()
            .map(|(ordinal, field)| {
                let field_id =
                    crate::value::RuntimeRecordFieldId::from_accepted_zero_based(ordinal)
                        .map_err(|_| RuntimeEvalError::InvalidExpressionType(ty))?;
                field.ok_or(RuntimeEvalError::MissingRecordInitializer {
                    ty,
                    field: field_id,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RuntimeValue::NominalRecord(
            crate::value::RuntimeNominalRecordValue::new(nominal.clone(), *layout, fields),
        ))
    }

    fn evaluate_variant_expr(
        &mut self,
        ty: crate::runtime_id::RuntimePlanTypeId,
        ordinal: u32,
        payload: Option<&RuntimeExpr>,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let plan = Arc::clone(self.plan);
        let declaration = plan
            .type_table()
            .get(ty)
            .ok_or(RuntimeEvalError::UnknownPlanType(ty))?;
        let (owner, name, payload_ty) = match declaration.projection() {
            RuntimePlanTypeProjection::Option(item) => match ordinal {
                0 => (
                    RuntimeVariantIdentity::Option,
                    "Some".to_owned(),
                    Some(*item),
                ),
                1 => (RuntimeVariantIdentity::Option, "None".to_owned(), None),
                _ => return Err(RuntimeEvalError::UnknownVariantCase { ty, ordinal }),
            },
            RuntimePlanTypeProjection::Result { value, error } => match ordinal {
                0 => (
                    RuntimeVariantIdentity::Result,
                    "Ok".to_owned(),
                    Some(*value),
                ),
                1 => (
                    RuntimeVariantIdentity::Result,
                    "Err".to_owned(),
                    Some(*error),
                ),
                _ => return Err(RuntimeEvalError::UnknownVariantCase { ty, ordinal }),
            },
            RuntimePlanTypeProjection::ProjectNominal { .. }
            | RuntimePlanTypeProjection::Opaque { .. } => {
                let domain = plan
                    .variant_domains()
                    .get(ty)
                    .ok_or(RuntimeEvalError::MissingVariantDomain(ty))?;
                let case = domain
                    .case(ordinal)
                    .ok_or(RuntimeEvalError::UnknownVariantCase { ty, ordinal })?;
                (
                    RuntimeVariantIdentity::Nominal {
                        nominal: domain.nominal().clone(),
                        semantic_identity: declaration.semantic_identity(),
                    },
                    case.name().to_owned(),
                    case.payload(),
                )
            }
            _ => return Err(RuntimeEvalError::InvalidExpressionType(ty)),
        };
        let payload = payload.map(|expr| self.evaluate_expr(expr)).transpose()?;
        match (payload_ty, payload.as_ref()) {
            (Some(expected), Some(value)) if plan.value_matches_type(expected, value)? => {}
            (None, None) => {}
            _ => return Err(RuntimeEvalError::InvalidExpressionType(ty)),
        }
        Ok(RuntimeValue::Variant {
            owner,
            ordinal,
            name,
            payload: payload.map(Box::new),
        })
    }

    fn evaluate_reduction_unchanged(
        &mut self,
        ty: crate::runtime_id::RuntimePlanTypeId,
        state: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let plan = Arc::clone(self.plan);
        let declaration = plan
            .type_table()
            .get(ty)
            .ok_or(RuntimeEvalError::UnknownPlanType(ty))?;
        let RuntimePlanTypeProjection::Opaque {
            producer,
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: crate::value::RuntimeOpaqueValueClass::Plain,
            persistence: crate::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
            arguments,
        } = declaration.projection()
        else {
            return Err(RuntimeEvalError::InvalidExpressionType(ty));
        };
        let [state_ty] = arguments.as_ref() else {
            return Err(RuntimeEvalError::InvalidExpressionType(ty));
        };
        let state_ty = *state_ty;
        let materialized_state_ty = match plan
            .type_table()
            .get(state.ty())
            .map(RuntimePlanTypeDeclaration::projection)
        {
            Some(RuntimePlanTypeProjection::Reference(inner)) => *inner,
            _ => state.ty(),
        };
        if state_ty != materialized_state_ty {
            return Err(RuntimeEvalError::InvalidExpressionType(ty));
        }
        let producer = producer.clone();
        let semantic_identity = declaration.semantic_identity();
        let state = self.evaluate_expr(state)?;
        if !plan.value_matches_type(state_ty, &state)? {
            return Err(RuntimeEvalError::InvalidExpressionType(ty));
        }
        let owner = RuntimeOpaqueTypeOwner::exact(producer, semantic_identity);
        RuntimeReductionValue::try_unchanged(owner, state)
            .map(RuntimeValue::Reduction)
            .map_err(|_| RuntimeEvalError::InvalidExpressionType(ty))
    }

    fn evaluate_assign_field_expr(
        &mut self,
        base: RuntimeLocalDeclarationId,
        field: crate::value::RuntimeRecordFieldId,
        expr: &RuntimeExpr,
        body: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(expr)?;
        self.env
            .set_record_field(base, field, value)
            .map_err(|target| RuntimeEvalError::InvalidFieldAssignment {
                field: field.zero_based().to_string(),
                value: runtime_value_label(&target),
            })?;
        self.evaluate_expr(body)
    }

    fn evaluate_nested_pure_call(
        &mut self,
        helper_id: RuntimePureHelperId,
        args: &[RuntimeCallArgument],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let values = self.evaluate_call_args(args)?;
        let helper = resolve_validated_pure_helper(self.plan, helper_id)?;
        let bindings = prepare_helper_bindings(self.plan, helper, values)?;
        let expr = helper.expr.clone();
        self.with_temp_bindings(bindings, |this| this.evaluate_expr(&expr))
    }

    fn evaluate_repeat_seq_expr(
        &mut self,
        value: &RuntimeExpr,
        len: usize,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if let RuntimeExprKind::Value(value) = value.kind() {
            return Ok(runtime_sequence_values(vec![value.clone(); len]));
        }
        (0..len)
            .map(|_| self.evaluate_expr(value))
            .collect::<Result<Vec<_>, _>>()
            .map(runtime_sequence_values)
    }

    fn evaluate_if_expr(
        &mut self,
        condition: &RuntimeExpr,
        then_expr: &RuntimeExpr,
        else_expr: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if self.evaluate_bool(condition)? {
            self.evaluate_expr(then_expr)
        } else {
            self.evaluate_expr(else_expr)
        }
    }

    fn evaluate_if_let_expr(
        &mut self,
        pattern: &RuntimePattern,
        expr: &RuntimeExpr,
        guard: Option<&RuntimeExpr>,
        then_expr: &RuntimeExpr,
        else_expr: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(expr)?;
        let Some(bindings) = match_runtime_pattern(self.plan, pattern, &value)? else {
            return self.evaluate_expr(else_expr);
        };
        let guard_matched = if let Some(guard) = guard {
            self.with_temp_bindings_ref(&bindings, |this| this.evaluate_bool(guard))?
        } else {
            true
        };
        if guard_matched {
            self.with_temp_bindings(bindings, |this| this.evaluate_expr(then_expr))
        } else {
            self.evaluate_expr(else_expr)
        }
    }

    fn evaluate_match_expr(
        &mut self,
        scrutinee: &RuntimeExpr,
        arms: &[RuntimeExprMatchArm],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(scrutinee)?;
        for arm in arms {
            let Some(bindings) = match_runtime_pattern(self.plan, arm.pattern(), &value)? else {
                continue;
            };
            if let Some(guard) = arm.guard()
                && !self.with_temp_bindings_ref(&bindings, |this| this.evaluate_bool(guard))?
            {
                continue;
            }
            return self.with_temp_bindings(bindings, |this| this.evaluate_expr(arm.value()));
        }
        Err(RuntimeEvalError::PatternMismatch(runtime_value_label(
            &value,
        )))
    }

    fn with_temp_bindings<T>(
        &mut self,
        bindings: Vec<RuntimeLocalBinding>,
        f: impl FnOnce(&mut Self) -> Result<T, RuntimeEvalError>,
    ) -> Result<T, RuntimeEvalError> {
        self.env.push_scope_with_capacity(bindings.len());
        self.env.bind_all(bindings);
        let result = f(self);
        self.env.pop_scope();
        result
    }

    fn with_temp_bindings_ref<T>(
        &mut self,
        bindings: &[RuntimeLocalBinding],
        f: impl FnOnce(&mut Self) -> Result<T, RuntimeEvalError>,
    ) -> Result<T, RuntimeEvalError> {
        self.env.push_scope_with_capacity(bindings.len());
        self.env.bind_all_ref(bindings);
        let result = f(self);
        self.env.pop_scope();
        result
    }

    fn unsupported_flow_runtime_expr() -> Result<RuntimeValue, RuntimeEvalError> {
        Err(RuntimeEvalError::UnsupportedPure {
            name: "trait method".to_owned(),
            reason: "trait dispatch and mutation require the flow runtime".to_owned(),
        })
    }

    fn evaluate_range_expr(
        &mut self,
        start: Option<&RuntimeExpr>,
        end: Option<&RuntimeExpr>,
        inclusive: bool,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let start = start.map(|expr| self.evaluate_expr(expr)).transpose()?;
        let end = end.map(|expr| self.evaluate_expr(expr)).transpose()?;
        crate::value::RuntimeRange::new(start, end, inclusive).map(RuntimeValue::Range)
    }

    fn evaluate_let_expr(
        &mut self,
        binding: RuntimeLocalDeclarationId,
        expr: &RuntimeExpr,
        body: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(expr)?;
        self.env.push_scope_with_capacity(1);
        self.env.set(binding, value);
        let result = self.evaluate_expr(body);
        self.env.pop_scope();
        result
    }

    fn evaluate_scalar_expr(
        &mut self,
        expr: &RuntimeExpr,
    ) -> Result<RuntimePureScalar, RuntimeEvalError> {
        self.stats.evaluated_exprs += 1;
        match expr.kind() {
            RuntimeExprKind::Value(RuntimeValue::Bool(value)) => {
                Ok(RuntimePureScalar::Bool(*value))
            }
            RuntimeExprKind::Value(RuntimeValue::Int(value)) => Ok(runtime_int_as_scalar(*value)),
            RuntimeExprKind::Value(RuntimeValue::UInt(value)) => Ok(runtime_uint_as_scalar(*value)),
            RuntimeExprKind::Value(RuntimeValue::F32(value)) => Ok(RuntimePureScalar::F32(*value)),
            RuntimeExprKind::Value(RuntimeValue::F64(value)) => Ok(RuntimePureScalar::F64(*value)),
            RuntimeExprKind::Local(local) => match self.env.get(*local) {
                Some(value) => runtime_value_as_scalar(value)
                    .ok_or_else(|| RuntimeEvalError::ExpectedInt(runtime_value_label(value))),
                None => Err(RuntimeEvalError::UnknownLocal(*local)),
            },
            RuntimeExprKind::Let {
                binding,
                expr,
                body,
            } => {
                let value = self.evaluate_scalar_expr(expr)?.into_runtime_value();
                self.env.push_scope_with_capacity(1);
                self.env.set(*binding, value);
                let result = self.evaluate_scalar_expr(body);
                self.env.pop_scope();
                result
            }
            RuntimeExprKind::Unary { op, expr } => {
                let value = self.evaluate_scalar_expr(expr)?;
                evaluate_scalar_unary(*op, value)
            }
            RuntimeExprKind::Binary { lhs, op, rhs } => {
                self.stats.evaluated_binary_ops += 1;
                let lhs = self.evaluate_scalar_expr(lhs)?;
                let rhs = self.evaluate_scalar_expr(rhs)?;
                evaluate_scalar_binary(lhs, *op, rhs)
            }
            RuntimeExprKind::If {
                condition,
                then_expr,
                else_expr,
            } => {
                if self.evaluate_scalar_bool(condition)? {
                    self.evaluate_scalar_expr(then_expr)
                } else {
                    self.evaluate_scalar_expr(else_expr)
                }
            }
            _ => self.evaluate_expr(expr).and_then(runtime_value_into_scalar),
        }
    }

    fn evaluate_scalar_bool(&mut self, expr: &RuntimeExpr) -> Result<bool, RuntimeEvalError> {
        match self.evaluate_scalar_expr(expr)? {
            RuntimePureScalar::Bool(value) => Ok(value),
            value => Err(RuntimeEvalError::ExpectedBool(value.label())),
        }
    }

    fn evaluate_map_expr(
        &mut self,
        source: &RuntimeExpr,
        param: RuntimeLocalDeclarationId,
        body: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let iterator = match RuntimeIterator::from_value(self.evaluate_expr(source)?) {
            Ok(iterator) => iterator,
            Err(value) => {
                return Err(RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(
                    &value,
                )));
            }
        };
        iterator
            .collect::<Vec<_>>()
            .into_iter()
            .map(|item| {
                self.env.push_scope_with_capacity(1);
                self.env.set(param, item);
                let result = self.evaluate_expr(body);
                self.env.pop_scope();
                result
            })
            .collect::<Result<Vec<_>, _>>()
            .map(runtime_sequence_values)
    }

    fn evaluate_filter_expr(
        &mut self,
        source: &RuntimeExpr,
        param: RuntimeLocalDeclarationId,
        body: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let iterator = match RuntimeIterator::from_value(self.evaluate_expr(source)?) {
            Ok(iterator) => iterator,
            Err(value) => {
                return Err(RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(
                    &value,
                )));
            }
        };
        let mut filtered = Vec::new();
        for item in iterator.collect::<Vec<_>>() {
            self.env.push_scope_with_capacity(1);
            self.env.set(param, item.clone());
            let keep = self.evaluate_bool(body);
            self.env.pop_scope();
            if keep? {
                filtered.push(item);
            }
        }
        Ok(runtime_sequence_values(filtered))
    }

    fn evaluate_sum_expr(
        &mut self,
        source: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if let RuntimeExprKind::Local(local) = source.kind()
            && let Some(sum) = self.evaluate_i64_local_sequence_sum(*local)?
        {
            return Ok(RuntimeValue::i64(sum));
        }
        let value = self.evaluate_expr(source)?;
        if let RuntimeValue::Seq(seq) = &value
            && let Some(sum) = seq.sum_as_i64()
        {
            return Ok(RuntimeValue::i64(sum));
        }
        let iterator = match RuntimeIterator::from_value(value) {
            Ok(iterator) => iterator,
            Err(value) => {
                return Err(RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(
                    &value,
                )));
            }
        };
        let items = iterator.collect::<Vec<_>>();
        sum_i64_sequence_ref(&items).map(RuntimeValue::i64)
    }

    fn evaluate_i64_local_sequence_sum(
        &self,
        local: RuntimeLocalDeclarationId,
    ) -> Result<Option<i64>, RuntimeEvalError> {
        let Some(value) = self.env.get(local) else {
            return Ok(None);
        };
        match value {
            RuntimeValue::Seq(seq) => match seq {
                RuntimeSeq::Values(items) => sum_i64_sequence_ref(items).map(Some),
                RuntimeSeq::Dense(items) => Ok(items.sum_as_i64()),
                RuntimeSeq::TupleColumns(_) | RuntimeSeq::RecordColumns(_) => Ok(None),
            },
            RuntimeValue::Tuple(items) => sum_i64_sequence_ref(items).map(Some),
            _ => Ok(None),
        }
    }

    fn evaluate_call_expr(
        &mut self,
        callee: &RuntimeCallTarget,
        args: &[RuntimeCallArgument],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.stats.evaluated_calls += 1;
        let args = self.evaluate_call_args(args)?;
        if let Some(intrinsic) = callee.as_intrinsic()
            && let Some(value) = evaluate_std_float_intrinsic(intrinsic, &args)?
        {
            return Ok(value);
        }
        if let Some(intrinsic) = callee.as_intrinsic()
            && let Some(value) = evaluate_string_intrinsic(intrinsic, &args)?
        {
            return Ok(value);
        }
        if let Some(intrinsic) = callee.as_intrinsic()
            && let Some(value) = evaluate_index_intrinsic(intrinsic, &args)?
        {
            return Ok(value);
        }
        match (callee.as_intrinsic(), args.as_slice()) {
            (Some(RuntimeIntrinsic::Add), [RuntimeValue::Int(lhs), RuntimeValue::Int(rhs)]) => {
                evaluate_binary(
                    RuntimeValue::Int(*lhs),
                    RuntimeBinaryOp::Add,
                    RuntimeValue::Int(*rhs),
                )
            }
            (Some(RuntimeIntrinsic::CoreRange), _) => evaluate_core_range_intrinsic(&args),
            (Some(RuntimeIntrinsic::CoreIterCollect), [value]) => {
                evaluate_core_iter_collect_intrinsic(value.clone())
            }
            (Some(RuntimeIntrinsic::CoreIterIntoIter), [value, evidence]) => {
                evaluate_core_iter_into_iter_intrinsic(value.clone(), evidence)
            }
            (Some(RuntimeIntrinsic::CoreIterNext), [value]) => {
                evaluate_core_iter_next_intrinsic(value.clone())
            }
            (Some(RuntimeIntrinsic::CoreOptionIsSome), [value]) => {
                evaluate_core_option_is_some_intrinsic(value)
            }
            (Some(RuntimeIntrinsic::CoreOptionUnwrap), [value]) => {
                evaluate_core_option_unwrap_intrinsic(value.clone())
            }
            (
                Some(RuntimeIntrinsic::MathMatmulF32),
                [RuntimeValue::MatrixF32(lhs), RuntimeValue::MatrixF32(rhs)],
            ) => lhs
                .matmul_scalar(rhs)
                .map(RuntimeValue::matrix_f32)
                .map_err(|error| RuntimeEvalError::UnsupportedPure {
                    name: callee.as_label().to_owned(),
                    reason: error.to_string(),
                }),
            (
                Some(RuntimeIntrinsic::MathMatrixAddF32),
                [RuntimeValue::MatrixF32(lhs), RuntimeValue::MatrixF32(rhs)],
            ) => lhs
                .add_scalar(rhs)
                .map(RuntimeValue::matrix_f32)
                .map_err(|error| RuntimeEvalError::UnsupportedPure {
                    name: callee.as_label().to_owned(),
                    reason: error.to_string(),
                }),
            (
                Some(RuntimeIntrinsic::MathTensorAddF32),
                [RuntimeValue::TensorF32(lhs), RuntimeValue::TensorF32(rhs)],
            ) => lhs
                .add_scalar(rhs)
                .map(RuntimeValue::tensor_f32)
                .map_err(|error| RuntimeEvalError::UnsupportedPure {
                    name: callee.as_label().to_owned(),
                    reason: error.to_string(),
                }),
            (
                Some(RuntimeIntrinsic::MathMatmulF64),
                [RuntimeValue::MatrixF64(lhs), RuntimeValue::MatrixF64(rhs)],
            ) => lhs
                .matmul_scalar(rhs)
                .map(RuntimeValue::matrix_f64)
                .map_err(|error| RuntimeEvalError::UnsupportedPure {
                    name: callee.as_label().to_owned(),
                    reason: error.to_string(),
                }),
            (
                Some(RuntimeIntrinsic::MathMatrixAddF64),
                [RuntimeValue::MatrixF64(lhs), RuntimeValue::MatrixF64(rhs)],
            ) => lhs
                .add_scalar(rhs)
                .map(RuntimeValue::matrix_f64)
                .map_err(|error| RuntimeEvalError::UnsupportedPure {
                    name: callee.as_label().to_owned(),
                    reason: error.to_string(),
                }),
            (
                Some(RuntimeIntrinsic::MathTensorAddF64),
                [RuntimeValue::TensorF64(lhs), RuntimeValue::TensorF64(rhs)],
            ) => lhs
                .add_scalar(rhs)
                .map(RuntimeValue::tensor_f64)
                .map_err(|error| RuntimeEvalError::UnsupportedPure {
                    name: callee.as_label().to_owned(),
                    reason: error.to_string(),
                }),
            _ => Err(RuntimeEvalError::UnsupportedPure {
                name: callee.as_label().to_owned(),
                reason: "call is not registered as a pure helper".to_owned(),
            }),
        }
    }

    fn evaluate_field_expr(
        &mut self,
        target: &RuntimeExpr,
        field: RuntimeFieldProjection,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(target)?;
        match (field, value) {
            (RuntimeFieldProjection::Nominal(field), RuntimeValue::NominalRecord(record)) => record
                .field(field)
                .cloned()
                .ok_or_else(|| RuntimeEvalError::MissingField {
                    field: field.zero_based().to_string(),
                    value: "nominal record".to_owned(),
                }),
            (RuntimeFieldProjection::EntityReference(field), RuntimeValue::EntityRef(id)) => {
                Ok(Self::entity_ref_field(&id, field))
            }
            (RuntimeFieldProjection::Agent(field), RuntimeValue::Agent(value)) => value
                .project_typed_field(field)
                .ok_or_else(|| RuntimeEvalError::MissingField {
                    field: field.as_label().to_owned(),
                    value: value.label().to_owned(),
                }),
            (RuntimeFieldProjection::Agent(field), RuntimeValue::Record(fields))
                if field.permits_protocol_record() =>
            {
                fields
                    .iter()
                    .find(|entry| entry.name() == field.as_label())
                    .map(|entry| entry.value().clone())
                    .ok_or_else(|| RuntimeEvalError::MissingField {
                        field: field.as_label().to_owned(),
                        value: "Agent protocol record".to_owned(),
                    })
            }
            (RuntimeFieldProjection::Progress(field), RuntimeValue::Progress(progress)) => {
                Ok(match field {
                    crate::value::RuntimeProgressField::Ratio => {
                        RuntimeValue::F32(progress.ratio())
                    }
                    crate::value::RuntimeProgressField::Label => progress
                        .label()
                        .map_or_else(RuntimeValue::option_none, |label| {
                            RuntimeValue::option_some(RuntimeValue::String(label.to_owned()))
                        }),
                })
            }
            (field, value) => Err(RuntimeEvalError::MissingField {
                field: field.label(),
                value: runtime_value_label(&value),
            }),
        }
    }

    fn entity_ref_field(id: &str, field: RuntimeEntityReferenceField) -> RuntimeValue {
        RuntimeValue::String(match field {
            RuntimeEntityReferenceField::Id => id.to_owned(),
            RuntimeEntityReferenceField::Family => Self::entity_ref_family(id).to_owned(),
            RuntimeEntityReferenceField::Name => Self::entity_ref_name(id).to_owned(),
        })
    }

    fn entity_ref_family(id: &str) -> &str {
        id.split_once('.').map_or(id, |(family, _)| family)
    }

    fn entity_ref_name(id: &str) -> &str {
        id.split_once('.').map_or("", |(_, name)| name)
    }

    fn evaluate_project_tuple_expr(
        &mut self,
        target: &RuntimeExpr,
        ordinal: usize,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(target)?;
        match value {
            RuntimeValue::Tuple(items) => {
                items
                    .into_iter()
                    .nth(ordinal)
                    .ok_or_else(|| RuntimeEvalError::MissingField {
                        field: ordinal.to_string(),
                        value: "tuple".to_owned(),
                    })
            }
            RuntimeValue::Seq(RuntimeSeq::TupleColumns(columns)) => columns
                .column(ordinal)
                .cloned()
                .map(RuntimeValue::Seq)
                .ok_or_else(|| RuntimeEvalError::MissingField {
                    field: ordinal.to_string(),
                    value: "tuple sequence".to_owned(),
                }),
            value => Err(RuntimeEvalError::MissingField {
                field: ordinal.to_string(),
                value: runtime_value_label(&value),
            }),
        }
    }

    fn evaluate_project_record_expr(
        &mut self,
        target: &RuntimeExpr,
        ordinal: usize,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(target)?;
        match value {
            RuntimeValue::Record(fields) => fields.into_iter().nth(ordinal).map_or_else(
                || {
                    Err(RuntimeEvalError::MissingField {
                        field: ordinal.to_string(),
                        value: "record".to_owned(),
                    })
                },
                |field| Ok(field.into_value()),
            ),
            RuntimeValue::Seq(RuntimeSeq::RecordColumns(records)) => records
                .field_by_ordinal(ordinal)
                .cloned()
                .map(RuntimeValue::Seq)
                .ok_or_else(|| RuntimeEvalError::MissingField {
                    field: ordinal.to_string(),
                    value: "record sequence".to_owned(),
                }),
            value => Err(RuntimeEvalError::MissingField {
                field: ordinal.to_string(),
                value: runtime_value_label(&value),
            }),
        }
    }

    fn evaluate_function_expr(
        &self,
        site: RuntimeFunctionSiteId,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let declaration = self
            .plan
            .function_sites()
            .get(site)
            .ok_or(RuntimeFunctionApplyError::UnknownStructuredSite { site })?;
        let captures = declaration
            .captures()
            .iter()
            .map(|&local| {
                self.env
                    .get(local)
                    .cloned()
                    .ok_or(RuntimeEvalError::UnboundStructuredCapture { site, local })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RuntimeValue::Function(RuntimeFunctionValue::capture_site(
            Arc::clone(self.plan),
            site,
            captures,
        )?))
    }

    fn evaluate_apply_expr(
        &mut self,
        callee: &RuntimeExpr,
        args: &[RuntimeCallArgument],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let callee = self.evaluate_expr(callee)?;
        let args = self.evaluate_call_args(args)?;
        match callee {
            RuntimeValue::Function(function) => self.apply_runtime_function(&function, &args),
            value => Err(RuntimeEvalError::ExpectedFunction(runtime_value_label(
                &value,
            ))),
        }
    }

    fn apply_runtime_function(
        &mut self,
        function: &RuntimeFunctionValue,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let Some(closure) = function.as_structured() else {
            return Err(Self::structured_awbc_function_error());
        };
        if !Arc::ptr_eq(self.plan, closure.plan()) {
            return Err(RuntimeEvalError::ForeignStructuredFunction {
                site: closure.site(),
            });
        }
        let remaining = function.remaining_arity()?;
        if args.len() < remaining {
            return Ok(RuntimeValue::Function(function.try_bind_prefix(args)?));
        }
        let (call_args, remaining_args) = args.split_at(remaining);
        let value = self.call_runtime_function(function, call_args)?;
        if remaining_args.is_empty() {
            return Ok(value);
        }
        match value {
            RuntimeValue::Function(next) => self.apply_runtime_function(&next, remaining_args),
            _ => Err(RuntimeEvalError::FunctionArgumentCount {
                expected: remaining,
                found: args.len(),
            }),
        }
    }

    fn call_runtime_function(
        &mut self,
        function: &RuntimeFunctionValue,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let Some(closure) = function.as_structured() else {
            return Err(Self::structured_awbc_function_error());
        };
        if !Arc::ptr_eq(self.plan, closure.plan()) {
            return Err(RuntimeEvalError::ForeignStructuredFunction {
                site: closure.site(),
            });
        }
        let remaining = function.remaining_arity()?;
        if args.len() != remaining {
            return Err(RuntimeEvalError::FunctionArgumentCount {
                expected: remaining,
                found: args.len(),
            });
        }
        function.validate_bind_prefix(args)?;
        let site = closure.site();
        let declaration = self
            .plan
            .function_sites()
            .get(site)
            .ok_or(RuntimeFunctionApplyError::UnknownStructuredSite { site })?;
        let capture_locals = declaration.captures().to_vec();
        let params = declaration.params().to_vec();
        let body = declaration.body().clone();
        self.env
            .push_scope_with_capacity(capture_locals.len() + params.len());
        for (&local, value) in capture_locals.iter().zip(closure.capture_values()) {
            self.env.set_ref(local, value);
        }
        let bound_count = closure.bound_args().len();
        for (&local, value) in params[..bound_count].iter().zip(closure.bound_args()) {
            self.env.set_ref(local, value);
        }
        for (&local, value) in params[bound_count..].iter().zip(args) {
            self.env.set_ref(local, value);
        }
        let result = self.evaluate_expr(&body);
        self.env.pop_scope();
        result
    }

    fn structured_awbc_function_error() -> RuntimeEvalError {
        RuntimeEvalError::UnsupportedPure {
            name: "awbc.function".to_owned(),
            reason: "structured pure evaluation cannot evaluate an AWBC function body".to_owned(),
        }
    }

    fn evaluate_bool(&mut self, expr: &RuntimeExpr) -> Result<bool, RuntimeEvalError> {
        match self.evaluate_expr(expr)? {
            RuntimeValue::Bool(value) => Ok(value),
            value => Err(RuntimeEvalError::ExpectedBool(runtime_value_label(&value))),
        }
    }

    fn evaluate_call_args(
        &mut self,
        args: &[RuntimeCallArgument],
    ) -> Result<Vec<RuntimeValue>, RuntimeEvalError> {
        let mut values = Vec::with_capacity(args.len());
        for argument in args {
            let value = self.evaluate_expr(argument.value())?;
            match argument.mode() {
                RuntimeCallArgumentMode::Value => values.push(value),
                RuntimeCallArgumentMode::Spread => {
                    values.extend(spread_runtime_values(value)?);
                }
            }
        }
        Ok(values)
    }
}

fn spread_runtime_values(value: RuntimeValue) -> Result<Vec<RuntimeValue>, RuntimeEvalError> {
    match runtime_value_into_sequence_values(value) {
        Ok(items) => Ok(items),
        Err(value) => Err(RuntimeEvalError::InvalidSpread(runtime_value_label(&value))),
    }
}
