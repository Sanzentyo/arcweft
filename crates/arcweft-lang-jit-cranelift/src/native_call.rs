//! Audited invocation boundary for the checked native pure-call ABI.
//!
//! All code pointers come from matching definitions in this crate. Their
//! owning JITModule remains live throughout each call. Scalar formals preserve
//! their exact physical widths/order; results and progress use caller-owned
//! stack storage. Callees never unwind or retain borrowed pointers.
#![allow(unsafe_code)]
use crate::{CraneliftCodegenError, native_abi::check_outcome};
use arcweft_core::value::{RuntimeISizeValue, RuntimeUSizeValue};
use std::mem::{self, MaybeUninit};

fn scalar_result<T>(
    requested: usize,
    invoke: impl FnOnce(*mut T, *mut u64) -> u8,
) -> Result<T, CraneliftCodegenError> {
    let mut output = MaybeUninit::<T>::uninit();
    let mut completed = 0_u64;
    let code = invoke(output.as_mut_ptr(), &mut completed);
    check_outcome(code, completed, requested)?;
    // SAFETY: matching emitted entrypoints store the complete T result before
    // returning Success. Alignment/lifetime come from MaybeUninit<T>; progress
    // and outcome are checked before this read. Error paths never read output.
    Ok(unsafe { output.assume_init() })
}

fn sum_result<T>(
    requested: usize,
    convert: impl FnOnce(T) -> Result<i64, arcweft_core::value::RuntimeEvalError>,
    invoke: impl FnOnce(*mut i64, *mut u64, *mut T) -> u8,
) -> Result<i64, CraneliftCodegenError> {
    let mut output = MaybeUninit::<i64>::uninit();
    let mut rejected = MaybeUninit::<T>::uninit();
    let mut completed = 0_u64;
    let code = invoke(output.as_mut_ptr(), &mut completed, rejected.as_mut_ptr());
    match crate::native_abi::decode_outcome(code, completed, requested)? {
        crate::native_abi::NativePureCallOutcome::Success => {
            // SAFETY: the owning reduction factory initializes i64 output only
            // after every requested row converts and accumulates successfully.
            Ok(unsafe { output.assume_init() })
        }
        crate::native_abi::NativePureCallOutcome::DivisionByZero => {
            Err(crate::native_abi::division_failure(completed))
        }
        crate::native_abi::NativePureCallOutcome::SumConversionRejected => {
            // SAFETY: matching reduction entrypoints write the exact rejected T
            // into this separately aligned failure slot before returning this
            // outcome. Sum output remains unread and may be uninitialized.
            let value = unsafe { rejected.assume_init() };
            let Err(error) = convert(value) else {
                return Err(CraneliftCodegenError::InvalidNativeOutcome {
                    code,
                    completed,
                    requested: u64::try_from(requested).map_err(|_| {
                        CraneliftCodegenError::Backend(
                            "native row count is not representable".to_owned(),
                        )
                    })?,
                });
            };
            let completed_rows = usize::try_from(completed).map_err(|_| {
                CraneliftCodegenError::Backend(
                    "native failure progress is not representable".to_owned(),
                )
            })?;
            Err(CraneliftCodegenError::Execution {
                error,
                completed_rows,
            })
        }
    }
}

macro_rules! scalar_caller {
    ($name:ident, $ty:ty) => {
        #[derive(Clone, Copy)]
        pub(crate) enum $name {
            Nullary(extern "C" fn(*mut $ty, *mut u64) -> u8),
            Unary(extern "C" fn($ty, *mut $ty, *mut u64) -> u8),
            Binary(extern "C" fn($ty, $ty, *mut $ty, *mut u64) -> u8),
            Ternary(extern "C" fn($ty, $ty, $ty, *mut $ty, *mut u64) -> u8),
            Quaternary(extern "C" fn($ty, $ty, $ty, $ty, *mut $ty, *mut u64) -> u8),
        }
        impl $name {
            pub(crate) fn from_code(code: *const u8, arity: usize) -> Option<Self> {
                // SAFETY: arity selects the exact emitted C signature and
                // physical type. The constructor is private to this adapter,
                // and every receiver retains the JITModule owning code.
                unsafe {
                    match arity {
                        0 => Some(Self::Nullary(mem::transmute::<
                            *const u8,
                            extern "C" fn(*mut $ty, *mut u64) -> u8,
                        >(code))),
                        1 => Some(Self::Unary(mem::transmute::<
                            *const u8,
                            extern "C" fn($ty, *mut $ty, *mut u64) -> u8,
                        >(code))),
                        2 => Some(Self::Binary(mem::transmute::<
                            *const u8,
                            extern "C" fn($ty, $ty, *mut $ty, *mut u64) -> u8,
                        >(code))),
                        3 => Some(Self::Ternary(mem::transmute::<
                            *const u8,
                            extern "C" fn($ty, $ty, $ty, *mut $ty, *mut u64) -> u8,
                        >(code))),
                        4 => Some(Self::Quaternary(mem::transmute::<
                            *const u8,
                            extern "C" fn($ty, $ty, $ty, $ty, *mut $ty, *mut u64) -> u8,
                        >(code))),
                        _ => None,
                    }
                }
            }
            pub(crate) fn call(self, inputs: &[$ty]) -> Result<Option<$ty>, CraneliftCodegenError> {
                match (self, inputs) {
                    (Self::Nullary(function), []) => {
                        scalar_result(1, |out, done| function(out, done)).map(Some)
                    }
                    (Self::Unary(function), [a]) => {
                        scalar_result(1, |out, done| function(*a, out, done)).map(Some)
                    }
                    (Self::Binary(function), [a, b]) => {
                        scalar_result(1, |out, done| function(*a, *b, out, done)).map(Some)
                    }
                    (Self::Ternary(function), [a, b, c]) => {
                        scalar_result(1, |out, done| function(*a, *b, *c, out, done)).map(Some)
                    }
                    (Self::Quaternary(function), [a, b, c, d]) => {
                        scalar_result(1, |out, done| function(*a, *b, *c, *d, out, done)).map(Some)
                    }
                    _ => Ok(None),
                }
            }
        }
    };
}
scalar_caller!(I8InputCaller, i8);
scalar_caller!(I16InputCaller, i16);
scalar_caller!(I32InputCaller, i32);
scalar_caller!(I64InputCaller, i64);
scalar_caller!(U8InputCaller, u8);
scalar_caller!(U16InputCaller, u16);
scalar_caller!(U32InputCaller, u32);
scalar_caller!(U64InputCaller, u64);
scalar_caller!(F32InputCaller, f32);
scalar_caller!(F64InputCaller, f64);

