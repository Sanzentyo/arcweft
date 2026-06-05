use arcweft_core::value::{RuntimeISizeValue, RuntimeUSizeValue};
use std::mem;

type JitI64Fn = extern "C" fn() -> i64;
type JitI64UnaryFn = extern "C" fn(i64) -> i64;
type JitI64BinaryFn = extern "C" fn(i64, i64) -> i64;
type JitI64TernaryFn = extern "C" fn(i64, i64, i64) -> i64;
type JitI64QuaternaryFn = extern "C" fn(i64, i64, i64, i64) -> i64;
type JitI64BatchFn = extern "C" fn(i64, i64, i64) -> i64;
type JitI64RowsBatchFn = extern "C" fn(*const i64, i64, *mut i64);
type JitI64RowsBatchSumFn = extern "C" fn(*const i64, i64) -> i64;
type JitISizeRowsBatchFn = extern "C" fn(*const RuntimeISizeValue, i64, *mut RuntimeISizeValue);
type JitISizeRowsBatchSumFn = extern "C" fn(*const RuntimeISizeValue, i64) -> i64;
type JitI128RowsBatchFn = extern "C" fn(*const i128, i64, *mut i128);
type JitI128RowsBatchSumFn = extern "C" fn(*const i128, i64) -> i64;
type JitI8Fn = extern "C" fn() -> i8;
type JitI8UnaryFn = extern "C" fn(i8) -> i8;
type JitI8BinaryFn = extern "C" fn(i8, i8) -> i8;
type JitI8TernaryFn = extern "C" fn(i8, i8, i8) -> i8;
type JitI8QuaternaryFn = extern "C" fn(i8, i8, i8, i8) -> i8;
type JitI8RowsBatchFn = extern "C" fn(*const i8, i64, *mut i8);
type JitI8RowsBatchSumFn = extern "C" fn(*const i8, i64) -> i64;
type JitI16Fn = extern "C" fn() -> i16;
type JitI16UnaryFn = extern "C" fn(i16) -> i16;
type JitI16BinaryFn = extern "C" fn(i16, i16) -> i16;
type JitI16TernaryFn = extern "C" fn(i16, i16, i16) -> i16;
type JitI16QuaternaryFn = extern "C" fn(i16, i16, i16, i16) -> i16;
type JitI16RowsBatchFn = extern "C" fn(*const i16, i64, *mut i16);
type JitI16RowsBatchSumFn = extern "C" fn(*const i16, i64) -> i64;
type JitI32Fn = extern "C" fn() -> i32;
type JitI32UnaryFn = extern "C" fn(i32) -> i32;
type JitI32BinaryFn = extern "C" fn(i32, i32) -> i32;
type JitI32TernaryFn = extern "C" fn(i32, i32, i32) -> i32;
type JitI32QuaternaryFn = extern "C" fn(i32, i32, i32, i32) -> i32;
type JitI32RowsBatchFn = extern "C" fn(*const i32, i64, *mut i32);
type JitI32RowsBatchSumFn = extern "C" fn(*const i32, i64) -> i64;
type JitU32Fn = extern "C" fn() -> u32;
type JitU32UnaryFn = extern "C" fn(u32) -> u32;
type JitU32BinaryFn = extern "C" fn(u32, u32) -> u32;
type JitU32TernaryFn = extern "C" fn(u32, u32, u32) -> u32;
type JitU32QuaternaryFn = extern "C" fn(u32, u32, u32, u32) -> u32;
type JitU32RowsBatchFn = extern "C" fn(*const u32, i64, *mut u32);
type JitU32RowsBatchSumFn = extern "C" fn(*const u32, i64) -> i64;
type JitU8Fn = extern "C" fn() -> u8;
type JitU8UnaryFn = extern "C" fn(u8) -> u8;
type JitU8BinaryFn = extern "C" fn(u8, u8) -> u8;
type JitU8TernaryFn = extern "C" fn(u8, u8, u8) -> u8;
type JitU8QuaternaryFn = extern "C" fn(u8, u8, u8, u8) -> u8;
type JitU8RowsBatchFn = extern "C" fn(*const u8, i64, *mut u8);
type JitU8RowsBatchSumFn = extern "C" fn(*const u8, i64) -> i64;
type JitU16Fn = extern "C" fn() -> u16;
type JitU16UnaryFn = extern "C" fn(u16) -> u16;
type JitU16BinaryFn = extern "C" fn(u16, u16) -> u16;
type JitU16TernaryFn = extern "C" fn(u16, u16, u16) -> u16;
type JitU16QuaternaryFn = extern "C" fn(u16, u16, u16, u16) -> u16;
type JitU16RowsBatchFn = extern "C" fn(*const u16, i64, *mut u16);
type JitU16RowsBatchSumFn = extern "C" fn(*const u16, i64) -> i64;
type JitU64Fn = extern "C" fn() -> u64;
type JitU64UnaryFn = extern "C" fn(u64) -> u64;
type JitU64BinaryFn = extern "C" fn(u64, u64) -> u64;
type JitU64TernaryFn = extern "C" fn(u64, u64, u64) -> u64;
type JitU64QuaternaryFn = extern "C" fn(u64, u64, u64, u64) -> u64;
type JitU64RowsBatchFn = extern "C" fn(*const u64, i64, *mut u64);
type JitU64RowsBatchSumFn = extern "C" fn(*const u64, i64) -> i64;
type JitUSizeRowsBatchFn = extern "C" fn(*const RuntimeUSizeValue, i64, *mut RuntimeUSizeValue);
type JitUSizeRowsBatchSumFn = extern "C" fn(*const RuntimeUSizeValue, i64) -> i64;
type JitU128RowsBatchFn = extern "C" fn(*const u128, i64, *mut u128);
type JitU128RowsBatchSumFn = extern "C" fn(*const u128, i64) -> i64;
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

