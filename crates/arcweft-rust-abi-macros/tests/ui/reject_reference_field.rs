use arcweft_rust_abi_macros::ArcweftType;

#[derive(ArcweftType)]
struct BorrowedField {
    value: &'static str,
}

fn main() {}
