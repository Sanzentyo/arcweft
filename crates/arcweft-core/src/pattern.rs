use crate::entry::{RuntimeIdentityError, RuntimeNominalTypeId, TypeLayoutHash};
use crate::value::{
    RuntimeBinding, RuntimeEvalError, RuntimeOpaqueValue, RuntimeOpaqueValueError, RuntimeSeq,
    RuntimeSignedIntWidth, RuntimeUnsignedIntWidth, RuntimeValue, runtime_sequence_values,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
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

/// Pattern subset executable by the Sans I/O flow runtime.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimePattern {
    Ident(String),
    MutIdent(String),
    Discard,
    Literal(RuntimeValue),
    Entity(String),
    Tuple(Vec<RuntimePattern>),
    Record {
        owner: Option<RuntimeCheckedType>,
        fields: Vec<RuntimeRecordPatternField>,
        rest: bool,
    },
    BracketSeq {
        items: Vec<RuntimePattern>,
        rest: Option<String>,
    },
    Variant {
        owner: RuntimeCheckedType,
        ordinal: u32,
        name: String,
        payload: Option<Box<RuntimePattern>>,
    },
    Whole {
        name: String,
        pattern: Box<RuntimePattern>,
    },
    Typed {
        name: String,
        ty: RuntimeCheckedType,
    },
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
            (
                RuntimeValue::NominalRecord(record),
                Self::Nominal {
                    nominal, layout, ..
                },
            ) => record.type_id() == nominal && record.layout() == *layout,
            (
                RuntimeValue::Variant { owner, .. },
                Self::Variant {
                    nominal,
                    semantic_identity,
                    ..
                },
            ) => {
                owner
                    == &RuntimeVariantIdentity::Nominal {
                        nominal: nominal.clone(),
                        semantic_identity: *semantic_identity,
                    }
            }
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
            _ => false,
        }
    }
}

/// One field inside a runtime record pattern.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeRecordPatternField {
    pub name: String,
    pub pattern: RuntimePattern,
}

pub(crate) fn match_runtime_pattern(
    pattern: &RuntimePattern,
    value: &RuntimeValue,
) -> Result<Option<Vec<RuntimeBinding>>, RuntimeEvalError> {
    let mut bindings = Vec::with_capacity(pattern_binding_capacity(pattern));
    if collect_pattern_bindings(pattern, value, &mut bindings)? {
        reject_duplicate_bindings(&bindings)?;
        Ok(Some(bindings))
    } else {
        Ok(None)
    }
}

pub(crate) fn pattern_binding_capacity(pattern: &RuntimePattern) -> usize {
    let direct = match pattern {
        RuntimePattern::Ident(_) | RuntimePattern::MutIdent(_) | RuntimePattern::Typed { .. } => 1,
        RuntimePattern::Discard | RuntimePattern::Literal(_) | RuntimePattern::Entity(_) => 0,
        RuntimePattern::Tuple(patterns)
        | RuntimePattern::BracketSeq {
            items: patterns, ..
        } => patterns.iter().map(pattern_binding_capacity).sum(),
        RuntimePattern::Record { fields, .. } => fields
            .iter()
            .map(|field| pattern_binding_capacity(&field.pattern))
            .sum(),
        RuntimePattern::Variant { payload, .. } => {
            payload.as_deref().map_or(0, pattern_binding_capacity)
        }
        RuntimePattern::Whole { pattern, .. } => pattern_binding_capacity(pattern) + 1,
    };
    direct
        + usize::from(matches!(
            pattern,
            RuntimePattern::BracketSeq { rest: Some(_), .. }
        ))
}

fn reject_duplicate_bindings(bindings: &[RuntimeBinding]) -> Result<(), RuntimeEvalError> {
    let mut seen = BTreeSet::<&str>::new();
    for binding in bindings {
        if !seen.insert(binding.name.as_str()) {
            return Err(RuntimeEvalError::DuplicateBinding(binding.name.clone()));
        }
    }
    Ok(())
}

fn collect_pattern_bindings(
    pattern: &RuntimePattern,
    value: &RuntimeValue,
    bindings: &mut Vec<RuntimeBinding>,
) -> Result<bool, RuntimeEvalError> {
    match pattern {
        RuntimePattern::Ident(name) | RuntimePattern::MutIdent(name) => {
            bindings.push(RuntimeBinding {
                name: name.clone(),
                value: value.clone(),
            });
            Ok(true)
        }
        RuntimePattern::Typed { name, ty } => {
            if !ty.accepts_value(value) {
                return Ok(false);
            }
            bindings.push(RuntimeBinding {
                name: name.clone(),
                value: value.clone(),
            });
            Ok(true)
        }
        RuntimePattern::Discard => Ok(true),
        RuntimePattern::Literal(expected) => Ok(expected == value),
        RuntimePattern::Entity(expected) => {
            Ok(matches!(value, RuntimeValue::EntityRef(actual) if actual == expected))
        }
        RuntimePattern::Tuple(patterns) => {
            let RuntimeValue::Tuple(values) = value else {
                return Ok(false);
            };
            if patterns.len() != values.len() {
                return Ok(false);
            }
            collect_pattern_list(patterns, values, bindings)
        }
        RuntimePattern::Record {
            owner,
            fields,
            rest,
        } => collect_record_pattern_bindings(owner.as_ref(), fields, *rest, value, bindings),
        RuntimePattern::BracketSeq { items, rest } => {
            collect_bracket_seq_pattern_bindings(items, rest.as_deref(), value, bindings)
        }
        RuntimePattern::Variant {
            owner,
            ordinal,
            name,
            payload,
        } => collect_variant_pattern_bindings(
            owner,
            *ordinal,
            name,
            payload.as_deref(),
            value,
            bindings,
        ),
        RuntimePattern::Whole { name, pattern } => {
            if !collect_pattern_bindings(pattern, value, bindings)? {
                return Ok(false);
            }
            bindings.push(RuntimeBinding {
                name: name.clone(),
                value: value.clone(),
            });
            Ok(true)
        }
    }
}

