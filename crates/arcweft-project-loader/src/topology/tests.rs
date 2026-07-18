use super::{
    LoadedDocumentOwnership, ProfileDependencyResourceSeed, ProfileTopologyErrorCode,
    ProfileTopologyLimits, ProfileTopologyLoadRequest, ProfileTopologyLogicalPath,
    ProfileTopologyOverlaySeed, ProfileTopologyOwnerId, ProfileTopologyResourceId,
    ProfileTopologyResourceKind, ProfileTopologyResourceOrigin, load_profile_topology,
};
use arcweft_adapter_context::{manifest::AdapterEffectCapability, standard::standard_registry};
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_sema::{
    check::typecheck_hir, diagnostics::TypeCheckErrorKind,
    effect_diagnostics::EffectDiagnosticCode, env::TypeCheckEnv,
};
use arcweft_lang_syntax::parser::parse_source;
use arcweft_launch::LaunchProfileSelection;
use arcweft_rust_abi::{
    ArcweftRustFunction, ArcweftRustManifest, ArcweftRustPackage, ArcweftRustPurity,
    ArcweftRustTypeRef,
};
use arcweft_source::SourceDocumentId;
use std::{fmt::Write as _, fs, path::PathBuf};

const ROOT_SOURCE: &str = "fn main() -> Unit { () }\n";

#[test]
fn open_manifest_overlay_precedes_disk_parse() {
    let project = TestProject::new("topology-manifest-overlay");
    project.write("arcw.toml", &manifest("disk", "src/main.arcw", ""));
    project.write("src/main.arcw", ROOT_SOURCE);
    project.write("src/lib.arcw", ROOT_SOURCE);
    let overlays = vec![project.overlay("arcw.toml", &manifest("open", "src/lib.arcw", ""))];

    let topology = project.load(
        LaunchProfileSelection::Automatic { previous: None },
        &overlays,
        &[],
    );

    assert_eq!(topology.selected_profile().id().as_str(), "open");
    let manifest = topology
        .resources()
        .find(|resource| resource.kind() == &ProfileTopologyResourceKind::Manifest)
        .expect("manifest retained");
    assert_eq!(manifest.origin(), ProfileTopologyResourceOrigin::Overlay);
    assert_eq!(manifest.document().text(), overlays[0].source().as_ref());
}

#[test]
fn overlay_manifest_can_exist_without_disk_file() {
    let project = TestProject::new("topology-overlay-only-manifest");
    project.write("src/main.arcw", ROOT_SOURCE);
    let overlays = vec![project.overlay("arcw.toml", &manifest("dev", "src/main.arcw", ""))];

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &overlays, &[]);

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

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &overlays, &[]);
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

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &[], &[]);
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

    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[], &[]);

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

    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[], &[]);

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
fn awchar_suffix_resolves_without_directory_probe() {
    let project = TestProject::new("topology-awchar-overlay");
    project.write(
        "arcw.toml",
        &manifest(
            "dev",
            "src/main.arcw",
            "character_manifests = [\"characters/zundamon.awchar\"]",
        ),
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    let overlays = vec![project.overlay(
        "characters/zundamon.awchar/character.awchar.json",
        include_str!(
            "../../../arcweft-character/tests/fixtures/zundamon.awchar/character.awchar.json"
        ),
    )];

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &overlays, &[]);
    let character = topology
        .resources()
        .find(|resource| resource.kind() == &ProfileTopologyResourceKind::CharacterManifest)
        .expect("character retained");

    assert_eq!(character.origin(), ProfileTopologyResourceOrigin::Overlay);
    assert_eq!(
        character.id().path().as_str(),
        "characters/zundamon.awchar/character.awchar.json"
    );
}

#[test]
fn direct_character_manifest_path_remains_direct() {
    let project = TestProject::new("topology-direct-character");
    project.write(
        "arcw.toml",
        &manifest(
            "dev",
            "src/main.arcw",
            "character_manifests = [\"characters/zundamon.json\"]",
        ),
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    let overlays = vec![project.overlay(
        "characters/zundamon.json",
        include_str!(
            "../../../arcweft-character/tests/fixtures/zundamon.awchar/character.awchar.json"
        ),
    )];

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &overlays, &[]);

    assert!(topology.resources().any(|resource| {
        resource.kind() == &ProfileTopologyResourceKind::CharacterManifest
            && resource.id().path().as_str() == "characters/zundamon.json"
    }));
}

