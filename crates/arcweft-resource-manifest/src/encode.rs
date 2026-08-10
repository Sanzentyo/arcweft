use crate::wire::{
    RESOURCE_TYPE_MANIFEST_FORMAT, RESOURCE_TYPE_MANIFEST_SCHEMA, TypedResourceTypeManifestV1,
};
use arcweft_manifest_model::canonical_json_bytes;
use arcweft_resource_model::{
    descriptor::{
        ResourceAgentExposure, ResourceFieldPresence, ResourceHotReloadClass,
        ResourceTypeDescriptor, ResourceValueSchema,
    },
    identity::NominalTypeId,
    retained::{PresentationTargetScope, ResolvedRetainedIdentityRef},
    value::{
        ResourceBoundKind, ResourceConstValue, ResourceScalarType, ResourceScalarValue,
        ResourceValueType,
    },
};
use serde_json::{Map, Value, json};
use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ResourceManifestEncodeError {
    #[error("resource manifest cannot be canonically encoded: {0}")]
    Canonical(String),
}

pub fn encode_resource_type_manifest_v1(
    manifest: &TypedResourceTypeManifestV1,
) -> Result<Vec<u8>, ResourceManifestEncodeError> {
    let mut schemas = manifest
        .schemas()
        .iter()
        .map(schema_value)
        .collect::<Vec<_>>();
    schemas.sort_by_key(schema_sort_key);
    let mut resource_types = manifest
        .resource_types()
        .iter()
        .map(descriptor_value)
        .collect::<Vec<_>>();
    resource_types.sort_by_key(descriptor_sort_key);
    let mut codecs = manifest
        .codecs()
        .iter()
        .map(|codec| {
            let versions = codec
                .versions()
                .iter()
                .map(|v| json!(v.get()))
                .collect::<Vec<_>>();
            object([
                ("codec_id", json!(codec.codec_id().as_str())),
                ("versions", Value::Array(versions)),
            ])
        })
        .collect::<Vec<_>>();
    codecs.sort_by_key(|value| value["codec_id"].as_str().unwrap_or_default().to_owned());
    let root = object([
        ("format", json!(RESOURCE_TYPE_MANIFEST_FORMAT)),
        ("schema", json!(RESOURCE_TYPE_MANIFEST_SCHEMA)),
        (
            "package",
            object([
                ("id", json!(manifest.package().id().as_str())),
                ("version", json!(manifest.package().version().to_string())),
            ]),
        ),
        ("schemas", Value::Array(schemas)),
        ("resource_types", Value::Array(resource_types)),
        ("codecs", Value::Array(codecs)),
    ]);
    canonical_json_bytes(&root)
        .map_err(|error| ResourceManifestEncodeError::Canonical(error.to_string()))
}

