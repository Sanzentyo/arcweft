use arcweft_character::id::CharacterId;
use arcweft_core::{pattern::RuntimeCheckedType, program_types::RuntimeProgramTypes};
use arcweft_dialogue::{
    CharacterDialogueType, CharacterDialogueVisualType,
    character_presentation::CharacterPresentationTargetEvidence,
};
use arcweft_interaction_model::dialogue::CharacterDialogueRuntimeRole;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

use super::*;

#[test]
fn an_admitted_profile_emits_its_generation_without_character_or_line_use() {
    let compiled = crate::source::compile_source(
        "flow main() -> i64 { return 42i64; }\nentry cli @entry.main { goto @flow.main }\n",
    )
    .unwrap();
    let declaration = compiled.character_dialogue_generation.as_ref().unwrap();
    assert!(declaration.characters().is_empty());
    assert!(compiled.character_catalog.characters().next().is_none());
    assert!(compiled.dialogue_content.records().is_empty());
    let mut count = 0;
    declaration.visit_type_refs(&mut |ty| {
        count += 1;
        RuntimeProgramTypes::Plan(&compiled.plan)
            .require_type(*ty)
            .unwrap();
    });
    assert!(count > CharacterDialogueRuntimeRole::ALL.len());
}

#[test]
fn factory_only_program_roots_generation_types_in_native_and_awbc() {
    let compiled = crate::source::compile_source(
        r"
pub character alice {}
flow main() -> i64 {
    let configured = alice()
    return 42i64
}
entry cli @entry.main { goto @flow.main }
",
    )
    .expect("a factory-only program carries complete producer inputs");
    assert!(compiled.dialogue_content.records().is_empty());
    let declaration = compiled
        .character_dialogue_generation
        .as_ref()
        .expect("generation declaration is independent of dialogue lines");
    assert_eq!(
        declaration.characters().keys().collect::<Vec<_>>(),
        vec![&CharacterId::try_new("character.alice").unwrap()],
    );
    let awbc = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "factory-generation",
    )
    .lower()
    .expect("factory-only AWBC retains the same type authority");
    let mut roots = std::collections::BTreeSet::new();
    declaration.visit_type_refs(&mut |ty| {
        roots.insert(*ty);
        RuntimeProgramTypes::Plan(&compiled.plan)
            .require_type(*ty)
            .expect("every declaration reference exists in the native plan");
        RuntimeProgramTypes::Awbc(&awbc.program)
            .require_type(*ty)
            .expect("every declaration reference exists in AWBC");
    });
    assert!(roots.contains(&CharacterDialogueType::any().runtime_semantic_identity()));
    for role in CharacterDialogueRuntimeRole::ALL {
        let identity = compiled
            .analysis
            .registered_world()
            .environment()
            .character_dialogue_roles()
            .semantic_type(role)
            .semantic_identity_digest()
            .unwrap();
        assert!(roots.contains(&RuntimeSemanticTypeId::from_bytes(*identity.as_bytes())));
    }
    let RuntimeCheckedType::Variant { cases, .. } = RuntimeProgramTypes::Plan(&compiled.plan)
        .checked_type(*declaration.voice())
        .expect("unused Voice still owns its complete source cases")
    else {
        panic!("Voice is the accepted closed source enum")
    };
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].name, "auto");
}

