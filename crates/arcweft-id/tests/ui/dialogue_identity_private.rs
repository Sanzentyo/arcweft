use arcweft_id::{PublicId, dialogue::DialogueLineId};

fn main() {
    let public_id = PublicId::try_new("say.opening.greeting").unwrap();
    let _line = DialogueLineId(public_id);
}
