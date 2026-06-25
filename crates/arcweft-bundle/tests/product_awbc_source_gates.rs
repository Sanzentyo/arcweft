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
    assert!(!product.contains("legacy_structured_product_fallback"));
    assert!(!product.contains("decode_structured_product_fallback"));
}
