use arcweft_lang_hir::expr::{
    HirCallArgument, HirCallArgumentListTerminator, HirCallCallee, HirCallExpr,
    HirCallTypeApplication,
};

fn raw_construct(
    callee: HirCallCallee,
    explicit_type_application: HirCallTypeApplication,
    arguments: Box<[HirCallArgument]>,
    terminator: HirCallArgumentListTerminator,
) -> HirCallExpr {
    HirCallExpr {
        callee,
        explicit_type_application,
        arguments,
        terminator,
    }
}

fn main() {}
