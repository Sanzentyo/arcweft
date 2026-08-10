use arcweft_manifest_model::{PackageId, PackageVersion};
use arcweft_resource_manifest::{
    PackageCoordinateFile, ResourceConstSourcePath, ResourceManifestDecodeLimits,
    ResourceManifestDiagnosticCode, decode_resource_type_manifest,
    encode_resource_type_manifest_v1,
};
use arcweft_resource_model::{
    identity::{ResourceFieldId, ResourceSchemaId},
    value::{ResourceValueTypePath, ResourceValueTypePathSegment},
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use std::sync::Arc;

fn minimal_document() -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("resource-manifest-test").unwrap(),
            SourceName::Memory,
            include_str!("fixtures/minimal.input.json"),
        )
        .unwrap(),
    )
}

fn coordinate() -> PackageCoordinateFile {
    PackageCoordinateFile::new(
        PackageId::new("org.example.weather").unwrap(),
        PackageVersion::new("1.0.0").unwrap(),
    )
}

fn full_coordinate() -> PackageCoordinateFile {
    PackageCoordinateFile::new(
        PackageId::new("org.example.resources").unwrap(),
        PackageVersion::new("2.3.4").unwrap(),
    )
}

#[test]
fn minimal_manifest_decodes_and_regenerates_frozen_canonical_bytes() {
    let accepted = decode_resource_type_manifest(
        minimal_document(),
        &coordinate(),
        ResourceManifestDecodeLimits::PRODUCTION,
    )
    .unwrap();
    let expected = include_bytes!("fixtures/minimal.canonical.json")
        .strip_suffix(b"\n")
        .expect("text fixture carries one repository newline");
    assert_eq!(accepted.canonical_bytes(), expected);
    assert_eq!(
        encode_resource_type_manifest_v1(accepted.typed()).unwrap(),
        expected
    );
    assert_eq!(
        accepted.canonical_digest().to_string(),
        "blake3:fba4266ffcf2c61bccaf179546dd2311f7aa4495538733c91438771e5c75f22f"
    );
}

#[test]
fn accepted_manifest_binds_typed_semantic_identities_to_the_same_source_revision() {
    let document = minimal_document();
    let accepted = decode_resource_type_manifest(
        document.clone(),
        &coordinate(),
        ResourceManifestDecodeLimits::PRODUCTION,
    )
    .unwrap();
    assert!(Arc::ptr_eq(accepted.document(), &document));
    let schema = accepted
        .source_map()
        .schemas()
        .get(&ResourceSchemaId::try_new("org.example.weather.station").unwrap())
        .unwrap();
    let field_id = ResourceFieldId::try_new(1).unwrap();
    let field = schema.fields().get(&field_id).unwrap();
    let path = ResourceValueTypePath::new([ResourceValueTypePathSegment::RecordField(field_id)]);
    let type_range = field.value_type_paths()[&path].value();
    assert_eq!(&document.text()[type_range.as_range()], "\"string\"");
    let default_range = field.default_paths()[&ResourceConstSourcePath::default()].value();
    assert_eq!(
        &document.text()[default_range.as_range()],
        "{ \"kind\": \"scalar\", \"value\": { \"kind\": \"string\", \"value\": \"Station\" } }"
    );
}

#[test]
fn full_manifest_covers_every_closed_wire_variant_and_frozen_digest() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("full-resource-manifest").unwrap(),
            SourceName::Memory,
            include_str!("fixtures/full.input.json"),
        )
        .unwrap(),
    );
    let accepted = decode_resource_type_manifest(
        document,
        &full_coordinate(),
        ResourceManifestDecodeLimits::PRODUCTION,
    )
    .unwrap();
    let expected = include_bytes!("fixtures/full.canonical.json")
        .strip_suffix(b"\n")
        .expect("text fixture carries one repository newline");
    assert_eq!(accepted.canonical_bytes(), expected);
    assert_eq!(
        accepted.canonical_digest().to_string(),
        "blake3:303b2035f837ee6593bf33cb02aeb4f7140ecf85cf3eb2ff7c40a20fd568ffaa"
    );
}

