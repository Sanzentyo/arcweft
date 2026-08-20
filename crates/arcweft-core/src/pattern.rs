use crate::entry::{RuntimeIdentityError, RuntimeNominalTypeId, TypeLayoutHash};
use crate::plan::{
    RuntimePlan, RuntimePlanTypeDeclaration, RuntimePlanTypeProjection, RuntimePlanValueTypeError,
};
use crate::runtime_id::{RuntimeLocalDeclarationId, RuntimePlanTypeId};
use crate::value::{
    RuntimeEntityReference, RuntimeLocalBinding, RuntimeNominalRecordValue, RuntimeOpaqueValue,
    RuntimeOpaqueValueError, RuntimeRecordFieldId, RuntimeSeq, RuntimeSignedIntWidth,
    RuntimeUnsignedIntWidth, RuntimeValue,
};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

mod binding;

pub use binding::{
    MAX_RUNTIME_PATTERN_BINDING_DEPTH, RuntimePatternBindingCoordinate,
    RuntimePatternBindingCoordinateError, RuntimePatternBindingPath,
    RuntimePatternBindingPathError, RuntimePatternBindingStep, RuntimePatternBindingWireError,
};

/// Stable semantic identity for a checked type after alias and projection
/// normalization.
///
/// This identity is owned by core because native runtime patterns and AWBC
/// projection consume the same checked type boundary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct RuntimeSemanticTypeId([u8; 32]);

impl RuntimeSemanticTypeId {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Canonical semantic-type identity encoder shared by semantic producers.
///
/// The domain is fixed at version 1. Producers own their typed fragments while
/// this lower-layer encoder owns the common byte grammar and digest algorithm.
pub struct RuntimeSemanticTypeIdentityEncoder(blake3::Hasher);

impl RuntimeSemanticTypeIdentityEncoder {
    #[must_use]
    pub fn new() -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.semantic-type.identity.v1\0");
        Self(hasher)
    }

    pub fn write_tag(&mut self, value: u16) {
        self.write_u16(value);
    }

    pub fn write_u8(&mut self, value: u8) {
        self.0.update(&[value]);
    }

    pub fn write_u16(&mut self, value: u16) {
        self.0.update(&value.to_le_bytes());
    }

    pub fn write_u32(&mut self, value: u32) {
        self.0.update(&value.to_le_bytes());
    }

    pub fn write_u64(&mut self, value: u64) {
        self.0.update(&value.to_le_bytes());
    }

    /// Writes a sequence length in the canonical `u32` representation.
    ///
    /// # Panics
    ///
    /// Panics when `value` exceeds the semantic identity grammar's `u32`
    /// sequence-length bound.
    pub fn write_len(&mut self, value: usize) {
        self.write_u32(
            u32::try_from(value).expect("accepted semantic sequences fit the u32 contract"),
        );
    }

    pub fn write_str(&mut self, value: &str) {
        self.write_len(value.len());
        self.write_bytes(value.as_bytes());
    }

    pub fn write_bytes(&mut self, value: &[u8]) {
        self.0.update(value);
    }

    #[must_use]
    pub fn finish(self) -> RuntimeSemanticTypeId {
        RuntimeSemanticTypeId::from_bytes(*self.0.finalize().as_bytes())
    }
}

impl Default for RuntimeSemanticTypeIdentityEncoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Stable identity of a producer that validates opaque runtime payloads.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct RuntimeOpaqueTypeProducerId(RuntimeNominalTypeId);

impl RuntimeOpaqueTypeProducerId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, RuntimeIdentityError> {
        RuntimeNominalTypeId::try_new(value).map(Self)
    }

    #[must_use]
    pub const fn from_nominal(value: RuntimeNominalTypeId) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn nominal(&self) -> &RuntimeNominalTypeId {
        &self.0
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl<'de> Deserialize<'de> for RuntimeOpaqueTypeProducerId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

/// Whether an opaque checked owner names one exact type or a producer-defined top.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u8)]
pub enum RuntimeOpaqueTypeAdmission {
    ExactIdentity = 0,
    ProducerWide = 1,
}

/// Closed checked owner for one producer-validated opaque type.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeOpaqueTypeOwner {
    producer: RuntimeOpaqueTypeProducerId,
    semantic_identity: RuntimeSemanticTypeId,
    admission: RuntimeOpaqueTypeAdmission,
}

impl RuntimeOpaqueTypeOwner {
    #[must_use]
    pub const fn exact(
        producer: RuntimeOpaqueTypeProducerId,
        semantic_identity: RuntimeSemanticTypeId,
    ) -> Self {
        Self {
            producer,
            semantic_identity,
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
        }
    }

    #[must_use]
    pub const fn producer_wide(
        producer: RuntimeOpaqueTypeProducerId,
        semantic_identity: RuntimeSemanticTypeId,
    ) -> Self {
        Self {
            producer,
            semantic_identity,
            admission: RuntimeOpaqueTypeAdmission::ProducerWide,
        }
    }

    #[must_use]
    pub const fn producer(&self) -> &RuntimeOpaqueTypeProducerId {
        &self.producer
    }

    #[must_use]
    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId {
        self.semantic_identity
    }

    #[must_use]
    pub const fn admission(&self) -> RuntimeOpaqueTypeAdmission {
        self.admission
    }

    #[must_use]
    pub fn accepts_owner(&self, actual: &Self) -> bool {
        self == actual
            || (self.admission == RuntimeOpaqueTypeAdmission::ProducerWide
                && actual.admission == RuntimeOpaqueTypeAdmission::ExactIdentity
                && self.producer == actual.producer)
    }

    #[must_use]
    pub fn accepts_opaque_value(&self, actual: &RuntimeOpaqueValue) -> bool {
        &self.producer == actual.producer()
            && (self.admission == RuntimeOpaqueTypeAdmission::ProducerWide
                || self.semantic_identity == actual.semantic_identity())
    }

    pub fn try_wrap(&self, payload: RuntimeValue) -> Result<RuntimeValue, RuntimeOpaqueValueError> {
        if self.admission == RuntimeOpaqueTypeAdmission::ProducerWide {
            return Err(RuntimeOpaqueValueError::NonConcreteOwner {
                producer: self.producer.clone(),
                semantic_identity: self.semantic_identity,
            });
        }
        Ok(RuntimeValue::Opaque(RuntimeOpaqueValue::new_exact(
            self, payload,
        )))
    }
}

/// Closed identity of a runtime variant value after semantic checking.
///
/// Generic payload types remain on [`RuntimeCheckedType`]. Values retain only
/// the owner family and source-ordered case ordinal, so Option/Result
/// intrinsics never invent erased generic arguments and nominal values never
/// fall back to source-path strings.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeVariantIdentity {
    Nominal {
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
    },
    Option,
    Result,
}

