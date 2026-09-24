use super::*;
use crate::{
    CharacterDialogueCharacterDeclaration, CharacterDialogueGenerationDeclaration,
    CharacterDialogueGenerationDeclarationError, CharacterDialoguePresentationContract,
    CharacterDialogueRolePayloadCodec, CharacterDialogueRuntimeDefault,
    CharacterDialogueRuntimeRole as Role, CharacterDialogueRuntimeRoleBody,
    CharacterDialogueRuntimeRoleType, CharacterDialogueRuntimeRoleTypes,
    CharacterDialogueTypeReference, CharacterDialogueTypeReferenceMapError,
    CharacterDialogueVisualType, DialoguePresentationProfile, DialogueProfileRevision,
};
use arcweft_character::id::CharacterId;
use arcweft_core::{
    character_nominal::CharacterNominalType, entry::RuntimeValueDigest,
    pattern::RuntimeSemanticTypeId,
};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceSetRevision};
use arcweft_view::{AcceptedViewProgramRevision, ViewId, ViewProgramId, ViewStyleSheetId};

#[derive(Clone, Debug, Eq, PartialEq)]
struct PlannerTypeRef {
    identity: RuntimeSemanticTypeId,
    plan_node: u32,
}

impl CharacterDialogueTypeReference for PlannerTypeRef {
    fn semantic_identity(&self) -> RuntimeSemanticTypeId {
        self.identity
    }
}

fn semantic(byte: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([byte; 32])
}

fn presentation_contract(
    views: &ViewRegistry,
    profile: DialoguePresentationProfile,
    style_resource: Option<RuntimeValueDigest>,
) -> CharacterDialoguePresentationContract {
    let manifest = SourceDocument::try_new(
        SourceDocumentId::try_new("manifest").expect("document ID"),
        SourceName::Memory,
        "schema = 1\n",
    )
    .expect("manifest document");
    let compiled = SourceDocument::try_new(
        SourceDocumentId::try_new("main").expect("document ID"),
        SourceName::Memory,
        "flow main {}\n",
    )
    .expect("compiled document");
    let topology =
        SourceSetRevision::try_for_identities([manifest.identity()]).expect("topology revision");
    let compiled_sources =
        SourceSetRevision::try_for_identities([compiled.identity()]).expect("compiled revision");
    let revision = DialogueProfileRevision::from_admitted_parts(
        manifest.identity().clone(),
        topology,
        compiled_sources,
        ViewProgramId::try_new("view_program.dialogue").expect("View program ID"),
        AcceptedViewProgramRevision::try_from_bytes([0x31; 32]).expect("View revision"),
        ResourceTypeRegistry::empty().digest(),
    );
    CharacterDialoguePresentationContract::try_new(
        profile,
        revision,
        RuntimeValueDigest::from_bytes(*views.runtime_digest_v1().expect("View digest").as_bytes()),
        style_resource,
    )
    .expect("presentation contract")
}

fn role_types() -> CharacterDialogueRuntimeRoleTypes {
    CharacterDialogueRuntimeRoleTypes::new(
        Role::AUTHORED_BASE.map(|role| {
            let body = if role == Role::RichText {
                CharacterDialogueRuntimeRoleBody::bound(
                    CharacterDialogueRolePayloadCodec::RichTextProperties
                        .payload_schema()
                        .expect("RichText payload schema")
                        .root(),
                    CharacterDialogueRolePayloadCodec::RichTextProperties,
                )
            } else {
                CharacterDialogueRuntimeRoleBody::Unbound
            };
            CharacterDialogueRuntimeRoleType::new(semantic(40 + role.canonical_tag()), body)
        }),
        semantic(60),
    )
}

fn character_row(
    character: CharacterId,
    config: crate::CharacterDialogueConfig,
    visual: CharacterDialogueVisualType<RuntimeSemanticTypeId>,
) -> (
    CharacterId,
    CharacterDialogueCharacterDeclaration<RuntimeSemanticTypeId>,
) {
    let dialogue_type =
        crate::CharacterDialogueType::exact(character.clone()).runtime_semantic_identity();
    (
        character.clone(),
        CharacterDialogueCharacterDeclaration::new(
            dialogue_type,
            visual,
            CharacterDialogueRuntimeDefault::new(character, config),
        ),
    )
}

