use crate::{
    JsonTokenRange, ResourceConstSourcePath, ResourceManifestDiagnostic,
    ResourceManifestDiagnosticCode, ResourceManifestPublicationLimits, ResourceManifestRelatedSpan,
    ResourceManifestReport, ResourceSchemaSource, ResourceTypeSource,
    SourceBackedResourceTypeManifestV1,
};
use arcweft_manifest_model::PackageId;
use arcweft_resource_model::registry::{
    RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION, ResourceDefaultValidationError, ResourceRegistryIssue,
    ResourceRegistryPublication, ResourceTypeRegistry, ResourceTypeRegistryDigest,
};
use arcweft_resource_model::{
    descriptor::ResourceValueSchema,
    identity::{ResourceCodecId, ResourceSchemaId, ResourceTypeId},
    value::{
        ResourceConstValue, ResourceValidationPathSegment, ResourceValueType,
        ResourceValueTypePath, ResourceValueTypePathSegment,
    },
};
use arcweft_source::SourceSpan;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedResourceTypeManifestSetV1 {
    manifests: Box<[SourceBackedResourceTypeManifestV1]>,
    registry: Arc<ResourceTypeRegistry>,
    registry_digest: ResourceTypeRegistryDigest,
}

pub fn publish_resource_type_manifests_v1(
    base: &ResourceTypeRegistry,
    manifests: impl IntoIterator<Item = SourceBackedResourceTypeManifestV1>,
    limits: ResourceManifestPublicationLimits,
) -> Result<PublishedResourceTypeManifestSetV1, ResourceManifestReport> {
    let mut manifests = manifests.into_iter().collect::<Vec<_>>();
    let Some(first_manifest) = manifests
        .iter()
        .min_by_key(|manifest| manifest.typed().package())
    else {
        return Ok(PublishedResourceTypeManifestSetV1 {
            manifests: Box::new([]),
            registry: Arc::new(base.clone()),
            registry_digest: base.digest(),
        });
    };
    let first_span = first_manifest.document().start_span();
    let work_units = publication_work_units(base, &manifests).unwrap_or(u64::MAX);
    if work_units > limits.work_units() {
        return Err(ResourceManifestReport::one(
            ResourceManifestDiagnostic::new(
                ResourceManifestDiagnosticCode::WorkLimit,
                format!(
                    "aggregate resource publication requires {work_units} work units; maximum is {}",
                    limits.work_units()
                ),
                first_span.clone(),
                [],
            ),
        ));
    }
    manifests.sort_by(|left, right| left.typed().package().cmp(right.typed().package()));
    let mut versions = BTreeMap::<PackageId, usize>::new();
    for (index, manifest) in manifests.iter().enumerate() {
        if let Some(first) = versions.insert(manifest.typed().package().id().clone(), index) {
            let first_manifest = &manifests[first];
            let code = if first_manifest.typed().package() == manifest.typed().package() {
                ResourceManifestDiagnosticCode::DuplicateRecord
            } else {
                ResourceManifestDiagnosticCode::VersionConflict
            };
            return Err(ResourceManifestReport::one(
                ResourceManifestDiagnostic::new(
                    code,
                    format!(
                        "package `{}` has more than one selected resource manifest",
                        manifest.typed().package().id()
                    ),
                    manifest.document().start_span(),
                    [crate::ResourceManifestRelatedSpan::new(
                        "first selected manifest",
                        first_manifest.document().start_span(),
                    )],
                ),
            ));
        }
    }
    let record_count = aggregate_record_count(base, &manifests).unwrap_or(usize::MAX);
    if record_count > limits.semantic_records() {
        return Err(ResourceManifestReport::one(
            ResourceManifestDiagnostic::new(
                ResourceManifestDiagnosticCode::RecordLimit,
                format!(
                    "aggregate resource publication has {record_count} records; maximum is {}",
                    limits.semantic_records()
                ),
                first_span.clone(),
                [],
            ),
        ));
    }
    let mut schemas = base
        .schemas()
        .map(|(_, value)| value.clone())
        .collect::<Vec<_>>();
    let mut types = base
        .types()
        .map(|(_, value)| value.clone())
        .collect::<Vec<_>>();
    let mut codecs = base
        .codecs()
        .map(|(_, value)| value.clone())
        .collect::<Vec<_>>();
    for manifest in &manifests {
        schemas.extend_from_slice(manifest.typed().schemas());
        types.extend_from_slice(manifest.typed().resource_types());
        codecs.extend_from_slice(manifest.typed().codecs());
    }
    let registry = ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
        RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
        schemas,
        types,
        codecs,
    ))
    .map_err(|error| registry_report(error.issues(), &manifests))?;
    let registry_digest = registry.digest();
    Ok(PublishedResourceTypeManifestSetV1 {
        manifests: manifests.into_boxed_slice(),
        registry: Arc::new(registry),
        registry_digest,
    })
}

