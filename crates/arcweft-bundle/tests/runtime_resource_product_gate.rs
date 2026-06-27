use std::fs;
use std::path::Path;

#[test]
fn migrated_runtime_sections_do_not_use_product_json_fallback() {
    let product = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/product.rs"))
        .expect("product.rs is readable");

    for forbidden in [
        "struct RuntimeTypesSection",
        "struct EntrypointsSection",
        "struct AdapterRequirementsSection",
        "encode_json(&RuntimeTypesSection",
        "encode_json(&EntrypointsSection",
        "encode_json(&AdapterRequirementsSection",
        "required_payload::<RuntimeTypesSection>",
        "required_payload::<EntrypointsSection>",
        "required_payload::<AdapterRequirementsSection>",
    ] {
        assert!(
            !product.contains(forbidden),
            "migrated runtime product sections must use compact resource codecs, but product.rs still contains `{forbidden}`"
        );
    }
}
