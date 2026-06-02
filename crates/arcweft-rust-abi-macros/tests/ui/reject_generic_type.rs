use arcweft_rust_abi_macros::ArcweftType;

#[derive(ArcweftType)]
pub struct Wrapper<T> {
    pub value: T,
}

fn main() {}
