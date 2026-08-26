use arcweft_character::id::{CharacterId, CharacterLookId, CharacterPartId, CharacterVariantId};
use arcweft_character::manifest::CharacterPartSelection;

use super::accepted_character_look_semantic_id;

fn selection(part: &str, variant: &str) -> CharacterPartSelection {
    CharacterPartSelection::new(
        CharacterPartId::try_new(part).expect("part ID"),
        CharacterVariantId::try_new(variant).expect("variant ID"),
    )
}

#[test]
fn character_look_semantic_id_is_order_canonical_and_payload_sensitive() {
    let character = CharacterId::try_new("character.akane").expect("character ID");
    let other_character = CharacterId::try_new("character.aoi").expect("character ID");
    let normal = CharacterLookId::try_new("normal").expect("look ID");
    let smile = CharacterLookId::try_new("smile").expect("look ID");
    let source_order = [selection("face", "neutral"), selection("body", "default")];
    let reordered = [selection("body", "default"), selection("face", "neutral")];
    let changed = [selection("body", "default"), selection("face", "smile")];

    let digest = accepted_character_look_semantic_id(&character, &normal, &source_order)
        .expect("bounded canonical look");
    assert_eq!(
        digest,
        accepted_character_look_semantic_id(&character, &normal, &reordered)
            .expect("reordered bounded look")
    );
    assert_ne!(
        digest,
        accepted_character_look_semantic_id(&character, &normal, &changed)
            .expect("changed selection")
    );
    assert_ne!(
        digest,
        accepted_character_look_semantic_id(&character, &smile, &source_order)
            .expect("changed look")
    );
    assert_ne!(
        digest,
        accepted_character_look_semantic_id(&other_character, &normal, &source_order)
            .expect("changed character")
    );
}
