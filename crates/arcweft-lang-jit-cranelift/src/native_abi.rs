//! One checked native-call ABI for scalar, row and reduction entrypoints.
use crate::{CraneliftCodegenError, FunctionBuilder, InstBuilder, Value, types};
use arcweft_core::value::RuntimeExpressionFailure;
use cranelift::codegen::ir::{MemFlags, Signature, Type};
use cranelift::prelude::AbiParam;

/// Current Arcweft-owned native pure-call contract.
pub const NATIVE_PURE_CALL_ABI_VERSION: u32 = 1;

/// Closed result status for every native pure scalar, row and reduction call.
///
/// Scalar and sum result slots are initialized only on `Success`. Row outputs
/// contain exactly the completed prefix. A failing current row is attempted
/// once; no subsequent row executes. `SumConversionRejected` additionally
/// initializes the separate exact-width rejected-value slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum NativePureCallOutcome {
    Success = 0,
    DivisionByZero = 1,
    SumConversionRejected = 2,
}

impl NativePureCallOutcome {
    /// Decodes a closed status. Unknown codes must be rejected before reading
    /// any output or rejected-value storage.
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Success),
            1 => Some(Self::DivisionByZero),
            2 => Some(Self::SumConversionRejected),
            _ => None,
        }
    }
}

pub(crate) fn append_scalar_signature(signature: &mut Signature, pointer: Type) {
    signature
        .params
        .extend([AbiParam::new(pointer), AbiParam::new(pointer)]);
    signature.returns.clear();
    signature.returns.push(AbiParam::new(types::I8));
}

pub(crate) fn append_rows_signature(signature: &mut Signature, pointer: Type) {
    signature.params.push(AbiParam::new(pointer));
    signature.returns.push(AbiParam::new(types::I8));
}

pub(crate) fn store_progress(builder: &mut FunctionBuilder<'_>, progress: Value, completed: Value) {
    builder.ins().store(MemFlags::new(), completed, progress, 0);
}

pub(crate) fn return_result(
    builder: &mut FunctionBuilder<'_>,
    output: Value,
    progress: Value,
    value: Value,
    completed: Value,
) {
    builder.ins().store(MemFlags::new(), value, output, 0);
    store_progress(builder, progress, completed);
    return_success(builder);
}

pub(crate) fn return_success(builder: &mut FunctionBuilder<'_>) {
    let code = builder
        .ins()
        .iconst(types::I8, NativePureCallOutcome::Success as i64);
    builder.ins().return_(&[code]);
}

/// Rejects zero through the owning checked status before an integer division
/// can trap or any current-row output is initialized.
fn require_nonzero_divisor(builder: &mut FunctionBuilder<'_>, rhs: Value) {
    let ty = builder.func.dfg.value_type(rhs);
    let zero = signed_constant(builder, ty, 0);
    let valid = builder.create_block();
    let failed = builder.create_block();
    let nonzero = builder
        .ins()
        .icmp(cranelift::prelude::IntCC::NotEqual, rhs, zero);
    builder.ins().brif(nonzero, valid, &[], failed, &[]);
    builder.switch_to_block(failed);
    let code = builder
        .ins()
        .iconst(types::I8, NativePureCallOutcome::DivisionByZero as i64);
    builder.ins().return_(&[code]);
    builder.switch_to_block(valid);
}

/// Creates a small signed constant without an unsupported wide iconst.
fn signed_constant(builder: &mut FunctionBuilder<'_>, ty: Type, value: i64) -> Value {
    if ty.bits() > 64 {
        let narrow = builder.ins().iconst(types::I64, value);
        builder.ins().sextend(ty, narrow)
    } else {
        builder.ins().iconst(ty, value)
    }
}

/// Emits unsigned division only after the common zero-fault guard.
pub(crate) fn unsigned_division(
    builder: &mut FunctionBuilder<'_>,
    lhs: Value,
    rhs: Value,
) -> Value {
    require_nonzero_divisor(builder, rhs);
    builder.ins().udiv(lhs, rhs)
}

