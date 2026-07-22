use super::{
    ProfileTopologyBinaryOverlaySeed, ProfileTopologyErrorCode, ProfileTopologyLimits,
    ProfileTopologyLoadRequest, ProfileTopologyOverlaySeed, ProfileTopologyOwnerId,
    ProfileTopologyResourceKind, ProfileTopologyResourceOrigin, load_profile_topology,
};
use arcweft_adapter_context::{
    manifest::{
        AdapterCallableName, AdapterEffectCapability, AdapterManifest, AdapterNominalOwner,
        AdapterNominalVisibility, AdapterRegistry, AdapterTypeKind,
    },
    standard::standard_registry,
};
use arcweft_adapter_metadata::{
    AdapterFunctionExport, AdapterMetadata, AdapterParameter, AdapterTarget, FunctionPurity,
    ProcessAbi, ProcessTarget, ProcessTransport, WasmAbi, WasmTarget,
};
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_sema::{
    check::typecheck_hir, diagnostics::TypeCheckErrorKind,
    effect_diagnostics::EffectDiagnosticCode, env::TypeCheckEnv,
};
use arcweft_lang_syntax::parser::parse_source;
use arcweft_launch::LaunchProfileSelection;
use arcweft_manifest_model::{
    CapabilityId, FieldName, FunctionName, ManifestVisibility, RawDigest, TypeReference, WitWorldId,
};
use arcweft_source::SourceSetRevision;
use std::{fmt::Write as _, fs, path::PathBuf};

const ROOT_SOURCE: &str = "fn main() -> Unit { () }\n";
const TRUCK_METADATA: &str =
    include_str!("../../../arcweft-adapter-metadata/tests/fixtures/truck-rust.adapter.json");
const CHARACTER_MANIFEST: &str =
    include_str!("../../../arcweft-character/tests/fixtures/zundamon.awchar/character.awchar.json");
const CHARACTER_LAYER_FIXTURES: [(&str, &[u8]); 5] = [
    (
        "layers/body--default.png",
        include_bytes!(
            "../../../arcweft-character/tests/fixtures/zundamon.awchar/layers/body--default.png"
        ),
    ),
    (
        "layers/eyes--normal.png",
        include_bytes!(
            "../../../arcweft-character/tests/fixtures/zundamon.awchar/layers/eyes--normal.png"
        ),
    ),
    (
        "layers/eyes--smile.png",
        include_bytes!(
            "../../../arcweft-character/tests/fixtures/zundamon.awchar/layers/eyes--smile.png"
        ),
    ),
    (
        "layers/mouth--neutral.png",
        include_bytes!(
            "../../../arcweft-character/tests/fixtures/zundamon.awchar/layers/mouth--neutral.png"
        ),
    ),
    (
        "layers/mouth--smile.png",
        include_bytes!(
            "../../../arcweft-character/tests/fixtures/zundamon.awchar/layers/mouth--smile.png"
        ),
    ),
];

#[test]
fn open_manifest_overlay_precedes_disk_decode() {
    let project = TestProject::new("topology-manifest-overlay");
    project.write("arcw.toml", &manifest("disk", "src/main.arcw", ""));
    project.write("src/main.arcw", ROOT_SOURCE);
    project.write("src/lib.arcw", ROOT_SOURCE);
    let overlays = vec![project.overlay("arcw.toml", &manifest("open", "src/lib.arcw", ""))];

    let topology = project.load(
        LaunchProfileSelection::Automatic { previous: None },
        &overlays,
    );

    assert_eq!(topology.selected_profile().id().as_str(), "open");
    let manifest = topology
        .resources()
        .find(|resource| resource.kind() == &ProfileTopologyResourceKind::Manifest)
        .expect("manifest retained");
    assert_eq!(manifest.origin(), ProfileTopologyResourceOrigin::Overlay);
    assert_eq!(
        manifest.text_document().expect("text manifest").text(),
        overlays[0].source().as_ref()
    );
}

#[test]
fn overlay_manifest_can_exist_without_disk_file() {
    let project = TestProject::new("topology-overlay-only-manifest");
    project.write("src/main.arcw", ROOT_SOURCE);
    let overlays = vec![project.overlay("arcw.toml", &manifest("dev", "src/main.arcw", ""))];

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &overlays);

    assert_eq!(topology.selected_profile().id().as_str(), "dev");
    assert_eq!(topology.consumed_overlay_ids().len(), 1);
}

#[test]
fn selected_source_overlay_builds_exact_import_closure() {
    let project = TestProject::new("topology-import-overlay");
    project.write("arcw.toml", &manifest("dev", "src/main.arcw", ""));
    project.write("src/unrelated.arcw", "mod crate.unrelated\n");
    let overlays = vec![
        project.overlay(
            "src/main.arcw",
            "use crate.feature.value\nfn main() -> Unit { () }\n",
        ),
        project.overlay(
            "src/feature.arcw",
            "mod crate.feature\nfn value() -> Unit { () }\n",
        ),
    ];

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &overlays);
    let logical_paths = topology
        .resources()
        .map(|resource| resource.id().path().as_str())
        .collect::<Vec<_>>();

    assert_eq!(logical_paths.len(), 3);
    assert!(logical_paths.contains(&"src/main.arcw"));
    assert!(logical_paths.contains(&"src/feature.arcw"));
    assert!(!logical_paths.contains(&"src/unrelated.arcw"));
}

#[test]
fn selected_source_outside_default_root_is_the_profile_crate_root() {
    let project = TestProject::new("topology-profile-root-outside-src");
    project.write("arcw.toml", &manifest("dev", "tests/smoke.arcw", ""));
    project.write(
        "tests/smoke.arcw",
        "use crate.feature.value\nfn main() -> Unit { () }\n",
    );
    project.write(
        "src/feature.arcw",
        "mod crate.feature\nfn value() -> Unit { () }\n",
    );

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &[]);
    let logical_paths = topology
        .resources()
        .map(|resource| resource.id().path().as_str())
        .collect::<Vec<_>>();

    assert!(logical_paths.contains(&"tests/smoke.arcw"));
    assert!(logical_paths.contains(&"src/feature.arcw"));
}

