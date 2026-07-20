use crate::identity::{
    ResourceAssetPayloadKindId, ResourceFieldId, ResourceSchemaId, ResourceTypeId,
    ResourceVariantId,
};
use crate::retained::{ResolvedRetainedIdentityRef, RetainedIdentityKind};
use arcweft_core::{locale::LocaleId, time::LogicalDuration};
use arcweft_id::{EntityId, PublicId};
use arcweft_interaction_model::audio::{GainDbMilli, PanMilli};
use arcweft_layout::LayoutUnit;
use core::cmp::Ordering;
use std::collections::BTreeMap;
use thiserror::Error;

mod reference;

pub use reference::{
    ResourceReferenceRequirement, ResourceReferenceRequirementKind,
    ResourceReferenceTraversalError, ResourceSchemaError, ResourceValueTypePath,
    ResourceValueTypePathSegment,
};

pub(crate) const MAX_RESOURCE_VALUE_NESTING: usize = 64;

/// Closed scalar inventory accepted by resource schemas.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceScalarType {
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

/// Deterministic finite IEEE-754 value represented by canonical bits.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ResourceFloat(u64);

/// Ratio in millionths, constrained to the inclusive range `[0, 1]`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceRatio(u32);

/// Typed fixed-point length.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceLength {
    milli_units: i64,
    unit: LayoutUnit,
}

/// Closed scalar value inventory used by typed resource constants.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceScalarValue {
    Unit,
    Bool(bool),
    SignedInteger(i64),
    UnsignedInteger(u64),
    Float(ResourceFloat),
    String(String),
    Char(char),
    Duration(LogicalDuration),
    Ratio(ResourceRatio),
    Length(ResourceLength),
    Gain(GainDbMilli),
    Pan(PanMilli),
    Locale(LocaleId),
    PublicId(PublicId),
}

/// Inclusive or exclusive scalar constraint edge.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceBoundKind {
    Inclusive,
    Exclusive,
}

/// One typed scalar bound.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceScalarBound {
    value: ResourceScalarValue,
    kind: ResourceBoundKind,
}

/// Validated lower and upper bounds for one scalar type.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceScalarConstraint {
    scalar: ResourceScalarType,
    lower: Option<ResourceScalarBound>,
    upper: Option<ResourceScalarBound>,
}

/// Closed structural type accepted by a generic resource schema.
///
/// Resource, asset, and retained-identity references remain disjoint exact
/// categories.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceValueType {
    Scalar(ResourceScalarType),
    Option(Box<Self>),
    Vec(Box<Self>),
    NonEmptyVec(Box<Self>),
    Map {
        key: Box<Self>,
        value: Box<Self>,
    },
    NominalRecord(ResourceSchemaId),
    NominalEnum(ResourceSchemaId),
    AssetRef {
        payload_kind: ResourceAssetPayloadKindId,
    },
    ResourceRef {
        type_id: ResourceTypeId,
    },
    RetainedIdentityRef {
        identity: RetainedIdentityKind,
    },
    ConstrainedScalar(ResourceScalarConstraint),
}

/// Exact reference to one packaged asset and its validated payload kind.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceAssetRefValue {
    public_id: PublicId,
    payload_kind: ResourceAssetPayloadKindId,
}

/// Exact reference to one accepted configured resource declaration.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceRefValue {
    entity: EntityId,
    public: PublicId,
    resource_type: ResourceTypeId,
}

/// Canonically ordered map value.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceMapValue(BTreeMap<ResourceConstValue, ResourceConstValue>);

/// One nominal record constant with canonically ordered stable field IDs.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceRecordValue {
    schema_id: ResourceSchemaId,
    fields: BTreeMap<ResourceFieldId, ResourceConstValue>,
}

/// One nominal enum constant with an optional typed payload.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceEnumValue {
    schema_id: ResourceSchemaId,
    variant: ResourceVariantId,
    payload: Option<Box<ResourceConstValue>>,
}

/// Closed build-time constant inventory accepted by resource descriptors.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceConstValue {
    Scalar(ResourceScalarValue),
    Option(Option<Box<Self>>),
    Sequence(Vec<Self>),
    Map(ResourceMapValue),
    Record(ResourceRecordValue),
    Enum(ResourceEnumValue),
    AssetRef(ResourceAssetRefValue),
    ResourceRef(ResourceRefValue),
    RetainedIdentityRef { value: ResolvedRetainedIdentityRef },
}

