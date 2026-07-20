use arcweft_manifest_model::PackageId;
use arcweft_resource_model::descriptor::{
    ResourceAgentExposure, ResourceCapabilities, ResourceCodecSupport,
    ResourceDescriptorProvenance, ResourceEnumSchema, ResourceFieldDescriptor,
    ResourceHotReloadClass, ResourceLoweringBinding, ResourceRecordSchema, ResourceTypeDescriptor,
    ResourceTypeDocs, ResourceValueSchema, ResourceVariantDescriptor,
};
use arcweft_resource_model::identity::{
    NominalTypeId, ResourceAssetPayloadKindId, ResourceBundleSectionId,
    ResourceBundleSectionVersion, ResourceCodecId, ResourceCodecVersion,
    ResourceDescriptorSourceId, ResourceFamilyGroupId, ResourceFieldId, ResourceFieldName,
    ResourceModulePath, ResourcePublicIdFamily, ResourceSchemaId, ResourceSchemaVersion,
    ResourceTypeId, ResourceTypeName, ResourceVariantId, ResourceVariantName,
};
use arcweft_resource_model::registry::{
    RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION, ResourceDefaultValidationError, ResourceRegistryIssue,
    ResourceRegistryPublication, ResourceTypeRegistry,
};
use arcweft_resource_model::retained::{ResolvedRetainedIdentityRef, RetainedIdentityKind};
use arcweft_resource_model::value::{
    ResourceConstValue, ResourceEnumValue, ResourceRecordValue, ResourceScalarType,
    ResourceScalarValue, ResourceSchemaError, ResourceValueType, ResourceValueTypePath,
    ResourceValueTypePathSegment, ResourceValueValidationError,
};

#[test]
fn publication_is_canonical_and_ignores_docs_and_provenance_in_semantic_digest() {
    let codec_id = codec_id("example.resource");
    let left = canonical_registry(&codec_id, false);
    let right = canonical_registry(&codec_id, true);

    assert_eq!(left.digest(), right.digest());
    assert_eq!(
        left.types()
            .map(|(type_id, _)| type_id.nominal().name().as_str())
            .collect::<Vec<_>>(),
        ["Voice", "VoiceProfile"]
    );
    assert_eq!(
        left.schema(&schema_id("example.voice"))
            .and_then(|schema| match schema {
                ResourceValueSchema::Record(schema) => Some(
                    schema
                        .fields()
                        .iter()
                        .map(|field| field.id().get())
                        .collect::<Vec<_>>(),
                ),
                ResourceValueSchema::Enum(_) => None,
            }),
        Some(vec![1, 2])
    );
    left.verify_integrity().unwrap();
    right.verify_integrity().unwrap();
}

#[test]
fn exact_manifest_version_is_the_only_accepted_version() {
    for actual in [0, 2] {
        let error =
            ResourceTypeRegistry::publish(ResourceRegistryPublication::new(actual, [], [], []))
                .unwrap_err();
        assert_eq!(
            error.issues(),
            [ResourceRegistryIssue::UnsupportedManifestSchemaVersion {
                expected: RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
                actual,
            }]
        );
    }
}

#[test]
fn value_type_owns_exact_reference_validation_and_preserves_the_caller_path() {
    let registry = ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
        RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
        [],
        [],
        [],
    ))
    .unwrap();
    let initial_path = ResourceValueTypePath::new([ResourceValueTypePathSegment::MapValue]);
    let value_type = ResourceValueType::option(ResourceValueType::ResourceRef {
        type_id: resource_type("Missing"),
    });

    assert_eq!(
        value_type.validate_reference_invariants(&registry, &initial_path),
        Err(ResourceSchemaError::UnknownResourceType {
            path: ResourceValueTypePath::new([
                ResourceValueTypePathSegment::MapValue,
                ResourceValueTypePathSegment::OptionValue,
            ]),
            type_id: resource_type("Missing"),
        })
    );

    let registry = canonical_registry(&codec_id("example.resource"), false);
    ResourceValueType::NominalRecord(schema_id("example.voice"))
        .validate_reference_invariants(&registry, &ResourceValueTypePath::default())
        .unwrap();
}