#[test]
fn adapter_overlay_decodes_before_adapter_selection() {
    let project = TestProject::new("topology-adapter-overlay");
    project.write(
        "arcw.toml",
        &manifest(
            "dev",
            "src/main.arcw",
            "adapter = \"custom-overlay\"\nadapter_manifests = [\"adapters/custom.toml\"]",
        ),
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    project.write("adapters/custom.toml", &adapter_manifest("disk-other"));
    let overlays =
        vec![project.overlay("adapters/custom.toml", &adapter_manifest("custom-overlay"))];

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &overlays, &[]);

    assert_eq!(topology.adapter().id().as_str(), "custom-overlay");
    assert_eq!(topology.registration_adapter_manifests().len(), 1);
    assert_eq!(
        topology.registration_adapter_manifests()[0].id().as_str(),
        "custom-overlay"
    );
}

#[test]
fn selected_profile_owns_one_exact_adapter_effect_inventory() {
    let project = TestProject::new("topology-selected-adapter-effects");
    project.write(
        "arcw.toml",
        r#"[package]
name = "topology-tests"
version = "0.1.0"

[profiles.read]
kind = "game"
entry = "entry.game.main"
source = "src/main.arcw"
adapter = "reader"
adapter_manifests = ["adapters/reader.toml", "adapters/network.toml"]

[profiles.network]
kind = "game"
entry = "entry.game.main"
source = "src/main.arcw"
adapter = "network"
adapter_manifests = ["adapters/reader.toml", "adapters/network.toml"]
"#,
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    project.write(
        "adapters/reader.toml",
        &adapter_manifest_with_effects("reader", &["fs.read"]),
    );
    project.write(
        "adapters/network.toml",
        &adapter_manifest_with_effects("network", &["net.read"]),
    );

    let read = project.load(LaunchProfileSelection::Explicit("read"), &[], &[]);
    assert_eq!(read.selected_profile().id().as_str(), "read");
    assert_eq!(read.adapter().id().as_str(), "reader");
    assert_eq!(
        read.adapter()
            .effects()
            .iter()
            .map(AdapterEffectCapability::as_str)
            .collect::<Vec<_>>(),
        ["fs.read"]
    );
    assert_eq!(read.adapter_sources().len(), 2);
    assert_eq!(read.registration_adapter_manifests().len(), 1);
    assert_eq!(
        read.registration_adapter_manifests()[0].id().as_str(),
        "reader"
    );

    let network = project.load(LaunchProfileSelection::Explicit("network"), &[], &[]);
    assert_eq!(network.selected_profile().id().as_str(), "network");
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
    assert_eq!(network.registration_adapter_manifests().len(), 1);
    assert_eq!(
        network.registration_adapter_manifests()[0].id().as_str(),
        "network"
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

    let read_env = read.adapter().apply_to_target_env(TypeCheckEnv::new());
    typecheck_hir(&hir, &read_env).expect("selected reader adapter grants fs.read");

    let network_env = network.adapter().apply_to_target_env(TypeCheckEnv::new());
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
fn duplicate_adapter_ids_are_rejected() {
    let project = TestProject::new("topology-duplicate-adapter");
    project.write(
        "arcw.toml",
        &manifest(
            "dev",
            "src/main.arcw",
            "adapter = \"duplicate\"\nadapter_manifests = [\"adapters/a.toml\", \"adapters/b.toml\"]",
        ),
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    project.write("adapters/a.toml", &adapter_manifest("duplicate"));
    project.write("adapters/b.toml", &adapter_manifest("duplicate"));

    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[], &[]);

    assert_eq!(error.code(), ProfileTopologyErrorCode::DuplicateAdapterId);
}

#[test]
fn declared_adapter_failure_has_no_sans_io_fallback() {
    let project = TestProject::new("topology-malformed-adapter");
    project.write(
        "arcw.toml",
        &manifest(
            "dev",
            "src/main.arcw",
            "adapter = \"custom\"\nadapter_manifests = [\"adapters/custom.toml\"]",
        ),
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    project.write("adapters/custom.toml", "schema_version = ");

    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[], &[]);

    assert_eq!(error.code(), ProfileTopologyErrorCode::AdapterManifest);
}

#[test]
fn rust_metadata_overlay_applies_before_base_environment() {
    let project = TestProject::new("topology-rust-overlay");
    project.write(
        "arcw.toml",
        &manifest(
            "dev",
            "src/main.arcw",
            "adapter = \"custom\"\nadapter_manifests = [\"adapters/custom.toml\"]\nrust_metadata = [\"metadata/custom.json\"]",
        ),
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    project.write("adapters/custom.toml", &adapter_manifest("custom"));
    project.write("metadata/custom.json", &rust_manifest_json("disk_export"));
    let overlays = vec![project.overlay(
        "metadata/custom.json",
        &rust_manifest_json("overlay_export"),
    )];

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &overlays, &[]);

    assert_eq!(topology.adapter().rust_functions().len(), 1);
    assert_eq!(
        topology.adapter().rust_functions()[0].path().segments()[0].as_str(),
        "overlay_export"
    );
}

#[test]
fn declared_rust_failure_has_no_partial_application() {
    let project = TestProject::new("topology-rust-atomic-failure");
    project.write(
        "arcw.toml",
        &manifest(
            "dev",
            "src/main.arcw",
            "adapter = \"custom\"\nadapter_manifests = [\"adapters/custom.toml\"]\nrust_metadata = [\"metadata/first.json\", \"metadata/second.json\"]",
        ),
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    project.write("adapters/custom.toml", &adapter_manifest("custom"));
    project.write("metadata/first.json", &rust_manifest_json("first_export"));
    project.write("metadata/second.json", "{ malformed");

    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[], &[]);

    assert_eq!(error.code(), ProfileTopologyErrorCode::RustMetadata);
}

#[test]
fn workspace_path_outside_root_requires_dependency_seed() {
    let project = TestProject::new("topology-outside-without-seed");
    let dependency = TestProject::new("topology-outside-resource");
    dependency.write("custom.toml", &adapter_manifest("custom"));
    project.write(
        "arcw.toml",
        &manifest(
            "dev",
            "src/main.arcw",
            &format!(
                "adapter = \"custom\"\nadapter_manifests = [\"{}\"]",
                slash(&dependency.path("custom.toml"))
            ),
        ),
    );
    project.write("src/main.arcw", ROOT_SOURCE);

    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[], &[]);

    assert_eq!(error.code(), ProfileTopologyErrorCode::UnownedResourcePath);
}

#[test]
fn dependency_seed_satisfies_exact_outside_path() {
    let project = TestProject::new("topology-dependency-seed");
    let dependency = TestProject::new("topology-dependency-root");
    let resource_path = dependency.path("custom.toml");
    let owner = ProfileTopologyOwnerId::dependency("registry:custom@1").expect("dependency owner");
    let id = ProfileTopologyResourceId::new(
        owner,
        ProfileTopologyLogicalPath::try_new("custom.toml").expect("logical path"),
    );
    let source_id = SourceDocumentId::try_new("arcweft-dependency://custom@1/custom.toml")
        .expect("dependency source id");
    let seed = ProfileDependencyResourceSeed::try_new(
        id.clone(),
        ProfileTopologyResourceKind::AdapterManifest,
        dependency.root().to_path_buf(),
        resource_path.clone(),
        source_id.clone(),
    )
    .expect("dependency seed");
    project.write(
        "arcw.toml",
        &manifest(
            "dev",
            "src/main.arcw",
            &format!(
                "adapter = \"custom\"\nadapter_manifests = [\"{}\"]",
                slash(&resource_path)
            ),
        ),
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    let overlays = vec![
        ProfileTopologyOverlaySeed::try_new(resource_path, adapter_manifest("custom"))
            .expect("dependency overlay"),
    ];

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &overlays, &[seed]);
    let resource = topology.resource(&id).expect("dependency retained");

    assert_eq!(resource.ownership(), LoadedDocumentOwnership::Dependency);
    assert_eq!(resource.document().identity().id(), &source_id);
    assert_eq!(resource.origin(), ProfileTopologyResourceOrigin::Overlay);
}

#[test]
fn consumed_overlay_ids_are_sorted_and_complete() {
    let project = TestProject::new("topology-consumed-overlay-order");
    let manifest_text = manifest(
        "dev",
        "src/main.arcw",
        "character_manifests = [\"characters/zundamon.json\"]\nadapter = \"custom\"\nadapter_manifests = [\"adapters/custom.toml\"]",
    );
    let overlays = vec![
        project.overlay("arcw.toml", &manifest_text),
        project.overlay(
            "src/main.arcw",
            "use crate.feature\nfn main() -> Unit { () }\n",
        ),
        project.overlay("src/feature.arcw", "mod crate.feature\n"),
        project.overlay(
            "characters/zundamon.json",
            include_str!(
                "../../../arcweft-character/tests/fixtures/zundamon.awchar/character.awchar.json"
            ),
        ),
        project.overlay("adapters/custom.toml", &adapter_manifest("custom")),
    ];

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &overlays, &[]);
    let actual = topology
        .consumed_overlay_ids()
        .map(|id| id.path().as_str().to_owned())
        .collect::<Vec<_>>();
    let mut expected = actual.clone();
    expected.sort();

    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 5);
}

