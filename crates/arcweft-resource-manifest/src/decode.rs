use crate::{
    diagnostic::{
        ResourceManifestDiagnostic, ResourceManifestDiagnosticCode, ResourceManifestRelatedSpan,
        ResourceManifestReport,
    },
    encode::encode_resource_type_manifest_v1,
    limits::ResourceManifestDecodeLimits,
    strict_json::parse_strict_json,
    wire::{
        PackageCoordinateFile, ResourceTypeManifestFileV1, SourceBackedResourceTypeManifestV1,
        TypedResourceTypeManifestV1,
    },
};
use arcweft_core::{locale::LocaleId, time::LogicalDuration};
use arcweft_id::{EntityId, PublicId};
use arcweft_interaction_model::audio::{GainDbMilli, PanMilli};
use arcweft_layout::LayoutUnit;
use arcweft_manifest_model::{PackageId, PackageVersion, RawDigest, SemanticDigest};
use arcweft_resource_model::{
    descriptor::{
        ResourceAgentExposure, ResourceCapabilities, ResourceCodecSupport,
        ResourceDescriptorProvenance, ResourceEnumSchema, ResourceFieldDescriptor,
        ResourceHotReloadClass, ResourceLoweringBinding, ResourceRecordSchema,
        ResourceTypeDescriptor, ResourceTypeDocs, ResourceValueSchema, ResourceVariantDescriptor,
    },
    identity::{
        NominalTypeId, ResourceAssetPayloadKindId, ResourceBundleSectionId,
        ResourceBundleSectionVersion, ResourceCodecId, ResourceCodecVersion,
        ResourceDescriptorSourceId, ResourceFamilyGroupId, ResourceFieldId, ResourceFieldName,
        ResourceModulePath, ResourcePublicIdFamily, ResourceRuntimeHandleKindId, ResourceSchemaId,
        ResourceSchemaVersion, ResourceTypeId, ResourceTypeName, ResourceVariantId,
        ResourceVariantName,
    },
    retained::{PresentationTargetScope, ResolvedRetainedIdentityRef, RetainedIdentityKind},
    value::{
        ResourceAssetRefValue, ResourceBoundKind, ResourceConstValue, ResourceEnumValue,
        ResourceFloat, ResourceLength, ResourceMapValue, ResourceRatio, ResourceRecordValue,
        ResourceRefValue, ResourceScalarBound, ResourceScalarConstraint, ResourceScalarType,
        ResourceScalarValue, ResourceValueType,
    },
};
use arcweft_source::{SourceDocument, SourceRange};
use serde::Deserialize;
use std::{collections::BTreeSet, str::FromStr, sync::Arc};

#[derive(Clone, Copy, Debug)]
pub enum ResourceManifestPackageExpectation<'a> {
    Selected(&'a PackageCoordinateFile),
    EmbeddedArtifact,
}

impl<'a> From<&'a PackageCoordinateFile> for ResourceManifestPackageExpectation<'a> {
    fn from(value: &'a PackageCoordinateFile) -> Self {
        Self::Selected(value)
    }
}

pub fn decode_resource_type_manifest<'a>(
    document: Arc<SourceDocument>,
    expected: impl Into<ResourceManifestPackageExpectation<'a>>,
    limits: ResourceManifestDecodeLimits,
) -> Result<SourceBackedResourceTypeManifestV1, ResourceManifestReport> {
    let expected = expected.into();
    let (json, mut source_map) = parse_strict_json(document.text(), limits)
        .map_err(|error| strict_error_report(&document, error))?;
    probe_dispatch(&document, &json)?;
    let mut budget = crate::budget::DecodeBudget::new(limits);
    crate::shape::validate_manifest_shape(&json, &mut budget)
        .map_err(|error| shape_error_report(&document, &source_map, error))?;
    let dto = ManifestDto::deserialize(&json).map_err(|error| {
        report(
            &document,
            ResourceManifestDiagnosticCode::WrongShape,
            format!("resource manifest shape is invalid: {error}"),
            SourceRange::new(0, document.text().len()),
            [],
        )
    })?;
    let file = lower_manifest(&document, &source_map, dto, expected, &mut budget)?;
    source_map.bind_semantics(&file, &json);
    let typed = TypedResourceTypeManifestV1::from_file(&file);
    let canonical_bytes = encode_resource_type_manifest_v1(&typed).map_err(|error| {
        report(
            &document,
            ResourceManifestDiagnosticCode::WorkLimit,
            error.to_string(),
            SourceRange::new(0, document.text().len()),
            [],
        )
    })?;
    budget
        .charge_bytes(
            canonical_bytes.len(),
            &crate::JsonPath::default(),
            "canonical JSON emission",
        )
        .map_err(|error| {
            shape_error_report(
                &document,
                &source_map,
                crate::shape::ShapeError::from(error),
            )
        })?;
    let canonical_digest = RawDigest::for_bytes(&canonical_bytes);
    Ok(SourceBackedResourceTypeManifestV1::new(
        document,
        file,
        typed,
        source_map,
        canonical_bytes.into(),
        canonical_digest,
    ))
}