#[test]
fn unused_visual_character_retains_its_accepted_look_schema() {
    let compiled = fixtures::compile_with_character_manifest(
        r"
pub character alice {}
pub character bob {}
flow main() -> i64 { return 42i64 }
entry cli @entry.main { goto @flow.main }
",
    );
    let report = compiled.runtime_plan();
    assert!(report.dialogue_content_catalog.records().is_empty());
    let declaration = report
        .character_dialogue_generation
        .as_ref()
        .expect("unused logical Characters still belong to the generation");
    assert_eq!(declaration.characters().len(), 2);
    let look = TypeKind::character_look(CharacterId::try_new("character.alice").unwrap())
        .semantic_identity_digest()
        .unwrap();
    let look = RuntimeSemanticTypeId::from_bytes(*look.as_bytes());
    let awbc = AwbcLowerer::new(
        &report.plan,
        &report.dialogue_content_catalog,
        "unused-look-generation",
    )
    .lower()
    .expect("unused visual schemas are retained by AWBC");
    for types in [
        RuntimeProgramTypes::Plan(&report.plan),
        RuntimeProgramTypes::Awbc(&awbc.program),
    ] {
        let RuntimeCheckedType::Variant { cases, .. } = types
            .checked_type(look)
            .expect("the accepted Look type is rooted without an expression use")
        else {
            panic!("Look remains an accepted Character nominal")
        };
        assert_eq!(
            cases
                .iter()
                .map(|case| case.name.as_str())
                .collect::<Vec<_>>(),
            ["normal", "bright"],
        );
    }
    assert_eq!(
        declaration.digest(),
        compiled
            .runtime_facts()
            .character_dialogue_generation()
            .expect("normalized generation input")
            .digest(),
        "mapping normalized references to program identities preserves the input contract",
    );
}

#[test]
fn generation_fingerprints_the_accepted_style_resource_without_a_profile_selection() {
    let compile = |red| {
        crate::source::compile_source(&format!(
            r"
pub character alice {{}}
pub style Extra {{ Button {{ color = rgba({red}, 20, 30, 255) }} }}
flow main() -> i64 {{ return 42i64 }}
entry cli @entry.main {{ goto @flow.main }}
"
        ))
        .expect("unselected accepted Style remains a dynamic configuration resource")
    };
    let first = compile(10);
    let changed = compile(11);
    let mut style_digests = Vec::new();
    let mut declaration_digests = Vec::new();
    for compiled in [&first, &changed] {
        let declaration = compiled.character_dialogue_generation.as_ref().unwrap();
        assert!(declaration.presentation().profile().style().is_none());
        let actual = compiled
            .dialogue_profile
            .product()
            .style()
            .expect("the exact admitted product retains its Style resource")
            .resource()
            .canonical_digest()
            .unwrap();
        let expected = arcweft_core::entry::RuntimeValueDigest::from_bytes(actual.as_bytes());
        assert_eq!(
            declaration.presentation().style_resource_digest(),
            Some(expected),
        );
        style_digests.push(expected);
        declaration_digests.push(declaration.digest());
    }
    assert_ne!(style_digests[0], style_digests[1]);
    assert_ne!(declaration_digests[0], declaration_digests[1]);
}

#[test]
fn an_unused_custom_field_roots_its_source_type_and_owns_its_descriptor_digest() {
    let source = "pub character alice {}\nflow main() -> i64 { return 42i64; }\nentry cli @entry.main { goto @flow.main }\n";
    let mut digests = Vec::new();
    for clearable in [true, false] {
        let compiled = fixtures::compile_with_custom_field(source, clearable);
        let report = compiled.runtime_plan();
        let declaration = report.character_dialogue_generation.as_ref().unwrap();
        let field = declaration
            .custom_fields()
            .fields()
            .values()
            .next()
            .unwrap();
        assert_eq!(declaration.custom_fields().fields().len(), 1);
        assert_eq!(field.clearable(), clearable);
        let awbc = AwbcLowerer::new(
            &report.plan,
            &report.dialogue_content_catalog,
            "custom-generation",
        )
        .lower()
        .unwrap();
        for types in [
            RuntimeProgramTypes::Plan(&report.plan),
            RuntimeProgramTypes::Awbc(&awbc.program),
        ] {
            assert_eq!(
                types.checked_type(*field.semantic_type_ref()).unwrap(),
                RuntimeCheckedType::String
            );
        }
        digests.push((declaration.custom_fields().digest(), declaration.digest()));
    }
    assert_ne!(digests[0].0, digests[1].0);
    assert_ne!(digests[0].1, digests[1].1);
}