#[test]
fn loaded_topology_survives_disk_mutation() {
    let project = TestProject::new("topology-retains-bytes");
    let manifest_text = manifest("dev", "src/main.arcw", "");
    project.write("arcw.toml", &manifest_text);
    project.write("src/main.arcw", ROOT_SOURCE);

    let topology = project.load(LaunchProfileSelection::Explicit("dev"), &[], &[]);
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
fn duplicate_topology_logical_id_is_fatal() {
    let project = TestProject::new("topology-duplicate-logical-id");
    project.write(
        "arcw.toml",
        &manifest(
            "dev",
            "src/main.arcw",
            "character_manifests = [\"characters/zundamon.json\", \"characters/zundamon.json\"]",
        ),
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    project.write(
        "characters/zundamon.json",
        include_str!(
            "../../../arcweft-character/tests/fixtures/zundamon.awchar/character.awchar.json"
        ),
    );

    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[], &[]);

    assert_eq!(error.code(), ProfileTopologyErrorCode::DuplicateLogicalId);
}

#[test]
fn duplicate_normalized_path_with_distinct_ids_is_fatal() {
    let project = TestProject::new("topology-duplicate-path");
    let dependency = TestProject::new("topology-duplicate-path-dependency");
    let shared_path = dependency.path("shared.toml");
    dependency.write("shared.toml", &adapter_manifest("custom"));
    project.write(
        "arcw.toml",
        &manifest(
            "dev",
            "src/main.arcw",
            &format!(
                "adapter = \"custom\"\nadapter_manifests = [\"{0}\"]\nrust_metadata = [\"{0}\"]",
                slash(&shared_path)
            ),
        ),
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    let adapter_seed = dependency_seed(
        "registry:adapter@1",
        "shared.toml",
        ProfileTopologyResourceKind::AdapterManifest,
        dependency.root(),
        &shared_path,
        "arcweft-dependency://adapter@1/shared.toml",
    );
    let rust_seed = dependency_seed(
        "registry:rust@1",
        "shared.toml",
        ProfileTopologyResourceKind::RustMetadata,
        dependency.root(),
        &shared_path,
        "arcweft-dependency://rust@1/shared.toml",
    );

    let error = project.load_error(
        LaunchProfileSelection::Explicit("dev"),
        &[],
        &[adapter_seed, rust_seed],
    );

    assert_eq!(error.code(), ProfileTopologyErrorCode::DuplicatePath);
}

#[test]
fn dependency_seed_does_not_match_by_filename() {
    let project = TestProject::new("topology-dependency-no-fuzzy-match");
    let referenced = TestProject::new("topology-dependency-referenced");
    let seeded = TestProject::new("topology-dependency-seeded");
    let referenced_path = referenced.path("custom.toml");
    let seeded_path = seeded.path("custom.toml");
    seeded.write("custom.toml", &adapter_manifest("custom"));
    project.write(
        "arcw.toml",
        &manifest(
            "dev",
            "src/main.arcw",
            &format!(
                "adapter = \"custom\"\nadapter_manifests = [\"{}\"]",
                slash(&referenced_path)
            ),
        ),
    );
    project.write("src/main.arcw", ROOT_SOURCE);
    let seed = dependency_seed(
        "registry:custom@1",
        "custom.toml",
        ProfileTopologyResourceKind::AdapterManifest,
        seeded.root(),
        &seeded_path,
        "arcweft-dependency://custom@1/custom.toml",
    );

    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[], &[seed]);

    assert_eq!(error.code(), ProfileTopologyErrorCode::UnownedResourcePath);
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

    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &[], &[]);

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

    project.load(
        LaunchProfileSelection::Explicit("dev"),
        &exact_overlays,
        &[],
    );

    let one_over = format!("{exact}x");
    let one_overlays = vec![project.overlay("arcw.toml", &one_over)];
    let error = project.load_error(LaunchProfileSelection::Explicit("dev"), &one_overlays, &[]);
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

    let topology = exact_project.load(
        LaunchProfileSelection::Explicit("dev"),
        &exact_overlays,
        &[],
    );
    assert_eq!(
        u64::try_from(topology.resources().len()).expect("resource count fits u64"),
        ProfileTopologyLimits::PRODUCTION.resources()
    );

    let one_over_project = TestProject::new("topology-resource-limit-one-over");
    one_over_project.write("arcw.toml", &manifest("dev", "src/main.arcw", ""));
    let one_overlays = module_overlays(&one_over_project, exact_module_count + 1);
    let error =
        one_over_project.load_error(LaunchProfileSelection::Explicit("dev"), &one_overlays, &[]);
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
        r#"[package]
name = "topology-tests"
version = "0.1.0"

[profiles."{profile}"]
kind = "game"
entry = "entry.game.main"
source = "{source}"
{extra}
"#
    )
}