#[test]
fn canonical_redecode_preserves_source_independent_typed_semantics() {
    let original = decode_resource_type_manifest(
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("full-round-trip-input").unwrap(),
                SourceName::Memory,
                include_str!("fixtures/full.input.json"),
            )
            .unwrap(),
        ),
        &full_coordinate(),
        ResourceManifestDecodeLimits::PRODUCTION,
    )
    .unwrap();
    let canonical = std::str::from_utf8(original.canonical_bytes()).unwrap();
    let redecode = decode_resource_type_manifest(
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("full-round-trip-canonical").unwrap(),
                SourceName::Memory,
                canonical,
            )
            .unwrap(),
        ),
        &full_coordinate(),
        ResourceManifestDecodeLimits::PRODUCTION,
    )
    .unwrap();
    assert_eq!(original.typed(), redecode.typed());
    assert_eq!(original.canonical_bytes(), redecode.canonical_bytes());
}

#[test]
fn unordered_wire_collections_canonicalize_independently_of_authored_order() {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/full.input.json")).unwrap();
    value["schemas"].as_array_mut().unwrap().reverse();
    value["resource_types"].as_array_mut().unwrap().reverse();
    value["codecs"].as_array_mut().unwrap().reverse();
    for schema in value["schemas"].as_array_mut().unwrap() {
        let is_record = schema["kind"] == "record";
        let content = &mut schema["value"];
        let collection = if is_record { "fields" } else { "variants" };
        content[collection].as_array_mut().unwrap().reverse();
        if collection == "fields" {
            for field in content["fields"].as_array_mut().unwrap() {
                let Some(default) = field.as_object_mut().unwrap().get_mut("default") else {
                    continue;
                };
                match default["kind"].as_str() {
                    Some("ordered_map") => default["value"].as_array_mut().unwrap().reverse(),
                    Some("record") => default["value"]["fields"].as_array_mut().unwrap().reverse(),
                    Some(_) | None => {}
                }
            }
        }
    }
    for codec in value["codecs"].as_array_mut().unwrap() {
        codec["versions"].as_array_mut().unwrap().reverse();
    }
    let source = serde_json::to_string_pretty(&value).unwrap();
    let accepted = decode_resource_type_manifest(
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("permuted-full-resource-manifest").unwrap(),
                SourceName::Memory,
                source,
            )
            .unwrap(),
        ),
        &full_coordinate(),
        ResourceManifestDecodeLimits::PRODUCTION,
    )
    .unwrap();
    let expected = include_bytes!("fixtures/full.canonical.json")
        .strip_suffix(b"\n")
        .unwrap();
    assert_eq!(accepted.canonical_bytes(), expected);
}

#[test]
fn authored_sequence_order_remains_semantic() {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/full.input.json")).unwrap();
    let fields = value["schemas"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|schema| schema["kind"] == "record")
        .unwrap()["value"]["fields"]
        .as_array_mut()
        .unwrap();
    let checkpoints = fields
        .iter_mut()
        .find(|field| field["name"] == "checkpoints")
        .unwrap();
    checkpoints["default"]["value"]
        .as_array_mut()
        .unwrap()
        .reverse();
    let accepted = decode_resource_type_manifest(
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("reordered-sequence-resource-manifest").unwrap(),
                SourceName::Memory,
                serde_json::to_string(&value).unwrap(),
            )
            .unwrap(),
        ),
        &full_coordinate(),
        ResourceManifestDecodeLimits::PRODUCTION,
    )
    .unwrap();
    let expected = include_bytes!("fixtures/full.canonical.json")
        .strip_suffix(b"\n")
        .unwrap();
    assert_ne!(accepted.canonical_bytes(), expected);
}

