use arcweft_codec_binary::ArcweftBinaryCodec;
use arcweft_codec_cbor::CborCodec;
use arcweft_codec_json::JsonCodec;
use arcweft_codec_msgpack::MessagePackCodec;
use arcweft_core::effect::{
    RuntimeArtifactFingerprint, RuntimeAssertion, RuntimeAssertionFailure, RuntimeAssertionGuardId,
    RuntimeAssertionProfile, RuntimeIdentityDecodeError,
};
use arcweft_data::{Codec, DecodeOptions, EncodeOptions, FieldShape, TypeShape};
use arcweft_serde_bridge::{from_arcweft_value, to_arcweft_value};

fn artifact_fingerprint_fixture() -> RuntimeArtifactFingerprint {
    RuntimeArtifactFingerprint::try_from_bytes([9; 32]).expect("non-zero artifact fingerprint")
}

fn failure_fixture() -> RuntimeAssertionFailure {
    RuntimeAssertionFailure::new(RuntimeAssertion::new(
        RuntimeAssertionGuardId::try_from_bytes([7; 16]).expect("non-zero assertion guard"),
        "ready".to_owned(),
        "must be ready".to_owned(),
        RuntimeAssertionProfile::Always,
    ))
}

fn failure_shape() -> TypeShape {
    TypeShape::record(
        "RuntimeAssertionFailure",
        [FieldShape::new(
            "assertion",
            "assertion",
            TypeShape::record(
                "RuntimeAssertion",
                [
                    FieldShape::new("guard", "guard", TypeShape::seq(TypeShape::I128)),
                    FieldShape::new("condition", "condition", TypeShape::String),
                    FieldShape::new("message", "message", TypeShape::String),
                    FieldShape::new("profile", "profile", TypeShape::String),
                ],
            ),
        )],
    )
}

fn assert_codec_round_trip(codec: &dyn Codec) {
    let expected_fingerprint = artifact_fingerprint_fixture();
    let fingerprint_shape = TypeShape::seq(TypeShape::I128);
    let fingerprint_value = to_arcweft_value(&expected_fingerprint)
        .expect("artifact fingerprint enters the typed data boundary");
    let fingerprint_bytes = codec
        .encode_value(
            &fingerprint_value,
            &fingerprint_shape,
            &EncodeOptions::default(),
        )
        .expect("artifact fingerprint encodes");
    let decoded_fingerprint_value = codec
        .decode_value(
            &fingerprint_bytes,
            &fingerprint_shape,
            &DecodeOptions::default(),
        )
        .expect("artifact fingerprint decodes");
    let actual_fingerprint: RuntimeArtifactFingerprint =
        from_arcweft_value(&decoded_fingerprint_value)
            .expect("artifact fingerprint leaves the typed data boundary");

    let expected_failure = failure_fixture();
    let failure_shape = failure_shape();
    let failure_value = to_arcweft_value(&expected_failure)
        .expect("assertion failure enters the typed data boundary");
    let bytes = codec
        .encode_value(&failure_value, &failure_shape, &EncodeOptions::default())
        .expect("assertion failure encodes");
    let decoded = codec
        .decode_value(&bytes, &failure_shape, &DecodeOptions::default())
        .expect("assertion failure decodes");
    let actual_failure: RuntimeAssertionFailure =
        from_arcweft_value(&decoded).expect("assertion failure leaves the typed data boundary");

    assert_eq!(actual_fingerprint, expected_fingerprint);
    assert_eq!(actual_fingerprint.as_bytes(), &[9; 32]);
    assert_eq!(actual_failure, expected_failure);
    assert_eq!(
        actual_failure.assertion().guard(),
        RuntimeAssertionGuardId::try_from_bytes([7; 16]).expect("fixture guard")
    );
    assert_eq!(actual_failure.assertion().condition(), "ready");
    assert_eq!(actual_failure.assertion().message(), "must be ready");
    assert_eq!(
        actual_failure.assertion().profile(),
        RuntimeAssertionProfile::Always
    );
}

#[test]
fn invalid_guard_and_fingerprint_zero_values_are_rejected() {
    assert_eq!(
        RuntimeAssertionGuardId::try_from_bytes([0; 16]),
        Err(RuntimeIdentityDecodeError::ZeroAssertionGuard)
    );
    assert_eq!(
        RuntimeArtifactFingerprint::try_from_bytes([0; 32]),
        Err(RuntimeIdentityDecodeError::ZeroArtifactFingerprint)
    );
    assert_eq!(
        RuntimeAssertionGuardId::try_from_bytes([7; 16])
            .expect("valid guard")
            .as_bytes(),
        &[7; 16]
    );
    assert_eq!(
        RuntimeArtifactFingerprint::try_from_bytes([9; 32])
            .expect("valid fingerprint")
            .as_bytes(),
        &[9; 32]
    );
}

#[test]
fn runtime_assertion_core_codec_has_no_session_identity() {
    for codec in [
        &JsonCodec as &dyn Codec,
        &CborCodec,
        &MessagePackCodec,
        &ArcweftBinaryCodec,
    ] {
        assert_codec_round_trip(codec);
    }
}