fn adapter_manifest(id: &str) -> String {
    format!(
        r#"schema_version = 1
id = "{id}"
display_name = "{id}"
functions = []
host_calls = []
"#
    )
}

fn adapter_manifest_with_effects(id: &str, effects: &[&str]) -> String {
    let effects = effects
        .iter()
        .map(|effect| format!("\"{effect}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"schema_version = 1
id = "{id}"
display_name = "{id}"
effects = [{effects}]
functions = []
host_calls = []
"#
    )
}

fn rust_manifest_json(function: &str) -> String {
    ArcweftRustManifest::new(ArcweftRustPackage {
        name: "topology_adapter".to_owned(),
        version: "0.1.0".to_owned(),
        metadata_hash: None,
    })
    .with_function(ArcweftRustFunction {
        name: function.to_owned(),
        rust_path: format!("topology_adapter::{}", function.replace('.', "_")),
        params: Vec::new(),
        return_type: ArcweftRustTypeRef::Unit,
        purity: ArcweftRustPurity::Pure,
        effects: Vec::new(),
    })
    .to_json_pretty()
    .expect("Rust metadata encodes")
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

fn dependency_seed(
    package: &str,
    logical_path: &str,
    kind: ProfileTopologyResourceKind,
    root: &std::path::Path,
    path: &std::path::Path,
    source_id: &str,
) -> ProfileDependencyResourceSeed {
    ProfileDependencyResourceSeed::try_new(
        ProfileTopologyResourceId::new(
            ProfileTopologyOwnerId::dependency(package).expect("dependency owner"),
            ProfileTopologyLogicalPath::try_new(logical_path).expect("logical path"),
        ),
        kind,
        root.to_path_buf(),
        path.to_path_buf(),
        SourceDocumentId::try_new(source_id).expect("source id"),
    )
    .expect("dependency seed")
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

    fn root(&self) -> &std::path::Path {
        &self.root
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture directory");
        }
        fs::write(path, contents).expect("fixture file");
    }

    fn overlay(&self, relative: &str, contents: &str) -> ProfileTopologyOverlaySeed {
        ProfileTopologyOverlaySeed::try_new(self.path(relative), contents.to_owned())
            .expect("normalized overlay")
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
        dependencies: &[ProfileDependencyResourceSeed],
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
            .with_dependency_resources(dependencies),
        )
        .expect("topology loads")
    }

    fn load_error(
        &self,
        selection: LaunchProfileSelection<'_>,
        overlays: &[ProfileTopologyOverlaySeed],
        dependencies: &[ProfileDependencyResourceSeed],
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
            .with_dependency_resources(dependencies),
        )
        .expect_err("topology fails")
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