fn schema_sort_key(value: &Value) -> String {
    value["value"]["schema_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}
fn descriptor_sort_key(value: &Value) -> (String, String, String) {
    let id = &value["type_id"];
    (
        id["package"].as_str().unwrap_or_default().into(),
        id["module"].as_str().unwrap_or_default().into(),
        id["name"].as_str().unwrap_or_default().into(),
    )
}

fn schema_value(schema: &ResourceValueSchema) -> Value {
    match schema {
        ResourceValueSchema::Record(schema) => {
            let mut fields = schema
                .fields()
                .iter()
                .map(|field| {
                    let mut value = Map::new();
                    value.insert("field_id".into(), json!(field.id().get()));
                    value.insert("name".into(), json!(field.name().as_str()));
                    value.insert("value_type".into(), value_type(field.value_type()));
                    value.insert(
                        "presence".into(),
                        json!(match field.presence() {
                            ResourceFieldPresence::Required => "required",
                            ResourceFieldPresence::Optional => "optional",
                        }),
                    );
                    if let Some(default) = field.default() {
                        value.insert("default".into(), const_value(default));
                    }
                    if !field.docs().is_empty() {
                        value.insert("docs".into(), json!(field.docs()));
                    }
                    Value::Object(value)
                })
                .collect::<Vec<_>>();
            fields.sort_by_key(|v| {
                (
                    v["field_id"].as_u64().unwrap_or_default(),
                    v["name"].as_str().unwrap_or_default().to_owned(),
                )
            });
            tagged(
                "record",
                object([
                    ("schema_id", json!(schema.id().as_str())),
                    ("nominal_type", nominal(schema.nominal_type())),
                    ("version", json!(schema.version().get())),
                    ("fields", Value::Array(fields)),
                ]),
            )
        }
        ResourceValueSchema::Enum(schema) => {
            let mut variants = schema
                .variants()
                .iter()
                .map(|variant| {
                    let mut value = Map::new();
                    value.insert("variant_id".into(), json!(variant.id().get()));
                    value.insert("name".into(), json!(variant.name().as_str()));
                    if let Some(payload) = variant.payload() {
                        value.insert("payload".into(), value_type(payload));
                    }
                    if !variant.docs().is_empty() {
                        value.insert("docs".into(), json!(variant.docs()));
                    }
                    Value::Object(value)
                })
                .collect::<Vec<_>>();
            variants.sort_by_key(|v| {
                (
                    v["variant_id"].as_u64().unwrap_or_default(),
                    v["name"].as_str().unwrap_or_default().to_owned(),
                )
            });
            tagged(
                "enum",
                object([
                    ("schema_id", json!(schema.id().as_str())),
                    ("nominal_type", nominal(schema.nominal_type())),
                    ("version", json!(schema.version().get())),
                    ("variants", Value::Array(variants)),
                ]),
            )
        }
    }
}

fn descriptor_value(descriptor: &ResourceTypeDescriptor) -> Value {
    let capabilities = descriptor.capabilities();
    let mut cap = Map::new();
    if let Some(handle) = capabilities.runtime_handle_kind() {
        cap.insert("runtime_handle_kind".into(), json!(handle.as_str()));
    }
    cap.insert(
        "agent_exposure".into(),
        json!(match capabilities.agent_exposure() {
            ResourceAgentExposure::Hidden => "hidden",
            ResourceAgentExposure::Catalog => "catalog",
            ResourceAgentExposure::CatalogAndRuntime => "catalog_and_runtime",
        }),
    );
    cap.insert(
        "save_definition_reference".into(),
        json!(capabilities.saves_definition_reference()),
    );
    cap.insert(
        "hot_reload".into(),
        json!(match capabilities.hot_reload() {
            ResourceHotReloadClass::RestartRequired => "restart_required",
            ResourceHotReloadClass::ReplaceDefinition => "replace_definition",
            ResourceHotReloadClass::UpdateLiveHandle => "update_live_handle",
        }),
    );
    let lowering = descriptor.lowering();
    let mut fields = Map::new();
    fields.insert("type_id".into(), nominal(descriptor.type_id().nominal()));
    fields.insert(
        "public_id_family".into(),
        json!(descriptor.public_id_family().as_str()),
    );
    fields.insert(
        "family_group".into(),
        json!(descriptor.family_group().as_str()),
    );
    fields.insert(
        "body_schema".into(),
        json!(descriptor.body_schema().as_str()),
    );
    fields.insert("capabilities".into(), Value::Object(cap));
    fields.insert(
        "lowering".into(),
        object([
            ("codec_id", json!(lowering.codec_id().as_str())),
            ("codec_version", json!(lowering.codec_version().get())),
            ("section_id", json!(lowering.section_id().as_str())),
            ("section_version", json!(lowering.section_version().get())),
        ]),
    );
    if !descriptor.docs().summary().is_empty() {
        fields.insert(
            "docs".into(),
            object([("summary", json!(descriptor.docs().summary()))]),
        );
    }
    fields.insert(
        "descriptor_digest".into(),
        json!(descriptor.semantic_digest().to_string()),
    );
    Value::Object(fields)
}

fn nominal(value: &NominalTypeId) -> Value {
    object([
        ("package", json!(value.package().as_str())),
        ("module", json!(value.module().as_str())),
        ("name", json!(value.name().as_str())),
    ])
}
fn scalar_type(value: ResourceScalarType) -> &'static str {
    match value {
        ResourceScalarType::Unit => "unit",
        ResourceScalarType::Bool => "bool",
        ResourceScalarType::SignedInteger => "signed_integer",
        ResourceScalarType::UnsignedInteger => "unsigned_integer",
        ResourceScalarType::Float => "float",
        ResourceScalarType::String => "string",
        ResourceScalarType::Char => "char",
        ResourceScalarType::Duration => "duration",
        ResourceScalarType::Ratio => "ratio",
        ResourceScalarType::Length => "length",
        ResourceScalarType::Gain => "gain",
        ResourceScalarType::Pan => "pan",
        ResourceScalarType::Locale => "locale",
        ResourceScalarType::PublicId => "public_id",
    }
}
fn value_type(value: &ResourceValueType) -> Value {
    match value {
        ResourceValueType::Scalar(v) => tagged("scalar", json!(scalar_type(*v))),
        ResourceValueType::Option(v) => tagged("option", value_type(v)),
        ResourceValueType::Vec(v) => tagged("list", value_type(v)),
        ResourceValueType::NonEmptyVec(v) => tagged("non_empty_list", value_type(v)),
        ResourceValueType::Map { key, value } => tagged(
            "ordered_map",
            object([("key", value_type(key)), ("value", value_type(value))]),
        ),
        ResourceValueType::NominalRecord(v) => tagged("record", json!(v.as_str())),
        ResourceValueType::NominalEnum(v) => tagged("enum", json!(v.as_str())),
        ResourceValueType::AssetRef { payload_kind } => tagged(
            "asset_ref",
            object([("payload_kind", json!(payload_kind.as_str()))]),
        ),
        ResourceValueType::ResourceRef { type_id } => tagged(
            "resource_ref",
            object([("type_id", nominal(type_id.nominal()))]),
        ),
        ResourceValueType::RetainedIdentityRef { identity } => {
            tagged("retained_identity_ref", json!(identity.as_str()))
        }
        ResourceValueType::ConstrainedScalar(v) => {
            let mut out = Map::new();
            out.insert("scalar".into(), json!(scalar_type(v.scalar())));
            if let Some(bound) = v.lower() {
                out.insert(
                    "lower".into(),
                    object([
                        (
                            "kind",
                            json!(if bound.kind() == ResourceBoundKind::Inclusive {
                                "inclusive"
                            } else {
                                "exclusive"
                            }),
                        ),
                        ("value", scalar_value(bound.value())),
                    ]),
                );
            }
            if let Some(bound) = v.upper() {
                out.insert(
                    "upper".into(),
                    object([
                        (
                            "kind",
                            json!(if bound.kind() == ResourceBoundKind::Inclusive {
                                "inclusive"
                            } else {
                                "exclusive"
                            }),
                        ),
                        ("value", scalar_value(bound.value())),
                    ]),
                );
            }
            tagged("constrained_scalar", Value::Object(out))
        }
    }
}

fn scalar_value(value: &ResourceScalarValue) -> Value {
    match value {
        ResourceScalarValue::Unit => object([("kind", json!("unit"))]),
        ResourceScalarValue::Bool(v) => tagged("bool", json!(v)),
        ResourceScalarValue::SignedInteger(v) => tagged("signed_integer", json!(v)),
        ResourceScalarValue::UnsignedInteger(v) => tagged("unsigned_integer", json!(v)),
        ResourceScalarValue::Float(v) => tagged("float", json!(format!("0x{:016x}", v.bits()))),
        ResourceScalarValue::String(v) => tagged("string", json!(v)),
        ResourceScalarValue::Char(v) => tagged("char", json!(v.to_string())),
        ResourceScalarValue::Duration(v) => tagged("duration", json!(v.as_nanos())),
        ResourceScalarValue::Ratio(v) => tagged("ratio", json!(v.millionths())),
        ResourceScalarValue::Length(v) => tagged(
            "length",
            object([
                ("milli_units", json!(v.milli_units())),
                (
                    "unit",
                    serde_json::to_value(v.unit()).expect("LayoutUnit is serializable"),
                ),
            ]),
        ),
        ResourceScalarValue::Gain(v) => tagged("gain", json!(v.get())),
        ResourceScalarValue::Pan(v) => tagged("pan", json!(v.get())),
        ResourceScalarValue::Locale(v) => tagged("locale", json!(v.as_str())),
        ResourceScalarValue::PublicId(v) => tagged("public_id", json!(v.as_str())),
    }
}

pub(crate) fn const_value(value: &ResourceConstValue) -> Value {
    match value {
        ResourceConstValue::Scalar(v) => tagged("scalar", scalar_value(v)),
        ResourceConstValue::Option(None) => object([("kind", json!("option"))]),
        ResourceConstValue::Option(Some(v)) => tagged("option", const_value(v)),
        ResourceConstValue::Sequence(v) => {
            tagged("list", Value::Array(v.iter().map(const_value).collect()))
        }
        ResourceConstValue::Map(v) => {
            let mut entries = v
                .entries()
                .iter()
                .map(|(k, v)| object([("key", const_value(k)), ("value", const_value(v))]))
                .collect::<Vec<_>>();
            entries.sort_by_key(|entry| canonical_json_bytes(&entry["key"]).unwrap_or_default());
            tagged("ordered_map", Value::Array(entries))
        }
        ResourceConstValue::Record(v) => {
            let fields = v
                .fields()
                .iter()
                .map(|(id, value)| {
                    object([("field_id", json!(id.get())), ("value", const_value(value))])
                })
                .collect();
            tagged(
                "record",
                object([
                    ("schema_id", json!(v.schema_id().as_str())),
                    ("fields", Value::Array(fields)),
                ]),
            )
        }
        ResourceConstValue::Enum(v) => {
            let mut out = Map::new();
            out.insert("schema_id".into(), json!(v.schema_id().as_str()));
            out.insert("variant_id".into(), json!(v.variant().get()));
            if let Some(payload) = v.payload() {
                out.insert("payload".into(), const_value(payload));
            }
            tagged("enum", Value::Object(out))
        }
        ResourceConstValue::AssetRef(v) => tagged(
            "asset_ref",
            object([
                ("public_id", json!(v.public_id().as_str())),
                ("payload_kind", json!(v.payload_kind().as_str())),
            ]),
        ),
        ResourceConstValue::ResourceRef(v) => tagged(
            "resource_ref",
            object([
                ("entity_id", json!(v.entity_id().as_str())),
                ("public_id", json!(v.public_id().as_str())),
                ("type_id", nominal(v.type_id().nominal())),
            ]),
        ),
        ResourceConstValue::RetainedIdentityRef { value } => {
            tagged("retained_identity_ref", retained(value))
        }
    }
}
fn retained(value: &ResolvedRetainedIdentityRef) -> Value {
    match value {
        ResolvedRetainedIdentityRef::Character { entity_id } => tagged(
            "character",
            object([("entity_id", json!(entity_id.as_str()))]),
        ),
        ResolvedRetainedIdentityRef::View { entity_id } => {
            tagged("view", object([("entity_id", json!(entity_id.as_str()))]))
        }
        ResolvedRetainedIdentityRef::Action { entity_id } => {
            tagged("action", object([("entity_id", json!(entity_id.as_str()))]))
        }
        ResolvedRetainedIdentityRef::Layer { entity_id } => {
            tagged("layer", object([("entity_id", json!(entity_id.as_str()))]))
        }
        ResolvedRetainedIdentityRef::Signal { entity_id } => {
            tagged("signal", object([("entity_id", json!(entity_id.as_str()))]))
        }
        ResolvedRetainedIdentityRef::PresentationTarget { scope, target_id } => {
            let scope = match scope {
                PresentationTargetScope::Global => object([("kind", json!("global"))]),
                PresentationTargetScope::View {
                    owner_view_entity_id,
                } => tagged(
                    "view",
                    object([("owner_view_entity_id", json!(owner_view_entity_id.as_str()))]),
                ),
            };
            tagged(
                "presentation_target",
                object([("scope", scope), ("target_id", json!(target_id.as_str()))]),
            )
        }
        ResolvedRetainedIdentityRef::ScrollRegion {
            owner_view_entity_id,
            region_id,
        } => tagged(
            "scroll_region",
            object([
                ("owner_view_entity_id", json!(owner_view_entity_id.as_str())),
                ("region_id", json!(region_id.as_str())),
            ]),
        ),
    }
}
fn tagged(kind: &str, value: Value) -> Value {
    object([("kind", json!(kind)), ("value", value)])
}
fn object<const N: usize>(entries: [(&str, Value); N]) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect::<Map<_, _>>(),
    )
}
