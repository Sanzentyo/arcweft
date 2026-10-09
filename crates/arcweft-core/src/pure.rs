mod callable;
use crate::math::{DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64};
use arcweft_interaction_model::dialogue::{
    CharacterDialogueOperation, CharacterDialoguePatchField,
};

#[cfg(test)]
mod function_application_tests;
use crate::pattern::{
    RuntimeBuiltinVariantCaseIdentity, RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner,
    RuntimePattern,
};
use crate::plan::{
    RuntimePlan, RuntimePlanTypeDeclaration, RuntimePlanTypeProjection, RuntimePureHelper,
    RuntimePureHelperId, RuntimePureInputType, RuntimePureOutputType, RuntimeReceiverMode,
    RuntimeTraitMethodId,
};
use crate::runtime_id::{RuntimeFunctionSiteId, RuntimeLocalDeclarationId};
use crate::scope::RuntimeScopeIdentity;
use crate::step::RuntimePureCallStats;
use crate::value::{
    RuntimeAgentExpr, RuntimeAgentValue, RuntimeBinaryOp, RuntimeCallArgument,
    RuntimeCallArgumentMode, RuntimeCallTarget, RuntimeCallableValue, RuntimeEnv, RuntimeEvalError,
    RuntimeExactInteger, RuntimeExpr, RuntimeExprKind, RuntimeExprMatchArm, RuntimeFieldProjection,
    RuntimeFunctionApplyError, RuntimeISizeValue, RuntimeIntrinsic, RuntimeIterator,
    RuntimeLocalBinding, RuntimeLocalRead, RuntimeLocalReadMode, RuntimeNominalRecordExpr,
    RuntimeReductionValue, RuntimeSeq, RuntimeSignedIntWidth, RuntimeStandardMapFamily,
    RuntimeStandardMapOperandOrder, RuntimeUSizeValue, RuntimeUnaryOp, RuntimeUnsignedIntWidth,
    RuntimeValue, evaluate_binary, evaluate_capacity_intrinsic, evaluate_core_iterator_intrinsic,
    evaluate_core_range_intrinsic, evaluate_index_intrinsic, evaluate_numeric_op,
    evaluate_std_float_intrinsic, evaluate_string_intrinsic, evaluate_unary,
    runtime_sequence_values, runtime_value_into_sequence_values, runtime_value_label,
    sum_i64_sequence_ref,
};
use crate::{
    entry::RuntimeSchemaLimits,
    pattern::RuntimeSemanticTypeId,
    program_types::{RuntimeProgramTypeError, RuntimeProgramTypes},
    task::RuntimeProgramOwner,
};
use std::sync::Arc;

mod aot;
mod function;
mod program;
mod runtime_backend;
pub use function::{
    RuntimePureFunctionId, RuntimePureFunctionInputIter, RuntimePureFunctionInputRef,
    RuntimePureFunctionInputs, RuntimePureFunctionRef, RuntimePureFunctionView,
};
pub use program::evaluate_pure_program_with_backend;

