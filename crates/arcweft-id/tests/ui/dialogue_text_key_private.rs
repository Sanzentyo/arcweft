use arcweft_id::{TextKey, dialogue::DialogueTextKey};

fn main() {
    let text_key = TextKey::try_new("text.opening.greeting").unwrap();
    let _dialogue_key = DialogueTextKey(text_key);
}
