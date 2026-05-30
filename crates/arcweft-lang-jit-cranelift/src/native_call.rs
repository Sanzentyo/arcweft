use std::mem;

type JitI64Fn = extern "C" fn() -> i64;
type JitI64UnaryFn = extern "C" fn(i64) -> i64;
type JitI64BinaryFn = extern "C" fn(i64, i64) -> i64;
type JitI64TernaryFn = extern "C" fn(i64, i64, i64) -> i64;
type JitI64QuaternaryFn = extern "C" fn(i64, i64, i64, i64) -> i64;
type JitI64BatchFn = extern "C" fn(i64, i64, i64) -> i64;
type JitI64RowsBatchFn = extern "C" fn(*const i64, i64, *mut i64);

pub(crate) fn call_i64(code: *const u8) -> i64 {
    // SAFETY: `code` is returned by `JITModule::get_finalized_function` for a
    // function emitted in this crate with signature `extern "C" fn() -> i64`.
    // The owning `JITModule` is kept alive by the caller until after the call.
    let function = unsafe { mem::transmute::<*const u8, JitI64Fn>(code) };
    function()
}

pub(crate) fn call_i64_inputs(code: *const u8, inputs: &[i64]) -> Option<i64> {
    match inputs {
        [] => Some(call_i64(code)),
        [value] => {
            // SAFETY: `code` is returned by `JITModule::get_finalized_function`
            // for a function emitted in this crate with signature
            // `extern "C" fn(i64) -> i64`. The owning `JITModule` is kept alive
            // by the caller until after the call.
            let function = unsafe { mem::transmute::<*const u8, JitI64UnaryFn>(code) };
            Some(function(*value))
        }
        [lhs, rhs] => {
            // SAFETY: `code` is returned by `JITModule::get_finalized_function`
            // for a function emitted in this crate with signature
            // `extern "C" fn(i64, i64) -> i64`. The owning `JITModule` is kept
            // alive by the caller until after the call.
            let function = unsafe { mem::transmute::<*const u8, JitI64BinaryFn>(code) };
            Some(function(*lhs, *rhs))
        }
        [a, b, c] => {
            // SAFETY: `code` is returned by `JITModule::get_finalized_function`
            // for a function emitted in this crate with signature
            // `extern "C" fn(i64, i64, i64) -> i64`. The owning `JITModule` is
            // kept alive by the caller until after the call.
            let function = unsafe { mem::transmute::<*const u8, JitI64TernaryFn>(code) };
            Some(function(*a, *b, *c))
        }
        [a, b, c, d] => {
            // SAFETY: `code` is returned by `JITModule::get_finalized_function`
            // for a function emitted in this crate with signature
            // `extern "C" fn(i64, i64, i64, i64) -> i64`. The owning
            // `JITModule` is kept alive by the caller until after the call.
            let function = unsafe { mem::transmute::<*const u8, JitI64QuaternaryFn>(code) };
            Some(function(*a, *b, *c, *d))
        }
        _ => None,
    }
}

pub(crate) fn call_i64_batch(code: *const u8, seed: i64, sample: i64, iterations: i64) -> i64 {
    // SAFETY: `code` is returned by `JITModule::get_finalized_function` for a
    // function emitted in this crate with signature
    // `extern "C" fn(i64, i64, i64) -> i64`. The owning `JITModule` is kept
    // alive by the caller until after the call.
    let function = unsafe { mem::transmute::<*const u8, JitI64BatchFn>(code) };
    function(seed, sample, iterations)
}

pub(crate) fn call_i64_rows_batch(
    code: *const u8,
    inputs: &[i64],
    arity: usize,
    out: &mut [i64],
) -> bool {
    if inputs.len() != arity.saturating_mul(out.len()) {
        return false;
    }
    let Ok(rows) = i64::try_from(out.len()) else {
        return false;
    };
    // SAFETY: `code` is returned by `JITModule::get_finalized_function` for a
    // function emitted in this crate with signature
    // `extern "C" fn(*const i64, i64, *mut i64)`. The owning `JITModule` is kept
    // alive by the caller until after the call, and the slices are checked to
    // cover the row/arity shape before their pointers are passed.
    let function = unsafe { mem::transmute::<*const u8, JitI64RowsBatchFn>(code) };
    function(inputs.as_ptr(), rows, out.as_mut_ptr());
    true
}