/// One source-ordered case in a checked nominal enum.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeCheckedVariantCase {
    pub name: String,
    pub payload: Option<Box<RuntimeCheckedType>>,
}

/// One recursively typed pattern admitted into a runtime plan.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimePattern {
    ty: RuntimePlanTypeId,
    kind: RuntimePatternKind,
}

impl RuntimePattern {
    pub(crate) const fn from_admitted_parts(
        ty: RuntimePlanTypeId,
        kind: RuntimePatternKind,
    ) -> Self {
        Self { ty, kind }
    }

    #[must_use]
    pub const fn ty(&self) -> RuntimePlanTypeId {
        self.ty
    }

    #[must_use]
    pub const fn kind(&self) -> &RuntimePatternKind {
        &self.kind
    }
}

/// Closed executable pattern algebra. Local destinations are plan-local IDs
/// paired with coordinates derived by the aggregate plan builder.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimePatternKind {
    Bind {
        mutable: bool,
        binding: RuntimePatternBindingCoordinate,
    },
    Discard,
    Literal(RuntimeValue),
    Entity(RuntimeEntityReference),
    Tuple(Box<[RuntimePattern]>),
    Record {
        fields: Box<[RuntimeRecordPatternField]>,
        rest: RuntimePatternRest,
    },
    Sequence {
        items: Box<[RuntimePattern]>,
        rest: RuntimePatternRest,
    },
    Variant {
        ordinal: u32,
        payload: Option<Box<RuntimePattern>>,
    },
    Whole {
        binding: RuntimePatternBindingCoordinate,
        pattern: Box<RuntimePattern>,
    },
    Typed {
        binding: RuntimePatternBindingCoordinate,
    },
}

/// Exact, open, or binding remainder semantics shared by structural patterns.
///
/// A record rest binding receives the original complete record. A bracket-
/// sequence rest binding receives only the unmatched tail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimePatternRest {
    Exact,
    Ignore,
    Bind(RuntimePatternBindingCoordinate),
}

impl RuntimePatternRest {
    /// Returns whether `actual` elements/fields satisfy this remainder mode.
    #[must_use]
    pub const fn accepts_len(&self, required: usize, actual: usize) -> bool {
        match self {
            Self::Exact => required == actual,
            Self::Ignore | Self::Bind(_) => required <= actual,
        }
    }

    /// Returns the admitted local coordinate written by a binding rest.
    #[must_use]
    pub const fn binding(&self) -> Option<&RuntimePatternBindingCoordinate> {
        match self {
            Self::Bind(binding) => Some(binding),
            Self::Exact | Self::Ignore => None,
        }
    }
}

/// Closed structural type predicate for a runtime typed-binding pattern.
///
/// The compiler projects this value once from checked semantic type facts.
/// Native execution and AWBC lowering consume the same typed vocabulary; no
/// source/display label is reparsed at either boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeCheckedType {
    Never,
    Unit,
    Bool,
    Signed(RuntimeSignedIntWidth),
    Unsigned(RuntimeUnsignedIntWidth),
    F32,
    F64,
    String,
    Char,
    Duration,
    Progress,
    EntityReference,
    Bytes,
    Sequence(Box<RuntimeCheckedType>),
    Tuple(Vec<RuntimeCheckedType>),
    Choice(Vec<RuntimeCheckedType>),
    Nominal {
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
    },
    Opaque {
        owner: RuntimeOpaqueTypeOwner,
    },
    Variant {
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        cases: Vec<RuntimeCheckedVariantCase>,
    },
    Result {
        ok: Box<RuntimeCheckedType>,
        error: Box<RuntimeCheckedType>,
    },
    Option(Box<RuntimeCheckedType>),
    Agent(crate::plan::RuntimeAgentOperationalType),
}

impl RuntimeCheckedType {
    #[must_use]
    pub fn variant_identity(&self) -> Option<RuntimeVariantIdentity> {
        match self {
            Self::Variant {
                nominal,
                semantic_identity,
                ..
            } => Some(RuntimeVariantIdentity::Nominal {
                nominal: nominal.clone(),
                semantic_identity: *semantic_identity,
            }),
            Self::Result { .. } => Some(RuntimeVariantIdentity::Result),
            Self::Option(_) => Some(RuntimeVariantIdentity::Option),
            _ => None,
        }
    }

    #[must_use]
    pub fn variant_case(&self, ordinal: u32) -> Option<RuntimeCheckedVariantCase> {
        match self {
            Self::Variant { cases, .. } => usize::try_from(ordinal)
                .ok()
                .and_then(|ordinal| cases.get(ordinal))
                .cloned(),
            Self::Result { ok, error } => match ordinal {
                0 => Some(RuntimeCheckedVariantCase {
                    name: "Ok".to_owned(),
                    payload: Some(ok.clone()),
                }),
                1 => Some(RuntimeCheckedVariantCase {
                    name: "Err".to_owned(),
                    payload: Some(error.clone()),
                }),
                _ => None,
            },
            Self::Option(item) => match ordinal {
                0 => Some(RuntimeCheckedVariantCase {
                    name: "Some".to_owned(),
                    payload: Some(item.clone()),
                }),
                1 => Some(RuntimeCheckedVariantCase {
                    name: "None".to_owned(),
                    payload: None,
                }),
                _ => None,
            },
            _ => None,
        }
    }

    /// Returns whether a runtime value satisfies this exact closed predicate.
    #[must_use]
    pub fn accepts_value(&self, value: &RuntimeValue) -> bool {
        self.accepts_value_at_depth(value, 0)
    }

