use arcweft_rust_abi_macros::ArcweftType;

#[derive(ArcweftType)]
pub struct Buffer<const N: usize> {
    pub value: [u8; N],
}

fn main() {}
