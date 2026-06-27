#[test]
fn migrated_product_catalog_families_do_not_use_product_json_fallback() {
    let product = std::fs::read_to_string("src/product.rs").expect("product.rs is readable");

    for removed in [
        "struct ContentCatalogSection",
        "required_payload::<ContentCatalogSection>",
        "required_payload::<",
        "optional_payload::<",
        "BundleSectionKind::NormalizedSource",
        "encode_json(&bundle.display",
        "encode_json(&bundle.source",
        "audio: bundle.audio",
        "audio: content.audio",
    ] {
        assert!(
            !product.contains(removed),
            "product AWFB path must not retain migrated catalog JSON fallback pattern `{removed}`"
        );
    }

    for required in [
        "CompactContentCatalogSection::from_bundle",
        "CompactDisplayCatalogSection::from_bundle",
        "CompactSourceMapSection::from_bundle",
        "CompactAudioGraphSection::from_graph",
        "BundleSectionKind::SourceMap",
        "BundleSectionKind::AudioGraph",
    ] {
        assert!(
            product.contains(required),
            "product AWFB path must retain compact catalog wiring `{required}`"
        );
    }
}
