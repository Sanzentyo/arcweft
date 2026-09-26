//! Persistent runtime schemas, canonical value bytes, and validation.

use super::identity::{RuntimeNominalTypeId, RuntimeValueDigest, TypeLayoutHash};
use crate::canonical_varint::encode_u32;
use crate::pattern::{RuntimeOpaqueTypeOwner, RuntimeVariantIdentity};
use crate::value::{RuntimeEntityReference, RuntimeInt, RuntimePayload, RuntimeUInt, RuntimeValue};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

mod builtin;
mod codec_use;
mod data;
mod encoding;
mod nominal;
mod record_shape;
pub(crate) mod traversal;
pub(crate) mod value_budget;
pub(crate) mod value_encoding;
mod value_validation;

pub use builtin::{RuntimeBuiltinSchema, RuntimeBuiltinSchemaError};
pub use codec_use::{
    RuntimeCodecUse, RuntimeCodecUseError, RuntimeFieldCodecUse, RuntimeNominalCodecUses,
    RuntimeVariantCodecUse,
};
pub use nominal::{
    RuntimeNominalSchemaBody, RuntimeNominalSchemaCase, RuntimeNominalSchemaDefinition,
    RuntimeNominalSchemaField, RuntimeNominalSchemaGraph, RuntimeNominalSchemaGraphError,
    RuntimeNominalSchemaIdentity, RuntimeSchemaValueField,
};
pub use record_shape::{RuntimeNominalRecordShape, RuntimeNominalRecordShapeError};

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
    Color,
    Char,
    Bytes {
        format: RuntimeBytesFormat,
    },
    Builtin(RuntimeBuiltinSchema),
    Seq(Box<Self>),
    Array {
        item: Box<Self>,
        length: u64,
    },
    Map {
        kind: RuntimeMapKind,
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
    Tuple(Box<[Self]>),
    RecordValue {
        fields: Box<[RuntimeSchemaValueField]>,
    },
    ExactOpaque {
        owner: RuntimeOpaqueTypeOwner,
        arguments: Box<[Self]>,
    },
    NominalRef(RuntimeNominalSchemaIdentity),
    Never,
    Duration,
    Progress,
    EntityReference,
    AgentValue,
    Choice(Box<[Self]>),
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

/// Deterministic map ordering contract retained in the schema layout.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[repr(u8)]
pub enum RuntimeMapKind {
    Ordered = 0,
    Sorted = 1,
    BTree = 2,
}

impl RuntimeMapKind {
    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_semantic_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Ordered),
            1 => Some(Self::Sorted),
            2 => Some(Self::BTree),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[repr(u8)]
pub enum RuntimeBytesFormat {
    Binary = 0,
    Base64 = 1,
    Hex = 2,
    Array = 3,
}

impl RuntimeBytesFormat {
    pub const ALL: &'static [Self] = &[Self::Binary, Self::Base64, Self::Hex, Self::Array];

    const DECODE: [Option<Self>; 256] = {
        let mut table = [None; 256];
        let mut index = 0;
        while index < Self::ALL.len() {
            let value = Self::ALL[index];
            let encoded = value as u8 as usize;
            assert!(
                table[encoded].is_none(),
                "duplicate runtime bytes format tag"
            );
            table[encoded] = Some(value);
            index += 1;
        }
        table
    };

    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_semantic_tag(tag: u8) -> Option<Self> {
        Self::DECODE[tag as usize]
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeEnumTagStyle {
    External,
    Internal { tag: String },
    Adjacent { tag: String, content: String },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[repr(u8)]
pub enum RuntimeEnumRepr {
    I8 = 0,
    I16 = 1,
    I32 = 2,
    I64 = 3,
    I128 = 4,
    ISize = 5,
    U8 = 6,
    U16 = 7,
    U32 = 8,
    U64 = 9,
    U128 = 10,
    USize = 11,
}

impl RuntimeEnumRepr {
    pub const ALL: &'static [Self] = &[
        Self::I8,
        Self::I16,
        Self::I32,
        Self::I64,
        Self::I128,
        Self::ISize,
        Self::U8,
        Self::U16,
        Self::U32,
        Self::U64,
        Self::U128,
        Self::USize,
    ];

    const DECODE: [Option<Self>; 256] = {
        let mut table = [None; 256];
        let mut index = 0;
        while index < Self::ALL.len() {
            let value = Self::ALL[index];
            let encoded = value as u8 as usize;
            assert!(table[encoded].is_none(), "duplicate runtime enum repr tag");
            table[encoded] = Some(value);
            index += 1;
        }
        table
    };

    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_semantic_tag(tag: u8) -> Option<Self> {
        Self::DECODE[tag as usize]
    }
}

/// Persistent-value validation limits used at startup, ingress, reducer,
/// save/restore, and replay boundaries.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeSchemaLimits {
    pub max_depth: u32,
    pub max_nodes: u32,
    pub max_sequence_items: u32,
    pub max_string_bytes: u64,
    pub max_encoded_bytes: u64,
    /// Shared expected-type and Choice-candidate work for one value admission.
    pub max_validation_work: u64,
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
            max_validation_work: 262_144,
        }
    }

    #[must_use]
    pub fn permits_depth(self, actual: usize) -> bool {
        u32::try_from(actual).is_ok_and(|actual| actual <= self.max_depth)
    }

    #[must_use]
    pub fn permits_nodes(self, actual: usize) -> bool {
        u32::try_from(actual).is_ok_and(|actual| actual <= self.max_nodes)
    }

    #[must_use]
    pub fn permits_sequence_items(self, actual: usize) -> bool {
        u32::try_from(actual).is_ok_and(|actual| actual <= self.max_sequence_items)
    }

    #[must_use]
    pub fn permits_string_bytes(self, actual: usize) -> bool {
        u64::try_from(actual).is_ok_and(|actual| actual <= self.max_string_bytes)
    }

    /// Projects the u64 product limit onto the host address space. When the
    /// selected limit exceeds `usize`, every representable allocation is still
    /// within that limit, so `usize::MAX` is the exact host-side bound.
    pub(crate) fn platform_encoded_bytes(self) -> usize {
        match usize::try_from(self.max_encoded_bytes) {
            Ok(limit) => limit,
            Err(_) => usize::MAX,
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeSchemaError {
    #[error("runtime builtin variant at `{path}` has owner {actual:?}, expected {expected:?}")]
    BuiltinVariantOwner {
        path: String,
        expected: crate::pattern::RuntimeBuiltinVariantIdentity,
        actual: RuntimeVariantIdentity,
    },
    #[error("runtime schema validation at `{path}` exceeds {limit} work units")]
    ValidationWork {
        path: String,
        limit: u64,
        consumed: u64,
    },
    #[error("runtime schema validation at `{path}` exceeds depth {limit}")]
    ValidationDepth { path: String, limit: u32 },
    #[error("runtime value at `{path}` matches no Choice alternative")]
    ChoiceNoMatch {
        path: String,
        branches: Box<[RuntimeSchemaChoiceMismatch]>,
    },
    #[error("runtime value at `{path}` matches Choice alternatives {first} and {second}")]
    ChoiceAmbiguous {
        path: String,
        first: u32,
        second: u32,
    },
    #[error("runtime value at `{path}` has {actual} items, expected {expected}")]
    Arity {
        path: String,
        expected: usize,
        actual: usize,
    },
    #[error("runtime array at `{path}` has {actual} items, expected {expected}")]
    ArrayLength {
        path: String,
        expected: u64,
        actual: usize,
    },
    #[error("runtime record field {ordinal} at `{path}` has the wrong identity or name")]
    RecordField { path: String, ordinal: usize },
    #[error("runtime opaque value at `{path}` has the wrong exact owner")]
    OpaqueOwner { path: String },
    #[error("runtime value at `{path}` has type `{actual}`, expected `{expected}`")]
    Type {
        path: String,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("runtime value at `{path}` contains non-finite {kind}")]
    NonFinite { path: String, kind: &'static str },
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
    #[error(
        "runtime nominal value at `{path}` has semantic identity {actual:?}, expected {expected:?}"
    )]
    NominalSemanticIdentity {
        path: String,
        expected: crate::pattern::RuntimeSemanticTypeId,
        actual: crate::pattern::RuntimeSemanticTypeId,
    },
    #[error("runtime nominal graph validation failed: {source}")]
    NominalGraph {
        source: Box<RuntimeNominalSchemaGraphError>,
    },
    #[error("runtime schema canonical encoding exceeds u32 collection limits")]
    SchemaEncodingOverflow,
    #[error("schema reference {identity:?} requires its validated nominal graph")]
    NominalGraphRequired {
        identity: RuntimeNominalSchemaIdentity,
    },
    #[error("schema reference {identity:?} is absent from the supplied nominal graph")]
    UnresolvedNominal {
        identity: RuntimeNominalSchemaIdentity,
    },
}

/// One ordinary Choice mismatch, retained in source alternative order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSchemaChoiceMismatch {
    alternative: u32,
    source: Option<Box<RuntimeSchemaError>>,
}

impl RuntimeSchemaChoiceMismatch {
    fn new(alternative: u32, source: RuntimeSchemaError) -> Self {
        Self {
            alternative,
            source: Some(Box::new(source)),
        }
    }

    #[must_use]
    pub const fn alternative(&self) -> u32 {
        self.alternative
    }

    #[must_use]
    /// Returns the cause retained for this candidate.
    ///
    /// # Panics
    ///
    /// Panics if the internal lifetime invariant is violated. The cause is
    /// present for every live instance and is extracted only during destruction.
    pub fn source(&self) -> &RuntimeSchemaError {
        self.source
            .as_deref()
            .expect("a live mismatch owns its cause")
    }
}

impl Drop for RuntimeSchemaChoiceMismatch {
    fn drop(&mut self) {
        let mut pending = Vec::new();
        if let Some(source) = self.source.take() {
            pending.push(source);
        }
        while let Some(mut source) = pending.pop() {
            if let RuntimeSchemaError::ChoiceNoMatch { branches, .. } = source.as_mut() {
                for mut branch in std::mem::take(branches) {
                    if let Some(source) = branch.source.take() {
                        pending.push(source);
                    }
                }
            }
        }
    }
}

impl RuntimeTypeSchema {
    pub fn try_layout_hash(&self) -> Result<TypeLayoutHash, RuntimeSchemaError> {
        canonical_schema_layout_hash(self, usize::MAX).map_err(|error| match error {
            RuntimeSchemaError::Encoding { .. } => RuntimeSchemaError::SchemaEncodingOverflow,
            error => error,
        })
    }

    pub fn validate_payload(
        &self,
        payload: &RuntimePayload,
        limits: RuntimeSchemaLimits,
    ) -> Result<RuntimeValueDigest, RuntimeSchemaError> {
        self.validate_value(&payload.0, limits)
    }

    pub fn validate_value(
        &self,
        value: &RuntimeValue,
        limits: RuntimeSchemaLimits,
    ) -> Result<RuntimeValueDigest, RuntimeSchemaError> {
        validate_schema_value(
            self,
            value,
            limits,
            value_validation::Expected::Schema(self),
        )
    }
}

#[cfg(test)]
fn canonical_schema_bytes(
    schema: &RuntimeTypeSchema,
    max_encoded_bytes: usize,
) -> Result<Vec<u8>, RuntimeSchemaError> {
    let mut sink = CanonicalBytesSink::default();
    visit_schema_document(schema, max_encoded_bytes, &mut sink)?;
    Ok(sink.finish())
}

fn validate_schema_value<'a>(
    schema: &'a RuntimeTypeSchema,
    value: &RuntimeValue,
    limits: RuntimeSchemaLimits,
    expected: value_validation::Expected<'a>,
) -> Result<RuntimeValueDigest, RuntimeSchemaError> {
    value_validation::SchemaValueValidation::tree(schema, limits)?.validate(value, expected)
}