#[test]
fn duplicate_type_evidence_is_deterministic_for_every_input_order() {
    let codec_id = codec_id("example.duplicate");
    let schema = record_schema("WeatherIcon", "example.weather", []);
    let earlier = descriptor(
        "WeatherIcon",
        "example.weather",
        "weather",
        "example.weather",
        &codec_id,
        "a/source.arcw",
    );
    let later = descriptor(
        "WeatherIcon",
        "example.weather",
        "weather",
        "example.weather",
        &codec_id,
        "z/source.arcw",
    );

    let publish = |descriptors| {
        ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
            RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
            [schema.clone()],
            descriptors,
            [codec(&codec_id, [1])],
        ))
        .unwrap_err()
    };
    let forward = publish([earlier.clone(), later.clone()]);
    let reverse = publish([later, earlier]);

    assert_eq!(forward, reverse);
    assert!(matches!(
        forward.issues(),
        [ResourceRegistryIssue::DuplicateType { first, second, .. }]
            if first.source().as_str() == "a/source.arcw"
                && second.source().as_str() == "z/source.arcw"
    ));
}

#[test]
fn duplicate_schema_and_codec_failures_are_input_order_independent() {
    let shared_schema_id = schema_id("example.duplicate_shape");
    let record = ResourceValueSchema::Record(ResourceRecordSchema::new(
        shared_schema_id.clone(),
        nominal("DuplicateShape"),
        schema_version(1),
        [],
    ));
    let enumeration = ResourceValueSchema::Enum(ResourceEnumSchema::new(
        shared_schema_id.clone(),
        nominal("DuplicateShape"),
        schema_version(1),
        [ResourceVariantDescriptor::unit(
            variant_id(1),
            variant_name("Only"),
        )],
    ));
    let shared_codec_id = codec_id("example.duplicate_shape");
    let codec_v1 = codec(&shared_codec_id, [1]);
    let codec_v2 = codec(&shared_codec_id, [2]);
    let resource = descriptor(
        "DuplicateShape",
        shared_schema_id.as_str(),
        "duplicate_shape",
        "example.duplicate_shape",
        &shared_codec_id,
        "duplicate-shape.arcw",
    );

    let publish = |schemas, codecs| {
        ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
            RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
            schemas,
            [resource.clone()],
            codecs,
        ))
        .unwrap_err()
    };
    let forward = publish(
        [record.clone(), enumeration.clone()],
        [codec_v1.clone(), codec_v2.clone()],
    );
    let reverse = publish([enumeration, record], [codec_v2, codec_v1]);

    assert_eq!(forward, reverse);
    assert!(
        forward
            .issues()
            .iter()
            .any(|issue| matches!(issue, ResourceRegistryIssue::DuplicateSchema { .. }))
    );
    assert!(
        forward
            .issues()
            .iter()
            .any(|issue| matches!(issue, ResourceRegistryIssue::DuplicateCodec { .. }))
    );
}

#[test]
fn same_kind_duplicate_schema_failures_are_input_order_independent() {
    let shared_schema_id = schema_id("example.duplicate_record");
    let duplicate_ids = ResourceValueSchema::Record(ResourceRecordSchema::new(
        shared_schema_id.clone(),
        nominal("DuplicateRecord"),
        schema_version(1),
        [
            ResourceFieldDescriptor::optional(
                field_id(1),
                field_name("alpha"),
                ResourceValueType::Scalar(ResourceScalarType::Bool),
            ),
            ResourceFieldDescriptor::optional(
                field_id(1),
                field_name("beta"),
                ResourceValueType::Scalar(ResourceScalarType::String),
            ),
        ],
    ));
    let duplicate_names = ResourceValueSchema::Record(ResourceRecordSchema::new(
        shared_schema_id,
        nominal("DuplicateRecord"),
        schema_version(1),
        [
            ResourceFieldDescriptor::optional(
                field_id(2),
                field_name("gamma"),
                ResourceValueType::Scalar(ResourceScalarType::Bool),
            ),
            ResourceFieldDescriptor::optional(
                field_id(3),
                field_name("gamma"),
                ResourceValueType::Scalar(ResourceScalarType::String),
            ),
        ],
    ));

    let publish = |schemas| {
        ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
            RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
            schemas,
            [],
            [],
        ))
        .unwrap_err()
    };
    let forward = publish([duplicate_ids.clone(), duplicate_names.clone()]);
    let reverse = publish([duplicate_names, duplicate_ids]);

    assert_eq!(forward, reverse);
    assert!(forward.issues().iter().any(|issue| matches!(
        issue,
        ResourceRegistryIssue::DuplicateFieldId { field, .. } if field.get() == 1
    )));
    assert!(forward.issues().iter().any(|issue| matches!(
        issue,
        ResourceRegistryIssue::DuplicateFieldName { field, .. }
            if field.as_str() == "gamma"
    )));
}