fn shape_error_report(
    document: &SourceDocument,
    source_map: &crate::ResourceManifestSourceMap,
    error: crate::shape::ShapeError,
) -> ResourceManifestReport {
    let token = source_map.token(&error.path);
    let range = if error.code == ResourceManifestDiagnosticCode::MissingField {
        token.map_or_else(
            || document.end_span().range(),
            |token| {
                let end = token.value().end().saturating_sub(1);
                SourceRange::new(end, end)
            },
        )
    } else {
        token.map_or_else(
            || document.end_span().range(),
            |token| token.key().unwrap_or(token.value()),
        )
    };
    let related = error
        .related
        .and_then(|path| source_map.token(&path))
        .and_then(|token| document.span(token.value()).ok())
        .map(|span| ResourceManifestRelatedSpan::new("first semantic record", span));
    report(document, error.code, error.message, range, related)
}

fn probe_dispatch(
    document: &SourceDocument,
    value: &serde_json::Value,
) -> Result<(), ResourceManifestReport> {
    let object = value.as_object().ok_or_else(|| {
        report(
            document,
            ResourceManifestDiagnosticCode::RootWrongShape,
            "resource manifest root must be an object",
            SourceRange::new(0, document.text().len()),
            [],
        )
    })?;
    match object.get("format") {
        None => {
            return Err(report(
                document,
                ResourceManifestDiagnosticCode::MissingFormat,
                "resource manifest format is required",
                document.end_span().range(),
                [],
            ));
        }
        Some(serde_json::Value::String(value))
            if value == crate::wire::RESOURCE_TYPE_MANIFEST_FORMAT => {}
        Some(serde_json::Value::String(value)) => {
            return Err(report(
                document,
                ResourceManifestDiagnosticCode::UnsupportedFormat,
                format!("unsupported resource manifest format `{value}`"),
                SourceRange::new(0, document.text().len()),
                [],
            ));
        }
        Some(_) => {
            return Err(report(
                document,
                ResourceManifestDiagnosticCode::MalformedFormat,
                "resource manifest format must be a string",
                SourceRange::new(0, document.text().len()),
                [],
            ));
        }
    }
    match object.get("schema") {
        None => Err(report(
            document,
            ResourceManifestDiagnosticCode::MissingSchemaVersion,
            "resource manifest schema is required",
            document.end_span().range(),
            [],
        )),
        Some(serde_json::Value::Number(value)) if value.as_u64() == Some(1) => Ok(()),
        Some(serde_json::Value::Number(value)) => Err(report(
            document,
            ResourceManifestDiagnosticCode::UnsupportedSchemaVersion,
            format!("unsupported resource manifest schema `{value}`"),
            SourceRange::new(0, document.text().len()),
            [],
        )),
        Some(_) => Err(report(
            document,
            ResourceManifestDiagnosticCode::MalformedSchemaVersion,
            "resource manifest schema must be an integer",
            SourceRange::new(0, document.text().len()),
            [],
        )),
    }
}

fn strict_error_report(
    document: &SourceDocument,
    error: crate::strict_json::StrictJsonError,
) -> ResourceManifestReport {
    let related = error
        .related
        .and_then(|range| document.span(SourceRange::new(range.start, range.end)).ok())
        .map(|span| ResourceManifestRelatedSpan::new("first occurrence", span));
    report(
        document,
        error.code,
        error.message,
        SourceRange::new(error.primary.start, error.primary.end),
        related,
    )
}

fn report(
    document: &SourceDocument,
    code: ResourceManifestDiagnosticCode,
    message: impl Into<String>,
    primary: SourceRange,
    related: impl IntoIterator<Item = ResourceManifestRelatedSpan>,
) -> ResourceManifestReport {
    let primary = document
        .span(primary)
        .unwrap_or_else(|_| document.start_span());
    ResourceManifestReport::one(ResourceManifestDiagnostic::new(
        code, message, primary, related,
    ))
}

