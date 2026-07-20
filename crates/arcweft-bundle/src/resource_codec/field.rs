use super::budget::{SectionCodecBudget, check_budget};
use super::codec_io::{Cursor, u32_from_usize, usize_from_u32};
use super::error::SectionCodecError;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const FIELD_HEADER_LEN: usize = 12;
const FIELD_REQUIRED_FLAG: u8 = 0b0000_0001;

/// Field identifier inside a compact product resource section family.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    serde::Deserialize,
    serde::Serialize,
)]
pub struct FieldId(pub u16);

/// Common field requirement flag used for forward compatibility.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldRequirement {
    Optional,
    Required,
}

/// Shared scalar/container payload tags for common field headers.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceWireType {
    Bytes,
    U32,
    U64,
    I64,
    Bool,
    StringRef,
    PublicIdRef,
    StableId,
    DigestRef,
    SourceRangeRef,
    CrossSectionRef,
    Nested,
}

/// Known field contract for one section-family codec.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct FieldSpec {
    pub id: FieldId,
    pub requirement: FieldRequirement,
    pub wire_type: ResourceWireType,
}

/// Field registry supplied by the owning section-family codec.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FieldRegistry {
    specs: BTreeMap<FieldId, FieldSpec>,
}

/// Common field payload with enough metadata for deterministic skip/reject
/// decisions before a section-family decoder interprets its bytes.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ResourceField {
    pub id: FieldId,
    pub requirement: FieldRequirement,
    pub wire_type: ResourceWireType,
    pub nesting_depth: u16,
    pub reference_count: u16,
    pub payload: Vec<u8>,
}

impl FieldSpec {
    pub const fn optional(id: FieldId, wire_type: ResourceWireType) -> Self {
        Self {
            id,
            requirement: FieldRequirement::Optional,
            wire_type,
        }
    }

    pub const fn required(id: FieldId, wire_type: ResourceWireType) -> Self {
        Self {
            id,
            requirement: FieldRequirement::Required,
            wire_type,
        }
    }
}

impl FieldRegistry {
    pub fn new(specs: impl IntoIterator<Item = FieldSpec>) -> Result<Self, SectionCodecError> {
        let mut map = BTreeMap::new();
        for spec in specs {
            if map.insert(spec.id, spec).is_some() {
                return Err(SectionCodecError::DuplicateFieldSpec(spec.id));
            }
        }
        Ok(Self { specs: map })
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn spec(&self, id: FieldId) -> Option<FieldSpec> {
        self.specs.get(&id).copied()
    }

    pub(crate) fn validate_known_field(
        &self,
        field: &ResourceField,
    ) -> Result<bool, SectionCodecError> {
        let Some(spec) = self.spec(field.id) else {
            return Ok(false);
        };
        if spec.requirement != field.requirement {
            return Err(SectionCodecError::FieldRequirementMismatch {
                field: field.id,
                expected: spec.requirement,
                actual: field.requirement,
            });
        }
        if spec.wire_type != field.wire_type {
            return Err(SectionCodecError::FieldWireTypeMismatch {
                field: field.id,
                expected: spec.wire_type,
                actual: field.wire_type,
            });
        }
        Ok(true)
    }

    pub(crate) fn validate_required_presence(
        &self,
        fields: &[ResourceField],
    ) -> Result<(), SectionCodecError> {
        let present = fields.iter().map(|field| field.id).collect::<BTreeSet<_>>();
        self.specs
            .values()
            .filter(|spec| spec.requirement == FieldRequirement::Required)
            .find(|spec| !present.contains(&spec.id))
            .map_or(Ok(()), |missing| {
                Err(SectionCodecError::MissingRequiredField(missing.id))
            })
    }
}

impl ResourceWireType {
    pub const fn encoded(self) -> u8 {
        match self {
            Self::Bytes => 1,
            Self::U32 => 2,
            Self::U64 => 3,
            Self::I64 => 4,
            Self::Bool => 5,
            Self::StringRef => 6,
            Self::PublicIdRef => 7,
            Self::StableId => 8,
            Self::DigestRef => 9,
            Self::SourceRangeRef => 10,
            Self::CrossSectionRef => 11,
            Self::Nested => 12,
        }
    }

