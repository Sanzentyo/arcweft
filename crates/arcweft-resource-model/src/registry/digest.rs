use super::{ResourceSchemaDigest, ResourceTypeDescriptorDigest, ResourceTypeRegistryDigest};
use crate::descriptor::{
    ResourceAgentExposure, ResourceCapabilities, ResourceCodecSupport, ResourceEnumSchema,
    ResourceFieldDescriptor, ResourceFieldPresence, ResourceHotReloadClass,
    ResourceLoweringBinding, ResourceRecordSchema, ResourceTypeDescriptor, ResourceValueSchema,
    ResourceVariantDescriptor,
};
use crate::identity::{NominalTypeId, ResourceCodecId, ResourceSchemaId, ResourceTypeId};
use crate::retained::RetainedIdentityKind;
use crate::value::{
    ResourceBoundKind, ResourceConstValue, ResourceEnumValue, ResourceMapValue,
    ResourceRecordValue, ResourceScalarBound, ResourceScalarConstraint, ResourceScalarType,
    ResourceScalarValue, ResourceValueType,
};
use arcweft_layout::LayoutUnit;
use arcweft_manifest_model::SemanticDigest;
use std::collections::BTreeMap;

const SCHEMA_DIGEST_CONTEXT: &str = "arcweft-resource-value-schema-v1";
const DESCRIPTOR_DIGEST_CONTEXT: &str = "arcweft-resource-type-descriptor-v1";
const REGISTRY_DIGEST_CONTEXT: &str = "arcweft-resource-type-registry-v1";

pub(super) fn schema_digest(schema: &ResourceValueSchema) -> ResourceSchemaDigest {
    let mut encoder = CanonicalEncoder::default();
    encode_schema(&mut encoder, schema);
    ResourceSchemaDigest(SemanticDigest::derive(
        SCHEMA_DIGEST_CONTEXT,
        encoder.as_bytes(),
    ))
}

pub(super) fn descriptor_digest(
    descriptor: &ResourceTypeDescriptor,
) -> ResourceTypeDescriptorDigest {
    let mut encoder = CanonicalEncoder::default();
    encode_descriptor(&mut encoder, descriptor);
    ResourceTypeDescriptorDigest(SemanticDigest::derive(
        DESCRIPTOR_DIGEST_CONTEXT,
        encoder.as_bytes(),
    ))
}

pub(super) fn descriptor_digest_transcript_len(descriptor: &ResourceTypeDescriptor) -> usize {
    let mut encoder = CanonicalEncoder::default();
    encode_descriptor(&mut encoder, descriptor);
    encoder.as_bytes().len()
}

pub(super) fn registry_digest(
    manifest_schema_version: u32,
    schemas: &BTreeMap<ResourceSchemaId, ResourceValueSchema>,
    resource_types: &BTreeMap<ResourceTypeId, ResourceTypeDescriptor>,
    codecs: &BTreeMap<ResourceCodecId, ResourceCodecSupport>,
) -> ResourceTypeRegistryDigest {
    let mut encoder = CanonicalEncoder::default();
    encoder.u32(manifest_schema_version);
    encoder.len(schemas.len());
    for (schema_id, schema) in schemas {
        encoder.string(schema_id.as_str());
        encoder.bytes(schema_digest(schema).semantic_digest().as_bytes());
        encode_nominal_type(&mut encoder, schema.nominal_type());
    }
    encoder.len(resource_types.len());
    for descriptor in resource_types.values() {
        encode_descriptor(&mut encoder, descriptor);
    }
    encoder.len(codecs.len());
    for codec in codecs.values() {
        encoder.string(codec.codec_id().as_str());
        encoder.len(codec.versions().len());
        for version in codec.versions() {
            encoder.u32(version.get());
        }
    }
    ResourceTypeRegistryDigest(SemanticDigest::derive(
        REGISTRY_DIGEST_CONTEXT,
        encoder.as_bytes(),
    ))
}

fn encode_schema(encoder: &mut CanonicalEncoder, schema: &ResourceValueSchema) {
    match schema {
        ResourceValueSchema::Record(schema) => {
            encoder.u8(0);
            encode_record_schema(encoder, schema);
        }
        ResourceValueSchema::Enum(schema) => {
            encoder.u8(1);
            encode_enum_schema(encoder, schema);
        }
    }
}

fn encode_record_schema(encoder: &mut CanonicalEncoder, schema: &ResourceRecordSchema) {
    encoder.string(schema.id().as_str());
    encode_nominal_type(encoder, schema.nominal_type());
    encoder.u32(schema.version().get());
    encoder.len(schema.fields().len());
    for field in schema.fields() {
        encode_field(encoder, field);
    }
}

