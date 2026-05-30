use std::mem;

type JitI64Fn = extern "C" fn() -> i64;
type JitI64UnaryFn = extern "C" fn(i64) -> i64;
type JitI64BinaryFn = extern "C" fn(i64, i64) -> i64;
type JitI64TernaryFn = extern "C" fn(i64, i64, i64) -> i64;
type JitI64QuaternaryFn = extern "C" fn(i64, i64, i64, i64) -> i64;
type JitI64BatchFn = extern "C" fn(i64, i64, i64) -> i64;
type JitI64RowsBatchFn = extern "C" fn(*const i64, i64, *mut i64);

#[derive(Clone, Copy)]
pub(crate) enum I64InputCaller {
    Nullary(JitI64Fn),
    Unary(JitI64UnaryFn),
    Binary(JitI64BinaryFn),
    Ternary(JitI64TernaryFn),
    Quaternary(JitI64QuaternaryFn),
}

impl I64InputCaller {
    pub(crate) fn from_code(code: *const u8, arity: usize) -> Option<Self> {
        match arity {
            0 => {
                // SAFETY: `code` is returned by `JITModule::get_finalized_function`
                // for a function emitted in this crate with signature
                // `extern "C" fn() -> i64`. The owning `JITModule` is stored
                // next to this typed caller and outlives every call through it.
                let function = unsafe { mem::transmute::<*const u8, JitI64Fn>(code) };
                Some(Self::Nullary(function))
            }
            1 => {
                // SAFETY: `code` is returned by `JITModule::get_finalized_function`
                // for a function emitted in this crate with signature
                // `extern "C" fn(i64) -> i64`. The owning `JITModule` is stored
                // next to this typed caller and outlives every call through it.
                let function = unsafe { mem::transmute::<*const u8, JitI64UnaryFn>(code) };
                Some(Self::Unary(function))
            }
            2 => {
                // SAFETY: `code` is returned by `JITModule::get_finalized_function`
                // for a function emitted in this crate with signature
                // `extern "C" fn(i64, i64) -> i64`. The owning `JITModule` is
                // stored next to this typed caller and outlives every call
                // through it.
                let function = unsafe { mem::transmute::<*const u8, JitI64BinaryFn>(code) };
                Some(Self::Binary(function))
            }
            3 => {
                // SAFETY: `code` is returned by `JITModule::get_finalized_function`
                // for a function emitted in this crate with signature
                // `extern "C" fn(i64, i64, i64) -> i64`. The owning `JITModule`
                // is stored next to this typed caller and outlives every call
                // through it.
                let function = unsafe { mem::transmute::<*const u8, JitI64TernaryFn>(code) };
                Some(Self::Ternary(function))
            }
            4 => {
                // SAFETY: `code` is returned by `JITModule::get_finalized_function`
                // for a function emitted in this crate with signature
                // `extern "C" fn(i64, i64, i64, i64) -> i64`. The owning
                // `JITModule` is stored next to this typed caller and outlives
                // every call through it.
                let function = unsafe { mem::transmute::<*const u8, JitI64QuaternaryFn>(code) };
                Some(Self::Quaternary(function))
            }
            _ => None,
        }
    }

    pub(crate) fn call(self, inputs: &[i64]) -> Option<i64> {
        match (self, inputs) {
            (Self::Nullary(function), []) => Some(function()),
            (Self::Unary(function), [value]) => Some(function(*value)),
            (Self::Binary(function), [lhs, rhs]) => Some(function(*lhs, *rhs)),
            (Self::Ternary(function), [a, b, c]) => Some(function(*a, *b, *c)),
            (Self::Quaternary(function), [a, b, c, d]) => Some(function(*a, *b, *c, *d)),
            _ => None,
        }
    }
}

pub(crate) fn call_i64(code: *const u8) -> i64 {
    // SAFETY: `code` is returned by `JITModule::get_finalized_function` for a
    // function emitted in this crate with signature `extern "C" fn() -> i64`.
    // The owning `JITModule` is kept alive by the caller until after the call.
    let function = unsafe { mem::transmute::<*const u8, JitI64Fn>(code) };
    function()
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