impl I64InputCaller {
    pub(crate) fn call_packed(
        self,
        inputs: [i64; 4],
        len: usize,
    ) -> Result<Option<i64>, CraneliftCodegenError> {
        match inputs.get(..len) {
            Some(inputs) => self.call(inputs),
            None => Ok(None),
        }
    }
    pub(crate) fn call_isize(
        self,
        inputs: &[RuntimeISizeValue],
    ) -> Result<Option<RuntimeISizeValue>, CraneliftCodegenError> {
        let mut values = [0_i64; 4];
        let Some(slots) = values.get_mut(..inputs.len()) else {
            return Ok(None);
        };
        for (slot, value) in slots.iter_mut().zip(inputs) {
            *slot = value.get()
        }
        self.call(slots)
            .map(|value| value.map(RuntimeISizeValue::new))
    }
}
impl U64InputCaller {
    pub(crate) fn call_usize(
        self,
        inputs: &[RuntimeUSizeValue],
    ) -> Result<Option<RuntimeUSizeValue>, CraneliftCodegenError> {
        let mut values = [0_u64; 4];
        let Some(slots) = values.get_mut(..inputs.len()) else {
            return Ok(None);
        };
        for (slot, value) in slots.iter_mut().zip(inputs) {
            *slot = value.get()
        }
        self.call(slots)
            .map(|value| value.map(RuntimeUSizeValue::new))
    }
}
pub(crate) fn call_i64(code: *const u8) -> Result<i64, CraneliftCodegenError> {
    let Some(caller) = I64InputCaller::from_code(code, 0) else {
        return Err(CraneliftCodegenError::Backend(
            "invalid nullary native ABI".to_owned(),
        ));
    };
    caller
        .call(&[])?
        .ok_or_else(|| CraneliftCodegenError::Backend("nullary native ABI mismatch".to_owned()))
}
pub(crate) fn call_i64_batch(
    code: *const u8,
    seed: i64,
    sample: i64,
    iterations: i64,
) -> Result<i64, CraneliftCodegenError> {
    // SAFETY: this code is emitted by the benchmark batch factory with the
    // exact typed C signature; scalar_result lends valid stack output storage.
    let function = unsafe {
        mem::transmute::<*const u8, extern "C" fn(i64, i64, i64, *mut i64, *mut u64) -> u8>(code)
    };
    let rows = usize::try_from(iterations).map_err(|_| {
        CraneliftCodegenError::UnsupportedExpr("negative native iteration count".to_owned())
    })?;
    scalar_result(rows, |out, done| {
        function(seed, sample, iterations, out, done)
    })
}
macro_rules! rows_callers {
    ($batch:ident, $sum:ident, $ty:ty) => {
        rows_callers!($batch, $sum, $ty, |value: $ty| < $ty as arcweft_core::value::RuntimeExactInteger>::try_sum_as_i64(value, "native_exact_int_batch_sum"));
    };
    ($batch:ident, $sum:ident, $ty:ty, $convert:expr) => {
        pub(crate) fn $batch(code:*const u8, inputs:&[$ty],arity:usize,out:&mut[$ty])->Result<bool,CraneliftCodegenError>{
            if arity.checked_mul(out.len()) != Some(inputs.len()) {return Ok(false)}
            let Ok(rows)=i64::try_from(out.len()) else{return Ok(false)};
            // SAFETY: exact emitted C pointer ABI. Slices cover the validated
            // row-major shape, do not alias, and remain borrowed for the call.
            // Wide/target integer wrappers preserve their original storage.
            let function=unsafe{mem::transmute::<*const u8,extern "C" fn(*const $ty,i64,*mut $ty,*mut u64)->u8>(code)};
            let mut completed=0_u64;
            let code=function(inputs.as_ptr(),rows,out.as_mut_ptr(),&mut completed);
            check_outcome(code,completed,out.len())?;
            Ok(true)
        }
        pub(crate) fn $sum(code:*const u8, inputs:&[$ty],arity:usize,rows:usize)->Result<Option<i64>,CraneliftCodegenError>{
            if arity.checked_mul(rows) != Some(inputs.len()) {return Ok(None)}
            let Ok(native_rows)=i64::try_from(rows) else{return Ok(None)};
            // SAFETY: exact emitted reduction signature; input shape and row
            // count are checked. Output is read only after complete Success.
            let function=unsafe{mem::transmute::<*const u8,extern "C" fn(*const $ty,i64,*mut i64,*mut u64,*mut $ty)->u8>(code)};
            sum_result(rows,$convert,|out,done,rejected|function(inputs.as_ptr(),native_rows,out,done,rejected)).map(Some)
        }
    };
}
rows_callers!(call_i8_rows_batch, call_i8_rows_batch_sum, i8);
rows_callers!(call_i16_rows_batch, call_i16_rows_batch_sum, i16);
rows_callers!(call_i32_rows_batch, call_i32_rows_batch_sum, i32);
rows_callers!(call_i64_rows_batch, call_i64_rows_batch_sum, i64, |value| {
    Ok(value)
});
rows_callers!(call_i128_rows_batch, call_i128_rows_batch_sum, i128);
rows_callers!(
    call_isize_rows_batch,
    call_isize_rows_batch_sum,
    RuntimeISizeValue
);
rows_callers!(call_u8_rows_batch, call_u8_rows_batch_sum, u8);
rows_callers!(call_u16_rows_batch, call_u16_rows_batch_sum, u16);
rows_callers!(call_u32_rows_batch, call_u32_rows_batch_sum, u32);
rows_callers!(call_u64_rows_batch, call_u64_rows_batch_sum, u64);
rows_callers!(call_u128_rows_batch, call_u128_rows_batch_sum, u128);
rows_callers!(
    call_usize_rows_batch,
    call_usize_rows_batch_sum,
    RuntimeUSizeValue
);

