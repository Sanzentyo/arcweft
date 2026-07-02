use arcweft_project::persistent_object::{
    AUDITED_BYTECODE_LINK_PRODUCER_FAMILIES, BytecodeLinkProducerFamily,
};

#[test]
fn producer_inventory_contains_every_required_seq04_8_3_family() {
    let required = [
        BytecodeLinkProducerFamily::FullBuild,
        BytecodeLinkProducerFamily::FullBuildWatch,
        BytecodeLinkProducerFamily::DirectBundle,
        BytecodeLinkProducerFamily::SingleSourceCompile,
        BytecodeLinkProducerFamily::PatchBundle,
        BytecodeLinkProducerFamily::AgentScript,
        BytecodeLinkProducerFamily::RuntimeDriver,
        BytecodeLinkProducerFamily::FixtureRegeneration,
        BytecodeLinkProducerFamily::PersistentCacheTestBuilder,
    ];

    for family in required {
        assert!(
            AUDITED_BYTECODE_LINK_PRODUCER_FAMILIES.contains(&family),
            "missing bytecode/link producer family inventory entry for {family:?}"
        );
    }
}

#[test]
fn producer_family_wire_names_are_stable() {
    assert_eq!(BytecodeLinkProducerFamily::FullBuild.as_str(), "full_build");
    assert_eq!(
        BytecodeLinkProducerFamily::FullBuildWatch.as_str(),
        "full_build_watch"
    );
    assert_eq!(
        BytecodeLinkProducerFamily::DirectBundle.as_str(),
        "direct_bundle"
    );
    assert_eq!(
        BytecodeLinkProducerFamily::PersistentCacheTestBuilder.as_str(),
        "persistent_cache_test_builder"
    );
}
