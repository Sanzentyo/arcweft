use arcweft_character::{
    id::CharacterId,
    presentation_name::{
        CharacterPresentationCatalogGeneration, CharacterPresentationCatalogRevision,
        CharacterPresentationLocalePolicyDigest, CharacterPresentationSemanticDigest,
    },
};
use arcweft_dialogue::character_presentation::{
    CharacterPresentationTargetEvidence, CheckedCharacterPresentationPlan,
};
use arcweft_dialogue::{DialoguePresentationProfile, DialogueProfileRevision};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceSetRevision};
use arcweft_view::{AcceptedViewProgramRevision, ViewProgramId};

pub fn character_plan(character: &str) -> CheckedCharacterPresentationPlan {
    CheckedCharacterPresentationPlan::try_new(
        CharacterPresentationTargetEvidence::Exact(
            CharacterId::try_new(character).expect("fixture CharacterId is valid"),
        ),
        CharacterPresentationCatalogGeneration::new(
            CharacterPresentationCatalogRevision::INITIAL,
            CharacterPresentationSemanticDigest::from_bytes([1; 32]),
            CharacterPresentationLocalePolicyDigest::from_bytes([2; 32]),
        ),
    )
    .expect("fixture character presentation plan is valid")
}

pub fn dialogue_profile() -> DialoguePresentationProfile {
    DialoguePresentationProfile::engine_default()
}

pub fn dialogue_profile_revision() -> DialogueProfileRevision {
    let source = SourceDocument::try_new(
        SourceDocumentId::try_new("render-text-dialogue-profile-fixture").unwrap(),
        SourceName::Memory,
        "schema = 1\n",
    )
    .unwrap();
    let sources = SourceSetRevision::try_for_identities([source.identity()]).unwrap();
    DialogueProfileRevision::from_admitted_parts(
        source.identity().clone(),
        sources,
        sources,
        ViewProgramId::try_new("view_program.render_text_dialogue").unwrap(),
        AcceptedViewProgramRevision::try_from_bytes([0x42; 32]).unwrap(),
        ResourceTypeRegistry::empty().digest(),
    )
}
