use super::*;
use crate::CharacterDialoguePresentationContract;

fn with_visual(
    declaration: &CharacterDialogueGenerationDeclaration,
    visual: &CharacterDialogueVisualType<RuntimeSemanticTypeId>,
) -> CharacterDialogueGenerationDeclaration {
    CharacterDialogueGenerationDeclaration::try_new(
        declaration.characters().iter().map(|(character, row)| {
            (
                character.clone(),
                CharacterDialogueCharacterDeclaration::new(
                    *row.dialogue_type(),
                    visual.clone(),
                    row.defaults().clone(),
                ),
            )
        }),
        *declaration.any_dialogue(),
        *declaration.voice(),
        declaration.roles().clone(),
        declaration.custom_fields().clone(),
        declaration.presentation().clone(),
    )
    .unwrap()
}

#[test]
fn runtime_binding_admits_both_program_kinds_and_retains_the_generation_and_exact_lease() {
    let (_, characters, views, custom) = fixture();
    let types = Types::new();
    let declaration = types.declaration(&characters, &views, &custom);
    for owner in types.program_owners() {
        let schema = declaration
            .bind_runtime(
                Arc::new(views.clone()),
                Arc::new(characters.clone()),
                None,
                owner.clone(),
            )
            .unwrap();
        assert_eq!(schema.generation_digest(), declaration.digest());
        assert!(schema.program_owner().same_program(&owner));
        assert!(
            schema
                .look_source_authority()
                .program_owner()
                .same_program(&owner)
        );
        let target = RuntimeValue::EntityRef(RuntimeEntityReference::Project {
            family: DeclarationIdentityFamily::Character,
            public_id: PublicId::try_new(sample_manifest().character().as_str()).unwrap(),
        });
        assert!(
            schema
                .construct(&owner, &target, &[], types.any_type)
                .is_ok()
        );
        let foreign = types
            .program_owners()
            .into_iter()
            .find(|candidate| std::mem::discriminant(candidate) == std::mem::discriminant(&owner))
            .unwrap();
        assert!(matches!(
            schema.construct(&foreign, &target, &[], types.any_type),
            Err(CharacterDialogueValueError::ForeignProgramOwner)
        ));
    }
}

#[test]
fn runtime_binding_accepts_empty_logical_membership_and_rejects_missing_declared_members() {
    let (_, characters, views, custom) = fixture();
    let types = Types::new();
    let declaration = types.declaration(&characters, &views, &custom);
    let empty = Arc::new(CharacterCatalog::try_from_declarations([]).unwrap());
    let owner = types.program_owners()[0].clone();
    assert!(matches!(
        declaration.bind_runtime(Arc::new(views.clone()), Arc::clone(&empty), None, owner.clone()),
        Err(CharacterDialogueGenerationBindingError::MissingCharacter(character))
            if character == *sample_manifest().character()
    ));
    let empty_declaration = CharacterDialogueGenerationDeclaration::try_new(
        [],
        *declaration.any_dialogue(),
        *declaration.voice(),
        declaration.roles().clone(),
        declaration.custom_fields().clone(),
        declaration.presentation().clone(),
    )
    .unwrap();
    let schema = empty_declaration
        .bind_runtime(Arc::new(views), empty, None, owner)
        .unwrap();
    assert_eq!(schema.generation_digest(), empty_declaration.digest());
    assert!(
        schema
            .look_source_authority()
            .character_catalog()
            .characters()
            .next()
            .is_none()
    );
}

#[test]
fn runtime_binding_requires_exact_visual_presence_and_manifest_fingerprints() {
    let (_, characters, views, custom) = fixture();
    let types = Types::new();
    let declaration = types.declaration(&characters, &views, &custom);
    let owner = types.program_owners()[0].clone();
    let absent_catalog = CharacterCatalog::try_from_declarations([(
        sample_manifest().character().clone(),
        CharacterVisualManifestEvidence::Absent,
    )])
    .unwrap();
    let absent = with_visual(&declaration, &CharacterDialogueVisualType::Absent);
    assert!(
        absent
            .bind_runtime(
                Arc::new(views.clone()),
                Arc::new(absent_catalog.clone()),
                None,
                owner.clone(),
            )
            .is_ok()
    );
    let stale = with_visual(
        &declaration,
        &CharacterDialogueVisualType::Present {
            manifest: RuntimeValueDigest::from_bytes([0x71; 32]),
            look_type: look_source_type(),
        },
    );
    for (candidate, catalog) in [
        (&declaration, &absent_catalog),
        (&absent, &characters),
        (&stale, &characters),
    ] {
        assert!(matches!(
            candidate.bind_runtime(Arc::new(views.clone()), Arc::new(catalog.clone()), None, owner.clone()),
            Err(CharacterDialogueGenerationBindingError::VisualManifestMismatch { character, expected, actual })
                if character == *sample_manifest().character() && expected != actual
        ));
    }
}