    pub const fn from_encoded(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Bytes),
            2 => Some(Self::U32),
            3 => Some(Self::U64),
            4 => Some(Self::I64),
            5 => Some(Self::Bool),
            6 => Some(Self::StringRef),
            7 => Some(Self::PublicIdRef),
            8 => Some(Self::StableId),
            9 => Some(Self::DigestRef),
            10 => Some(Self::SourceRangeRef),
            11 => Some(Self::CrossSectionRef),
            12 => Some(Self::Nested),
            _ => None,
        }
    }
}

impl ResourceField {
    pub fn optional(id: FieldId, wire_type: ResourceWireType, payload: impl Into<Vec<u8>>) -> Self {
        Self::new(id, FieldRequirement::Optional, wire_type, 0, 0, payload)
    }

    pub fn required(id: FieldId, wire_type: ResourceWireType, payload: impl Into<Vec<u8>>) -> Self {
        Self::new(id, FieldRequirement::Required, wire_type, 0, 0, payload)
    }

    pub fn new(
        id: FieldId,
        requirement: FieldRequirement,
        wire_type: ResourceWireType,
        nesting_depth: u16,
        reference_count: u16,
        payload: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            id,
            requirement,
            wire_type,
            nesting_depth,
            reference_count,
            payload: payload.into(),
        }
    }

    pub const fn is_required(&self) -> bool {
        matches!(self.requirement, FieldRequirement::Required)
    }

    pub(crate) fn canonical_key(&self) -> (u16, u8, u8, u16, u16, &[u8]) {
        (
            self.id.0,
            self.wire_type.encoded(),
            u8::from(self.is_required()),
            self.nesting_depth,
            self.reference_count,
            &self.payload,
        )
    }

    pub(crate) fn encode_into(&self, out: &mut Vec<u8>) -> Result<(), SectionCodecError> {
        let payload_len = u32_from_usize(self.payload.len())?;
        out.extend_from_slice(&self.id.0.to_le_bytes());
        out.push(self.wire_type.encoded());
        out.push(if self.is_required() {
            FIELD_REQUIRED_FLAG
        } else {
            0
        });
        out.extend_from_slice(&self.nesting_depth.to_le_bytes());
        out.extend_from_slice(&self.reference_count.to_le_bytes());
        out.extend_from_slice(&payload_len.to_le_bytes());
        out.extend_from_slice(&self.payload);
        Ok(())
    }

    pub(crate) fn decode_from(cursor: &mut Cursor<'_>) -> Result<Self, SectionCodecError> {
        let id = FieldId(cursor.read_u16()?);
        let wire_type_tag = cursor.read_u8()?;
        let wire_type = ResourceWireType::from_encoded(wire_type_tag)
            .ok_or(SectionCodecError::UnsupportedWireType(wire_type_tag))?;
        let flags = cursor.read_u8()?;
        if flags & !FIELD_REQUIRED_FLAG != 0 {
            return Err(SectionCodecError::InvalidFieldFlags(flags));
        }
        let requirement = if flags & FIELD_REQUIRED_FLAG == FIELD_REQUIRED_FLAG {
            FieldRequirement::Required
        } else {
            FieldRequirement::Optional
        };
        let nesting_depth = cursor.read_u16()?;
        let reference_count = cursor.read_u16()?;
        let payload_len = cursor.read_u32()?;
        let payload = cursor.read_bytes(usize_from_u32(payload_len)?)?.to_vec();
        Ok(Self {
            id,
            requirement,
            wire_type,
            nesting_depth,
            reference_count,
            payload,
        })
    }
}

pub(crate) fn validate_field_budgets(
    fields: &[ResourceField],
    budget: SectionCodecBudget,
) -> Result<(), SectionCodecError> {
    check_budget(fields.len(), budget.items, "items")?;
    let max_depth = fields
        .iter()
        .map(|field| field.nesting_depth as usize)
        .max()
        .unwrap_or(0);
    check_budget(max_depth, budget.depth, "depth")?;
    let references = fields
        .iter()
        .map(|field| field.reference_count as usize)
        .sum::<usize>();
    check_budget(references, budget.references, "references")
}

pub(crate) fn validate_strict_field_order(
    fields: &[ResourceField],
) -> Result<(), SectionCodecError> {
    if let Some(pair) = fields
        .windows(2)
        .find(|pair| pair[0].canonical_key() >= pair[1].canonical_key())
    {
        return Err(SectionCodecError::NonCanonicalFieldOrder {
            previous: pair[0].id,
            current: pair[1].id,
        });
    }
    Ok(())
}
