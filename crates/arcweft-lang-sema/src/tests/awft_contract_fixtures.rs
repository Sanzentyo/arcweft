use std::fs;
use std::path::{Path, PathBuf};

use super::support::*;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/awft")
}

fn awft_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
        .map(|entry| entry.expect("fixture dir entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "awft"))
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn file_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .expect("fixture file name is utf-8")
}

fn is_current_check_gap(path: &Path) -> bool {
    matches!(
        file_name(path),
        // Direction-package fixtures kept as future syntax/semantic targets.
        // Remove entries here as each gap becomes implemented.
        "008_let_else_diverge.awft"
            | "011_dialogue_with_plan.awft"
            | "013_task_fn_await_shape.awft"
            | "014_struct_enum_type_alias.awft"
            | "015_state_defaults.awft"
            | "016_source_decl.awft"
            | "017_stream_fn_shape.awft"
    )
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
    for path in awft_files(&fixture_root().join("current_pass/check"))
        .into_iter()
        .filter(|path| !is_current_check_gap(path))
    {
        assert_check_pipeline(&path);
    }
}

#[test]
#[ignore = "enable after documented spec gaps are fixed"]
fn spec_should_pass_check_fixtures_pass_parser_hir_sema_after_refactor() {
    for path in awft_files(&fixture_root().join("spec_should_pass/check")) {
        assert_check_pipeline(&path);
    }
}