/// Request for evaluating a deterministic pure helper expression.
#[derive(Clone, Debug, PartialEq)]
pub struct PureFunctionRequest {
    plan: Arc<RuntimePlan>,
    helper: RuntimePureFunctionId,
    bindings: Box<[RuntimeLocalBinding]>,
    format_context: crate::value::RuntimeFormatContext,
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
#[derive(Clone, Debug)]
pub struct RuntimeExternalCallContext {
    state: RuntimeExternalCallContextState,
}

#[derive(Clone, Debug)]
enum RuntimeExternalCallContextState {
    Program {
        owner: RuntimeProgramOwner,
        argument_types: Box<[RuntimeSemanticTypeId]>,
        result_type: RuntimeSemanticTypeId,
        limits: RuntimeSchemaLimits,
    },
    Unbound,
}

impl RuntimeExternalCallContext {
    /// Binds an external call to the exact selected program and its semantic
    /// argument and result types.
    pub fn for_program(
        owner: RuntimeProgramOwner,
        argument_types: impl IntoIterator<Item = RuntimeSemanticTypeId>,
        result_type: RuntimeSemanticTypeId,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimeProgramTypeError> {
        let argument_types = argument_types.into_iter().collect::<Box<[_]>>();
        let types = match &owner {
            RuntimeProgramOwner::Plan(plan) => RuntimeProgramTypes::Plan(plan),
            RuntimeProgramOwner::Awbc(program) => RuntimeProgramTypes::Awbc(program),
        };
        argument_types
            .iter()
            .copied()
            .chain(std::iter::once(result_type))
            .try_for_each(|semantic_type| types.require_type(semantic_type))?;
        Ok(Self {
            state: RuntimeExternalCallContextState::Program {
                owner,
                argument_types,
                result_type,
                limits,
            },
        })
    }

    /// Creates the explicit context for a raw call that has no selected
    /// program or semantic signature.
    #[must_use]
    pub const fn unbound() -> Self {
        Self {
            state: RuntimeExternalCallContextState::Unbound,
        }
    }

    /// Returns the exact selected program owner for a typed call.
    #[must_use]
    pub const fn program_owner(&self) -> Option<&RuntimeProgramOwner> {
        match &self.state {
            RuntimeExternalCallContextState::Program { owner, .. } => Some(owner),
            RuntimeExternalCallContextState::Unbound => None,
        }
    }

    /// Returns the typed arguments for a program-bound call.
    #[must_use]
    pub fn argument_types(&self) -> Option<&[RuntimeSemanticTypeId]> {
        match &self.state {
            RuntimeExternalCallContextState::Program { argument_types, .. } => Some(argument_types),
            RuntimeExternalCallContextState::Unbound => None,
        }
    }

    /// Returns the typed result for a program-bound call.
    #[must_use]
    pub const fn result_type(&self) -> Option<RuntimeSemanticTypeId> {
        match &self.state {
            RuntimeExternalCallContextState::Program { result_type, .. } => Some(*result_type),
            RuntimeExternalCallContextState::Unbound => None,
        }
    }

    /// Returns the schema limits selected for a program-bound call.
    #[must_use]
    pub const fn limits(&self) -> Option<RuntimeSchemaLimits> {
        match &self.state {
            RuntimeExternalCallContextState::Program { limits, .. } => Some(*limits),
            RuntimeExternalCallContextState::Unbound => None,
        }
    }

    /// Returns whether this call has a selected program and semantic types.
    #[must_use]
    pub const fn is_program_bound(&self) -> bool {
        matches!(&self.state, RuntimeExternalCallContextState::Program { .. })
    }
}

pub trait RuntimeExternalCallBackend {
    fn call_external(
        &mut self,
        context: &RuntimeExternalCallContext,
        callee: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>>;

    /// Executes the closed CharacterDialogue operation against the backend's
    /// accepted generation. The Core evaluator has already evaluated every
    /// authored operand exactly once in source order.
    fn produce_character_dialogue(
        &mut self,
        _owner: &RuntimeProgramOwner,
        _operation: CharacterDialogueOperation,
        _target: RuntimeValue,
        _fields: &[CharacterDialoguePatchField<RuntimeValue>],
        _result_type: RuntimeSemanticTypeId,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        Err(RuntimeEvalError::CharacterDialogueProducerUnavailable)
    }
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

/// Runtime-facing backend for deterministic pure helper calls.
pub trait RuntimePureCallBackend {
    /// Records one accepted top-level compact-AWBC pure-program invocation.
    ///
    /// Backends may project this into their existing non-semantic statistics;
    /// it must not influence helper selection or evaluation results.
    fn record_awbc_pure_program_call(&mut self);

    fn call_i8_slice(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        args: &[i8],
    ) -> Result<Option<i8>, RuntimeEvalError>;

    fn call_i8_flat_batch(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[i8],
        arity: usize,
        out: &mut [i8],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i8_flat_batch_sum(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[i8],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_i16_slice(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        args: &[i16],
    ) -> Result<Option<i16>, RuntimeEvalError>;

    fn call_i16_flat_batch(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[i16],
        arity: usize,
        out: &mut [i16],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i16_flat_batch_sum(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[i16],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_i128_flat_batch(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[i128],
        arity: usize,
        out: &mut [i128],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i128_flat_batch_sum(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[i128],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_i32(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        args: RuntimeI32Args,
    ) -> Result<Option<i32>, RuntimeEvalError>;

    fn call_i32_slice(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        args: &[i32],
    ) -> Result<Option<i32>, RuntimeEvalError>;

    fn call_i32_flat_batch(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[i32],
        arity: usize,
        out: &mut [i32],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i32_flat_batch_sum(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[i32],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_u32_slice(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        args: &[u32],
    ) -> Result<Option<u32>, RuntimeEvalError>;

    fn call_u8_slice(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        args: &[u8],
    ) -> Result<Option<u8>, RuntimeEvalError>;

    fn call_u8_flat_batch(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[u8],
        arity: usize,
        out: &mut [u8],
    ) -> Result<(), RuntimeEvalError>;

    fn call_u8_flat_batch_sum(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[u8],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_u16_slice(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        args: &[u16],
    ) -> Result<Option<u16>, RuntimeEvalError>;

    fn call_u16_flat_batch(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[u16],
        arity: usize,
        out: &mut [u16],
    ) -> Result<(), RuntimeEvalError>;

    fn call_u16_flat_batch_sum(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[u16],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_u128_flat_batch(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[u128],
        arity: usize,
        out: &mut [u128],
    ) -> Result<(), RuntimeEvalError>;

    fn call_u128_flat_batch_sum(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[u128],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_u32_flat_batch(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[u32],
        arity: usize,
        out: &mut [u32],
    ) -> Result<(), RuntimeEvalError>;

    fn call_u32_flat_batch_sum(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[u32],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_u64_slice(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        args: &[u64],
    ) -> Result<Option<u64>, RuntimeEvalError>;

    fn call_u64_flat_batch(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[u64],
        arity: usize,
        out: &mut [u64],
    ) -> Result<(), RuntimeEvalError>;

    fn call_u64_flat_batch_sum(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[u64],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_exact_int_flat_batch_sum<T: RuntimePureScalarInteger>(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[T],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_exact_int_slice<T: RuntimePureScalarInteger>(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        args: &[T],
    ) -> Result<Option<T>, RuntimeEvalError>;

    fn call_exact_int_flat_batch<T: RuntimePureScalarInteger>(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[T],
        arity: usize,
        out: &mut [T],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i64(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        args: RuntimeI64Args,
    ) -> Result<Option<i64>, RuntimeEvalError>;

    fn call_i64_slice(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        args: &[i64],
    ) -> Result<Option<i64>, RuntimeEvalError>;

    fn call_i64_batch(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        rows: &[RuntimeI64Args],
        out: &mut [i64],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i64_flat_batch(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[i64],
        arity: usize,
        out: &mut [i64],
    ) -> Result<(), RuntimeEvalError>;

    fn call_i64_flat_batch_sum(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[i64],
        arity: usize,
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_i64_repeated_flat_batch_sum(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        row: &[i64],
        rows: usize,
    ) -> Result<i64, RuntimeEvalError>;

    fn call_f32_slice(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        args: &[f32],
    ) -> Result<Option<f32>, RuntimeEvalError>;

    fn call_f32_flat_batch(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[f32],
        arity: usize,
        out: &mut [f32],
    ) -> Result<(), RuntimeEvalError>;

    fn call_f64_slice(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        args: &[f64],
    ) -> Result<Option<f64>, RuntimeEvalError>;

    fn call_f64_flat_batch(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        flat_inputs: &[f64],
        arity: usize,
        out: &mut [f64],
    ) -> Result<(), RuntimeEvalError>;

    fn call_values(
        &mut self,
        helper: RuntimePureFunctionRef<'_>,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, RuntimeEvalError>;

    /// Optionally evaluates a canonical compact-AWBC helper directly.
    ///
    /// Returning `None` selects the verified helper frame on the current AWBC
    /// fiber. Returning `Some` preserves backend selection and
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
    format_context: crate::value::RuntimeFormatContext,
}

/// AOT backend for deterministic pure helpers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AotPureFunctionBackend;

/// VM runtime backend used when no external pure accelerator is provided.
#[derive(Clone, Debug, PartialEq)]
pub struct VmRuntimePureCallBackend<E = NoRuntimeExternalCalls> {
    stats: RuntimePureCallStats,
    scratch: VmPureFunctionScratch,
    external: E,
}

impl Default for VmRuntimePureCallBackend<NoRuntimeExternalCalls> {
    fn default() -> Self {
        Self {
            stats: RuntimePureCallStats::default(),
            scratch: VmPureFunctionScratch::default(),
            external: NoRuntimeExternalCalls,
        }
    }
}

/// Explicit absence of host-provided pure external callable implementations.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NoRuntimeExternalCalls;

impl RuntimeExternalCallBackend for NoRuntimeExternalCalls {
    fn call_external(
        &mut self,
        _context: &RuntimeExternalCallContext,
        _callee: &RuntimeCallTarget,
        _args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        None
    }
}

impl<E> VmRuntimePureCallBackend<E> {
    /// Selects the session locale for helpers called through this backend.
    pub fn set_format_context(&mut self, context: crate::value::RuntimeFormatContext) {
        self.scratch.set_format_context(context);
    }

    /// Installs the caller-owned implementation of registered pure Rust calls.
    /// Core retains no I/O service or globally registered default provider.
    pub fn with_external_calls<X: RuntimeExternalCallBackend>(
        self,
        external: X,
    ) -> VmRuntimePureCallBackend<X> {
        VmRuntimePureCallBackend {
            stats: self.stats,
            scratch: self.scratch,
            external,
        }
    }
}

/// Compiled AOT plan for the current deterministic `i64` pure-helper subset.
#[derive(Clone, Debug, PartialEq)]
pub struct AotPureI64Plan {
    plan: Arc<RuntimePlan>,
    helper: RuntimePureFunctionId,
    expr: aot::AotI64Expr,
    initial_slots: Vec<i64>,
    input_slots: Vec<usize>,
    slot_count: usize,
}

/// Compiled AOT plan for exact-width scalar helpers that are not widened to `i64`.
#[derive(Clone, Debug, PartialEq)]
pub struct AotPureScalarPlan {
    plan: Arc<RuntimePlan>,
    helper: RuntimePureFunctionId,
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
        helper: impl Into<RuntimePureFunctionId>,
        args: impl IntoIterator<Item = RuntimeValue>,
    ) -> Result<Self, RuntimeEvalError> {
        let helper = helper.into();
        let args = args.into_iter().collect::<Vec<_>>();
        let declaration = RuntimePureFunctionRef::resolve(&plan, helper)?;
        validate_function_arguments(declaration, &args)?;
        let mut bindings = Vec::with_capacity(args.len());
        for (input, value) in declaration.inputs.iter().zip(args) {
            let local = input.local();
            bindings.push(RuntimeLocalBinding { local, value });
        }
        Ok(Self {
            plan,
            helper,
            bindings: bindings.into_boxed_slice(),
            format_context: crate::value::RuntimeFormatContext::default(),
        })
    }

    /// Binds a host-selected locale to this deterministic pure request.
    #[must_use]
    pub fn with_format_context(mut self, context: crate::value::RuntimeFormatContext) -> Self {
        self.format_context = context;
        self
    }

    #[must_use]
    pub fn plan(&self) -> &Arc<RuntimePlan> {
        &self.plan
    }

    pub fn function_ref(&self) -> Result<RuntimePureFunctionRef<'_>, RuntimeEvalError> {
        RuntimePureFunctionRef::resolve(&self.plan, self.helper)
    }

    #[must_use]
    pub const fn function_id(&self) -> RuntimePureFunctionId {
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

fn resolve_validated_pure_function(
    plan: &Arc<RuntimePlan>,
    function: impl Into<RuntimePureFunctionId>,
) -> Result<RuntimePureFunctionRef<'_>, RuntimeEvalError> {
    RuntimePureFunctionRef::resolve(plan, function)
}

fn validate_pure_helper_contract(
    plan: &RuntimePlan,
    helper: &RuntimePureHelper,
) -> Result<(), RuntimeEvalError> {
    for input in &helper.inputs {
        let local = input.local();
        let input_type = input.abi();
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
        let helper = request.function_ref()?;
        let mut evaluator = PureEvaluator::new_ref(&request.plan, &request.bindings)
            .with_format_context(request.format_context.clone());
        let value = match helper.id() {
            RuntimePureFunctionId::Recipe(_) => evaluator.evaluate_expr(helper.expr)?,
            RuntimePureFunctionId::Function(site) => {
                let (captures, arguments) = helper.split_function_arguments(
                    request.bindings.iter().map(|binding| binding.value.clone()),
                )?;
                evaluator.evaluate_function_site(site, captures, arguments)?
            }
        };
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
        helper: impl Into<RuntimePureFunctionId>,
        args: RuntimeI32Args,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.evaluate_i32_slice(plan, helper, args.as_slice())
    }

    pub fn evaluate_i32_slice(
        &self,
        plan: &Arc<RuntimePlan>,
        helper: impl Into<RuntimePureFunctionId>,
        args: &[i32],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let mut scratch = VmPureFunctionScratch::default();
        scratch.evaluate_i32_slice(plan, helper, args)
    }

    pub fn evaluate_i64_args(
        &self,
        plan: &Arc<RuntimePlan>,
        helper: impl Into<RuntimePureFunctionId>,
        args: RuntimeI64Args,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.evaluate_i64_slice(plan, helper, args.as_slice())
    }

    pub fn evaluate_i64_slice(
        &self,
        plan: &Arc<RuntimePlan>,
        helper: impl Into<RuntimePureFunctionId>,
        args: &[i64],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let mut scratch = VmPureFunctionScratch::default();
        scratch.evaluate_i64_slice(plan, helper, args)
    }

    pub fn evaluate_f32_slice(
        &self,
        plan: &Arc<RuntimePlan>,
        helper: impl Into<RuntimePureFunctionId>,
        args: &[f32],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let mut scratch = VmPureFunctionScratch::default();
        scratch.evaluate_f32_slice(plan, helper, args)
    }

    pub fn evaluate_f64_slice(
        &self,
        plan: &Arc<RuntimePlan>,
        helper: impl Into<RuntimePureFunctionId>,
        args: &[f64],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let mut scratch = VmPureFunctionScratch::default();
        scratch.evaluate_f64_slice(plan, helper, args)
    }
}

impl VmPureFunctionScratch {
    /// Selects the session locale for subsequent pure helper evaluations.
    pub fn set_format_context(&mut self, context: crate::value::RuntimeFormatContext) {
        self.format_context = context;
    }

    #[must_use]
    pub const fn format_context(&self) -> &crate::value::RuntimeFormatContext {
        &self.format_context
    }

    /// Evaluates a capture-free structured function-site body for an Entry
    /// root callable. Function-site input rows are the sole ABI authority:
    /// each logical parameter is installed at its synthetic input local and
    /// then passed through the same checked pattern binder used by ordinary
    /// structured calls. Executable sites are rejected here because a root
    /// callable evaluator is deliberately synchronous; they must enter the
    /// flow runtime instead of being treated as pure helpers.
    pub fn evaluate_function_site(
        &mut self,
        plan: &Arc<RuntimePlan>,
        site: RuntimeFunctionSiteId,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let declaration =
            plan.function_sites()
                .get(site)
                .ok_or(RuntimeEvalError::FunctionApply(
                    RuntimeFunctionApplyError::UnknownStructuredSite { site },
                ))?;
        if declaration.capture_inputs().next().is_some() {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: "structured.function".to_owned(),
                reason: "an Entry function site must be capture-free".to_owned(),
            });
        }
        self.env.replace_scopes_with_bindings([Vec::new()]);
        let mut evaluator = PureEvaluator::with_env(plan, std::mem::take(&mut self.env))
            .with_format_context(self.format_context.clone());
        let result = evaluator.evaluate_function_site(site, Vec::new(), args);
        self.env = evaluator.into_env();
        result
    }

    pub fn evaluate_i32_args(
        &mut self,
        plan: &Arc<RuntimePlan>,
        helper: impl Into<RuntimePureFunctionId>,
        args: RuntimeI32Args,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.evaluate_i32_slice(plan, helper, args.as_slice())
    }

    pub fn evaluate_i32_slice(
        &mut self,
        plan: &Arc<RuntimePlan>,
        helper: impl Into<RuntimePureFunctionId>,
        args: &[i32],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.evaluate_values(
            plan,
            helper,
            args.iter().copied().map(RuntimeValue::i32).collect(),
        )
    }

    pub fn evaluate_exact_int_slice<T: RuntimePureScalarInteger>(
        &mut self,
        plan: &Arc<RuntimePlan>,
        helper: impl Into<RuntimePureFunctionId>,
        args: &[T],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let helper = resolve_validated_pure_function(plan, helper)?;
        let values = args
            .iter()
            .copied()
            .map(RuntimePureScalarInteger::into_pure_scalar)
            .map(RuntimePureScalar::into_runtime_value)
            .collect::<Vec<_>>();
        validate_function_arguments(helper, &values)?;
        if helper.scalar_eval_supported {
            let mut evaluator = PureScalarEvaluator::new_exact(helper.inputs, args);
            let result = evaluator
                .evaluate(helper.expr)
                .map(RuntimePureScalar::into_runtime_value);
            if !matches!(result, Err(RuntimeEvalError::UnsupportedPure { .. })) {
                return validate_helper_result(helper, result);
            }
        }
        self.evaluate_values(plan, helper.id(), values)
    }

    pub fn evaluate_i64_args(
        &mut self,
        plan: &Arc<RuntimePlan>,
        helper: impl Into<RuntimePureFunctionId>,
        args: RuntimeI64Args,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.evaluate_i64_slice(plan, helper, args.as_slice())
    }

    pub fn evaluate_i64_slice(
        &mut self,
        plan: &Arc<RuntimePlan>,
        helper: impl Into<RuntimePureFunctionId>,
        args: &[i64],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.evaluate_values(
            plan,
            helper,
            args.iter().copied().map(RuntimeValue::i64).collect(),
        )
    }

    pub fn evaluate_f32_slice(
        &mut self,
        plan: &Arc<RuntimePlan>,
        helper: impl Into<RuntimePureFunctionId>,
        args: &[f32],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.evaluate_values(
            plan,
            helper,
            args.iter().copied().map(RuntimeValue::F32).collect(),
        )
    }

    pub fn evaluate_f64_slice(
        &mut self,
        plan: &Arc<RuntimePlan>,
        helper: impl Into<RuntimePureFunctionId>,
        args: &[f64],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.evaluate_values(
            plan,
            helper,
            args.iter().copied().map(RuntimeValue::F64).collect(),
        )
    }

    pub fn evaluate_values(
        &mut self,
        plan: &Arc<RuntimePlan>,
        helper: impl Into<RuntimePureFunctionId>,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let helper = resolve_validated_pure_function(plan, helper)?;
        if let RuntimePureFunctionId::Function(site) = helper.id() {
            let (captures, arguments) = helper.split_function_arguments(args)?;
            plan.validate_function_site_inputs(site, &captures, &arguments)?;
            self.env.replace_scopes_with_bindings([Vec::new()]);
            let mut evaluator = PureEvaluator::with_env(plan, std::mem::take(&mut self.env))
                .with_format_context(self.format_context.clone());
            let result = evaluator.evaluate_function_site(site, captures, arguments);
            self.env = evaluator.into_env();
            return result;
        }
        let bindings = prepare_helper_bindings(helper, args)?;
        self.env.replace_scopes_with_bindings([bindings]);
        let mut evaluator = PureEvaluator::with_env(plan, std::mem::take(&mut self.env))
            .with_format_context(self.format_context.clone());
        let result = validate_helper_result(
            helper,
            if helper.scalar_eval_supported {
                evaluator
                    .evaluate_scalar_expr(helper.expr)
                    .map(RuntimePureScalar::into_runtime_value)
            } else {
                evaluator.evaluate_expr(helper.expr)
            },
        );
        self.env = evaluator.into_env();
        result
    }
}

fn prepare_helper_bindings(
    helper: RuntimePureFunctionRef<'_>,
    values: impl IntoIterator<Item = RuntimeValue>,
) -> Result<Vec<RuntimeLocalBinding>, RuntimeEvalError> {
    let values = values.into_iter().collect::<Vec<_>>();
    validate_function_arguments(helper, &values)?;
    Ok(helper
        .inputs
        .iter()
        .map(RuntimePureFunctionInputRef::local)
        .zip(values)
        .map(|(local, value)| RuntimeLocalBinding { local, value })
        .collect())
}

fn validate_function_arguments(
    function: RuntimePureFunctionRef<'_>,
    values: &[RuntimeValue],
) -> Result<(), RuntimeEvalError> {
    if values.len() != function.inputs.len() {
        return Err(RuntimeEvalError::TooManyPureArgs {
            helper: function.name.to_owned(),
            max: function.inputs.len(),
            found: values.len(),
        });
    }
    if let RuntimePureFunctionId::Function(site) = function.id() {
        for (input, value) in function.inputs.iter().zip(values) {
            if !value.ownership().permits_copy() {
                return Err(RuntimeEvalError::AffineLocalCopy(input.input_local()));
            }
        }
        let (captures, arguments) = function.split_function_arguments(values.iter().cloned())?;
        function
            .plan()
            .validate_function_site_inputs(site, &captures, &arguments)?;
        return Ok(());
    }
    function
        .inputs
        .iter()
        .zip(values)
        .try_for_each(|(input, value)| {
            let local = input.input_local();
            let declaration = function
                .plan()
                .local_declarations()
                .get(local)
                .ok_or(RuntimeEvalError::UnknownLocal(local))?;
            if !function
                .plan()
                .value_matches_type(declaration.ty(), value)?
            {
                return Err(RuntimeEvalError::InvalidExpressionType(declaration.ty()));
            }
            Ok(())
        })
}

fn validate_helper_result(
    helper: RuntimePureFunctionRef<'_>,
    result: Result<RuntimeValue, RuntimeEvalError>,
) -> Result<RuntimeValue, RuntimeEvalError> {
    let value = result?;
    if !helper.plan().value_matches_type(helper.expr.ty(), &value)? {
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
    external: Option<&'a mut dyn RuntimeExternalCallBackend>,
    format_context: crate::value::RuntimeFormatContext,
    evaluating_pure_trait_call: bool,
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
            evaluate_scalar_numeric(lhs, op, rhs).map(RuntimePureScalar::I8)
        }
        (RuntimePureScalar::I16(lhs), RuntimePureScalar::I16(rhs)) => {
            evaluate_scalar_numeric(lhs, op, rhs).map(RuntimePureScalar::I16)
        }
        (RuntimePureScalar::I32(lhs), RuntimePureScalar::I32(rhs)) => {
            evaluate_scalar_numeric(lhs, op, rhs).map(RuntimePureScalar::I32)
        }
        (RuntimePureScalar::I64(lhs), RuntimePureScalar::I64(rhs)) => {
            evaluate_scalar_numeric(lhs, op, rhs).map(RuntimePureScalar::I64)
        }
        (RuntimePureScalar::I128(lhs), RuntimePureScalar::I128(rhs)) => {
            evaluate_scalar_numeric(lhs, op, rhs).map(RuntimePureScalar::I128)
        }
        (RuntimePureScalar::ISize(lhs), RuntimePureScalar::ISize(rhs)) => {
            evaluate_scalar_numeric(lhs, op, rhs).map(RuntimePureScalar::ISize)
        }
        (RuntimePureScalar::U8(lhs), RuntimePureScalar::U8(rhs)) => {
            evaluate_scalar_numeric(lhs, op, rhs).map(RuntimePureScalar::U8)
        }
        (RuntimePureScalar::U16(lhs), RuntimePureScalar::U16(rhs)) => {
            evaluate_scalar_numeric(lhs, op, rhs).map(RuntimePureScalar::U16)
        }
        (RuntimePureScalar::U32(lhs), RuntimePureScalar::U32(rhs)) => {
            evaluate_scalar_numeric(lhs, op, rhs).map(RuntimePureScalar::U32)
        }
        (RuntimePureScalar::U64(lhs), RuntimePureScalar::U64(rhs)) => {
            evaluate_scalar_numeric(lhs, op, rhs).map(RuntimePureScalar::U64)
        }
        (RuntimePureScalar::U128(lhs), RuntimePureScalar::U128(rhs)) => {
            evaluate_scalar_numeric(lhs, op, rhs).map(RuntimePureScalar::U128)
        }
        (RuntimePureScalar::USize(lhs), RuntimePureScalar::USize(rhs)) => {
            evaluate_scalar_numeric(lhs, op, rhs).map(RuntimePureScalar::USize)
        }
        (RuntimePureScalar::F32(lhs), RuntimePureScalar::F32(rhs)) => {
            evaluate_scalar_numeric(lhs, op, rhs).map(RuntimePureScalar::F32)
        }
        (RuntimePureScalar::F64(lhs), RuntimePureScalar::F64(rhs)) => {
            evaluate_scalar_numeric(lhs, op, rhs).map(RuntimePureScalar::F64)
        }
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
) -> Result<T, RuntimeEvalError> {
    evaluate_numeric_op(lhs, op, rhs)
}

struct PureScalarEvaluator<'a, T> {
    inputs: RuntimePureFunctionInputs<'a>,
    args: &'a [T],
    locals: Vec<(RuntimeLocalDeclarationId, RuntimePureScalar)>,
    scopes: Vec<PureScalarScopeFrame>,
}

struct PureScalarScopeFrame {
    _identity: RuntimeScopeIdentity,
    local_start: usize,
}

impl<'a, T: RuntimePureScalarInteger> PureScalarEvaluator<'a, T> {
    fn new_exact(inputs: RuntimePureFunctionInputs<'a>, args: &'a [T]) -> Self {
        Self {
            inputs,
            args,
            locals: Vec::new(),
            scopes: Vec::new(),
        }
    }

    fn push_scope(&mut self, identity: RuntimeScopeIdentity) {
        self.scopes.push(PureScalarScopeFrame {
            _identity: identity,
            local_start: self.locals.len(),
        });
    }

    fn pop_scope(&mut self) {
        if let Some(scope) = self.scopes.pop() {
            self.locals.truncate(scope.local_start);
        }
    }

    fn evaluate(&mut self, expr: &RuntimeExpr) -> Result<RuntimePureScalar, RuntimeEvalError> {
        match expr.kind() {
            RuntimeExprKind::Value(value) => runtime_value_as_scalar(value)
                .ok_or_else(|| RuntimeEvalError::ExpectedInt(runtime_value_label(value))),
            RuntimeExprKind::Local(read) => {
                if read.mode() != RuntimeLocalReadMode::Copy {
                    return Err(RuntimeEvalError::UnsupportedPure {
                        name: "scalar accelerator".to_owned(),
                        reason: "a moved local requires the general pure evaluator".to_owned(),
                    });
                }
                self.get(read.local())
                    .ok_or(RuntimeEvalError::UnknownLocal(read.local()))
            }
            RuntimeExprKind::Let {
                binding,
                expr,
                body,
            } => {
                let value = self.evaluate(expr)?;
                self.push_scope(RuntimeScopeIdentity::Anonymous);
                self.locals.push((*binding, value));
                let result = self.evaluate(body);
                self.pop_scope();
                result
            }
            RuntimeExprKind::Scope { identity, body } => {
                self.push_scope(identity.clone());
                let result = self.evaluate(body);
                self.pop_scope();
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
                self.inputs.iter().zip(self.args.iter().copied()).find_map(
                    |(input_local, value)| {
                        (input_local.local() == local).then_some(value.into_pure_scalar())
                    },
                )
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
            external: None,
            format_context: crate::value::RuntimeFormatContext::default(),
            evaluating_pure_trait_call: false,
        }
    }

    fn with_env(plan: &'a Arc<RuntimePlan>, env: RuntimeEnv) -> Self {
        Self {
            plan,
            env,
            stats: PureFunctionStats::default(),
            external: None,
            format_context: crate::value::RuntimeFormatContext::default(),
            evaluating_pure_trait_call: false,
        }
    }

    fn with_format_context(mut self, context: crate::value::RuntimeFormatContext) -> Self {
        self.format_context = context;
        self
    }

    fn into_env(self) -> RuntimeEnv {
        self.env
    }

    fn evaluate_expr(&mut self, expr: &RuntimeExpr) -> Result<RuntimeValue, RuntimeEvalError> {
        self.stats.evaluated_exprs += 1;
        if self.evaluating_pure_trait_call {
            match expr.kind() {
                RuntimeExprKind::SequencePopFront { .. }
                | RuntimeExprKind::SequencePush { .. }
                | RuntimeExprKind::SequencePopBack { .. }
                | RuntimeExprKind::Assign { .. }
                | RuntimeExprKind::CharacterDialogue { .. } => {
                    return Self::unsupported_pure_trait_operation(
                        "mutation or dialogue construction requires the flow runtime",
                    );
                }
                RuntimeExprKind::Call { callee, .. } if callee.as_intrinsic().is_none() => {
                    return Self::unsupported_pure_trait_operation(
                        "host calls require the flow runtime",
                    );
                }
                _ => {}
            }
        }
        let value = match expr.kind() {
            RuntimeExprKind::Value(value) => value
                .ownership()
                .permits_copy()
                .then(|| value.clone())
                .ok_or(RuntimeEvalError::AffineLiteralCopy),
            RuntimeExprKind::Agent(agent) => self.evaluate_agent_expr(agent),
            RuntimeExprKind::Local(read) => self.evaluate_local(read),
            RuntimeExprKind::SequencePopFront { place } => {
                self.env.pop_sequence_front(place).map(|value| {
                    value.map_or_else(RuntimeValue::option_none, RuntimeValue::option_some)
                })
            }
            RuntimeExprKind::SequencePush { place, value } => {
                let value = self.evaluate_expr(value)?;
                self.env.push_vector_item(place, value)?;
                Ok(RuntimeValue::Unit)
            }
            RuntimeExprKind::SequencePopBack { place } => {
                self.env.pop_vector_item(place).map(|value| {
                    value.map_or_else(RuntimeValue::option_none, RuntimeValue::option_some)
                })
            }
            RuntimeExprKind::EntityRef(target) => Ok(RuntimeValue::EntityRef(target.clone())),
            RuntimeExprKind::Let {
                binding,
                expr,
                body,
            } => self.evaluate_let_expr(*binding, expr, body),
            RuntimeExprKind::Scope { identity, body } => {
                self.env.push_scope_with_identity(identity.clone());
                let result = self.evaluate_expr(body);
                self.env.pop_scope();
                result
            }
            RuntimeExprKind::DialogueContent {
                template,
                values,
                effects,
            } => self.evaluate_dialogue_content_expr(*template, values, effects),
            RuntimeExprKind::FormatContent {
                template,
                attempt,
                operands,
                project_method,
                project_option,
            } => self.evaluate_format_content_expr(
                *template,
                *attempt,
                operands,
                *project_method,
                *project_option,
            ),
            RuntimeExprKind::CharacterDialogue {
                operation,
                target,
                fields,
            } => self.evaluate_character_dialogue_expr(expr.ty(), *operation, target, fields),
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
            RuntimeExprKind::Field { target, field } => self.evaluate_field_expr(target, field),
            RuntimeExprKind::ProjectTuple { target, ordinal } => {
                self.evaluate_project_tuple_expr(target, *ordinal)
            }
            RuntimeExprKind::ProjectRecord { target, ordinal } => {
                self.evaluate_project_record_expr(target, *ordinal)
            }
            RuntimeExprKind::Assign { place, expr, body } => {
                self.evaluate_assign_expr(place, expr, body)
            }
            RuntimeExprKind::Call { callee, args }
                if callee.as_intrinsic().is_none() && self.external.is_some() =>
            {
                self.evaluate_external_call_expr(callee, args, expr.ty())
            }
            RuntimeExprKind::Call { callee, args } => self.evaluate_call_expr(callee, args),
            RuntimeExprKind::MakeCallable { state, captures } => {
                self.evaluate_callable_expr(*state, captures)
            }
            RuntimeExprKind::SpecializeCallable {
                value,
                specialization,
            } => self.evaluate_specialize_callable_expr(value, *specialization),
            RuntimeExprKind::ApplyGroup { callee, args } => self.evaluate_apply_expr(callee, args),
            RuntimeExprKind::TraitCall {
                callable,
                receiver,
                receiver_mode,
                args,
            } => {
                self.evaluate_trait_call_expr(*callable, *receiver_mode, receiver, args, expr.ty())
            }
            RuntimeExprKind::PureCall { helper, args } => {
                self.evaluate_nested_pure_call(*helper, args)
            }
            RuntimeExprKind::StandardMap {
                family,
                order,
                mapping,
                source,
            } => self.evaluate_standard_map_expr(*family, *order, mapping, source),
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
        if !self
            .env
            .value_matches_type(self.plan.as_ref(), expr.ty(), &value)?
        {
            return Err(RuntimeEvalError::InvalidExpressionType(expr.ty()));
        }
        Ok(value)
    }

    fn evaluate_dialogue_content_expr(
        &mut self,
        template: crate::runtime_id::RuntimeDialogueContentTemplateId,
        values: &[RuntimeExpr],
        effects: &[crate::value::RuntimeDialogueContentEffectBindingExpr],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if self
            .plan
            .dialogue_content_templates()
            .get(template)
            .is_none()
        {
            return Err(RuntimeEvalError::MissingDialogueTemplateManifest { template });
        }
        let evaluated = values
            .iter()
            .map(|expression| self.evaluate_expr(expression))
            .collect::<Result<Vec<_>, RuntimeEvalError>>()?;
        let manifest = self
            .plan
            .dialogue_content_templates()
            .get(template)
            .expect("dialogue content template was checked before evaluation");
        if evaluated.len() != manifest.slots().len() {
            return Err(RuntimeEvalError::DialogueContentBindingCount {
                expected: manifest.slots().len(),
                actual: evaluated.len(),
            });
        }
        let evaluated = values
            .iter()
            .zip(evaluated)
            .zip(manifest.slots())
            .map(|((_, value), slot)| {
                Ok(crate::plan::RuntimeDialogueValueBinding {
                    slot: slot.slot(),
                    role: slot.role(),
                    value,
                })
            })
            .collect::<Result<Vec<_>, RuntimeEvalError>>()?;
        if effects.len() != manifest.effects().len() {
            return Err(RuntimeEvalError::DialogueContentConstruction(format!(
                "evaluated effect count {} does not match template count {}",
                effects.len(),
                manifest.effects().len()
            )));
        }
        let mut effect_bindings = Vec::with_capacity(effects.len());
        for (index, effect) in effects.iter().enumerate() {
            let expected = manifest.effects().get(index).ok_or_else(|| {
                RuntimeEvalError::DialogueContentConstruction(
                    "dialogue content effect site is absent".to_owned(),
                )
            })?;
            if effect.site != expected.site() {
                return Err(RuntimeEvalError::DialogueContentConstruction(
                    "dialogue content effect sites are not canonical".to_owned(),
                ));
            }
            let captures = effect
                .captures
                .iter()
                .map(|capture| self.evaluate_expr(capture))
                .collect::<Result<Vec<_>, RuntimeEvalError>>()?;
            let callback = RuntimeCallableValue::try_new(
                crate::task::RuntimeProgramOwner::Plan(Arc::clone(self.plan)),
                effect.state,
                captures,
            )?;
            if callback.remaining_arity()? != 0 {
                return Err(RuntimeEvalError::FunctionArgumentCount {
                    expected: 0,
                    found: callback.remaining_arity()?,
                });
            }
            effect_bindings.push(crate::value::RuntimeDialogueContentEffectBinding::new(
                effect.site,
                callback,
            ));
        }
        let artifact = self
            .plan
            .artifact()
            .ok_or(RuntimeEvalError::DialogueContentUnboundArtifact)?;
        crate::value::RuntimeDialogueContentValue::try_from_evaluated_bindings_with_effects(
            artifact,
            manifest,
            &evaluated,
            &effect_bindings,
        )
        .map(crate::value::RuntimeDialogueContentValue::into_runtime_value)
        .map_err(|error| RuntimeEvalError::DialogueContentConstruction(error.to_string()))
    }

    fn evaluate_format_content_expr(
        &mut self,
        template: crate::runtime_id::RuntimeDialogueContentTemplateId,
        attempt: Option<crate::runtime_id::RuntimeFormatAttemptId>,
        operands: &[crate::value::RuntimeFormatContentOperand],
        project_method: Option<RuntimeTraitMethodId>,
        project_option: bool,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if attempt.is_some() {
            return Err(RuntimeEvalError::DialogueContentConstruction(
                "flow-evaluated formatter attempt requires its owning flow frame".to_owned(),
            ));
        }
        let manifest = self
            .plan
            .dialogue_content_templates()
            .get(template)
            .cloned()
            .ok_or(RuntimeEvalError::MissingDialogueTemplateManifest { template })?;
        let [slot] = manifest.slots() else {
            return Err(RuntimeEvalError::DialogueContentConstruction(
                "fmt Content template must have one Formatted slot".to_owned(),
            ));
        };
        if slot.role() != crate::plan::RuntimeDialogueValueRole::Formatted
            || slot.semantic_type()
                != crate::value::RuntimeDialogueOpaqueRole::Content.semantic_identity()
            || !manifest.effects().is_empty()
        {
            return Err(RuntimeEvalError::DialogueContentConstruction(
                "fmt Content template must have one exact Formatted/Content slot and no effects"
                    .to_owned(),
            ));
        }
        let format_context = self.format_context.clone();
        let mut evaluated = Vec::with_capacity(operands.len());
        let mut first_recoverable = None;
        for operand in operands {
            let value = match self.evaluate_expr(operand.expression()) {
                Ok(value) => Some(value),
                Err(RuntimeEvalError::RecoverableExpression(failure)) => {
                    if first_recoverable.is_none() {
                        first_recoverable = Some(failure.to_string());
                    }
                    None
                }
                Err(error) => return Err(error),
            };
            evaluated.push((operand.parameter(), value));
        }
        let primary = operands
            .iter()
            .find(|operand| operand.parameter() == crate::value::RuntimeFmtParameterId::Value)
            .ok_or_else(|| {
                RuntimeEvalError::DialogueContentConstruction(
                    "fmt Content has no primary value expression".to_owned(),
                )
            })?;
        let primary_type = self
            .plan
            .type_table()
            .get(primary.expression().ty())
            .ok_or(RuntimeEvalError::InvalidExpressionType(
                primary.expression().ty(),
            ))?;
        let mut primary_kind = match primary_type.projection() {
            RuntimePlanTypeProjection::Option { item, .. } => {
                let item = self
                    .plan
                    .type_table()
                    .get(*item)
                    .ok_or(RuntimeEvalError::InvalidExpressionType(*item))?;
                crate::value::RuntimeFormatPrimaryKind::OptionScalar(item.semantic_identity())
            }
            _ if primary_type.semantic_identity()
                == crate::value::RuntimeDialogueOpaqueRole::Content.semantic_identity() =>
            {
                crate::value::RuntimeFormatPrimaryKind::Content
            }
            _ => crate::value::RuntimeFormatPrimaryKind::Scalar(primary_type.semantic_identity()),
        };
        if let Some(method_id) = project_method {
            primary_kind = if project_option {
                crate::value::RuntimeFormatPrimaryKind::OptionProjectContent
            } else {
                crate::value::RuntimeFormatPrimaryKind::ProjectContent
            };
            if first_recoverable.is_none() {
                let method = self
                    .plan
                    .trait_methods()
                    .get(method_id.0)
                    .filter(|method| method.id == method_id)
                    .cloned()
                    .ok_or(RuntimeEvalError::UnknownTraitMethod(method_id.0))?;
                let [receiver_input, context_input] = method.inputs.as_ref() else {
                    return Err(RuntimeEvalError::DialogueContentConstruction(
                        "project DisplayText method must have receiver and context".to_owned(),
                    ));
                };

                let receiver_local = receiver_input.local();
                let context_local = context_input.local();
                let receiver_ty = self
                    .plan
                    .local_declarations()
                    .get(receiver_local)
                    .ok_or(RuntimeEvalError::UnknownLocal(receiver_local))?
                    .ty();
                let context_ty = self
                    .plan
                    .local_declarations()
                    .get(context_local)
                    .ok_or(RuntimeEvalError::UnknownLocal(context_local))?
                    .ty();
                let context_layout = crate::value::project_display_layout(self.plan, context_ty)
                    .map_err(|error| {
                        RuntimeEvalError::DialogueContentConstruction(error.to_string())
                    })?;
                let result_ty = self
                    .plan
                    .type_table()
                    .get(method.body.ty())
                    .ok_or(RuntimeEvalError::InvalidExpressionType(method.body.ty()))?;
                let RuntimePlanTypeProjection::Result { error, .. } = result_ty.projection() else {
                    return Err(RuntimeEvalError::InvalidExpressionType(method.body.ty()));
                };
                let error_layout = crate::value::project_display_layout(self.plan, *error)
                    .map_err(|error| {
                        RuntimeEvalError::DialogueContentConstruction(error.to_string())
                    })?;
                let value_position = evaluated
                    .iter()
                    .position(|(parameter, _)| {
                        *parameter == crate::value::RuntimeFmtParameterId::Value
                    })
                    .ok_or_else(|| {
                        RuntimeEvalError::DialogueContentConstruction(
                            "project DisplayText has no primary value".to_owned(),
                        )
                    })?;
                let receiver = evaluated[value_position].1.clone().ok_or_else(|| {
                    RuntimeEvalError::DialogueContentConstruction(
                        "project DisplayText primary was not evaluated".to_owned(),
                    )
                })?;
                let receiver = if project_option {
                    match receiver.try_into_builtin_variant_case() {
                        Ok((
                            crate::pattern::RuntimeBuiltinVariantCaseIdentity::OptionSome,
                            Some(value),
                        )) => Some(value),
                        Ok((
                            crate::pattern::RuntimeBuiltinVariantCaseIdentity::OptionNone,
                            None,
                        )) => None,
                        _ => {
                            return Err(RuntimeEvalError::InvalidExpressionType(
                                primary.expression().ty(),
                            ));
                        }
                    }
                } else {
                    Some(receiver)
                };
                if let Some(receiver) = receiver {
                    let context = crate::value::project_display_context(
                        &context_layout,
                        &format_context,
                        &evaluated,
                    )
                    .map_err(|error| {
                        RuntimeEvalError::DialogueContentConstruction(error.to_string())
                    })?;
                    match context {
                        Ok(context) => {
                            let receiver = RuntimeExpr::from_admitted_parts(
                                receiver_ty,
                                RuntimeExprKind::Value(receiver),
                            );
                            let context = RuntimeExpr::from_admitted_parts(
                                context_ty,
                                RuntimeExprKind::Value(context),
                            );
                            let argument = RuntimeCallArgument::from_admitted_parts(
                                context,
                                crate::value::RuntimeCallArgumentMode::Value,
                                0,
                            );
                            let result = self.evaluate_trait_call_expr(
                                method_id,
                                RuntimeReceiverMode::Owned,
                                &receiver,
                                &[argument],
                                method.body.ty(),
                            )?;
                            match crate::value::project_display_result(result, &error_layout)
                                .map_err(|error| {
                                    RuntimeEvalError::DialogueContentConstruction(error.to_string())
                                })? {
                                Ok(content) => {
                                    evaluated[value_position].1 = Some(if project_option {
                                        RuntimeValue::option_some(content)
                                    } else {
                                        content
                                    });
                                }
                                Err(reason) => first_recoverable = Some(reason),
                            }
                        }
                        Err(reason) => first_recoverable = Some(reason),
                    }
                }
            }
        } else if project_option {
            return Err(RuntimeEvalError::DialogueContentConstruction(
                "project fmt option has no selected DisplayText method".to_owned(),
            ));
        }
        let formatted = crate::value::finish_format_content_attempt(
            &format_context,
            primary_kind,
            &evaluated,
            first_recoverable.as_deref(),
        )
        .map_err(|error| RuntimeEvalError::DialogueContentConstruction(error.to_string()))?;
        let artifact = self
            .plan
            .artifact()
            .ok_or(RuntimeEvalError::DialogueContentUnboundArtifact)?;
        let binding = crate::plan::RuntimeDialogueValueBinding {
            slot: slot.slot(),
            role: slot.role(),
            value: formatted.into_runtime_value(),
        };
        crate::value::RuntimeDialogueContentValue::try_from_evaluated_bindings(
            artifact,
            &manifest,
            &[binding],
        )
        .map(crate::value::RuntimeDialogueContentValue::into_runtime_value)
        .map_err(|error| RuntimeEvalError::DialogueContentConstruction(error.to_string()))
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
        &mut self,
        read: &RuntimeLocalRead,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.env.read(read)
    }

    fn evaluate_agent_expr(
        &mut self,
        agent: &RuntimeAgentExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let mut operands = Vec::new();
        if let Some(choice) = agent.choice() {
            operands.push(RuntimeValue::String(choice.as_str().to_owned()));
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
        let RuntimePlanTypeProjection::Nominal {
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
            if !self
                .env
                .value_matches_type(plan.as_ref(), field.ty(), &value)?
            {
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
                    crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)
                        .map_err(|_| RuntimeEvalError::InvalidExpressionType(ty))?;
                field.ok_or(RuntimeEvalError::MissingRecordInitializer {
                    ty,
                    field: field_id,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RuntimeValue::NominalRecord(
            crate::value::RuntimeNominalRecordValue::new(
                nominal.clone(),
                declaration.semantic_identity(),
                *layout,
                fields,
            )
            .with_type_instantiation(
                (!declaration.scope().is_root())
                    .then(|| self.env.function_instantiation_lease().cloned())
                    .flatten(),
            ),
        ))
    }

    fn evaluate_variant_expr(
        &mut self,
        ty: crate::runtime_id::RuntimePlanTypeId,
        ordinal: u32,
        payload: Option<&RuntimeExpr>,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let plan = Arc::clone(self.plan);
        let case = plan.variant_case(ty, ordinal)?;
        let payload = payload.map(|expr| self.evaluate_expr(expr)).transpose()?;
        match (case.payload(), payload.as_ref()) {
            (Some(expected), Some(value))
                if self
                    .env
                    .value_matches_type(plan.as_ref(), expected, value)? => {}
            (None, None) => {}
            _ => return Err(RuntimeEvalError::InvalidExpressionType(ty)),
        }
        Ok(RuntimeValue::Variant {
            owner: case.owner().clone(),
            ordinal,
            name: case.name().to_owned(),
            payload: payload.map(Box::new),
            type_instantiation: (!plan
                .type_table()
                .get(ty)
                .ok_or(RuntimeEvalError::UnknownPlanType(ty))?
                .scope()
                .is_root())
            .then(|| self.env.function_instantiation_lease().cloned())
            .flatten(),
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
        if !self
            .env
            .value_matches_type(plan.as_ref(), state_ty, &state)?
        {
            return Err(RuntimeEvalError::InvalidExpressionType(ty));
        }
        let owner = RuntimeOpaqueTypeOwner::exact(producer, semantic_identity);
        RuntimeReductionValue::try_unchanged(owner, state)
            .map(RuntimeValue::Reduction)
            .map_err(|_| RuntimeEvalError::InvalidExpressionType(ty))
    }

    fn evaluate_assign_expr(
        &mut self,
        place: &crate::value::RuntimeAssignment,
        expr: &RuntimeExpr,
        body: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(expr)?;
        self.env
            .assign_place(place, value)
            .map_err(|error| error.into_parts().0)?;
        self.evaluate_expr(body)
    }

    fn evaluate_nested_pure_call(
        &mut self,
        helper_id: RuntimePureHelperId,
        args: &[RuntimeCallArgument],
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let values = self.evaluate_call_args(args)?;
        let plan = Arc::clone(self.plan);
        let helper = resolve_validated_pure_function(&plan, helper_id)?;
        let bindings = prepare_helper_bindings(helper, values)?;
        self.with_temp_bindings(bindings, |this| this.evaluate_expr(helper.expr))
    }

    fn evaluate_trait_call_expr(
        &mut self,
        callable: RuntimeTraitMethodId,
        receiver_mode: RuntimeReceiverMode,
        receiver: &RuntimeExpr,
        args: &[RuntimeCallArgument],
        result_ty: crate::runtime_id::RuntimePlanTypeId,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let method = self
            .plan
            .trait_methods()
            .get(callable.0)
            .filter(|method| method.id == callable)
            .ok_or(RuntimeEvalError::UnknownTraitMethod(callable.0))?
            .clone();
        if receiver_mode != RuntimeReceiverMode::Owned || method.receiver != receiver_mode {
            return Self::unsupported_pure_trait_operation(
                "only owned receiver methods can run in pure evaluation",
            );
        }
        let Some(receiver_local) = method.inputs.first().map(|input| input.local()) else {
            return Err(RuntimeEvalError::InvalidTraitReceiverUpdate {
                method: method.identity.method_name,
                receiver: method.identity.self_type,
            });
        };
        for input in &method.inputs {
            let local = input.local();
            let input_type = input.abi();
            let declaration = self
                .plan
                .local_declarations()
                .get(local)
                .ok_or(RuntimeEvalError::UnknownLocal(local))?;
            if input_type != RuntimePureInputType::Value
                && pure_scalar_projection(self.plan, declaration.ty())
                    != Some(input_as_output(input_type))
            {
                return Err(RuntimeEvalError::InvalidExpressionType(declaration.ty()));
            }
        }
        let receiver_ty = self
            .plan
            .local_declarations()
            .get(receiver_local)
            .ok_or(RuntimeEvalError::UnknownLocal(receiver_local))?
            .ty();
        if receiver.ty() != receiver_ty {
            return Err(RuntimeEvalError::InvalidExpressionType(receiver.ty()));
        }
        if method.body.ty() != result_ty
            || (method.output_type != RuntimePureOutputType::Value
                && pure_scalar_projection(self.plan, method.body.ty()) != Some(method.output_type))
        {
            return Err(RuntimeEvalError::InvalidExpressionType(result_ty));
        }

        self.stats.evaluated_calls += 1;
        let previous = std::mem::replace(&mut self.evaluating_pure_trait_call, true);
        let result = (|| {
            let receiver_value = self.evaluate_expr(receiver)?;
            let mut values = vec![receiver_value];
            values.extend(self.evaluate_call_args(args)?);
            if values.len() != method.inputs.len() {
                return Err(RuntimeEvalError::TraitMethodArgumentCount {
                    method: method.identity.method_name.clone(),
                    expected: method.inputs.len() - 1,
                    found: values.len() - 1,
                });
            }
            let bindings = method
                .inputs
                .iter()
                .map(|input| input.local())
                .zip(values)
                .map(|(local, value)| {
                    let declaration = self
                        .plan
                        .local_declarations()
                        .get(local)
                        .ok_or(RuntimeEvalError::UnknownLocal(local))?;
                    if !self
                        .env
                        .value_matches_type(self.plan.as_ref(), declaration.ty(), &value)?
                    {
                        return Err(RuntimeEvalError::InvalidExpressionType(declaration.ty()));
                    }
                    Ok(RuntimeLocalBinding { local, value })
                })
                .collect::<Result<Vec<_>, _>>()?;
            self.with_temp_bindings(bindings, |this| this.evaluate_expr(&method.body))
        })();
        self.evaluating_pure_trait_call = previous;
        result
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
        if !crate::pattern::inspect_runtime_pattern_owned(
            self.plan,
            pattern,
            &value,
            self.env.function_instantiation(),
        )? {
            return self.evaluate_expr(else_expr);
        }
        let guard_matched = if let Some(guard) = guard {
            let projected = crate::pattern::prepare_runtime_pattern_guard_bindings(
                self.plan,
                pattern,
                &value,
                guard,
                self.env.function_instantiation(),
            )?;
            self.with_temp_bindings(projected, |this| this.evaluate_bool(guard))?
        } else {
            true
        };
        if guard_matched {
            let bindings = crate::pattern::match_runtime_pattern_owned(
                self.plan,
                pattern,
                value,
                self.env.function_instantiation(),
            )?
            .expect("checked pure if-let pattern remains matched");
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
            if !crate::pattern::inspect_runtime_pattern_owned(
                self.plan,
                arm.pattern(),
                &value,
                self.env.function_instantiation(),
            )? {
                continue;
            }
            if let Some(guard) = arm.guard() {
                let projected = crate::pattern::prepare_runtime_pattern_guard_bindings(
                    self.plan,
                    arm.pattern(),
                    &value,
                    guard,
                    self.env.function_instantiation(),
                )?;
                if !self.with_temp_bindings(projected, |this| this.evaluate_bool(guard))? {
                    continue;
                }
            }
            let bindings = crate::pattern::match_runtime_pattern_owned(
                self.plan,
                arm.pattern(),
                value,
                self.env.function_instantiation(),
            )?
            .expect("checked pure match arm remains selected");
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

    fn unsupported_pure_trait_operation(reason: &str) -> Result<RuntimeValue, RuntimeEvalError> {
        Err(RuntimeEvalError::UnsupportedPure {
            name: "trait method".to_owned(),
            reason: reason.to_owned(),
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
            RuntimeExprKind::Local(read) => match self.env.read(read) {
                Ok(value) => runtime_value_as_scalar(&value)
                    .ok_or_else(|| RuntimeEvalError::ExpectedInt(runtime_value_label(&value))),
                Err(error) => Err(error),
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
            RuntimeExprKind::Scope { identity, body } => {
                self.env.push_scope_with_identity(identity.clone());
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

    fn evaluate_standard_map_expr(
        &mut self,
        family: RuntimeStandardMapFamily,
        order: RuntimeStandardMapOperandOrder,
        mapping: &RuntimeExpr,
        source: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let (mapping, source) = match order {
            RuntimeStandardMapOperandOrder::MappingThenReceiver => {
                (self.evaluate_expr(mapping)?, self.evaluate_expr(source)?)
            }
            RuntimeStandardMapOperandOrder::ReceiverThenMapping => {
                let source = self.evaluate_expr(source)?;
                let mapping = self.evaluate_expr(mapping)?;
                (mapping, source)
            }
        };
        let RuntimeValue::Callable(mapping) = mapping else {
            return Err(RuntimeEvalError::ExpectedFunction(runtime_value_label(
                &mapping,
            )));
        };
        match family {
            RuntimeStandardMapFamily::Vec
            | RuntimeStandardMapFamily::Seq
            | RuntimeStandardMapFamily::Array
            | RuntimeStandardMapFamily::Slice => {
                let iterator = RuntimeIterator::from_value(source).map_err(|value| {
                    RuntimeEvalError::ExpectedBracketSeq(runtime_value_label(&value))
                })?;
                iterator
                    .map(|item| {
                        let use_value = mapping.try_duplicate_unrestricted()?;
                        self.apply_runtime_function(use_value, vec![item])
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map(runtime_sequence_values)
            }
            RuntimeStandardMapFamily::Option => {
                let (case, payload) = source
                    .try_into_builtin_variant_case()
                    .map_err(|_| RuntimeEvalError::InvalidStandardMapSource { family })?;
                match (case, payload) {
                    (RuntimeBuiltinVariantCaseIdentity::OptionSome, Some(value)) => self
                        .apply_runtime_function(mapping, vec![value])
                        .map(RuntimeValue::option_some),
                    (RuntimeBuiltinVariantCaseIdentity::OptionNone, None) => {
                        Ok(RuntimeValue::option_none())
                    }
                    _ => Err(RuntimeEvalError::InvalidStandardMapSource { family }),
                }
            }
            RuntimeStandardMapFamily::Result => {
                let (case, payload) = source
                    .try_into_builtin_variant_case()
                    .map_err(|_| RuntimeEvalError::InvalidStandardMapSource { family })?;
                match (case, payload) {
                    (RuntimeBuiltinVariantCaseIdentity::ResultOk, Some(value)) => self
                        .apply_runtime_function(mapping, vec![value])
                        .map(RuntimeValue::result_ok),
                    (RuntimeBuiltinVariantCaseIdentity::ResultErr, Some(error)) => {
                        Ok(RuntimeValue::result_err(error))
                    }
                    _ => Err(RuntimeEvalError::InvalidStandardMapSource { family }),
                }
            }
        }
    }

    fn evaluate_sum_expr(
        &mut self,
        source: &RuntimeExpr,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if let RuntimeExprKind::Local(read) = source.kind()
            && read.mode() == RuntimeLocalReadMode::Copy
            && read.fields().is_empty()
            && let Some(sum) = self.evaluate_i64_local_sequence_sum(read.local())?
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
            && let Some(value) = evaluate_capacity_intrinsic(intrinsic, &args)?
        {
            return Ok(value);
        }
        if let Some(intrinsic) = callee.as_intrinsic()
            && let Some(value) = evaluate_index_intrinsic(intrinsic, &args)?
        {
            return Ok(value);
        }
        Self::evaluate_pure_call(callee, args)
    }

    fn evaluate_pure_call(
        callee: &RuntimeCallTarget,
        mut args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if let Some(intrinsic) = callee.as_intrinsic()
            && let Some(result) = evaluate_core_iterator_intrinsic(intrinsic, &mut args)
        {
            return result;
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
        field: &RuntimeFieldProjection,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let value = self.evaluate_expr(target)?;
        match (field, value) {
            (RuntimeFieldProjection::Nominal(field), RuntimeValue::NominalRecord(record)) => record
                .field(*field)
                .cloned()
                .ok_or_else(|| RuntimeEvalError::MissingField {
                    field: field.zero_based().to_string(),
                    value: "nominal record".to_owned(),
                }),
            (
                RuntimeFieldProjection::OpaqueRecord { owner, field },
                RuntimeValue::Opaque(value),
            ) if owner.accepts_opaque_value(&value) => {
                let RuntimeValue::Tuple(fields) = value.payload() else {
                    return Err(RuntimeEvalError::MissingField {
                        field: field.zero_based().to_string(),
                        value: "opaque record payload".to_owned(),
                    });
                };
                fields
                    .get(field.zero_based() as usize)
                    .cloned()
                    .ok_or_else(|| RuntimeEvalError::MissingField {
                        field: field.zero_based().to_string(),
                        value: "opaque record payload".to_owned(),
                    })
            }
            (RuntimeFieldProjection::EntityReference(field), RuntimeValue::EntityRef(id)) => {
                Ok(RuntimeValue::String(id.field_value(*field)))
            }
            (RuntimeFieldProjection::Agent(field), RuntimeValue::Agent(value)) => value
                .project_typed_field(*field)
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
        let mut materialized = Vec::with_capacity(args.len());
        for argument in args {
            let value = self.evaluate_expr(argument.value())?;
            let values = match argument.mode() {
                RuntimeCallArgumentMode::Value => vec![value],
                RuntimeCallArgumentMode::Spread => spread_runtime_values(value)?,
            };
            materialized.push((argument.abi_position(), values));
        }
        materialized.sort_by_key(|(position, _)| *position);
        let mut values = Vec::new();
        for (_, materialized) in materialized {
            values.extend(materialized);
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

#[cfg(test)]
mod opaque_record_projection_tests {
    use super::*;
    use crate::pattern::{RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId};
    use crate::plan::{RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed};
    use crate::value::{
        RuntimeFieldProjection, RuntimeHandleKind, RuntimeOpaquePersistence,
        RuntimeOpaqueValueClass, RuntimeRecordFieldId,
    };

    fn identity(marker: u8) -> RuntimeSemanticTypeId {
        RuntimeSemanticTypeId::from_bytes([marker; 32])
    }

    fn producer(label: &str) -> RuntimeOpaqueTypeProducerId {
        RuntimeOpaqueTypeProducerId::try_new(label).expect("test opaque producer")
    }

    fn plan_and_owner() -> (Arc<RuntimePlan>, RuntimeOpaqueTypeOwner) {
        let owner = RuntimeOpaqueTypeOwner::exact_with(
            producer("fixture.dialogue-view"),
            identity(101),
            RuntimeOpaqueValueClass::Plain,
            RuntimeOpaquePersistence::ConstantAndSnapshot,
        );
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [
                    RuntimePlanTypeSeed::new(
                        owner.semantic_identity(),
                        RuntimePlanTypeProjection::Opaque {
                            producer: owner.producer().clone(),
                            admission: owner.admission(),
                            value_class: owner.value_class(),
                            persistence: owner.persistence(),
                            arguments: Box::new([]),
                        },
                    ),
                    RuntimePlanTypeSeed::new(identity(102), RuntimePlanTypeProjection::String),
                ],
                [],
            )
            .expect("test type graph");
        (
            Arc::new(builder.finish().expect("test runtime plan")),
            owner,
        )
    }

    fn expression(
        plan: &RuntimePlan,
        expected: &RuntimeOpaqueTypeOwner,
        actual: &RuntimeOpaqueTypeOwner,
    ) -> RuntimeExpr {
        let owner_ty = plan
            .type_table()
            .id_for_semantic(expected.semantic_identity())
            .expect("opaque owner type");
        let field_ty = plan
            .type_table()
            .id_for_semantic(identity(102))
            .expect("field type");
        let target = RuntimeExpr::from_admitted_parts(
            owner_ty,
            RuntimeExprKind::Value(
                actual
                    .try_wrap(RuntimeValue::Tuple(vec![RuntimeValue::String(
                        "accepted".to_owned(),
                    )]))
                    .expect("exact tamper fixture"),
            ),
        );
        RuntimeExpr::from_admitted_parts(
            field_ty,
            RuntimeExprKind::Field {
                target: Box::new(target),
                field: RuntimeFieldProjection::OpaqueRecord {
                    owner: expected.clone(),
                    field: RuntimeRecordFieldId::try_from_zero_based_ordinal(0)
                        .expect("first field"),
                },
            },
        )
    }

    #[test]
    fn pure_field_projection_rejects_each_tampered_opaque_owner_dimension() {
        let (plan, expected) = plan_and_owner();
        let mut evaluator = PureEvaluator::new_ref(&plan, &[]);
        assert_eq!(
            evaluator
                .evaluate_expr(&expression(&plan, &expected, &expected))
                .expect("exact opaque owner"),
            RuntimeValue::String("accepted".to_owned())
        );

        let tampered = [
            RuntimeOpaqueTypeOwner::exact_with(
                producer("fixture.other-dialogue-view"),
                expected.semantic_identity(),
                expected.value_class(),
                expected.persistence(),
            ),
            RuntimeOpaqueTypeOwner::exact_with(
                expected.producer().clone(),
                expected.semantic_identity(),
                RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::StageActor),
                expected.persistence(),
            ),
            RuntimeOpaqueTypeOwner::exact_with(
                expected.producer().clone(),
                expected.semantic_identity(),
                expected.value_class(),
                RuntimeOpaquePersistence::SnapshotOnly,
            ),
        ];
        let owner_ty = plan
            .type_table()
            .id_for_semantic(expected.semantic_identity())
            .expect("opaque owner type");
        for (index, actual) in tampered.into_iter().enumerate() {
            let result = evaluator.evaluate_expr(&expression(&plan, &expected, &actual));
            if index == 1 {
                assert_eq!(result, Err(RuntimeEvalError::AffineLiteralCopy));
            } else {
                assert_eq!(
                    result,
                    Err(RuntimeEvalError::InvalidExpressionType(owner_ty))
                );
            }
        }
    }
}
