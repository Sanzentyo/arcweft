use std::fs;

#[test]
fn patch_session_path_consumes_declared_patch_compatibility_without_reclassifying() {
    let source = fs::read_to_string("src/session.rs").expect("session source reads");
    let start = source
        .find("pub fn hot_swap_patch_bytes")
        .expect("patch entrypoint exists");
    let tail = &source[start..];
    let end = tail
        .find("pub fn inspect_hot_swap_patch_artifact")
        .unwrap_or(tail.len());
    let patch_path = &tail[..end];

    assert!(patch_path.contains("readiness.compatibility"));
    assert!(patch_path.contains("hot_swap_bundle_with_declared_compatibility"));
    assert!(!patch_path.contains("classify_swap"));
}
