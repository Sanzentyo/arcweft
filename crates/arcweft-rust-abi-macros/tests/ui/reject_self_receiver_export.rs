use arcweft_rust_abi_macros::arcweft_export;

pub struct Counter;

impl Counter {
    #[arcweft_export(name = "bad.receiver")]
    pub fn value(&self) -> i32 {
        1
    }
}

fn main() {}