fn aggregate_record_count(
    base: &ResourceTypeRegistry,
    manifests: &[SourceBackedResourceTypeManifestV1],
) -> Option<usize> {
    let mut count = base
        .schemas()
        .len()
        .checked_add(base.types().len())?
        .checked_add(base.codecs().len())?;
    for manifest in manifests {
        count = count
            .checked_add(manifest.typed().schemas().len())?
            .checked_add(manifest.typed().resource_types().len())?
            .checked_add(manifest.typed().codecs().len())?;
    }
    Some(count)
}

fn publication_work_units(
    base: &ResourceTypeRegistry,
    manifests: &[SourceBackedResourceTypeManifestV1],
) -> Option<u64> {
    let schema_count = manifests
        .iter()
        .try_fold(base.schemas().len(), |count, manifest| {
            count.checked_add(manifest.typed().schemas().len())
        })?;
    let type_count = manifests
        .iter()
        .try_fold(base.types().len(), |count, manifest| {
            count.checked_add(manifest.typed().resource_types().len())
        })?;
    let codec_count = manifests
        .iter()
        .try_fold(base.codecs().len(), |count, manifest| {
            count.checked_add(manifest.typed().codecs().len())
        })?;
    let mut work = 0;
    charge(&mut work, sort_work(manifests.len())?)?;
    charge(&mut work, u64::try_from(manifests.len()).ok()?)?;
    charge(&mut work, sort_work(schema_count)?)?;
    charge(&mut work, sort_work(type_count)?)?;
    charge(&mut work, sort_work(codec_count)?)?;
    charge(
        &mut work,
        u64::try_from(
            schema_count
                .checked_add(type_count)?
                .checked_add(codec_count)?,
        )
        .ok()?,
    )?;
    for (_, schema) in base.schemas() {
        charge_schema_work(&mut work, schema)?;
    }
    for manifest in manifests {
        for schema in manifest.typed().schemas() {
            charge_schema_work(&mut work, schema)?;
        }
    }
    for (_, descriptor) in base.types() {
        charge_descriptor_work(&mut work, descriptor)?;
    }
    for manifest in manifests {
        for descriptor in manifest.typed().resource_types() {
            charge_descriptor_work(&mut work, descriptor)?;
        }
    }
    for (_, codec) in base.codecs() {
        charge_codec_work(&mut work, codec)?;
    }
    for manifest in manifests {
        for codec in manifest.typed().codecs() {
            charge_codec_work(&mut work, codec)?;
        }
    }
    Some(work)
}

fn charge_schema_work(work: &mut u64, schema: &ResourceValueSchema) -> Option<()> {
    charge(work, 1)?;
    match schema {
        ResourceValueSchema::Record(schema) => {
            charge(work, sort_work(schema.fields().len())?)?;
            charge(work, u64::try_from(schema.fields().len()).ok()?)?;
            for field in schema.fields() {
                charge_value_type_work(work, field.value_type())?;
                if let Some(default) = field.default() {
                    charge_const_work(work, default)?;
                }
            }
        }
        ResourceValueSchema::Enum(schema) => {
            charge(work, sort_work(schema.variants().len())?)?;
            charge(work, u64::try_from(schema.variants().len()).ok()?)?;
            for variant in schema.variants() {
                if let Some(payload) = variant.payload() {
                    charge_value_type_work(work, payload)?;
                }
            }
        }
    }
    Some(())
}

fn charge_value_type_work(work: &mut u64, value_type: &ResourceValueType) -> Option<()> {
    charge(work, 1)?;
    match value_type {
        ResourceValueType::Option(value)
        | ResourceValueType::Vec(value)
        | ResourceValueType::NonEmptyVec(value) => charge_value_type_work(work, value),
        ResourceValueType::Map { key, value } => {
            charge_value_type_work(work, key)?;
            charge_value_type_work(work, value)
        }
        ResourceValueType::Scalar(_)
        | ResourceValueType::NominalRecord(_)
        | ResourceValueType::NominalEnum(_)
        | ResourceValueType::AssetRef { .. }
        | ResourceValueType::ResourceRef { .. }
        | ResourceValueType::ConstrainedScalar(_)
        | ResourceValueType::RetainedIdentityRef { .. } => Some(()),
    }
}

