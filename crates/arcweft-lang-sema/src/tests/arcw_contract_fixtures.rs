use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::support::*;
use arcweft_lang_syntax::parser::{ParseOptions, parse_document_with_source};

/// The check-fixture corpus is authored against these published domain types.
///
/// Project declarations are registered from each fixture source; this inventory
/// covers only domain types supplied by the host contracts exercised here.
fn contract_fixture_environment() -> TypeCheckEnv {
    let mut environment = [
        // The view and task fixtures use these published runtime-domain
        // values without declaring project-owned counterparts.
        "ChoiceView",
        "Flow",
        "OpeningAssets",
        // Capability declarations introduce these adapter-owned values.
        "FsError",
        "HttpRequest",
        "HttpResponse",
        "RenderContext",
        "RenderNode",
        // Independent fixture modules use these project-model names in
        // contracts without repeating their definitions.
        "RouteInfo",
        "GameState",
        "GameEvent",
        "IteratorItem",
        "CaptureError",
        "AudioConfig",
        "VisualConfig",
        "Summary",
    ]
    .into_iter()
    .fold(TypeCheckEnv::standard(), accept_fixture_domain_type);
    environment = accept_fixture_opaque_domain_type(environment, "ChoiceOption", 0);
    environment.nominal_records.insert(
        "ChoiceView".to_owned(),
        [("label".to_owned(), TypeKind::String)].into(),
    );
    environment.nominal_records.insert(
        "OpeningAssets".to_owned(),
        [("bg".to_owned(), TypeKind::Named("ImageHandle".to_owned()))].into(),
    );
    environment.nominal_records.insert(
        "RouteInfo".to_owned(),
        [("label".to_owned(), TypeKind::String)].into(),
    );
    environment.nominal_records.insert(
        "Summary".to_owned(),
        [("route".to_owned(), TypeKind::entity_ref(EntityKind::Flow))].into(),
    );
    environment
}

fn accept_fixture_domain_type(environment: TypeCheckEnv, name: &str) -> TypeCheckEnv {
    let record = crate::env::nominal::AcceptedNominalRecord::try_new(
        fixture_nominal_id(name),
        0,
        crate::env::nominal::AcceptedNominalSemantics::Exact(TypeKind::Named(name.to_owned())),
        crate::env::nominal::AcceptedNominalOrigin::Test,
        None,
    )
    .expect("fixture domain type is an accepted nominal record");
    environment
        .try_with_nominal_record(record)
        .expect("fixture domain type has a distinct path")
}

fn accept_fixture_opaque_domain_type(
    environment: TypeCheckEnv,
    name: &str,
    arity: u16,
) -> TypeCheckEnv {
    let record = crate::env::nominal::AcceptedNominalRecord::try_new(
        fixture_nominal_id(name),
        arity,
        crate::env::nominal::AcceptedNominalSemantics::Opaque,
        crate::env::nominal::AcceptedNominalOrigin::Test,
        None,
    )
    .expect("fixture domain type is an accepted opaque nominal record");
    environment
        .try_with_nominal_record(record)
        .expect("fixture domain type has a distinct path")
}

fn fixture_nominal_id(name: &str) -> crate::env::nominal::AcceptedNominalId {
    let authored = parse_type_ref(name).expect("fixture domain type path parses");
    let TypeRef::Path(path) = authored.value() else {
        panic!("fixture domain type is a direct type path");
    };
    crate::env::nominal::AcceptedNominalId::new(
        crate::env::nominal::AcceptedNominalOwnerId::Standard,
        path.clone(),
    )
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/arcw")
}

fn arcw_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
        .map(|entry| entry.expect("fixture dir entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "arcw"))
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn assert_check_pipeline(path: &Path) {
    let source = fs::read_to_string(path).expect("fixture source");
    let label = path
        .file_stem()
        .and_then(std::ffi::OsStr::to_str)
        .expect("fixture filename is UTF-8");
    let document = Arc::new(
        arcweft_source::SourceDocument::try_new(
            arcweft_source::SourceDocumentId::try_new(format!(
                "memory:///contract-fixtures/{label}.arcw"
            ))
            .expect("valid fixture document ID"),
            arcweft_source::SourceName::Generated,
            source.as_str(),
        )
        .expect("valid fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    assert!(
        parsed.errors().is_empty(),
        "{} parse errors: {:?}",
        path.display(),
        parsed.errors(),
    );
    let hir = lower_document_to_hir(parsed.document(), parsed.typed_tree())
        .unwrap_or_else(|errors| panic!("{} HIR errors: {errors:?}", path.display()));
    let registry = registry_from_hir(&hir);
    validate_hir_references(&hir, &registry)
        .unwrap_or_else(|errors| panic!("{} reference errors: {errors:?}", path.display()));
    validate_typecheck_ready(&hir)
        .unwrap_or_else(|errors| panic!("{} readiness errors: {errors:?}", path.display()));
    typecheck_registered_source(
        "arcw-contract-fixture",
        &source,
        contract_fixture_environment(),
    )
    .unwrap_or_else(|errors| panic!("{} typecheck errors: {errors:?}", path.display()));
}

#[test]
fn current_check_fixtures_pass_parser_hir_sema() {
    for path in arcw_files(&fixture_root().join("current_pass/check")) {
        assert_check_pipeline(&path);
    }
}

#[test]
fn spec_should_pass_check_fixtures_pass_parser_hir_sema_after_refactor() {
    for path in arcw_files(&fixture_root().join("spec_should_pass/check")) {
        assert_check_pipeline(&path);
    }
}
