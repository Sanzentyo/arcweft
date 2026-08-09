use arcweft_lang_hir::expr::HirCallExpr;

fn requires_serialize<T: serde::Serialize>() {}
fn requires_deserialize<T: serde::de::DeserializeOwned>() {}

fn main() {
    requires_serialize::<HirCallExpr>();
    requires_deserialize::<HirCallExpr>();
}