#[test]
fn local_schema_invariants_report_typed_issues_without_string_coercion() {
    let codec_id = codec_id("example.invalid-schema");
    let schema = record_schema(
        "InvalidSchema",
        "example.invalid_schema",
        [
            ResourceFieldDescriptor::required(
                field_id(1),
                field_name("first"),
                ResourceValueType::Scalar(ResourceScalarType::Bool),
            )
            .with_default(ResourceConstValue::Scalar(ResourceScalarValue::Bool(true))),
            ResourceFieldDescriptor::optional(
                field_id(1),
                field_name("second"),
                ResourceValueType::Scalar(ResourceScalarType::Bool),
            ),
            ResourceFieldDescriptor::optional(
                field_id(2),
                field_name("first"),
                ResourceValueType::Scalar(ResourceScalarType::Bool),
            ),
            ResourceFieldDescriptor::optional(
                field_id(3),
                field_name("third"),
                ResourceValueType::Scalar(ResourceScalarType::Bool),
            )
            .with_default(ResourceConstValue::Scalar(ResourceScalarValue::String(
                "not a bool".into(),
            ))),
        ],
    );
    let error = ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
        RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
        [schema],
        [descriptor(
            "InvalidSchema",
            "example.invalid_schema",
            "invalid",
            "example.invalid",
            &codec_id,
            "invalid.arcw",
        )],
        [codec(&codec_id, [1])],
    ))
    .unwrap_err();

    assert!(error.issues().iter().any(|issue| matches!(
        issue,
        ResourceRegistryIssue::DuplicateFieldId { field, .. } if field.get() == 1
    )));
    assert!(error.issues().iter().any(|issue| matches!(
        issue,
        ResourceRegistryIssue::DuplicateFieldName { field, .. }
            if field.as_str() == "first"
    )));
    assert!(error.issues().iter().any(|issue| matches!(
        issue,
        ResourceRegistryIssue::RequiredFieldHasDefault { field, .. } if field.get() == 1
    )));
    assert!(error.issues().iter().any(|issue| matches!(
        issue,
        ResourceRegistryIssue::InvalidFieldDefault { field, .. } if field.get() == 3
    )));
}

#[test]
fn malformed_field_order_does_not_change_registry_diagnostics() {
    let mut fields = vec![
        ResourceFieldDescriptor::required(
            field_id(1),
            field_name("zeta"),
            ResourceValueType::Scalar(ResourceScalarType::Bool),
        ),
        ResourceFieldDescriptor::required(
            field_id(1),
            field_name("alpha"),
            ResourceValueType::Scalar(ResourceScalarType::String),
        ),
        ResourceFieldDescriptor::required(
            field_id(2),
            field_name("zeta"),
            ResourceValueType::Scalar(ResourceScalarType::Bool),
        ),
    ];
    let codec_id = codec_id("example.field_order");
    let publish = |fields| {
        ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
            RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
            [record_schema("FieldOrder", "example.field_order", fields)],
            [descriptor(
                "FieldOrder",
                "example.field_order",
                "field_order",
                "example.field_order",
                &codec_id,
                "field-order.arcw",
            )],
            [codec(&codec_id, [1])],
        ))
        .unwrap_err()
    };
    let forward = publish(fields.clone());
    fields.reverse();
    let reverse = publish(fields);

    assert_eq!(forward, reverse);
}