fn declaration(
    characters: impl IntoIterator<
        Item = (
            CharacterId,
            CharacterDialogueCharacterDeclaration<RuntimeSemanticTypeId>,
        ),
    >,
    views: &ViewRegistry,
) -> CharacterDialogueGenerationDeclaration {
    CharacterDialogueGenerationDeclaration::try_new(
        characters,
        crate::CharacterDialogueType::any().runtime_semantic_identity(),
        semantic(39),
        role_types(),
        CharacterDialogueRuntimeCustomFieldCatalog::try_new([]).expect("empty custom catalog"),
        presentation_contract(views, DialoguePresentationProfile::engine_default(), None),
    )
    .expect("valid generation declaration")
}

#[test]
fn generation_declaration_digest_is_ordered_and_source_owned() {
    let (base, _, views, _) = fixture();
    let character_a = base.character().clone();
    let character_b = CharacterId::try_new("character.beta").expect("Character ID");
    let rows = [
        character_row(
            character_a.clone(),
            base.config().clone(),
            CharacterDialogueVisualType::Absent,
        ),
        character_row(
            character_b.clone(),
            base.config().clone(),
            CharacterDialogueVisualType::Present {
                manifest: RuntimeValueDigest::from_bytes([0x21; 32]),
                look_type: CharacterNominalType::Look {
                    character: character_b.clone(),
                }
                .runtime_semantic_identity(),
            },
        ),
    ];
    let first = declaration(rows.clone(), &views);
    let reversed = declaration(rows.into_iter().rev(), &views);
    assert_eq!(first.digest(), reversed.digest());

    let changed_config = base
        .patched(
            &crate::CharacterDialoguePatch::default().with_source_locale(PatchField::Set(
                crate::DialogueLocaleId::try_new("ja-jp").expect("locale"),
            )),
        )
        .expect("source-locale patch")
        .config()
        .clone();
    let changed = declaration(
        [
            character_row(
                character_a,
                changed_config,
                CharacterDialogueVisualType::Absent,
            ),
            character_row(
                character_b,
                base.config().clone(),
                CharacterDialogueVisualType::Present {
                    manifest: RuntimeValueDigest::from_bytes([0x21; 32]),
                    look_type: CharacterNominalType::Look {
                        character: CharacterId::try_new("character.beta").unwrap(),
                    }
                    .runtime_semantic_identity(),
                },
            ),
        ],
        &views,
    );
    assert_ne!(first.digest(), changed.digest());
}

#[test]
fn generation_type_reference_mapping_preserves_identity_and_digest() {
    let (base, _, views, _) = fixture();
    let character = base.character().clone();
    let declaration = declaration(
        [character_row(
            character,
            base.config().clone(),
            CharacterDialogueVisualType::Absent,
        )],
        &views,
    );
    let mut visited = Vec::new();
    declaration.visit_type_refs(&mut |ty| visited.push(*ty));
    assert_eq!(visited.len(), 2 + 1 + Role::AUTHORED_BASE.len() + 1 + 1);

    let mapped = declaration
        .try_map_type_refs(|ty| {
            Ok::<_, std::io::Error>(PlannerTypeRef {
                identity: *ty,
                plan_node: u32::from(ty.as_bytes()[0]),
            })
        })
        .expect("identity-preserving projection");
    assert_eq!(declaration.digest(), mapped.digest());
    let round_trip = mapped
        .try_map_type_refs(|ty| Ok::<_, std::io::Error>(ty.identity))
        .expect("identity-preserving runtime projection");
    assert_eq!(declaration.digest(), round_trip.digest());

    let changed = declaration.try_map_type_refs(|ty| {
        let mut bytes = *ty.as_bytes();
        bytes[0] = bytes[0].wrapping_add(1);
        Ok::<_, std::io::Error>(PlannerTypeRef {
            identity: RuntimeSemanticTypeId::from_bytes(bytes),
            plan_node: 0,
        })
    });
    assert!(matches!(
        changed,
        Err(CharacterDialogueTypeReferenceMapError::IdentityChanged { .. })
    ));
}