fn encode_enum_schema(encoder: &mut CanonicalEncoder, schema: &ResourceEnumSchema) {
    encoder.string(schema.id().as_str());
    encode_nominal_type(encoder, schema.nominal_type());
    encoder.u32(schema.version().get());
    encoder.len(schema.variants().len());
    for variant in schema.variants() {
        encode_variant(encoder, variant);
    }
}

fn encode_field(encoder: &mut CanonicalEncoder, field: &ResourceFieldDescriptor) {
    encoder.u32(field.id().get());
    encoder.string(field.name().as_str());
    encode_value_type(encoder, field.value_type());
    encoder.u8(match field.presence() {
        ResourceFieldPresence::Required => 0,
        ResourceFieldPresence::Optional => 1,
    });
    encode_optional(encoder, field.default(), encode_const_value);
}

fn encode_variant(encoder: &mut CanonicalEncoder, variant: &ResourceVariantDescriptor) {
    encoder.u32(variant.id().get());
    encoder.string(variant.name().as_str());
    encode_optional(encoder, variant.payload(), encode_value_type);
}

fn encode_descriptor(encoder: &mut CanonicalEncoder, descriptor: &ResourceTypeDescriptor) {
    encode_resource_type(encoder, descriptor.type_id());
    encoder.string(descriptor.public_id_family().as_str());
    encoder.string(descriptor.family_group().as_str());
    encoder.string(descriptor.body_schema().as_str());
    encode_capabilities(encoder, descriptor.capabilities());
    encode_lowering(encoder, descriptor.lowering());
}

fn encode_capabilities(encoder: &mut CanonicalEncoder, capabilities: &ResourceCapabilities) {
    encode_optional(
        encoder,
        capabilities.runtime_handle_kind(),
        |encoder, handle| encoder.string(handle.as_str()),
    );
    encoder.u8(match capabilities.agent_exposure() {
        ResourceAgentExposure::Hidden => 0,
        ResourceAgentExposure::Catalog => 1,
        ResourceAgentExposure::CatalogAndRuntime => 2,
    });
    encoder.bool(capabilities.saves_definition_reference());
    encoder.u8(match capabilities.hot_reload() {
        ResourceHotReloadClass::RestartRequired => 0,
        ResourceHotReloadClass::ReplaceDefinition => 1,
        ResourceHotReloadClass::UpdateLiveHandle => 2,
    });
}

fn encode_lowering(encoder: &mut CanonicalEncoder, lowering: &ResourceLoweringBinding) {
    encoder.string(lowering.codec_id().as_str());
    encoder.u32(lowering.codec_version().get());
    encoder.string(lowering.section_id().as_str());
    encoder.u32(lowering.section_version().get());
}

fn encode_value_type(encoder: &mut CanonicalEncoder, value_type: &ResourceValueType) {
    match value_type {
        ResourceValueType::Scalar(scalar) => {
            encoder.u8(0);
            encode_scalar_type(encoder, *scalar);
        }
        ResourceValueType::Option(value) => {
            encoder.u8(1);
            encode_value_type(encoder, value);
        }
        ResourceValueType::Vec(value) => {
            encoder.u8(2);
            encode_value_type(encoder, value);
        }
        ResourceValueType::NonEmptyVec(value) => {
            encoder.u8(3);
            encode_value_type(encoder, value);
        }
        ResourceValueType::Map { key, value } => {
            encoder.u8(4);
            encode_value_type(encoder, key);
            encode_value_type(encoder, value);
        }
        ResourceValueType::NominalRecord(schema) => {
            encoder.u8(5);
            encoder.string(schema.as_str());
        }
        ResourceValueType::NominalEnum(schema) => {
            encoder.u8(6);
            encoder.string(schema.as_str());
        }
        ResourceValueType::AssetRef { payload_kind } => {
            encoder.u8(7);
            encoder.string(payload_kind.as_str());
        }
        ResourceValueType::ResourceRef { type_id } => {
            encoder.u8(8);
            encode_resource_type(encoder, type_id);
        }
        ResourceValueType::ConstrainedScalar(constraint) => {
            encoder.u8(9);
            encode_constraint(encoder, constraint);
        }
        ResourceValueType::RetainedIdentityRef { identity } => {
            encoder.u8(10);
            encode_retained_identity_kind(encoder, *identity);
        }
    }
}

fn encode_constraint(encoder: &mut CanonicalEncoder, constraint: &ResourceScalarConstraint) {
    encode_scalar_type(encoder, constraint.scalar());
    encode_optional(encoder, constraint.lower(), encode_bound);
    encode_optional(encoder, constraint.upper(), encode_bound);
}

fn encode_bound(encoder: &mut CanonicalEncoder, bound: &ResourceScalarBound) {
    encoder.u8(match bound.kind() {
        ResourceBoundKind::Inclusive => 0,
        ResourceBoundKind::Exclusive => 1,
    });
    encode_scalar_value(encoder, bound.value());
}

