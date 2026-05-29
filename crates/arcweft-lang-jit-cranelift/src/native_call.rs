use std::mem;

type JitI64Fn = extern "C" fn() -> i64;
type JitI64BinaryFn = extern "C" fn(i64, i64) -> i64;

pub(crate) fn call_i64(code: *const u8) -> i64 {
    // SAFETY: `code` is returned by `JITModule::get_finalized_function` for a
    // function emitted in this crate with signature `extern "C" fn() -> i64`.
    // The owning `JITModule` is kept alive by the caller until after the call.
    let function = unsafe { mem::transmute::<*const u8, JitI64Fn>(code) };
    function()
}

pub(crate) fn call_i64_binary(code: *const u8, lhs: i64, rhs: i64) -> i64 {
    // SAFETY: `code` is returned by `JITModule::get_finalized_function` for a
    // function emitted in this crate with signature
    // `extern "C" fn(i64, i64) -> i64`. The owning `JITModule` is kept alive by
    // the caller until after the call.
    let function = unsafe { mem::transmute::<*const u8, JitI64BinaryFn>(code) };
    function(lhs, rhs)
}