fn canonical_schema_layout_hash(
    schema: &RuntimeTypeSchema,
    max_encoded_bytes: usize,
) -> Result<TypeLayoutHash, RuntimeSchemaError> {
    let mut sink = CanonicalBlake3Sink::default();
    visit_schema_document(schema, max_encoded_bytes, &mut sink)?;
    Ok(TypeLayoutHash::from_bytes(sink.finish()))
}

fn visit_schema_document<S: CanonicalSink + ?Sized>(
    schema: &RuntimeTypeSchema,
    max_encoded_bytes: usize,
    sink: &mut S,
) -> Result<(), RuntimeSchemaError> {
    let mut writer = CanonicalWriter {
        sink,
        max_string_bytes: None,
        max_encoded_bytes: u64::try_from(max_encoded_bytes).map_err(|_| {
            RuntimeSchemaError::BudgetExceeded {
                budget: "encoded_bytes",
            }
        })?,
    };
    writer.extend(b"arcweft.nominal-schema\0")?;
    writer.var_u32(1)?;
    encoding::schema(schema, &mut writer, None)
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
    let mut sink = CanonicalBytesSink::default();
    visit_runtime_value(value, max_encoded_bytes, &mut sink)?;
    Ok(sink.finish())
}

/// Produces a bounded canonical transcript for one snapshot-admissible
/// runtime value. Snapshot mode permits snapshot-only opaque owners while
/// retaining the same deterministic value encoding and shared limits used by
/// constant admission.
pub(crate) fn canonical_runtime_snapshot_value_bytes(
    value: &RuntimeValue,
    limits: RuntimeSchemaLimits,
) -> Result<Vec<u8>, RuntimeSchemaError> {
    let mut sink = CanonicalBytesSink::default();
    let mut writer = CanonicalWriter {
        sink: &mut sink,
        max_string_bytes: Some(limits.max_string_bytes),
        max_encoded_bytes: limits.max_encoded_bytes,
    };
    let mut validation = value_encoding::NoSchemaValidation;
    value_encoding::visit_snapshot_bounded(value, limits, &mut writer, &mut validation, ())?;
    Ok(sink.finish())
}

