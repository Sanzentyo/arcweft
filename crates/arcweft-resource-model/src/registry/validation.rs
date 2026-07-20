use super::{
    RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION, ResourceDefaultValidationError, ResourceRegistryIssue,
    ResourceRegistryPublication, ResourceRegistryPublicationError,
};
use crate::descriptor::{
    ResourceCodecSupport, ResourceDescriptorProvenance, ResourceFieldPresence,
    ResourceTypeDescriptor, ResourceValueSchema, ResourceValueSchemaKind,
};
use crate::identity::{
    NominalTypeId, ResourceCodecId, ResourceFamilyGroupId, ResourcePublicIdFamily,
    ResourceSchemaId, ResourceTypeId,
};
use crate::value::{
    MAX_RESOURCE_VALUE_NESTING, ResourceConstValue, ResourceEnumValue, ResourceMapValue,
    ResourceRecordValue, ResourceReferenceRequirementKind, ResourceValidationPathSegment,
    ResourceValueType,
};
use core::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct ValidatedRegistryParts {
    pub(super) schemas: BTreeMap<ResourceSchemaId, ResourceValueSchema>,
    pub(super) resource_types: BTreeMap<ResourceTypeId, ResourceTypeDescriptor>,
    pub(super) codecs: BTreeMap<ResourceCodecId, ResourceCodecSupport>,
}

pub(super) fn validate_and_normalize(
    mut publication: ResourceRegistryPublication,
) -> Result<ValidatedRegistryParts, ResourceRegistryPublicationError> {
    let mut issues = Vec::new();
    if publication.manifest_schema_version != RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION {
        issues.push(ResourceRegistryIssue::UnsupportedManifestSchemaVersion {
            expected: RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
            actual: publication.manifest_schema_version,
        });
    }

    for schema in &mut publication.schemas {
        schema.canonicalize();
    }
    publication.schemas.sort_by(|left, right| {
        left.id()
            .cmp(right.id())
            .then_with(|| left.nominal_type().cmp(right.nominal_type()))
            .then_with(|| left.kind().cmp(&right.kind()))
            .then_with(|| left.version().cmp(&right.version()))
            .then_with(|| compare_local_schema_shape(left, right))
    });
    let schemas = collect_schemas(publication.schemas, &mut issues);

    publication.codecs.sort_by(|left, right| {
        left.codec_id()
            .cmp(right.codec_id())
            .then_with(|| left.versions().cmp(right.versions()))
    });
    let codecs = collect_codecs(publication.codecs, &mut issues);

    publication.resource_types.sort_by(|left, right| {
        left.type_id()
            .cmp(right.type_id())
            .then_with(|| left.provenance().cmp(right.provenance()))
            .then_with(|| left.cmp(right))
    });
    let resource_types = collect_resource_types(publication.resource_types, &mut issues);

    validate_schema_references(&schemas, &resource_types, &mut issues);
    validate_descriptors(&schemas, &resource_types, &codecs, &mut issues);

    if issues.is_empty() {
        Ok(ValidatedRegistryParts {
            schemas,
            resource_types,
            codecs,
        })
    } else {
        Err(ResourceRegistryPublicationError::new(issues))
    }
}