    fn accepts_value_at_depth(&self, value: &RuntimeValue, depth: usize) -> bool {
        if depth > crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH {
            return false;
        }
        match (value, self) {
            (RuntimeValue::Unit, Self::Unit)
            | (RuntimeValue::Bool(_), Self::Bool)
            | (RuntimeValue::F32(_), Self::F32)
            | (RuntimeValue::F64(_), Self::F64)
            | (RuntimeValue::String(_), Self::String)
            | (RuntimeValue::Char(_), Self::Char)
            | (RuntimeValue::Duration(_), Self::Duration)
            | (RuntimeValue::Progress(_), Self::Progress)
            | (RuntimeValue::EntityRef(_), Self::EntityReference) => true,
            (RuntimeValue::Int(value), Self::Signed(width)) => value.width() == *width,
            (RuntimeValue::UInt(value), Self::Unsigned(width)) => value.width() == *width,
            (RuntimeValue::Seq(sequence), Self::Bytes) => sequence
                .clone()
                .into_values()
                .iter()
                .all(|value| matches!(value, RuntimeValue::UInt(value) if value.width() == RuntimeUnsignedIntWidth::U8)),
            (RuntimeValue::Seq(sequence), Self::Sequence(item)) => sequence
                .clone()
                .into_values()
                .iter()
                .all(|value| item.accepts_value_at_depth(value, depth + 1)),
            (RuntimeValue::Tuple(values), Self::Tuple(items)) => {
                values.len() == items.len()
                    && values
                        .iter()
                        .zip(items)
                        .all(|(value, item)| item.accepts_value_at_depth(value, depth + 1))
            }
            (value, Self::Choice(alternatives)) => alternatives
                .iter()
                .any(|alternative| alternative.accepts_value_at_depth(value, depth + 1)),
            (RuntimeValue::Opaque(value), Self::Opaque { owner }) => {
                owner.accepts_opaque_value(value)
            }
            (RuntimeValue::Reduction(value), Self::Opaque { owner }) => {
                owner.accepts_owner(value.owner())
            }
            (
                RuntimeValue::NominalRecord(record),
                Self::Nominal {
                    nominal, layout, ..
                },
            ) => record.type_id() == nominal && record.layout() == *layout,
            (
                RuntimeValue::Variant {
                    owner,
                    ordinal,
                    name,
                    payload,
                },
                Self::Variant { .. },
            ) => self.accepts_nominal_variant_at_depth(
                owner,
                *ordinal,
                name,
                payload.as_deref(),
                depth,
            ),
            (
                RuntimeValue::Variant {
                    owner,
                    ordinal,
                    name,
                    payload,
                },
                Self::Result { ok, error },
            ) if *owner == RuntimeVariantIdentity::Result => {
                match (*ordinal, name.as_str(), payload.as_deref()) {
                    (0, "Ok", Some(value)) => ok.accepts_value_at_depth(value, depth + 1),
                    (1, "Err", Some(value)) => error.accepts_value_at_depth(value, depth + 1),
                    _ => false,
                }
            }
            (
                RuntimeValue::Variant {
                    owner,
                    ordinal,
                    name,
                    payload,
                },
                Self::Option(item),
            ) if *owner == RuntimeVariantIdentity::Option => {
                match (*ordinal, name.as_str(), payload.as_deref()) {
                    (0, "Some", Some(value)) => item.accepts_value_at_depth(value, depth + 1),
                    (1, "None", None) => true,
                    _ => false,
                }
            }
            (RuntimeValue::Agent(value), Self::Agent(expected)) => {
                value.operational_type() == *expected
            }
            (RuntimeValue::Record(_), Self::Agent(expected)) => expected.accepts_protocol_record(),
            _ => false,
        }
    }

    fn accepts_nominal_variant_at_depth(
        &self,
        owner: &RuntimeVariantIdentity,
        ordinal: u32,
        name: &str,
        payload: Option<&RuntimeValue>,
        depth: usize,
    ) -> bool {
        let Self::Variant {
            nominal,
            semantic_identity,
            cases,
        } = self
        else {
            return false;
        };
        if owner
            != &(RuntimeVariantIdentity::Nominal {
                nominal: nominal.clone(),
                semantic_identity: *semantic_identity,
            })
        {
            return false;
        }
        usize::try_from(ordinal)
            .ok()
            .and_then(|ordinal| cases.get(ordinal))
            .is_some_and(|case| {
                case.name == name
                    && match (case.payload.as_deref(), payload) {
                        (None, None) => true,
                        (Some(expected), Some(actual)) => {
                            expected.accepts_value_at_depth(actual, depth + 1)
                        }
                        (None, Some(_)) | (Some(_), None) => false,
                    }
            })
    }
}

/// One field inside a runtime record pattern.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeRecordPatternField {
    field: RuntimeRecordFieldId,
    pattern: RuntimePattern,
}

impl RuntimeRecordPatternField {
    pub(crate) const fn from_admitted_parts(
        field: RuntimeRecordFieldId,
        pattern: RuntimePattern,
    ) -> Self {
        Self { field, pattern }
    }

    #[must_use]
    pub const fn field(&self) -> RuntimeRecordFieldId {
        self.field
    }

    #[must_use]
    pub const fn pattern(&self) -> &RuntimePattern {
        &self.pattern
    }
}

pub(crate) fn match_runtime_pattern(
    plan: &RuntimePlan,
    pattern: &RuntimePattern,
    value: &RuntimeValue,
) -> Result<Option<Vec<RuntimeLocalBinding>>, RuntimePatternMatchError> {
    validate_runtime_pattern(plan, pattern)?;
    if !plan.value_matches_type(pattern.ty(), value)? {
        return Ok(None);
    }
    let mut bindings = Vec::with_capacity(pattern_binding_capacity(pattern));
    if collect_pattern_bindings(plan, pattern, value, &mut bindings)? {
        Ok(Some(bindings))
    } else {
        Ok(None)
    }
}