#[test]
fn external_character_factory_is_a_logical_member_without_a_hir_declaration() {
    let compiled = fixtures::compile_with_external_character(
        r"
flow main() -> i64 { let configured = alice(look = .bright); return 42i64; }
entry cli @entry.main { goto @flow.main }
",
    )
    .expect("accepted external Character owns factory and Look type roots");
    let report = compiled.runtime_plan();
    let declaration = report.character_dialogue_generation.as_ref().unwrap();
    let alice = CharacterId::try_new("character.alice").unwrap();
    let row = declaration.characters().get(&alice).unwrap();
    let CharacterDialogueVisualType::Present { look_type, .. } = row.visual() else {
        panic!("the external declaration joins its accepted visual resource");
    };
    assert_eq!(declaration.characters().len(), 1);
    let awbc = AwbcLowerer::new(
        &report.plan,
        &report.dialogue_content_catalog,
        "external-generation",
    )
    .lower()
    .expect("external factory types survive the AWBC boundary");
    for types in [
        RuntimeProgramTypes::Plan(&report.plan),
        RuntimeProgramTypes::Awbc(&awbc.program),
    ] {
        types.require_type(*look_type).unwrap();
        types.require_type(*row.dialogue_type()).unwrap();
    }
}

#[test]
fn a_loaded_visual_manifest_does_not_invent_a_logical_character() {
    let compiled = fixtures::compile_with_character_manifest(
        r"
pub character bob {}
flow main() -> i64 { return 42i64; }
entry cli @entry.main { goto @flow.main }
",
    );
    let declaration = compiled
        .runtime_plan()
        .character_dialogue_generation
        .as_ref()
        .unwrap();
    let rows = declaration.characters().iter().collect::<Vec<_>>();
    let [(character, row)] = rows.as_slice() else {
        panic!("one accepted logical declaration");
    };
    assert_eq!(character.as_str(), "character.bob");
    assert_eq!(row.visual(), &CharacterDialogueVisualType::Absent);
}

#[test]
fn an_external_line_without_accepted_name_evidence_is_rejected_precisely() {
    let error = fixtures::compile_with_external_character(
        r"
flow main() -> Unit { alice[Hello]; }
entry cli @entry.main { goto @flow.main }
",
    )
    .expect_err("display names must be supplied by a real declaration authority");
    assert!(error.diagnostics().iter().any(|diagnostic| {
        diagnostic.diagnostic().message().contains(
            "Character `character.alice` has no accepted display-name declaration for dialogue presentation",
        )
    }), "{error:?}");
}

#[test]
fn any_target_retains_one_generation_across_distinct_characters_and_closed_instances() {
    let compiled = fixtures::compile_with_character_manifest(
        r#"
pub character alice { display = "Alice" }
pub character bob { display = "Bob" }
fn speak<T>(value: T) {
    let selected = if true { alice(look = .bright, source_locale = "ja-JP") } else { bob(source_locale = "en-US") }
    selected[#[value]];
}
flow main() -> Unit {
    let selected = if false { alice() } else { bob() }
    selected[Global];
    speak(1i64);
    speak("content");
}
entry cli @entry.main { goto @flow.main }
"#,
    );
    let report = compiled.runtime_plan();
    let declaration = report.character_dialogue_generation.as_ref().unwrap();
    assert_eq!(declaration.characters().len(), 2);
    assert!(matches!(
        declaration.characters()[&CharacterId::try_new("character.alice").unwrap()].visual(),
        CharacterDialogueVisualType::Present { .. }
    ));
    assert_eq!(
        declaration.characters()[&CharacterId::try_new("character.bob").unwrap()].visual(),
        &CharacterDialogueVisualType::Absent,
    );
    assert_eq!(report.dialogue_content_catalog.records().len(), 3);
    for content in report.dialogue_content_catalog.records() {
        assert_eq!(
            content.character().target(),
            &CharacterPresentationTargetEvidence::RuntimeCharacterDialogue {
                generation: declaration.digest(),
            },
        );
    }
    let awbc = AwbcLowerer::new(
        &report.plan,
        &report.dialogue_content_catalog,
        "any-generation",
    )
    .lower()
    .expect("Any target has the same source-owned contract in AWBC");
    assert_eq!(awbc.program.content_units.len(), 3);
}