#[test]
fn selected_source_outside_default_root_rejects_non_root_module_declaration() {
    let project = TestProject::new("topology-profile-root-module-mismatch");
    project.write("arcw.toml", &manifest("dev", "tests/smoke.arcw", ""));
    project.write(
        "tests/smoke.arcw",
        "mod crate.feature\nfn main() -> Unit { () }\n",
    );

    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[]);

    let super::ProfileTopologyLoadError::ModuleDeclaration { id, source, .. } = error else {
        panic!("expected selected-root module declaration error");
    };
    let crate::project::ProjectLoadError::ModulePathMismatch {
        declared, expected, ..
    } = *source
    else {
        panic!("expected selected-root module path mismatch");
    };
    assert_eq!(id.path().as_str(), "tests/smoke.arcw");
    assert_eq!(declared.to_string(), "crate.feature");
    assert!(expected.is_crate_root());
}

#[test]
fn unresolved_import_reports_exact_import_source() {
    let project = TestProject::new("topology-unresolved-import");
    project.write("arcw.toml", &manifest("dev", "src/main.arcw", ""));
    project.write(
        "src/main.arcw",
        "use crate.missing.value\nfn main() -> Unit { () }\n",
    );

    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[]);

    let super::ProfileTopologyLoadError::ModuleImport {
        id, import, span, ..
    } = error
    else {
        panic!("expected module import error");
    };
    assert_eq!(id.path().as_str(), "src/main.arcw");
    assert_eq!(import.as_ref(), "crate.missing.value");
    assert!(span.is_some());
}

#[test]
fn character_content_root_uses_canonical_asset_package_path() {
    let project = TestProject::new("topology-character-content-root");
    project.write("arcw.toml", &character_profile_manifest());
    project.write("src/main.arcw", ROOT_SOURCE);
    let overlays = vec![project.overlay(
        "assets/zundamon.awchar/character.awchar.json",
        CHARACTER_MANIFEST,
    )];
    project.write_character_layers("assets/zundamon.awchar");

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &overlays);
    let character = topology
        .resources()
        .find(|resource| {
            matches!(
                resource.kind(),
                ProfileTopologyResourceKind::CharacterPackageManifest { .. }
            )
        })
        .expect("character retained");

    assert_eq!(character.origin(), ProfileTopologyResourceOrigin::Overlay);
    assert_eq!(
        character.id().path().as_str(),
        "assets/zundamon.awchar/character.awchar.json"
    );
}