pub(crate) fn canonical_runtime_value_digest(
    value: &RuntimeValue,
    max_encoded_bytes: usize,
) -> Result<RuntimeValueDigest, RuntimeSchemaError> {
    let mut sink = CanonicalBlake3Sink::default();
    visit_runtime_value(value, max_encoded_bytes, &mut sink)?;
    Ok(RuntimeValueDigest::from_bytes(sink.finish()))
}

/// Private byte boundary shared by canonical schema and value transcripts.
/// Each algebra has one exhaustive visitor; bytes and direct BLAKE3 consumers
/// use the same primitive encoding and bounded write accounting.
trait CanonicalSink {
    fn write(&mut self, bytes: &[u8]) -> Result<(), RuntimeSchemaError>;
    fn bytes_written(&self) -> u64;
}

#[derive(Default)]
struct CanonicalBytesSink {
    bytes: Vec<u8>,
    bytes_written: u64,
}

impl CanonicalBytesSink {
    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

impl CanonicalSink for CanonicalBytesSink {
    fn write(&mut self, bytes: &[u8]) -> Result<(), RuntimeSchemaError> {
        self.bytes.extend_from_slice(bytes);
        self.bytes_written = self
            .bytes_written
            .checked_add(u64::try_from(bytes.len()).map_err(|_| {
                RuntimeSchemaError::BudgetExceeded {
                    budget: "encoded_bytes",
                }
            })?)
            .ok_or(RuntimeSchemaError::BudgetExceeded {
                budget: "encoded_bytes",
            })?;
        Ok(())
    }

