use arcweft_lang_hir::expr::HirCallInvocation;

fn requires_serialize<T: serde::Serialize>() {}
fn requires_deserialize<T: serde::de::DeserializeOwned>() {}

fn main() {
    requires_serialize::<HirCallInvocation>();
    requires_deserialize::<HirCallInvocation>();
}