fn collect_record_pattern_bindings(
    owner: Option<&RuntimeCheckedType>,
    fields: &[RuntimeRecordPatternField],
    rest: bool,
    value: &RuntimeValue,
    bindings: &mut Vec<RuntimeBinding>,
) -> Result<bool, RuntimeEvalError> {
    match (owner, value) {
        (None, RuntimeValue::Record(values)) => {
            if !rest && fields.len() != values.len() {
                return Ok(false);
            }
            for field in fields {
                let Some(value_field) =
                    values.iter().find(|candidate| candidate.name == field.name)
                else {
                    return Ok(false);
                };
                if !collect_pattern_bindings(&field.pattern, &value_field.value, bindings)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (
            Some(RuntimeCheckedType::Nominal { nominal, .. }),
            RuntimeValue::NominalRecord(record),
        ) if record.type_id() == nominal => {
            if (!rest && fields.len() != record.fields().len())
                || fields.len() > record.fields().len()
            {
                return Ok(false);
            }
            for (field, value) in fields.iter().zip(record.fields()) {
                if !collect_pattern_bindings(&field.pattern, value, bindings)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn collect_bracket_seq_pattern_bindings(
    items: &[RuntimePattern],
    rest: Option<&str>,
    value: &RuntimeValue,
    bindings: &mut Vec<RuntimeBinding>,
) -> Result<bool, RuntimeEvalError> {
    match value {
        RuntimeValue::Seq(RuntimeSeq::Values(values)) => {
            if !bracket_pattern_len_matches(items.len(), rest, values.len()) {
                return Ok(false);
            }
            if !collect_pattern_list(items, &values[..items.len()], bindings)? {
                return Ok(false);
            }
            if let Some(name) = rest {
                bindings.push(RuntimeBinding {
                    name: name.to_owned(),
                    value: runtime_sequence_values(values[items.len()..].to_vec()),
                });
            }
            Ok(true)
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(values)) => {
            if !bracket_pattern_len_matches(items.len(), rest, values.len()) {
                return Ok(false);
            }
            for (index, pattern) in items.iter().enumerate() {
                if !collect_pattern_bindings(pattern, &values.value_at(index), bindings)? {
                    return Ok(false);
                }
            }
            if let Some(name) = rest {
                bindings.push(RuntimeBinding {
                    name: name.to_owned(),
                    value: RuntimeValue::Seq(RuntimeSeq::Dense(values.tail_from(items.len()))),
                });
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn bracket_pattern_len_matches(pattern_len: usize, rest: Option<&str>, value_len: usize) -> bool {
    if rest.is_none() && pattern_len != value_len {
        return false;
    }
    if rest.is_some() && pattern_len > value_len {
        return false;
    }
    true
}

fn collect_variant_pattern_bindings(
    owner: &RuntimeCheckedType,
    ordinal: u32,
    name: &str,
    payload: Option<&RuntimePattern>,
    value: &RuntimeValue,
    bindings: &mut Vec<RuntimeBinding>,
) -> Result<bool, RuntimeEvalError> {
    let RuntimeValue::Variant {
        owner: actual_owner,
        ordinal: actual_ordinal,
        name: actual_name,
        payload: actual_payload,
    } = value
    else {
        return Ok(false);
    };
    if owner
        .variant_case(ordinal)
        .is_none_or(|case| case.name != name)
        || owner.variant_identity().as_ref() != Some(actual_owner)
        || ordinal != *actual_ordinal
        || name != actual_name
    {
        return Ok(false);
    }
    match (payload, actual_payload) {
        (Some(pattern), Some(value)) => collect_pattern_bindings(pattern, value, bindings),
        (None, None | Some(_)) => Ok(true),
        (Some(_), None) => Ok(false),
    }
}

fn collect_pattern_list(
    patterns: &[RuntimePattern],
    values: &[RuntimeValue],
    bindings: &mut Vec<RuntimeBinding>,
) -> Result<bool, RuntimeEvalError> {
    for (pattern, value) in patterns.iter().zip(values) {
        if !collect_pattern_bindings(pattern, value, bindings)? {
            return Ok(false);
        }
    }
    Ok(true)
}
