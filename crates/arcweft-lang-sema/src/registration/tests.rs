use std::{collections::BTreeMap, fmt::Write as _, sync::Arc};

use arcweft_character::{
    id::{CharacterId, CharacterLookId, CharacterPartId, CharacterVariantId},
    manifest::{
        CharacterAssetPath, CharacterBlendMode, CharacterCanvas, CharacterLook, CharacterManifest,
        CharacterPart, CharacterPartSelection, CharacterPoint, CharacterRect, CharacterVariant,
        registration::{
            CharacterManifestRootField, CharacterManifestTokenPath, SourceBackedCharacterManifest,
        },
    },
    registration_catalog::SourceBackedCharacterCatalog,
    symbol::CharacterSymbolDescriptor,
};
use arcweft_lang_hir::symbol::{
    CallablePackageId, ProjectSymbolLinkError, ProjectSymbolTargetId, ProjectSymbolWorldId,
};
use arcweft_lang_syntax::ast::{
    module_path::{CanonicalModulePath, ModulePathRoot, ModuleSegment},
    symbol_path::SymbolPath,
};
use arcweft_source::{SourceDocument, SourceRange};

use crate::test_support::character_project::{
    PACKAGE, backed_manifest, character_binding_paths, declaration_span, external_fact,
    one_character_facts, one_character_facts_with_documents, one_character_facts_with_environment,
    project_modules, project_path, register, root_project, root_project_source, sample_manifest,
    sample_manifest_for, source_document,
};
use crate::{
    callable::{
        CallableName, CallableParameterType, CallablePath, ProjectCallablePath, ProjectNameBinding,
    },
    env::{
        TypeCheckEnv,
        identity::{EnvironmentBindingId, EnvironmentBindingIdError},
        nominal::{AcceptedNominalOrigin, AcceptedNominalOwnerId, AcceptedNominalSemantics},
    },
    types::{EntityKind, TypeKind},
};

use super::model::{RegistrationDocumentView, registration_document_diagnostics};
use super::registrar::{charge, merge_manifest_occurrence};
use super::{
    CharacterInventoryDigest, CharacterInventoryIntegrityError, CharacterInventoryRevision,
    CharacterRegistrar, CharacterRegistrationCode, CharacterRegistrationDiagnostic,
    CharacterRegistrationDiagnosticKind, CharacterRegistrationLimitKind,
    CharacterRegistrationLimits, CharacterRegistrationReport, CharacterRegistrationRequest,
    ExternalOwnerLookupError, ExternalRegistrationFact, ProjectRegistrationFacts,
    RegisteredExternalOwner, RegisteredExternalOwnerKind, RegisteredSemanticWorld,
};

fn environment_external_owner(id: EnvironmentBindingId) -> RegisteredExternalOwner {
    RegisteredExternalOwner::environment(id.clone(), id)
}

fn reordered_manifest(reverse: bool) -> CharacterManifest {
    let owner = CharacterId::try_new("character.akane").expect("character");
    let body = CharacterPartId::try_new("body").expect("body part");
    let face = CharacterPartId::try_new("face").expect("face part");
    let default = CharacterVariantId::try_new("default").expect("default variant");
    let alternate = CharacterVariantId::try_new("alternate").expect("alternate variant");
    let normal = CharacterLookId::try_new("normal").expect("normal look");
    let happy = CharacterLookId::try_new("happy").expect("happy look");
    let variant = |id: CharacterVariantId, asset: &str| {
        CharacterVariant::new(
            id,
            CharacterAssetPath::try_new(asset).expect("asset"),
            CharacterRect::new(0, 0, 64, 128),
            u8::MAX,
            CharacterBlendMode::Normal,
            false,
        )
    };
    let mut body_variants = vec![
        variant(default.clone(), "layers/body-default.png"),
        variant(alternate.clone(), "layers/body-alternate.png"),
    ];
    let mut face_variants = vec![
        variant(default.clone(), "layers/face-default.png"),
        variant(alternate.clone(), "layers/face-alternate.png"),
    ];
    let mut normal_selections = vec![
        CharacterPartSelection::new(body.clone(), default.clone()),
        CharacterPartSelection::new(face.clone(), default.clone()),
    ];
    let mut happy_selections = vec![
        CharacterPartSelection::new(body.clone(), alternate.clone()),
        CharacterPartSelection::new(face.clone(), alternate),
    ];
    if reverse {
        body_variants.reverse();
        face_variants.reverse();
        normal_selections.reverse();
        happy_selections.reverse();
    }
    let mut parts = vec![
        CharacterPart::new(body, 0, body_variants),
        CharacterPart::new(face, 1, face_variants),
    ];
    let mut looks = vec![
        CharacterLook::new(normal.clone(), normal_selections),
        CharacterLook::new(happy, happy_selections),
    ];
    if reverse {
        parts.reverse();
        looks.reverse();
    }
    CharacterManifest::new(
        owner,
        CharacterCanvas::new(64, 128),
        CharacterPoint::new(32, 128),
        normal,
        parts,
        looks,
        None,
    )
    .expect("reordered manifest")
}