/// Coarse constant kind used by structured type mismatch evidence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceConstValueKind {
    Scalar(ResourceScalarType),
    Option,
    Sequence,
    Map,
    Record,
    Enum,
    AssetRef,
    ResourceRef,
    RetainedIdentityRef,
}

/// Nested location of a typed resource constant validation failure.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceValidationPathSegment {
    OptionValue,
    SequenceIndex(usize),
    MapKey(usize),
    MapValue(usize),
    RecordField(ResourceFieldId),
    EnumPayload,
}

/// Which constraint edge rejected a scalar constant.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceConstraintSide {
    Lower,
    Upper,
}

/// Invalid floating-point or ratio scalar construction.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ResourceScalarConstructionError {
    #[error("resource float must be finite")]
    NonFiniteFloat,
    #[error("resource ratio millionths must be within 0..=1_000_000, got {value}")]
    RatioOutOfRange { value: u32 },
}

/// Invalid scalar constraint construction.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ResourceConstraintError {
    #[error("constraint bound has scalar type {actual:?}, expected {expected:?}")]
    BoundTypeMismatch {
        expected: ResourceScalarType,
        actual: ResourceScalarType,
    },
    #[error("constraint lower bound is greater than its upper bound")]
    Inverted,
    #[error("constraint describes an empty interval")]
    Empty,
}

/// Invalid canonical map or nominal record construction.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ResourceConstConstructionError {
    #[error("resource map contains a duplicate key")]
    DuplicateMapKey { key: ResourceConstValue },
    #[error("resource record contains duplicate field ID {field}")]
    DuplicateRecordField { field: ResourceFieldId },
}

/// Typed structural mismatch between a resource schema and a constant value.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ResourceValueValidationError {
    #[error("resource constant kind {actual:?} does not match {expected:?}")]
    TypeMismatch {
        expected: ResourceValueType,
        actual: ResourceConstValueKind,
    },
    #[error("non-empty sequence value is empty")]
    EmptyNonEmptyVec,
    #[error("resource scalar violates the {side:?} {kind:?} constraint")]
    ConstraintViolation {
        side: ResourceConstraintSide,
        kind: ResourceBoundKind,
    },
    #[error("resource constant nesting exceeds the supported depth")]
    NestingTooDeep,
    #[error("retained identity kind {actual:?} does not match {expected:?}")]
    RetainedIdentityKindMismatch {
        expected: RetainedIdentityKind,
        actual: RetainedIdentityKind,
    },
    #[error("nested resource constant is invalid at {segment:?}: {source}")]
    Nested {
        segment: ResourceValidationPathSegment,
        source: Box<Self>,
    },
}

impl ResourceFloat {
    pub fn try_new(value: f64) -> Result<Self, ResourceScalarConstructionError> {
        if !value.is_finite() {
            return Err(ResourceScalarConstructionError::NonFiniteFloat);
        }
        let canonical = if value == 0.0 { 0.0 } else { value };
        Ok(Self(canonical.to_bits()))
    }

    pub const fn bits(self) -> u64 {
        self.0
    }

    pub fn get(self) -> f64 {
        f64::from_bits(self.0)
    }
}

impl PartialOrd for ResourceFloat {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ResourceFloat {
    fn cmp(&self, other: &Self) -> Ordering {
        self.get().total_cmp(&other.get())
    }
}

impl ResourceRatio {
    pub fn try_from_millionths(millionths: u32) -> Result<Self, ResourceScalarConstructionError> {
        if millionths > 1_000_000 {
            return Err(ResourceScalarConstructionError::RatioOutOfRange { value: millionths });
        }
        Ok(Self(millionths))
    }

    pub const fn millionths(self) -> u32 {
        self.0
    }
}

impl ResourceLength {
    pub const fn new(milli_units: i64, unit: LayoutUnit) -> Self {
        Self { milli_units, unit }
    }

    pub const fn milli_units(self) -> i64 {
        self.milli_units
    }