fn compare_local_schema_shape(left: &ResourceValueSchema, right: &ResourceValueSchema) -> Ordering {
    match (left, right) {
        (ResourceValueSchema::Record(left), ResourceValueSchema::Record(right)) => {
            for (left, right) in left.fields().iter().zip(right.fields()) {
                let ordering = left
                    .id()
                    .cmp(&right.id())
                    .then_with(|| left.name().cmp(right.name()))
                    .then_with(|| left.presence().cmp(&right.presence()))
                    .then_with(|| left.default().is_some().cmp(&right.default().is_some()));
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            left.fields().len().cmp(&right.fields().len())
        }
        (ResourceValueSchema::Enum(left), ResourceValueSchema::Enum(right)) => {
            for (left, right) in left.variants().iter().zip(right.variants()) {
                let ordering = left
                    .id()
                    .cmp(&right.id())
                    .then_with(|| left.name().cmp(right.name()))
                    .then_with(|| left.payload().is_some().cmp(&right.payload().is_some()));
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            left.variants().len().cmp(&right.variants().len())
        }
        (ResourceValueSchema::Record(_), ResourceValueSchema::Enum(_)) => Ordering::Less,
        (ResourceValueSchema::Enum(_), ResourceValueSchema::Record(_)) => Ordering::Greater,
    }
}

fn collect_schemas(
    schemas: Vec<ResourceValueSchema>,
    issues: &mut Vec<ResourceRegistryIssue>,
) -> BTreeMap<ResourceSchemaId, ResourceValueSchema> {
    let mut by_id = BTreeMap::<ResourceSchemaId, ResourceValueSchema>::new();
    let mut by_nominal = BTreeMap::<NominalTypeId, ResourceSchemaId>::new();
    let mut duplicate_ids = BTreeSet::new();
    for schema in schemas {
        validate_local_schema(&schema, issues);
        if duplicate_ids.contains(schema.id()) {
            issues.push(ResourceRegistryIssue::DuplicateSchema {
                schema: schema.id().clone(),
            });
            continue;
        }
        if let Some(first) = by_id.remove(schema.id()) {
            if by_nominal.get(first.nominal_type()) == Some(first.id()) {
                by_nominal.remove(first.nominal_type());
            }
            duplicate_ids.insert(schema.id().clone());
            issues.push(ResourceRegistryIssue::DuplicateSchema {
                schema: schema.id().clone(),
            });
            continue;
        }
        if let Some(first) = by_nominal.get(schema.nominal_type()) {
            issues.push(ResourceRegistryIssue::DuplicateNominalSchema {
                nominal_type: schema.nominal_type().clone(),
                first: first.clone(),
                second: schema.id().clone(),
            });
        } else {
            by_nominal.insert(schema.nominal_type().clone(), schema.id().clone());
        }
        by_id.insert(schema.id().clone(), schema);
    }
    by_id
}

fn validate_local_schema(schema: &ResourceValueSchema, issues: &mut Vec<ResourceRegistryIssue>) {
    match schema {
        ResourceValueSchema::Record(schema) => {
            let mut ids = BTreeSet::new();
            let mut names = BTreeSet::new();
            for field in schema.fields() {
                if !ids.insert(field.id()) {
                    issues.push(ResourceRegistryIssue::DuplicateFieldId {
                        schema: schema.id().clone(),
                        field: field.id(),
                    });
                }
                if !names.insert(field.name().clone()) {
                    issues.push(ResourceRegistryIssue::DuplicateFieldName {
                        schema: schema.id().clone(),
                        field: field.name().clone(),
                    });
                }
                if field.presence() == ResourceFieldPresence::Required && field.default().is_some()
                {
                    issues.push(ResourceRegistryIssue::RequiredFieldHasDefault {
                        schema: schema.id().clone(),
                        field: field.id(),
                    });
                }
            }
        }
        ResourceValueSchema::Enum(schema) => {
            let mut ids = BTreeSet::new();
            let mut names = BTreeSet::new();
            for variant in schema.variants() {
                if !ids.insert(variant.id()) {
                    issues.push(ResourceRegistryIssue::DuplicateVariantId {
                        schema: schema.id().clone(),
                        variant: variant.id(),
                    });
                }
                if !names.insert(variant.name().clone()) {
                    issues.push(ResourceRegistryIssue::DuplicateVariantName {
                        schema: schema.id().clone(),
                        variant: variant.name().clone(),
                    });
                }
            }
        }
    }
}

fn collect_codecs(
    codecs: Vec<ResourceCodecSupport>,
    issues: &mut Vec<ResourceRegistryIssue>,
) -> BTreeMap<ResourceCodecId, ResourceCodecSupport> {
    let mut by_id = BTreeMap::new();
    let mut duplicate_ids = BTreeSet::new();
    for codec in codecs {
        let codec_id = codec.codec_id().clone();
        if codec.versions().is_empty() {
            issues.push(ResourceRegistryIssue::CodecWithoutVersions {
                codec: codec_id.clone(),
            });
        }
        if duplicate_ids.contains(&codec_id) {
            issues.push(ResourceRegistryIssue::DuplicateCodec { codec: codec_id });
            continue;
        }
        if by_id.remove(&codec_id).is_some() {
            duplicate_ids.insert(codec_id.clone());
            issues.push(ResourceRegistryIssue::DuplicateCodec { codec: codec_id });
            continue;
        }
        by_id.insert(codec_id, codec);
    }
    by_id
}

fn collect_resource_types(
    descriptors: Vec<ResourceTypeDescriptor>,
    issues: &mut Vec<ResourceRegistryIssue>,
) -> BTreeMap<ResourceTypeId, ResourceTypeDescriptor> {
    let mut by_id = BTreeMap::<ResourceTypeId, ResourceTypeDescriptor>::new();
    let mut first_duplicate = BTreeMap::<ResourceTypeId, ResourceDescriptorProvenance>::new();
    for descriptor in descriptors {
        if let Some(first) = first_duplicate.get(descriptor.type_id()) {
            issues.push(ResourceRegistryIssue::DuplicateType {
                type_id: descriptor.type_id().clone(),
                first: first.clone(),
                second: descriptor.provenance().clone(),
            });
            continue;
        }
        if let Some(first) = by_id.remove(descriptor.type_id()) {
            let first_provenance = first.provenance().clone();
            first_duplicate.insert(descriptor.type_id().clone(), first_provenance.clone());
            issues.push(ResourceRegistryIssue::DuplicateType {
                type_id: descriptor.type_id().clone(),
                first: first_provenance,
                second: descriptor.provenance().clone(),
            });
            continue;
        }
        by_id.insert(descriptor.type_id().clone(), descriptor);
    }
    by_id
}

fn validate_schema_references(
    schemas: &BTreeMap<ResourceSchemaId, ResourceValueSchema>,
    resource_types: &BTreeMap<ResourceTypeId, ResourceTypeDescriptor>,
    issues: &mut Vec<ResourceRegistryIssue>,
) {
    for (schema_id, schema) in schemas {
        match schema {
            ResourceValueSchema::Record(schema) => {
                for field in schema.fields() {
                    validate_value_type(
                        schema_id,
                        field.value_type(),
                        schemas,
                        resource_types,
                        issues,
                    );
                    if let Some(default) = field.default()
                        && let Err(source) =
                            validate_default(field.value_type(), default, schemas, 0)
                    {
                        issues.push(ResourceRegistryIssue::InvalidFieldDefault {
                            schema: schema_id.clone(),
                            field: field.id(),
                            source,
                        });
                    }
                }
            }
            ResourceValueSchema::Enum(schema) => {
                for variant in schema.variants() {
                    if let Some(payload) = variant.payload() {
                        validate_value_type(schema_id, payload, schemas, resource_types, issues);
                    }
                }
            }
        }
    }
}

fn validate_value_type(
    owner: &ResourceSchemaId,
    value_type: &ResourceValueType,
    schemas: &BTreeMap<ResourceSchemaId, ResourceValueSchema>,
    resource_types: &BTreeMap<ResourceTypeId, ResourceTypeDescriptor>,
    issues: &mut Vec<ResourceRegistryIssue>,
) {
    let Ok(requirements) = value_type.reference_requirements() else {
        issues.push(ResourceRegistryIssue::ValueTypeNestingTooDeep {
            owner: owner.clone(),
        });
        return;
    };
    for requirement in requirements {
        match requirement.kind() {
            ResourceReferenceRequirementKind::NominalRecord { schema_id } => {
                validate_schema_kind(
                    owner,
                    schema_id,
                    ResourceValueSchemaKind::Record,
                    requirement.path(),
                    schemas,
                    issues,
                );
            }
            ResourceReferenceRequirementKind::NominalEnum { schema_id } => {
                validate_schema_kind(
                    owner,
                    schema_id,
                    ResourceValueSchemaKind::Enum,
                    requirement.path(),
                    schemas,
                    issues,
                );
            }
            ResourceReferenceRequirementKind::Resource { type_id }
                if !resource_types.contains_key(type_id) =>
            {
                issues.push(ResourceRegistryIssue::UnknownResourceReferenceType {
                    owner: owner.clone(),
                    target: type_id.clone(),
                    path: requirement.path().clone(),
                });
            }
            ResourceReferenceRequirementKind::Asset { .. }
            | ResourceReferenceRequirementKind::Resource { .. }
            | ResourceReferenceRequirementKind::Retained { .. } => {}
        }
    }
}

fn validate_schema_kind(
    owner: &ResourceSchemaId,
    target: &ResourceSchemaId,
    expected: ResourceValueSchemaKind,
    path: &crate::value::ResourceValueTypePath,
    schemas: &BTreeMap<ResourceSchemaId, ResourceValueSchema>,
    issues: &mut Vec<ResourceRegistryIssue>,
) {
    match schemas.get(target) {
        None => issues.push(ResourceRegistryIssue::UnknownValueSchema {
            owner: owner.clone(),
            target: target.clone(),
            path: path.clone(),
        }),
        Some(schema) if schema.kind() != expected => {
            issues.push(ResourceRegistryIssue::ValueSchemaKindMismatch {
                owner: owner.clone(),
                target: target.clone(),
                path: path.clone(),
                expected,
                actual: schema.kind(),
            });
        }
        Some(_) => {}
    }
}

fn validate_descriptors(
    schemas: &BTreeMap<ResourceSchemaId, ResourceValueSchema>,
    resource_types: &BTreeMap<ResourceTypeId, ResourceTypeDescriptor>,
    codecs: &BTreeMap<ResourceCodecId, ResourceCodecSupport>,
    issues: &mut Vec<ResourceRegistryIssue>,
) {
    let mut families =
        BTreeMap::<ResourcePublicIdFamily, (ResourceFamilyGroupId, ResourceTypeId)>::new();
    for descriptor in resource_types.values() {
        let type_id = descriptor.type_id();
        if descriptor.provenance().package() != type_id.nominal().package() {
            issues.push(ResourceRegistryIssue::ProvenancePackageMismatch {
                type_id: type_id.clone(),
                expected: descriptor.provenance().package().clone(),
                actual: type_id.nominal().package().clone(),
            });
        }
        match schemas.get(descriptor.body_schema()) {
            None => issues.push(ResourceRegistryIssue::UnknownBodySchema {
                type_id: type_id.clone(),
                schema: descriptor.body_schema().clone(),
            }),
            Some(schema) if schema.kind() != ResourceValueSchemaKind::Record => {
                issues.push(ResourceRegistryIssue::BodySchemaNotRecord {
                    type_id: type_id.clone(),
                    schema: descriptor.body_schema().clone(),
                });
            }
            Some(schema) if schema.nominal_type() != type_id.nominal() => {
                issues.push(ResourceRegistryIssue::BodySchemaNominalTypeMismatch {
                    type_id: type_id.clone(),
                    schema: descriptor.body_schema().clone(),
                    actual: schema.nominal_type().clone(),
                });
            }
            Some(_) => {}
        }
        if let Some((group, first_type)) = families.get(descriptor.public_id_family()) {
            if group != descriptor.family_group() {
                issues.push(ResourceRegistryIssue::FamilyCollision {
                    family: descriptor.public_id_family().clone(),
                    first_group: group.clone(),
                    first_type: first_type.clone(),
                    second_group: descriptor.family_group().clone(),
                    second_type: type_id.clone(),
                });
            }
        } else {
            families.insert(
                descriptor.public_id_family().clone(),
                (descriptor.family_group().clone(), type_id.clone()),
            );
        }
        let lowering = descriptor.lowering();
        match codecs.get(lowering.codec_id()) {
            None => issues.push(ResourceRegistryIssue::MissingCodec {
                type_id: type_id.clone(),
                codec: lowering.codec_id().clone(),
            }),
            Some(codec) if !codec.supports(lowering.codec_version()) => {
                issues.push(ResourceRegistryIssue::UnsupportedCodecVersion {
                    type_id: type_id.clone(),
                    codec: lowering.codec_id().clone(),
                    version: lowering.codec_version(),
                });
            }
            Some(_) => {}
        }
        if let Err(source) = descriptor.capabilities().validate() {
            issues.push(ResourceRegistryIssue::InvalidCapabilities {
                type_id: type_id.clone(),
                source,
            });
        }
    }
}

fn validate_default(
    value_type: &ResourceValueType,
    value: &ResourceConstValue,
    schemas: &BTreeMap<ResourceSchemaId, ResourceValueSchema>,
    depth: usize,
) -> Result<(), ResourceDefaultValidationError> {
    if depth > MAX_RESOURCE_VALUE_NESTING {
        return Err(ResourceDefaultValidationError::NestingTooDeep);
    }
    value_type.validate_const_shallow(value)?;
    match (value_type, value) {
        (ResourceValueType::Option(expected), ResourceConstValue::Option(Some(value))) => {
            validate_default(expected, value, schemas, depth + 1).map_err(|source| {
                ResourceDefaultValidationError::Nested {
                    segment: ResourceValidationPathSegment::OptionValue,
                    source: Box::new(source),
                }
            })
        }
        (
            ResourceValueType::Vec(expected) | ResourceValueType::NonEmptyVec(expected),
            ResourceConstValue::Sequence(values),
        ) => values.iter().enumerate().try_for_each(|(index, value)| {
            validate_default(expected, value, schemas, depth + 1).map_err(|source| {
                ResourceDefaultValidationError::Nested {
                    segment: ResourceValidationPathSegment::SequenceIndex(index),
                    source: Box::new(source),
                }
            })
        }),
        (
            ResourceValueType::Map {
                key: expected_key,
                value: expected_value,
            },
            ResourceConstValue::Map(values),
        ) => validate_map_default(expected_key, expected_value, values, schemas, depth),
        (ResourceValueType::NominalRecord(schema_id), ResourceConstValue::Record(value)) => {
            validate_record_default(schema_id, value, schemas, depth)
        }
        (ResourceValueType::NominalEnum(schema_id), ResourceConstValue::Enum(value)) => {
            validate_enum_default(schema_id, value, schemas, depth)
        }
        (
            ResourceValueType::Scalar(_)
            | ResourceValueType::ConstrainedScalar(_)
            | ResourceValueType::AssetRef { .. }
            | ResourceValueType::ResourceRef { .. }
            | ResourceValueType::RetainedIdentityRef { .. },
            _,
        )
        | (ResourceValueType::Option(_), ResourceConstValue::Option(None)) => Ok(()),
        _ => Err(ResourceDefaultValidationError::Structural(
            crate::value::ResourceValueValidationError::TypeMismatch {
                expected: value_type.clone(),
                actual: value.kind(),
            },
        )),
    }
}

fn validate_map_default(
    expected_key: &ResourceValueType,
    expected_value: &ResourceValueType,
    values: &ResourceMapValue,
    schemas: &BTreeMap<ResourceSchemaId, ResourceValueSchema>,
    depth: usize,
) -> Result<(), ResourceDefaultValidationError> {
    values
        .entries()
        .iter()
        .enumerate()
        .try_for_each(|(index, (key, value))| {
            validate_default(expected_key, key, schemas, depth + 1).map_err(|source| {
                ResourceDefaultValidationError::Nested {
                    segment: ResourceValidationPathSegment::MapKey(index),
                    source: Box::new(source),
                }
            })?;
            validate_default(expected_value, value, schemas, depth + 1).map_err(|source| {
                ResourceDefaultValidationError::Nested {
                    segment: ResourceValidationPathSegment::MapValue(index),
                    source: Box::new(source),
                }
            })
        })
}

fn validate_record_default(
    schema_id: &ResourceSchemaId,
    value: &ResourceRecordValue,
    schemas: &BTreeMap<ResourceSchemaId, ResourceValueSchema>,
    depth: usize,
) -> Result<(), ResourceDefaultValidationError> {
    let Some(ResourceValueSchema::Record(schema)) = schemas.get(schema_id) else {
        return Ok(());
    };
    let fields = schema
        .fields()
        .iter()
        .map(|field| (field.id(), field))
        .collect::<BTreeMap<_, _>>();
    for field in value.fields().keys() {
        if !fields.contains_key(field) {
            return Err(ResourceDefaultValidationError::UnknownRecordField { field: *field });
        }
    }
    for field in schema.fields() {
        match value.fields().get(&field.id()) {
            Some(value) => {
                validate_default(field.value_type(), value, schemas, depth + 1).map_err(
                    |source| ResourceDefaultValidationError::Nested {
                        segment: ResourceValidationPathSegment::RecordField(field.id()),
                        source: Box::new(source),
                    },
                )?;
            }
            None if field.presence() == ResourceFieldPresence::Required => {
                return Err(ResourceDefaultValidationError::MissingRecordField {
                    field: field.id(),
                });
            }
            None => {}
        }
    }
    Ok(())
}

fn validate_enum_default(
    schema_id: &ResourceSchemaId,
    value: &ResourceEnumValue,
    schemas: &BTreeMap<ResourceSchemaId, ResourceValueSchema>,
    depth: usize,
) -> Result<(), ResourceDefaultValidationError> {
    let Some(ResourceValueSchema::Enum(schema)) = schemas.get(schema_id) else {
        return Ok(());
    };
    let Some(variant) = schema
        .variants()
        .iter()
        .find(|variant| variant.id() == value.variant())
    else {
        return Err(ResourceDefaultValidationError::UnknownEnumVariant {
            variant: value.variant(),
        });
    };
    match (variant.payload(), value.payload()) {
        (None, None) => Ok(()),
        (Some(expected), Some(value)) => validate_default(expected, value, schemas, depth + 1)
            .map_err(|source| ResourceDefaultValidationError::Nested {
                segment: ResourceValidationPathSegment::EnumPayload,
                source: Box::new(source),
            }),
        (None, Some(_)) | (Some(_), None) => {
            Err(ResourceDefaultValidationError::EnumPayloadPresence)
        }
    }
}
