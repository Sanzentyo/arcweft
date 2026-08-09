use arcweft_lang_hir::expr::HirThreadBody;
use arcweft_lang_hir::identity::{ItemId, LocalId, ScopeId};

fn requires_serialize<T: serde::Serialize>() {}
fn requires_deserialize<T: serde::de::DeserializeOwned>() {}

fn main() {
    requires_serialize::<ItemId>();
    requires_deserialize::<ItemId>();
    requires_serialize::<ScopeId>();
    requires_deserialize::<ScopeId>();
    requires_serialize::<LocalId>();
    requires_deserialize::<LocalId>();
    requires_serialize::<HirThreadBody>();
    requires_deserialize::<HirThreadBody>();
}
