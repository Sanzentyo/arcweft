use std::fs;
use std::path::{Path, PathBuf};

use super::support::*;

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
    let parsed = parse_source(source);
    assert!(
        parsed.errors().is_empty(),
        "{} parse errors: {:?}",
        path.display(),
        parsed.errors(),
    );
    let tree = parsed.into_typed_tree();
    let hir = lower_to_hir(&tree)
        .unwrap_or_else(|errors| panic!("{} HIR errors: {errors:?}", path.display()));
    let registry = registry_from_hir(&hir);
    validate_hir_references(&hir, &registry)
        .unwrap_or_else(|errors| panic!("{} reference errors: {errors:?}", path.display()));
    validate_typecheck_ready(&hir)
        .unwrap_or_else(|errors| panic!("{} readiness errors: {errors:?}", path.display()));
    typecheck_hir(&hir, &TypeCheckEnv::new())
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