pub(crate) fn pattern_binding_capacity(pattern: &RuntimePattern) -> usize {
    let direct = match pattern.kind() {
        RuntimePatternKind::Bind { .. } | RuntimePatternKind::Typed { .. } => 1,
        RuntimePatternKind::Discard
        | RuntimePatternKind::Literal(_)
        | RuntimePatternKind::Entity(_) => 0,
        RuntimePatternKind::Tuple(patterns)
        | RuntimePatternKind::Sequence {
            items: patterns, ..
        } => patterns.iter().map(pattern_binding_capacity).sum(),
        RuntimePatternKind::Record { fields, .. } => fields
            .iter()
            .map(|field| pattern_binding_capacity(field.pattern()))
            .sum(),
        RuntimePatternKind::Variant { payload, .. } => {
            payload.as_deref().map_or(0, pattern_binding_capacity)
        }
        RuntimePatternKind::Whole { pattern, .. } => pattern_binding_capacity(pattern) + 1,
    };
    direct
        + usize::from(matches!(
            pattern.kind(),
            RuntimePatternKind::Record {
                rest: RuntimePatternRest::Bind(_),
                ..
            } | RuntimePatternKind::Sequence {
                rest: RuntimePatternRest::Bind(_),
                ..
            }
        ))
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimePatternMatchError {
    #[error(transparent)]
    ValueType(#[from] RuntimePlanValueTypeError),
    #[error("runtime pattern references unknown plan type {ty}")]
    UnknownType { ty: RuntimePlanTypeId },
    #[error("runtime pattern kind is incompatible with plan type {ty}")]
    InvalidKind { ty: RuntimePlanTypeId },
    #[error("runtime pattern child has type {actual}, expected {expected}")]
    ChildType {
        expected: RuntimePlanTypeId,
        actual: RuntimePlanTypeId,
    },
    #[error("runtime pattern references unknown local declaration {local}")]
    UnknownLocal { local: RuntimeLocalDeclarationId },
    #[error("runtime pattern local {local} has type {actual}, expected {expected}")]
    LocalType {
        local: RuntimeLocalDeclarationId,
        expected: RuntimePlanTypeId,
        actual: RuntimePlanTypeId,
    },
    #[error("runtime pattern local {local} has a coordinate inconsistent with its node")]
    InvalidCoordinate { local: RuntimeLocalDeclarationId },
    #[error("runtime pattern binds local {local} more than once")]
    DuplicateLocal { local: RuntimeLocalDeclarationId },
    #[error("runtime record pattern type {ty} has no nominal-record domain")]
    MissingRecordDomain { ty: RuntimePlanTypeId },
    #[error("runtime record pattern type {ty} references unknown field {field}")]
    UnknownRecordField {
        ty: RuntimePlanTypeId,
        field: RuntimeRecordFieldId,
    },
    #[error("runtime record pattern type {ty} repeats field {field}")]
    DuplicateRecordField {
        ty: RuntimePlanTypeId,
        field: RuntimeRecordFieldId,
    },
    #[error("runtime variant pattern type {ty} has no case {ordinal}")]
    UnknownVariantCase { ty: RuntimePlanTypeId, ordinal: u32 },
    #[error("runtime variant pattern type {ty} has no nominal-variant domain")]
    MissingVariantDomain { ty: RuntimePlanTypeId },
    #[error("runtime variant pattern type {ty} has an invalid payload shape for case {ordinal}")]
    VariantPayload { ty: RuntimePlanTypeId, ordinal: u32 },
}

fn validate_runtime_pattern(
    plan: &RuntimePlan,
    pattern: &RuntimePattern,
) -> Result<(), RuntimePatternMatchError> {
    let mut locals = BTreeSet::new();
    validate_pattern_node(plan, pattern, &[], &mut locals)
}

fn validate_pattern_node(
    plan: &RuntimePlan,
    pattern: &RuntimePattern,
    path: &[RuntimePatternBindingStep],
    locals: &mut BTreeSet<RuntimeLocalDeclarationId>,
) -> Result<(), RuntimePatternMatchError> {
    let declaration = plan
        .type_table()
        .get(pattern.ty())
        .ok_or(RuntimePatternMatchError::UnknownType { ty: pattern.ty() })?;
    match pattern.kind() {
        RuntimePatternKind::Bind { binding, .. } | RuntimePatternKind::Typed { binding } => {
            validate_binding(plan, pattern.ty(), path, binding, locals)
        }
        RuntimePatternKind::Discard => Ok(()),
        RuntimePatternKind::Literal(value) => {
            if plan.value_matches_type(pattern.ty(), value)? {
                Ok(())
            } else {
                Err(RuntimePatternMatchError::InvalidKind { ty: pattern.ty() })
            }
        }
        RuntimePatternKind::Entity(_) => match declaration.projection() {
            RuntimePlanTypeProjection::EntityReference => Ok(()),
            _ => Err(RuntimePatternMatchError::InvalidKind { ty: pattern.ty() }),
        },
        RuntimePatternKind::Tuple(patterns) => {
            validate_tuple_pattern(plan, pattern.ty(), patterns, path, locals)
        }
        RuntimePatternKind::Record { fields, rest } => {
            validate_record_pattern(plan, pattern.ty(), fields, rest, path, locals)
        }
        RuntimePatternKind::Sequence { items, rest } => {
            validate_sequence_pattern(plan, pattern.ty(), items, rest, path, locals)
        }
        RuntimePatternKind::Variant { ordinal, payload } => validate_variant_pattern(
            plan,
            pattern.ty(),
            *ordinal,
            payload.as_deref(),
            path,
            locals,
        ),
        RuntimePatternKind::Whole {
            binding,
            pattern: inner,
        } => {
            validate_child_type(pattern.ty(), inner.ty())?;
            validate_binding(plan, pattern.ty(), path, binding, locals)?;
            validate_pattern_node(plan, inner, path, locals)
        }
    }
}

fn validate_tuple_pattern(
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    patterns: &[RuntimePattern],
    path: &[RuntimePatternBindingStep],
    locals: &mut BTreeSet<RuntimeLocalDeclarationId>,
) -> Result<(), RuntimePatternMatchError> {
    let Some(RuntimePlanTypeProjection::Tuple(types)) = plan
        .type_table()
        .get(ty)
        .map(crate::plan::RuntimePlanTypeDeclaration::projection)
    else {
        return Err(RuntimePatternMatchError::InvalidKind { ty });
    };
    if patterns.len() != types.len() {
        return Err(RuntimePatternMatchError::InvalidKind { ty });
    }
    for (ordinal, (pattern, expected)) in patterns.iter().zip(types).enumerate() {
        validate_child_type(*expected, pattern.ty())?;
        let ordinal =
            u32::try_from(ordinal).map_err(|_| RuntimePatternMatchError::InvalidKind { ty })?;
        let mut child_path = path.to_vec();
        child_path.push(RuntimePatternBindingStep::TupleElement(ordinal));
        validate_pattern_node(plan, pattern, &child_path, locals)?;
    }
    Ok(())
}

fn validate_record_pattern(
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    fields: &[RuntimeRecordPatternField],
    rest: &RuntimePatternRest,
    path: &[RuntimePatternBindingStep],
    locals: &mut BTreeSet<RuntimeLocalDeclarationId>,
) -> Result<(), RuntimePatternMatchError> {
    if !matches!(
        plan.type_table()
            .get(ty)
            .map(crate::plan::RuntimePlanTypeDeclaration::projection),
        Some(RuntimePlanTypeProjection::ProjectNominal { .. })
    ) {
        return Err(RuntimePatternMatchError::InvalidKind { ty });
    }
    let domain = plan
        .nominal_record_domains()
        .get(ty)
        .ok_or(RuntimePatternMatchError::MissingRecordDomain { ty })?;
    let mut seen_fields = BTreeSet::new();
    for (ordinal, field) in fields.iter().enumerate() {
        if !seen_fields.insert(field.field()) {
            return Err(RuntimePatternMatchError::DuplicateRecordField {
                ty,
                field: field.field(),
            });
        }
        let expected = usize::try_from(field.field().zero_based())
            .ok()
            .and_then(|index| domain.fields().get(index))
            .ok_or(RuntimePatternMatchError::UnknownRecordField {
                ty,
                field: field.field(),
            })?
            .ty();
        validate_child_type(expected, field.pattern().ty())?;
        let ordinal =
            u32::try_from(ordinal).map_err(|_| RuntimePatternMatchError::InvalidKind { ty })?;
        let mut child_path = path.to_vec();
        child_path.push(RuntimePatternBindingStep::RecordField(ordinal));
        validate_pattern_node(plan, field.pattern(), &child_path, locals)?;
    }
    if let Some(binding) = rest.binding() {
        let mut rest_path = path.to_vec();
        rest_path.push(RuntimePatternBindingStep::RecordRest);
        validate_binding(plan, ty, &rest_path, binding, locals)?;
    }
    Ok(())
}

fn validate_sequence_pattern(
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    items: &[RuntimePattern],
    rest: &RuntimePatternRest,
    path: &[RuntimePatternBindingStep],
    locals: &mut BTreeSet<RuntimeLocalDeclarationId>,
) -> Result<(), RuntimePatternMatchError> {
    let declaration = plan
        .type_table()
        .get(ty)
        .ok_or(RuntimePatternMatchError::UnknownType { ty })?;
    let item_ty = match declaration.projection() {
        RuntimePlanTypeProjection::Sequence { item, .. } => *item,
        RuntimePlanTypeProjection::Array { item, .. }
            if !matches!(rest, RuntimePatternRest::Bind(_)) =>
        {
            *item
        }
        _ => return Err(RuntimePatternMatchError::InvalidKind { ty }),
    };
    for (ordinal, item) in items.iter().enumerate() {
        validate_child_type(item_ty, item.ty())?;
        let ordinal =
            u32::try_from(ordinal).map_err(|_| RuntimePatternMatchError::InvalidKind { ty })?;
        let mut child_path = path.to_vec();
        child_path.push(RuntimePatternBindingStep::SequenceElement(ordinal));
        validate_pattern_node(plan, item, &child_path, locals)?;
    }
    if let Some(binding) = rest.binding() {
        let mut rest_path = path.to_vec();
        rest_path.push(RuntimePatternBindingStep::SequenceRest);
        validate_binding(plan, ty, &rest_path, binding, locals)?;
    }
    Ok(())
}

fn validate_variant_pattern(
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    ordinal: u32,
    payload: Option<&RuntimePattern>,
    path: &[RuntimePatternBindingStep],
    locals: &mut BTreeSet<RuntimeLocalDeclarationId>,
) -> Result<(), RuntimePatternMatchError> {
    match (variant_payload_type(plan, ty, ordinal)?, payload) {
        (Some(expected), Some(payload)) => {
            validate_child_type(expected, payload.ty())?;
            let mut child_path = path.to_vec();
            child_path.push(RuntimePatternBindingStep::VariantPayload);
            validate_pattern_node(plan, payload, &child_path, locals)
        }
        (None, None) => Ok(()),
        (Some(_), None) | (None, Some(_)) => {
            Err(RuntimePatternMatchError::VariantPayload { ty, ordinal })
        }
    }
}

fn validate_binding(
    plan: &RuntimePlan,
    expected_ty: RuntimePlanTypeId,
    path: &[RuntimePatternBindingStep],
    binding: &RuntimePatternBindingCoordinate,
    locals: &mut BTreeSet<RuntimeLocalDeclarationId>,
) -> Result<(), RuntimePatternMatchError> {
    let local = plan.local_declarations().get(binding.local()).ok_or(
        RuntimePatternMatchError::UnknownLocal {
            local: binding.local(),
        },
    )?;
    if local.ty() != expected_ty {
        return Err(RuntimePatternMatchError::LocalType {
            local: binding.local(),
            expected: expected_ty,
            actual: local.ty(),
        });
    }
    let expected_path = if path.is_empty() {
        &[RuntimePatternBindingStep::Whole][..]
    } else {
        path
    };
    if binding.path().steps() != expected_path {
        return Err(RuntimePatternMatchError::InvalidCoordinate {
            local: binding.local(),
        });
    }
    if !locals.insert(binding.local()) {
        return Err(RuntimePatternMatchError::DuplicateLocal {
            local: binding.local(),
        });
    }
    Ok(())
}

fn validate_child_type(
    expected: RuntimePlanTypeId,
    actual: RuntimePlanTypeId,
) -> Result<(), RuntimePatternMatchError> {
    if expected == actual {
        Ok(())
    } else {
        Err(RuntimePatternMatchError::ChildType { expected, actual })
    }
}

fn variant_payload_type(
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    ordinal: u32,
) -> Result<Option<RuntimePlanTypeId>, RuntimePatternMatchError> {
    let declaration = plan
        .type_table()
        .get(ty)
        .ok_or(RuntimePatternMatchError::UnknownType { ty })?;
    match declaration.projection() {
        RuntimePlanTypeProjection::Result { value, error } => match ordinal {
            0 => Ok(Some(*value)),
            1 => Ok(Some(*error)),
            _ => Err(RuntimePatternMatchError::UnknownVariantCase { ty, ordinal }),
        },
        RuntimePlanTypeProjection::Option(item) => match ordinal {
            0 => Ok(Some(*item)),
            1 => Ok(None),
            _ => Err(RuntimePatternMatchError::UnknownVariantCase { ty, ordinal }),
        },
        RuntimePlanTypeProjection::ProjectNominal { .. }
        | RuntimePlanTypeProjection::Opaque { .. } => {
            let domain = plan
                .variant_domains()
                .get(ty)
                .ok_or(RuntimePatternMatchError::MissingVariantDomain { ty })?;
            domain
                .case(ordinal)
                .map(crate::plan::RuntimeVariantCase::payload)
                .ok_or(RuntimePatternMatchError::UnknownVariantCase { ty, ordinal })
        }
        _ => Err(RuntimePatternMatchError::InvalidKind { ty }),
    }
}

fn collect_pattern_bindings(
    plan: &RuntimePlan,
    pattern: &RuntimePattern,
    value: &RuntimeValue,
    bindings: &mut Vec<RuntimeLocalBinding>,
) -> Result<bool, RuntimePatternMatchError> {
    match pattern.kind() {
        RuntimePatternKind::Bind { binding, .. } | RuntimePatternKind::Typed { binding } => {
            bindings.push(RuntimeLocalBinding {
                local: binding.local(),
                value: value.clone(),
            });
            Ok(true)
        }
        RuntimePatternKind::Discard => Ok(true),
        RuntimePatternKind::Literal(expected) => Ok(expected == value),
        RuntimePatternKind::Entity(expected) => Ok(matches!(
            value,
            RuntimeValue::EntityRef(actual) if actual == &expected.runtime_label()
        )),
        RuntimePatternKind::Tuple(patterns) => {
            let RuntimeValue::Tuple(values) = value else {
                return Ok(false);
            };
            collect_pattern_list(plan, patterns, values, bindings)
        }
        RuntimePatternKind::Record { fields, rest } => {
            collect_record_pattern_bindings(plan, pattern.ty(), fields, rest, value, bindings)
        }
        RuntimePatternKind::Sequence { items, rest } => {
            collect_sequence_pattern_bindings(plan, items, rest, value, bindings)
        }
        RuntimePatternKind::Variant {
            ordinal, payload, ..
        } => {
            let RuntimeValue::Variant {
                ordinal: actual_ordinal,
                payload: actual_payload,
                ..
            } = value
            else {
                return Ok(false);
            };
            if ordinal != actual_ordinal {
                return Ok(false);
            }
            match (payload.as_deref(), actual_payload.as_deref()) {
                (Some(pattern), Some(value)) => {
                    collect_pattern_bindings(plan, pattern, value, bindings)
                }
                (None, None) => Ok(true),
                (Some(_), None) | (None, Some(_)) => Ok(false),
            }
        }
        RuntimePatternKind::Whole {
            binding,
            pattern: inner,
        } => {
            if !collect_pattern_bindings(plan, inner, value, bindings)? {
                return Ok(false);
            }
            bindings.push(RuntimeLocalBinding {
                local: binding.local(),
                value: value.clone(),
            });
            Ok(true)
        }
    }
}

fn collect_record_pattern_bindings(
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    fields: &[RuntimeRecordPatternField],
    rest: &RuntimePatternRest,
    value: &RuntimeValue,
    bindings: &mut Vec<RuntimeLocalBinding>,
) -> Result<bool, RuntimePatternMatchError> {
    let RuntimeValue::NominalRecord(record) = value else {
        return Ok(false);
    };
    if !rest.accepts_len(fields.len(), record.fields().len()) {
        return Ok(false);
    }
    for field in fields {
        let Some(value) = record.field(field.field()) else {
            return Ok(false);
        };
        if !collect_pattern_bindings(plan, field.pattern(), value, bindings)? {
            return Ok(false);
        }
    }
    if let Some(binding) = rest.binding() {
        bindings.push(RuntimeLocalBinding {
            local: binding.local(),
            value: value.clone(),
        });
    }
    debug_assert!(plan.nominal_record_domains().get(ty).is_some());
    Ok(true)
}

fn collect_sequence_pattern_bindings(
    plan: &RuntimePlan,
    items: &[RuntimePattern],
    rest: &RuntimePatternRest,
    value: &RuntimeValue,
    bindings: &mut Vec<RuntimeLocalBinding>,
) -> Result<bool, RuntimePatternMatchError> {
    let RuntimeValue::Seq(sequence) = value else {
        return Ok(false);
    };
    if !rest.accepts_len(items.len(), sequence.len()) {
        return Ok(false);
    }
    for (index, pattern) in items.iter().enumerate() {
        if !collect_pattern_bindings(plan, pattern, &sequence.value_at(index), bindings)? {
            return Ok(false);
        }
    }
    if let Some(binding) = rest.binding() {
        bindings.push(RuntimeLocalBinding {
            local: binding.local(),
            value: RuntimeValue::Seq(sequence.tail_from(items.len())),
        });
    }
    Ok(true)
}

fn collect_pattern_list(
    plan: &RuntimePlan,
    patterns: &[RuntimePattern],
    values: &[RuntimeValue],
    bindings: &mut Vec<RuntimeLocalBinding>,
) -> Result<bool, RuntimePatternMatchError> {
    for (pattern, value) in patterns.iter().zip(values) {
        if !collect_pattern_bindings(plan, pattern, value, bindings)? {
            return Ok(false);
        }
    }
    Ok(true)
}

impl RuntimePlan {
    /// Checks one value against the sole admitted plan type graph.
    ///
    /// Pattern matching and structured closure capture/application share this
    /// context-owned operation instead of maintaining parallel predicates.
    pub(crate) fn value_matches_type(
        &self,
        ty: RuntimePlanTypeId,
        value: &RuntimeValue,
    ) -> Result<bool, RuntimePlanValueTypeError> {
        if self.type_table().get(ty).is_none() {
            return Err(RuntimePlanValueTypeError::UnknownType { ty });
        }
        Ok(runtime_value_matches_type_inner(self, ty, value, 0))
    }
}

fn runtime_value_matches_type_inner(
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    value: &RuntimeValue,
    depth: usize,
) -> bool {
    if depth > crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH {
        return false;
    }
    let Some(declaration) = plan.type_table().get(ty) else {
        return false;
    };
    match (declaration.projection(), value) {
        (RuntimePlanTypeProjection::Unit, RuntimeValue::Unit)
        | (RuntimePlanTypeProjection::Bool, RuntimeValue::Bool(_))
        | (RuntimePlanTypeProjection::F32, RuntimeValue::F32(_))
        | (RuntimePlanTypeProjection::F64, RuntimeValue::F64(_))
        | (RuntimePlanTypeProjection::String, RuntimeValue::String(_))
        | (RuntimePlanTypeProjection::Char, RuntimeValue::Char(_))
        | (RuntimePlanTypeProjection::Duration, RuntimeValue::Duration(_))
        | (RuntimePlanTypeProjection::Progress, RuntimeValue::Progress(_))
        | (RuntimePlanTypeProjection::EntityReference, RuntimeValue::EntityRef(_))
        | (RuntimePlanTypeProjection::Range(_), RuntimeValue::Range(_))
        | (RuntimePlanTypeProjection::Iterator(_), RuntimeValue::Iterator(_))
        | (RuntimePlanTypeProjection::Function { .. }, RuntimeValue::Function(_)) => true,
        (RuntimePlanTypeProjection::Signed(expected), RuntimeValue::Int(actual)) => {
            *expected == actual.width()
        }
        (RuntimePlanTypeProjection::Unsigned(expected), RuntimeValue::UInt(actual)) => {
            *expected == actual.width()
        }
        (RuntimePlanTypeProjection::Bytes, RuntimeValue::Seq(sequence)) => {
            runtime_sequence_is_bytes(sequence)
        }
        (RuntimePlanTypeProjection::Sequence { item, .. }, RuntimeValue::Seq(sequence)) => {
            runtime_sequence_matches_type(plan, *item, sequence, None, depth)
        }
        (RuntimePlanTypeProjection::Array { item, length }, RuntimeValue::Seq(sequence)) => {
            runtime_sequence_matches_type(plan, *item, sequence, Some(*length), depth)
        }
        (RuntimePlanTypeProjection::Tuple(types), RuntimeValue::Tuple(values)) => {
            runtime_tuple_matches_type(plan, types, values, depth)
        }
        (RuntimePlanTypeProjection::Choice(types), value) => {
            runtime_choice_matches_type(plan, types, value, depth)
        }
        (
            RuntimePlanTypeProjection::Result { value: ok, error },
            RuntimeValue::Variant {
                owner: RuntimeVariantIdentity::Result,
                ordinal,
                name,
                payload,
            },
        ) => runtime_result_matches_type(
            plan,
            *ok,
            *error,
            *ordinal,
            name,
            payload.as_deref(),
            depth,
        ),
        (
            RuntimePlanTypeProjection::Option(item),
            RuntimeValue::Variant {
                owner: RuntimeVariantIdentity::Option,
                ordinal,
                name,
                payload,
            },
        ) => runtime_option_matches_type(plan, *item, *ordinal, name, payload.as_deref(), depth),
        (
            RuntimePlanTypeProjection::ProjectNominal {
                nominal, layout, ..
            },
            RuntimeValue::NominalRecord(record),
        ) => runtime_nominal_record_matches_type(plan, ty, nominal, *layout, record, depth),
        (
            RuntimePlanTypeProjection::ProjectNominal { .. }
            | RuntimePlanTypeProjection::Opaque { .. },
            value @ RuntimeValue::Variant {
                owner: RuntimeVariantIdentity::Nominal { .. },
                ..
            },
        ) => runtime_nominal_variant_matches_type(plan, ty, declaration, value, depth),
        (
            RuntimePlanTypeProjection::Opaque {
                producer,
                admission,
                ..
            },
            RuntimeValue::Opaque(value),
        ) => runtime_opaque_matches_type(declaration, producer, *admission, value),
        (
            RuntimePlanTypeProjection::Shared(inner) | RuntimePlanTypeProjection::Reference(inner),
            value,
        ) => runtime_value_matches_type_inner(plan, *inner, value, depth + 1),
        (RuntimePlanTypeProjection::Agent(agent), RuntimeValue::Agent(value)) => {
            agent.operational_type() == value.operational_type()
        }
        _ => false,
    }
}

fn runtime_sequence_is_bytes(sequence: &RuntimeSeq) -> bool {
    sequence.clone().into_values().iter().all(
        |value| matches!(value, RuntimeValue::UInt(value) if value.width() == RuntimeUnsignedIntWidth::U8),
    )
}

fn runtime_sequence_matches_type(
    plan: &RuntimePlan,
    item: RuntimePlanTypeId,
    sequence: &RuntimeSeq,
    expected_length: Option<u64>,
    depth: usize,
) -> bool {
    if expected_length.is_some_and(|length| usize::try_from(length).ok() != Some(sequence.len())) {
        return false;
    }
    sequence
        .clone()
        .into_values()
        .iter()
        .all(|value| runtime_value_matches_type_inner(plan, item, value, depth + 1))
}

fn runtime_tuple_matches_type(
    plan: &RuntimePlan,
    types: &[RuntimePlanTypeId],
    values: &[RuntimeValue],
    depth: usize,
) -> bool {
    types.len() == values.len()
        && types
            .iter()
            .zip(values)
            .all(|(ty, value)| runtime_value_matches_type_inner(plan, *ty, value, depth + 1))
}

fn runtime_choice_matches_type(
    plan: &RuntimePlan,
    types: &[RuntimePlanTypeId],
    value: &RuntimeValue,
    depth: usize,
) -> bool {
    types
        .iter()
        .any(|ty| runtime_value_matches_type_inner(plan, *ty, value, depth + 1))
}

fn runtime_result_matches_type(
    plan: &RuntimePlan,
    ok: RuntimePlanTypeId,
    error: RuntimePlanTypeId,
    ordinal: u32,
    name: &str,
    payload: Option<&RuntimeValue>,
    depth: usize,
) -> bool {
    match (ordinal, name, payload) {
        (0, "Ok", Some(value)) => runtime_value_matches_type_inner(plan, ok, value, depth + 1),
        (1, "Err", Some(value)) => runtime_value_matches_type_inner(plan, error, value, depth + 1),
        _ => false,
    }
}

fn runtime_option_matches_type(
    plan: &RuntimePlan,
    item: RuntimePlanTypeId,
    ordinal: u32,
    name: &str,
    payload: Option<&RuntimeValue>,
    depth: usize,
) -> bool {
    match (ordinal, name, payload) {
        (0, "Some", Some(value)) => runtime_value_matches_type_inner(plan, item, value, depth + 1),
        (1, "None", None) => true,
        _ => false,
    }
}

fn runtime_nominal_record_matches_type(
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    nominal: &RuntimeNominalTypeId,
    layout: TypeLayoutHash,
    record: &RuntimeNominalRecordValue,
    depth: usize,
) -> bool {
    plan.nominal_record_domains().get(ty).is_some_and(|domain| {
        record.type_id() == nominal
            && record.layout() == layout
            && record.fields().len() == domain.fields().len()
            && record
                .fields()
                .iter()
                .zip(domain.fields())
                .all(|(value, field)| {
                    runtime_value_matches_type_inner(plan, field.ty(), value, depth + 1)
                })
    })
}

fn runtime_nominal_variant_matches_type(
    plan: &RuntimePlan,
    ty: RuntimePlanTypeId,
    declaration: &RuntimePlanTypeDeclaration,
    value: &RuntimeValue,
    depth: usize,
) -> bool {
    let RuntimeValue::Variant {
        owner:
            RuntimeVariantIdentity::Nominal {
                nominal: actual_nominal,
                semantic_identity,
            },
        ordinal,
        name,
        payload,
    } = value
    else {
        return false;
    };
    let Some(domain) = plan.variant_domains().get(ty) else {
        return false;
    };
    actual_nominal == domain.nominal()
        && *semantic_identity == declaration.semantic_identity()
        && domain.case(*ordinal).is_some_and(|case| {
            case.name() == name
                && match (case.payload(), payload.as_deref()) {
                    (Some(ty), Some(value)) => {
                        runtime_value_matches_type_inner(plan, ty, value, depth + 1)
                    }
                    (None, None) => true,
                    _ => false,
                }
        })
}

fn runtime_opaque_matches_type(
    declaration: &RuntimePlanTypeDeclaration,
    producer: &RuntimeOpaqueTypeProducerId,
    admission: RuntimeOpaqueTypeAdmission,
    value: &RuntimeOpaqueValue,
) -> bool {
    let owner = match admission {
        RuntimeOpaqueTypeAdmission::ExactIdentity => {
            RuntimeOpaqueTypeOwner::exact(producer.clone(), declaration.semantic_identity())
        }
        RuntimeOpaqueTypeAdmission::ProducerWide => {
            RuntimeOpaqueTypeOwner::producer_wide(producer.clone(), declaration.semantic_identity())
        }
    };
    owner.accepts_opaque_value(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{
        RuntimeLocalDeclarationSeed, RuntimeNominalRecordDomainFieldSeed,
        RuntimeNominalRecordDomainSeed, RuntimePatternSeed, RuntimePatternSeedKind,
        RuntimePlanBuildError, RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
        RuntimeRecordFieldSeedId, RuntimeRecordPatternFieldSeed,
    };
    use crate::value::RuntimeNominalRecordValue;

    fn identity(marker: u8) -> RuntimeSemanticTypeId {
        RuntimeSemanticTypeId::from_bytes([marker; 32])
    }

    #[test]
    fn tuple_pattern_binds_plan_local_ids() {
        let mut builder = RuntimePlanBuilder::new();
        let admitted = builder
            .admit_semantic_batch(
                [
                    RuntimePlanTypeSeed::new(identity(1), RuntimePlanTypeProjection::Bool),
                    RuntimePlanTypeSeed::new(identity(2), RuntimePlanTypeProjection::String),
                    RuntimePlanTypeSeed::new(
                        identity(3),
                        RuntimePlanTypeProjection::Tuple(
                            vec![identity(1), identity(2)].into_boxed_slice(),
                        ),
                    ),
                ],
                [
                    RuntimeLocalDeclarationSeed::new(identity(1)),
                    RuntimeLocalDeclarationSeed::new(identity(2)),
                ],
                [],
                [],
            )
            .expect("typed tuple admission");
        let pattern = builder
            .lower_pattern_seed_for_test(RuntimePatternSeed::new(
                identity(3),
                RuntimePatternSeedKind::Tuple(
                    vec![
                        RuntimePatternSeed::new(
                            identity(1),
                            RuntimePatternSeedKind::Bind {
                                mutable: false,
                                local: admitted.local_ids()[0].clone(),
                            },
                        ),
                        RuntimePatternSeed::new(
                            identity(2),
                            RuntimePatternSeedKind::Typed {
                                local: admitted.local_ids()[1].clone(),
                            },
                        ),
                    ]
                    .into_boxed_slice(),
                ),
            ))
            .expect("admitted tuple pattern");
        let RuntimePatternKind::Tuple(items) = pattern.kind() else {
            panic!("tuple pattern kind");
        };
        let RuntimePatternKind::Bind {
            binding: bool_binding,
            ..
        } = items[0].kind()
        else {
            panic!("bool binding kind");
        };
        let RuntimePatternKind::Typed {
            binding: string_binding,
        } = items[1].kind()
        else {
            panic!("string binding kind");
        };
        let bool_local = bool_binding.local();
        let string_local = string_binding.local();
        let plan = builder.finish().expect("plan");
        let value = RuntimeValue::Tuple(vec![
            RuntimeValue::Bool(true),
            RuntimeValue::String("ready".to_owned()),
        ]);

        assert_eq!(
            match_runtime_pattern(&plan, &pattern, &value),
            Ok(Some(vec![
                RuntimeLocalBinding {
                    local: bool_local,
                    value: RuntimeValue::Bool(true),
                },
                RuntimeLocalBinding {
                    local: string_local,
                    value: RuntimeValue::String("ready".to_owned()),
                },
            ]))
        );
    }

    #[test]
    fn nominal_record_pattern_uses_owner_domain_and_field_id() {
        let nominal = RuntimeNominalTypeId::try_new("game.Pair").unwrap();
        let layout = TypeLayoutHash::from_bytes([7; 32]);
        let mut builder = RuntimePlanBuilder::new();
        let admitted = builder
            .admit_semantic_batch(
                [
                    RuntimePlanTypeSeed::new(
                        identity(1),
                        RuntimePlanTypeProjection::ProjectNominal {
                            nominal: nominal.clone(),
                            layout,
                            arguments: Box::new([]),
                        },
                    ),
                    RuntimePlanTypeSeed::new(identity(2), RuntimePlanTypeProjection::Bool),
                    RuntimePlanTypeSeed::new(identity(3), RuntimePlanTypeProjection::String),
                ],
                [RuntimeLocalDeclarationSeed::new(identity(2))],
                [RuntimeNominalRecordDomainSeed::new(
                    identity(1),
                    [
                        RuntimeNominalRecordDomainFieldSeed::new("alpha", identity(2)),
                        RuntimeNominalRecordDomainFieldSeed::new("zeta", identity(3)),
                    ],
                )],
                [],
            )
            .expect("record admission");
        let pattern = builder
            .lower_pattern_seed_for_test(RuntimePatternSeed::new(
                identity(1),
                RuntimePatternSeedKind::Record {
                    fields: Box::new([RuntimeRecordPatternFieldSeed::new(
                        RuntimeRecordFieldSeedId::from_zero_based(0),
                        RuntimePatternSeed::new(
                            identity(2),
                            RuntimePatternSeedKind::Bind {
                                mutable: false,
                                local: admitted.local_ids()[0].clone(),
                            },
                        ),
                    )]),
                    rest: crate::plan::RuntimePatternRestSeed::Ignore,
                },
            ))
            .expect("admitted record pattern");
        let RuntimePatternKind::Record { fields, .. } = pattern.kind() else {
            panic!("record pattern kind");
        };
        let RuntimePatternKind::Bind { binding, .. } = fields[0].pattern().kind() else {
            panic!("record binding kind");
        };
        let local = binding.local();
        let plan = builder.finish().expect("plan");
        let value = RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
            nominal,
            layout,
            vec![
                RuntimeValue::Bool(true),
                RuntimeValue::String("tail".to_owned()),
            ],
        ));

        assert_eq!(
            match_runtime_pattern(&plan, &pattern, &value),
            Ok(Some(vec![RuntimeLocalBinding {
                local,
                value: RuntimeValue::Bool(true),
            }]))
        );
    }

    #[test]
    fn binding_local_type_must_equal_the_pattern_node_type() {
        let mut builder = RuntimePlanBuilder::new();
        let admitted = builder
            .admit_semantic_batch(
                [
                    RuntimePlanTypeSeed::new(identity(1), RuntimePlanTypeProjection::Bool),
                    RuntimePlanTypeSeed::new(identity(2), RuntimePlanTypeProjection::String),
                ],
                [RuntimeLocalDeclarationSeed::new(identity(2))],
                [],
                [],
            )
            .expect("local admission");
        let result = builder.lower_pattern_seed_for_test(RuntimePatternSeed::new(
            identity(1),
            RuntimePatternSeedKind::Bind {
                mutable: false,
                local: admitted.local_ids()[0].clone(),
            },
        ));
        let plan = builder.finish().expect("plan");
        let bool_ty = plan
            .type_table()
            .id_for_semantic(identity(1))
            .expect("bool type");
        let string_ty = plan
            .type_table()
            .id_for_semantic(identity(2))
            .expect("string type");

        assert_eq!(
            result,
            Err(RuntimePlanBuildError::TypeMismatch {
                context: "pattern binding local",
                expected: bool_ty,
                actual: string_ty,
            })
        );
    }
}
