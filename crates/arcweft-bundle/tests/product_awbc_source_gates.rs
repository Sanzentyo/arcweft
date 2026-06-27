use std::{fs, path::Path};

#[test]
fn product_awbc_paths_do_not_depend_on_structured_bytecode_or_compact_sidecars() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let checked = [
        "crates/arcweft-runtime-driver/src/session.rs",
        "crates/arcweft-runtime-host/src/bundle_runner.rs",
        "crates/arcweft-player-native/src/lib.rs",
    ];
    let forbidden = [
        "bundle.bytecode.program.clone()",
        "from_bytecode_parts(",
        "BytecodeVmExecutor::new",
    ];
    for file in checked {
        let content = fs::read_to_string(root.join(file)).expect("source file reads");
        for term in forbidden {
            assert!(
                !content.contains(term),
                "{file} still contains forbidden product structured-bytecode term `{term}`"
            );
        }
    }
}

#[test]
fn product_awbc_codec_rejects_old_structured_product_payloads_explicitly() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let product = fs::read_to_string(root.join("crates/arcweft-bundle/src/product.rs"))
        .expect("product source reads");
    assert!(product.contains("StructuredProductBytecodeUnsupported"));
    assert!(product.contains("reject_structured_or_decode_awbc"));
    assert!(!product.contains("decode_structured_product_fallback"));
    assert!(!product.contains("structured_product_fallback"));
}
#[test]
fn project_and_run_awfb_writers_share_product_bundle_builder() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bundle = fs::read_to_string(root.join("crates/arcweft-cli/src/app/bundle.rs"))
        .expect("bundle source reads");
    assert!(bundle.contains(".with_product_awbc(compiled.product_awbc)"));

    for file in [
        "crates/arcweft-cli/src/app/project_commands.rs",
        "crates/arcweft-cli/src/app/runtime/run.rs",
    ] {
        let content = fs::read_to_string(root.join(file)).expect("AWFB writer source reads");
        assert!(content.contains("compile_bundle_for_selection"));
        assert!(content.contains("BundleFormat::Awfb"));
    }
}

#[test]
fn compact_bytecode_residue_is_deleted_after_import_gate() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let core =
        fs::read_to_string(root.join("crates/arcweft-core/src/lib.rs")).expect("core source reads");
    assert!(!core.contains("pub mod compact_bytecode;"));
    assert!(
        !root
            .join("crates/arcweft-core/src/compact_bytecode.rs")
            .exists()
    );
}
