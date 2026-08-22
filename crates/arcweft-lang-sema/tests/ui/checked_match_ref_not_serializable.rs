use arcweft_lang_sema::final_analysis::CheckedMatchRef;

fn require_serialize<T: serde::Serialize>() {}

fn main() {
    require_serialize::<CheckedMatchRef>();
}