#[test]
fn character_layers_accept_binary_overlays_and_retain_one_byte_authority() {
    let project = TestProject::new("topology-character-binary-overlays");
    project.write("arcw.toml", &character_profile_manifest());
    project.write("src/main.arcw", ROOT_SOURCE);
    let text_overlays = vec![project.overlay(
        "assets/zundamon.awchar/character.awchar.json",
        CHARACTER_MANIFEST,
    )];
    let binary_overlays = CHARACTER_LAYER_FIXTURES
        .iter()
        .map(|(relative, bytes)| {
            project.binary_overlay(&format!("assets/zundamon.awchar/{relative}"), *bytes)
        })
        .collect::<Vec<_>>();

    let topology = project.load_with_binary_overlays(
        LaunchProfileSelection::Explicit("dev"),
        &text_overlays,
        &binary_overlays,
    );
    let (_, loaded_package) = topology
        .character_packages()
        .next()
        .expect("character package retained");
    let layers = topology
        .resources()
        .filter(|resource| {
            matches!(
                resource.kind(),
                ProfileTopologyResourceKind::CharacterLayerPayload { .. }
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(layers.len(), CHARACTER_LAYER_FIXTURES.len());
    for resource in layers {
        let ProfileTopologyResourceKind::CharacterLayerPayload { asset, .. } = resource.kind()
        else {
            unreachable!("filtered to Character layers");
        };
        let binary = resource.binary_resource().expect("binary layer resource");
        let payload = loaded_package
            .package()
            .layer_payloads()
            .find(|payload| payload.path() == asset)
            .expect("package payload uses retained layer");
        assert_eq!(resource.origin(), ProfileTopologyResourceOrigin::Overlay);
        assert!(std::sync::Arc::ptr_eq(
            &binary.shared_bytes(),
            payload.shared_bytes()
        ));
    }
    assert_eq!(topology.watch_inventory().len(), topology.resources().len());
}

#[test]
fn corrupt_binary_overlay_does_not_fall_back_to_valid_disk_layer() {
    let project = TestProject::new("topology-character-corrupt-binary-overlay");
    project.write("arcw.toml", &character_profile_manifest());
    project.write("src/main.arcw", ROOT_SOURCE);
    project.write(
        "assets/zundamon.awchar/character.awchar.json",
        CHARACTER_MANIFEST,
    );
    project.write_character_layers("assets/zundamon.awchar");
    let binary_overlays = vec![project.binary_overlay(
        "assets/zundamon.awchar/layers/eyes--normal.png",
        &b"not a png"[..],
    )];

    let error = project.load_error_with_binary_overlays(
        LaunchProfileSelection::Explicit("dev"),
        &[],
        &binary_overlays,
    );

    assert!(matches!(
        error,
        super::ProfileTopologyLoadError::CharacterPackage {
            source,
            ..
        } if matches!(
            source.as_ref(),
            arcweft_character::package::CharacterPackageError::InvalidLayerPng { path, .. }
                if path.as_str() == "layers/eyes--normal.png"
        )
    ));
}

#[test]
fn text_and_binary_overlays_cannot_claim_the_same_path() {
    let project = TestProject::new("topology-overlay-kind-conflict");
    project.write("arcw.toml", &manifest("dev", "src/main.arcw", ""));
    project.write("src/main.arcw", ROOT_SOURCE);
    let text_overlays = vec![project.overlay("src/main.arcw", ROOT_SOURCE)];
    let binary_overlays = vec![project.binary_overlay("src/main.arcw", &b"binary"[..])];

    let error = project.load_error_with_binary_overlays(
        LaunchProfileSelection::Explicit("dev"),
        &text_overlays,
        &binary_overlays,
    );

    assert!(matches!(
        error,
        super::ProfileTopologyLoadError::DependencySeed {
            source: super::ProfileTopologySeedError::OverlayKindConflict { .. }
        }
    ));
}

#[test]
fn unconsumed_binary_overlay_is_rejected() {
    let project = TestProject::new("topology-unconsumed-binary-overlay");
    project.write("arcw.toml", &manifest("dev", "src/main.arcw", ""));
    project.write("src/main.arcw", ROOT_SOURCE);
    let binary_overlays = vec![project.binary_overlay("assets/unclaimed.png", &b"binary"[..])];

    let error = project.load_error_with_binary_overlays(
        LaunchProfileSelection::Explicit("dev"),
        &[],
        &binary_overlays,
    );

    assert!(matches!(
        error,
        super::ProfileTopologyLoadError::DependencySeed {
            source: super::ProfileTopologySeedError::UnconsumedBinaryOverlay { .. }
        }
    ));
}

#[test]
fn character_package_identity_must_match_the_selected_content_root() {
    let project = TestProject::new("topology-character-id-mismatch");
    project.write("arcw.toml", &character_profile_manifest());
    project.write("src/main.arcw", ROOT_SOURCE);
    project.write(
        "assets/zundamon.awchar/character.awchar.json",
        &CHARACTER_MANIFEST.replace("character.zundamon", "character.akane"),
    );

    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[]);

    assert!(matches!(
        error,
        super::ProfileTopologyLoadError::CharacterIdentityMismatch {
            expected,
            actual,
            ..
        } if expected.as_str() == "character.zundamon"
            && actual.as_str() == "character.akane"
    ));
}

#[test]
fn nested_character_identity_maps_to_nested_asset_package_path() {
    let project = TestProject::new("topology-nested-character-path");
    project.write(
        "arcw.toml",
        &character_profile_manifest().replace("@character.zundamon", "@character.npc.alice"),
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    let nested_manifest = CHARACTER_MANIFEST.replace("character.zundamon", "character.npc.alice");
    let overlays = vec![project.overlay(
        "assets/npc/alice.awchar/character.awchar.json",
        &nested_manifest,
    )];
    project.write_character_layers("assets/npc/alice.awchar");

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &overlays);
    let character = topology
        .resources()
        .find(|resource| {
            matches!(
                resource.kind(),
                ProfileTopologyResourceKind::CharacterPackageManifest { .. }
            )
        })
        .expect("nested character retained");

    assert_eq!(
        character.id().path().as_str(),
        "assets/npc/alice.awchar/character.awchar.json"
    );
}

#[test]
fn missing_or_invalid_selected_character_package_aborts_topology() {
    let missing = TestProject::new("topology-character-manifest-missing");
    missing.write("arcw.toml", &character_profile_manifest());
    missing.write("src/main.arcw", ROOT_SOURCE);
    let error = missing.load_error(LaunchProfileSelection::Explicit("dev"), &[]);
    assert!(matches!(
        error,
        super::ProfileTopologyLoadError::ResourceRead {
            kind: ProfileTopologyResourceKind::CharacterPackageManifest { .. },
            ..
        }
    ));

    let invalid = TestProject::new("topology-character-manifest-invalid");
    invalid.write("arcw.toml", &character_profile_manifest());
    invalid.write("src/main.arcw", ROOT_SOURCE);
    invalid.write("assets/zundamon.awchar/character.awchar.json", "{ invalid");
    let error = invalid.load_error(LaunchProfileSelection::Explicit("dev"), &[]);
    assert!(matches!(
        error,
        super::ProfileTopologyLoadError::CharacterManifest { .. }
    ));
}

#[test]
fn unselected_optional_content_unit_does_not_claim_a_file_backed_resource() {
    let project = TestProject::new("topology-unselected-optional-content");
    let manifest = character_profile_manifest()
        .replace("demand = \"required\"", "demand = \"optional\"")
        .replace(
            "[profiles.dev.content.characters]\nresidency = \"startup\"\nplacement = \"embedded\"\ncompression = \"none\"\n",
            "",
        );
    project.write("arcw.toml", &manifest);
    project.write("src/main.arcw", ROOT_SOURCE);

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &[]);

    assert!(!topology.resources().any(|resource| matches!(
        resource.kind(),
        ProfileTopologyResourceKind::CharacterPackageManifest { .. }
    )));
}

#[test]
fn selected_profile_uses_only_the_host_registry_adapter() {
    let project = TestProject::new("topology-host-adapter-selection");
    project.write(
        "arcw.toml",
        &manifest("dev", "src/main.arcw", "adapter = \"custom\""),
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    let registry =
        AdapterRegistry::new().with_manifest(AdapterManifest::new("custom", "Custom Host"));

    let topology =
        project.load_with_registry(LaunchProfileSelection::Explicit("dev"), &[], registry);

    assert_eq!(topology.adapter().id().as_str(), "custom");
    assert_eq!(topology.registration_adapter_manifests().len(), 1);
    assert_eq!(
        topology.registration_adapter_manifests()[0].id().as_str(),
        "custom"
    );
}

#[test]
fn selected_profile_owns_one_exact_adapter_effect_inventory() {
    let project = TestProject::new("topology-selected-adapter-effects");
    project.write(
        "arcw.toml",
        r#"schema = 1

[package]
id = "org.arcweft.topology-tests"
version = "0.1.0"

[profiles.read]
kind = "game"
entry = "@entry.game.main"
source = "src/main.arcw"
adapter = "reader"

[profiles.network]
kind = "game"
entry = "@entry.game.main"
source = "src/main.arcw"
adapter = "network"
"#,
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    let registry = AdapterRegistry::new()
        .with_manifest(
            AdapterManifest::new("reader", "Reader")
                .with_effect(AdapterEffectCapability::new("fs.read")),
        )
        .with_manifest(
            AdapterManifest::new("network", "Network")
                .with_effect(AdapterEffectCapability::new("net.read")),
        );

    let read = project.load_with_registry(
        LaunchProfileSelection::Explicit("read"),
        &[],
        registry.clone(),
    );
    assert_eq!(read.adapter().id().as_str(), "reader");
    assert_eq!(
        read.adapter()
            .effects()
            .iter()
            .map(AdapterEffectCapability::as_str)
            .collect::<Vec<_>>(),
        ["fs.read"]
    );

    let network =
        project.load_with_registry(LaunchProfileSelection::Explicit("network"), &[], registry);
    assert_eq!(network.adapter().id().as_str(), "network");
    assert_eq!(
        network
            .adapter()
            .effects()
            .iter()
            .map(AdapterEffectCapability::as_str)
            .collect::<Vec<_>>(),
        ["net.read"]
    );

    let parsed = parse_source(
        r"
extern capability fs {
    fn read() -> String effects { fs.read }
}
flow @flow.main main effects { fs.read } {
    let body = fs.read()
}
",
    );
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = lower_to_hir(parsed.typed_tree()).expect("capability fixture lowers");

    let read_env = read.adapter().declare_target_effects(TypeCheckEnv::new());
    typecheck_hir(&hir, &read_env).expect("selected reader adapter grants fs.read");

    let network_env = network
        .adapter()
        .declare_target_effects(TypeCheckEnv::new());
    let errors =
        typecheck_hir(&hir, &network_env).expect_err("selected network adapter lacks fs.read");
    assert!(errors.iter().any(|error| {
        matches!(
            error.kind(),
            TypeCheckErrorKind::Effect { diagnostic }
                if diagnostic.code() == EffectDiagnosticCode::CapabilityUnavailable
        )
    }));
}

#[test]
fn selected_host_adapter_must_exist() {
    let project = TestProject::new("topology-missing-host-adapter");
    project.write(
        "arcw.toml",
        &manifest("dev", "src/main.arcw", "adapter = \"missing\""),
    );
    project.write("src/main.arcw", ROOT_SOURCE);

    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[]);

    assert_eq!(error.code(), ProfileTopologyErrorCode::AdapterSelection);
}

#[test]
fn generated_metadata_publishes_exact_mounted_type_function_and_activity_facts() {
    let project = TestProject::new("topology-generated-facts");
    let metadata = metadata_with_function();
    project.write(
        "arcw.toml",
        &external_module_manifest(&metadata, Some("activity.truck_game"), None),
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    project.write("generated/truck.adapter.json", &metadata);

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &[]);

    assert_eq!(topology.external_modules().len(), 1);
    assert_eq!(
        topology.external_modules()[0].document().text(),
        metadata.as_str()
    );
    let retained = topology.external_modules()[0].metadata().metadata();
    assert_eq!(
        retained.exports.types[1].visibility,
        ManifestVisibility::Private
    );
    assert_eq!(retained.exports.functions[0].purity, FunctionPurity::Pure);
    assert!(topology.adapter().symbols().iter().any(|symbol| {
        symbol.path().to_string() == "mini_games.truck.TruckResult"
            && matches!(
                symbol.ty(),
                AdapterTypeKind::Nominal { nominal }
                    if matches!(nominal.owner(), AdapterNominalOwner::Environment { owner }
                        if owner.as_str() == "adapter:sans-io")
                        && nominal
                            .path()
                            .segments()
                            .iter()
                            .map(arcweft_adapter_context::manifest::AdapterNominalPathSegment::as_str)
                            .eq(["mini_games", "truck", "TruckResult"])
                        && nominal.arguments().is_empty()
            )
    }));
    assert!(
        !topology
            .adapter()
            .symbols()
            .iter()
            .any(|symbol| symbol.path().to_string().ends_with("TruckTelemetry")),
        "private metadata exports are not project-visible"
    );
    assert!(
        topology
            .adapter()
            .nominal_declarations()
            .iter()
            .any(|declaration| {
                declaration.visibility() == AdapterNominalVisibility::Private
                    && declaration
                        .path()
                        .segments()
                        .iter()
                        .map(arcweft_adapter_context::manifest::AdapterNominalPathSegment::as_str)
                        .eq(["mini_games", "truck", "TruckTelemetry"])
            })
    );
    let function = topology
        .adapter()
        .functions()
        .iter()
        .find(|function| {
            function
                .path()
                .segments()
                .iter()
                .map(AdapterCallableName::as_str)
                .collect::<Vec<_>>()
                == ["mini_games", "truck", "drive"]
        })
        .expect("mounted generated function");
    assert_eq!(function.signature().groups()[0].parameters().len(), 1);
    assert_eq!(topology.registration_adapter_manifests().len(), 1);
}

#[test]
fn generated_projection_rejects_purity_and_effect_mismatch_without_altering_retained_shape() {
    let project = TestProject::new("topology-generated-purity-mismatch");
    let metadata = metadata_with_inconsistent_purity();
    project.write(
        "arcw.toml",
        &external_module_manifest(&metadata, None, None),
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    project.write("generated/truck.adapter.json", &metadata);

    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[]);

    assert!(matches!(
        error,
        super::ProfileTopologyLoadError::ExternalModuleFacts(
            super::ExternalModuleFactsError::FunctionPurity { purity: "pure", .. }
        )
    ));
}

#[test]
fn generated_metadata_overlay_is_the_only_admitted_revision() {
    let project = TestProject::new("topology-generated-overlay");
    let metadata = metadata_with_function();
    project.write(
        "arcw.toml",
        &external_module_manifest(&metadata, Some("activity.truck_game"), None),
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    project.write("generated/truck.adapter.json", TRUCK_METADATA);
    let overlays = vec![project.overlay("generated/truck.adapter.json", &metadata)];

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &overlays);
    let resource = topology
        .resources()
        .find(|resource| {
            matches!(
                resource.kind(),
                ProfileTopologyResourceKind::ExternalModuleMetadata { .. }
            )
        })
        .expect("metadata resource retained");

    assert_eq!(resource.origin(), ProfileTopologyResourceOrigin::Overlay);
    assert_eq!(
        resource.text_document().expect("text metadata").text(),
        metadata
    );
}

#[test]
fn generated_metadata_raw_hash_mismatch_aborts_topology() {
    let project = TestProject::new("topology-generated-hash-mismatch");
    project.write(
        "arcw.toml",
        &external_module_manifest(TRUCK_METADATA, None, None),
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    project.write(
        "generated/truck.adapter.json",
        &format!("{TRUCK_METADATA}\n"),
    );

    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[]);

    let super::ProfileTopologyLoadError::ExternalModuleMetadataHash { id, .. } = error else {
        panic!("expected generated metadata hash error");
    };
    assert_eq!(id.owner(), &project.owner());
    assert_eq!(id.path().as_str(), "generated/truck.adapter.json");
}

#[test]
fn generated_metadata_decode_failure_aborts_topology_after_exact_hash() {
    let project = TestProject::new("topology-generated-decode-failure");
    let malformed = "{ malformed";
    project.write(
        "arcw.toml",
        &external_module_manifest(malformed, None, None),
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    project.write("generated/truck.adapter.json", malformed);

    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[]);

    let super::ProfileTopologyLoadError::ExternalModuleMetadataDecode { id, .. } = error else {
        panic!("expected generated metadata decode error");
    };
    assert_eq!(id.owner(), &project.owner());
    assert_eq!(id.path().as_str(), "generated/truck.adapter.json");
}

#[test]
fn every_generated_metadata_expectation_mismatch_aborts_topology() {
    let canonical = external_module_manifest(TRUCK_METADATA, None, None);
    for (field, value) in [
        ("expected-package", "com.example.not-truck"),
        ("expected-version", "9.9.9"),
        ("expected-module", "not_truck"),
        (
            "expected-abi-hash",
            "blake3:0000000000000000000000000000000000000000000000000000000000000000",
        ),
    ] {
        let project = TestProject::new(&format!("topology-generated-{field}-mismatch"));
        project.write(
            "arcw.toml",
            &replace_manifest_string_field(&canonical, field, value),
        );
        project.write("src/main.arcw", ROOT_SOURCE);
        project.write("generated/truck.adapter.json", TRUCK_METADATA);

        let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[]);

        let expected_field = field.strip_prefix("expected-").expect("expected field");
        assert!(
            matches!(
                &error,
                super::ProfileTopologyLoadError::ExternalModuleMetadataExpectation {
                    field: actual,
                    ..
                } if *actual == expected_field
            ),
            "{field}: {error:?}"
        );
    }
}

#[test]
fn expected_family_mismatch_rejects_rust_wasm_and_process_metadata() {
    let families = [
        ("rust", TRUCK_METADATA.to_owned(), "wasm"),
        (
            "wasm",
            metadata_with_target(AdapterTarget::Wasm(WasmTarget {
                abi: WasmAbi,
                world: WitWorldId::new("arcweft:test/truck").expect("WIT world"),
            })),
            "process",
        ),
        (
            "process",
            metadata_with_target(AdapterTarget::Process(ProcessTarget {
                abi: ProcessAbi,
                transport: ProcessTransport,
            })),
            "rust",
        ),
    ];
    for (actual_family, metadata, wrong_family) in families {
        let project = TestProject::new(&format!(
            "topology-generated-{actual_family}-family-mismatch"
        ));
        let manifest = external_module_manifest(&metadata, None, None);
        project.write(
            "arcw.toml",
            &replace_manifest_string_field(&manifest, "expected-family", wrong_family),
        );
        project.write("src/main.arcw", ROOT_SOURCE);
        project.write("generated/truck.adapter.json", &metadata);

        let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[]);

        assert!(matches!(
            &error,
            super::ProfileTopologyLoadError::ExternalModuleMetadataExpectation {
                field: "family",
                actual,
                ..
            } if actual == actual_family
        ));
    }
}