#[test]
fn duplicate_key_is_rejected_with_both_revision_bound_ranges() {
    let source =
        r#"{"format":"arcweft.resource-type-manifest","format":"arcweft.resource-type-manifest"}"#;
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("duplicate-resource-manifest").unwrap(),
            SourceName::Memory,
            source,
        )
        .unwrap(),
    );
    let report = decode_resource_type_manifest(
        document,
        &coordinate(),
        ResourceManifestDecodeLimits::PRODUCTION,
    )
    .unwrap_err();
    let diagnostic = &report.diagnostics()[0];
    assert_eq!(
        diagnostic.code().as_str(),
        "resource_manifest.duplicate_key"
    );
    assert_eq!(diagnostic.related().len(), 1);
    assert_eq!(
        &source[diagnostic.primary().range().as_range()],
        "\"format\""
    );
    assert_eq!(
        &source[diagnostic.related()[0].span().range().as_range()],
        "\"format\""
    );
}

fn diagnostic_code(
    source: &str,
    expected: &PackageCoordinateFile,
) -> ResourceManifestDiagnosticCode {
    diagnostic_code_with_limits(source, expected, ResourceManifestDecodeLimits::PRODUCTION)
}

fn diagnostic_code_with_limits(
    source: &str,
    expected: &PackageCoordinateFile,
    limits: ResourceManifestDecodeLimits,
) -> ResourceManifestDiagnosticCode {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("negative-resource-manifest").unwrap(),
            SourceName::Memory,
            source,
        )
        .unwrap(),
    );
    decode_resource_type_manifest(document, expected, limits)
        .unwrap_err()
        .diagnostics()[0]
        .code()
}

fn duplicate_array_entry(source: &str, array_path: &[&str]) -> String {
    let mut value: serde_json::Value = serde_json::from_str(source).unwrap();
    let mut current = &mut value;
    for segment in array_path {
        current = if let Ok(index) = segment.parse::<usize>() {
            &mut current[index]
        } else {
            &mut current[*segment]
        };
    }
    let array = current.as_array_mut().unwrap();
    array.push(array[0].clone());
    serde_json::to_string(&value).unwrap()
}

fn limits(
    depth: usize,
    nodes: usize,
    string_bytes: usize,
    collection_items: usize,
    object_members: usize,
    semantic_records: usize,
    work_units: u64,
) -> ResourceManifestDecodeLimits {
    ResourceManifestDecodeLimits::new(
        ResourceManifestDecodeLimits::PRODUCTION.bytes(),
        depth,
        nodes,
        string_bytes,
        collection_items,
        object_members,
        semantic_records,
        work_units,
    )
}

#[test]
fn dispatch_and_closed_shape_failures_keep_distinct_codes() {
    let minimal = include_str!("fixtures/minimal.input.json");
    let cases = [
        (
            minimal.replace("\"format\": \"arcweft.resource-type-manifest\",", ""),
            ResourceManifestDiagnosticCode::MissingFormat,
        ),
        (
            minimal.replace("\"schema\": 1", "\"schema\": 2"),
            ResourceManifestDiagnosticCode::UnsupportedSchemaVersion,
        ),
        (
            minimal.replace("\"schema\": 1", "\"schema\": \"1\""),
            ResourceManifestDiagnosticCode::MalformedSchemaVersion,
        ),
        (
            minimal.replace("  \"schema\": 1,\n", ""),
            ResourceManifestDiagnosticCode::MissingSchemaVersion,
        ),
        (
            minimal.replace(
                "\"format\": \"arcweft.resource-type-manifest\"",
                "\"format\": \"other\"",
            ),
            ResourceManifestDiagnosticCode::UnsupportedFormat,
        ),
        (
            minimal.replace(
                "\"format\": \"arcweft.resource-type-manifest\"",
                "\"format\": []",
            ),
            ResourceManifestDiagnosticCode::MalformedFormat,
        ),
        (
            minimal.replace("\"schema\": 1,", "\"schema\": 1, \"extra\": true,"),
            ResourceManifestDiagnosticCode::UnknownField,
        ),
        (
            minimal.replace(
                "\"docs\": { \"summary\": \"A package-defined weather station resource.\" }",
                "\"docs\": null",
            ),
            ResourceManifestDiagnosticCode::NullNotAllowed,
        ),
        (
            minimal.replace("\"codecs\":", "\"removed_codecs\":"),
            ResourceManifestDiagnosticCode::UnknownField,
        ),
        (
            minimal.replace(
                "\"kind\": \"scalar\", \"value\": \"string\"",
                "\"kind\": \"bytes\", \"value\": \"AA==\"",
            ),
            ResourceManifestDiagnosticCode::UnknownTag,
        ),
        (
            minimal.replace(
                "\"kind\": \"scalar\", \"value\": \"string\"",
                "\"kind\": \"scalar\"",
            ),
            ResourceManifestDiagnosticCode::WrongTagContent,
        ),
        (
            minimal.replace(
                "\"kind\": \"string\", \"value\": \"Station\"",
                "\"kind\": \"unit\", \"value\": \"Station\"",
            ),
            ResourceManifestDiagnosticCode::WrongTagContent,
        ),
        (
            minimal.replace(
                "d5ee2afe4c115782169c8428574689716e5d9d621e202883c2beb1af8a76bb59",
                "05ee2afe4c115782169c8428574689716e5d9d621e202883c2beb1af8a76bb59",
            ),
            ResourceManifestDiagnosticCode::DescriptorDigestMismatch,
        ),
        (
            minimal.replace(
                "blake3:d5ee2afe4c115782169c8428574689716e5d9d621e202883c2beb1af8a76bb59",
                "sha256:bad",
            ),
            ResourceManifestDiagnosticCode::InvalidDigest,
        ),
    ];
    for (source, expected_code) in cases {
        assert_eq!(diagnostic_code(&source, &coordinate()), expected_code);
    }
}

