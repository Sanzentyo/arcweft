use arcweft_lang_hir::expr::{
    HirCallArgument, HirCallArgumentListTerminator, HirCallCallee, HirCallInvocation,
    HirCallTypeApplication,
};

fn raw_construct(
    callee: HirCallCallee,
    explicit_type_application: HirCallTypeApplication,
    arguments: Box<[HirCallArgument]>,
    terminator: HirCallArgumentListTerminator,
) -> HirCallInvocation {
    HirCallInvocation {
        callee,
        explicit_type_application,
        arguments,
        terminator,
    }
}

fn main() {}