#[test]
fn presentation_fingerprints_follow_accepted_resources_not_profile_selection() {
    let (_, _, views, _) = fixture();
    let profile = DialoguePresentationProfile::engine_default();
    let style_resource = RuntimeValueDigest::from_bytes([0x41; 32]);
    let contract = presentation_contract(&views, profile, Some(style_resource));
    contract
        .verify_resource_fingerprints(contract.view_registry_digest(), Some(style_resource))
        .expect("unselected accepted Style resource matches");
    assert!(matches!(
        contract.verify_resource_fingerprints(contract.view_registry_digest(), None,),
        Err(CharacterDialogueGenerationDeclarationError::StyleResourceFingerprintMismatch { .. })
    ));

    let selected_style = DialoguePresentationProfile::new(
        ViewId::standard_dialogue(),
        Some(ViewStyleSheetId::try_new("style.main_dialogue").expect("Style ID")),
        crate::InlineFailurePolicy::FailLine,
    );
    let revision = contract.revision().clone();
    assert!(matches!(
        CharacterDialoguePresentationContract::try_new(
            selected_style.clone(),
            revision.clone(),
            contract.view_registry_digest(),
            None,
        ),
        Err(CharacterDialogueGenerationDeclarationError::MissingAcceptedStyleResource)
    ));
    assert!(
        CharacterDialoguePresentationContract::try_new(
            selected_style,
            revision,
            contract.view_registry_digest(),
            Some(style_resource),
        )
        .is_ok()
    );
}

#[test]
fn declaration_rejects_noncanonical_dialogue_and_look_type_identities() {
    let (base, _, views, _) = fixture();
    let character = base.character().clone();
    let row = CharacterDialogueCharacterDeclaration::new(
        semantic(1),
        CharacterDialogueVisualType::Absent,
        CharacterDialogueRuntimeDefault::new(character.clone(), base.config().clone()),
    );
    assert!(matches!(
        CharacterDialogueGenerationDeclaration::try_new(
            [(character.clone(), row)],
            crate::CharacterDialogueType::any().runtime_semantic_identity(),
            semantic(2),
            role_types(),
            CharacterDialogueRuntimeCustomFieldCatalog::try_new([]).unwrap(),
            presentation_contract(&views, DialoguePresentationProfile::engine_default(), None),
        ),
        Err(CharacterDialogueGenerationDeclarationError::CharacterDialogueTypeIdentity { .. })
    ));

    let row = CharacterDialogueCharacterDeclaration::new(
        crate::CharacterDialogueType::exact(character.clone()).runtime_semantic_identity(),
        CharacterDialogueVisualType::Present {
            manifest: RuntimeValueDigest::from_bytes([1; 32]),
            look_type: semantic(3),
        },
        CharacterDialogueRuntimeDefault::new(character.clone(), base.config().clone()),
    );
    assert!(matches!(
        CharacterDialogueGenerationDeclaration::try_new(
            [(character, row)],
            crate::CharacterDialogueType::any().runtime_semantic_identity(),
            semantic(2),
            role_types(),
            CharacterDialogueRuntimeCustomFieldCatalog::try_new([]).unwrap(),
            presentation_contract(&views, DialoguePresentationProfile::engine_default(), None),
        ),
        Err(CharacterDialogueGenerationDeclarationError::CharacterLookTypeIdentity { .. })
    ));
}

#[test]
fn declaration_rejects_a_caller_supplied_default_digest_that_does_not_match() {
    let (base, _, views, _) = fixture();
    let character = base.character().clone();
    let row = CharacterDialogueCharacterDeclaration::new(
        crate::CharacterDialogueType::exact(character.clone()).runtime_semantic_identity(),
        CharacterDialogueVisualType::Absent,
        CharacterDialogueRuntimeDefault::with_expected_digest(
            character.clone(),
            base.config().clone(),
            RuntimeValueDigest::from_bytes([0xD7; 32]),
        ),
    );

    let result = CharacterDialogueGenerationDeclaration::try_new(
        [(character.clone(), row)],
        crate::CharacterDialogueType::any().runtime_semantic_identity(),
        semantic(2),
        role_types(),
        CharacterDialogueRuntimeCustomFieldCatalog::try_new([]).unwrap(),
        presentation_contract(&views, DialoguePresentationProfile::engine_default(), None),
    );

    assert!(matches!(
        result,
        Err(CharacterDialogueGenerationDeclarationError::Value(
            crate::CharacterDialogueValueError::DefaultDigestMismatch(actual)
        )) if actual == character
    ));
}
