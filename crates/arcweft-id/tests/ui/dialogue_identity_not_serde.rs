use arcweft_id::dialogue::{DialogueLineId, DialogueTextKey};
use serde::{Deserialize, Serialize};

fn require_serialize<T: Serialize>() {}
fn require_deserialize<T: for<'de> Deserialize<'de>>() {}

fn main() {
    require_serialize::<DialogueLineId>();
    require_deserialize::<DialogueLineId>();
    require_serialize::<DialogueTextKey>();
    require_deserialize::<DialogueTextKey>();
}
