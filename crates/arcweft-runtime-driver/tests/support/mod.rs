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
use arcweft_dialogue::{DialoguePresentationProfile, DialogueProfileRevision};
use arcweft_id::LocaleTag;
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceSetRevision};
use arcweft_view::{AcceptedViewProgramRevision, ViewProgramId};

pub fn character_plan(character: &str) -> CheckedCharacterPresentationPlan {
    let catalog = character_catalog(character, "Fixture");
    CheckedCharacterPresentationPlan::try_new(
        CharacterPresentationTargetEvidence::Exact(
            CharacterId::try_new(character).expect("fixture CharacterId is valid"),
        ),
        CharacterPresentationCatalogGeneration::new(
            CharacterPresentationCatalogRevision::INITIAL,
            catalog.semantic_digest(),
            catalog.locale_policy_digest(),
        ),
    )
    .expect("fixture character presentation plan is valid")
}

pub fn character_catalog(character: &str, display_name: &str) -> CharacterPresentationCatalogData {
    let locale = CharacterNameLocale::new(LocaleTag::try_new("en").unwrap());
    let policy = CharacterNameLocalePolicy::try_new(locale, Vec::new()).unwrap();
    let base = CharacterDisplayNameInput::Visible(
        CharacterDisplayNameValue::try_new(display_name).unwrap(),
    );
    let record = CharacterDisplayNameRecordInput::try_new(
        CharacterId::try_new(character).unwrap(),
        CharacterPresentationRole::Character,
        None,
        Some(base),
        Vec::new(),
        None,
    )
    .unwrap();
    CharacterPresentationCatalogData::try_from_inputs(
        CharacterPresentationCatalogInput::try_new(policy, vec![record]).unwrap(),
    )
    .unwrap()
}

pub fn dialogue_profile() -> DialoguePresentationProfile {
    DialoguePresentationProfile::engine_default()
}

pub fn dialogue_profile_revision() -> DialogueProfileRevision {
    let manifest = SourceDocument::try_new(
        SourceDocumentId::try_new("runtime-driver-dialogue-profile-fixture").unwrap(),
        SourceName::Memory,
        "schema = 1\n",
    )
    .unwrap();
    let sources = SourceSetRevision::try_for_identities([manifest.identity()]).unwrap();
    DialogueProfileRevision::from_admitted_parts(
        manifest.identity().clone(),
        sources,
        sources,
        ViewProgramId::try_new("view_program.dialogue_fixture").unwrap(),
        AcceptedViewProgramRevision::try_from_bytes([0x4d; 32]).unwrap(),
        ResourceTypeRegistry::empty().digest(),
    )
}