macro_rules! small_int_input_caller {
    (
        $caller:ident,
        $ty:ty,
        $fn0:ty,
        $fn1:ty,
        $fn2:ty,
        $fn3:ty,
        $fn4:ty,
        $label:literal
    ) => {
        #[derive(Clone, Copy)]
        pub(crate) enum $caller {
            Nullary($fn0),
            Unary($fn1),
            Binary($fn2),
            Ternary($fn3),
            Quaternary($fn4),
        }

        impl $caller {
            pub(crate) fn from_code(code: *const u8, arity: usize) -> Option<Self> {
                match arity {
                    0 => {
                        // SAFETY: `code` is emitted in this crate with the
                        // matching `extern "C"` small-integer signature, and
                        // the owning JIT module is stored next to this caller.
                        let function = unsafe { mem::transmute::<*const u8, $fn0>(code) };
                        Some(Self::Nullary(function))
                    }
                    1 => {
                        // SAFETY: see the nullary case; arity selects the
                        // exact emitted function pointer signature.
                        let function = unsafe { mem::transmute::<*const u8, $fn1>(code) };
                        Some(Self::Unary(function))
                    }
                    2 => {
                        // SAFETY: see the nullary case; arity selects the
                        // exact emitted function pointer signature.
                        let function = unsafe { mem::transmute::<*const u8, $fn2>(code) };
                        Some(Self::Binary(function))
                    }
                    3 => {
                        // SAFETY: see the nullary case; arity selects the
                        // exact emitted function pointer signature.
                        let function = unsafe { mem::transmute::<*const u8, $fn3>(code) };
                        Some(Self::Ternary(function))
                    }
                    4 => {
                        // SAFETY: see the nullary case; arity selects the
                        // exact emitted function pointer signature.
                        let function = unsafe { mem::transmute::<*const u8, $fn4>(code) };
                        Some(Self::Quaternary(function))
                    }
                    _ => None,
                }
            }

            pub(crate) fn call(self, inputs: &[$ty]) -> Option<$ty> {
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
    };
}

small_int_input_caller!(
    I8InputCaller,
    i8,
    JitI8Fn,
    JitI8UnaryFn,
    JitI8BinaryFn,
    JitI8TernaryFn,
    JitI8QuaternaryFn,
    "i8"
);
small_int_input_caller!(
    I16InputCaller,
    i16,
    JitI16Fn,
    JitI16UnaryFn,
    JitI16BinaryFn,
    JitI16TernaryFn,
    JitI16QuaternaryFn,
    "i16"
);
small_int_input_caller!(
    U8InputCaller,
    u8,
    JitU8Fn,
    JitU8UnaryFn,
    JitU8BinaryFn,
    JitU8TernaryFn,
    JitU8QuaternaryFn,
    "u8"
);
small_int_input_caller!(
    U16InputCaller,
    u16,
    JitU16Fn,
    JitU16UnaryFn,
    JitU16BinaryFn,
    JitU16TernaryFn,
    JitU16QuaternaryFn,
    "u16"
);

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

    pub(crate) fn call_isize(self, inputs: &[RuntimeISizeValue]) -> Option<RuntimeISizeValue> {
        match (self, inputs) {
            (Self::Nullary(function), []) => Some(RuntimeISizeValue::new(function())),
            (Self::Unary(function), [value]) => Some(RuntimeISizeValue::new(function(value.get()))),
            (Self::Binary(function), [lhs, rhs]) => {
                Some(RuntimeISizeValue::new(function(lhs.get(), rhs.get())))
            }
            (Self::Ternary(function), [a, b, c]) => {
                Some(RuntimeISizeValue::new(function(a.get(), b.get(), c.get())))
            }
            (Self::Quaternary(function), [a, b, c, d]) => Some(RuntimeISizeValue::new(function(
                a.get(),
                b.get(),
                c.get(),
                d.get(),
            ))),
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
pub(crate) enum U32InputCaller {
    Nullary(JitU32Fn),
    Unary(JitU32UnaryFn),
    Binary(JitU32BinaryFn),
    Ternary(JitU32TernaryFn),
    Quaternary(JitU32QuaternaryFn),
}

impl U32InputCaller {
    pub(crate) fn from_code(code: *const u8, arity: usize) -> Option<Self> {
        match arity {
            0 => {
                // SAFETY: `code` is emitted in this crate with signature
                // `extern "C" fn() -> u32`, and the owning JIT module is
                // stored next to the typed caller.
                let function = unsafe { mem::transmute::<*const u8, JitU32Fn>(code) };
                Some(Self::Nullary(function))
            }
            1 => {
                // SAFETY: see the nullary case; the emitted signature is
                // `extern "C" fn(u32) -> u32`.
                let function = unsafe { mem::transmute::<*const u8, JitU32UnaryFn>(code) };
                Some(Self::Unary(function))
            }
            2 => {
                // SAFETY: see the nullary case; the emitted signature is
                // `extern "C" fn(u32, u32) -> u32`.
                let function = unsafe { mem::transmute::<*const u8, JitU32BinaryFn>(code) };
                Some(Self::Binary(function))
            }
            3 => {
                // SAFETY: see the nullary case; the emitted signature is
                // `extern "C" fn(u32, u32, u32) -> u32`.
                let function = unsafe { mem::transmute::<*const u8, JitU32TernaryFn>(code) };
                Some(Self::Ternary(function))
            }
            4 => {
                // SAFETY: see the nullary case; the emitted signature is
                // `extern "C" fn(u32, u32, u32, u32) -> u32`.
                let function = unsafe { mem::transmute::<*const u8, JitU32QuaternaryFn>(code) };
                Some(Self::Quaternary(function))
            }
            _ => None,
        }
    }

    pub(crate) fn call(self, inputs: &[u32]) -> Option<u32> {
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
pub(crate) enum U64InputCaller {
    Nullary(JitU64Fn),
    Unary(JitU64UnaryFn),
    Binary(JitU64BinaryFn),
    Ternary(JitU64TernaryFn),
    Quaternary(JitU64QuaternaryFn),
}

impl U64InputCaller {
    pub(crate) fn from_code(code: *const u8, arity: usize) -> Option<Self> {
        match arity {
            0 => {
                // SAFETY: `code` is emitted in this crate with signature
                // `extern "C" fn() -> u64`, and the owning JIT module is
                // stored next to the typed caller.
                let function = unsafe { mem::transmute::<*const u8, JitU64Fn>(code) };
                Some(Self::Nullary(function))
            }
            1 => {
                // SAFETY: see the nullary case; the emitted signature is
                // `extern "C" fn(u64) -> u64`.
                let function = unsafe { mem::transmute::<*const u8, JitU64UnaryFn>(code) };
                Some(Self::Unary(function))
            }
            2 => {
                // SAFETY: see the nullary case; the emitted signature is
                // `extern "C" fn(u64, u64) -> u64`.
                let function = unsafe { mem::transmute::<*const u8, JitU64BinaryFn>(code) };
                Some(Self::Binary(function))
            }
            3 => {
                // SAFETY: see the nullary case; the emitted signature is
                // `extern "C" fn(u64, u64, u64) -> u64`.
                let function = unsafe { mem::transmute::<*const u8, JitU64TernaryFn>(code) };
                Some(Self::Ternary(function))
            }
            4 => {
                // SAFETY: see the nullary case; the emitted signature is
                // `extern "C" fn(u64, u64, u64, u64) -> u64`.
                let function = unsafe { mem::transmute::<*const u8, JitU64QuaternaryFn>(code) };
                Some(Self::Quaternary(function))
            }
            _ => None,
        }
    }

    pub(crate) fn call(self, inputs: &[u64]) -> Option<u64> {
        match (self, inputs) {
            (Self::Nullary(function), []) => Some(function()),
            (Self::Unary(function), [value]) => Some(function(*value)),
            (Self::Binary(function), [lhs, rhs]) => Some(function(*lhs, *rhs)),
            (Self::Ternary(function), [a, b, c]) => Some(function(*a, *b, *c)),
            (Self::Quaternary(function), [a, b, c, d]) => Some(function(*a, *b, *c, *d)),
            _ => None,
        }
    }

    pub(crate) fn call_usize(self, inputs: &[RuntimeUSizeValue]) -> Option<RuntimeUSizeValue> {
        match (self, inputs) {
            (Self::Nullary(function), []) => Some(RuntimeUSizeValue::new(function())),
            (Self::Unary(function), [value]) => Some(RuntimeUSizeValue::new(function(value.get()))),
            (Self::Binary(function), [lhs, rhs]) => {
                Some(RuntimeUSizeValue::new(function(lhs.get(), rhs.get())))
            }
            (Self::Ternary(function), [a, b, c]) => {
                Some(RuntimeUSizeValue::new(function(a.get(), b.get(), c.get())))
            }
            (Self::Quaternary(function), [a, b, c, d]) => Some(RuntimeUSizeValue::new(function(
                a.get(),
                b.get(),
                c.get(),
                d.get(),
            ))),
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

pub(crate) fn call_isize_rows_batch(
    code: *const u8,
    inputs: &[RuntimeISizeValue],
    arity: usize,
    out: &mut [RuntimeISizeValue],
) -> bool {
    if inputs.len() != arity.saturating_mul(out.len()) {
        return false;
    }
    let Ok(rows) = i64::try_from(out.len()) else {
        return false;
    };
    // SAFETY: `code` is emitted with the same pointer ABI as the i64 row
    // batch entrypoint. `RuntimeISizeValue` is `repr(transparent)` over i64,
    // and the JIT function only reads/writes checked row-major element slots.
    let function = unsafe { mem::transmute::<*const u8, JitISizeRowsBatchFn>(code) };
    function(inputs.as_ptr(), rows, out.as_mut_ptr());
    true
}

pub(crate) fn call_isize_rows_batch_sum(
    code: *const u8,
    inputs: &[RuntimeISizeValue],
    arity: usize,
    rows: usize,
) -> Option<i64> {
    if inputs.len() != arity.saturating_mul(rows) {
        return None;
    }
    let Ok(rows) = i64::try_from(rows) else {
        return None;
    };
    // SAFETY: `code` is emitted with the same pointer ABI as the i64 row
    // batch-sum entrypoint. `RuntimeISizeValue` is `repr(transparent)` over
    // i64, and slice shape is checked before passing the pointer.
    let function = unsafe { mem::transmute::<*const u8, JitISizeRowsBatchSumFn>(code) };
    Some(function(inputs.as_ptr(), rows))
}

pub(crate) fn call_i128_rows_batch(
    code: *const u8,
    inputs: &[i128],
    arity: usize,
    out: &mut [i128],
) -> bool {
    if inputs.len() != arity.saturating_mul(out.len()) {
        return false;
    }
    let Ok(rows) = i64::try_from(out.len()) else {
        return false;
    };
    // SAFETY: `code` is emitted in this crate with signature
    // `extern "C" fn(*const i128, i64, *mut i128)`. Only pointers and `i64`
    // cross the ABI boundary; the generated function loads/stores i128 values
    // from checked slices while the owning JIT module outlives the call.
    let function = unsafe { mem::transmute::<*const u8, JitI128RowsBatchFn>(code) };
    function(inputs.as_ptr(), rows, out.as_mut_ptr());
    true
}

pub(crate) fn call_i128_rows_batch_sum(
    code: *const u8,
    inputs: &[i128],
    arity: usize,
    rows: usize,
) -> Option<i64> {
    if inputs.len() != arity.saturating_mul(rows) {
        return None;
    }
    let Ok(rows) = i64::try_from(rows) else {
        return None;
    };
    // SAFETY: `code` is emitted in this crate with signature
    // `extern "C" fn(*const i128, i64) -> i64`. Slice shape is checked before
    // passing pointers, and no by-value i128 crosses the ABI boundary.
    let function = unsafe { mem::transmute::<*const u8, JitI128RowsBatchSumFn>(code) };
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

pub(crate) fn call_i8_rows_batch(
    code: *const u8,
    inputs: &[i8],
    arity: usize,
    out: &mut [i8],
) -> bool {
    if inputs.len() != arity.saturating_mul(out.len()) {
        return false;
    }
    let Ok(rows) = i64::try_from(out.len()) else {
        return false;
    };
    // SAFETY: `code` is emitted in this crate with signature
    // `extern "C" fn(*const i8, i64, *mut i8)`. Slice shape is checked before
    // passing pointers, and the owning JIT module outlives the call.
    let function = unsafe { mem::transmute::<*const u8, JitI8RowsBatchFn>(code) };
    function(inputs.as_ptr(), rows, out.as_mut_ptr());
    true
}

pub(crate) fn call_i8_rows_batch_sum(
    code: *const u8,
    inputs: &[i8],
    arity: usize,
    rows: usize,
) -> Option<i64> {
    if inputs.len() != arity.saturating_mul(rows) {
        return None;
    }
    let Ok(rows) = i64::try_from(rows) else {
        return None;
    };
    // SAFETY: `code` is emitted in this crate with signature
    // `extern "C" fn(*const i8, i64) -> i64`. Slice shape is checked before
    // passing pointers, and the owning JIT module outlives the call.
    let function = unsafe { mem::transmute::<*const u8, JitI8RowsBatchSumFn>(code) };
    Some(function(inputs.as_ptr(), rows))
}

pub(crate) fn call_i16_rows_batch(
    code: *const u8,
    inputs: &[i16],
    arity: usize,
    out: &mut [i16],
) -> bool {
    if inputs.len() != arity.saturating_mul(out.len()) {
        return false;
    }
    let Ok(rows) = i64::try_from(out.len()) else {
        return false;
    };
    // SAFETY: `code` is emitted in this crate with signature
    // `extern "C" fn(*const i16, i64, *mut i16)`. Slice shape is checked before
    // passing pointers, and the owning JIT module outlives the call.
    let function = unsafe { mem::transmute::<*const u8, JitI16RowsBatchFn>(code) };
    function(inputs.as_ptr(), rows, out.as_mut_ptr());
    true
}

pub(crate) fn call_i16_rows_batch_sum(
    code: *const u8,
    inputs: &[i16],
    arity: usize,
    rows: usize,
) -> Option<i64> {
    if inputs.len() != arity.saturating_mul(rows) {
        return None;
    }
    let Ok(rows) = i64::try_from(rows) else {
        return None;
    };
    // SAFETY: `code` is emitted in this crate with signature
    // `extern "C" fn(*const i16, i64) -> i64`. Slice shape is checked before
    // passing pointers, and the owning JIT module outlives the call.
    let function = unsafe { mem::transmute::<*const u8, JitI16RowsBatchSumFn>(code) };
    Some(function(inputs.as_ptr(), rows))
}

pub(crate) fn call_u32_rows_batch(
    code: *const u8,
    inputs: &[u32],
    arity: usize,
    out: &mut [u32],
) -> bool {
    if inputs.len() != arity.saturating_mul(out.len()) {
        return false;
    }
    let Ok(rows) = i64::try_from(out.len()) else {
        return false;
    };
    // SAFETY: `code` is returned by `JITModule::get_finalized_function` for a
    // function emitted in this crate with signature
    // `extern "C" fn(*const u32, i64, *mut u32)`. Slice shape is checked before
    // passing pointers, and the owning JIT module outlives the call.
    let function = unsafe { mem::transmute::<*const u8, JitU32RowsBatchFn>(code) };
    function(inputs.as_ptr(), rows, out.as_mut_ptr());
    true
}

pub(crate) fn call_u32_rows_batch_sum(
    code: *const u8,
    inputs: &[u32],
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
    // `extern "C" fn(*const u32, i64) -> i64`. Slice shape is checked before
    // passing pointers, and the owning JIT module outlives the call.
    let function = unsafe { mem::transmute::<*const u8, JitU32RowsBatchSumFn>(code) };
    Some(function(inputs.as_ptr(), rows))
}

pub(crate) fn call_u8_rows_batch(
    code: *const u8,
    inputs: &[u8],
    arity: usize,
    out: &mut [u8],
) -> bool {
    if inputs.len() != arity.saturating_mul(out.len()) {
        return false;
    }
    let Ok(rows) = i64::try_from(out.len()) else {
        return false;
    };
    // SAFETY: `code` is emitted in this crate with signature
    // `extern "C" fn(*const u8, i64, *mut u8)`. Slice shape is checked before
    // passing pointers, and the owning JIT module outlives the call.
    let function = unsafe { mem::transmute::<*const u8, JitU8RowsBatchFn>(code) };
    function(inputs.as_ptr(), rows, out.as_mut_ptr());
    true
}

pub(crate) fn call_u8_rows_batch_sum(
    code: *const u8,
    inputs: &[u8],
    arity: usize,
    rows: usize,
) -> Option<i64> {
    if inputs.len() != arity.saturating_mul(rows) {
        return None;
    }
    let Ok(rows) = i64::try_from(rows) else {
        return None;
    };
    // SAFETY: `code` is emitted in this crate with signature
    // `extern "C" fn(*const u8, i64) -> i64`. Slice shape is checked before
    // passing pointers, and the owning JIT module outlives the call.
    let function = unsafe { mem::transmute::<*const u8, JitU8RowsBatchSumFn>(code) };
    Some(function(inputs.as_ptr(), rows))
}

pub(crate) fn call_u16_rows_batch(
    code: *const u8,
    inputs: &[u16],
    arity: usize,
    out: &mut [u16],
) -> bool {
    if inputs.len() != arity.saturating_mul(out.len()) {
        return false;
    }
    let Ok(rows) = i64::try_from(out.len()) else {
        return false;
    };
    // SAFETY: `code` is emitted in this crate with signature
    // `extern "C" fn(*const u16, i64, *mut u16)`. Slice shape is checked before
    // passing pointers, and the owning JIT module outlives the call.
    let function = unsafe { mem::transmute::<*const u8, JitU16RowsBatchFn>(code) };
    function(inputs.as_ptr(), rows, out.as_mut_ptr());
    true
}

pub(crate) fn call_u16_rows_batch_sum(
    code: *const u8,
    inputs: &[u16],
    arity: usize,
    rows: usize,
) -> Option<i64> {
    if inputs.len() != arity.saturating_mul(rows) {
        return None;
    }
    let Ok(rows) = i64::try_from(rows) else {
        return None;
    };
    // SAFETY: `code` is emitted in this crate with signature
    // `extern "C" fn(*const u16, i64) -> i64`. Slice shape is checked before
    // passing pointers, and the owning JIT module outlives the call.
    let function = unsafe { mem::transmute::<*const u8, JitU16RowsBatchSumFn>(code) };
    Some(function(inputs.as_ptr(), rows))
}

pub(crate) fn call_u64_rows_batch(
    code: *const u8,
    inputs: &[u64],
    arity: usize,
    out: &mut [u64],
) -> bool {
    if inputs.len() != arity.saturating_mul(out.len()) {
        return false;
    }
    let Ok(rows) = i64::try_from(out.len()) else {
        return false;
    };
    // SAFETY: `code` is returned by `JITModule::get_finalized_function` for a
    // function emitted in this crate with signature
    // `extern "C" fn(*const u64, i64, *mut u64)`. Slice shape is checked before
    // passing pointers, and the owning JIT module outlives the call.
    let function = unsafe { mem::transmute::<*const u8, JitU64RowsBatchFn>(code) };
    function(inputs.as_ptr(), rows, out.as_mut_ptr());
    true
}

pub(crate) fn call_u64_rows_batch_sum(
    code: *const u8,
    inputs: &[u64],
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
    // `extern "C" fn(*const u64, i64) -> i64`. Slice shape is checked before
    // passing pointers, and the owning JIT module outlives the call.
    let function = unsafe { mem::transmute::<*const u8, JitU64RowsBatchSumFn>(code) };
    Some(function(inputs.as_ptr(), rows))
}

pub(crate) fn call_usize_rows_batch(
    code: *const u8,
    inputs: &[RuntimeUSizeValue],
    arity: usize,
    out: &mut [RuntimeUSizeValue],
) -> bool {
    if inputs.len() != arity.saturating_mul(out.len()) {
        return false;
    }
    let Ok(rows) = i64::try_from(out.len()) else {
        return false;
    };
    // SAFETY: `code` is emitted with the same pointer ABI as the u64 row
    // batch entrypoint. `RuntimeUSizeValue` is `repr(transparent)` over u64,
    // and the JIT function only reads/writes checked row-major element slots.
    let function = unsafe { mem::transmute::<*const u8, JitUSizeRowsBatchFn>(code) };
    function(inputs.as_ptr(), rows, out.as_mut_ptr());
    true
}

pub(crate) fn call_usize_rows_batch_sum(
    code: *const u8,
    inputs: &[RuntimeUSizeValue],
    arity: usize,
    rows: usize,
) -> Option<i64> {
    if inputs.len() != arity.saturating_mul(rows) {
        return None;
    }
    let Ok(rows) = i64::try_from(rows) else {
        return None;
    };
    // SAFETY: `code` is emitted with the same pointer ABI as the u64 row
    // batch-sum entrypoint. `RuntimeUSizeValue` is `repr(transparent)` over
    // u64, and slice shape is checked before passing the pointer.
    let function = unsafe { mem::transmute::<*const u8, JitUSizeRowsBatchSumFn>(code) };
    Some(function(inputs.as_ptr(), rows))
}

pub(crate) fn call_u128_rows_batch(
    code: *const u8,
    inputs: &[u128],
    arity: usize,
    out: &mut [u128],
) -> bool {
    if inputs.len() != arity.saturating_mul(out.len()) {
        return false;
    }
    let Ok(rows) = i64::try_from(out.len()) else {
        return false;
    };
    // SAFETY: `code` is emitted in this crate with signature
    // `extern "C" fn(*const u128, i64, *mut u128)`. Only pointers and `i64`
    // cross the ABI boundary; the generated function loads/stores u128 values
    // from checked slices while the owning JIT module outlives the call.
    let function = unsafe { mem::transmute::<*const u8, JitU128RowsBatchFn>(code) };
    function(inputs.as_ptr(), rows, out.as_mut_ptr());
    true
}

pub(crate) fn call_u128_rows_batch_sum(
    code: *const u8,
    inputs: &[u128],
    arity: usize,
    rows: usize,
) -> Option<i64> {
    if inputs.len() != arity.saturating_mul(rows) {
        return None;
    }
    let Ok(rows) = i64::try_from(rows) else {
        return None;
    };
    // SAFETY: `code` is emitted in this crate with signature
    // `extern "C" fn(*const u128, i64) -> i64`. Slice shape is checked before
    // passing pointers, and no by-value u128 crosses the ABI boundary.
    let function = unsafe { mem::transmute::<*const u8, JitU128RowsBatchSumFn>(code) };
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
