use arcweft_lang_syntax::incremental::SyntaxNodeId;

fn requires_serialize<T: serde::Serialize>() {}
fn requires_deserialize<T: serde::de::DeserializeOwned>() {}

fn main() {
    requires_serialize::<SyntaxNodeId>();
    requires_deserialize::<SyntaxNodeId>();
}