/// Implements Core's exact-width wrapping signed division. MIN / -1 succeeds
/// with MIN, while zero is a typed fault. A machine sdiv is issued only on the
/// nonzero, nonoverflow branch.
pub(crate) fn signed_division(builder: &mut FunctionBuilder<'_>, lhs: Value, rhs: Value) -> Value {
    use cranelift::prelude::IntCC;

    require_nonzero_divisor(builder, rhs);
    let ty = builder.func.dfg.value_type(lhs);
    let one = signed_constant(builder, ty, 1);
    let minus_one = signed_constant(builder, ty, -1);
    let minimum = builder.ins().ishl_imm(one, i64::from(ty.bits() - 1));
    let lhs_is_minimum = builder.ins().icmp(IntCC::Equal, lhs, minimum);
    let rhs_is_minus_one = builder.ins().icmp(IntCC::Equal, rhs, minus_one);
    let wraps = builder.ins().band(lhs_is_minimum, rhs_is_minus_one);
    let wrapping = builder.create_block();
    let dividing = builder.create_block();
    let finished = builder.create_block();
    builder.append_block_param(finished, ty);
    builder.ins().brif(wraps, wrapping, &[], dividing, &[]);
    builder.switch_to_block(wrapping);
    builder.ins().jump(finished, &[lhs.into()]);
    builder.switch_to_block(dividing);
    let quotient = if ty.bits() > 64 {
        wide_signed_quotient(builder, lhs, rhs)
    } else {
        builder.ins().sdiv(lhs, rhs)
    };
    builder.ins().jump(finished, &[quotient.into()]);
    builder.switch_to_block(finished);
    builder.block_params(finished)[0]
}

/// Cranelift 0.121 has no scalar i128 sdiv lowering. Divide the exact-width
/// magnitudes in 128 bounded iterations, then restore the signed result.
/// Magnitudes are at most 2^127, so a remainder smaller than the divisor can
/// be shifted left with the next bit without losing a carry. This is native
/// lowering of one admitted numeric operation; it introduces no new source
/// type, FFI representation, callback, or runtime execution model.
fn wide_signed_quotient(builder: &mut FunctionBuilder<'_>, lhs: Value, rhs: Value) -> Value {
    use cranelift::prelude::IntCC;

    let ty = builder.func.dfg.value_type(lhs);
    let zero = signed_constant(builder, ty, 0);
    let lhs_negative = builder.ins().icmp(IntCC::SignedLessThan, lhs, zero);
    let rhs_negative = builder.ins().icmp(IntCC::SignedLessThan, rhs, zero);
    let negated_lhs = builder.ins().ineg(lhs);
    let negated_rhs = builder.ins().ineg(rhs);
    let magnitude = builder.ins().select(lhs_negative, negated_lhs, lhs);
    let divisor = builder.ins().select(rhs_negative, negated_rhs, rhs);
    let negative = builder.ins().bxor(lhs_negative, rhs_negative);

    let dividing = builder.create_block();
    builder.append_block_param(dividing, types::I64);
    builder.append_block_param(dividing, ty);
    builder.append_block_param(dividing, ty);
    let finished = builder.create_block();
    builder.append_block_param(finished, ty);
    let iterations = builder.ins().iconst(types::I64, i64::from(ty.bits()));
    builder.ins().jump(
        dividing,
        &[iterations.into(), magnitude.into(), zero.into()],
    );
    builder.switch_to_block(dividing);
    let remaining = builder.block_params(dividing)[0];
    let quotient = builder.block_params(dividing)[1];
    let remainder = builder.block_params(dividing)[2];
    let next_bit = builder.ins().ushr_imm(quotient, i64::from(ty.bits() - 1));
    let shifted_quotient = builder.ins().ishl_imm(quotient, 1);
    let shifted_remainder = builder.ins().ishl_imm(remainder, 1);
    let candidate_remainder = builder.ins().bor(shifted_remainder, next_bit);
    let subtract = builder.ins().icmp(
        IntCC::UnsignedGreaterThanOrEqual,
        candidate_remainder,
        divisor,
    );
    let reduced_remainder = builder.ins().isub(candidate_remainder, divisor);
    let next_remainder = builder
        .ins()
        .select(subtract, reduced_remainder, candidate_remainder);
    let quotient_bit = builder.ins().uextend(ty, subtract);
    let next_quotient = builder.ins().bor(shifted_quotient, quotient_bit);
    let next_remaining = builder.ins().iadd_imm(remaining, -1);
    let again = builder.ins().icmp_imm(IntCC::NotEqual, next_remaining, 0);
    builder.ins().brif(
        again,
        dividing,
        &[
            next_remaining.into(),
            next_quotient.into(),
            next_remainder.into(),
        ],
        finished,
        &[next_quotient.into()],
    );
    builder.switch_to_block(finished);
    let magnitude = builder.block_params(finished)[0];
    let negated = builder.ins().ineg(magnitude);
    builder.ins().select(negative, negated, magnitude)
}

