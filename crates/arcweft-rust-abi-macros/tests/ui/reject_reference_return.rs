use arcweft_rust_abi_macros::arcweft_export;

#[arcweft_export(name = "bad.borrowed_return")]
pub fn borrowed_return() -> &'static str {
    "value"
}

fn main() {}
