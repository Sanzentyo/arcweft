use arcweft_character::{
    id::CharacterId,
    presentation_name::{
        CharacterDisplayNameInput, CharacterDisplayNameRecordInput, CharacterDisplayNameValue,
        CharacterNameLocale, CharacterNameLocalePolicy, CharacterPresentationCatalogData,
        CharacterPresentationCatalogGeneration, CharacterPresentationCatalogInput,
        CharacterPresentationCatalogRevision, CharacterPresentationRole,
    },
};
use arcweft_dialogue::character_presentation::{
    CharacterPresentationTargetEvidence, CheckedCharacterPresentationPlan,
};
use arcweft_id::LocaleTag;

pub(crate) fn character_catalog() -> CharacterPresentationCatalogData {
    let locale = CharacterNameLocale::new(LocaleTag::try_new("en").unwrap());
    let policy = CharacterNameLocalePolicy::try_new(locale, Vec::new()).unwrap();
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
        CharacterPresentationCatalogInput::try_new(policy, vec![record]).unwrap(),
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
            catalog.locale_policy_digest(),
        ),
    )
    .unwrap()
}