#[test]
fn selected_package_coordinate_mismatch_points_at_the_document_package() {
    let report = decode_resource_type_manifest(
        minimal_document(),
        &PackageCoordinateFile::new(
            PackageId::new("org.example.other").unwrap(),
            PackageVersion::new("1.0.0").unwrap(),
        ),
        ResourceManifestDecodeLimits::PRODUCTION,
    )
    .unwrap_err();
    let diagnostic = &report.diagnostics()[0];
    assert_eq!(
        diagnostic.code(),
        ResourceManifestDiagnosticCode::PackageMismatch
    );
    assert!(
        minimal_document().text()[diagnostic.primary().range().as_range()]
            .contains("org.example.weather")
    );
}

#[test]
fn scalar_wire_failures_keep_typed_codes() {
    let minimal = include_str!("fixtures/minimal.input.json");
    let scalar = "\"kind\": \"string\", \"value\": \"Station\"";
    let cases = [
        (
            "\"kind\": \"float\", \"value\": \"0x7ff0000000000000\"",
            ResourceManifestDiagnosticCode::NonFiniteFloat,
        ),
        (
            "\"kind\": \"float\", \"value\": \"0x7ff8000000000000\"",
            ResourceManifestDiagnosticCode::NonFiniteFloat,
        ),
        (
            "\"kind\": \"float\", \"value\": \"0x8000000000000000\"",
            ResourceManifestDiagnosticCode::NonCanonicalFloat,
        ),
        (
            "\"kind\": \"signed_integer\", \"value\": 9223372036854775808",
            ResourceManifestDiagnosticCode::IntegerOverflow,
        ),
        (
            "\"kind\": \"unsigned_integer\", \"value\": 18446744073709551616",
            ResourceManifestDiagnosticCode::IntegerOverflow,
        ),
        (
            "\"kind\": \"signed_integer\", \"value\": 1.0",
            ResourceManifestDiagnosticCode::InvalidInteger,
        ),
        (
            "\"kind\": \"signed_integer\", \"value\": 1e2",
            ResourceManifestDiagnosticCode::InvalidInteger,
        ),
        (
            "\"kind\": \"float\", \"value\": \"0x000000000000000A\"",
            ResourceManifestDiagnosticCode::NonCanonicalFloat,
        ),
        (
            "\"kind\": \"char\", \"value\": \"ab\"",
            ResourceManifestDiagnosticCode::InvalidString,
        ),
    ];
    for (replacement, expected_code) in cases {
        assert_eq!(
            diagnostic_code(&minimal.replace(scalar, replacement), &coordinate()),
            expected_code
        );
    }
}