pub(crate) fn append_sum_signature(signature: &mut Signature, pointer: Type) {
    append_scalar_signature(signature, pointer);
    signature.params.push(AbiParam::new(pointer));
}

pub(crate) fn sum_conversion(
    builder: &mut FunctionBuilder<'_>,
    value: Value,
    signed: bool,
    rejected: Value,
) -> Value {
    let ty = builder.func.dfg.value_type(value);
    if ty.bits() < 64 {
        return if signed {
            builder.ins().sextend(types::I64, value)
        } else {
            builder.ins().uextend(types::I64, value)
        };
    }
    let narrow = if ty.bits() == 64 {
        value
    } else {
        builder.ins().ireduce(types::I64, value)
    };
    if ty.bits() == 64 && signed {
        return narrow;
    }
    let valid = if ty.bits() > 64 {
        let extended = if signed {
            builder.ins().sextend(ty, narrow)
        } else {
            builder.ins().uextend(ty, narrow)
        };
        let same = builder
            .ins()
            .icmp(cranelift::prelude::IntCC::Equal, value, extended);
        if signed {
            same
        } else {
            let positive = builder.ins().icmp_imm(
                cranelift::prelude::IntCC::SignedGreaterThanOrEqual,
                narrow,
                0,
            );
            builder.ins().band(same, positive)
        }
    } else {
        builder.ins().icmp_imm(
            cranelift::prelude::IntCC::SignedGreaterThanOrEqual,
            narrow,
            0,
        )
    };
    let accepted = builder.create_block();
    let failed = builder.create_block();
    builder.ins().brif(valid, accepted, &[], failed, &[]);
    builder.switch_to_block(failed);
    builder.ins().store(MemFlags::new(), value, rejected, 0);
    let code = builder.ins().iconst(
        types::I8,
        NativePureCallOutcome::SumConversionRejected as i64,
    );
    builder.ins().return_(&[code]);
    builder.switch_to_block(accepted);
    narrow
}

pub(crate) fn decode_outcome(
    code: u8,
    completed: u64,
    requested: usize,
) -> Result<NativePureCallOutcome, CraneliftCodegenError> {
    let requested = u64::try_from(requested).map_err(|_| {
        CraneliftCodegenError::Backend("native row count is not representable".to_owned())
    })?;
    match NativePureCallOutcome::from_code(code) {
        Some(NativePureCallOutcome::Success) if completed == requested => {
            Ok(NativePureCallOutcome::Success)
        }
        Some(NativePureCallOutcome::DivisionByZero) if completed < requested => {
            Ok(NativePureCallOutcome::DivisionByZero)
        }
        Some(NativePureCallOutcome::SumConversionRejected) if completed < requested => {
            Ok(NativePureCallOutcome::SumConversionRejected)
        }
        _ => Err(CraneliftCodegenError::InvalidNativeOutcome {
            code,
            completed,
            requested,
        }),
    }
}

pub(crate) fn division_failure(completed: u64) -> CraneliftCodegenError {
    match usize::try_from(completed) {
        Ok(completed_rows) => CraneliftCodegenError::Execution {
            error: RuntimeExpressionFailure::DivisionByZero.into(),
            completed_rows,
        },
        Err(_) => CraneliftCodegenError::Backend(
            "native failure progress is not representable".to_owned(),
        ),
    }
}

pub(crate) fn check_outcome(
    code: u8,
    completed: u64,
    requested: usize,
) -> Result<(), CraneliftCodegenError> {
    match decode_outcome(code, completed, requested)? {
        NativePureCallOutcome::Success => Ok(()),
        NativePureCallOutcome::DivisionByZero => Err(division_failure(completed)),
        NativePureCallOutcome::SumConversionRejected => {
            Err(CraneliftCodegenError::InvalidNativeOutcome {
                code,
                completed,
                requested: u64::try_from(requested).map_err(|_| {
                    CraneliftCodegenError::Backend(
                        "native row count is not representable".to_owned(),
                    )
                })?,
            })
        }
    }
}
