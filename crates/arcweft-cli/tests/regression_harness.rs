use std::path::{Path, PathBuf};

#[test]
fn fixture_tree_does_not_contain_runtime_generated_arcweft_dirs() {
    let fixture_root = workspace_root().join("tests/fixtures/arcw");
    let leaked = find_dirs_named(&fixture_root, ".arcweft");
    assert!(
        leaked.is_empty(),
        "runtime-generated .arcweft directories must not be left under fixtures: {leaked:?}"
    );
}

#[test]
fn source_tree_does_not_reintroduce_removed_whitespace_command_dsl_or_shims() {
    let root = workspace_root();
    let search_roots = [root.join("crates"), root.join("docs"), root.join("tests")];
    let denied = [
        "legacy_",
        "deprecated",
        "compat shim",
        "compatibility shim",
        "ref bg(",
        "ref show(",
        "clear bg(",
        "clear show(",
        "memo rich_text",
        "wait mark ",
        "cancel on input ",
        "at(phoneme \"",
        "at(char ",
        "at(word ",
    ];
    let violations = search_roots
        .iter()
        .flat_map(|root| text_files(root))
        .filter(|path| !is_historical_review(path))
        .filter(|path| {
            path.file_name()
                .is_none_or(|name| name != "regression_harness.rs")
        })
        .flat_map(|path| denied_patterns_in_file(&path, &denied))
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "removed DSL or compatibility-shim text reappeared outside docs/reviews: {violations:?}"
    );
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root is two levels above arcweft-cli")
        .to_path_buf()
}

fn find_dirs_named(root: &Path, name: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    visit_dirs(root, &mut |path| {
        if path.file_name().is_some_and(|file_name| file_name == name) {
            found.push(relative_to_workspace(path));
        }
    });
    found
}

fn text_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    visit_files(root, &mut |path| {
        if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("rs" | "md" | "arcw" | "toml" | "json")
        ) {
            files.push(path.to_path_buf());
        }
    });
    files
}

fn denied_patterns_in_file(path: &Path, denied: &[&str]) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    denied
        .iter()
        .filter(|pattern| text.contains(**pattern))
        .map(|pattern| {
            format!(
                "{} contains `{pattern}`",
                relative_to_workspace(path).display()
            )
        })
        .collect()
}

fn visit_dirs(root: &Path, visit: &mut impl FnMut(&Path)) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit(&path);
            visit_dirs(&path, visit);
        }
    }
}

fn visit_files(root: &Path, visit: &mut impl FnMut(&Path)) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if !is_ignored_dir(&path) {
                visit_files(&path, visit);
            }
        } else {
            visit(&path);
        }
    }
}

fn is_ignored_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, "target" | ".git" | ".jj"))
}

fn is_historical_review(path: &Path) -> bool {
    let mut previous = None;
    for component in path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
    {
        if previous == Some("docs") && component == "reviews" {
            return true;
        }
        previous = Some(component);
    }
    false
}

fn relative_to_workspace(path: &Path) -> PathBuf {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .to_path_buf()
}
