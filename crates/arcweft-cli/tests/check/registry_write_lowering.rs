#[test]
fn check_registry_write_preserves_lazy_rhs_branches() {
    let path = temp_arcw(
        "registry-write-branch-rhs",
        r"
flow registry effects { state.write('flow) } {
    'flow.flags.seen <- if true { true } else { false }
}
",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg(path)
        .output()
        .expect("check branch-valued registry write");
    assert!(
        output.status.success(),
        "stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn check_registry_write_accepts_an_ordinary_call_rhs() {
    let path = temp_arcw(
        "registry-write-call-rhs",
        r"
fn seen() -> bool { return true }
flow registry effects { state.write('flow) } {
    'flow.flags.seen <- seen()
}
",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg(path)
        .output()
        .expect("check call-valued registry write");
    assert!(
        output.status.success(),
        "stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