    pub const fn unit(self) -> LayoutUnit {
        self.unit
    }
}

impl ResourceScalarValue {
    pub const fn scalar_type(&self) -> ResourceScalarType {
        match self {
            Self::Unit => ResourceScalarType::Unit,
            Self::Bool(_) => ResourceScalarType::Bool,
            Self::SignedInteger(_) => ResourceScalarType::SignedInteger,
            Self::UnsignedInteger(_) => ResourceScalarType::UnsignedInteger,
            Self::Float(_) => ResourceScalarType::Float,
            Self::String(_) => ResourceScalarType::String,
            Self::Char(_) => ResourceScalarType::Char,
            Self::Duration(_) => ResourceScalarType::Duration,
            Self::Ratio(_) => ResourceScalarType::Ratio,
            Self::Length(_) => ResourceScalarType::Length,
            Self::Gain(_) => ResourceScalarType::Gain,
            Self::Pan(_) => ResourceScalarType::Pan,
            Self::Locale(_) => ResourceScalarType::Locale,
            Self::PublicId(_) => ResourceScalarType::PublicId,
        }
    }
}

impl ResourceScalarBound {
    pub const fn new(value: ResourceScalarValue, kind: ResourceBoundKind) -> Self {
        Self { value, kind }
    }

    pub const fn value(&self) -> &ResourceScalarValue {
        &self.value
    }

    pub const fn kind(&self) -> ResourceBoundKind {
        self.kind
    }
}

impl ResourceScalarConstraint {
    pub fn try_new(
        scalar: ResourceScalarType,
        lower: Option<ResourceScalarBound>,
        upper: Option<ResourceScalarBound>,
    ) -> Result<Self, ResourceConstraintError> {
        for bound in lower.iter().chain(upper.iter()) {
            let actual = bound.value.scalar_type();
            if actual != scalar {
                return Err(ResourceConstraintError::BoundTypeMismatch {
                    expected: scalar,
                    actual,
                });
            }
        }
        if let (Some(lower), Some(upper)) = (&lower, &upper) {
            match lower.value.cmp(&upper.value) {
                Ordering::Greater => return Err(ResourceConstraintError::Inverted),
                Ordering::Equal
                    if lower.kind == ResourceBoundKind::Exclusive
                        || upper.kind == ResourceBoundKind::Exclusive =>
                {
                    return Err(ResourceConstraintError::Empty);
                }
                Ordering::Less | Ordering::Equal => {}
            }
        }
        Ok(Self {
            scalar,
            lower,
            upper,
        })
    }

    pub const fn scalar(&self) -> ResourceScalarType {
        self.scalar
    }

    pub const fn lower(&self) -> Option<&ResourceScalarBound> {
        self.lower.as_ref()
    }

    pub const fn upper(&self) -> Option<&ResourceScalarBound> {
        self.upper.as_ref()
    }

    fn validate(&self, value: &ResourceScalarValue) -> Result<(), ResourceValueValidationError> {
        if value.scalar_type() != self.scalar {
            return Err(ResourceValueValidationError::TypeMismatch {
                expected: ResourceValueType::ConstrainedScalar(self.clone()),
                actual: ResourceConstValueKind::Scalar(value.scalar_type()),
            });
        }
        if let Some(lower) = &self.lower {
            let ordering = value.cmp(&lower.value);
            if ordering == Ordering::Less
                || (ordering == Ordering::Equal && lower.kind == ResourceBoundKind::Exclusive)
            {
                return Err(ResourceValueValidationError::ConstraintViolation {
                    side: ResourceConstraintSide::Lower,
                    kind: lower.kind,
                });
            }
        }
        if let Some(upper) = &self.upper {
            let ordering = value.cmp(&upper.value);
            if ordering == Ordering::Greater
                || (ordering == Ordering::Equal && upper.kind == ResourceBoundKind::Exclusive)
            {
                return Err(ResourceValueValidationError::ConstraintViolation {
                    side: ResourceConstraintSide::Upper,
                    kind: upper.kind,
                });
            }
        }
        Ok(())
    }
}

impl ResourceValueType {
    pub fn option(value: Self) -> Self {
        Self::Option(Box::new(value))
    }

    pub fn vec(value: Self) -> Self {
        Self::Vec(Box::new(value))
    }

    pub fn non_empty_vec(value: Self) -> Self {
        Self::NonEmptyVec(Box::new(value))
    }

