use arcweft_save::decode_typed_json_save;

fn main() {
    let _ = decode_typed_json_save::<serde_json::Value>;
}