fn charge_const_work(work: &mut u64, value: &ResourceConstValue) -> Option<()> {
    charge(work, 1)?;
    match value {
        ResourceConstValue::Option(value) => value
            .as_deref()
            .map_or(Some(()), |value| charge_const_work(work, value)),
        ResourceConstValue::Sequence(values) => {
            charge(work, u64::try_from(values.len()).ok()?)?;
            values
                .iter()
                .try_for_each(|value| charge_const_work(work, value))
        }
        ResourceConstValue::Map(map) => {
            charge(work, sort_work(map.entries().len())?)?;
            charge(work, u64::try_from(map.entries().len()).ok()?)?;
            for (key, value) in map.entries() {
                charge_const_work(work, key)?;
                charge_const_work(work, value)?;
            }
            Some(())
        }
        ResourceConstValue::Record(record) => {
            charge(work, sort_work(record.fields().len())?)?;
            charge(work, u64::try_from(record.fields().len()).ok()?)?;
            record
                .fields()
                .values()
                .try_for_each(|value| charge_const_work(work, value))
        }
        ResourceConstValue::Enum(value) => value
            .payload()
            .map_or(Some(()), |value| charge_const_work(work, value)),
        ResourceConstValue::Scalar(_)
        | ResourceConstValue::AssetRef(_)
        | ResourceConstValue::ResourceRef(_)
        | ResourceConstValue::RetainedIdentityRef { .. } => Some(()),
    }
}

fn charge_descriptor_work(
    work: &mut u64,
    _descriptor: &arcweft_resource_model::descriptor::ResourceTypeDescriptor,
) -> Option<()> {
    charge(work, 3)
}

fn charge_codec_work(
    work: &mut u64,
    codec: &arcweft_resource_model::descriptor::ResourceCodecSupport,
) -> Option<()> {
    charge(work, 1)?;
    charge(work, sort_work(codec.versions().len())?)?;
    charge(work, u64::try_from(codec.versions().len()).ok()?)
}

fn sort_work(len: usize) -> Option<u64> {
    let n = u64::try_from(len).ok()?;
    let levels = u64::from(n.max(2).ilog2() + u32::from(!n.max(2).is_power_of_two()));
    n.checked_mul(levels)
}

fn charge(work: &mut u64, units: u64) -> Option<()> {
    *work = work.checked_add(units)?;
    Some(())
}

#[derive(Clone, Copy)]
struct Location<'a> {
    manifest: &'a SourceBackedResourceTypeManifestV1,
    token: JsonTokenRange,
}

impl Location<'_> {
    fn span(self) -> SourceSpan {
        self.manifest
            .document()
            .span(self.token.value())
            .expect("accepted source-map ranges belong to their retained document")
    }
}

fn registry_report(
    issues: &[ResourceRegistryIssue],
    manifests: &[SourceBackedResourceTypeManifestV1],
) -> ResourceManifestReport {
    ResourceManifestReport::new(
        issues
            .iter()
            .map(|issue| registry_diagnostic(issue, manifests)),
    )
}