fn encode_const_value(encoder: &mut CanonicalEncoder, value: &ResourceConstValue) {
    match value {
        ResourceConstValue::Scalar(value) => {
            encoder.u8(0);
            encode_scalar_value(encoder, value);
        }
        ResourceConstValue::Option(value) => {
            encoder.u8(1);
            encode_optional(encoder, value.as_deref(), encode_const_value);
        }
        ResourceConstValue::Sequence(values) => {
            encoder.u8(2);
            encoder.len(values.len());
            for value in values {
                encode_const_value(encoder, value);
            }
        }
        ResourceConstValue::Map(value) => {
            encoder.u8(3);
            encode_map_value(encoder, value);
        }
        ResourceConstValue::Record(value) => {
            encoder.u8(4);
            encode_record_value(encoder, value);
        }
        ResourceConstValue::Enum(value) => {
            encoder.u8(5);
            encode_enum_value(encoder, value);
        }
        ResourceConstValue::AssetRef(value) => {
            encoder.u8(6);
            encoder.string(value.public_id().as_str());
            encoder.string(value.payload_kind().as_str());
        }
        ResourceConstValue::ResourceRef(value) => {
            encoder.u8(7);
            encoder.string(value.entity_id().as_str());
            encoder.string(value.public_id().as_str());
            encode_resource_type(encoder, value.type_id());
        }
        ResourceConstValue::RetainedIdentityRef { .. } => {
            encoder.u8(8);
            let canonical = value
                .canonical_bytes_v1()
                .expect("validated retained identities always fit the canonical transcript");
            encoder.bytes(&canonical);
        }
    }
}

fn encode_retained_identity_kind(encoder: &mut CanonicalEncoder, identity: RetainedIdentityKind) {
    encoder.string(identity.as_str());
}