    pub fn map(key: Self, value: Self) -> Self {
        Self::Map {
            key: Box::new(key),
            value: Box::new(value),
        }
    }

    pub fn validate_const(
        &self,
        value: &ResourceConstValue,
    ) -> Result<(), ResourceValueValidationError> {
        self.validate_const_at_depth(value, 0)
    }

    /// Whether this exact structural type accepts the supplied constant.
    pub fn accepts_const_value(&self, value: &ResourceConstValue) -> bool {
        self.validate_const(value).is_ok()
    }

    fn validate_const_at_depth(
        &self,
        value: &ResourceConstValue,
        depth: usize,
    ) -> Result<(), ResourceValueValidationError> {
        if depth > MAX_RESOURCE_VALUE_NESTING {
            return Err(ResourceValueValidationError::NestingTooDeep);
        }
        match (self, value) {
            (Self::Option(expected), ResourceConstValue::Option(value)) => {
                value.as_deref().map_or(Ok(()), |value| {
                    expected
                        .validate_const_at_depth(value, depth + 1)
                        .map_err(|source| ResourceValueValidationError::Nested {
                            segment: ResourceValidationPathSegment::OptionValue,
                            source: Box::new(source),
                        })
                })
            }
            (
                Self::Vec(expected) | Self::NonEmptyVec(expected),
                ResourceConstValue::Sequence(values),
            ) if !values.is_empty() || matches!(self, Self::Vec(_)) => {
                validate_sequence(expected, values, depth)
            }
            (Self::Map { key, value }, ResourceConstValue::Map(values)) => {
                values.entries().iter().enumerate().try_for_each(
                    |(index, (actual_key, actual_value))| {
                        key.validate_const_at_depth(actual_key, depth + 1)
                            .map_err(|source| ResourceValueValidationError::Nested {
                                segment: ResourceValidationPathSegment::MapKey(index),
                                source: Box::new(source),
                            })?;
                        value
                            .validate_const_at_depth(actual_value, depth + 1)
                            .map_err(|source| ResourceValueValidationError::Nested {
                                segment: ResourceValidationPathSegment::MapValue(index),
                                source: Box::new(source),
                            })
                    },
                )
            }
            _ => self.validate_const_shallow(value),
        }
    }

    pub(crate) fn validate_const_shallow(
        &self,
        value: &ResourceConstValue,
    ) -> Result<(), ResourceValueValidationError> {
        match (self, value) {
            (Self::Scalar(expected), ResourceConstValue::Scalar(actual))
                if *expected == actual.scalar_type() =>
            {
                Ok(())
            }
            (Self::ConstrainedScalar(constraint), ResourceConstValue::Scalar(value)) => {
                constraint.validate(value)
            }
            (Self::NonEmptyVec(_), ResourceConstValue::Sequence(values)) if values.is_empty() => {
                Err(ResourceValueValidationError::EmptyNonEmptyVec)
            }
            (Self::Option(_), ResourceConstValue::Option(_))
            | (Self::Vec(_) | Self::NonEmptyVec(_), ResourceConstValue::Sequence(_))
            | (Self::Map { .. }, ResourceConstValue::Map(_)) => Ok(()),
            (Self::NominalRecord(expected), ResourceConstValue::Record(actual))
                if expected == actual.schema_id() =>
            {
                Ok(())
            }
            (Self::NominalEnum(expected), ResourceConstValue::Enum(actual))
                if expected == actual.schema_id() =>
            {
                Ok(())
            }
            (
                Self::AssetRef {
                    payload_kind: expected,
                },
                ResourceConstValue::AssetRef(actual),
            ) if expected == actual.payload_kind() => Ok(()),
            (Self::ResourceRef { type_id: expected }, ResourceConstValue::ResourceRef(actual))
                if expected == actual.type_id() =>
            {
                Ok(())
            }
            (
                Self::RetainedIdentityRef { identity: expected },
                ResourceConstValue::RetainedIdentityRef { value },
            ) if *expected == value.kind() => Ok(()),
            (
                Self::RetainedIdentityRef { identity: expected },
                ResourceConstValue::RetainedIdentityRef { value },
            ) => Err(ResourceValueValidationError::RetainedIdentityKindMismatch {
                expected: *expected,
                actual: value.kind(),
            }),
            _ => Err(ResourceValueValidationError::TypeMismatch {
                expected: self.clone(),
                actual: value.kind(),
            }),
        }
    }
}

impl ResourceMapValue {
    pub fn try_new(
        entries: impl IntoIterator<Item = (ResourceConstValue, ResourceConstValue)>,
    ) -> Result<Self, ResourceConstConstructionError> {
        let mut canonical = BTreeMap::new();
        for (key, value) in entries {
            if canonical.insert(key.clone(), value).is_some() {
                return Err(ResourceConstConstructionError::DuplicateMapKey { key });
            }
        }
        Ok(Self(canonical))
    }