fn registry_diagnostic(
    issue: &ResourceRegistryIssue,
    manifests: &[SourceBackedResourceTypeManifestV1],
) -> ResourceManifestDiagnostic {
    let (code, primary, related) = match issue {
        ResourceRegistryIssue::UnsupportedManifestSchemaVersion { .. } => registry_root(),
        ResourceRegistryIssue::DuplicateCodec { codec } => {
            duplicate_locations(codec_locations(manifests, codec))
        }
        ResourceRegistryIssue::CodecWithoutVersions { codec } => {
            registry_location(codec_locations(manifests, codec).into_iter().next())
        }
        ResourceRegistryIssue::DuplicateType { type_id, .. } => {
            duplicate_locations(type_locations(manifests, type_id))
        }
        ResourceRegistryIssue::DuplicateSchema { schema } => {
            duplicate_locations(schema_locations(manifests, schema))
        }
        ResourceRegistryIssue::DuplicateNominalSchema { first, second, .. } => {
            duplicate_nominal_locations(manifests, first, second)
        }
        ResourceRegistryIssue::DuplicateFieldId { schema, field } => {
            field_location(manifests, schema, *field, FieldToken::Identity, true)
        }
        ResourceRegistryIssue::DuplicateFieldName { schema, field } => {
            named_field_location(manifests, schema, field.as_str())
        }
        ResourceRegistryIssue::DuplicateVariantId { schema, variant } => {
            variant_location(manifests, schema, *variant)
        }
        ResourceRegistryIssue::DuplicateVariantName { schema, variant } => {
            named_variant_location(manifests, schema, variant.as_str())
        }
        ResourceRegistryIssue::RequiredFieldHasDefault { schema, field } => {
            field_location(manifests, schema, *field, FieldToken::Default, false)
        }
        ResourceRegistryIssue::InvalidFieldDefault {
            schema,
            field,
            source,
        } => invalid_default_locations(manifests, schema, *field, source),
        ResourceRegistryIssue::UnknownValueSchema { owner, path, .. }
        | ResourceRegistryIssue::ValueSchemaKindMismatch { owner, path, .. }
        | ResourceRegistryIssue::UnknownResourceReferenceType { owner, path, .. }
        | ResourceRegistryIssue::ValueTypeNestingTooDeep { owner, path } => (
            ResourceManifestDiagnosticCode::RegistryValidation,
            value_type_location(manifests, owner, path),
            None,
        ),
        ResourceRegistryIssue::UnknownBodySchema { type_id, .. }
        | ResourceRegistryIssue::BodySchemaNotRecord { type_id, .. }
        | ResourceRegistryIssue::BodySchemaNominalTypeMismatch { type_id, .. } => {
            descriptor_location(manifests, type_id, DescriptorToken::BodySchema)
        }
        ResourceRegistryIssue::ProvenancePackageMismatch { type_id, .. } => {
            descriptor_location(manifests, type_id, DescriptorToken::Identity)
        }
        ResourceRegistryIssue::FamilyCollision {
            first_type,
            second_type,
            ..
        } => family_locations(manifests, first_type, second_type),
        ResourceRegistryIssue::MissingCodec { type_id, .. } => {
            descriptor_location(manifests, type_id, DescriptorToken::Codec)
        }
        ResourceRegistryIssue::UnsupportedCodecVersion { type_id, .. } => {
            descriptor_location(manifests, type_id, DescriptorToken::CodecVersion)
        }
        ResourceRegistryIssue::InvalidCapabilities { type_id, .. } => {
            descriptor_location(manifests, type_id, DescriptorToken::Capabilities)
        }
    };
    let primary = primary.map_or_else(|| manifests[0].document().start_span(), Location::span);
    ResourceManifestDiagnostic::new(
        code,
        issue.to_string(),
        primary,
        related.map(|location| {
            ResourceManifestRelatedSpan::new("related declaration", location.span())
        }),
    )
}

type DiagnosticLocations<'a> = (
    ResourceManifestDiagnosticCode,
    Option<Location<'a>>,
    Option<Location<'a>>,
);

fn registry_root<'a>() -> DiagnosticLocations<'a> {
    (
        ResourceManifestDiagnosticCode::RegistryValidation,
        None,
        None,
    )
}

fn registry_location(location: Option<Location<'_>>) -> DiagnosticLocations<'_> {
    (
        ResourceManifestDiagnosticCode::RegistryValidation,
        location,
        None,
    )
}

fn duplicate_nominal_locations<'a>(
    manifests: &'a [SourceBackedResourceTypeManifestV1],
    first: &ResourceSchemaId,
    second: &ResourceSchemaId,
) -> DiagnosticLocations<'a> {
    (
        ResourceManifestDiagnosticCode::DuplicateRecord,
        schema_location(manifests, second).map(|location| Location {
            manifest: location.manifest,
            token: location.source.nominal_type(),
        }),
        schema_location(manifests, first).map(|location| Location {
            manifest: location.manifest,
            token: location.source.nominal_type(),
        }),
    )
}

#[derive(Clone, Copy)]
enum FieldToken {
    Identity,
    Default,
}

