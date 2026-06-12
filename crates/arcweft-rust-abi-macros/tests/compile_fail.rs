#[test]
fn rejects_unsupported_abi_shapes() {
    for case in compile_fail_cases() {
        assert_compile_fails(case);
    }
}

fn compile_fail_cases() -> &'static [&'static str] {
    &[
        "reject_generic_type",
        "reject_reference_export",
        "reject_reference_field",
        "reject_reference_return",
        "reject_self_receiver_export",
    ]
}

fn assert_compile_fails(case: &str) {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .expect("macro crate lives under workspace crates directory");
    let project_dir = workspace_root
        .join("target")
        .join("tests")
        .join("arcweft-rust-abi-macros-ui");
    let ui_dir = project_dir.join("tests").join("ui");
    std::fs::create_dir_all(&ui_dir).expect("create compile-fail ui test directory");

    let source_name = format!("{case}.rs");
    let stderr_name = format!("{case}.stderr");
    std::fs::copy(
        manifest_dir.join("tests").join("ui").join(&source_name),
        ui_dir.join(&source_name),
    )
    .unwrap_or_else(|err| panic!("copy {source_name}: {err}"));
    write_compile_fail_manifest(&project_dir, workspace_root, case);

    let output = std::process::Command::new("cargo")
        .arg("check")
        .arg("--quiet")
        .arg("--color")
        .arg("never")
        .arg("--bin")
        .arg(case)
        .current_dir(&project_dir)
        .output()
        .unwrap_or_else(|err| panic!("run cargo check for {case}: {err}"));

    assert!(
        !output.status.success(),
        "{case} should fail to compile, stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let expected = std::fs::read_to_string(manifest_dir.join("tests").join("ui").join(stderr_name))
        .unwrap_or_else(|err| panic!("read expected stderr for {case}: {err}"));
    let actual = normalize_compile_stderr(&String::from_utf8_lossy(&output.stderr));
    let expected = normalize_compile_stderr(&expected);
    assert!(
        actual.contains(&expected),
        "{case} stderr did not contain expected diagnostic\n\nexpected:\n{expected}\n\nactual:\n{actual}"
    );
}

fn write_compile_fail_manifest(
    project_dir: &std::path::Path,
    workspace_root: &std::path::Path,
    case: &str,
) {
    let macros_path = workspace_root
        .join("crates")
        .join("arcweft-rust-abi-macros");
    let abi_path = workspace_root.join("crates").join("arcweft-rust-abi");
    let manifest = format!(
        r#"[package]
name = "arcweft-rust-abi-macros-ui"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
arcweft-rust-abi = {{ path = "{}" }}
arcweft-rust-abi-macros = {{ path = "{}" }}

[[bin]]
name = "{}"
path = "tests/ui/{}.rs"
"#,
        toml_path(&abi_path),
        toml_path(&macros_path),
        case,
        case
    );
    std::fs::write(project_dir.join("Cargo.toml"), manifest).expect("write compile-fail manifest");
}

fn toml_path(path: &std::path::Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn normalize_compile_stderr(stderr: &str) -> String {
    stderr
        .replace('\\', "/")
        .lines()
        .filter(|line| !line.starts_with("error: could not compile "))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}