fn lower_manifest(
    document: &SourceDocument,
    source_map: &crate::ResourceManifestSourceMap,
    dto: ManifestDto,
    expected: ResourceManifestPackageExpectation<'_>,
    budget: &mut crate::budget::DecodeBudget,
) -> Result<ResourceTypeManifestFileV1, ResourceManifestReport> {
    let package = package(dto.package).map_err(|message| invalid(document, message))?;
    if let ResourceManifestPackageExpectation::Selected(expected) = expected
        && &package != expected
    {
        return Err(report(
            document,
            ResourceManifestDiagnosticCode::PackageMismatch,
            format!(
                "manifest package {}@{} does not match selected {}@{}",
                package.id(),
                package.version(),
                expected.id(),
                expected.version()
            ),
            source_map
                .token(&crate::JsonPath::default().field("package"))
                .map_or_else(|| document.end_span().range(), crate::JsonTokenRange::value),
            [],
        ));
    }
    let mut schemas = dto
        .schemas
        .into_iter()
        .map(lower_schema)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|message| invalid(document, message))?;
    schemas.sort_by(|left, right| left.id().cmp(right.id()));
    let mut resource_types = dto
        .resource_types
        .into_iter()
        .enumerate()
        .map(|(index, descriptor)| lower_descriptor(&package, descriptor, index, budget))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| match error {
            DescriptorLowerError::Invalid(message) => invalid(document, message),
            DescriptorLowerError::InvalidDigest { index, message } => {
                let descriptor = crate::JsonPath::default()
                    .field("resource_types")
                    .index(index);
                report(
                    document,
                    ResourceManifestDiagnosticCode::InvalidDigest,
                    message,
                    source_map
                        .token(&descriptor.field("descriptor_digest"))
                        .map_or_else(|| document.end_span().range(), crate::JsonTokenRange::value),
                    [],
                )
            }
            DescriptorLowerError::DigestMismatch { index } => {
                let descriptor = crate::JsonPath::default()
                    .field("resource_types")
                    .index(index);
                let related = source_map
                    .token(&descriptor.field("type_id"))
                    .and_then(|token| document.span(token.value()).ok())
                    .map(|span| ResourceManifestRelatedSpan::new("descriptor type identity", span));
                report(
                    document,
                    ResourceManifestDiagnosticCode::DescriptorDigestMismatch,
                    "resource descriptor semantic digest does not match its claim",
                    source_map
                        .token(&descriptor.field("descriptor_digest"))
                        .map_or_else(|| document.end_span().range(), crate::JsonTokenRange::value),
                    related,
                )
            }
            DescriptorLowerError::Budget(error) => {
                shape_error_report(document, source_map, crate::shape::ShapeError::from(error))
            }
        })?;
    resource_types.sort_by(|left, right| left.type_id().cmp(right.type_id()));
    let mut codecs = dto
        .codecs
        .into_iter()
        .map(lower_codec)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|message| invalid(document, message))?;
    codecs.sort_by(|left, right| left.codec_id().cmp(right.codec_id()));
    Ok(ResourceTypeManifestFileV1::new(
        package,
        schemas,
        resource_types,
        codecs,
    ))
}

