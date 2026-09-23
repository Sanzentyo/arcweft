use arcweft_rust_abi_macros::{ArcweftType, arcweft_export};

#[arcweft_export(pure)]
fn default_flag() -> bool { false }

#[derive(ArcweftType)]
struct WrongField {
    #[arcweft(default = "default_flag")]
    value: String,
}

fn main() {}