fn field_location<'a>(
    manifests: &'a [SourceBackedResourceTypeManifestV1],
    schema_id: &ResourceSchemaId,
    field_id: arcweft_resource_model::identity::ResourceFieldId,
    token: FieldToken,
    duplicate: bool,
) -> DiagnosticLocations<'a> {
    let location = schema_location(manifests, schema_id).and_then(|schema| {
        schema.source.fields().get(&field_id).map(|field| Location {
            manifest: schema.manifest,
            token: match token {
                FieldToken::Identity => field.identity(),
                FieldToken::Default => field.default().unwrap_or(field.record()),
            },
        })
    });
    (
        if duplicate {
            ResourceManifestDiagnosticCode::DuplicateRecord
        } else {
            ResourceManifestDiagnosticCode::RegistryValidation
        },
        location,
        None,
    )
}

fn named_field_location<'a>(
    manifests: &'a [SourceBackedResourceTypeManifestV1],
    schema_id: &ResourceSchemaId,
    name: &str,
) -> DiagnosticLocations<'a> {
    let location = schema_location(manifests, schema_id).and_then(|schema| {
        schema
            .source
            .fields()
            .values()
            .find(|source| token_text(source.name(), schema.manifest) == name)
            .map(|source| Location {
                manifest: schema.manifest,
                token: source.name(),
            })
    });
    (
        ResourceManifestDiagnosticCode::DuplicateRecord,
        location,
        None,
    )
}

fn variant_location<'a>(
    manifests: &'a [SourceBackedResourceTypeManifestV1],
    schema_id: &ResourceSchemaId,
    variant_id: arcweft_resource_model::identity::ResourceVariantId,
) -> DiagnosticLocations<'a> {
    let location = schema_location(manifests, schema_id).and_then(|schema| {
        schema
            .source
            .variants()
            .get(&variant_id)
            .map(|variant| Location {
                manifest: schema.manifest,
                token: variant.identity(),
            })
    });
    (
        ResourceManifestDiagnosticCode::DuplicateRecord,
        location,
        None,
    )
}

fn named_variant_location<'a>(
    manifests: &'a [SourceBackedResourceTypeManifestV1],
    schema_id: &ResourceSchemaId,
    name: &str,
) -> DiagnosticLocations<'a> {
    let location = schema_location(manifests, schema_id).and_then(|schema| {
        schema
            .source
            .variants()
            .values()
            .find(|source| token_text(source.name(), schema.manifest) == name)
            .map(|source| Location {
                manifest: schema.manifest,
                token: source.name(),
            })
    });
    (
        ResourceManifestDiagnosticCode::DuplicateRecord,
        location,
        None,
    )
}

#[derive(Clone, Copy)]
enum DescriptorToken {
    Identity,
    BodySchema,
    Codec,
    CodecVersion,
    Capabilities,
}

fn descriptor_location<'a>(
    manifests: &'a [SourceBackedResourceTypeManifestV1],
    type_id: &ResourceTypeId,
    token: DescriptorToken,
) -> DiagnosticLocations<'a> {
    registry_location(type_location(manifests, type_id).map(|location| Location {
        manifest: location.manifest,
        token: match token {
            DescriptorToken::Identity => location.source.identity(),
            DescriptorToken::BodySchema => location.source.body_schema(),
            DescriptorToken::Codec => location.source.lowering_codec(),
            DescriptorToken::CodecVersion => location.source.lowering_version(),
            DescriptorToken::Capabilities => location.source.capabilities(),
        },
    }))
}

fn family_locations<'a>(
    manifests: &'a [SourceBackedResourceTypeManifestV1],
    first: &ResourceTypeId,
    second: &ResourceTypeId,
) -> DiagnosticLocations<'a> {
    (
        ResourceManifestDiagnosticCode::RegistryValidation,
        type_location(manifests, second).map(|location| Location {
            manifest: location.manifest,
            token: location.source.public_id_family(),
        }),
        type_location(manifests, first).map(|location| Location {
            manifest: location.manifest,
            token: location.source.public_id_family(),
        }),
    )
}

fn duplicate_locations(
    mut locations: Vec<Location<'_>>,
) -> (
    ResourceManifestDiagnosticCode,
    Option<Location<'_>>,
    Option<Location<'_>>,
) {
    locations.sort_by_key(|location| location.manifest.typed().package());
    let related = locations.first().copied();
    let primary = locations.last().copied();
    (
        ResourceManifestDiagnosticCode::DuplicateRecord,
        primary,
        related.filter(|related| {
            primary.is_some_and(|primary| !std::ptr::eq(primary.manifest, related.manifest))
        }),
    )
}

