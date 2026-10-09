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

/// Emits a division only on the nonzero branch. Failure returns through the
/// owning function's checked outcome ABI before any current-row output write.
pub(crate) fn unsigned_division(
    builder: &mut FunctionBuilder<'_>,
    lhs: Value,
    rhs: Value,
) -> Value {
    let valid = builder.create_block();
    let failed = builder.create_block();
    let nonzero = builder
        .ins()
        .icmp_imm(cranelift::prelude::IntCC::NotEqual, rhs, 0);
    builder.ins().brif(nonzero, valid, &[], failed, &[]);
    builder.switch_to_block(failed);
    let code = builder
        .ins()
        .iconst(types::I8, NativePureCallOutcome::DivisionByZero as i64);
    builder.ins().return_(&[code]);
    builder.switch_to_block(valid);
    builder.ins().udiv(lhs, rhs)
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
