use arcweft_rust_abi_macros::ArcweftType;

#[derive(ArcweftType)]
#[arcweft(opaque_producer = "fixture.rust-abi.reference-field")]
struct BorrowedField {
    value: &'static str,
}

fn main() {}