#[test]
fn activity_binding_requires_the_selected_generated_export() {
    let project = TestProject::new("topology-generated-activity-missing");
    project.write(
        "arcw.toml",
        &external_module_manifest(TRUCK_METADATA, Some("activity.truck_game"), None)
            .replace("export = \"truck_game\"", "export = \"missing\""),
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    project.write("generated/truck.adapter.json", TRUCK_METADATA);

    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[]);

    assert!(matches!(
        error,
        super::ProfileTopologyLoadError::ExternalModuleFacts(
            super::ExternalModuleFactsError::ActivityExportMissing { .. }
        )
    ));
}

#[test]
fn activity_binding_requires_the_exact_abstract_activity_identity() {
    let project = TestProject::new("topology-generated-activity-identity");
    project.write(
        "arcw.toml",
        &external_module_manifest(TRUCK_METADATA, Some("activity.other"), None),
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    project.write("generated/truck.adapter.json", TRUCK_METADATA);

    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[]);

    assert!(matches!(
        error,
        super::ProfileTopologyLoadError::ExternalModuleFacts(
            super::ExternalModuleFactsError::ActivityIdentityMismatch { .. }
        )
    ));
}

#[test]
fn consumed_overlay_ids_are_sorted_and_complete() {
    let project = TestProject::new("topology-consumed-overlay-order");
    project.write_character_layers("assets/zundamon.awchar");
    let overlays = vec![
        project.overlay("arcw.toml", &character_profile_manifest()),
        project.overlay(
            "src/main.arcw",
            "use crate.feature\nfn main() -> Unit { () }\n",
        ),
        project.overlay("src/feature.arcw", "mod crate.feature\n"),
        project.overlay(
            "assets/zundamon.awchar/character.awchar.json",
            CHARACTER_MANIFEST,
        ),
    ];

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &overlays);
    let actual = topology
        .consumed_overlay_ids()
        .map(|id| id.path().as_str().to_owned())
        .collect::<Vec<_>>();
    let mut expected = actual.clone();
    expected.sort();

    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 4);
}

