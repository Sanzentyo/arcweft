use std::sync::Arc;

use arcweft_character::{
    catalog::CharacterCatalog,
    id::{CharacterId, CharacterLookId, CharacterPartId, CharacterVariantId},
    manifest::{
        CharacterAssetPath, CharacterBlendMode, CharacterCanvas, CharacterLook, CharacterManifest,
        CharacterPart, CharacterPartSelection, CharacterPoint, CharacterRect, CharacterVariant,
    },
};

use super::{
    CharacterNominalType, RuntimeCharacterLookSourceAuthority, RuntimeCharacterLookSourceError,
};
use crate::{
    entry::{
        RuntimeNominalSchemaBody, RuntimeNominalSchemaCase, RuntimeNominalSchemaDefinition,
        RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity, RuntimeNominalTypeId,
        RuntimeSchemaLimits,
    },
    pattern::RuntimeVariantIdentity,
    plan::{
        RuntimePlanBuilder, RuntimePlanTypeProjection as Type, RuntimePlanTypeSeed,
        RuntimeVariantCaseSeed, RuntimeVariantDomainSeed,
    },
    task::RuntimeProgramOwner,
    value::RuntimeValue,
};

fn manifest(name: &str) -> CharacterManifest {
    let character = CharacterId::try_new(name).unwrap();
    let part = CharacterPartId::try_new("body").unwrap();
    let variant = CharacterVariantId::try_new("default").unwrap();
    let look = CharacterLookId::try_new("normal").unwrap();
    CharacterManifest::new(
        character,
        CharacterCanvas::new(64, 128),
        CharacterPoint::new(32, 128),
        look.clone(),
        vec![CharacterPart::new(
            part.clone(),
            0,
            vec![CharacterVariant::new(
                variant.clone(),
                CharacterAssetPath::try_new("layers/body.png").unwrap(),
                CharacterRect::new(0, 0, 64, 128),
                u8::MAX,
                CharacterBlendMode::Normal,
                false,
            )],
        )],
        vec![CharacterLook::new(
            look,
            vec![CharacterPartSelection::new(part, variant)],
        )],
        None,
    )
    .unwrap()
}

fn program(characters: &[CharacterId]) -> RuntimeProgramOwner {
    let definitions = characters
        .iter()
        .map(|character| {
            let semantic = CharacterNominalType::Look {
                character: character.clone(),
            }
            .runtime_semantic_identity();
            let nominal = RuntimeNominalTypeId::from_checked_digest(*semantic.as_bytes());
            RuntimeNominalSchemaDefinition::new(
                RuntimeNominalSchemaIdentity::new(nominal, semantic),
                vec![],
                RuntimeNominalSchemaBody::Variant {
                    cases: vec![RuntimeNominalSchemaCase::new(0, "normal".to_owned(), None)]
                        .into_boxed_slice(),
                },
            )
        })
        .collect::<Vec<_>>();
    let graph =
        RuntimeNominalSchemaGraph::try_new(definitions, RuntimeSchemaLimits::engine_default())
            .unwrap();
    let mut builder = RuntimePlanBuilder::new();
    let types = characters.iter().map(|character| {
        let semantic = CharacterNominalType::Look {
            character: character.clone(),
        }
        .runtime_semantic_identity();
        let nominal = RuntimeNominalTypeId::from_checked_digest(*semantic.as_bytes());
        RuntimePlanTypeSeed::new(
            semantic,
            Type::Nominal {
                nominal,
                layout: graph.try_layout_hash(semantic).unwrap(),
                arguments: Box::new([]),
            },
        )
    });
    let domains = characters.iter().map(|character| {
        let semantic = CharacterNominalType::Look {
            character: character.clone(),
        }
        .runtime_semantic_identity();
        RuntimeVariantDomainSeed::new(
            semantic,
            RuntimeNominalTypeId::from_checked_digest(*semantic.as_bytes()),
            graph.try_layout_hash(semantic).unwrap(),
            [RuntimeVariantCaseSeed::new("normal", None)],
        )
    });
    builder
        .admit_semantic_batch(types, [], [], domains, &graph)
        .unwrap();
    RuntimeProgramOwner::Plan(Arc::new(builder.finish().unwrap()))
}

#[test]
fn look_source_requires_manifest_row_and_exact_program_lease() {
    let akane = CharacterId::try_new("character.akane").unwrap();
    let aoi = CharacterId::try_new("character.aoi").unwrap();
    let catalog = Arc::new(
        CharacterCatalog::try_from_manifests([manifest(akane.as_str()), manifest(aoi.as_str())])
            .unwrap(),
    );

    let incomplete = program(std::slice::from_ref(&akane));
    assert!(matches!(
        RuntimeCharacterLookSourceAuthority::try_new(incomplete, Arc::clone(&catalog)),
        Err(RuntimeCharacterLookSourceError::ProgramType(_))
    ));

    let owner = program(&[akane.clone(), aoi.clone()]);
    let authority = RuntimeCharacterLookSourceAuthority::try_new(owner.clone(), catalog).unwrap();
    let semantic = CharacterNominalType::Look {
        character: akane.clone(),
    }
    .runtime_semantic_identity();
    let checked = owner.types().checked_type(semantic).unwrap();
    let crate::pattern::RuntimeCheckedType::Variant {
        owner: variant_owner,
        ..
    } = checked
    else {
        panic!("Look<C> is a nominal variant");
    };
    let value = RuntimeValue::Variant {
        owner: variant_owner.clone(),
        ordinal: 0,
        name: "normal".to_owned(),
        payload: None,
    };
    assert_eq!(
        authority.decode(&owner, &akane, &value).unwrap().as_str(),
        "normal"
    );
    assert!(matches!(
        authority.decode(&owner, &aoi, &value),
        Err(RuntimeCharacterLookSourceError::ProgramType(_))
    ));
    assert!(matches!(
        authority.decode(
            &program(&[akane, aoi]),
            &CharacterId::try_new("character.akane").unwrap(),
            &value
        ),
        Err(RuntimeCharacterLookSourceError::ForeignProgram)
    ));
    assert!(matches!(
        variant_owner,
        RuntimeVariantIdentity::Nominal { .. }
    ));
}
