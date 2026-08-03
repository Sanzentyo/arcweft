use arcweft_lang_hir::identity::{
    SyntheticKey, SyntheticKeyFingerprintInput, SyntheticOwner,
};

fn requires_serialize<T: serde::Serialize>() {}
fn requires_deserialize<T: serde::de::DeserializeOwned>() {}

fn main() {
    requires_serialize::<SyntheticOwner>();
    requires_deserialize::<SyntheticOwner>();
    requires_serialize::<SyntheticKey>();
    requires_deserialize::<SyntheticKey>();
    requires_serialize::<SyntheticKeyFingerprintInput>();
    requires_deserialize::<SyntheticKeyFingerprintInput>();
}