struct SchemaLocation<'a> {
    manifest: &'a SourceBackedResourceTypeManifestV1,
    source: &'a ResourceSchemaSource,
}

struct TypeLocation<'a> {
    manifest: &'a SourceBackedResourceTypeManifestV1,
    source: &'a ResourceTypeSource,
}

fn schema_location<'a>(
    manifests: &'a [SourceBackedResourceTypeManifestV1],
    id: &ResourceSchemaId,
) -> Option<SchemaLocation<'a>> {
    manifests.iter().find_map(|manifest| {
        manifest
            .source_map()
            .schemas()
            .get(id)
            .map(|source| SchemaLocation { manifest, source })
    })
}

fn schema_locations<'a>(
    manifests: &'a [SourceBackedResourceTypeManifestV1],
    id: &ResourceSchemaId,
) -> Vec<Location<'a>> {
    manifests
        .iter()
        .filter_map(|manifest| {
            manifest
                .source_map()
                .schemas()
                .get(id)
                .map(|source| Location {
                    manifest,
                    token: source.identity(),
                })
        })
        .collect()
}

fn type_location<'a>(
    manifests: &'a [SourceBackedResourceTypeManifestV1],
    id: &ResourceTypeId,
) -> Option<TypeLocation<'a>> {
    manifests.iter().find_map(|manifest| {
        manifest
            .source_map()
            .resource_types()
            .get(id)
            .map(|source| TypeLocation { manifest, source })
    })
}

fn type_locations<'a>(
    manifests: &'a [SourceBackedResourceTypeManifestV1],
    id: &ResourceTypeId,
) -> Vec<Location<'a>> {
    manifests
        .iter()
        .filter_map(|manifest| {
            manifest
                .source_map()
                .resource_types()
                .get(id)
                .map(|source| Location {
                    manifest,
                    token: source.identity(),
                })
        })
        .collect()
}

fn codec_locations<'a>(
    manifests: &'a [SourceBackedResourceTypeManifestV1],
    id: &ResourceCodecId,
) -> Vec<Location<'a>> {
    manifests
        .iter()
        .filter_map(|manifest| {
            manifest
                .source_map()
                .codecs()
                .get(id)
                .map(|source| Location {
                    manifest,
                    token: source.identity(),
                })
        })
        .collect()
}

fn value_type_location<'a>(
    manifests: &'a [SourceBackedResourceTypeManifestV1],
    owner: &ResourceSchemaId,
    path: &ResourceValueTypePath,
) -> Option<Location<'a>> {
    let schema = schema_location(manifests, owner)?;
    let token = match path.segments().first()? {
        ResourceValueTypePathSegment::RecordField(field) => {
            let source = schema.source.fields().get(field)?;
            source
                .value_type_paths()
                .get(path)
                .copied()
                .unwrap_or_else(|| source.value_type())
        }
        ResourceValueTypePathSegment::EnumVariant(variant) => {
            let source = schema.source.variants().get(variant)?;
            source
                .value_type_paths()
                .get(path)
                .copied()
                .or_else(|| source.payload())?
        }
        ResourceValueTypePathSegment::OptionValue
        | ResourceValueTypePathSegment::SequenceElement
        | ResourceValueTypePathSegment::MapKey
        | ResourceValueTypePathSegment::MapValue => schema.source.record(),
    };
    Some(Location {
        manifest: schema.manifest,
        token,
    })
}

fn invalid_default_locations<'a>(
    manifests: &'a [SourceBackedResourceTypeManifestV1],
    schema_id: &ResourceSchemaId,
    field_id: arcweft_resource_model::identity::ResourceFieldId,
    error: &ResourceDefaultValidationError,
) -> (
    ResourceManifestDiagnosticCode,
    Option<Location<'a>>,
    Option<Location<'a>>,
) {
    let Some(schema) = schema_location(manifests, schema_id) else {
        return (
            ResourceManifestDiagnosticCode::RegistryValidation,
            None,
            None,
        );
    };
    let Some(field) = schema.source.fields().get(&field_id) else {
        return (
            ResourceManifestDiagnosticCode::RegistryValidation,
            None,
            None,
        );
    };
    let mut segments = Vec::new();
    collect_default_segments(error, &mut segments);
    let default_path = ResourceConstSourcePath::new(segments.iter().cloned());
    let primary = field
        .default_paths()
        .get(&default_path)
        .copied()
        .or_else(|| field.default())
        .map(|token| Location {
            manifest: schema.manifest,
            token,
        });
    let related = default_type_location(manifests, schema_id, field_id, &segments).or_else(|| {
        let type_path =
            ResourceValueTypePath::new([ResourceValueTypePathSegment::RecordField(field_id)]);
        field
            .value_type_paths()
            .get(&type_path)
            .copied()
            .map(|token| Location {
                manifest: schema.manifest,
                token,
            })
    });
    (
        ResourceManifestDiagnosticCode::RegistryValidation,
        primary,
        related,
    )
}

