use arcweft_character::{
    id::CharacterId,
    presentation_name::{
        CharacterDisplayNameInput, CharacterDisplayNameRecordInput, CharacterDisplayNameValue,
        CharacterPresentationCatalogData, CharacterPresentationCatalogGeneration,
        CharacterPresentationCatalogInput, CharacterPresentationCatalogRevision,
        CharacterPresentationRole,
    },
};
use arcweft_dialogue::character_presentation::{
    CharacterPresentationTargetEvidence, CheckedCharacterPresentationPlan,
};

pub(crate) fn character_catalog() -> CharacterPresentationCatalogData {
    let record = CharacterDisplayNameRecordInput::try_new(
        CharacterId::try_new("character.fixture").unwrap(),
        CharacterPresentationRole::Character,
        None,
        Some(CharacterDisplayNameInput::Visible(
            CharacterDisplayNameValue::try_new("Fixture").unwrap(),
        )),
        Vec::new(),
        None,
    )
    .unwrap();
    CharacterPresentationCatalogData::try_from_inputs(
        CharacterPresentationCatalogInput::try_new(vec![record]).unwrap(),
    )
    .unwrap()
}

pub(crate) fn character_plan() -> CheckedCharacterPresentationPlan {
    let catalog = character_catalog();
    CheckedCharacterPresentationPlan::try_new(
        CharacterPresentationTargetEvidence::Exact(
            CharacterId::try_new("character.fixture").unwrap(),
        ),
        CharacterPresentationCatalogGeneration::new(
            CharacterPresentationCatalogRevision::INITIAL,
            catalog.semantic_digest(),
        ),
    )
    .unwrap()
}