#[test]
fn topology_source_revision_covers_every_retained_resource_identity() {
    let project = TestProject::new("topology-complete-source-revision");
    let manifest = format!(
        "{}\n[content-units.characters]\nroots = [\"@character.zundamon\"]\nvisibility = \"package\"\ndemand = \"required\"\n[profiles.dev.content.characters]\nresidency = \"startup\"\nplacement = \"embedded\"\ncompression = \"none\"\n",
        external_module_manifest(TRUCK_METADATA, None, None)
    );
    project.write("arcw.toml", &manifest);
    project.write("src/main.arcw", ROOT_SOURCE);
    project.write("generated/truck.adapter.json", TRUCK_METADATA);
    let overlays = vec![project.overlay(
        "assets/zundamon.awchar/character.awchar.json",
        CHARACTER_MANIFEST,
    )];
    project.write_character_layers("assets/zundamon.awchar");

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &overlays);
    let expected = SourceSetRevision::try_for_identities(
        topology
            .resources()
            .filter_map(|resource| resource.text_document())
            .map(|document| document.identity()),
    )
    .expect("retained source identities form one revision");

    assert_eq!(topology.source_documents_revision(), expected);
    assert!(
        topology
            .resources()
            .any(|resource| matches!(resource.kind(), ProfileTopologyResourceKind::Manifest))
    );
    assert!(topology.resources().any(|resource| matches!(
        resource.kind(),
        ProfileTopologyResourceKind::ArcweftModule { .. }
    )));
    assert!(topology.resources().any(|resource| matches!(
        resource.kind(),
        ProfileTopologyResourceKind::ExternalModuleMetadata { .. }
    )));
    assert!(topology.resources().any(|resource| matches!(
        resource.kind(),
        ProfileTopologyResourceKind::CharacterPackageManifest { .. }
    )));
}

