use arcweft_lang_hir::identity::{ExprId, HirSnapshotId, LocalGeneration};
use arcweft_source::identity::{SourceGeneration, SourceSnapshotId};

fn requires_serialize<T: serde::Serialize>() {}
fn requires_deserialize<T: serde::de::DeserializeOwned>() {}

fn main() {
    requires_serialize::<ExprId>();
    requires_deserialize::<HirSnapshotId>();
    requires_serialize::<LocalGeneration>();
    requires_serialize::<SourceGeneration>();
    requires_deserialize::<SourceSnapshotId>();
}
