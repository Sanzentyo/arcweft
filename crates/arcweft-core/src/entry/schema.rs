//! Persistent runtime schemas, canonical value bytes, and validation.

use super::identity::{RuntimeNominalTypeId, RuntimeValueDigest, TypeLayoutHash};
use crate::pattern::RuntimeVariantIdentity;
use crate::value::{RuntimeInt, RuntimePayload, RuntimeUInt, RuntimeValue};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
/// Runtime-verifiable persistent data shape.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeTypeSchema {
    Unit,
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
    F32,
    F64,
    String,
    Char,
    Bytes {
        format: RuntimeBytesFormat,
    },
    Option(Box<Self>),
    Seq(Box<Self>),
    Map {
        key: Box<Self>,
        value: Box<Self>,
    },
    Record {
        name: String,
        fields: Vec<RuntimeSchemaField>,
        deny_unknown_fields: bool,
    },
    Enum {
        name: String,
        variants: Vec<RuntimeSchemaVariant>,
        tag: RuntimeEnumTagStyle,
        repr: Option<RuntimeEnumRepr>,
    },
    Named(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeSchemaField {
    pub rust_name: String,
    pub wire_name: String,
    pub schema: RuntimeTypeSchema,
    pub has_default: bool,
    pub skip: bool,
    pub bytes_format: Option<RuntimeBytesFormat>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeSchemaVariant {
    pub rust_name: String,
    pub wire_name: String,
    pub payload: Option<RuntimeTypeSchema>,
    pub discriminant: Option<i128>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeBytesFormat {
    Binary,
    Base64,
    Hex,
    Array,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeEnumTagStyle {
    External,
    Internal { tag: String },
    Adjacent { tag: String, content: String },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeEnumRepr {
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
}

/// Persistent-value validation limits used at startup, ingress, reducer,
/// save/restore, and replay boundaries.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeSchemaLimits {
    pub max_depth: usize,
    pub max_nodes: usize,
    pub max_sequence_items: usize,
    pub max_string_bytes: usize,
    pub max_encoded_bytes: usize,
}

impl RuntimeSchemaLimits {
    /// Intentional general-purpose engine limit set.
    ///
    /// Production entry construction must select this or another named policy
    /// explicitly; root startup never calls it implicitly.
    #[must_use]
    pub const fn engine_default() -> Self {
        Self {
            max_depth: 128,
            max_nodes: 262_144,
            max_sequence_items: 65_536,
            max_string_bytes: 1_048_576,
            max_encoded_bytes: 8_388_608,
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeSchemaError {
    #[error("runtime value at `{path}` has type `{actual}`, expected `{expected}`")]
    Type {
        path: String,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("runtime value at `{path}` contains non-finite {kind}")]
    NonFinite { path: String, kind: &'static str },
    #[error("runtime record at `{path}` contains duplicate field `{field}`")]
    DuplicateField { path: String, field: String },
    #[error("runtime record at `{path}` is missing field `{field}`")]
    MissingField { path: String, field: String },
    #[error("runtime record at `{path}` contains unknown field `{field}`")]
    UnknownField { path: String, field: String },
    #[error("runtime enum at `{path}` contains unknown variant `{variant}`")]
    UnknownVariant { path: String, variant: String },
    #[error("runtime enum variant at `{path}` has the wrong payload presence")]
    VariantPayload { path: String },
    #[error("runtime schema reference `{name}` at `{path}` is unresolved")]
    UnresolvedNamed { path: String, name: String },
    #[error("runtime value exceeds `{budget}` budget")]
    BudgetExceeded { budget: &'static str },
    #[error("runtime value canonical encoding failed: {message}")]
    Encoding { message: String },
    #[error("runtime nominal value at `{path}` has identity `{actual}`, expected `{expected}`")]
    NominalIdentity {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("runtime nominal value at `{path}` has the wrong accepted layout")]
    NominalLayout { path: String },
    #[error("runtime schema canonical encoding exceeds u32 collection limits")]
    SchemaEncodingOverflow,
}

impl RuntimeTypeSchema {
    pub fn try_layout_hash(&self) -> Result<TypeLayoutHash, RuntimeSchemaError> {
        let bytes = self
            .canonical_bytes()
            .ok_or(RuntimeSchemaError::SchemaEncodingOverflow)?;
        Ok(TypeLayoutHash::from_bytes(blake3::hash(&bytes).into()))
    }

    pub fn validate_payload(
        &self,
        payload: &RuntimePayload,
        limits: RuntimeSchemaLimits,
    ) -> Result<RuntimeValueDigest, RuntimeSchemaError> {
        self.validate_value(&payload.0, limits)
    }

    /// Validates a nominal root payload against its exact accepted role and
    /// this structural persistence schema.
    pub fn validate_nominal_payload(
        &self,
        payload: &RuntimePayload,
        expected_identity: &RuntimeNominalTypeId,
        expected_layout: TypeLayoutHash,
        limits: RuntimeSchemaLimits,
    ) -> Result<RuntimeValueDigest, RuntimeSchemaError> {
        let definitions = schema_definitions(self);
        let mut state = SchemaValidationState {
            limits,
            nodes: 0,
            definitions,
        };
        match (self, &payload.0) {
            (Self::Record { fields, .. }, RuntimeValue::NominalRecord(record)) => {
                validate_nominal_identity(expected_identity, record.type_id(), "$")?;
                if record.layout() != expected_layout {
                    return Err(RuntimeSchemaError::NominalLayout {
                        path: "$".to_owned(),
                    });
                }
                state.validate_nominal_record(fields, record.fields(), "$", 0)?;
            }
            (
                Self::Enum { variants, .. },
                RuntimeValue::Variant {
                    owner: RuntimeVariantIdentity::Nominal { nominal, .. },
                    ordinal,
                    name,
                    payload,
                },
            ) => {
                validate_nominal_identity(expected_identity, nominal, "$")?;
                state.validate_enum(variants, *ordinal, name, payload.as_deref(), "$", 0)?;
            }
            _ => state.validate(self, &payload.0, "$", 0)?,
        }
        let encoded = canonical_runtime_value_bytes(&payload.0, limits.max_encoded_bytes)?;
        Ok(RuntimeValueDigest::from_bytes(
            blake3::hash(&encoded).into(),
        ))
    }

    pub fn validate_value(
        &self,
        value: &RuntimeValue,
        limits: RuntimeSchemaLimits,
    ) -> Result<RuntimeValueDigest, RuntimeSchemaError> {
        let definitions = schema_definitions(self);
        let mut state = SchemaValidationState {
            limits,
            nodes: 0,
            definitions,
        };
        state.validate(self, value, "$", 0)?;
        let encoded = canonical_runtime_value_bytes(value, limits.max_encoded_bytes)?;
        Ok(RuntimeValueDigest::from_bytes(
            blake3::hash(&encoded).into(),
        ))
    }

    fn canonical_bytes(&self) -> Option<Vec<u8>> {
        let mut bytes = CanonicalSchemaBytes::new();
        bytes.schema(self)?;
        Some(bytes.finish())
    }
}

fn validate_nominal_identity(
    expected: &RuntimeNominalTypeId,
    actual: &RuntimeNominalTypeId,
    path: &str,
) -> Result<(), RuntimeSchemaError> {
    if expected == actual {
        Ok(())
    } else {
        Err(RuntimeSchemaError::NominalIdentity {
            path: path.to_owned(),
            expected: expected.as_str().to_owned(),
            actual: actual.as_str().to_owned(),
        })
    }
}

/// Produces the sole replay/save digest encoding for runtime values.
///
/// Record fields are ordered by field identity, integers retain their exact
/// width, and every collection length is checked before it is encoded.
pub fn canonical_runtime_value_bytes(
    value: &RuntimeValue,
    max_encoded_bytes: usize,
) -> Result<Vec<u8>, RuntimeSchemaError> {
    let mut bytes = CanonicalRuntimeValueBytes::new(max_encoded_bytes);
    bytes.value(value)?;
    Ok(bytes.finish())
}

struct CanonicalRuntimeValueBytes {
    bytes: Vec<u8>,
    max_encoded_bytes: usize,
}

impl CanonicalRuntimeValueBytes {
    fn new(max_encoded_bytes: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(max_encoded_bytes.min(4096)),
            max_encoded_bytes,
        }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn extend(&mut self, bytes: &[u8]) -> Result<(), RuntimeSchemaError> {
        let next = self.bytes.len().checked_add(bytes.len()).ok_or(
            RuntimeSchemaError::BudgetExceeded {
                budget: "encoded_bytes",
            },
        )?;
        if next > self.max_encoded_bytes {
            return Err(RuntimeSchemaError::BudgetExceeded {
                budget: "encoded_bytes",
            });
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<(), RuntimeSchemaError> {
        self.extend(&[value])
    }

    fn u32(&mut self, value: u32) -> Result<(), RuntimeSchemaError> {
        self.extend(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), RuntimeSchemaError> {
        self.extend(&value.to_le_bytes())
    }

    fn u128(&mut self, value: u128) -> Result<(), RuntimeSchemaError> {
        self.extend(&value.to_le_bytes())
    }

    fn i128(&mut self, value: i128) -> Result<(), RuntimeSchemaError> {
        self.extend(&value.to_le_bytes())
    }

    fn len(&mut self, value: usize) -> Result<(), RuntimeSchemaError> {
        let value = u32::try_from(value).map_err(|_| RuntimeSchemaError::Encoding {
            message: "runtime value collection length does not fit u32".to_owned(),
        })?;
        self.u32(value)
    }

    fn string(&mut self, value: &str) -> Result<(), RuntimeSchemaError> {
        self.len(value.len())?;
        self.extend(value.as_bytes())
    }

    fn option<T: ?Sized>(
        &mut self,
        value: Option<&T>,
        encode: impl FnOnce(&mut Self, &T) -> Result<(), RuntimeSchemaError>,
    ) -> Result<(), RuntimeSchemaError> {
        match value {
            Some(value) => {
                self.u8(1)?;
                encode(self, value)
            }
            None => self.u8(0),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive encoder owns the stable runtime-value byte contract"
    )]
    fn value(&mut self, value: &RuntimeValue) -> Result<(), RuntimeSchemaError> {
        match value {
            RuntimeValue::Unit => self.u8(1),
            RuntimeValue::Bool(value) => {
                self.u8(2)?;
                self.u8(u8::from(*value))
            }
            RuntimeValue::Int(value) => {
                self.u8(3)?;
                let (width, value) = match value {
                    RuntimeInt::I8(value) => (1, i128::from(*value)),
                    RuntimeInt::I16(value) => (2, i128::from(*value)),
                    RuntimeInt::I32(value) => (3, i128::from(*value)),
                    RuntimeInt::I64(value) => (4, i128::from(*value)),
                    RuntimeInt::I128(value) => (5, *value),
                    RuntimeInt::ISize(value) => (6, i128::from(*value)),
                };
                self.u8(width)?;
                self.i128(value)
            }
            RuntimeValue::UInt(value) => {
                self.u8(4)?;
                let (width, value) = match value {
                    RuntimeUInt::U8(value) => (1, u128::from(*value)),
                    RuntimeUInt::U16(value) => (2, u128::from(*value)),
                    RuntimeUInt::U32(value) => (3, u128::from(*value)),
                    RuntimeUInt::U64(value) => (4, u128::from(*value)),
                    RuntimeUInt::U128(value) => (5, *value),
                    RuntimeUInt::USize(value) => (6, u128::from(*value)),
                };
                self.u8(width)?;
                self.u128(value)
            }
            RuntimeValue::F32(value) => {
                if !value.is_finite() {
                    return Err(RuntimeSchemaError::NonFinite {
                        path: "$".to_owned(),
                        kind: "f32",
                    });
                }
                self.u8(5)?;
                self.u32(if *value == 0.0 { 0 } else { value.to_bits() })
            }
            RuntimeValue::F64(value) => {
                if !value.is_finite() {
                    return Err(RuntimeSchemaError::NonFinite {
                        path: "$".to_owned(),
                        kind: "f64",
                    });
                }
                self.u8(6)?;
                self.u64(if *value == 0.0 { 0 } else { value.to_bits() })
            }
            RuntimeValue::String(value) => {
                self.u8(7)?;
                self.string(value)
            }
            RuntimeValue::Char(value) => {
                self.u8(8)?;
                self.u32(u32::from(*value))
            }
            RuntimeValue::Duration(value) => {
                self.u8(9)?;
                self.u64(value.as_nanos())
            }
            RuntimeValue::Progress(value) => {
                self.u8(19)?;
                self.u32(if value.ratio() == 0.0 {
                    0
                } else {
                    value.ratio().to_bits()
                })?;
                self.option(value.label(), Self::string)
            }
            RuntimeValue::EntityRef(value) => {
                self.u8(10)?;
                self.string(value)
            }
            RuntimeValue::Tuple(values) => {
                self.u8(11)?;
                self.len(values.len())?;
                for value in values {
                    self.value(value)?;
                }
                Ok(())
            }
            RuntimeValue::Seq(values) => {
                self.u8(12)?;
                let values = values.clone().into_values();
                self.len(values.len())?;
                for value in &values {
                    self.value(value)?;
                }
                Ok(())
            }
            RuntimeValue::Record(fields) => {
                self.u8(13)?;
                self.len(fields.len())?;
                let mut fields = fields.iter().collect::<Vec<_>>();
                fields.sort_unstable_by(|left, right| left.name().cmp(right.name()));
                if fields
                    .windows(2)
                    .any(|pair| pair[0].name() == pair[1].name())
                {
                    return Err(RuntimeSchemaError::DuplicateField {
                        path: "$".to_owned(),
                        field: fields
                            .windows(2)
                            .find(|pair| pair[0].name() == pair[1].name())
                            .expect("duplicate was just detected")[0]
                            .name()
                            .to_owned(),
                    });
                }
                for field in fields {
                    self.string(field.name())?;
                    self.value(field.value())?;
                }
                Ok(())
            }
            RuntimeValue::NominalRecord(record) => {
                self.u8(15)?;
                self.string(record.type_id().as_str())?;
                self.extend(record.layout().as_bytes())?;
                self.len(record.fields().len())?;
                for field in record.fields() {
                    self.value(field)?;
                }
                Ok(())
            }
            RuntimeValue::Opaque(value) => {
                self.u8(16)?;
                self.string(value.producer().as_str())?;
                self.extend(value.semantic_identity().as_bytes())?;
                self.value(value.payload())
            }
            RuntimeValue::Reduction(value) => {
                self.u8(18)?;
                self.string(value.owner().producer().as_str())?;
                self.extend(value.owner().semantic_identity().as_bytes())?;
                self.value(value.state())?;
                self.len(value.commands().len())?;
                for command in value.commands() {
                    self.string(command.constructor().as_str())?;
                    self.string(command.target().as_str())?;
                    self.value(&command.payload().0)?;
                }
                Ok(())
            }
            RuntimeValue::Agent(value) => {
                self.u8(17)?;
                self.agent_value(value)
            }
            RuntimeValue::Variant {
                owner,
                ordinal,
                name,
                payload,
            } => {
                self.u8(14)?;
                self.variant_identity(owner)?;
                self.u32(*ordinal)?;
                self.string(name)?;
                self.option(payload.as_deref(), Self::value)
            }
            RuntimeValue::MatrixF32(_)
            | RuntimeValue::MatrixF64(_)
            | RuntimeValue::TensorF32(_)
            | RuntimeValue::TensorF64(_)
            | RuntimeValue::Range(_)
            | RuntimeValue::Iterator(_)
            | RuntimeValue::Function(_) => Err(RuntimeSchemaError::Encoding {
                message: "runtime-only value has no replay/save encoding".to_owned(),
            }),
        }
    }

    fn agent_value(
        &mut self,
        value: &crate::value::RuntimeAgentValue,
    ) -> Result<(), RuntimeSchemaError> {
        use crate::value::RuntimeAgentCaptureTarget;
        match value {
            crate::value::RuntimeAgentValue::ActionTarget(target) => {
                self.u8(0)?;
                self.string(target.id().as_str())?;
                self.string(target.target().as_str())?;
                self.u8(match target.action() {
                    crate::value::RuntimeAgentAction::AdvanceText => 0,
                    crate::value::RuntimeAgentAction::SelectChoice => 1,
                    crate::value::RuntimeAgentAction::Invoke => 2,
                    crate::value::RuntimeAgentAction::Scroll => 3,
                    crate::value::RuntimeAgentAction::PointerClick => 4,
                })?;
                self.u8(match target.dispatch() {
                    crate::value::RuntimeAgentActionDispatch::Semantic => 0,
                    crate::value::RuntimeAgentActionDispatch::Physical => 1,
                })?;
                self.u8(u8::from(target.enabled()))
            }
            crate::value::RuntimeAgentValue::CaptureTarget(target) => {
                self.u8(1)?;
                match target {
                    RuntimeAgentCaptureTarget::Viewport => self.u8(0),
                    RuntimeAgentCaptureTarget::Layer { target } => {
                        self.u8(1)?;
                        self.string(target.as_str())
                    }
                    RuntimeAgentCaptureTarget::Object { target } => {
                        self.u8(2)?;
                        self.string(target.as_str())
                    }
                }
            }
            crate::value::RuntimeAgentValue::DebugStatePath(path) => {
                self.u8(2)?;
                self.string(path.as_str())
            }
            crate::value::RuntimeAgentValue::ObservationFieldPath(path) => {
                self.u8(3)?;
                self.string(path.as_str())
            }
            crate::value::RuntimeAgentValue::Probe(probe) => {
                self.u8(4)?;
                self.agent_probe(probe)
            }
            crate::value::RuntimeAgentValue::Diagnostics => self.u8(5),
            crate::value::RuntimeAgentValue::Predicate(predicate) => {
                self.u8(6)?;
                self.agent_predicate(predicate)
            }
            crate::value::RuntimeAgentValue::ViewportPoint { x, y } => {
                self.u8(7)?;
                self.u32(*x)?;
                self.u32(*y)
            }
        }
    }

    fn agent_probe(
        &mut self,
        probe: &crate::value::RuntimeAgentProbe,
    ) -> Result<(), RuntimeSchemaError> {
        use crate::value::RuntimeAgentProbe;
        match probe {
            RuntimeAgentProbe::Signal { target } => {
                self.u8(0)?;
                self.string(target.as_str())
            }
            RuntimeAgentProbe::Metric { target } => {
                self.u8(1)?;
                self.string(target.as_str())
            }
            RuntimeAgentProbe::StatePath { path } => {
                self.u8(2)?;
                self.string(path.as_str())
            }
            RuntimeAgentProbe::ObservationField { path } => {
                self.u8(3)?;
                self.string(path.as_str())
            }
        }
    }

    fn agent_predicate(
        &mut self,
        predicate: &crate::value::RuntimeAgentPredicate,
    ) -> Result<(), RuntimeSchemaError> {
        use crate::value::RuntimeAgentPredicate;
        match predicate {
            RuntimeAgentPredicate::Compare { probe, op, value } => {
                self.u8(0)?;
                self.agent_probe(probe)?;
                self.u8(match op {
                    crate::value::RuntimeAgentCompareOp::Eq => 0,
                    crate::value::RuntimeAgentCompareOp::NotEq => 1,
                    crate::value::RuntimeAgentCompareOp::Greater => 2,
                    crate::value::RuntimeAgentCompareOp::GreaterOrEqual => 3,
                    crate::value::RuntimeAgentCompareOp::Less => 4,
                    crate::value::RuntimeAgentCompareOp::LessOrEqual => 5,
                })?;
                self.value(value)
            }
            RuntimeAgentPredicate::Exists { probe } => {
                self.u8(1)?;
                self.agent_probe(probe)
            }
            RuntimeAgentPredicate::ActionEnabled { target } => {
                self.u8(2)?;
                self.string(target.as_str())
            }
            RuntimeAgentPredicate::DiagnosticsHasError => self.u8(3),
            RuntimeAgentPredicate::All { predicates } => {
                self.u8(4)?;
                self.len(predicates.len())?;
                for predicate in predicates {
                    self.agent_predicate(predicate)?;
                }
                Ok(())
            }
            RuntimeAgentPredicate::Any { predicates } => {
                self.u8(5)?;
                self.len(predicates.len())?;
                for predicate in predicates {
                    self.agent_predicate(predicate)?;
                }
                Ok(())
            }
            RuntimeAgentPredicate::Not { predicate } => {
                self.u8(6)?;
                self.agent_predicate(predicate)
            }
        }
    }

    fn variant_identity(
        &mut self,
        identity: &RuntimeVariantIdentity,
    ) -> Result<(), RuntimeSchemaError> {
        match identity {
            RuntimeVariantIdentity::Nominal {
                nominal,
                semantic_identity,
            } => {
                self.u8(0)?;
                self.string(nominal.as_str())?;
                self.extend(semantic_identity.as_bytes())
            }
            RuntimeVariantIdentity::Option => self.u8(1),
            RuntimeVariantIdentity::Result => self.u8(2),
        }
    }
}

struct CanonicalSchemaBytes(Vec<u8>);

impl CanonicalSchemaBytes {
    fn new() -> Self {
        let mut bytes = Self(Vec::with_capacity(256));
        bytes.0.extend_from_slice(b"arcweft.nominal-schema\0");
        bytes.u32(1);
        bytes
    }

    fn finish(self) -> Vec<u8> {
        self.0
    }

    fn u8(&mut self, value: u8) {
        self.0.push(value);
    }

    fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }

    fn i128(&mut self, value: i128) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }

    fn len(&mut self, value: usize) -> Option<()> {
        self.u32(u32::try_from(value).ok()?);
        Some(())
    }

    fn string(&mut self, value: &str) -> Option<()> {
        self.len(value.len())?;
        self.0.extend_from_slice(value.as_bytes());
        Some(())
    }

    fn option<T>(
        &mut self,
        value: Option<&T>,
        encode: impl FnOnce(&mut Self, &T) -> Option<()>,
    ) -> Option<()> {
        if let Some(value) = value {
            self.u8(1);
            encode(self, value)
        } else {
            self.u8(0);
            Some(())
        }
    }

    fn schema(&mut self, schema: &RuntimeTypeSchema) -> Option<()> {
        match schema {
            RuntimeTypeSchema::Unit => self.u8(1),
            RuntimeTypeSchema::Bool => self.u8(2),
            RuntimeTypeSchema::I8 => self.u8(3),
            RuntimeTypeSchema::I16 => self.u8(4),
            RuntimeTypeSchema::I32 => self.u8(5),
            RuntimeTypeSchema::I64 => self.u8(6),
            RuntimeTypeSchema::I128 => self.u8(7),
            RuntimeTypeSchema::ISize => self.u8(8),
            RuntimeTypeSchema::U8 => self.u8(9),
            RuntimeTypeSchema::U16 => self.u8(10),
            RuntimeTypeSchema::U32 => self.u8(11),
            RuntimeTypeSchema::U64 => self.u8(12),
            RuntimeTypeSchema::U128 => self.u8(13),
            RuntimeTypeSchema::USize => self.u8(14),
            RuntimeTypeSchema::F32 => self.u8(15),
            RuntimeTypeSchema::F64 => self.u8(16),
            RuntimeTypeSchema::String => self.u8(17),
            RuntimeTypeSchema::Char => self.u8(18),
            RuntimeTypeSchema::Bytes { format } => {
                self.u8(19);
                self.u8(format.tag());
            }
            RuntimeTypeSchema::Option(inner) => {
                self.u8(20);
                self.schema(inner)?;
            }
            RuntimeTypeSchema::Seq(inner) => {
                self.u8(21);
                self.schema(inner)?;
            }
            RuntimeTypeSchema::Map { key, value } => {
                self.u8(22);
                self.schema(key)?;
                self.schema(value)?;
            }
            RuntimeTypeSchema::Record {
                name,
                fields,
                deny_unknown_fields,
            } => {
                self.u8(23);
                self.string(name)?;
                self.bool(*deny_unknown_fields);
                self.len(fields.len())?;
                for field in fields {
                    self.string(&field.rust_name)?;
                    self.string(&field.wire_name)?;
                    self.schema(&field.schema)?;
                    self.bool(field.has_default);
                    self.bool(field.skip);
                    self.option(field.bytes_format.as_ref(), |bytes, format| {
                        bytes.u8(format.tag());
                        Some(())
                    })?;
                }
            }
            RuntimeTypeSchema::Enum {
                name,
                variants,
                tag,
                repr,
            } => {
                self.u8(24);
                self.string(name)?;
                match tag {
                    RuntimeEnumTagStyle::External => self.u8(1),
                    RuntimeEnumTagStyle::Internal { tag } => {
                        self.u8(2);
                        self.string(tag)?;
                    }
                    RuntimeEnumTagStyle::Adjacent { tag, content } => {
                        self.u8(3);
                        self.string(tag)?;
                        self.string(content)?;
                    }
                }
                self.option(repr.as_ref(), |bytes, repr| {
                    bytes.u8(repr.tag());
                    Some(())
                })?;
                self.len(variants.len())?;
                for variant in variants {
                    self.string(&variant.rust_name)?;
                    self.string(&variant.wire_name)?;
                    self.option(variant.payload.as_ref(), Self::schema)?;
                    self.option(variant.discriminant.as_ref(), |bytes, value| {
                        bytes.i128(*value);
                        Some(())
                    })?;
                }
            }
            RuntimeTypeSchema::Named(name) => {
                self.u8(25);
                self.string(name)?;
            }
        }
        Some(())
    }
}

impl RuntimeBytesFormat {
    const fn tag(self) -> u8 {
        match self {
            Self::Binary => 1,
            Self::Base64 => 2,
            Self::Hex => 3,
            Self::Array => 4,
        }
    }
}

impl RuntimeEnumRepr {
    const fn tag(self) -> u8 {
        match self {
            Self::I8 => 1,
            Self::I16 => 2,
            Self::I32 => 3,
            Self::I64 => 4,
            Self::I128 => 5,
            Self::ISize => 6,
            Self::U8 => 7,
            Self::U16 => 8,
            Self::U32 => 9,
            Self::U64 => 10,
            Self::U128 => 11,
            Self::USize => 12,
        }
    }
}

struct SchemaValidationState<'a> {
    limits: RuntimeSchemaLimits,
    nodes: usize,
    definitions: BTreeMap<&'a str, &'a RuntimeTypeSchema>,
}

impl<'a> SchemaValidationState<'a> {
    #[allow(
        clippy::too_many_lines,
        reason = "one recursive schema validator owns the shared depth/node budget and closed variant matrix"
    )]
    fn validate(
        &mut self,
        schema: &'a RuntimeTypeSchema,
        value: &RuntimeValue,
        path: &str,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError> {
        if depth > self.limits.max_depth {
            return Err(RuntimeSchemaError::BudgetExceeded { budget: "depth" });
        }
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or(RuntimeSchemaError::BudgetExceeded { budget: "nodes" })?;
        if self.nodes > self.limits.max_nodes {
            return Err(RuntimeSchemaError::BudgetExceeded { budget: "nodes" });
        }
        match (schema, value) {
            (RuntimeTypeSchema::Unit, RuntimeValue::Unit)
            | (RuntimeTypeSchema::Bool, RuntimeValue::Bool(_))
            | (RuntimeTypeSchema::I8, RuntimeValue::Int(RuntimeInt::I8(_)))
            | (RuntimeTypeSchema::I16, RuntimeValue::Int(RuntimeInt::I16(_)))
            | (RuntimeTypeSchema::I32, RuntimeValue::Int(RuntimeInt::I32(_)))
            | (RuntimeTypeSchema::I64, RuntimeValue::Int(RuntimeInt::I64(_)))
            | (RuntimeTypeSchema::I128, RuntimeValue::Int(RuntimeInt::I128(_)))
            | (RuntimeTypeSchema::ISize, RuntimeValue::Int(RuntimeInt::ISize(_)))
            | (RuntimeTypeSchema::U8, RuntimeValue::UInt(RuntimeUInt::U8(_)))
            | (RuntimeTypeSchema::U16, RuntimeValue::UInt(RuntimeUInt::U16(_)))
            | (RuntimeTypeSchema::U32, RuntimeValue::UInt(RuntimeUInt::U32(_)))
            | (RuntimeTypeSchema::U64, RuntimeValue::UInt(RuntimeUInt::U64(_)))
            | (RuntimeTypeSchema::U128, RuntimeValue::UInt(RuntimeUInt::U128(_)))
            | (RuntimeTypeSchema::USize, RuntimeValue::UInt(RuntimeUInt::USize(_)))
            | (RuntimeTypeSchema::Char, RuntimeValue::Char(_)) => Ok(()),
            (RuntimeTypeSchema::F32, RuntimeValue::F32(value)) if value.is_finite() => Ok(()),
            (RuntimeTypeSchema::F64, RuntimeValue::F64(value)) if value.is_finite() => Ok(()),
            (RuntimeTypeSchema::F32, RuntimeValue::F32(_)) => Err(RuntimeSchemaError::NonFinite {
                path: path.to_owned(),
                kind: "f32",
            }),
            (RuntimeTypeSchema::F64, RuntimeValue::F64(_)) => Err(RuntimeSchemaError::NonFinite {
                path: path.to_owned(),
                kind: "f64",
            }),
            (RuntimeTypeSchema::String, RuntimeValue::String(value)) => {
                if value.len() > self.limits.max_string_bytes {
                    Err(RuntimeSchemaError::BudgetExceeded {
                        budget: "string_bytes",
                    })
                } else {
                    Ok(())
                }
            }
            (RuntimeTypeSchema::Bytes { .. }, RuntimeValue::Seq(sequence)) => {
                let values = sequence.clone().into_values();
                self.validate_sequence(&RuntimeTypeSchema::U8, &values, path, depth)
            }
            (
                RuntimeTypeSchema::Option(inner),
                RuntimeValue::Variant {
                    owner: RuntimeVariantIdentity::Option,
                    ordinal,
                    name,
                    payload,
                },
            ) => match (*ordinal, name.as_str(), payload.as_deref()) {
                (1, "None", None) => Ok(()),
                (0, "Some", Some(payload)) => {
                    self.validate(inner, payload, &format!("{path}.Some"), depth + 1)
                }
                _ => Err(RuntimeSchemaError::VariantPayload {
                    path: path.to_owned(),
                }),
            },
            (RuntimeTypeSchema::Seq(inner), RuntimeValue::Seq(sequence)) => {
                let values = sequence.clone().into_values();
                self.validate_sequence(inner, &values, path, depth)
            }
            (RuntimeTypeSchema::Map { key, value }, RuntimeValue::Seq(sequence)) => {
                let values = sequence.clone().into_values();
                self.validate_map(key, value, &values, path, depth)
            }
            (RuntimeTypeSchema::Record { fields, .. }, RuntimeValue::Record(values)) => {
                self.validate_record(fields, values, path, depth)
            }
            (
                RuntimeTypeSchema::Record { name, fields, .. },
                RuntimeValue::NominalRecord(record),
            ) if record.type_id().as_str() == name => {
                self.validate_nominal_record(fields, record.fields(), path, depth)
            }
            (
                RuntimeTypeSchema::Enum {
                    name: owner_name,
                    variants,
                    ..
                },
                RuntimeValue::Variant {
                    owner: RuntimeVariantIdentity::Nominal { nominal, .. },
                    ordinal,
                    name,
                    payload,
                },
            ) if nominal.as_str() == owner_name => {
                self.validate_enum(variants, *ordinal, name, payload.as_deref(), path, depth)
            }
            (RuntimeTypeSchema::Named(name), _) => {
                let schema = self
                    .definitions
                    .get(name.as_str())
                    .copied()
                    .ok_or_else(|| RuntimeSchemaError::UnresolvedNamed {
                        path: path.to_owned(),
                        name: name.clone(),
                    })?;
                self.validate(schema, value, path, depth + 1)
            }
            _ => Err(type_error(path, schema.type_label(), value)),
        }
    }

    fn validate_map(
        &mut self,
        key: &'a RuntimeTypeSchema,
        value: &'a RuntimeTypeSchema,
        entries: &[RuntimeValue],
        path: &str,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError> {
        if entries.len() > self.limits.max_sequence_items {
            return Err(RuntimeSchemaError::BudgetExceeded {
                budget: "sequence_items",
            });
        }
        for (index, entry) in entries.iter().enumerate() {
            let RuntimeValue::Tuple(items) = entry else {
                return Err(type_error(path, "map entry tuple", entry));
            };
            if items.len() != 2 {
                return Err(type_error(path, "two-item map entry tuple", entry));
            }
            self.validate(key, &items[0], &format!("{path}[{index}].key"), depth + 1)?;
            self.validate(
                value,
                &items[1],
                &format!("{path}[{index}].value"),
                depth + 1,
            )?;
        }
        Ok(())
    }

    fn validate_record(
        &mut self,
        fields: &'a [RuntimeSchemaField],
        values: &[crate::value::RuntimeFieldValue],
        path: &str,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError> {
        let mut actual = BTreeMap::new();
        for field in values {
            if actual.insert(field.name(), field.value()).is_some() {
                return Err(RuntimeSchemaError::DuplicateField {
                    path: path.to_owned(),
                    field: field.name().to_owned(),
                });
            }
        }
        let expected = fields
            .iter()
            .filter(|field| !field.skip)
            .map(|field| field.rust_name.as_str())
            .collect::<BTreeSet<_>>();
        if let Some(unknown) = actual.keys().find(|name| !expected.contains(**name)) {
            return Err(RuntimeSchemaError::UnknownField {
                path: path.to_owned(),
                field: (*unknown).to_owned(),
            });
        }
        for field in fields {
            if field.skip {
                continue;
            }
            let Some(value) = actual.get(field.rust_name.as_str()) else {
                if field.has_default {
                    continue;
                }
                return Err(RuntimeSchemaError::MissingField {
                    path: path.to_owned(),
                    field: field.rust_name.clone(),
                });
            };
            self.validate(
                &field.schema,
                value,
                &format!("{path}.{}", field.rust_name),
                depth + 1,
            )?;
        }
        Ok(())
    }

    fn validate_nominal_record(
        &mut self,
        fields: &'a [RuntimeSchemaField],
        values: &[RuntimeValue],
        path: &str,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError> {
        if values.len() != fields.len() {
            return Err(RuntimeSchemaError::Encoding {
                message: format!(
                    "nominal record at `{path}` has {} fields, expected {}",
                    values.len(),
                    fields.len()
                ),
            });
        }
        for (field, value) in fields.iter().zip(values) {
            self.validate(
                &field.schema,
                value,
                &format!("{path}.{}", field.rust_name),
                depth + 1,
            )?;
        }
        Ok(())
    }

    fn validate_enum(
        &mut self,
        variants: &'a [RuntimeSchemaVariant],
        ordinal: u32,
        name: &str,
        payload: Option<&RuntimeValue>,
        path: &str,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError> {
        let variant = usize::try_from(ordinal)
            .ok()
            .and_then(|ordinal| variants.get(ordinal))
            .filter(|variant| variant.rust_name == name)
            .ok_or_else(|| RuntimeSchemaError::UnknownVariant {
                path: path.to_owned(),
                variant: name.to_owned(),
            })?;
        match (&variant.payload, payload) {
            (None, None) => Ok(()),
            (Some(schema), Some(value)) => {
                self.validate(schema, value, &format!("{path}.{name}"), depth + 1)
            }
            _ => Err(RuntimeSchemaError::VariantPayload {
                path: path.to_owned(),
            }),
        }
    }

    fn validate_sequence(
        &mut self,
        schema: &'a RuntimeTypeSchema,
        values: &[RuntimeValue],
        path: &str,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError> {
        if values.len() > self.limits.max_sequence_items {
            return Err(RuntimeSchemaError::BudgetExceeded {
                budget: "sequence_items",
            });
        }
        for (index, value) in values.iter().enumerate() {
            self.validate(schema, value, &format!("{path}[{index}]"), depth + 1)?;
        }
        Ok(())
    }
}

impl RuntimeTypeSchema {
    const fn type_label(&self) -> &'static str {
        match self {
            Self::Unit => "unit",
            Self::Bool => "bool",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::I128 => "i128",
            Self::ISize => "isize",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::U128 => "u128",
            Self::USize => "usize",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::String => "string",
            Self::Char => "char",
            Self::Bytes { .. } => "bytes",
            Self::Option(_) => "option",
            Self::Seq(_) => "sequence",
            Self::Map { .. } => "map",
            Self::Record { .. } => "record",
            Self::Enum { .. } => "enum",
            Self::Named(_) => "named value",
        }
    }
}

fn schema_definitions(schema: &RuntimeTypeSchema) -> BTreeMap<&str, &RuntimeTypeSchema> {
    fn visit<'a>(
        schema: &'a RuntimeTypeSchema,
        definitions: &mut BTreeMap<&'a str, &'a RuntimeTypeSchema>,
    ) {
        match schema {
            RuntimeTypeSchema::Record { name, fields, .. } => {
                definitions.insert(name, schema);
                for field in fields {
                    visit(&field.schema, definitions);
                }
            }
            RuntimeTypeSchema::Enum { name, variants, .. } => {
                definitions.insert(name, schema);
                for variant in variants {
                    if let Some(payload) = &variant.payload {
                        visit(payload, definitions);
                    }
                }
            }
            RuntimeTypeSchema::Option(inner) | RuntimeTypeSchema::Seq(inner) => {
                visit(inner, definitions);
            }
            RuntimeTypeSchema::Map { key, value } => {
                visit(key, definitions);
                visit(value, definitions);
            }
            RuntimeTypeSchema::Unit
            | RuntimeTypeSchema::Bool
            | RuntimeTypeSchema::I8
            | RuntimeTypeSchema::I16
            | RuntimeTypeSchema::I32
            | RuntimeTypeSchema::I64
            | RuntimeTypeSchema::I128
            | RuntimeTypeSchema::ISize
            | RuntimeTypeSchema::U8
            | RuntimeTypeSchema::U16
            | RuntimeTypeSchema::U32
            | RuntimeTypeSchema::U64
            | RuntimeTypeSchema::U128
            | RuntimeTypeSchema::USize
            | RuntimeTypeSchema::F32
            | RuntimeTypeSchema::F64
            | RuntimeTypeSchema::String
            | RuntimeTypeSchema::Char
            | RuntimeTypeSchema::Bytes { .. }
            | RuntimeTypeSchema::Named(_) => {}
        }
    }
    let mut definitions = BTreeMap::new();
    visit(schema, &mut definitions);
    definitions
}

fn type_error(path: &str, expected: &'static str, value: &RuntimeValue) -> RuntimeSchemaError {
    RuntimeSchemaError::Type {
        path: path.to_owned(),
        expected,
        actual: runtime_value_type(value),
    }
}

const fn runtime_value_type(value: &RuntimeValue) -> &'static str {
    match value {
        RuntimeValue::Unit => "unit",
        RuntimeValue::Bool(_) => "bool",
        RuntimeValue::Int(_) => "signed integer",
        RuntimeValue::UInt(_) => "unsigned integer",
        RuntimeValue::F32(_) => "f32",
        RuntimeValue::F64(_) => "f64",
        RuntimeValue::MatrixF32(_) => "f32 matrix",
        RuntimeValue::MatrixF64(_) => "f64 matrix",
        RuntimeValue::TensorF32(_) => "f32 tensor",
        RuntimeValue::TensorF64(_) => "f64 tensor",
        RuntimeValue::String(_) => "string",
        RuntimeValue::Char(_) => "char",
        RuntimeValue::Duration(_) => "duration",
        RuntimeValue::Progress(_) => "progress",
        RuntimeValue::Range(_) => "range",
        RuntimeValue::Iterator(_) => "iterator",
        RuntimeValue::EntityRef(_) => "entity reference",
        RuntimeValue::Tuple(_) => "tuple",
        RuntimeValue::Seq(_) => "sequence",
        RuntimeValue::Record(_) => "record",
        RuntimeValue::NominalRecord(_) => "nominal record",
        RuntimeValue::Opaque(_) => "opaque value",
        RuntimeValue::Reduction(_) => "Reduction value",
        RuntimeValue::Agent(_) => "Agent value",
        RuntimeValue::Function(_) => "function",
        RuntimeValue::Variant { .. } => "variant",
    }
}