#[test]
fn loaded_topology_survives_disk_mutation() {
    let project = TestProject::new("topology-retains-bytes");
    let manifest_text = manifest("dev", "src/main.arcw", "");
    project.write("arcw.toml", &manifest_text);
    project.write("src/main.arcw", ROOT_SOURCE);

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &[]);
    fs::remove_file(project.path("arcw.toml")).expect("manifest removed");
    fs::remove_file(project.path("src/main.arcw")).expect("source removed");

    assert_eq!(
        topology.loaded_project().manifest_document().text(),
        manifest_text
    );
    let root = topology
        .loaded_project()
        .module_documents()
        .next()
        .expect("root module retained")
        .1;
    assert_eq!(root.text(), ROOT_SOURCE);
}

#[test]
fn one_path_cannot_be_claimed_as_source_and_generated_metadata() {
    let project = TestProject::new("topology-duplicate-path");
    project.write(
        "arcw.toml",
        &external_module_manifest_with_path(ROOT_SOURCE, "src/main.arcw"),
    );
    project.write("src/main.arcw", ROOT_SOURCE);

    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[]);

    assert_eq!(error.code(), ProfileTopologyErrorCode::DuplicatePath);
}

#[test]
fn topology_diagnostics_are_bounded() {
    let project = TestProject::new("topology-bounded-diagnostics");
    project.write("arcw.toml", &manifest("dev", "src/main.arcw", ""));
    let mut source = String::new();
    for index in 0..129 {
        writeln!(source, "unknown_{index} {{}}").expect("writing to String succeeds");
    }
    project.write("src/main.arcw", &source);

    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[]);

    let super::ProfileTopologyLoadError::ModuleSyntax {
        diagnostics,
        truncated,
        ..
    } = error
    else {
        panic!("expected module syntax error");
    };
    assert!(truncated);
    assert_eq!(
        diagnostics.len(),
        usize::try_from(ProfileTopologyLimits::PRODUCTION.diagnostics())
            .expect("diagnostic limit fits usize")
    );
    assert!(
        diagnostics
            .last()
            .is_some_and(|message| message.contains("diagnostic limit exceeded"))
    );
}

#[test]
fn topology_single_resource_byte_limit_is_inclusive() {
    let project = TestProject::new("topology-source-byte-limit");
    project.write("src/main.arcw", ROOT_SOURCE);
    let maximum = usize::try_from(ProfileTopologyLimits::PRODUCTION.source_bytes())
        .expect("source limit fits usize");
    let exact = padded_toml(&manifest("dev", "src/main.arcw", ""), maximum);
    let exact_overlays = vec![project.overlay("arcw.toml", &exact)];

    project.load(LaunchProfileSelection::Explicit("dev"), &exact_overlays);

    let one_over = format!("{exact}x");
    let one_overlays = vec![project.overlay("arcw.toml", &one_over)];
    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &one_overlays);
    assert!(matches!(
        error,
        super::ProfileTopologyLoadError::Limit {
            kind: super::ProfileTopologyLimitKind::SourceBytes,
            observed,
            maximum: limit,
        } if observed == ProfileTopologyLimits::PRODUCTION.source_bytes() + 1
            && limit == ProfileTopologyLimits::PRODUCTION.source_bytes()
    ));
}

