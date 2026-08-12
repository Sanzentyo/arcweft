use arcweft_rust_abi_macros::ArcweftType;

#[derive(ArcweftType)]
#[arcweft(opaque_producer = "fixture.rust-abi.const-generic")]
pub struct Buffer<const N: usize> {
    pub value: [u8; N],
}

fn main() {}