fn default_type_location<'a>(
    manifests: &'a [SourceBackedResourceTypeManifestV1],
    schema_id: &ResourceSchemaId,
    field_id: arcweft_resource_model::identity::ResourceFieldId,
    segments: &[ResourceValidationPathSegment],
) -> Option<Location<'a>> {
    let schema = typed_schema(manifests, schema_id)?;
    let ResourceValueSchema::Record(schema) = schema else {
        return None;
    };
    let field = schema
        .fields()
        .iter()
        .find(|field| field.id() == field_id)?;
    walk_default_type(manifests, field.value_type(), segments)
}

fn walk_default_type<'a>(
    manifests: &'a [SourceBackedResourceTypeManifestV1],
    value_type: &ResourceValueType,
    segments: &[ResourceValidationPathSegment],
) -> Option<Location<'a>> {
    let (segment, rest) = segments.split_first()?;
    match (segment, value_type) {
        (ResourceValidationPathSegment::OptionValue, ResourceValueType::Option(value))
        | (
            ResourceValidationPathSegment::SequenceIndex(_),
            ResourceValueType::Vec(value) | ResourceValueType::NonEmptyVec(value),
        ) => walk_default_type(manifests, value, rest),
        (ResourceValidationPathSegment::MapKey(_), ResourceValueType::Map { key, .. }) => {
            walk_default_type(manifests, key, rest)
        }
        (ResourceValidationPathSegment::MapValue(_), ResourceValueType::Map { value, .. }) => {
            walk_default_type(manifests, value, rest)
        }
        (
            ResourceValidationPathSegment::RecordField(field_id),
            ResourceValueType::NominalRecord(target),
        ) => {
            let target_schema = typed_schema(manifests, target)?;
            let ResourceValueSchema::Record(target_schema) = target_schema else {
                return None;
            };
            let target_field = target_schema
                .fields()
                .iter()
                .find(|field| field.id() == *field_id)?;
            if rest.is_empty() {
                let schema_source = schema_location(manifests, target)?;
                let field_source = schema_source.source.fields().get(field_id)?;
                Some(Location {
                    manifest: schema_source.manifest,
                    token: field_source.value_type(),
                })
            } else {
                walk_default_type(manifests, target_field.value_type(), rest)
            }
        }
        (ResourceValidationPathSegment::EnumPayload, ResourceValueType::NominalEnum(_))
        | (_, _) => None,
    }
}

fn typed_schema<'a>(
    manifests: &'a [SourceBackedResourceTypeManifestV1],
    id: &ResourceSchemaId,
) -> Option<&'a ResourceValueSchema> {
    manifests
        .iter()
        .flat_map(|manifest| manifest.typed().schemas())
        .find(|schema| schema.id() == id)
}

fn collect_default_segments(
    error: &ResourceDefaultValidationError,
    output: &mut Vec<ResourceValidationPathSegment>,
) {
    if let ResourceDefaultValidationError::Nested { segment, source } = error {
        output.push(segment.clone());
        collect_default_segments(source, output);
    }
}

fn token_text(token: JsonTokenRange, manifest: &SourceBackedResourceTypeManifestV1) -> &str {
    manifest
        .document()
        .text()
        .get(token.value().as_range())
        .unwrap_or_default()
        .trim_matches('"')
}

impl PublishedResourceTypeManifestSetV1 {
    pub fn manifests(&self) -> &[SourceBackedResourceTypeManifestV1] {
        &self.manifests
    }
    pub const fn registry(&self) -> &Arc<ResourceTypeRegistry> {
        &self.registry
    }
    pub const fn registry_digest(&self) -> ResourceTypeRegistryDigest {
        self.registry_digest
    }
}
