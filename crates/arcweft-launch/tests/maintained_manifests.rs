use arcweft_launch::{LaunchProfileSelection, accepted::SourceBackedManifest};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use std::{fs, path::Path, sync::Arc};

const MAINTAINED_MANIFESTS: [&str; 13] = [
    "fixtures/diagnostics/project-wide/arcw.toml",
    "fixtures/persistent-cache-build/seq04-8-4/normal-conservative-multi/arcw.toml",
    "fixtures/persistent-cache-build/seq04-8-4/normal-single/arcw.toml",
    "samples/focus-navigation-controller-dsl/arcw.toml",
    "samples/function-curried-call-groups/arcw.toml",
    "samples/modern-feedback-view/arcw.toml",
    "samples/native-text-input/arcw.toml",
    "samples/reactive-view-style/arcw.toml",
    "samples/text-submit-flow/arcw.toml",
    "samples/visual-novel-mini/arcw.toml",
    "samples/zundamon-awchar/arcw.toml",
    "samples/zundamon-stand-switch/arcw.toml",
    "web/arcw.toml",
];

fn decode_manifest(repository: &Path, relative: &str) -> SourceBackedManifest {
    let source = fs::read_to_string(repository.join(relative))
        .unwrap_or_else(|error| panic!("failed to read maintained manifest {relative}: {error}"));
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!("arcweft-fixture://{relative}"))
                .expect("fixture document ID"),
            SourceName::path(relative),
            source,
        )
        .unwrap_or_else(|error| {
            panic!("failed to construct source document for {relative}: {error}")
        }),
    );

    SourceBackedManifest::decode(document)
        .unwrap_or_else(|report| panic!("{relative} must decode as schema 1: {report:?}"))
}

#[test]
fn maintained_manifests_decode_through_the_canonical_public_surface() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("launch crate is inside the workspace");

    for relative in MAINTAINED_MANIFESTS {
        let accepted = decode_manifest(repository, relative);

        assert_eq!(
            accepted.manifest().schema().get(),
            1,
            "{relative} must use canonical schema 1"
        );
    }
}

#[test]
fn root_level_samples_use_canonical_source_roots_and_existing_profile_sources() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("launch crate is inside the workspace");

    for (manifest_path, expected_source) in [
        ("samples/zundamon-stand-switch/arcw.toml", "src/main.arcw"),
        ("web/arcw.toml", "src/main.arcw"),
    ] {
        let accepted = decode_manifest(repository, manifest_path);
        assert_eq!(
            accepted.manifest().build().source_dir.as_str(),
            "src",
            "{manifest_path} must use the canonical source directory"
        );
        let profile = accepted
            .resolve_profile(LaunchProfileSelection::Automatic { previous: None })
            .unwrap_or_else(|report| panic!("{manifest_path} must select a profile: {report:?}"));
        assert_eq!(
            profile.source().as_str(),
            expected_source,
            "{manifest_path} must select its moved source"
        );

        let project_root = repository.join(
            Path::new(manifest_path)
                .parent()
                .expect("manifest has a project directory"),
        );
        assert!(
            project_root.join(profile.source().as_path()).is_file(),
            "{manifest_path} profile source must exist"
        );
    }
}
