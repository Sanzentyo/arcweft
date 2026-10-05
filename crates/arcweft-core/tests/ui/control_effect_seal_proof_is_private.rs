use arcweft_core::plan::{ControlEffectContractDigest, RuntimeControlEffectContractSeedId};

fn main() {
    let _ = ControlEffectContractDigest::from_bytes([0; 32]);
    let _ = serde_json::from_str::<ControlEffectContractDigest>("[]");
    let _ = serde_json::from_str::<RuntimeControlEffectContractSeedId>("[]");
}
