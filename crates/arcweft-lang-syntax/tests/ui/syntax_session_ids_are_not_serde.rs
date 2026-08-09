use arcweft_lang_syntax::attachment::{
    SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotId,
};

fn requires_serialize<T: serde::Serialize>() {}
fn requires_deserialize<T: serde::de::DeserializeOwned>() {}

macro_rules! require_serde {
    ($($identity:ty),+ $(,)?) => {
        $(
            requires_serialize::<$identity>();
            requires_deserialize::<$identity>();
        )+
    };
}

fn main() {
    require_serde!(
        SyntaxDatabaseId,
        SyntaxLineageId,
        SyntaxSnapshotId,
        SyntaxNodeId,
    );
}
