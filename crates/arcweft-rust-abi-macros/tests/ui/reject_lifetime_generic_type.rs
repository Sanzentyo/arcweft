use arcweft_rust_abi_macros::ArcweftType;

#[derive(ArcweftType)]
#[arcweft(opaque_producer = "fixture.rust-abi.lifetime-generic")]
pub struct Borrowed<'a> {
    pub value: &'a str,
}

fn main() {}