    fn bytes_written(&self) -> u64 {
        self.bytes_written
    }
}

#[derive(Default)]
struct CanonicalBlake3Sink {
    hasher: blake3::Hasher,
    bytes_written: u64,
}

impl CanonicalBlake3Sink {
    fn finish(self) -> [u8; 32] {
        *self.hasher.finalize().as_bytes()
    }
}

impl CanonicalSink for CanonicalBlake3Sink {
    fn write(&mut self, bytes: &[u8]) -> Result<(), RuntimeSchemaError> {
        self.hasher.update(bytes);
        self.bytes_written = self
            .bytes_written
            .checked_add(u64::try_from(bytes.len()).map_err(|_| {
                RuntimeSchemaError::BudgetExceeded {
                    budget: "encoded_bytes",
                }
            })?)
            .ok_or(RuntimeSchemaError::BudgetExceeded {
                budget: "encoded_bytes",
            })?;
        Ok(())
    }

    fn bytes_written(&self) -> u64 {
        self.bytes_written
    }
}

/// Visits one `RuntimeValue` through the canonical, bounded encoder and writes
/// the resulting transcript to a caller-owned private sink.
fn visit_runtime_value<S: CanonicalSink + ?Sized>(
    value: &RuntimeValue,
    max_encoded_bytes: usize,
    sink: &mut S,
) -> Result<(), RuntimeSchemaError> {
    let mut visitor = CanonicalWriter {
        sink,
        max_string_bytes: None,
        max_encoded_bytes: u64::try_from(max_encoded_bytes).map_err(|_| {
            RuntimeSchemaError::BudgetExceeded {
                budget: "encoded_bytes",
            }
        })?,
    };
    value_encoding::visit(
        value.view(),
        0,
        &mut visitor,
        None,
        &mut value_encoding::NoSchemaValidation,
        (),
    )
}

struct CanonicalWriter<'a, S: CanonicalSink + ?Sized> {
    sink: &'a mut S,
    max_encoded_bytes: u64,
    max_string_bytes: Option<u64>,
}