macro_rules! float_rows_caller {
    ($batch:ident,$ty:ty) => {
        pub(crate) fn $batch(code:*const u8,inputs:&[$ty],arity:usize,out:&mut[$ty])->Result<bool,CraneliftCodegenError>{
            if arity.checked_mul(out.len()) != Some(inputs.len()) {return Ok(false)}
            let Ok(rows)=i64::try_from(out.len()) else{return Ok(false)};
            // SAFETY: matching emitted floating entrypoint and checked,
            // disjoint borrowed buffers; outcome/progress remain stack-owned.
            let function=unsafe{mem::transmute::<*const u8,extern "C" fn(*const $ty,i64,*mut $ty,*mut u64)->u8>(code)};
            let mut completed=0;
            let code=function(inputs.as_ptr(),rows,out.as_mut_ptr(),&mut completed);
            check_outcome(code,completed,out.len())?;
            Ok(true)
        }
    };
}
float_rows_caller!(call_f32_rows_batch, f32);
float_rows_caller!(call_f64_rows_batch, f64);

#[cfg(test)]
mod tests {
    use super::{scalar_result, sum_result};
    use crate::CraneliftCodegenError;
    use arcweft_core::value::{RuntimeEvalError, RuntimeExactInteger, RuntimeExpressionFailure};

    #[test]
    fn checked_fault_outcomes_leave_uninitialized_result_slots_unread() {
        // Neither callback initializes result storage. Faults and unknown
        // statuses must return before the invocation boundary reads that slot.
        assert!(matches!(
            scalar_result::<u64>(1, |_, _| 1),
            Err(CraneliftCodegenError::Execution {
                error: RuntimeEvalError::RecoverableExpression(
                    RuntimeExpressionFailure::DivisionByZero
                ),
                completed_rows: 0,
            })
        ));
        assert!(matches!(
            scalar_result::<u64>(1, |_, _| 77),
            Err(CraneliftCodegenError::InvalidNativeOutcome { code: 77, .. })
        ));
        assert!(matches!(
            sum_result::<u128>(
                2,
                |value| value.try_sum_as_i64("checked_fault_boundary"),
                |_, _, _| 1
            ),
            Err(CraneliftCodegenError::Execution {
                error: RuntimeEvalError::RecoverableExpression(
                    RuntimeExpressionFailure::DivisionByZero
                ),
                completed_rows: 0,
            })
        ));
        assert!(matches!(
            sum_result::<u128>(
                2,
                |value| value.try_sum_as_i64("checked_fault_boundary"),
                |_, _, _| 77
            ),
            Err(CraneliftCodegenError::InvalidNativeOutcome { code: 77, .. })
        ));
    }
}
