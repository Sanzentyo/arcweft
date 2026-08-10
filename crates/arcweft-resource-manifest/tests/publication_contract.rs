use arcweft_manifest_model::{PackageId, PackageVersion};
use arcweft_resource_manifest::{
    PackageCoordinateFile, ResourceManifestDecodeLimits, ResourceManifestDiagnosticCode,
    ResourceManifestPublicationLimits, decode_resource_type_manifest,
    publish_resource_type_manifests_v1,
};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use std::sync::Arc;

fn decode(
    id: &str,
    source: String,
) -> arcweft_resource_manifest::SourceBackedResourceTypeManifestV1 {
    decode_version(id, "1.0.0", source)
}

fn decode_version(
    id: &str,
    version: &str,
    source: String,
) -> arcweft_resource_manifest::SourceBackedResourceTypeManifestV1 {
    decode_resource_type_manifest(
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(format!("manifest-{id}")).unwrap(),
                SourceName::Memory,
                source,
            )
            .unwrap(),
        ),
        &PackageCoordinateFile::new(
            PackageId::new(id).unwrap(),
            PackageVersion::new(version).unwrap(),
        ),
        ResourceManifestDecodeLimits::PRODUCTION,
    )
    .unwrap()
}

fn decode_error_version(
    id: &str,
    version: &str,
    source: String,
) -> arcweft_resource_manifest::ResourceManifestReport {
    decode_resource_type_manifest(
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(format!("manifest-error-{id}")).unwrap(),
                SourceName::Memory,
                source,
            )
            .unwrap(),
        ),
        &PackageCoordinateFile::new(
            PackageId::new(id).unwrap(),
            PackageVersion::new(version).unwrap(),
        ),
        ResourceManifestDecodeLimits::PRODUCTION,
    )
    .unwrap_err()
}

fn assert_registry_validation(source: String) {
    let manifest = decode_version("org.example.resources", "2.3.4", source);
    let report = publish_resource_type_manifests_v1(
        &ResourceTypeRegistry::empty(),
        [manifest],
        ResourceManifestPublicationLimits::PRODUCTION,
    )
    .unwrap_err();
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == ResourceManifestDiagnosticCode::RegistryValidation
    }));
}

fn schema_only_manifest(package: &str) -> String {
    format!(
        r#"{{
          "format":"arcweft.resource-type-manifest",
          "schema":1,
          "package":{{"id":"{package}","version":"1.0.0"}},
          "schemas":[{{"kind":"record","value":{{
            "schema_id":"org.example.shared",
            "nominal_type":{{"package":"{package}","module":"shared","name":"Shared"}},
            "version":1,
            "fields":[]
          }}}}],
          "resource_types":[],
          "codecs":[]
        }}"#
    )
}

#[test]
fn aggregate_duplicate_schema_keeps_cross_revision_primary_and_related_spans() {
    let first = decode("org.example.a", schema_only_manifest("org.example.a"));
    let second = decode("org.example.b", schema_only_manifest("org.example.b"));
    let report = publish_resource_type_manifests_v1(
        &ResourceTypeRegistry::empty(),
        [second, first],
        ResourceManifestPublicationLimits::PRODUCTION,
    )
    .unwrap_err();
    let diagnostic = &report.diagnostics()[0];
    assert_eq!(
        diagnostic.code(),
        ResourceManifestDiagnosticCode::DuplicateRecord
    );
    assert_eq!(diagnostic.related().len(), 1);
    assert_eq!(
        diagnostic.primary().source().id().as_str(),
        "manifest-org.example.b"
    );
    assert_eq!(
        diagnostic.related()[0].span().source().id().as_str(),
        "manifest-org.example.a"
    );
}

#[test]
fn selected_versions_for_one_package_are_rejected_before_registry_publication() {
    let first = decode_version(
        "org.example.a",
        "1.0.0",
        schema_only_manifest("org.example.a"),
    );
    let second = decode_version(
        "org.example.a",
        "2.0.0",
        schema_only_manifest("org.example.a").replace("1.0.0", "2.0.0"),
    );
    let report = publish_resource_type_manifests_v1(
        &ResourceTypeRegistry::empty(),
        [second, first],
        ResourceManifestPublicationLimits::PRODUCTION,
    )
    .unwrap_err();
    assert_eq!(
        report.diagnostics()[0].code(),
        ResourceManifestDiagnosticCode::VersionConflict
    );
}