fn registered_character_and_environment(
    profile: &str,
) -> (RegisteredSemanticWorld, EnvironmentBindingId) {
    let (root, project, world) = root_project(profile);
    let manifest = sample_manifest("layers/body.png");
    let (manifest_document, backed) = backed_manifest(
        "arcweft-project://registration-tests/characters/mixed-owner.awchar.json",
        &manifest,
    );
    let character = manifest.character().clone();
    let character_fact = external_fact(
        character.as_str(),
        &character_binding_paths(&character),
        RegisteredExternalOwner::Character(character.clone()),
        declaration_span(&backed),
    );
    let generated = source_document(
        "arcweft-generated://registration-tests/mixed-adapter",
        "adapter.viewport",
    );
    let environment = EnvironmentBindingId::try_new("adapter.viewport").expect("environment id");
    let environment_fact = external_fact(
        environment.as_str(),
        &[project_path(["adapter", "viewport"])],
        environment_external_owner(environment.clone()),
        generated
            .span(SourceRange::new(0, "adapter.viewport".len()))
            .expect("environment declaration"),
    );
    let catalog = SourceBackedCharacterCatalog::try_new(root.identity().clone(), vec![backed])
        .expect("catalog");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![root, manifest_document, generated],
        vec![character_fact, environment_fact],
        vec![catalog],
        Vec::new(),
    )
    .expect("mixed registration facts");
    let base = TypeCheckEnv::standard().with_symbol(environment.as_str(), TypeKind::I32);
    (
        register(&project, &facts, base, None).expect("mixed world registers"),
        environment,
    )
}

fn registration_snapshot(
    registered: &RegisteredSemanticWorld,
) -> (
    ProjectSymbolWorldId,
    arcweft_lang_hir::symbol::ProjectSymbolRevision,
    Vec<arcweft_lang_hir::symbol::CallableDeclarationId>,
    Vec<String>,
    Vec<CharacterId>,
    CharacterInventoryDigest,
    CharacterInventoryRevision,
) {
    (
        registered.symbols().world().clone(),
        *registered.symbols().revision(),
        registered
            .symbols()
            .callable_symbols()
            .map(|symbol| symbol.declaration().clone())
            .collect(),
        registered
            .symbols()
            .external_symbols()
            .map(|symbol| symbol.canonical_path().canonical_string())
            .collect(),
        registered
            .environment()
            .characters()
            .map(|(owner, _)| owner.clone())
            .collect(),
        registered.environment().character_digest(),
        registered.environment().character_revision(),
    )
}

// Keep registration tests grouped by the contract they exercise.  The shared
// fixtures above intentionally remain private to this parent module.
mod character_paths_and_integrity;
mod external_owners_and_atomicity;
mod limits_and_inventory;
mod occurrence_conflicts_and_diagnostics;
mod publication;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").expect("writing to a string cannot fail");
        output
    })
}