fn encode_map_value(encoder: &mut CanonicalEncoder, value: &ResourceMapValue) {
    encoder.len(value.entries().len());
    let mut entries = value
        .entries()
        .iter()
        .map(|(key, value)| {
            let mut key_encoder = CanonicalEncoder::default();
            encode_const_value(&mut key_encoder, key);
            (key_encoder.bytes, key, value)
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    for (_, key, value) in entries {
        encode_const_value(encoder, key);
        encode_const_value(encoder, value);
    }
}

fn encode_record_value(encoder: &mut CanonicalEncoder, value: &ResourceRecordValue) {
    encoder.string(value.schema_id().as_str());
    encoder.len(value.fields().len());
    for (field, value) in value.fields() {
        encoder.u32(field.get());
        encode_const_value(encoder, value);
    }
}

fn encode_enum_value(encoder: &mut CanonicalEncoder, value: &ResourceEnumValue) {
    encoder.string(value.schema_id().as_str());
    encoder.u32(value.variant().get());
    encode_optional(encoder, value.payload(), encode_const_value);
}

fn encode_scalar_type(encoder: &mut CanonicalEncoder, scalar: ResourceScalarType) {
    encoder.u8(match scalar {
        ResourceScalarType::Unit => 0,
        ResourceScalarType::Bool => 1,
        ResourceScalarType::SignedInteger => 2,
        ResourceScalarType::UnsignedInteger => 3,
        ResourceScalarType::Float => 4,
        ResourceScalarType::String => 5,
        ResourceScalarType::Char => 6,
        ResourceScalarType::Duration => 7,
        ResourceScalarType::Ratio => 8,
        ResourceScalarType::Length => 9,
        ResourceScalarType::Gain => 10,
        ResourceScalarType::Pan => 11,
        ResourceScalarType::Locale => 12,
        ResourceScalarType::PublicId => 13,
    });
}

fn encode_scalar_value(encoder: &mut CanonicalEncoder, value: &ResourceScalarValue) {
    encode_scalar_type(encoder, value.scalar_type());
    match value {
        ResourceScalarValue::Unit => {}
        ResourceScalarValue::Bool(value) => encoder.bool(*value),
        ResourceScalarValue::SignedInteger(value) => encoder.i64(*value),
        ResourceScalarValue::UnsignedInteger(value) => encoder.u64(*value),
        ResourceScalarValue::Float(value) => encoder.u64(value.bits()),
        ResourceScalarValue::String(value) => encoder.string(value),
        ResourceScalarValue::Char(value) => encoder.u32(u32::from(*value)),
        ResourceScalarValue::Duration(value) => encoder.u64(value.as_nanos()),
        ResourceScalarValue::Ratio(value) => encoder.u32(value.millionths()),
        ResourceScalarValue::Length(value) => {
            encoder.i64(value.milli_units());
            encoder.u8(length_unit_tag(value.unit()));
        }
        ResourceScalarValue::Gain(value) => encoder.i32(value.get()),
        ResourceScalarValue::Pan(value) => encoder.i16(value.get()),
        ResourceScalarValue::Locale(value) => encoder.string(value.as_str()),
        ResourceScalarValue::PublicId(value) => encoder.string(value.as_str()),
    }
}

fn length_unit_tag(unit: LayoutUnit) -> u8 {
    match unit {
        LayoutUnit::Px => 0,
        LayoutUnit::Sp => 1,
        LayoutUnit::Percent => 2,
        LayoutUnit::Vw => 3,
        LayoutUnit::Vh => 4,
        LayoutUnit::Cw => 5,
        LayoutUnit::Ch => 6,
        LayoutUnit::Em => 7,
        LayoutUnit::GlyphCh => 8,
        LayoutUnit::SafeAreaTop => 9,
        LayoutUnit::SafeAreaRight => 10,
        LayoutUnit::SafeAreaBottom => 11,
        LayoutUnit::SafeAreaLeft => 12,
    }
}

fn encode_resource_type(encoder: &mut CanonicalEncoder, type_id: &ResourceTypeId) {
    encode_nominal_type(encoder, type_id.nominal());
}

fn encode_nominal_type(encoder: &mut CanonicalEncoder, type_id: &NominalTypeId) {
    encoder.string(type_id.package().as_str());
    encoder.string(type_id.module().as_str());
    encoder.string(type_id.name().as_str());
}

fn encode_optional<T>(
    encoder: &mut CanonicalEncoder,
    value: Option<&T>,
    encode: impl FnOnce(&mut CanonicalEncoder, &T),
) {
    if let Some(value) = value {
        encoder.u8(1);
        encode(encoder, value);
    } else {
        encoder.u8(0);
    }
}

#[derive(Default)]
struct CanonicalEncoder {
    bytes: Vec<u8>,
}

impl CanonicalEncoder {
    fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn i16(&mut self, value: i16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn len(&mut self, mut value: usize) {
        // Canonical unsigned LEB128 keeps equal logical lengths identical on
        // every supported pointer width without a fallible integer cast.
        loop {
            let low = value.to_le_bytes()[0] & 0x7f;
            value >>= 7;
            self.u8(if value == 0 { low } else { low | 0x80 });
            if value == 0 {
                break;
            }
        }
    }

    fn string(&mut self, value: &str) {
        self.len(value.len());
        self.bytes.extend_from_slice(value.as_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.len(value.len());
        self.bytes.extend_from_slice(value);
    }
}

#[cfg(test)]
mod tests {
    use super::{CanonicalEncoder, encode_const_value, encode_map_value};
    use crate::retained::ResolvedRetainedIdentityRef;
    use crate::value::{ResourceConstValue, ResourceMapValue, ResourceScalarValue};
    use arcweft_id::EntityId;

    #[test]
    fn map_digest_order_is_canonical_key_bytes_not_rust_value_order() {
        let one = ResourceConstValue::Scalar(ResourceScalarValue::SignedInteger(1));
        let two_hundred_fifty_six =
            ResourceConstValue::Scalar(ResourceScalarValue::SignedInteger(256));
        assert!(one < two_hundred_fifty_six);

        let true_value = ResourceConstValue::Scalar(ResourceScalarValue::Bool(true));
        let false_value = ResourceConstValue::Scalar(ResourceScalarValue::Bool(false));
        let map = ResourceMapValue::try_new([
            (one.clone(), true_value.clone()),
            (two_hundred_fifty_six.clone(), false_value.clone()),
        ])
        .unwrap();

        let mut actual = CanonicalEncoder::default();
        encode_map_value(&mut actual, &map);

        let mut expected = CanonicalEncoder::default();
        expected.len(2);
        encode_const_value(&mut expected, &two_hundred_fifty_six);
        encode_const_value(&mut expected, &false_value);
        encode_const_value(&mut expected, &one);
        encode_const_value(&mut expected, &true_value);

        assert_eq!(actual.as_bytes(), expected.as_bytes());
    }

    #[test]
    fn retained_constant_digest_embeds_the_frozen_standalone_transcript() {
        let value = ResourceConstValue::RetainedIdentityRef {
            value: ResolvedRetainedIdentityRef::Character {
                entity_id: EntityId::try_new("entity.character.alice").unwrap(),
            },
        };
        let canonical = value.canonical_bytes_v1().unwrap();

        let mut actual = CanonicalEncoder::default();
        encode_const_value(&mut actual, &value);

        let mut expected = CanonicalEncoder::default();
        expected.u8(8);
        expected.bytes(&canonical);
        assert_eq!(actual.as_bytes(), expected.as_bytes());
    }
}