#[test]
fn duplicate_semantic_records_retain_first_and_duplicate_identity_ranges() {
    let minimal = include_str!("fixtures/minimal.input.json");
    let cases = [
        duplicate_array_entry(minimal, &["schemas"]),
        duplicate_array_entry(minimal, &["resource_types"]),
        duplicate_array_entry(minimal, &["codecs"]),
        duplicate_array_entry(minimal, &["schemas", "0", "value", "fields"]),
        duplicate_array_entry(minimal, &["codecs", "0", "versions"]),
    ];
    for source in cases {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("duplicate-semantic-record").unwrap(),
                SourceName::Memory,
                source.as_str(),
            )
            .unwrap(),
        );
        let report = decode_resource_type_manifest(
            document,
            &coordinate(),
            ResourceManifestDecodeLimits::PRODUCTION,
        )
        .unwrap_err();
        let diagnostic = &report.diagnostics()[0];
        assert_eq!(
            diagnostic.code(),
            ResourceManifestDiagnosticCode::DuplicateRecord
        );
        assert_eq!(diagnostic.related().len(), 1);
        assert_ne!(
            diagnostic.primary().range(),
            diagnostic.related()[0].span().range()
        );
    }
}

#[test]
fn duplicate_nested_map_and_record_entries_are_semantic_record_failures() {
    let full: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/full.input.json")).unwrap();
    let duplicate = |field_name: &str, collection: &str| {
        let mut value = full.clone();
        let fields = value["schemas"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|schema| {
                schema["kind"] == "record"
                    && schema["value"]["schema_id"] == "org.example.resources.catalog"
            })
            .unwrap()["value"]["fields"]
            .as_array_mut()
            .unwrap();
        let field = fields
            .iter_mut()
            .find(|field| field["name"] == field_name)
            .unwrap();
        let entries = if collection == "fields" {
            field["default"]["value"]["fields"].as_array_mut().unwrap()
        } else {
            field["default"]["value"].as_array_mut().unwrap()
        };
        entries.push(entries[0].clone());
        serde_json::to_string(&value).unwrap()
    };
    for source in [
        duplicate("metadata", "fields"),
        duplicate("weights", "entries"),
    ] {
        assert_eq!(
            diagnostic_code(&source, &full_coordinate()),
            ResourceManifestDiagnosticCode::DuplicateRecord
        );
    }
}

#[test]
fn structural_limits_are_inclusive_and_fail_with_their_exact_codes() {
    let source = include_str!("fixtures/minimal.input.json");
    let exact = limits(8, 104, 71, 1, 8, 11, 187);
    decode_resource_type_manifest(minimal_document(), &coordinate(), exact).unwrap();

    let one_over = [
        (
            limits(7, 104, 71, 1, 8, 11, 187),
            ResourceManifestDiagnosticCode::DepthLimit,
        ),
        (
            limits(8, 103, 71, 1, 8, 11, 187),
            ResourceManifestDiagnosticCode::NodeLimit,
        ),
        (
            limits(8, 104, 70, 1, 8, 11, 187),
            ResourceManifestDiagnosticCode::StringLimit,
        ),
        (
            limits(8, 104, 71, 0, 8, 11, 187),
            ResourceManifestDiagnosticCode::CollectionLimit,
        ),
        (
            limits(8, 104, 71, 1, 7, 11, 187),
            ResourceManifestDiagnosticCode::RecordLimit,
        ),
        (
            limits(8, 104, 71, 1, 8, 10, 187),
            ResourceManifestDiagnosticCode::RecordLimit,
        ),
        (
            limits(8, 104, 71, 1, 8, 11, 186),
            ResourceManifestDiagnosticCode::WorkLimit,
        ),
    ];
    for (limits, expected) in one_over {
        assert_eq!(
            diagnostic_code_with_limits(source, &coordinate(), limits),
            expected
        );
    }
}