    pub const fn entries(&self) -> &BTreeMap<ResourceConstValue, ResourceConstValue> {
        &self.0
    }
}

impl ResourceRecordValue {
    pub fn try_new(
        schema_id: ResourceSchemaId,
        fields: impl IntoIterator<Item = (ResourceFieldId, ResourceConstValue)>,
    ) -> Result<Self, ResourceConstConstructionError> {
        let mut canonical = BTreeMap::new();
        for (field, value) in fields {
            if canonical.insert(field, value).is_some() {
                return Err(ResourceConstConstructionError::DuplicateRecordField { field });
            }
        }
        Ok(Self {
            schema_id,
            fields: canonical,
        })
    }

    pub const fn schema_id(&self) -> &ResourceSchemaId {
        &self.schema_id
    }

    pub const fn fields(&self) -> &BTreeMap<ResourceFieldId, ResourceConstValue> {
        &self.fields
    }
}

impl ResourceEnumValue {
    pub fn new(
        schema_id: ResourceSchemaId,
        variant: ResourceVariantId,
        payload: Option<ResourceConstValue>,
    ) -> Self {
        Self {
            schema_id,
            variant,
            payload: payload.map(Box::new),
        }
    }

    pub const fn schema_id(&self) -> &ResourceSchemaId {
        &self.schema_id
    }

    pub const fn variant(&self) -> ResourceVariantId {
        self.variant
    }

    pub fn payload(&self) -> Option<&ResourceConstValue> {
        self.payload.as_deref()
    }
}

impl ResourceAssetRefValue {
    pub const fn new(public_id: PublicId, payload_kind: ResourceAssetPayloadKindId) -> Self {
        Self {
            public_id,
            payload_kind,
        }
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.public_id
    }

    pub const fn payload_kind(&self) -> &ResourceAssetPayloadKindId {
        &self.payload_kind
    }
}

impl ResourceRefValue {
    pub const fn new(entity_id: EntityId, public_id: PublicId, type_id: ResourceTypeId) -> Self {
        Self {
            entity: entity_id,
            public: public_id,
            resource_type: type_id,
        }
    }

    pub const fn entity_id(&self) -> &EntityId {
        &self.entity
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.public
    }

    pub const fn type_id(&self) -> &ResourceTypeId {
        &self.resource_type
    }
}

impl ResourceConstValue {
    pub const fn kind(&self) -> ResourceConstValueKind {
        match self {
            Self::Scalar(value) => ResourceConstValueKind::Scalar(value.scalar_type()),
            Self::Option(_) => ResourceConstValueKind::Option,
            Self::Sequence(_) => ResourceConstValueKind::Sequence,
            Self::Map(_) => ResourceConstValueKind::Map,
            Self::Record(_) => ResourceConstValueKind::Record,
            Self::Enum(_) => ResourceConstValueKind::Enum,
            Self::AssetRef(_) => ResourceConstValueKind::AssetRef,
            Self::ResourceRef(_) => ResourceConstValueKind::ResourceRef,
            Self::RetainedIdentityRef { .. } => ResourceConstValueKind::RetainedIdentityRef,
        }
    }
}

fn validate_sequence(
    expected: &ResourceValueType,
    values: &[ResourceConstValue],
    depth: usize,
) -> Result<(), ResourceValueValidationError> {
    values.iter().enumerate().try_for_each(|(index, value)| {
        expected
            .validate_const_at_depth(value, depth + 1)
            .map_err(|source| ResourceValueValidationError::Nested {
                segment: ResourceValidationPathSegment::SequenceIndex(index),
                source: Box::new(source),
            })
    })
}
