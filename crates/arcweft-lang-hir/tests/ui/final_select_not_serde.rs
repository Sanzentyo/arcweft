use arcweft_lang_hir::expr::HirSelectExpr;

fn requires_serialize<T: serde::Serialize>() {}
fn requires_deserialize<T: serde::de::DeserializeOwned>() {}

fn main() {
    requires_serialize::<HirSelectExpr>();
    requires_deserialize::<HirSelectExpr>();
}
