use arcweft_adapter_metadata::{AdapterMetadataCodecError, SourceBackedAdapterMetadata};
use arcweft_manifest_model::RawDigest;

const RUST_METADATA: &str = include_str!("fixtures/truck-rust.adapter.json");

#[test]
fn canonical_rust_fixture_matches_all_published_hashes() {
    let sourced = SourceBackedAdapterMetadata::decode(RUST_METADATA).unwrap();
    let metadata = sourced.metadata();
    assert_eq!(
        metadata.abi_hash.to_string(),
        "blake3:3dcd9ee62412b77d378faa808f5975e87e16c692b40bd2fc1f5093b9f3c6fac2"
    );
    assert_eq!(
        metadata.payload_hash.to_string(),
        "blake3:9e816b9128414a1c9acc72e405076da5e8f0c9a129a6778a2f98b469e0961b47"
    );
    assert_eq!(
        RawDigest::for_bytes(RUST_METADATA.as_bytes()).to_string(),
        "blake3:07f76b02974f97d7ce43bf62835f0f94f4b61802d7361b1633a92d52dfb68612"
    );
}

#[test]
fn wasm_and_process_use_the_same_neutral_envelope_and_hash_rules() {
    let wasm = RUST_METADATA
        .replace(
            "blake3:3dcd9ee62412b77d378faa808f5975e87e16c692b40bd2fc1f5093b9f3c6fac2",
            "blake3:5de2833365190622f084db5ed5ed3324159b338dfa080047f902f845b92bad03",
        )
        .replace(
            "blake3:be46e89f4e355d945dcef1765b077c4054aafbe857e3b1a4142b7287663ee61a",
            "blake3:2ceb0ef9468a0094c22bf8fc4628488f5f59201ad35e4016334a6bc27ad476c9",
        )
        .replace("artifacts/truck-rust.fixture", "artifacts/truck-wasm.fixture")
        .replace("\"size\": 41", "\"size\": 51")
        .replace("arcweft-rust-metadata", "arcweft-wasm-metadata")
        .replace(
            "blake3:9e816b9128414a1c9acc72e405076da5e8f0c9a129a6778a2f98b469e0961b47",
            "blake3:f93ca0b35f71adcf9753d9029b289d303bc8e07312a31c59553b92d825ee022a",
        )
        .replace(
            "\"abi\": \"arcweft-rust-v1\",\n    \"family\": \"rust\",\n    \"target_triple\": \"x86_64-unknown-linux-gnu\"",
            "\"abi\": \"arcweft-wasm-component-v1\",\n    \"family\": \"wasm\",\n    \"world\": \"arcweft:activity/host@1.0.0\"",
        );
    SourceBackedAdapterMetadata::decode(&wasm).unwrap();

    let process = RUST_METADATA
        .replace(
            "blake3:3dcd9ee62412b77d378faa808f5975e87e16c692b40bd2fc1f5093b9f3c6fac2",
            "blake3:859b1ed7b20c780a6636ff13eac422e98f955c8d75b387e170ce98032b4c5043",
        )
        .replace(
            "blake3:be46e89f4e355d945dcef1765b077c4054aafbe857e3b1a4142b7287663ee61a",
            "blake3:a4bfa2af8e7ed5df0bc2bbf30cfbf137271d56f39eda4ee25ed57894d182bf3c",
        )
        .replace("artifacts/truck-rust.fixture", "artifacts/truck-process.fixture")
        .replace("\"size\": 41", "\"size\": 53")
        .replace("arcweft-rust-metadata", "arcweft-process-metadata")
        .replace(
            "blake3:9e816b9128414a1c9acc72e405076da5e8f0c9a129a6778a2f98b469e0961b47",
            "blake3:7b44a1d1653ee3938ca60d51bef80c15c98f8ae85e5a9d9aee7d89fef19def78",
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
        "\"types\": [{\"name\":\"TruckResult\",\"visibility\":\"public\",\"shape\":{\"kind\":\"opaque\"}},",
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
