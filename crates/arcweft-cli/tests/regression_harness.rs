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
fn rust_unsafe_sites_stay_inside_audited_native_boundaries() {
    let root = workspace_root();
    let violations = text_files(&root.join("crates"))
        .into_iter()
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("rs"))
        .filter(|path| {
            path.file_name()
                .is_none_or(|name| name != "regression_harness.rs")
        })
        .flat_map(|path| rust_unsafe_violations_in_file(&path))
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "Rust unsafe must stay isolated in audited native boundaries: {violations:?}"
    );
}

fn rust_unsafe_violations_in_file(path: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let lines = text.lines().collect::<Vec<_>>();
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| contains_rust_unsafe_site(line))
        .filter(|(index, _)| {
            !is_audited_unsafe_boundary(path) || !has_safety_comment_near(&lines, *index)
        })
        .map(|(index, line)| {
            format!(
                "{}:{} contains `{}`",
                relative_to_workspace(path).display(),
                index + 1,
                line.trim()
            )
        })
        .collect()
}

fn contains_rust_unsafe_site(line: &str) -> bool {
    line.contains("unsafe {")
        || line.contains("unsafe{")
        || line.contains("unsafe fn")
        || line.contains("unsafe impl")
        || line.contains("unsafe trait")
}

fn has_safety_comment_near(lines: &[&str], index: usize) -> bool {
    let search_start = index.saturating_sub(6);
    let search_end = (index + 4).min(lines.len());
    (search_start..search_end).any(|comment_index| lines[comment_index].contains("SAFETY:"))
}

fn is_audited_unsafe_boundary(path: &Path) -> bool {
    let parts = relative_to_workspace(path)
        .components()
        .filter_map(|component| component.as_os_str().to_str().map(str::to_owned))
        .collect::<Vec<_>>();
    matches!(
        parts.as_slice(),
        [
            crates,
            jit_crate,
            src,
            native_call
        ] if crates == "crates"
            && jit_crate == "arcweft-lang-jit-cranelift"
            && src == "src"
            && native_call == "native_call.rs"
    ) || matches!(
        parts.as_slice(),
        [
            crates,
            desktop_crate,
            src,
            text_input,
            windows_tsf,
            unsafe_com
        ] if crates == "crates"
            && desktop_crate == "arcweft-desktop-native"
            && src == "src"
            && text_input == "text_input"
            && windows_tsf == "windows_tsf"
            && unsafe_com == "unsafe_com.rs"
    )
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

fn relative_to_workspace(path: &Path) -> PathBuf {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .to_path_buf()
}
