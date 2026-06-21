use arcweft_data::{ArcweftDecode, Decode, Value};

fn default_name() -> String {
    "system".to_owned()
}

#[derive(ArcweftDecode)]
struct Config {
    #[arcweft(default = "default_name")]
    name: String,
}

fn main() {
    let decoded = Config::decode(&Value::Record(Default::default())).expect("decode");
    assert_eq!(decoded.name, "system");
}
