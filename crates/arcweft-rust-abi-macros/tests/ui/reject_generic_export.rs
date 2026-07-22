use arcweft_rust_abi_macros::arcweft_export;

#[arcweft_export]
pub fn identity<T>(value: T) -> T {
    value
}

fn main() {}