#[test]
fn topology_resource_limit_is_inclusive() {
    let exact_project = TestProject::new("topology-resource-limit-exact");
    exact_project.write("arcw.toml", &manifest("dev", "src/main.arcw", ""));
    let exact_module_count = usize::try_from(ProfileTopologyLimits::PRODUCTION.resources() - 2)
        .expect("resource limit fits usize");
    let exact_overlays = module_overlays(&exact_project, exact_module_count);

    let topology = exact_project.load(LaunchProfileSelection::Explicit("dev"), &exact_overlays);
    assert_eq!(
        u64::try_from(topology.resources().len()).expect("resource count fits u64"),
        ProfileTopologyLimits::PRODUCTION.resources()
    );

    let one_over_project = TestProject::new("topology-resource-limit-one-over");
    one_over_project.write("arcw.toml", &manifest("dev", "src/main.arcw", ""));
    let one_overlays = module_overlays(&one_over_project, exact_module_count + 1);
    let error = one_over_project.load_error(LaunchProfileSelection::Explicit("dev"), &one_overlays);
    assert!(matches!(
        error,
        super::ProfileTopologyLoadError::Limit {
            kind: super::ProfileTopologyLimitKind::Resources,
            observed,
            maximum,
        } if observed == ProfileTopologyLimits::PRODUCTION.resources() + 1
            && maximum == ProfileTopologyLimits::PRODUCTION.resources()
    ));
}

fn manifest(profile: &str, source: &str, extra: &str) -> String {
    format!(
        r#"schema = 1

[package]
id = "org.arcweft.topology-tests"
version = "0.1.0"

[profiles."{profile}"]
kind = "game"
entry = "@entry.game.main"
source = "{source}"
{extra}
"#
    )
}

fn character_profile_manifest() -> String {
    r#"schema = 1

[package]
id = "org.arcweft.topology-tests"
version = "0.1.0"

[content-units.characters]
roots = ["@character.zundamon"]
visibility = "package"
demand = "required"

[profiles.dev]
kind = "game"
entry = "@entry.game.main"
source = "src/main.arcw"

[profiles.dev.content.characters]
residency = "startup"
placement = "embedded"
compression = "none"
"#
    .to_owned()
}

fn external_module_manifest(
    metadata: &str,
    activity: Option<&str>,
    expected_package: Option<&str>,
) -> String {
    external_module_manifest_with(
        metadata,
        "generated/truck.adapter.json",
        activity,
        expected_package,
    )
}

fn external_module_manifest_with_path(metadata: &str, path: &str) -> String {
    external_module_manifest_with(metadata, path, None, None)
}

fn external_module_manifest_with(
    metadata: &str,
    path: &str,
    activity: Option<&str>,
    expected_package: Option<&str>,
) -> String {
    let decoded: serde_json::Value = serde_json::from_str(metadata).unwrap_or_else(|_| {
        serde_json::json!({
            "abi_hash": "blake3:0000000000000000000000000000000000000000000000000000000000000000",
            "package": { "id": "com.example.truck", "version": "1.2.3" },
            "module": { "id": "truck" },
            "target": { "family": "rust" },
        })
    });
    let package = expected_package.unwrap_or_else(|| {
        decoded["package"]["id"]
            .as_str()
            .expect("metadata package ID")
    });
    let version = decoded["package"]["version"]
        .as_str()
        .expect("metadata package version");
    let module = decoded["module"]["id"]
        .as_str()
        .expect("metadata module ID");
    let family = decoded["target"]["family"]
        .as_str()
        .expect("metadata target family");
    let abi_hash = decoded["abi_hash"].as_str().expect("metadata ABI hash");
    let raw_hash = RawDigest::for_bytes(metadata.as_bytes());
    let implementation = activity.map_or(String::new(), |_| {
        r#"
[activity-implementations.truck]
module = "truck"
export = "truck_game"
"#
        .to_owned()
    });
    let binding = activity.map_or(String::new(), |activity| {
        format!(
            "activity-bindings = [{{ activity = \"{activity}\", implementation = \"truck\" }}]\n"
        )
    });
    format!(
        r#"schema = 1

[package]
id = "org.arcweft.topology-tests"
version = "0.1.0"

[external-modules.truck]
mount = "mini_games.truck"
metadata = "{path}"
metadata-hash = "{raw_hash}"
expected-package = "{package}"
expected-version = "{version}"
expected-module = "{module}"
expected-family = "{family}"
expected-abi-hash = "{abi_hash}"
visibility = "package"
demand = "required"
{implementation}
[profiles.dev]
kind = "game"
entry = "@entry.game.main"
source = "src/main.arcw"
external-modules = ["truck"]
{binding}"#
    )
}

fn metadata_with_function() -> String {
    let mut metadata: AdapterMetadata =
        serde_json::from_str(TRUCK_METADATA).expect("canonical metadata fixture");
    metadata.exports.functions.push(AdapterFunctionExport {
        name: FunctionName::new("drive").expect("function name"),
        visibility: arcweft_manifest_model::ManifestVisibility::Public,
        params: vec![AdapterParameter {
            name: FieldName::new("request").expect("field name"),
            ty: TypeReference::new("TruckResult").expect("type reference"),
        }],
        return_type: TypeReference::new("Need<TruckResult, TruckResult>").expect("type reference"),
        purity: FunctionPurity::Pure,
        effects: Vec::new(),
    });
    metadata.abi_hash = metadata.computed_abi_hash().expect("ABI hash");
    metadata.payload_hash = metadata.computed_payload_hash().expect("payload hash");
    serde_json::to_string_pretty(&metadata).expect("metadata JSON")
}

fn metadata_with_target(target: AdapterTarget) -> String {
    let mut metadata: AdapterMetadata =
        serde_json::from_str(TRUCK_METADATA).expect("canonical metadata fixture");
    metadata.target = target;
    metadata.abi_hash = metadata.computed_abi_hash().expect("ABI hash");
    metadata.payload_hash = metadata.computed_payload_hash().expect("payload hash");
    serde_json::to_string_pretty(&metadata).expect("metadata JSON")
}