#[test]
fn required_defaults_nonempty_lists_and_inverted_constraints_fail_closed() {
    let full = include_str!("fixtures/full.input.json");

    let required_default = full.replacen(
        "\"presence\": \"required\"",
        "\"presence\": \"required\", \"default\": {\"kind\":\"scalar\",\"value\":{\"kind\":\"string\",\"value\":\"forbidden\"}}",
        1,
    );
    assert_registry_validation(required_default);

    let checkpoints = full.find("\"name\": \"checkpoints\"").unwrap();
    let list_value = full[checkpoints..]
        .find("\"value\": [")
        .map(|offset| checkpoints + offset)
        .unwrap();
    let list_end = full[list_value..]
        .find("\n              ]")
        .map(|offset| list_value + offset + "\n              ]".len())
        .unwrap();
    let mut empty_nonempty_list = full.to_owned();
    empty_nonempty_list.replace_range(list_value..list_end, "\"value\": []");
    assert_registry_validation(empty_nonempty_list);

    let bounded = full.find("\"name\": \"bounded\"").unwrap();
    let upper = full[bounded..]
        .find("\"value\": 10")
        .map(|offset| bounded + offset)
        .unwrap();
    let mut inverted_constraint = full.to_owned();
    inverted_constraint.replace_range(upper..upper + "\"value\": 10".len(), "\"value\": -1");
    let report = decode_error_version("org.example.resources", "2.3.4", inverted_constraint);
    assert_eq!(
        report.diagnostics()[0].code(),
        ResourceManifestDiagnosticCode::InvalidId
    );

    let default = full[bounded..]
        .find("\"value\": 5")
        .map(|offset| bounded + offset)
        .unwrap();
    let mut constraint_mismatch = full.to_owned();
    constraint_mismatch.replace_range(default..default + "\"value\": 5".len(), "\"value\": 15");
    assert_registry_validation(constraint_mismatch);
}

#[test]
fn invalid_default_points_at_default_and_relates_its_typed_field_contract() {
    let source = include_str!("fixtures/minimal.input.json").replace(
        "\"kind\": \"string\", \"value\": \"Station\"",
        "\"kind\": \"signed_integer\", \"value\": 7",
    );
    let manifest = decode("org.example.weather", source);
    let document = Arc::clone(manifest.document());
    let report = publish_resource_type_manifests_v1(
        &ResourceTypeRegistry::empty(),
        [manifest],
        ResourceManifestPublicationLimits::PRODUCTION,
    )
    .unwrap_err();
    let diagnostic = report
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == ResourceManifestDiagnosticCode::RegistryValidation)
        .unwrap();
    assert!(document.text()[diagnostic.primary().range().as_range()].contains("signed_integer"));
    assert_eq!(diagnostic.related().len(), 1);
    assert_eq!(
        &document.text()[diagnostic.related()[0].span().range().as_range()],
        "\"string\""
    );
}

#[test]
fn aggregate_record_and_work_limits_are_inclusive() {
    let source = schema_only_manifest("org.example.a");
    publish_resource_type_manifests_v1(
        &ResourceTypeRegistry::empty(),
        [decode("org.example.a", source.clone())],
        ResourceManifestPublicationLimits::new(1, 5),
    )
    .unwrap();

    let work_report = publish_resource_type_manifests_v1(
        &ResourceTypeRegistry::empty(),
        [decode("org.example.a", source.clone())],
        ResourceManifestPublicationLimits::new(1, 4),
    )
    .unwrap_err();
    assert_eq!(
        work_report.diagnostics()[0].code(),
        ResourceManifestDiagnosticCode::WorkLimit
    );

    let record_report = publish_resource_type_manifests_v1(
        &ResourceTypeRegistry::empty(),
        [decode("org.example.a", source)],
        ResourceManifestPublicationLimits::new(0, 5),
    )
    .unwrap_err();
    assert_eq!(
        record_report.diagnostics()[0].code(),
        ResourceManifestDiagnosticCode::RecordLimit
    );
}

#[test]
fn nested_record_default_uses_nested_value_and_nested_field_type_ranges() {
    let mut source = include_str!("fixtures/full.input.json").to_owned();
    let value = source.find("\"value\": \"meta\"").unwrap();
    let kind = source[..value].rfind("\"kind\": \"string\"").unwrap();
    source.replace_range(value..value + "\"value\": \"meta\"".len(), "\"value\": 7");
    source.replace_range(
        kind..kind + "\"kind\": \"string\"".len(),
        "\"kind\": \"signed_integer\"",
    );
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("nested-default-manifest").unwrap(),
            SourceName::Memory,
            source,
        )
        .unwrap(),
    );
    let manifest = decode_resource_type_manifest(
        document.clone(),
        &PackageCoordinateFile::new(
            PackageId::new("org.example.resources").unwrap(),
            PackageVersion::new("2.3.4").unwrap(),
        ),
        ResourceManifestDecodeLimits::PRODUCTION,
    )
    .unwrap();
    let report = publish_resource_type_manifests_v1(
        &ResourceTypeRegistry::empty(),
        [manifest],
        ResourceManifestPublicationLimits::PRODUCTION,
    )
    .unwrap_err();
    let diagnostic = report
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.message().contains("field 19"))
        .unwrap();
    let primary = &document.text()[diagnostic.primary().range().as_range()];
    assert!(primary.contains("signed_integer"));
    assert_eq!(diagnostic.related().len(), 1);
    let related = &document.text()[diagnostic.related()[0].span().range().as_range()];
    assert!(related.contains("\"string\""));
}