impl<S: CanonicalSink + ?Sized> CanonicalWriter<'_, S> {
    fn extend(&mut self, bytes: &[u8]) -> Result<(), RuntimeSchemaError> {
        let len = u64::try_from(bytes.len()).map_err(|_| RuntimeSchemaError::BudgetExceeded {
            budget: "encoded_bytes",
        })?;
        let next = self.sink.bytes_written().checked_add(len).ok_or(
            RuntimeSchemaError::BudgetExceeded {
                budget: "encoded_bytes",
            },
        )?;
        if next > self.max_encoded_bytes {
            return Err(RuntimeSchemaError::BudgetExceeded {
                budget: "encoded_bytes",
            });
        }
        self.sink.write(bytes)
    }

    fn u8(&mut self, value: u8) -> Result<(), RuntimeSchemaError> {
        self.extend(&[value])
    }

    fn fixed_u32(&mut self, value: u32) -> Result<(), RuntimeSchemaError> {
        self.extend(&value.to_le_bytes())
    }

    fn var_u32(&mut self, value: u32) -> Result<(), RuntimeSchemaError> {
        let (bytes, length) = encode_u32(value);
        self.extend(&bytes[..length])
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
        self.var_u32(value)
    }

    fn string(&mut self, value: &str) -> Result<(), RuntimeSchemaError> {
        if self
            .max_string_bytes
            .is_some_and(|limit| u64::try_from(value.len()).map_or(true, |length| length > limit))
        {
            return Err(RuntimeSchemaError::BudgetExceeded {
                budget: "string_bytes",
            });
        }
        self.len(value.len())?;
        self.extend(value.as_bytes())
    }

    fn entity_reference(
        &mut self,
        value: &RuntimeEntityReference,
    ) -> Result<(), RuntimeSchemaError> {
        match value {
            RuntimeEntityReference::Project { family, public_id } => {
                self.u8(0)?;
                self.u8(family.semantic_tag())?;
                self.string(public_id.as_str())
            }
            RuntimeEntityReference::DialogueLine(line) => {
                self.u8(1)?;
                self.string(&line.canonical_label())
            }
            RuntimeEntityReference::CharacterLook { character, look } => {
                self.u8(2)?;
                self.string(character.as_str())?;
                self.string(look.as_str())
            }
        }
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

    fn scalar(
        &mut self,
        value: crate::value::RuntimeScalarView<'_>,
    ) -> Result<(), RuntimeSchemaError> {
        use crate::value::RuntimeScalarView as Scalar;
        match value {
            Scalar::Unit => self.u8(1),
            Scalar::Bool(value) => {
                self.u8(2)?;
                self.u8(u8::from(value))
            }
            Scalar::Int(value) => {
                self.u8(3)?;
                let (width, value) = match value {
                    RuntimeInt::I8(value) => (1, i128::from(value)),
                    RuntimeInt::I16(value) => (2, i128::from(value)),
                    RuntimeInt::I32(value) => (3, i128::from(value)),
                    RuntimeInt::I64(value) => (4, i128::from(value)),
                    RuntimeInt::I128(value) => (5, value),
                    RuntimeInt::ISize(value) => (6, i128::from(value)),
                };
                self.u8(width)?;
                self.i128(value)
            }
            Scalar::UInt(value) => {
                self.u8(4)?;
                let (width, value) = match value {
                    RuntimeUInt::U8(value) => (1, u128::from(value)),
                    RuntimeUInt::U16(value) => (2, u128::from(value)),
                    RuntimeUInt::U32(value) => (3, u128::from(value)),
                    RuntimeUInt::U64(value) => (4, u128::from(value)),
                    RuntimeUInt::U128(value) => (5, value),
                    RuntimeUInt::USize(value) => (6, u128::from(value)),
                };
                self.u8(width)?;
                self.u128(value)
            }
            Scalar::F32(value) => {
                if !value.is_finite() {
                    return Err(RuntimeSchemaError::NonFinite {
                        path: "$".to_owned(),
                        kind: "f32",
                    });
                }
                self.u8(5)?;
                self.fixed_u32(if value == 0.0 { 0 } else { value.to_bits() })
            }
            Scalar::F64(value) => {
                if !value.is_finite() {
                    return Err(RuntimeSchemaError::NonFinite {
                        path: "$".to_owned(),
                        kind: "f64",
                    });
                }
                self.u8(6)?;
                self.u64(if value == 0.0 { 0 } else { value.to_bits() })
            }
            Scalar::String(value) => {
                self.u8(7)?;
                self.string(value)
            }
            Scalar::Color(value) => {
                self.u8(20)?;
                for channel in value.rgba8() {
                    self.u8(channel)?;
                }
                Ok(())
            }
            Scalar::Char(value) => {
                self.u8(8)?;
                self.fixed_u32(u32::from(value))
            }
            Scalar::Duration(value) => {
                self.u8(9)?;
                self.u64(value.as_nanos())
            }
            Scalar::Progress(value) => {
                self.u8(19)?;
                self.fixed_u32(if value.ratio() == 0.0 {
                    0
                } else {
                    value.ratio().to_bits()
                })?;
                self.option(value.label(), Self::string)
            }
            Scalar::EntityRef(value) => {
                self.u8(10)?;
                self.entity_reference(value)
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

    fn variant_identity(
        &mut self,
        identity: &RuntimeVariantIdentity,
    ) -> Result<(), RuntimeSchemaError> {
        match identity {
            RuntimeVariantIdentity::Nominal {
                nominal,
                semantic_identity,
                layout,
            } => {
                self.u8(0)?;
                self.string(nominal.as_str())?;
                self.extend(semantic_identity.as_bytes())?;
                self.extend(layout.as_bytes())
            }
            RuntimeVariantIdentity::Builtin(owner) => {
                self.u8(1)?;
                self.u8(owner.semantic_tag())
            }
        }
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
            Self::Color => "color",
            Self::Char => "char",
            Self::Bytes { .. } => "bytes",
            Self::Builtin(_) => "builtin variant",
            Self::Seq(_) => "sequence",
            Self::Array { .. } => "array",
            Self::Map { .. } => "map",
            Self::Record { .. } => "record",
            Self::Enum { .. } => "enum",
            Self::Named(_) => "named value",
            Self::Tuple(_) => "tuple",
            Self::RecordValue { .. } => "record",
            Self::ExactOpaque { .. } => "opaque value",
            Self::NominalRef(_) => "nominal value",
            Self::Never => "never",
            Self::Duration => "duration",
            Self::Progress => "progress",
            Self::EntityReference => "entity reference",
            Self::AgentValue => "AgentValue",
            Self::Choice(_) => "Choice",
        }
    }
}

fn schema_definitions(
    schema: &RuntimeTypeSchema,
) -> Result<BTreeMap<&str, &RuntimeTypeSchema>, RuntimeSchemaError> {
    let mut definitions = BTreeMap::new();
    schema.walk(&mut traversal::SchemaPath::root("$"), |schema, _, _| {
        match schema {
            RuntimeTypeSchema::Record { name, .. } | RuntimeTypeSchema::Enum { name, .. } => {
                definitions.insert(name.as_str(), schema);
            }
            RuntimeTypeSchema::NominalRef(identity) => {
                return Err(RuntimeSchemaError::NominalGraphRequired {
                    identity: identity.clone(),
                });
            }
            _ => {}
        }
        Ok(())
    })?;
    Ok(definitions)
}

#[cfg(test)]
mod visitor_tests {
    use super::{
        CanonicalSink, canonical_runtime_value_bytes, canonical_runtime_value_digest,
        visit_runtime_value,
    };
    use crate::value::RuntimeValue;

    struct DigestSink {
        hasher: blake3::Hasher,
        bytes_written: u64,
    }

    impl CanonicalSink for DigestSink {
        fn write(&mut self, bytes: &[u8]) -> Result<(), super::RuntimeSchemaError> {
            self.hasher.update(bytes);
            self.bytes_written += u64::try_from(bytes.len()).expect("test bytes fit u64");
            Ok(())
        }

        fn bytes_written(&self) -> u64 {
            self.bytes_written
        }
    }

    #[test]
    fn visitor_uses_the_same_canonical_bytes_for_a_custom_sink() {
        let value = RuntimeValue::Tuple(vec![
            RuntimeValue::Bool(true),
            RuntimeValue::String("ok".to_owned()),
        ]);
        let bytes = canonical_runtime_value_bytes(&value, 1024).expect("canonical bytes");
        let mut sink = DigestSink {
            hasher: blake3::Hasher::new(),
            bytes_written: 0,
        };
        visit_runtime_value(&value, 1024, &mut sink).expect("sink visit");
        assert_eq!(
            *sink.hasher.finalize().as_bytes(),
            *blake3::hash(&bytes).as_bytes()
        );
        assert_eq!(
            canonical_runtime_value_digest(&value, 1024)
                .expect("direct digest")
                .as_bytes(),
            blake3::hash(&bytes).as_bytes()
        );
    }

    #[test]
    fn canonical_lengths_use_shortest_varints_and_exact_byte_budgets() {
        let value = RuntimeValue::String("x".repeat(300));
        let mut expected = vec![7, 0xac, 0x02];
        expected.extend_from_slice(&[b'x'; 300]);
        assert_eq!(
            canonical_runtime_value_bytes(&value, expected.len()).unwrap(),
            expected
        );
        assert_eq!(
            canonical_runtime_value_digest(&value, expected.len())
                .unwrap()
                .as_bytes(),
            blake3::hash(&expected).as_bytes(),
        );
        let error = super::RuntimeSchemaError::BudgetExceeded {
            budget: "encoded_bytes",
        };
        assert_eq!(
            canonical_runtime_value_bytes(&value, expected.len() - 1),
            Err(error.clone())
        );
        assert_eq!(
            canonical_runtime_value_digest(&value, expected.len() - 1),
            Err(error)
        );

        let sequence = RuntimeValue::Tuple(vec![RuntimeValue::Unit; 300]);
        let mut expected_sequence = vec![11, 0xac, 0x02];
        expected_sequence.extend_from_slice(&[1; 300]);
        assert_eq!(
            canonical_runtime_value_bytes(&sequence, expected_sequence.len()).unwrap(),
            expected_sequence
        );
    }

    #[test]
    fn schema_transcript_version_and_name_lengths_use_shortest_varints() {
        let schema = super::RuntimeTypeSchema::Named("x".repeat(300));
        let mut expected = b"arcweft.nominal-schema\0".to_vec();
        expected.extend_from_slice(&[1, 25, 0xac, 0x02]);
        expected.extend_from_slice(&[b'x'; 300]);
        assert_eq!(
            super::canonical_schema_bytes(&schema, expected.len()).unwrap(),
            expected
        );
        assert_eq!(
            schema.try_layout_hash().unwrap().as_bytes(),
            blake3::hash(&expected).as_bytes()
        );
    }

    #[test]
    fn canonical_numeric_payloads_retain_their_exact_fixed_bits() {
        for (value, expected) in [
            (RuntimeValue::F32(1.0), vec![5, 0, 0, 0x80, 0x3f]),
            (RuntimeValue::F32(-0.0), vec![5, 0, 0, 0, 0]),
            (RuntimeValue::Char('\u{100}'), vec![8, 0, 1, 0, 0]),
        ] {
            assert_eq!(
                canonical_runtime_value_bytes(&value, expected.len()).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn canonical_variant_ordinal_uses_the_shared_shortest_varint() {
        let value = RuntimeValue::Variant {
            owner: crate::pattern::RuntimeVariantIdentity::Nominal {
                nominal: super::RuntimeNominalTypeId::try_new("aw.test.variant").unwrap(),
                semantic_identity: crate::pattern::RuntimeSemanticTypeId::from_bytes([0x11; 32]),
                layout: super::TypeLayoutHash::from_bytes([0x22; 32]),
            },
            ordinal: 300,
            name: "case".to_owned(),
            payload: None,
        };
        let mut expected = vec![14, 0, 15];
        expected.extend_from_slice(b"aw.test.variant");
        expected.extend_from_slice(&[0x11; 32]);
        expected.extend_from_slice(&[0x22; 32]);
        expected.extend_from_slice(&[0xac, 0x02, 4]);
        expected.extend_from_slice(b"case");
        expected.push(0);
        assert_eq!(
            canonical_runtime_value_bytes(&value, expected.len()).unwrap(),
            expected
        );
        assert_eq!(
            canonical_runtime_value_digest(&value, expected.len())
                .unwrap()
                .as_bytes(),
            blake3::hash(&expected).as_bytes()
        );
    }

    #[test]
    fn canonical_records_retain_field_ids_and_declaration_order() {
        let declared = RuntimeValue::try_record(vec![
            ("z".to_owned(), RuntimeValue::Bool(true)),
            ("a".to_owned(), RuntimeValue::Bool(false)),
        ])
        .unwrap();
        let reordered = RuntimeValue::try_record(vec![
            ("a".to_owned(), RuntimeValue::Bool(false)),
            ("z".to_owned(), RuntimeValue::Bool(true)),
        ])
        .unwrap();
        let expected = [13, 2, 1, 1, b'z', 2, 1, 2, 1, b'a', 2, 0];
        assert_eq!(
            declared.try_canonical_bytes(expected.len()).unwrap(),
            expected
        );
        assert_eq!(
            canonical_runtime_value_digest(&declared, expected.len())
                .unwrap()
                .as_bytes(),
            blake3::hash(&expected).as_bytes()
        );
        assert_ne!(
            declared.try_canonical_bytes(32).unwrap(),
            reordered.try_canonical_bytes(32).unwrap()
        );
        assert!(declared.try_canonical_bytes(expected.len() - 1).is_err());
        assert!(canonical_runtime_value_digest(&declared, expected.len() - 1).is_err());
    }

    #[test]
    fn columnar_records_share_the_logical_row_transcript() {
        let row = RuntimeValue::try_record(vec![
            ("z".to_owned(), RuntimeValue::Bool(true)),
            ("a".to_owned(), RuntimeValue::Bool(false)),
        ])
        .unwrap();
        let rows = RuntimeValue::Seq(crate::value::RuntimeSeq::values(vec![row]));
        let columns = RuntimeValue::Seq(
            crate::value::RuntimeSeq::record_columns(
                1,
                vec![
                    (
                        "z".to_owned(),
                        crate::value::RuntimeSeq::values(vec![RuntimeValue::Bool(true)]),
                    ),
                    (
                        "a".to_owned(),
                        crate::value::RuntimeSeq::values(vec![RuntimeValue::Bool(false)]),
                    ),
                ],
            )
            .unwrap(),
        );
        let expected = rows.try_canonical_bytes(64).unwrap();
        assert_eq!(
            columns.try_canonical_bytes(expected.len()).unwrap(),
            expected
        );
        assert_eq!(
            canonical_runtime_value_digest(&columns, expected.len())
                .unwrap()
                .as_bytes(),
            blake3::hash(&expected).as_bytes()
        );
    }

    #[test]
    fn canonical_columnar_encoding_checks_budget_before_expanding_rows() {
        let columns = crate::value::TupleSeq::new(u32::MAX as usize, Vec::new()).unwrap();
        let value = RuntimeValue::Seq(crate::value::RuntimeSeq::TupleColumns(columns));
        let expected = super::RuntimeSchemaError::BudgetExceeded {
            budget: "encoded_bytes",
        };
        assert_eq!(value.try_canonical_bytes(5), Err(expected.clone()));
        assert_eq!(canonical_runtime_value_digest(&value, 5), Err(expected));
        for schema in [
            super::RuntimeTypeSchema::Seq(Box::new(super::RuntimeTypeSchema::Unit)),
            super::RuntimeTypeSchema::Bytes {
                format: super::RuntimeBytesFormat::Binary,
            },
            super::RuntimeTypeSchema::Map {
                kind: super::RuntimeMapKind::Ordered,
                key: Box::new(super::RuntimeTypeSchema::Unit),
                value: Box::new(super::RuntimeTypeSchema::Unit),
            },
        ] {
            assert_eq!(
                schema.validate_value(&value, super::RuntimeSchemaLimits::engine_default()),
                Err(super::RuntimeSchemaError::BudgetExceeded {
                    budget: "sequence_items"
                })
            );
        }
    }
}