#[test]
fn nested_record_and_enum_defaults_are_validated_against_published_schemas() {
    let child_id = schema_id("example.child");
    let enum_id = schema_id("example.mode");
    let parent_id = schema_id("example.parent");
    let child = ResourceValueSchema::Record(ResourceRecordSchema::new(
        child_id.clone(),
        nominal("Child"),
        schema_version(1),
        [ResourceFieldDescriptor::required(
            field_id(1),
            field_name("flag"),
            ResourceValueType::Scalar(ResourceScalarType::Bool),
        )],
    ));
    let mode = ResourceValueSchema::Enum(ResourceEnumSchema::new(
        enum_id.clone(),
        nominal("Mode"),
        schema_version(1),
        [ResourceVariantDescriptor::with_payload(
            variant_id(1),
            variant_name("Enabled"),
            ResourceValueType::Scalar(ResourceScalarType::Bool),
        )],
    ));
    let parent = ResourceValueSchema::Record(ResourceRecordSchema::new(
        parent_id.clone(),
        nominal("Parent"),
        schema_version(1),
        [
            ResourceFieldDescriptor::optional(
                field_id(1),
                field_name("child"),
                ResourceValueType::NominalRecord(child_id.clone()),
            )
            .with_default(ResourceConstValue::Record(
                ResourceRecordValue::try_new(child_id, []).unwrap(),
            )),
            ResourceFieldDescriptor::optional(
                field_id(2),
                field_name("mode"),
                ResourceValueType::NominalEnum(enum_id.clone()),
            )
            .with_default(ResourceConstValue::Enum(ResourceEnumValue::new(
                enum_id,
                variant_id(1),
                None,
            ))),
        ],
    ));
    let codec_id = codec_id("example.parent");
    let error = ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
        RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
        [parent, mode, child],
        [descriptor(
            "Parent",
            parent_id.as_str(),
            "parent",
            "example.parent",
            &codec_id,
            "parent.arcw",
        )],
        [codec(&codec_id, [1])],
    ))
    .unwrap_err();

    assert!(error.issues().iter().any(|issue| matches!(
        issue,
        ResourceRegistryIssue::InvalidFieldDefault {
            field,
            source: ResourceDefaultValidationError::MissingRecordField { .. },
            ..
        } if field.get() == 1
    )));
    assert!(error.issues().iter().any(|issue| matches!(
        issue,
        ResourceRegistryIssue::InvalidFieldDefault {
            field,
            source: ResourceDefaultValidationError::EnumPayloadPresence,
            ..
        } if field.get() == 2
    )));
}

#[test]
fn deeply_nested_defaults_fail_with_the_shared_budget_error() {
    let mut value_type = ResourceValueType::Scalar(ResourceScalarType::Bool);
    let mut value = ResourceConstValue::Scalar(ResourceScalarValue::Bool(true));
    for _ in 0..=64 {
        value_type = ResourceValueType::option(value_type);
        value = ResourceConstValue::Option(Some(Box::new(value)));
    }
    let schema_id = schema_id("example.deep_default");
    let schema = ResourceValueSchema::Record(ResourceRecordSchema::new(
        schema_id.clone(),
        nominal("DeepDefault"),
        schema_version(1),
        [
            ResourceFieldDescriptor::optional(field_id(1), field_name("value"), value_type)
                .with_default(value),
        ],
    ));
    let codec_id = codec_id("example.deep_default");

    let error = ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
        RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
        [schema],
        [descriptor(
            "DeepDefault",
            schema_id.as_str(),
            "deep_default",
            "example.deep_default",
            &codec_id,
            "deep-default.arcw",
        )],
        [codec(&codec_id, [1])],
    ))
    .unwrap_err();

    let mut source = error
        .issues()
        .iter()
        .find_map(|issue| match issue {
            ResourceRegistryIssue::InvalidFieldDefault { source, .. } => Some(source),
            _ => None,
        })
        .expect("deep default must produce a typed default issue");
    while let ResourceDefaultValidationError::Nested { source: nested, .. } = source {
        source = nested;
    }
    assert_eq!(source, &ResourceDefaultValidationError::NestingTooDeep);
}

#[test]
fn resource_refs_require_registered_types_but_asset_refs_require_only_payload_kind() {
    let codec_id = codec_id("example.refs");
    let missing_type = resource_type("Missing");
    let resource_schema = record_schema(
        "UsesResource",
        "example.uses_resource",
        [ResourceFieldDescriptor::required(
            field_id(1),
            field_name("target"),
            ResourceValueType::option(ResourceValueType::ResourceRef {
                type_id: missing_type.clone(),
            }),
        )],
    );
    let resource_error = ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
        RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
        [resource_schema],
        [descriptor(
            "UsesResource",
            "example.uses_resource",
            "uses_resource",
            "example.uses_resource",
            &codec_id,
            "resource-ref.arcw",
        )],
        [codec(&codec_id, [1])],
    ))
    .unwrap_err();
    assert!(resource_error.issues().iter().any(|issue| matches!(
        issue,
        ResourceRegistryIssue::UnknownResourceReferenceType { target, path, .. }
            if target == &missing_type
                && path.segments()
                    == [ResourceValueTypePathSegment::OptionValue]
    )));

    let asset_schema = record_schema(
        "UsesAsset",
        "example.uses_asset",
        [ResourceFieldDescriptor::required(
            field_id(1),
            field_name("asset"),
            ResourceValueType::AssetRef {
                payload_kind: payload_kind("example.weather.payload"),
            },
        )],
    );
    let asset_registry = ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
        RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
        [asset_schema],
        [descriptor(
            "UsesAsset",
            "example.uses_asset",
            "uses_asset",
            "example.uses_asset",
            &codec_id,
            "asset-ref.arcw",
        )],
        [codec(&codec_id, [1])],
    ))
    .unwrap();
    asset_registry.verify_integrity().unwrap();
}

