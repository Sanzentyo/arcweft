use arcweft_core::effect::{
    RuntimeAssertion, RuntimeAssertionFailure, RuntimeAssertionGuardId, RuntimeAssertionProfile,
};
use arcweft_save::{
    SaveDecodeOptions, SaveSchemaId, decode_strict_typed_json_save, encode_typed_json_save,
};

#[test]
fn save_round_trip_retains_the_real_core_runtime_assertion_payload() {
    let schema = SaveSchemaId::new("arcweft.runtime-assertion-failure");
    let failure = RuntimeAssertionFailure::new(RuntimeAssertion::new(
        RuntimeAssertionGuardId::try_from_bytes([0xa7; 16]).expect("non-zero assertion guard"),
        "inventory >= 0".to_owned(),
        "inventory must stay non-negative".to_owned(),
        RuntimeAssertionProfile::Always,
    ));

    let encoded = encode_typed_json_save(&failure, schema.clone(), 1)
        .expect("encode the core assertion failure through the existing save envelope");
    let decoded: RuntimeAssertionFailure =
        decode_strict_typed_json_save(&encoded, &schema, 1, &SaveDecodeOptions::default())
            .expect("strictly decode the same core assertion failure type");

    assert_eq!(decoded, failure);
    assert_eq!(
        decoded.assertion().guard(),
        RuntimeAssertionGuardId::try_from_bytes([0xa7; 16]).expect("fixture guard")
    );
    assert_eq!(decoded.assertion().condition(), "inventory >= 0");
    assert_eq!(
        decoded.assertion().message(),
        "inventory must stay non-negative"
    );
    assert_eq!(
        decoded.assertion().profile(),
        RuntimeAssertionProfile::Always
    );
}