#[test]
fn runtime_binding_rechecks_actual_view_and_style_resource_fingerprints() {
    let (_, characters, views, custom) = fixture();
    let types = Types::new();
    let declaration = types.declaration(&characters, &views, &custom);
    let owner = types.program_owners()[0].clone();
    assert!(matches!(
        declaration.bind_runtime(Arc::new(ViewRegistry::default()), Arc::new(characters.clone()), None, owner.clone()),
        Err(CharacterDialogueGenerationBindingError::Presentation(inner))
            if matches!(*inner, CharacterDialogueGenerationDeclarationError::ViewRegistryFingerprintMismatch { .. })
    ));
    let expected = RuntimeValueDigest::from_bytes([0x72; 32]);
    assert!(matches!(
        declaration.bind_runtime(Arc::new(views.clone()), Arc::new(characters.clone()), Some(expected), owner.clone()),
        Err(CharacterDialogueGenerationBindingError::Presentation(inner))
            if matches!(*inner, CharacterDialogueGenerationDeclarationError::StyleResourceFingerprintMismatch { .. })
    ));
    let presentation = declaration.presentation();
    let with_style = CharacterDialogueGenerationDeclaration::try_new(
        declaration
            .characters()
            .iter()
            .map(|(character, row)| (character.clone(), row.clone())),
        *declaration.any_dialogue(),
        *declaration.voice(),
        declaration.roles().clone(),
        declaration.custom_fields().clone(),
        CharacterDialoguePresentationContract::try_new(
            presentation.profile().clone(),
            presentation.revision().clone(),
            presentation.view_registry_digest(),
            Some(expected),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        with_style
            .bind_runtime(
                Arc::new(views.clone()),
                Arc::new(characters.clone()),
                Some(expected),
                owner.clone(),
            )
            .is_ok()
    );
    for actual in [None, Some(RuntimeValueDigest::from_bytes([0x73; 32]))] {
        assert!(matches!(
            with_style.bind_runtime(Arc::new(views.clone()), Arc::new(characters.clone()), actual, owner.clone()),
            Err(CharacterDialogueGenerationBindingError::Presentation(inner))
                if matches!(*inner, CharacterDialogueGenerationDeclarationError::StyleResourceFingerprintMismatch { .. })
        ));
    }
}

#[test]
fn runtime_binding_preflights_unused_roots_and_rejects_a_foreign_dialogue_type_shape() {
    let (_, characters, views, custom) = fixture();
    let types = Types::new();
    let declaration = types.declaration(&characters, &views, &custom);
    for missing in [types.any_type, voice_source_type(), types.nominal_identity] {
        let mut program = types.awbc.clone();
        program
            .runtime_types
            .retain(|row| row.semantic_identity() != missing);
        assert!(matches!(
            declaration.bind_runtime(
                Arc::new(views.clone()),
                Arc::new(characters.clone()),
                None,
                RuntimeProgramOwner::Awbc(Arc::new(program))
            ),
            Err(CharacterDialogueGenerationBindingError::ProgramType(_))
        ));
    }
    let mut program = types.awbc.clone();
    let row = program
        .runtime_types
        .iter_mut()
        .find(|row| row.semantic_identity() == types.character_type)
        .unwrap();
    *row = AwbcRuntimeType::new(types.character_type, AwbcType::Bool);
    assert!(matches!(
        declaration.bind_runtime(Arc::new(views), Arc::new(characters), None, RuntimeProgramOwner::Awbc(Arc::new(program))),
        Err(CharacterDialogueGenerationBindingError::DialogueTypeMismatch { identity })
            if identity == types.character_type
    ));
}

#[test]
fn runtime_binding_checks_the_bound_payloads_complete_private_graph() {
    let (_, characters, views, custom) = fixture();
    let types = Types::new();
    let declaration = types.declaration(&characters, &views, &custom);
    let payload = CharacterDialogueRolePayloadCodec::RichTextProperties
        .payload_schema()
        .unwrap();
    let missing = payload
        .types()
        .iter()
        .find(|row| row.semantic_identity() != payload.root())
        .unwrap()
        .semantic_identity();
    let mut program = types.awbc.clone();
    let row = program
        .runtime_types
        .iter_mut()
        .find(|row| row.semantic_identity() == missing)
        .unwrap();
    *row = AwbcRuntimeType::new(semantic(0xef), row.shape().clone());
    let error = declaration
        .bind_runtime(
            Arc::new(views),
            Arc::new(characters),
            None,
            RuntimeProgramOwner::Awbc(Arc::new(program)),
        )
        .expect_err("a missing private payload descendant must be rejected");
    assert!(
        matches!(&error, CharacterDialogueGenerationBindingError::Schema(_)),
        "{error:?}"
    );
}