#[test]
fn retained_references_are_validated_recursively_without_a_field_name_switch() {
    let codec_id = codec_id("example.retained");
    let retained_type = ResourceValueType::option(ResourceValueType::vec(
        ResourceValueType::RetainedIdentityRef {
            identity: RetainedIdentityKind::Action,
        },
    ));
    let action = ResourceConstValue::RetainedIdentityRef {
        value: ResolvedRetainedIdentityRef::Action {
            entity_id: arcweft_id::EntityId::try_new("entity.action.submit").unwrap(),
        },
    };
    let schema = record_schema(
        "RetainedHolder",
        "example.retained_holder",
        [ResourceFieldDescriptor::optional(
            field_id(1),
            field_name("arbitrary_nested_field"),
            retained_type,
        )
        .with_default(ResourceConstValue::Option(Some(Box::new(
            ResourceConstValue::Sequence(vec![action]),
        ))))],
    );

    let registry = ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
        RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
        [schema],
        [descriptor(
            "RetainedHolder",
            "example.retained_holder",
            "retained",
            "example.retained",
            &codec_id,
            "retained.arcw",
        )],
        [codec(&codec_id, [1])],
    ))
    .unwrap();

    registry.verify_integrity().unwrap();
}

#[test]
fn retained_reference_defaults_reject_the_wrong_exact_kind_at_the_nested_path() {
    let codec_id = codec_id("example.retained");
    let schema = record_schema(
        "RetainedHolder",
        "example.retained_holder",
        [ResourceFieldDescriptor::optional(
            field_id(1),
            field_name("target"),
            ResourceValueType::option(ResourceValueType::RetainedIdentityRef {
                identity: RetainedIdentityKind::View,
            }),
        )
        .with_default(ResourceConstValue::Option(Some(Box::new(
            ResourceConstValue::RetainedIdentityRef {
                value: ResolvedRetainedIdentityRef::Character {
                    entity_id: arcweft_id::EntityId::try_new("entity.character.alice").unwrap(),
                },
            },
        ))))],
    );

    let error = ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
        RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
        [schema],
        [descriptor(
            "RetainedHolder",
            "example.retained_holder",
            "retained",
            "example.retained",
            &codec_id,
            "retained.arcw",
        )],
        [codec(&codec_id, [1])],
    ))
    .unwrap_err();

    let issue = error
        .issues()
        .iter()
        .find_map(|issue| match issue {
            ResourceRegistryIssue::InvalidFieldDefault { source, .. } => Some(source),
            _ => None,
        })
        .expect("wrong retained kind must reject the default");
    let ResourceDefaultValidationError::Nested { source, .. } = issue else {
        panic!("optional value must retain its nested path, got {issue:?}");
    };
    assert!(matches!(
        source.as_ref(),
        ResourceDefaultValidationError::Structural(
            ResourceValueValidationError::RetainedIdentityKindMismatch {
                expected: RetainedIdentityKind::View,
                actual: RetainedIdentityKind::Character,
            }
        )
    ));
}

