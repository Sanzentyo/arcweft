use std::mem;

type JitI64Fn = extern "C" fn() -> i64;
type JitI64UnaryFn = extern "C" fn(i64) -> i64;
type JitI64BinaryFn = extern "C" fn(i64, i64) -> i64;
type JitI64TernaryFn = extern "C" fn(i64, i64, i64) -> i64;
type JitI64QuaternaryFn = extern "C" fn(i64, i64, i64, i64) -> i64;
type JitI64BatchFn = extern "C" fn(i64, i64, i64) -> i64;
type JitI64RowsBatchFn = extern "C" fn(*const i64, i64, *mut i64);
type JitI64RowsBatchSumFn = extern "C" fn(*const i64, i64) -> i64;
type JitI32Fn = extern "C" fn() -> i32;
type JitI32UnaryFn = extern "C" fn(i32) -> i32;
type JitI32BinaryFn = extern "C" fn(i32, i32) -> i32;
type JitI32TernaryFn = extern "C" fn(i32, i32, i32) -> i32;
type JitI32QuaternaryFn = extern "C" fn(i32, i32, i32, i32) -> i32;
type JitI32RowsBatchFn = extern "C" fn(*const i32, i64, *mut i32);
type JitI32RowsBatchSumFn = extern "C" fn(*const i32, i64) -> i64;
type JitF32Fn = extern "C" fn() -> f32;
type JitF32UnaryFn = extern "C" fn(f32) -> f32;
type JitF32BinaryFn = extern "C" fn(f32, f32) -> f32;
type JitF32TernaryFn = extern "C" fn(f32, f32, f32) -> f32;
type JitF32QuaternaryFn = extern "C" fn(f32, f32, f32, f32) -> f32;
type JitF32RowsBatchFn = extern "C" fn(*const f32, i64, *mut f32);
type JitF64Fn = extern "C" fn() -> f64;
type JitF64UnaryFn = extern "C" fn(f64) -> f64;
type JitF64BinaryFn = extern "C" fn(f64, f64) -> f64;
type JitF64TernaryFn = extern "C" fn(f64, f64, f64) -> f64;
type JitF64QuaternaryFn = extern "C" fn(f64, f64, f64, f64) -> f64;
type JitF64RowsBatchFn = extern "C" fn(*const f64, i64, *mut f64);

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

    pub(crate) fn call_packed(self, values: [i64; 4], len: usize) -> Option<i64> {
        match (self, len) {
            (Self::Nullary(function), 0) => Some(function()),
            (Self::Unary(function), 1) => Some(function(values[0])),
            (Self::Binary(function), 2) => Some(function(values[0], values[1])),
            (Self::Ternary(function), 3) => Some(function(values[0], values[1], values[2])),
            (Self::Quaternary(function), 4) => {
                Some(function(values[0], values[1], values[2], values[3]))
            }
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum I32InputCaller {
    Nullary(JitI32Fn),
    Unary(JitI32UnaryFn),
    Binary(JitI32BinaryFn),
    Ternary(JitI32TernaryFn),
    Quaternary(JitI32QuaternaryFn),
}

impl I32InputCaller {
    pub(crate) fn from_code(code: *const u8, arity: usize) -> Option<Self> {
        match arity {
            0 => {
                // SAFETY: `code` is emitted in this crate with signature
                // `extern "C" fn() -> i32`, and the owning JIT module is
                // stored next to the typed caller.
                let function = unsafe { mem::transmute::<*const u8, JitI32Fn>(code) };
                Some(Self::Nullary(function))
            }
            1 => {
                // SAFETY: see the nullary case; the emitted signature is
                // `extern "C" fn(i32) -> i32`.
                let function = unsafe { mem::transmute::<*const u8, JitI32UnaryFn>(code) };
                Some(Self::Unary(function))
            }
            2 => {
                // SAFETY: see the nullary case; the emitted signature is
                // `extern "C" fn(i32, i32) -> i32`.
                let function = unsafe { mem::transmute::<*const u8, JitI32BinaryFn>(code) };
                Some(Self::Binary(function))
            }
            3 => {
                // SAFETY: see the nullary case; the emitted signature is
                // `extern "C" fn(i32, i32, i32) -> i32`.
                let function = unsafe { mem::transmute::<*const u8, JitI32TernaryFn>(code) };
                Some(Self::Ternary(function))
            }
            4 => {
                // SAFETY: see the nullary case; the emitted signature is
                // `extern "C" fn(i32, i32, i32, i32) -> i32`.
                let function = unsafe { mem::transmute::<*const u8, JitI32QuaternaryFn>(code) };
                Some(Self::Quaternary(function))
            }
            _ => None,
        }
    }

    pub(crate) fn call(self, inputs: &[i32]) -> Option<i32> {
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

#[derive(Clone, Copy)]
pub(crate) enum F32InputCaller {
    Nullary(JitF32Fn),
    Unary(JitF32UnaryFn),
    Binary(JitF32BinaryFn),
    Ternary(JitF32TernaryFn),
    Quaternary(JitF32QuaternaryFn),
}

impl F32InputCaller {
    pub(crate) fn from_code(code: *const u8, arity: usize) -> Option<Self> {
        match arity {
            0 => {
                // SAFETY: `code` is emitted in this crate with signature
                // `extern "C" fn() -> f32`, and the owning JIT module is
                // stored next to the typed caller.
                let function = unsafe { mem::transmute::<*const u8, JitF32Fn>(code) };
                Some(Self::Nullary(function))
            }
            1 => {
                // SAFETY: see the nullary case; the emitted signature is
                // `extern "C" fn(f32) -> f32`.
                let function = unsafe { mem::transmute::<*const u8, JitF32UnaryFn>(code) };
                Some(Self::Unary(function))
            }
            2 => {
                // SAFETY: see the nullary case; the emitted signature is
                // `extern "C" fn(f32, f32) -> f32`.
                let function = unsafe { mem::transmute::<*const u8, JitF32BinaryFn>(code) };
                Some(Self::Binary(function))
            }
            3 => {
                // SAFETY: see the nullary case; the emitted signature is
                // `extern "C" fn(f32, f32, f32) -> f32`.
                let function = unsafe { mem::transmute::<*const u8, JitF32TernaryFn>(code) };
                Some(Self::Ternary(function))
            }
            4 => {
                // SAFETY: see the nullary case; the emitted signature is
                // `extern "C" fn(f32, f32, f32, f32) -> f32`.
                let function = unsafe { mem::transmute::<*const u8, JitF32QuaternaryFn>(code) };
                Some(Self::Quaternary(function))
            }
            _ => None,
        }
    }

    pub(crate) fn call(self, inputs: &[f32]) -> Option<f32> {
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

#[derive(Clone, Copy)]
pub(crate) enum F64InputCaller {
    Nullary(JitF64Fn),
    Unary(JitF64UnaryFn),
    Binary(JitF64BinaryFn),
    Ternary(JitF64TernaryFn),
    Quaternary(JitF64QuaternaryFn),
}

impl F64InputCaller {
    pub(crate) fn from_code(code: *const u8, arity: usize) -> Option<Self> {
        match arity {
            0 => {
                // SAFETY: `code` is emitted in this crate with signature
                // `extern "C" fn() -> f64`, and the owning JIT module is
                // stored next to the typed caller.
                let function = unsafe { mem::transmute::<*const u8, JitF64Fn>(code) };
                Some(Self::Nullary(function))
            }
            1 => {
                // SAFETY: see the nullary case; the emitted signature is
                // `extern "C" fn(f64) -> f64`.
                let function = unsafe { mem::transmute::<*const u8, JitF64UnaryFn>(code) };
                Some(Self::Unary(function))
            }
            2 => {
                // SAFETY: see the nullary case; the emitted signature is
                // `extern "C" fn(f64, f64) -> f64`.
                let function = unsafe { mem::transmute::<*const u8, JitF64BinaryFn>(code) };
                Some(Self::Binary(function))
            }
            3 => {
                // SAFETY: see the nullary case; the emitted signature is
                // `extern "C" fn(f64, f64, f64) -> f64`.
                let function = unsafe { mem::transmute::<*const u8, JitF64TernaryFn>(code) };
                Some(Self::Ternary(function))
            }
            4 => {
                // SAFETY: see the nullary case; the emitted signature is
                // `extern "C" fn(f64, f64, f64, f64) -> f64`.
                let function = unsafe { mem::transmute::<*const u8, JitF64QuaternaryFn>(code) };
                Some(Self::Quaternary(function))
            }
            _ => None,
        }
    }

    pub(crate) fn call(self, inputs: &[f64]) -> Option<f64> {
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

pub(crate) fn call_i64_rows_batch_sum(
    code: *const u8,
    inputs: &[i64],
    arity: usize,
    rows: usize,
) -> Option<i64> {
    if inputs.len() != arity.saturating_mul(rows) {
        return None;
    }
    let Ok(rows) = i64::try_from(rows) else {
        return None;
    };
    // SAFETY: `code` is returned by `JITModule::get_finalized_function` for a
    // function emitted in this crate with signature
    // `extern "C" fn(*const i64, i64) -> i64`. The owning `JITModule` is kept
    // alive by the caller until after the call, and the input slice is checked
    // to cover the row/arity shape before its pointer is passed.
    let function = unsafe { mem::transmute::<*const u8, JitI64RowsBatchSumFn>(code) };
    Some(function(inputs.as_ptr(), rows))
}

pub(crate) fn call_i32_rows_batch(
    code: *const u8,
    inputs: &[i32],
    arity: usize,
    out: &mut [i32],
) -> bool {
    if inputs.len() != arity.saturating_mul(out.len()) {
        return false;
    }
    let Ok(rows) = i64::try_from(out.len()) else {
        return false;
    };
    // SAFETY: `code` is returned by `JITModule::get_finalized_function` for a
    // function emitted in this crate with signature
    // `extern "C" fn(*const i32, i64, *mut i32)`. Slice shape is checked before
    // passing pointers, and the owning JIT module outlives the call.
    let function = unsafe { mem::transmute::<*const u8, JitI32RowsBatchFn>(code) };
    function(inputs.as_ptr(), rows, out.as_mut_ptr());
    true
}

pub(crate) fn call_i32_rows_batch_sum(
    code: *const u8,
    inputs: &[i32],
    arity: usize,
    rows: usize,
) -> Option<i64> {
    if inputs.len() != arity.saturating_mul(rows) {
        return None;
    }
    let Ok(rows) = i64::try_from(rows) else {
        return None;
    };
    // SAFETY: `code` is returned by `JITModule::get_finalized_function` for a
    // function emitted in this crate with signature
    // `extern "C" fn(*const i32, i64) -> i64`. Slice shape is checked before
    // passing pointers, and the owning JIT module outlives the call.
    let function = unsafe { mem::transmute::<*const u8, JitI32RowsBatchSumFn>(code) };
    Some(function(inputs.as_ptr(), rows))
}

pub(crate) fn call_f32_rows_batch(
    code: *const u8,
    inputs: &[f32],
    arity: usize,
    out: &mut [f32],
) -> bool {
    if inputs.len() != arity.saturating_mul(out.len()) {
        return false;
    }
    let Ok(rows) = i64::try_from(out.len()) else {
        return false;
    };
    // SAFETY: `code` is returned by `JITModule::get_finalized_function` for a
    // function emitted in this crate with signature
    // `extern "C" fn(*const f32, i64, *mut f32)`. Slice shape is checked before
    // passing pointers, and the owning JIT module outlives the call.
    let function = unsafe { mem::transmute::<*const u8, JitF32RowsBatchFn>(code) };
    function(inputs.as_ptr(), rows, out.as_mut_ptr());
    true
}

pub(crate) fn call_f64_rows_batch(
    code: *const u8,
    inputs: &[f64],
    arity: usize,
    out: &mut [f64],
) -> bool {
    if inputs.len() != arity.saturating_mul(out.len()) {
        return false;
    }
    let Ok(rows) = i64::try_from(out.len()) else {
        return false;
    };
    // SAFETY: `code` is returned by `JITModule::get_finalized_function` for a
    // function emitted in this crate with signature
    // `extern "C" fn(*const f64, i64, *mut f64)`. Slice shape is checked before
    // passing pointers, and the owning JIT module outlives the call.
    let function = unsafe { mem::transmute::<*const u8, JitF64RowsBatchFn>(code) };
    function(inputs.as_ptr(), rows, out.as_mut_ptr());
    true
}