fn invalid(document: &SourceDocument, message: String) -> ResourceManifestReport {
    report(
        document,
        ResourceManifestDiagnosticCode::InvalidId,
        message,
        SourceRange::new(0, document.text().len()),
        [],
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestDto {
    #[serde(rename = "format")]
    _format: String,
    #[serde(rename = "schema")]
    _schema: u32,
    package: PackageDto,
    schemas: Vec<SchemaDto>,
    resource_types: Vec<DescriptorDto>,
    codecs: Vec<CodecDto>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageDto {
    id: String,
    version: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct NominalDto {
    package: String,
    module: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum SchemaDto {
    Record(RecordSchemaDto),
    Enum(EnumSchemaDto),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordSchemaDto {
    schema_id: String,
    nominal_type: NominalDto,
    version: u32,
    fields: Vec<FieldDto>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnumSchemaDto {
    schema_id: String,
    nominal_type: NominalDto,
    version: u32,
    variants: Vec<VariantDto>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FieldDto {
    field_id: u32,
    name: String,
    value_type: ValueTypeDto,
    presence: PresenceDto,
    default: Option<ConstDto>,
    #[serde(default)]
    docs: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VariantDto {
    variant_id: u32,
    name: String,
    payload: Option<ValueTypeDto>,
    #[serde(default)]
    docs: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PresenceDto {
    Required,
    Optional,
}

#[derive(Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum ValueTypeDto {
    Scalar(ScalarTypeDto),
    Option(Box<Self>),
    List(Box<Self>),
    NonEmptyList(Box<Self>),
    OrderedMap(MapTypeDto),
    Record(String),
    Enum(String),
    AssetRef(AssetTypeDto),
    ResourceRef(ResourceRefTypeDto),
    RetainedIdentityRef(String),
    ConstrainedScalar(ConstraintDto),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapTypeDto {
    key: Box<ValueTypeDto>,
    value: Box<ValueTypeDto>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetTypeDto {
    payload_kind: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceRefTypeDto {
    type_id: NominalDto,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ScalarTypeDto {
    Unit,
    Bool,
    SignedInteger,
    UnsignedInteger,
    Float,
    String,
    Char,
    Duration,
    Ratio,
    Length,
    Gain,
    Pan,
    Locale,
    PublicId,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConstraintDto {
    scalar: ScalarTypeDto,
    lower: Option<BoundDto>,
    upper: Option<BoundDto>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BoundDto {
    kind: BoundKindDto,
    value: ScalarValueDto,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BoundKindDto {
    Inclusive,
    Exclusive,
}

#[derive(Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum ScalarValueDto {
    Unit,
    Bool(bool),
    SignedInteger(i64),
    UnsignedInteger(u64),
    Float(String),
    String(String),
    Char(String),
    Duration(u64),
    Ratio(u32),
    Length(LengthDto),
    Gain(i32),
    Pan(i16),
    Locale(String),
    PublicId(String),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LengthDto {
    milli_units: i64,
    unit: LayoutUnit,
}

#[derive(Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum ConstDto {
    Scalar(ScalarValueDto),
    Option(Option<Box<Self>>),
    List(Vec<Self>),
    OrderedMap(Vec<MapEntryDto>),
    Record(RecordValueDto),
    Enum(EnumValueDto),
    AssetRef(AssetValueDto),
    ResourceRef(ResourceValueDto),
    RetainedIdentityRef(RetainedDto),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapEntryDto {
    key: ConstDto,
    value: ConstDto,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordValueDto {
    schema_id: String,
    fields: Vec<RecordFieldDto>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordFieldDto {
    field_id: u32,
    value: ConstDto,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnumValueDto {
    schema_id: String,
    variant_id: u32,
    payload: Option<Box<ConstDto>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetValueDto {
    public_id: String,
    payload_kind: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceValueDto {
    #[serde(rename = "entity_id")]
    entity: String,
    #[serde(rename = "public_id")]
    public: String,
    #[serde(rename = "type_id")]
    resource_type: NominalDto,
}

#[derive(Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum RetainedDto {
    Character(EntityDto),
    View(EntityDto),
    Action(EntityDto),
    Layer(EntityDto),
    Signal(EntityDto),
    PresentationTarget(PresentationDto),
    ScrollRegion(ScrollDto),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntityDto {
    entity_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PresentationDto {
    scope: PresentationScopeDto,
    target_id: String,
}
#[derive(Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum PresentationScopeDto {
    Global,
    View(ViewScopeDto),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewScopeDto {
    owner_view_entity_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScrollDto {
    owner_view_entity_id: String,
    region_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DescriptorDto {
    type_id: NominalDto,
    public_id_family: String,
    family_group: String,
    body_schema: String,
    capabilities: CapabilitiesDto,
    lowering: LoweringDto,
    docs: Option<DocsDto>,
    descriptor_digest: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapabilitiesDto {
    runtime_handle_kind: Option<String>,
    agent_exposure: ExposureDto,
    save_definition_reference: bool,
    hot_reload: HotReloadDto,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ExposureDto {
    Hidden,
    Catalog,
    CatalogAndRuntime,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum HotReloadDto {
    RestartRequired,
    ReplaceDefinition,
    UpdateLiveHandle,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoweringDto {
    codec_id: String,
    codec_version: u32,
    section_id: String,
    section_version: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocsDto {
    summary: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CodecDto {
    codec_id: String,
    versions: Vec<u32>,
}

fn package(dto: PackageDto) -> Result<PackageCoordinateFile, String> {
    Ok(PackageCoordinateFile::new(
        PackageId::new(dto.id).map_err(|e| e.to_string())?,
        PackageVersion::new(dto.version).map_err(|e| e.to_string())?,
    ))
}
fn nominal(dto: NominalDto) -> Result<NominalTypeId, String> {
    Ok(NominalTypeId::new(
        PackageId::new(dto.package).map_err(|e| e.to_string())?,
        ResourceModulePath::try_new(dto.module).map_err(|e| e.to_string())?,
        ResourceTypeName::try_new(dto.name).map_err(|e| e.to_string())?,
    ))
}
fn lower_schema(dto: SchemaDto) -> Result<ResourceValueSchema, String> {
    match dto {
        SchemaDto::Record(dto) => {
            let mut fields = dto
                .fields
                .into_iter()
                .map(lower_field)
                .collect::<Result<Vec<_>, _>>()?;
            fields.sort_by(|left, right| {
                left.id()
                    .cmp(&right.id())
                    .then_with(|| left.name().cmp(right.name()))
            });
            Ok(ResourceValueSchema::Record(ResourceRecordSchema::new(
                ResourceSchemaId::try_new(dto.schema_id).map_err(|e| e.to_string())?,
                nominal(dto.nominal_type)?,
                ResourceSchemaVersion::try_new(dto.version).map_err(|e| e.to_string())?,
                fields,
            )))
        }
        SchemaDto::Enum(dto) => {
            let mut variants = dto
                .variants
                .into_iter()
                .map(lower_variant)
                .collect::<Result<Vec<_>, _>>()?;
            variants.sort_by(|left, right| {
                left.id()
                    .cmp(&right.id())
                    .then_with(|| left.name().cmp(right.name()))
            });
            Ok(ResourceValueSchema::Enum(ResourceEnumSchema::new(
                ResourceSchemaId::try_new(dto.schema_id).map_err(|e| e.to_string())?,
                nominal(dto.nominal_type)?,
                ResourceSchemaVersion::try_new(dto.version).map_err(|e| e.to_string())?,
                variants,
            )))
        }
    }
}
fn lower_field(dto: FieldDto) -> Result<ResourceFieldDescriptor, String> {
    let id = ResourceFieldId::try_new(dto.field_id).map_err(|e| e.to_string())?;
    let name = ResourceFieldName::try_new(dto.name).map_err(|e| e.to_string())?;
    let ty = lower_value_type(dto.value_type)?;
    let mut field = match dto.presence {
        PresenceDto::Required => ResourceFieldDescriptor::required(id, name, ty),
        PresenceDto::Optional => ResourceFieldDescriptor::optional(id, name, ty),
    };
    if let Some(value) = dto.default {
        field = field.with_default(lower_const(value)?);
    }
    if !dto.docs.is_empty() {
        field = field.with_docs(dto.docs);
    }
    Ok(field)
}
fn lower_variant(dto: VariantDto) -> Result<ResourceVariantDescriptor, String> {
    let id = ResourceVariantId::try_new(dto.variant_id).map_err(|e| e.to_string())?;
    let name = ResourceVariantName::try_new(dto.name).map_err(|e| e.to_string())?;
    let mut variant = match dto.payload {
        Some(value) => ResourceVariantDescriptor::with_payload(id, name, lower_value_type(value)?),
        None => ResourceVariantDescriptor::unit(id, name),
    };
    if !dto.docs.is_empty() {
        variant = variant.with_docs(dto.docs);
    }
    Ok(variant)
}
fn lower_value_type(dto: ValueTypeDto) -> Result<ResourceValueType, String> {
    Ok(match dto {
        ValueTypeDto::Scalar(v) => ResourceValueType::Scalar(scalar_type(v)),
        ValueTypeDto::Option(v) => ResourceValueType::option(lower_value_type(*v)?),
        ValueTypeDto::List(v) => ResourceValueType::vec(lower_value_type(*v)?),
        ValueTypeDto::NonEmptyList(v) => ResourceValueType::non_empty_vec(lower_value_type(*v)?),
        ValueTypeDto::OrderedMap(v) => {
            ResourceValueType::map(lower_value_type(*v.key)?, lower_value_type(*v.value)?)
        }
        ValueTypeDto::Record(v) => ResourceValueType::NominalRecord(
            ResourceSchemaId::try_new(v).map_err(|e| e.to_string())?,
        ),
        ValueTypeDto::Enum(v) => {
            ResourceValueType::NominalEnum(ResourceSchemaId::try_new(v).map_err(|e| e.to_string())?)
        }
        ValueTypeDto::AssetRef(v) => ResourceValueType::AssetRef {
            payload_kind: ResourceAssetPayloadKindId::try_new(v.payload_kind)
                .map_err(|e| e.to_string())?,
        },
        ValueTypeDto::ResourceRef(v) => ResourceValueType::ResourceRef {
            type_id: ResourceTypeId::new(nominal(v.type_id)?),
        },
        ValueTypeDto::RetainedIdentityRef(v) => ResourceValueType::RetainedIdentityRef {
            identity: RetainedIdentityKind::from_manifest_token(&v)
                .ok_or_else(|| format!("unknown retained identity `{v}`"))?,
        },
        ValueTypeDto::ConstrainedScalar(v) => ResourceValueType::ConstrainedScalar(
            ResourceScalarConstraint::try_new(
                scalar_type(v.scalar),
                v.lower.map(lower_bound).transpose()?,
                v.upper.map(lower_bound).transpose()?,
            )
            .map_err(|e| e.to_string())?,
        ),
    })
}
fn lower_bound(dto: BoundDto) -> Result<ResourceScalarBound, String> {
    Ok(ResourceScalarBound::new(
        lower_scalar(dto.value)?,
        match dto.kind {
            BoundKindDto::Inclusive => ResourceBoundKind::Inclusive,
            BoundKindDto::Exclusive => ResourceBoundKind::Exclusive,
        },
    ))
}
fn scalar_type(dto: ScalarTypeDto) -> ResourceScalarType {
    match dto {
        ScalarTypeDto::Unit => ResourceScalarType::Unit,
        ScalarTypeDto::Bool => ResourceScalarType::Bool,
        ScalarTypeDto::SignedInteger => ResourceScalarType::SignedInteger,
        ScalarTypeDto::UnsignedInteger => ResourceScalarType::UnsignedInteger,
        ScalarTypeDto::Float => ResourceScalarType::Float,
        ScalarTypeDto::String => ResourceScalarType::String,
        ScalarTypeDto::Char => ResourceScalarType::Char,
        ScalarTypeDto::Duration => ResourceScalarType::Duration,
        ScalarTypeDto::Ratio => ResourceScalarType::Ratio,
        ScalarTypeDto::Length => ResourceScalarType::Length,
        ScalarTypeDto::Gain => ResourceScalarType::Gain,
        ScalarTypeDto::Pan => ResourceScalarType::Pan,
        ScalarTypeDto::Locale => ResourceScalarType::Locale,
        ScalarTypeDto::PublicId => ResourceScalarType::PublicId,
    }
}
fn lower_scalar(dto: ScalarValueDto) -> Result<ResourceScalarValue, String> {
    Ok(match dto {
        ScalarValueDto::Unit => ResourceScalarValue::Unit,
        ScalarValueDto::Bool(v) => ResourceScalarValue::Bool(v),
        ScalarValueDto::SignedInteger(v) => ResourceScalarValue::SignedInteger(v),
        ScalarValueDto::UnsignedInteger(v) => ResourceScalarValue::UnsignedInteger(v),
        ScalarValueDto::Float(v) => {
            let bits = u64::from_str_radix(
                v.strip_prefix("0x")
                    .ok_or_else(|| "float must start with 0x".to_owned())?,
                16,
            )
            .map_err(|e| e.to_string())?;
            if bits == 0x8000_0000_0000_0000 {
                return Err("negative zero is noncanonical".into());
            }
            ResourceScalarValue::Float(
                ResourceFloat::try_new(f64::from_bits(bits)).map_err(|e| e.to_string())?,
            )
        }
        ScalarValueDto::String(v) => ResourceScalarValue::String(v),
        ScalarValueDto::Char(v) => {
            let mut chars = v.chars();
            let c = chars.next().ok_or_else(|| "char is empty".to_owned())?;
            if chars.next().is_some() {
                return Err("char must contain one scalar".into());
            }
            ResourceScalarValue::Char(c)
        }
        ScalarValueDto::Duration(v) => {
            ResourceScalarValue::Duration(LogicalDuration::from_nanos(v))
        }
        ScalarValueDto::Ratio(v) => ResourceScalarValue::Ratio(
            ResourceRatio::try_from_millionths(v).map_err(|e| e.to_string())?,
        ),
        ScalarValueDto::Length(v) => {
            ResourceScalarValue::Length(ResourceLength::new(v.milli_units, v.unit))
        }
        ScalarValueDto::Gain(v) => {
            ResourceScalarValue::Gain(GainDbMilli::new(v).map_err(|e| e.to_string())?)
        }
        ScalarValueDto::Pan(v) => {
            ResourceScalarValue::Pan(PanMilli::new(v).map_err(|e| e.to_string())?)
        }
        ScalarValueDto::Locale(v) => {
            ResourceScalarValue::Locale(LocaleId::try_new(v).map_err(|e| e.to_string())?)
        }
        ScalarValueDto::PublicId(v) => {
            ResourceScalarValue::PublicId(PublicId::try_new(v).map_err(|e| e.to_string())?)
        }
    })
}
fn lower_const(dto: ConstDto) -> Result<ResourceConstValue, String> {
    Ok(match dto {
        ConstDto::Scalar(v) => ResourceConstValue::Scalar(lower_scalar(v)?),
        ConstDto::Option(v) => {
            ResourceConstValue::Option(v.map(|v| lower_const(*v).map(Box::new)).transpose()?)
        }
        ConstDto::List(v) => {
            ResourceConstValue::Sequence(v.into_iter().map(lower_const).collect::<Result<_, _>>()?)
        }
        ConstDto::OrderedMap(v) => ResourceConstValue::Map(
            ResourceMapValue::try_new(
                v.into_iter()
                    .map(|e| Ok((lower_const(e.key)?, lower_const(e.value)?)))
                    .collect::<Result<Vec<_>, String>>()?,
            )
            .map_err(|e| e.to_string())?,
        ),
        ConstDto::Record(v) => ResourceConstValue::Record(
            ResourceRecordValue::try_new(
                ResourceSchemaId::try_new(v.schema_id).map_err(|e| e.to_string())?,
                v.fields
                    .into_iter()
                    .map(|f| {
                        Ok((
                            ResourceFieldId::try_new(f.field_id).map_err(|e| e.to_string())?,
                            lower_const(f.value)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>()?,
            )
            .map_err(|e| e.to_string())?,
        ),
        ConstDto::Enum(v) => ResourceConstValue::Enum(ResourceEnumValue::new(
            ResourceSchemaId::try_new(v.schema_id).map_err(|e| e.to_string())?,
            ResourceVariantId::try_new(v.variant_id).map_err(|e| e.to_string())?,
            v.payload.map(|v| lower_const(*v)).transpose()?,
        )),
        ConstDto::AssetRef(v) => ResourceConstValue::AssetRef(ResourceAssetRefValue::new(
            PublicId::try_new(v.public_id).map_err(|e| e.to_string())?,
            ResourceAssetPayloadKindId::try_new(v.payload_kind).map_err(|e| e.to_string())?,
        )),
        ConstDto::ResourceRef(v) => ResourceConstValue::ResourceRef(ResourceRefValue::new(
            EntityId::try_new(v.entity).map_err(|e| e.to_string())?,
            PublicId::try_new(v.public).map_err(|e| e.to_string())?,
            ResourceTypeId::new(nominal(v.resource_type)?),
        )),
        ConstDto::RetainedIdentityRef(v) => ResourceConstValue::RetainedIdentityRef {
            value: lower_retained(v)?,
        },
    })
}
fn lower_retained(dto: RetainedDto) -> Result<ResolvedRetainedIdentityRef, String> {
    Ok(match dto {
        RetainedDto::Character(v) => ResolvedRetainedIdentityRef::Character {
            entity_id: EntityId::try_new(v.entity_id).map_err(|e| e.to_string())?,
        },
        RetainedDto::View(v) => ResolvedRetainedIdentityRef::View {
            entity_id: EntityId::try_new(v.entity_id).map_err(|e| e.to_string())?,
        },
        RetainedDto::Action(v) => ResolvedRetainedIdentityRef::Action {
            entity_id: EntityId::try_new(v.entity_id).map_err(|e| e.to_string())?,
        },
        RetainedDto::Layer(v) => ResolvedRetainedIdentityRef::Layer {
            entity_id: EntityId::try_new(v.entity_id).map_err(|e| e.to_string())?,
        },
        RetainedDto::Signal(v) => ResolvedRetainedIdentityRef::Signal {
            entity_id: EntityId::try_new(v.entity_id).map_err(|e| e.to_string())?,
        },
        RetainedDto::PresentationTarget(v) => ResolvedRetainedIdentityRef::PresentationTarget {
            scope: match v.scope {
                PresentationScopeDto::Global => PresentationTargetScope::Global,
                PresentationScopeDto::View(s) => PresentationTargetScope::View {
                    owner_view_entity_id: EntityId::try_new(s.owner_view_entity_id)
                        .map_err(|e| e.to_string())?,
                },
            },
            target_id: PublicId::try_new(v.target_id).map_err(|e| e.to_string())?,
        },
        RetainedDto::ScrollRegion(v) => ResolvedRetainedIdentityRef::ScrollRegion {
            owner_view_entity_id: EntityId::try_new(v.owner_view_entity_id)
                .map_err(|e| e.to_string())?,
            region_id: PublicId::try_new(v.region_id).map_err(|e| e.to_string())?,
        },
    })
}
enum DescriptorLowerError {
    Invalid(String),
    InvalidDigest { index: usize, message: String },
    DigestMismatch { index: usize },
    Budget(crate::budget::BudgetError),
}

fn lower_descriptor(
    package: &PackageCoordinateFile,
    dto: DescriptorDto,
    index: usize,
    budget: &mut crate::budget::DecodeBudget,
) -> Result<ResourceTypeDescriptor, DescriptorLowerError> {
    let invalid = DescriptorLowerError::Invalid;
    let type_id = ResourceTypeId::new(nominal(dto.type_id).map_err(invalid)?);
    if type_id.nominal().package() != package.id() {
        return Err(invalid("descriptor package does not match document".into()));
    }
    let source_id = format!(
        "resource-type-manifest:{}:{}:{}",
        type_id.nominal().package(),
        type_id.nominal().module(),
        type_id.nominal().name()
    );
    let descriptor = ResourceTypeDescriptor::new(
        ResourceDescriptorProvenance::new(
            package.id().clone(),
            ResourceDescriptorSourceId::try_new(source_id).map_err(|e| invalid(e.to_string()))?,
        ),
        type_id,
        ResourcePublicIdFamily::try_new(dto.public_id_family)
            .map_err(|e| invalid(e.to_string()))?,
        ResourceFamilyGroupId::try_new(dto.family_group).map_err(|e| invalid(e.to_string()))?,
        ResourceSchemaId::try_new(dto.body_schema).map_err(|e| invalid(e.to_string()))?,
        ResourceCapabilities::new(
            dto.capabilities
                .runtime_handle_kind
                .map(ResourceRuntimeHandleKindId::try_new)
                .transpose()
                .map_err(|e| invalid(e.to_string()))?,
            match dto.capabilities.agent_exposure {
                ExposureDto::Hidden => ResourceAgentExposure::Hidden,
                ExposureDto::Catalog => ResourceAgentExposure::Catalog,
                ExposureDto::CatalogAndRuntime => ResourceAgentExposure::CatalogAndRuntime,
            },
            dto.capabilities.save_definition_reference,
            match dto.capabilities.hot_reload {
                HotReloadDto::RestartRequired => ResourceHotReloadClass::RestartRequired,
                HotReloadDto::ReplaceDefinition => ResourceHotReloadClass::ReplaceDefinition,
                HotReloadDto::UpdateLiveHandle => ResourceHotReloadClass::UpdateLiveHandle,
            },
        ),
        ResourceLoweringBinding::new(
            ResourceCodecId::try_new(dto.lowering.codec_id).map_err(|e| invalid(e.to_string()))?,
            ResourceCodecVersion::try_new(dto.lowering.codec_version)
                .map_err(|e| invalid(e.to_string()))?,
            ResourceBundleSectionId::try_new(dto.lowering.section_id)
                .map_err(|e| invalid(e.to_string()))?,
            ResourceBundleSectionVersion::try_new(dto.lowering.section_version)
                .map_err(|e| invalid(e.to_string()))?,
        ),
        ResourceTypeDocs::new(dto.docs.map_or(String::new(), |d| d.summary)),
    );
    budget
        .charge_bytes(
            descriptor.semantic_digest_transcript_len(),
            &crate::JsonPath::default()
                .field("resource_types")
                .index(index)
                .field("descriptor_digest"),
            "descriptor digest verification",
        )
        .map_err(DescriptorLowerError::Budget)?;
    budget
        .charge_bytes(
            descriptor.semantic_digest_transcript_len(),
            &crate::JsonPath::default()
                .field("resource_types")
                .index(index)
                .field("descriptor_digest"),
            "canonical descriptor digest",
        )
        .map_err(DescriptorLowerError::Budget)?;
    let claim = SemanticDigest::from_str(&dto.descriptor_digest).map_err(|error| {
        DescriptorLowerError::InvalidDigest {
            index,
            message: error.to_string(),
        }
    })?;
    if descriptor.semantic_digest().semantic_digest() != claim {
        return Err(DescriptorLowerError::DigestMismatch { index });
    }
    Ok(descriptor)
}
fn lower_codec(dto: CodecDto) -> Result<ResourceCodecSupport, String> {
    let versions = dto
        .versions
        .into_iter()
        .map(|v| ResourceCodecVersion::try_new(v).map_err(|e| e.to_string()))
        .collect::<Result<BTreeSet<_>, _>>()?;
    Ok(ResourceCodecSupport::new(
        ResourceCodecId::try_new(dto.codec_id).map_err(|e| e.to_string())?,
        versions,
    ))
}
