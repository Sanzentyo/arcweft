use arcweft_adapter_metadata::{
    AdapterMetadataCodecError, AdapterMetadataDecodeLimitKind, AdapterMetadataDecodeLimits,
    SourceBackedAdapterMetadata, StrictJsonError,
};
use arcweft_manifest_model::RawDigest;

const RUST_METADATA: &str = include_str!("fixtures/truck-rust.adapter.json");

#[test]
fn canonical_rust_fixture_matches_all_published_hashes() {
    let sourced = SourceBackedAdapterMetadata::decode(RUST_METADATA).unwrap();
    let metadata = sourced.metadata();
    assert_eq!(
        metadata.abi_hash.to_string(),
        "blake3:69ecedf29d819880d3a5fcb1058fbecadc3a9581fac68c8e574c1e96e103055a"
    );
    assert_eq!(
        metadata.payload_hash.to_string(),
        "blake3:46b5fbf8dc329eed43cd08e00de6e2e376a55d5df562f5fd6d4005d25e091b93"
    );
    assert_eq!(
        RawDigest::for_bytes(RUST_METADATA.as_bytes()).to_string(),
        "blake3:083fdc0211e9f2497b85ec91c2362239e328455c7d3b33da04d33ce7ac582c2d"
    );
}

#[test]
fn wasm_and_process_use_the_same_neutral_envelope_and_hash_rules() {
    let wasm = RUST_METADATA
        .replace(
            "blake3:69ecedf29d819880d3a5fcb1058fbecadc3a9581fac68c8e574c1e96e103055a",
            "blake3:c865c37a460777e93d83e84e67576d4c4a810469d09b765480af7332fa72a87e",
        )
        .replace(
            "blake3:be46e89f4e355d945dcef1765b077c4054aafbe857e3b1a4142b7287663ee61a",
            "blake3:2ceb0ef9468a0094c22bf8fc4628488f5f59201ad35e4016334a6bc27ad476c9",
        )
        .replace("artifacts/truck-rust.fixture", "artifacts/truck-wasm.fixture")
        .replace("\"size\": 41", "\"size\": 51")
        .replace("arcweft-rust-metadata", "arcweft-wasm-metadata")
        .replace(
            "blake3:46b5fbf8dc329eed43cd08e00de6e2e376a55d5df562f5fd6d4005d25e091b93",
            "blake3:a3461ff5b04b828dd6b7954d1922f82d699713a2ccbc90d3b906fe7387fef4e5",
        )
        .replace(
            "\"abi\": \"arcweft-rust-v1\",\n    \"family\": \"rust\",\n    \"target_triple\": \"x86_64-unknown-linux-gnu\"",
            "\"abi\": \"arcweft-wasm-component-v1\",\n    \"family\": \"wasm\",\n    \"world\": \"arcweft:activity/host@1.0.0\"",
        );
    SourceBackedAdapterMetadata::decode(&wasm).unwrap();

    let process = RUST_METADATA
        .replace(
            "blake3:69ecedf29d819880d3a5fcb1058fbecadc3a9581fac68c8e574c1e96e103055a",
            "blake3:054e328a6ae88cafdcb7d650f7d4a504d47e888dd532f1108ec1d70d5f53513d",
        )
        .replace(
            "blake3:be46e89f4e355d945dcef1765b077c4054aafbe857e3b1a4142b7287663ee61a",
            "blake3:a4bfa2af8e7ed5df0bc2bbf30cfbf137271d56f39eda4ee25ed57894d182bf3c",
        )
        .replace("artifacts/truck-rust.fixture", "artifacts/truck-process.fixture")
        .replace("\"size\": 41", "\"size\": 53")
        .replace("arcweft-rust-metadata", "arcweft-process-metadata")
        .replace(
            "blake3:46b5fbf8dc329eed43cd08e00de6e2e376a55d5df562f5fd6d4005d25e091b93",
            "blake3:c15164edaed6af67647dd384ad26141b2e2453811cfc37e1669745de360c6356",
        )
        .replace(
            "\"abi\": \"arcweft-rust-v1\",\n    \"family\": \"rust\",\n    \"target_triple\": \"x86_64-unknown-linux-gnu\"",
            "\"abi\": \"arcweft-process-v1\",\n    \"family\": \"process\",\n    \"transport\": \"stdio-framed-v1\"",
        );
    SourceBackedAdapterMetadata::decode(&process).unwrap();
}

#[test]
fn unknown_fields_and_explicit_null_are_rejected() {
    let unknown = RUST_METADATA.replacen(
        "\"schema\": 1,",
        "\"schema\": 1, \"legacy_symbols\": [],",
        1,
    );
    assert!(matches!(
        SourceBackedAdapterMetadata::decode(&unknown),
        Err(AdapterMetadataCodecError::Typed { .. })
    ));

    let null = RUST_METADATA.replacen(
        "\"target_triple\": \"x86_64-unknown-linux-gnu\"",
        "\"target_triple\": null",
        1,
    );
    assert!(matches!(
        SourceBackedAdapterMetadata::decode(&null),
        Err(AdapterMetadataCodecError::Json(_))
    ));
}