#[test]
fn retained_target_identity_participates_in_the_canonical_registry_digest() {
    let codec_id = codec_id("example.retained");
    let registry = |entity: &str| {
        let schema = record_schema(
            "RetainedHolder",
            "example.retained_holder",
            [ResourceFieldDescriptor::optional(
                field_id(1),
                field_name("character"),
                ResourceValueType::option(ResourceValueType::RetainedIdentityRef {
                    identity: RetainedIdentityKind::Character,
                }),
            )
            .with_default(ResourceConstValue::Option(Some(Box::new(
                ResourceConstValue::RetainedIdentityRef {
                    value: ResolvedRetainedIdentityRef::Character {
                        entity_id: arcweft_id::EntityId::try_new(entity).unwrap(),
                    },
                },
            ))))],
        );
        ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
            RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
            [schema],
            [descriptor(
                "RetainedHolder",
                "example.retained_holder",
                "retained",
                "example.retained",
                &codec_id,
                "retained.arcw",
            )],
            [codec(&codec_id, [1])],
        ))
        .unwrap()
    };

    let alice = registry("entity.character.alice");
    let bob = registry("entity.character.bob");
    assert_ne!(alice.digest(), bob.digest());
    assert_eq!(
        alice.digest(),
        registry("entity.character.alice").digest(),
        "canonical identity, not construction history, owns the digest"
    );
}

#[test]
fn family_codec_and_capability_invariants_are_typed() {
    let shared_codec_id = codec_id("example.shared");
    let first_schema = record_schema("First", "example.first", []);
    let second_schema = record_schema("Second", "example.second", []);
    let family_error = ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
        RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
        [first_schema, second_schema],
        [
            descriptor(
                "First",
                "example.first",
                "shared",
                "example.group.first",
                &shared_codec_id,
                "first.arcw",
            ),
            descriptor(
                "Second",
                "example.second",
                "shared",
                "example.group.second",
                &shared_codec_id,
                "second.arcw",
            ),
        ],
        [codec(&shared_codec_id, [1])],
    ))
    .unwrap_err();
    assert!(
        family_error
            .issues()
            .iter()
            .any(|issue| matches!(issue, ResourceRegistryIssue::FamilyCollision { .. }))
    );

    let missing_codec = codec_id("example.missing");
    let missing_codec_error = ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
        RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
        [record_schema("MissingCodec", "example.missing_codec", [])],
        [descriptor(
            "MissingCodec",
            "example.missing_codec",
            "missing",
            "example.missing",
            &missing_codec,
            "missing.arcw",
        )],
        [],
    ))
    .unwrap_err();
    assert!(missing_codec_error.issues().iter().any(|issue| matches!(
        issue,
        ResourceRegistryIssue::MissingCodec { codec, .. } if codec == &missing_codec
    )));

    let invalid_capabilities = ResourceCapabilities::new(
        None,
        ResourceAgentExposure::CatalogAndRuntime,
        true,
        ResourceHotReloadClass::UpdateLiveHandle,
    );
    let invalid_schema = record_schema("InvalidCaps", "example.invalid_caps", []);
    let invalid_descriptor = descriptor_with_capabilities(
        "InvalidCaps",
        "example.invalid_caps",
        "invalid_caps",
        "example.invalid_caps",
        &shared_codec_id,
        "invalid-caps.arcw",
        invalid_capabilities,
    );
    let capability_error = ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
        RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
        [invalid_schema],
        [invalid_descriptor],
        [codec(&shared_codec_id, [1])],
    ))
    .unwrap_err();
    assert!(
        capability_error
            .issues()
            .iter()
            .any(|issue| matches!(issue, ResourceRegistryIssue::InvalidCapabilities { .. }))
    );
}

fn canonical_registry(codec_id: &ResourceCodecId, reversed: bool) -> ResourceTypeRegistry {
    let mut voice_fields = vec![
        ResourceFieldDescriptor::optional(
            field_id(2),
            field_name("label"),
            ResourceValueType::Scalar(ResourceScalarType::String),
        )
        .with_default(ResourceConstValue::Scalar(ResourceScalarValue::String(
            "default".into(),
        )))
        .with_docs(if reversed {
            "different field docs"
        } else {
            "first field docs"
        }),
        ResourceFieldDescriptor::required(
            field_id(1),
            field_name("asset"),
            ResourceValueType::AssetRef {
                payload_kind: payload_kind("std.audio.payload"),
            },
        ),
    ];
    let voice_profile = record_schema(
        "VoiceProfile",
        "example.voice_profile",
        [ResourceFieldDescriptor::required(
            field_id(1),
            field_name("enabled"),
            ResourceValueType::Scalar(ResourceScalarType::Bool),
        )],
    );
    let mut descriptors = vec![
        descriptor(
            "Voice",
            "example.voice",
            "voice",
            "example.audio.voice",
            codec_id,
            if reversed {
                "different/voice.arcw"
            } else {
                "z/source.arcw"
            },
        ),
        descriptor(
            "VoiceProfile",
            "example.voice_profile",
            "voice",
            "example.audio.voice",
            codec_id,
            if reversed {
                "different/profile.arcw"
            } else {
                "a/source.arcw"
            },
        ),
    ];
    if reversed {
        voice_fields.reverse();
        descriptors.reverse();
    }
    let voice = record_schema("Voice", "example.voice", voice_fields);
    let schemas = if reversed {
        vec![voice_profile, voice]
    } else {
        vec![voice, voice_profile]
    };

    ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
        RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
        schemas,
        descriptors,
        [codec(codec_id, [1])],
    ))
    .unwrap()
}