fn metadata_with_inconsistent_purity() -> String {
    let mut metadata: AdapterMetadata =
        serde_json::from_str(&metadata_with_function()).expect("generated metadata");
    metadata.exports.functions[0]
        .effects
        .push(CapabilityId::new("truck.drive").expect("capability ID"));
    metadata.abi_hash = metadata.computed_abi_hash().expect("ABI hash");
    metadata.payload_hash = metadata.computed_payload_hash().expect("payload hash");
    serde_json::to_string_pretty(&metadata).expect("metadata JSON")
}

fn replace_manifest_string_field(manifest: &str, field: &str, value: &str) -> String {
    let prefix = format!("{field} = ");
    manifest
        .lines()
        .map(|line| {
            if line.starts_with(&prefix) {
                format!("{field} = \"{value}\"")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn padded_toml(base: &str, length: usize) -> String {
    assert!(base.len() < length);
    let mut padded = String::with_capacity(length);
    padded.push_str(base);
    padded.push('#');
    padded.extend(std::iter::repeat_n('x', length - base.len() - 1));
    padded
}

fn module_overlays(project: &TestProject, module_count: usize) -> Vec<ProfileTopologyOverlaySeed> {
    let mut root = String::new();
    let mut overlays = Vec::with_capacity(module_count + 1);
    for index in 0..module_count {
        let module = format!("m{index:04}");
        root.push_str("use crate.");
        root.push_str(&module);
        root.push('\n');
        overlays.push(project.overlay(
            &format!("src/{module}.arcw"),
            &format!("mod crate.{module}\n"),
        ));
    }
    root.push_str(ROOT_SOURCE);
    overlays.push(project.overlay("src/main.arcw", &root));
    overlays
}

fn slash(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

struct TestProject {
    root: PathBuf,
}

impl TestProject {
    fn new(label: &str) -> Self {
        let unique = format!(
            "arcweft-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock follows epoch")
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        fs::create_dir_all(&root).expect("fixture root");
        Self { root }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn write(&self, relative: &str, contents: &str) {
        self.write_bytes(relative, contents.as_bytes());
    }

    fn write_bytes(&self, relative: &str, contents: &[u8]) {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture directory");
        }
        fs::write(path, contents).expect("fixture file");
    }

    fn write_character_layers(&self, package_root: &str) {
        for (relative, bytes) in CHARACTER_LAYER_FIXTURES {
            self.write_bytes(&format!("{package_root}/{relative}"), bytes);
        }
    }

    fn overlay(&self, relative: &str, contents: &str) -> ProfileTopologyOverlaySeed {
        ProfileTopologyOverlaySeed::try_new(self.path(relative), contents.to_owned())
            .expect("normalized overlay")
    }

    fn binary_overlay(
        &self,
        relative: &str,
        bytes: impl Into<std::sync::Arc<[u8]>>,
    ) -> ProfileTopologyBinaryOverlaySeed {
        ProfileTopologyBinaryOverlaySeed::try_new(self.path(relative), bytes)
            .expect("normalized binary overlay")
    }

    fn owner(&self) -> ProfileTopologyOwnerId {
        ProfileTopologyOwnerId::workspace(
            format!("file:///{}", slash(&self.root)),
            format!("file:///{}", slash(&self.path("arcw.toml"))),
        )
        .expect("workspace owner")
    }

    fn load(
        &self,
        selection: LaunchProfileSelection<'_>,
        overlays: &[ProfileTopologyOverlaySeed],
    ) -> super::LoadedProfileTopology {
        self.load_with_registry(selection, overlays, standard_registry())
    }

    fn load_with_registry(
        &self,
        selection: LaunchProfileSelection<'_>,
        overlays: &[ProfileTopologyOverlaySeed],
        registry: AdapterRegistry,
    ) -> super::LoadedProfileTopology {
        let manifest_path = self.path("arcw.toml");
        load_profile_topology(ProfileTopologyLoadRequest::new(
            &manifest_path,
            self.owner(),
            selection,
            overlays,
            registry,
        ))
        .expect("topology loads")
    }

    fn load_with_binary_overlays(
        &self,
        selection: LaunchProfileSelection<'_>,
        overlays: &[ProfileTopologyOverlaySeed],
        binary_overlays: &[ProfileTopologyBinaryOverlaySeed],
    ) -> super::LoadedProfileTopology {
        let manifest_path = self.path("arcw.toml");
        load_profile_topology(
            ProfileTopologyLoadRequest::new(
                &manifest_path,
                self.owner(),
                selection,
                overlays,
                standard_registry(),
            )
            .with_binary_overlays(binary_overlays),
        )
        .expect("topology loads")
    }

    fn load_error(
        &self,
        selection: LaunchProfileSelection<'_>,
        overlays: &[ProfileTopologyOverlaySeed],
    ) -> super::ProfileTopologyLoadError {
        let manifest_path = self.path("arcw.toml");
        load_profile_topology(ProfileTopologyLoadRequest::new(
            &manifest_path,
            self.owner(),
            selection,
            overlays,
            standard_registry(),
        ))
        .expect_err("topology fails")
    }

    fn load_error_with_binary_overlays(
        &self,
        selection: LaunchProfileSelection<'_>,
        overlays: &[ProfileTopologyOverlaySeed],
        binary_overlays: &[ProfileTopologyBinaryOverlaySeed],
    ) -> super::ProfileTopologyLoadError {
        let manifest_path = self.path("arcw.toml");
        load_profile_topology(
            ProfileTopologyLoadRequest::new(
                &manifest_path,
                self.owner(),
                selection,
                overlays,
                standard_registry(),
            )
            .with_binary_overlays(binary_overlays),
        )
        .expect_err("topology fails")
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
