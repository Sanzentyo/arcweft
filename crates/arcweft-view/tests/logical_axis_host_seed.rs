use arcweft_view::{
    ViewBoxAxisHostSeed, ViewBoxAxisMode, ViewBoxAxisRevision, ViewBoxAxisSeedGeneration,
    ViewBoxAxisSeedGenerationError, ViewBoxAxisSeedSource, ViewInheritedBoxAxes, ViewMountId,
};

#[test]
fn host_seed_revisions_match_the_canonical_transcript() {
    let cases = [
        (
            ViewMountId::from_raw(1),
            ViewBoxAxisSeedGeneration::INITIAL,
            ViewBoxAxisHostSeed::Default,
            0x3abe_94ed_ecfd_401f,
        ),
        (
            ViewMountId::from_raw(1),
            ViewBoxAxisSeedGeneration::INITIAL,
            ViewBoxAxisHostSeed::Explicit(ViewBoxAxisMode::HorizontalLtr),
            0x3abb_2eed_ecfa_5cf6,
        ),
        (
            ViewMountId::from_raw(7),
            serde_json::from_str("3").expect("generation decodes"),
            ViewBoxAxisHostSeed::Explicit(ViewBoxAxisMode::VerticalRl),
            0x736d_712b_bbf5_2881,
        ),
        (
            ViewMountId::from_raw(7),
            serde_json::from_str("3").expect("generation decodes"),
            ViewBoxAxisHostSeed::Explicit(ViewBoxAxisMode::VerticalLr),
            0x736d_702b_bbf5_26ce,
        ),
    ];

    for (mount, generation, seed, expected) in cases {
        assert_eq!(
            ViewBoxAxisRevision::for_host_seed(mount, generation, seed).value(),
            expected
        );
        assert_eq!(
            ViewInheritedBoxAxes::for_host_seed(mount, generation, seed)
                .revision()
                .value(),
            expected
        );
    }
}

#[test]
fn host_seed_distinguishes_default_from_explicit_horizontal_ltr() {
    let mount = ViewMountId::from_raw(9);
    let default = ViewInheritedBoxAxes::for_host_seed(
        mount,
        ViewBoxAxisSeedGeneration::INITIAL,
        ViewBoxAxisHostSeed::Default,
    );
    let explicit = ViewInheritedBoxAxes::for_host_seed(
        mount,
        ViewBoxAxisSeedGeneration::INITIAL,
        ViewBoxAxisHostSeed::Explicit(ViewBoxAxisMode::HorizontalLtr),
    );

    assert_eq!(default.mode(), explicit.mode());
    assert_eq!(default.source(), ViewBoxAxisSeedSource::HostDefault);
    assert_eq!(explicit.source(), ViewBoxAxisSeedSource::HostExplicit);
    assert_ne!(default.revision(), explicit.revision());
}

#[test]
fn host_seed_wire_is_strict_and_round_trips() {
    let seeds = [
        ViewBoxAxisHostSeed::Default,
        ViewBoxAxisHostSeed::Explicit(ViewBoxAxisMode::HorizontalLtr),
        ViewBoxAxisHostSeed::Explicit(ViewBoxAxisMode::HorizontalRtl),
        ViewBoxAxisHostSeed::Explicit(ViewBoxAxisMode::VerticalRl),
        ViewBoxAxisHostSeed::Explicit(ViewBoxAxisMode::VerticalLr),
    ];
    for seed in seeds {
        let encoded = serde_json::to_string(&seed).expect("seed encodes");
        assert_eq!(
            serde_json::from_str::<ViewBoxAxisHostSeed>(&encoded).expect("seed decodes"),
            seed
        );
    }
    assert_eq!(
        serde_json::to_string(&ViewBoxAxisHostSeed::Default).unwrap(),
        r#"{"kind":"default"}"#
    );
    assert!(
        serde_json::from_str::<ViewBoxAxisHostSeed>(r#"{"kind":"default","unexpected":true}"#)
            .is_err()
    );
    assert!(serde_json::from_str::<ViewBoxAxisHostSeed>(r#"{"kind":"explicit"}"#).is_err());
}

#[test]
fn seed_generation_never_wraps() {
    let generation = ViewBoxAxisSeedGeneration::INITIAL;
    assert_eq!(generation.checked_next().unwrap().value(), 1);

    let exhausted: ViewBoxAxisSeedGeneration =
        serde_json::from_str(&u64::MAX.to_string()).expect("maximum generation decodes");
    assert_eq!(
        exhausted.checked_next(),
        Err(ViewBoxAxisSeedGenerationError::Exhausted)
    );
}