fn descriptor(
    name: &str,
    schema: &str,
    family: &str,
    family_group: &str,
    codec_id: &ResourceCodecId,
    source: &str,
) -> ResourceTypeDescriptor {
    descriptor_with_capabilities(
        name,
        schema,
        family,
        family_group,
        codec_id,
        source,
        ResourceCapabilities::definition_only(),
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "the test builder keeps every independently varied registry invariant explicit"
)]
fn descriptor_with_capabilities(
    name: &str,
    schema: &str,
    family: &str,
    family_group: &str,
    codec_id: &ResourceCodecId,
    source: &str,
    capabilities: ResourceCapabilities,
) -> ResourceTypeDescriptor {
    ResourceTypeDescriptor::new(
        ResourceDescriptorProvenance::new(
            package(),
            ResourceDescriptorSourceId::try_new(source).unwrap(),
        ),
        resource_type(name),
        ResourcePublicIdFamily::try_new(family).unwrap(),
        ResourceFamilyGroupId::try_new(family_group).unwrap(),
        schema_id(schema),
        capabilities,
        ResourceLoweringBinding::new(
            codec_id.clone(),
            codec_version(1),
            ResourceBundleSectionId::try_new("extension.resources").unwrap(),
            ResourceBundleSectionVersion::try_new(1).unwrap(),
        ),
        ResourceTypeDocs::new(format!("{name} docs")),
    )
}

fn record_schema(
    name: &str,
    schema: &str,
    fields: impl IntoIterator<Item = ResourceFieldDescriptor>,
) -> ResourceValueSchema {
    ResourceValueSchema::Record(ResourceRecordSchema::new(
        schema_id(schema),
        nominal(name),
        schema_version(1),
        fields,
    ))
}

fn codec(
    codec_id: &ResourceCodecId,
    versions: impl IntoIterator<Item = u32>,
) -> ResourceCodecSupport {
    ResourceCodecSupport::new(codec_id.clone(), versions.into_iter().map(codec_version))
}

fn package() -> PackageId {
    PackageId::new("com.example.resources").unwrap()
}

fn nominal(name: &str) -> NominalTypeId {
    NominalTypeId::new(
        package(),
        ResourceModulePath::try_new("extensions").unwrap(),
        ResourceTypeName::try_new(name).unwrap(),
    )
}

fn resource_type(name: &str) -> ResourceTypeId {
    ResourceTypeId::new(nominal(name))
}

fn schema_id(value: &str) -> ResourceSchemaId {
    ResourceSchemaId::try_new(value).unwrap()
}

fn field_id(value: u32) -> ResourceFieldId {
    ResourceFieldId::try_new(value).unwrap()
}

fn field_name(value: &str) -> ResourceFieldName {
    ResourceFieldName::try_new(value).unwrap()
}

fn variant_id(value: u32) -> ResourceVariantId {
    ResourceVariantId::try_new(value).unwrap()
}

fn variant_name(value: &str) -> ResourceVariantName {
    ResourceVariantName::try_new(value).unwrap()
}

fn schema_version(value: u32) -> ResourceSchemaVersion {
    ResourceSchemaVersion::try_new(value).unwrap()
}

fn codec_id(value: &str) -> ResourceCodecId {
    ResourceCodecId::try_new(value).unwrap()
}

fn codec_version(value: u32) -> ResourceCodecVersion {
    ResourceCodecVersion::try_new(value).unwrap()
}

fn payload_kind(value: &str) -> ResourceAssetPayloadKindId {
    ResourceAssetPayloadKindId::try_new(value).unwrap()
}