#[test]
fn capability_policy_is_rejected_as_an_ordinary_unknown_metadata_field() {
    let unknown = RUST_METADATA.replacen(
        "\"schema\": 1,",
        "\"schema\": 1, \"capability_policy\": {},",
        1,
    );

    let error = SourceBackedAdapterMetadata::decode(&unknown)
        .expect_err("unknown policy field is rejected");
    let AdapterMetadataCodecError::Typed { message } = error else {
        panic!("expected typed metadata rejection, got {error:?}");
    };
    assert!(
        message.contains("unknown field `capability_policy`"),
        "unexpected typed metadata error: {message}"
    );
}

#[test]
fn format_schema_family_and_nominal_ids_are_closed() {
    for tampered in [
        RUST_METADATA.replacen(
            "arcweft.adapter-metadata",
            "arcweft.adapter-metadata-legacy",
            1,
        ),
        RUST_METADATA.replacen("\"schema\": 1", "\"schema\": 2", 1),
        RUST_METADATA.replacen("\"family\": \"rust\"", "\"family\": \"native\"", 1),
        RUST_METADATA.replacen("com.example.truck", "Com.Example.Truck", 1),
    ] {
        assert!(matches!(
            SourceBackedAdapterMetadata::decode(&tampered),
            Err(AdapterMetadataCodecError::Typed { .. })
        ));
    }

    let wrong_target_payload = RUST_METADATA.replacen(
        "\"target_triple\": \"x86_64-unknown-linux-gnu\"",
        "\"target_triple\": \"x86_64-unknown-linux-gnu\", \"world\": \"legacy\"",
        1,
    );
    assert!(matches!(
        SourceBackedAdapterMetadata::decode(&wrong_target_payload),
        Err(AdapterMetadataCodecError::Typed { .. })
    ));
}

#[test]
fn payload_and_abi_tampering_are_distinguished() {
    let payload_tamper =
        RUST_METADATA.replacen("arcweft-rust-metadata", "arcweft-rust-generator", 1);
    assert_eq!(
        SourceBackedAdapterMetadata::decode(&payload_tamper).unwrap_err(),
        AdapterMetadataCodecError::PayloadHashMismatch
    );

    let abi_tamper = RUST_METADATA.replacen("TruckTelemetry", "TruckStatistics", 1);
    assert_eq!(
        SourceBackedAdapterMetadata::decode(&abi_tamper).unwrap_err(),
        AdapterMetadataCodecError::AbiHashMismatch
    );
}

#[test]
fn duplicate_exports_and_requirements_are_rejected_before_hash_verification() {
    let duplicate_export = RUST_METADATA.replacen(
        "\"types\": [",
        "\"types\": [{\"name\":\"TruckResult\",\"visibility\":\"public\",\"opaque_producer\":\"fixture.project.external-types\",\"shape\":{\"kind\":\"opaque\"}},",
        1,
    );
    assert!(matches!(
        SourceBackedAdapterMetadata::decode(&duplicate_export),
        Err(AdapterMetadataCodecError::DuplicateExport { .. })
    ));

    let duplicate_requirement = RUST_METADATA.replacen(
        "\"requirements\": [",
        "\"requirements\": [{\"kind\":\"capability\",\"id\":\"desktop.platform\",\"demand\":\"required\",\"interface_hash\":\"blake3:fe55d5fd0fe4707d8de48c97abdb00ab9231d003a37848afc472251b1cf82a35\"},",
        1,
    );
    assert!(matches!(
        SourceBackedAdapterMetadata::decode(&duplicate_requirement),
        Err(AdapterMetadataCodecError::DuplicateRequirement { .. })
    ));
}

#[test]
fn public_decoder_reports_typed_resource_limit_failures() {
    let error = SourceBackedAdapterMetadata::decode_with_limits(
        RUST_METADATA,
        AdapterMetadataDecodeLimits::new(32, usize::MAX, usize::MAX),
    )
    .expect_err("metadata must exceed the test byte limit");

    assert!(matches!(
        error,
        AdapterMetadataCodecError::Json(StrictJsonError::Limit {
            kind: AdapterMetadataDecodeLimitKind::Bytes,
            observed,
            maximum: 32,
            span: None,
        }) if observed == RUST_METADATA.len()
    ));

    let depth_error = SourceBackedAdapterMetadata::decode_with_limits(
        RUST_METADATA,
        AdapterMetadataDecodeLimits::new(usize::MAX, 1, usize::MAX),
    )
    .expect_err("metadata must exceed the test nesting limit");
    assert!(matches!(
        depth_error,
        AdapterMetadataCodecError::Json(StrictJsonError::Limit {
            kind: AdapterMetadataDecodeLimitKind::NestingDepth,
            observed: 2,
            maximum: 1,
            span: Some(_),
        })
    ));

    let node_error = SourceBackedAdapterMetadata::decode_with_limits(
        RUST_METADATA,
        AdapterMetadataDecodeLimits::new(usize::MAX, usize::MAX, 1),
    )
    .expect_err("metadata must exceed the test lexical-node limit");
    assert!(matches!(
        node_error,
        AdapterMetadataCodecError::Json(StrictJsonError::Limit {
            kind: AdapterMetadataDecodeLimitKind::Nodes,
            observed: 2,
            maximum: 1,
            span: Some(_),
        })
    ));
}
