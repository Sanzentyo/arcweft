use arcweft_rust_abi_macros::arcweft_export;

#[arcweft_export(name = "bad.borrowed")]
pub fn borrowed(value: &str) -> i32 {
    value.len() as i32
}

fn main() {}
