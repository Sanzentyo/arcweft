use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use thiserror::Error;

use super::{
    Angle, FX_MAX_CAPTURED_OR_PARAMETER_SLOTS, FiniteF32, FiniteF32Error, FxColor,
    FxDefinitionParameterLayout, FxRuntimeParameterRef, FxRuntimeParameterSlot, FxRuntimeType,
    FxRuntimeValue, FxSamplerProgram, FxSamplerProgramCanonicalError, FxSamplerProgramDecodeError,
    FxVec2, Length, Opacity, Seconds, Transform2DError,
    canonical::{
        CanonicalEncodeError, CanonicalEncoder, CanonicalHashSink, CanonicalLengthSink,
        CanonicalReader, CanonicalSink, CanonicalVecSink, FxCanonicalDecodeError,
    },
};

pub const FX_MAX_UNIFORM_FIELDS: usize = 62;
pub const FX_MAX_UNIFORM_NAME_BYTES: usize = 64;
pub const FX_MAX_UNIFORM_RECORD_CANONICAL_BYTES: usize = 65_536;
const PREFIX: &[u8] = b"arcweft.fx-uniform-record";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum FxUniformType {
    Bool = 0,
    I32 = 1,
    F32 = 2,
    Length = 3,
    Angle = 4,
    Seconds = 5,
    Color = 6,
    Vec2 = 7,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FxUniformName {
    value: String,
    byte_len: u8,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FxUniformConstant {
    value: FxUniformConstantValue,
    uniform_type: FxUniformType,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FxUniformConstantValue {
    Bool(bool),
    I32(i32),
    F32(FiniteF32),
    Length(Length),
    Angle(Angle),
    Seconds(Seconds),
    Color(FxColor),
    Vec2(FxVec2),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FxUniformProgram {
    sampler: FxSamplerProgram,
    uniform_type: FxUniformType,
    canonical_len: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FxUniformParameterRef {
    Bool(FxRuntimeParameterRef),
    I32(FxRuntimeParameterRef),
    F32(FxRuntimeParameterRef),
    Length(FxRuntimeParameterRef),
    Angle(FxRuntimeParameterRef),
    Seconds(FxRuntimeParameterRef),
    Color(FxRuntimeParameterRef),
    Vec2(FxRuntimeParameterRef),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum FxUniformValue {
    Constant(FxUniformConstant),
    Parameter(FxUniformParameterRef),
    Program(FxUniformProgram),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum FxUniformValueWire {
    Constant(FxUniformConstant),
    Parameter(FxUniformParameterRef),
    Program(FxUniformProgram),
}

impl<'de> Deserialize<'de> for FxUniformValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match FxUniformValueWire::deserialize(deserializer)? {
            FxUniformValueWire::Constant(value) => Ok(Self::Constant(value)),
            FxUniformValueWire::Parameter(parameter) => Ok(Self::Parameter(parameter)),
            FxUniformValueWire::Program(value) => Ok(Self::Program(value)),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FxUniformField {
    name: FxUniformName,
    value: FxUniformValue,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FxUniformRecord {
    fields: Box<[FxUniformField]>,
    field_count: u8,
    canonical_len: u32,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum FxUniformError {
    #[error("invalid uniform name")]
    InvalidName,
    #[error("invalid uniform type {0:?}")]
    InvalidType(FxRuntimeType),
    #[error("uniform slot limit {0}")]
    SlotLimit(u16),
    #[error("uniform program state")]
    ProgramState,
    #[error("uniform field limit {0}")]
    FieldLimit(usize),
    #[error("duplicate uniform {0}")]
    Duplicate(String),
    #[error("uniform byte limit")]
    ByteLimit,
    #[error("uniform canonical length overflow")]
    LengthOverflow,
    #[error("uniform canonical allocation failed")]
    AllocationFailed,
    #[error("uniform canonical encoding wrote {actual} bytes, expected {expected} bytes")]
    LengthMismatch { actual: usize, expected: usize },
    #[error(transparent)]
    ProgramCanonical(#[from] FxSamplerProgramCanonicalError),
    #[error("uniform definition schema")]
    DefinitionSchema,
}

/// Rejection while reconstructing one canonical v1 uniform record.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxUniformRecordDecodeError {
    #[error(transparent)]
    Canonical(#[from] FxCanonicalDecodeError),
    #[error("canonical uniform {kind} count {actual} exceeds the owner limit of {limit}")]
    OwnerLimit {
        kind: &'static str,
        actual: u64,
        limit: usize,
    },
    #[error("canonical uniform failed to allocate {count} {kind} rows")]
    AllocationFailed { kind: &'static str, count: usize },
    #[error("unknown uniform type tag {0}")]
    UnknownUniformType(u8),
    #[error("unknown uniform value tag {0}")]
    UnknownValueTag(u8),
    #[error("canonical uniform {kind} value {value} is out of range")]
    IntegerOutOfRange { kind: &'static str, value: u64 },
    #[error("canonical uniform contains non-canonical float bits {bits:#010x}")]
    NonCanonicalFloat { bits: u32 },
    #[error("canonical uniform fields are not strictly ordered: `{previous}` then `{current}`")]
    NonCanonicalFieldOrder { previous: String, current: String },
    #[error("canonical uniform value declares {declared:?}, but reconstructs as {actual:?}")]
    ValueTypeMismatch {
        declared: FxUniformType,
        actual: FxUniformType,
    },
    #[error(transparent)]
    InvalidFloat(#[from] FiniteF32Error),
    #[error(transparent)]
    InvalidClosedValue(#[from] Transform2DError),
    #[error(transparent)]
    Program(#[from] FxSamplerProgramDecodeError),
    #[error(transparent)]
    Validation(#[from] FxUniformError),
}

impl FxUniformType {
    pub const fn runtime_type(self) -> FxRuntimeType {
        match self {
            Self::Bool => FxRuntimeType::Bool,
            Self::I32 => FxRuntimeType::I32,
            Self::F32 => FxRuntimeType::F32,
            Self::Length => FxRuntimeType::Length,
            Self::Angle => FxRuntimeType::Angle,
            Self::Seconds => FxRuntimeType::Seconds,
            Self::Color => FxRuntimeType::Color,
            Self::Vec2 => FxRuntimeType::Vec2,
        }
    }
    pub const fn from_runtime_type(v: FxRuntimeType) -> Option<Self> {
        match v {
            FxRuntimeType::Bool => Some(Self::Bool),
            FxRuntimeType::I32 => Some(Self::I32),
            FxRuntimeType::F32 => Some(Self::F32),
            FxRuntimeType::Length => Some(Self::Length),
            FxRuntimeType::Angle => Some(Self::Angle),
            FxRuntimeType::Seconds => Some(Self::Seconds),
            FxRuntimeType::Color => Some(Self::Color),
            FxRuntimeType::Vec2 => Some(Self::Vec2),
            FxRuntimeType::Transform2D | FxRuntimeType::U32 => None,
        }
    }
}

impl FxUniformParameterRef {
    pub fn try_new(parameter: FxRuntimeParameterRef) -> Result<Self, FxUniformError> {
        if usize::from(parameter.slot().get()) >= FX_MAX_CAPTURED_OR_PARAMETER_SLOTS {
            return Err(FxUniformError::SlotLimit(parameter.slot().get()));
        }
        Ok(match parameter.runtime_type() {
            FxRuntimeType::Bool => Self::Bool(parameter),
            FxRuntimeType::I32 => Self::I32(parameter),
            FxRuntimeType::F32 => Self::F32(parameter),
            FxRuntimeType::Length => Self::Length(parameter),
            FxRuntimeType::Angle => Self::Angle(parameter),
            FxRuntimeType::Seconds => Self::Seconds(parameter),
            FxRuntimeType::Color => Self::Color(parameter),
            FxRuntimeType::Vec2 => Self::Vec2(parameter),
            FxRuntimeType::Transform2D | FxRuntimeType::U32 => {
                return Err(FxUniformError::InvalidType(parameter.runtime_type()));
            }
        })
    }

    pub const fn reference(self) -> FxRuntimeParameterRef {
        match self {
            Self::Bool(value)
            | Self::I32(value)
            | Self::F32(value)
            | Self::Length(value)
            | Self::Angle(value)
            | Self::Seconds(value)
            | Self::Color(value)
            | Self::Vec2(value) => value,
        }
    }

    pub const fn uniform_type(self) -> FxUniformType {
        match self {
            Self::Bool(_) => FxUniformType::Bool,
            Self::I32(_) => FxUniformType::I32,
            Self::F32(_) => FxUniformType::F32,
            Self::Length(_) => FxUniformType::Length,
            Self::Angle(_) => FxUniformType::Angle,
            Self::Seconds(_) => FxUniformType::Seconds,
            Self::Color(_) => FxUniformType::Color,
            Self::Vec2(_) => FxUniformType::Vec2,
        }
    }
}

impl Serialize for FxUniformParameterRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.reference().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FxUniformParameterRef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_new(FxRuntimeParameterRef::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}
impl FxUniformName {
    pub fn try_new(v: impl Into<String>) -> Result<Self, FxUniformError> {
        let v = v.into();
        let b = v.as_bytes();
        if b.is_empty()
            || b.len() > FX_MAX_UNIFORM_NAME_BYTES
            || !(b[0].is_ascii_alphabetic() || b[0] == b'_')
            || !b[1..]
                .iter()
                .all(|c| c.is_ascii_alphanumeric() || *c == b'_')
        {
            Err(FxUniformError::InvalidName)
        } else {
            let byte_len = u8::try_from(b.len()).map_err(|_| FxUniformError::InvalidName)?;
            Ok(Self { value: v, byte_len })
        }
    }
    pub fn as_str(&self) -> &str {
        &self.value
    }
    const fn byte_len(&self) -> u8 {
        self.byte_len
    }
}
impl Serialize for FxUniformName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.value)
    }
}
impl<'de> Deserialize<'de> for FxUniformName {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::try_new(String::deserialize(d)?).map_err(D::Error::custom)
    }
}

impl FxUniformConstant {
    pub fn try_new(value: FxRuntimeValue) -> Result<Self, FxUniformError> {
        let runtime_type = value.value_type();
        let uniform_type = FxUniformType::from_runtime_type(runtime_type)
            .ok_or(FxUniformError::InvalidType(runtime_type))?;
        let value = match value {
            FxRuntimeValue::Bool(v) => FxUniformConstantValue::Bool(v),
            FxRuntimeValue::I32(v) => FxUniformConstantValue::I32(v),
            FxRuntimeValue::F32(v) => FxUniformConstantValue::F32(v),
            FxRuntimeValue::Length(v) => FxUniformConstantValue::Length(v),
            FxRuntimeValue::Angle(v) => FxUniformConstantValue::Angle(v),
            FxRuntimeValue::Seconds(v) => FxUniformConstantValue::Seconds(v),
            FxRuntimeValue::Color(v) => FxUniformConstantValue::Color(v),
            FxRuntimeValue::Vec2(v) => FxUniformConstantValue::Vec2(v),
            FxRuntimeValue::Transform2D(_) | FxRuntimeValue::U32(_) => {
                return Err(FxUniformError::InvalidType(runtime_type));
            }
        };
        Ok(Self {
            value,
            uniform_type,
        })
    }
    pub const fn value(self) -> FxRuntimeValue {
        match self.value {
            FxUniformConstantValue::Bool(v) => FxRuntimeValue::Bool(v),
            FxUniformConstantValue::I32(v) => FxRuntimeValue::I32(v),
            FxUniformConstantValue::F32(v) => FxRuntimeValue::F32(v),
            FxUniformConstantValue::Length(v) => FxRuntimeValue::Length(v),
            FxUniformConstantValue::Angle(v) => FxRuntimeValue::Angle(v),
            FxUniformConstantValue::Seconds(v) => FxRuntimeValue::Seconds(v),
            FxUniformConstantValue::Color(v) => FxRuntimeValue::Color(v),
            FxUniformConstantValue::Vec2(v) => FxRuntimeValue::Vec2(v),
        }
    }
}
impl Serialize for FxUniformConstant {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.value().serialize(s)
    }
}
impl<'de> Deserialize<'de> for FxUniformConstant {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::try_new(FxRuntimeValue::deserialize(d)?).map_err(D::Error::custom)
    }
}
impl FxUniformProgram {
    pub fn try_new(sampler: FxSamplerProgram) -> Result<Self, FxUniformError> {
        if !sampler.program().schema().state_types().is_empty() {
            return Err(FxUniformError::ProgramState);
        }
        let uniform_type = FxUniformType::from_runtime_type(sampler.return_type())
            .ok_or(FxUniformError::InvalidType(sampler.return_type()))?;
        let canonical_len = sampler.canonical_v1_len()?;
        let canonical_len =
            u64::try_from(canonical_len).map_err(|_| FxUniformError::LengthOverflow)?;
        Ok(Self {
            sampler,
            uniform_type,
            canonical_len,
        })
    }
    pub const fn sampler(&self) -> &FxSamplerProgram {
        &self.sampler
    }
    pub const fn canonical_len(&self) -> u64 {
        self.canonical_len
    }
}
impl Serialize for FxUniformProgram {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.sampler.serialize(s)
    }
}
impl<'de> Deserialize<'de> for FxUniformProgram {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::try_new(FxSamplerProgram::deserialize(d)?).map_err(D::Error::custom)
    }
}

impl FxUniformValue {
    pub fn constant(v: FxRuntimeValue) -> Result<Self, FxUniformError> {
        Ok(Self::Constant(FxUniformConstant::try_new(v)?))
    }
    pub fn parameter(parameter: FxRuntimeParameterRef) -> Result<Self, FxUniformError> {
        Ok(Self::Parameter(FxUniformParameterRef::try_new(parameter)?))
    }
    pub fn program(v: FxSamplerProgram) -> Result<Self, FxUniformError> {
        Ok(Self::Program(FxUniformProgram::try_new(v)?))
    }
    pub const fn uniform_type(&self) -> FxUniformType {
        match self {
            Self::Constant(v) => v.uniform_type,
            Self::Parameter(parameter) => parameter.uniform_type(),
            Self::Program(v) => v.uniform_type,
        }
    }
    fn validate(&self) -> Result<(), FxUniformError> {
        if let Self::Parameter(parameter) = self {
            FxUniformParameterRef::try_new(parameter.reference())?;
        }
        Ok(())
    }
    fn validate_definition_layout(
        &self,
        layout: &FxDefinitionParameterLayout,
    ) -> Result<(), FxUniformError> {
        match self {
            Self::Parameter(parameter) => layout
                .runtime_rows()
                .get(usize::from(parameter.reference().slot().get()))
                .map(|row| row.reference())
                .filter(|expected| *expected == parameter.reference())
                .map(|_| ())
                .ok_or(FxUniformError::DefinitionSchema),
            Self::Program(v)
                if !layout
                    .matches_runtime_schema(v.sampler.program().schema().parameter_types()) =>
            {
                Err(FxUniformError::DefinitionSchema)
            }
            _ => Ok(()),
        }
    }
}
impl FxUniformField {
    pub fn try_new(name: impl Into<String>, value: FxUniformValue) -> Result<Self, FxUniformError> {
        value.validate()?;
        Ok(Self {
            name: FxUniformName::try_new(name)?,
            value,
        })
    }
    pub const fn name(&self) -> &FxUniformName {
        &self.name
    }
    pub const fn value(&self) -> &FxUniformValue {
        &self.value
    }
}
impl FxUniformRecord {
    pub fn decode_canonical_v1(bytes: &[u8]) -> Result<Self, FxUniformRecordDecodeError> {
        let byte_len =
            u64::try_from(bytes.len()).map_err(|_| FxCanonicalDecodeError::LengthOverflow)?;
        if bytes.len() > FX_MAX_UNIFORM_RECORD_CANONICAL_BYTES {
            return Err(FxUniformRecordDecodeError::OwnerLimit {
                kind: "byte",
                actual: byte_len,
                limit: FX_MAX_UNIFORM_RECORD_CANONICAL_BYTES,
            });
        }
        let mut reader = CanonicalReader::new(bytes);
        let record = Self::decode_canonical_v1_body(&mut reader)?;
        reader.finish()?;
        Ok(record)
    }

    pub(super) fn decode_canonical_v1_reader(
        reader: &mut CanonicalReader<'_>,
        encoded_len: usize,
    ) -> Result<Self, FxUniformRecordDecodeError> {
        let bytes = reader.raw_bytes(encoded_len)?;
        Self::decode_canonical_v1(bytes)
    }

    fn decode_canonical_v1_body(
        reader: &mut CanonicalReader<'_>,
    ) -> Result<Self, FxUniformRecordDecodeError> {
        reader.domain_v1(PREFIX)?;
        let field_count = decode_uniform_count(reader, FX_MAX_UNIFORM_FIELDS, "field")?;
        let mut fields = Vec::new();
        fields.try_reserve_exact(field_count).map_err(|_| {
            FxUniformRecordDecodeError::AllocationFailed {
                kind: "field",
                count: field_count,
            }
        })?;
        let mut previous_name: Option<String> = None;
        for _ in 0..field_count {
            let name = decode_uniform_name(reader)?;
            if let Some(previous) = &previous_name
                && previous.as_bytes() >= name.as_bytes()
            {
                return Err(FxUniformRecordDecodeError::NonCanonicalFieldOrder {
                    previous: previous.clone(),
                    current: name,
                });
            }
            let value = decode_uniform_value(reader)?;
            fields.push(FxUniformField::try_new(name.clone(), value)?);
            previous_name = Some(name);
        }
        Ok(Self::try_new(fields)?)
    }

    pub fn try_new(mut fields: Vec<FxUniformField>) -> Result<Self, FxUniformError> {
        if fields.len() > FX_MAX_UNIFORM_FIELDS {
            return Err(FxUniformError::FieldLimit(fields.len()));
        }
        let field_count = u8::try_from(fields.len()).map_err(|_| FxUniformError::LengthOverflow)?;
        for f in &fields {
            f.value.validate()?;
        }
        fields.sort_by(|a, b| a.name.value.as_bytes().cmp(b.name.value.as_bytes()));
        for pair in fields.windows(2) {
            if pair[0].name == pair[1].name {
                return Err(FxUniformError::Duplicate(pair[0].name.value.clone()));
            }
        }
        let canonical_len = measure_canonical(&fields)?;
        if canonical_len > FX_MAX_UNIFORM_RECORD_CANONICAL_BYTES {
            return Err(FxUniformError::ByteLimit);
        }
        let canonical_len =
            u32::try_from(canonical_len).map_err(|_| FxUniformError::LengthOverflow)?;
        Ok(Self {
            fields: fields.into(),
            field_count,
            canonical_len,
        })
    }
    pub fn fields(&self) -> &[FxUniformField] {
        &self.fields
    }
    pub fn validate_definition_layout(
        &self,
        layout: &FxDefinitionParameterLayout,
    ) -> Result<(), FxUniformError> {
        for f in &self.fields {
            f.value.validate_definition_layout(layout)?;
        }
        Ok(())
    }
    pub const fn canonical_len(&self) -> u64 {
        self.canonical_len as u64
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, FxUniformError> {
        let mut output = Vec::new();
        let canonical_len =
            usize::try_from(self.canonical_len).map_err(|_| FxUniformError::LengthOverflow)?;
        let sink = CanonicalVecSink::with_preflight(&mut output, canonical_len)
            .map_err(map_canonical_error)?;
        let mut encoder = CanonicalEncoder::new(sink);
        self.encode_canonical_v1(&mut encoder)
            .map_err(map_canonical_error)?;
        encoder.into_inner().finish().map_err(map_canonical_error)?;
        Ok(output)
    }

    pub fn digest(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        {
            let mut encoder = CanonicalEncoder::new(CanonicalHashSink::new(&mut hasher));
            match self.encode_canonical_v1(&mut encoder) {
                Ok(()) => {}
                Err(error) => match error {},
            }
        }
        *hasher.finalize().as_bytes()
    }

    pub(super) fn encode_canonical_v1<S: CanonicalSink>(
        &self,
        encoder: &mut CanonicalEncoder<S>,
    ) -> Result<(), S::Error> {
        encode_canonical(encoder, self.field_count, &self.fields)
    }
}

fn decode_uniform_count(
    reader: &mut CanonicalReader<'_>,
    limit: usize,
    kind: &'static str,
) -> Result<usize, FxUniformRecordDecodeError> {
    let actual = reader.unsigned()?;
    let actual_usize =
        usize::try_from(actual).map_err(|_| FxUniformRecordDecodeError::OwnerLimit {
            kind,
            actual,
            limit,
        })?;
    if actual_usize > limit {
        return Err(FxUniformRecordDecodeError::OwnerLimit {
            kind,
            actual,
            limit,
        });
    }
    Ok(actual_usize)
}

fn decode_uniform_name(
    reader: &mut CanonicalReader<'_>,
) -> Result<String, FxUniformRecordDecodeError> {
    let length = decode_uniform_count(reader, FX_MAX_UNIFORM_NAME_BYTES, "name byte")?;
    let bytes = reader.raw_bytes(length)?;
    let source = std::str::from_utf8(bytes).map_err(|_| FxCanonicalDecodeError::InvalidUtf8)?;
    let mut name = String::new();
    name.try_reserve_exact(length)
        .map_err(|_| FxUniformRecordDecodeError::AllocationFailed {
            kind: "name byte",
            count: length,
        })?;
    name.push_str(source);
    Ok(name)
}

fn decode_uniform_type(tag: u8) -> Result<FxUniformType, FxUniformRecordDecodeError> {
    match tag {
        0 => Ok(FxUniformType::Bool),
        1 => Ok(FxUniformType::I32),
        2 => Ok(FxUniformType::F32),
        3 => Ok(FxUniformType::Length),
        4 => Ok(FxUniformType::Angle),
        5 => Ok(FxUniformType::Seconds),
        6 => Ok(FxUniformType::Color),
        7 => Ok(FxUniformType::Vec2),
        _ => Err(FxUniformRecordDecodeError::UnknownUniformType(tag)),
    }
}

fn decode_uniform_value(
    reader: &mut CanonicalReader<'_>,
) -> Result<FxUniformValue, FxUniformRecordDecodeError> {
    let value_tag = reader.tag()?;
    let declared = decode_uniform_type(reader.tag()?)?;
    let value = match value_tag {
        0 => FxUniformValue::constant(decode_uniform_constant(reader, declared)?)?,
        1 => {
            let slot = reader.unsigned()?;
            let slot_index = usize::try_from(slot).map_err(|_| {
                FxUniformRecordDecodeError::IntegerOutOfRange {
                    kind: "parameter slot",
                    value: slot,
                }
            })?;
            let slot = FxRuntimeParameterSlot::from_index(slot_index).ok_or(
                FxUniformRecordDecodeError::IntegerOutOfRange {
                    kind: "parameter slot",
                    value: slot,
                },
            )?;
            FxUniformValue::parameter(FxRuntimeParameterRef::from_parts(
                slot,
                declared.runtime_type(),
            ))?
        }
        2 => {
            let length = reader.length()?;
            let bytes = reader.raw_bytes(length)?;
            FxUniformValue::program(FxSamplerProgram::decode_canonical_v1(bytes)?)?
        }
        tag => return Err(FxUniformRecordDecodeError::UnknownValueTag(tag)),
    };
    let actual = value.uniform_type();
    if actual != declared {
        return Err(FxUniformRecordDecodeError::ValueTypeMismatch { declared, actual });
    }
    Ok(value)
}

fn decode_uniform_constant(
    reader: &mut CanonicalReader<'_>,
    ty: FxUniformType,
) -> Result<FxRuntimeValue, FxUniformRecordDecodeError> {
    let value = match ty {
        FxUniformType::Bool => FxRuntimeValue::Bool(reader.boolean()?),
        FxUniformType::I32 => FxRuntimeValue::I32(reader.signed_i32()?),
        FxUniformType::F32 => FxRuntimeValue::F32(decode_uniform_finite(reader)?),
        FxUniformType::Length => {
            FxRuntimeValue::Length(Length::try_pixels(decode_uniform_finite(reader)?.get())?)
        }
        FxUniformType::Angle => {
            FxRuntimeValue::Angle(Angle::try_radians(decode_uniform_finite(reader)?.get())?)
        }
        FxUniformType::Seconds => {
            FxRuntimeValue::Seconds(Seconds::try_seconds(decode_uniform_finite(reader)?.get())?)
        }
        FxUniformType::Color => FxRuntimeValue::Color(FxColor::new(
            Opacity::try_new(decode_uniform_finite(reader)?)?,
            Opacity::try_new(decode_uniform_finite(reader)?)?,
            Opacity::try_new(decode_uniform_finite(reader)?)?,
            Opacity::try_new(decode_uniform_finite(reader)?)?,
        )),
        FxUniformType::Vec2 => FxRuntimeValue::Vec2(FxVec2 {
            x: decode_uniform_finite(reader)?,
            y: decode_uniform_finite(reader)?,
        }),
    };
    Ok(value)
}

fn decode_uniform_finite(
    reader: &mut CanonicalReader<'_>,
) -> Result<FiniteF32, FxUniformRecordDecodeError> {
    let bits = reader.f32_bits()?;
    let value = FiniteF32::try_from_bits(bits)?;
    if value.to_bits() != bits {
        return Err(FxUniformRecordDecodeError::NonCanonicalFloat { bits });
    }
    Ok(value)
}
impl Serialize for FxUniformRecord {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.fields.serialize(s)
    }
}
impl<'de> Deserialize<'de> for FxUniformRecord {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::try_new(Vec::<FxUniformField>::deserialize(d)?).map_err(D::Error::custom)
    }
}

fn measure_canonical(fields: &[FxUniformField]) -> Result<usize, FxUniformError> {
    let field_count = u8::try_from(fields.len()).map_err(|_| FxUniformError::LengthOverflow)?;
    let mut encoder = CanonicalEncoder::new(CanonicalLengthSink::default());
    encode_canonical(&mut encoder, field_count, fields).map_err(map_canonical_error)?;
    Ok(encoder.into_inner().finish())
}

fn encode_canonical<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    field_count: u8,
    fields: &[FxUniformField],
) -> Result<(), S::Error> {
    encoder.domain_v1(PREFIX)?;
    encoder.unsigned(u64::from(field_count))?;
    for field in fields {
        encoder.unsigned(u64::from(field.name.byte_len()))?;
        encoder.raw_bytes(field.name.value.as_bytes())?;
        encode_value(encoder, &field.value)?;
    }
    Ok(())
}

fn encode_value<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    v: &FxUniformValue,
) -> Result<(), S::Error> {
    let tag = match v {
        FxUniformValue::Constant(_) => 0,
        FxUniformValue::Parameter(_) => 1,
        FxUniformValue::Program(_) => 2,
    };
    encoder.tag(tag)?;
    encoder.tag(v.uniform_type() as u8)?;
    match v {
        FxUniformValue::Constant(v) => encode_runtime(encoder, v.value)?,
        FxUniformValue::Parameter(parameter) => {
            encoder.unsigned(u64::from(parameter.reference().slot().get()))?;
        }
        FxUniformValue::Program(v) => {
            encoder.unsigned(v.canonical_len())?;
            v.sampler.encode_canonical_v1(encoder)?;
        }
    }
    Ok(())
}

fn encode_runtime<S: CanonicalSink>(
    encoder: &mut CanonicalEncoder<S>,
    v: FxUniformConstantValue,
) -> Result<(), S::Error> {
    match v {
        FxUniformConstantValue::Bool(v) => encoder.boolean(v)?,
        FxUniformConstantValue::I32(v) => encoder.signed_i32(v)?,
        FxUniformConstantValue::F32(v) => encoder.f32_bits(v.to_bits())?,
        FxUniformConstantValue::Length(v) => encoder.f32_bits(v.value().to_bits())?,
        FxUniformConstantValue::Angle(v) => encoder.f32_bits(v.value().to_bits())?,
        FxUniformConstantValue::Seconds(v) => encoder.f32_bits(v.value().to_bits())?,
        FxUniformConstantValue::Color(v) => {
            for c in [v.red(), v.green(), v.blue(), v.alpha()] {
                encoder.f32_bits(c.value().to_bits())?;
            }
        }
        FxUniformConstantValue::Vec2(v) => {
            encoder.f32_bits(v.x.to_bits())?;
            encoder.f32_bits(v.y.to_bits())?;
        }
    }
    Ok(())
}

fn map_canonical_error(error: CanonicalEncodeError) -> FxUniformError {
    match error {
        CanonicalEncodeError::LengthOverflow => FxUniformError::LengthOverflow,
        CanonicalEncodeError::AllocationFailed => FxUniformError::AllocationFailed,
        CanonicalEncodeError::LengthMismatch { actual, expected } => {
            FxUniformError::LengthMismatch { actual, expected }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::{
        FxDefinition, FxDefinitionParameter, FxDefinitionParameterType, FxGraph, FxId, Transform2D,
        ValueInstruction, ValueProgramSchema,
    };

    fn field(name: &str, value: FxUniformValue) -> FxUniformField {
        FxUniformField::try_new(name, value).expect("field")
    }
    fn bool_value() -> FxUniformValue {
        FxUniformValue::constant(FxRuntimeValue::Bool(true)).expect("bool")
    }
    fn runtime_definition(ty: FxRuntimeType) -> FxDefinition {
        FxDefinition::new(
            FxId::try_new("test", "uniform.layout").unwrap(),
            vec![
                FxDefinitionParameter::try_new(
                    0,
                    "value",
                    FxDefinitionParameterType::Runtime(ty),
                    None,
                )
                .unwrap(),
            ],
            FxGraph::default(),
        )
        .unwrap()
    }
    fn program(
        parameters: Vec<FxRuntimeType>,
        states: Vec<FxRuntimeType>,
        return_type: FxRuntimeType,
    ) -> FxSamplerProgram {
        let value = match return_type {
            FxRuntimeType::Bool => FxRuntimeValue::Bool(true),
            FxRuntimeType::Transform2D => FxRuntimeValue::Transform2D(Transform2D::default()),
            FxRuntimeType::U32 => FxRuntimeValue::U32(1),
            _ => FxRuntimeValue::I32(1),
        };
        FxSamplerProgram::validate(
            ValueProgramSchema::new(parameters, states, return_type),
            vec![
                ValueInstruction::Constant { value },
                ValueInstruction::Return,
            ],
        )
        .expect("program")
    }

    #[test]
    fn uniform_name_boundaries() {
        assert!(FxUniformName::try_new("").is_err());
        assert!(FxUniformName::try_new("a".repeat(65)).is_err());
        assert!(FxUniformName::try_new("é").is_err());
        assert!(FxUniformName::try_new(format!("a{}", "x".repeat(63))).is_ok());
    }
    #[test]
    fn slot_boundaries_and_serde_revalidation() {
        let schema =
            ValueProgramSchema::new(vec![FxRuntimeType::Bool; 65], vec![], FxRuntimeType::Bool);
        assert!(FxUniformValue::parameter(schema.parameter_ref(63).unwrap()).is_ok());
        assert_eq!(
            FxUniformValue::parameter(schema.parameter_ref(64).unwrap()),
            Err(FxUniformError::SlotLimit(64))
        );
        let json = r#"[{"name":"x","value":{"kind":"parameter","value":{"slot":64,"ty":"bool"}}}]"#;
        assert!(serde_json::from_str::<FxUniformRecord>(json).is_err());
        assert!(
            serde_json::from_str::<FxUniformValue>(
                r#"{"kind":"parameter","value":{"slot":64,"ty":"bool"}}"#
            )
            .is_err()
        );
    }
    #[test]
    fn records_sort_and_reject_duplicates() {
        let r = FxUniformRecord::try_new(vec![field("z", bool_value()), field("a", bool_value())])
            .expect("record");
        assert_eq!(
            r.fields()
                .iter()
                .map(|f| f.name().as_str())
                .collect::<Vec<_>>(),
            vec!["a", "z"]
        );
        assert!(matches!(
            FxUniformRecord::try_new(vec![field("a", bool_value()), field("a", bool_value())]),
            Err(FxUniformError::Duplicate(_))
        ));
    }
    #[test]
    fn constant_and_program_domains_are_revalidated() {
        let transform = serde_json::to_value(FxRuntimeValue::Transform2D(Transform2D::default()))
            .expect("json");
        assert!(serde_json::from_value::<FxUniformConstant>(transform).is_err());
        assert!(
            FxUniformProgram::try_new(program(
                vec![],
                vec![FxRuntimeType::Bool],
                FxRuntimeType::Bool
            ))
            .is_err()
        );
        assert!(
            FxUniformProgram::try_new(program(vec![], vec![], FxRuntimeType::Transform2D)).is_err()
        );
        assert_eq!(
            FxUniformValue::constant(FxRuntimeValue::U32(0)),
            Err(FxUniformError::InvalidType(FxRuntimeType::U32))
        );
        assert_eq!(
            FxUniformProgram::try_new(program(vec![], vec![], FxRuntimeType::U32)),
            Err(FxUniformError::InvalidType(FxRuntimeType::U32))
        );
    }
    #[test]
    fn empty_record_has_exact_v1_bytes_and_digest() {
        let r = FxUniformRecord::try_new(vec![]).expect("record");
        let mut expected = PREFIX.to_vec();
        expected.extend_from_slice(&[0, 1, 0]);
        assert_eq!(r.canonical_bytes().unwrap(), expected);
        assert_eq!(r.digest(), *blake3::hash(&expected).as_bytes());
    }
    #[test]
    fn definition_schema_is_exact() {
        let bool_definition = runtime_definition(FxRuntimeType::Bool);
        let i32_definition = runtime_definition(FxRuntimeType::I32);
        let r = FxUniformRecord::try_new(vec![field(
            "x",
            FxUniformValue::parameter(
                bool_definition.parameter_layout().runtime_rows()[0].reference(),
            )
            .expect("parameter"),
        )])
        .expect("record");
        assert!(
            r.validate_definition_layout(bool_definition.parameter_layout())
                .is_ok()
        );
        assert_eq!(
            r.validate_definition_layout(i32_definition.parameter_layout()),
            Err(FxUniformError::DefinitionSchema)
        );
        let empty = FxDefinition::new(
            FxId::try_new("test", "uniform.empty").unwrap(),
            vec![],
            FxGraph::default(),
        )
        .unwrap();
        assert_eq!(
            r.validate_definition_layout(empty.parameter_layout()),
            Err(FxUniformError::DefinitionSchema)
        );
    }
    #[test]
    fn field_count_limit_is_exact() {
        let fields = (0..62)
            .map(|i| field(&format!("u{i}"), bool_value()))
            .collect();
        assert!(FxUniformRecord::try_new(fields).is_ok());
        let fields = (0..63)
            .map(|i| field(&format!("u{i}"), bool_value()))
            .collect();
        assert_eq!(
            FxUniformRecord::try_new(fields),
            Err(FxUniformError::FieldLimit(63))
        );
    }

    fn long_sampler(abs_count: usize) -> FxSamplerProgram {
        let mut instructions = vec![ValueInstruction::Constant {
            value: FxRuntimeValue::F32(FiniteF32::ONE),
        }];
        instructions.extend((0..abs_count).map(|_| ValueInstruction::Abs));
        instructions.push(ValueInstruction::Return);
        FxSamplerProgram::validate(
            ValueProgramSchema::new(vec![], vec![], FxRuntimeType::F32),
            instructions,
        )
        .expect("long sampler fixture is within owner limits")
    }

    fn boundary_fields(extra_bytes: bool) -> Vec<FxUniformField> {
        (0..62)
            .map(|index| {
                // The base shape is 65,500 bytes.  Thirty-six fields carry
                // one extra `Abs` instruction, reaching exactly 65,536; the
                // optional first-field increment is therefore the first byte
                // beyond the public canonical budget.
                let extra_abs = usize::from(index < 36) + usize::from(extra_bytes && index == 0);
                field(
                    &format!("u{index:02}"),
                    FxUniformValue::program(long_sampler(1008 + extra_abs))
                        .expect("sampler is a valid uniform program"),
                )
            })
            .collect()
    }

    #[test]
    fn canonical_uniform_record_accepts_exact_65536_bytes() {
        let record = FxUniformRecord::try_new(boundary_fields(false))
            .expect("exact canonical budget is accepted");
        assert_eq!(record.canonical_len(), 65_536);
        assert_eq!(record.canonical_bytes().unwrap().len(), 65_536);
    }

    #[test]
    fn canonical_uniform_record_rejects_first_65537_byte() {
        assert_eq!(
            FxUniformRecord::try_new(boundary_fields(true)),
            Err(FxUniformError::ByteLimit)
        );
    }

    #[test]
    fn canonical_uniform_decode_round_trips_constants_parameters_and_programs() {
        let definition = runtime_definition(FxRuntimeType::Bool);
        let sampler = FxSamplerProgram::validate(
            ValueProgramSchema::new(Vec::new(), Vec::new(), FxRuntimeType::F32),
            vec![
                ValueInstruction::Constant {
                    value: FxRuntimeValue::F32(FiniteF32::ONE),
                },
                ValueInstruction::Return,
            ],
        )
        .unwrap();
        let record = FxUniformRecord::try_new(vec![
            field(
                "program",
                FxUniformValue::program(sampler).expect("uniform program"),
            ),
            field(
                "constant",
                FxUniformValue::constant(FxRuntimeValue::I32(-65)).unwrap(),
            ),
            field(
                "parameter",
                FxUniformValue::parameter(
                    definition.parameter_layout().runtime_rows()[0].reference(),
                )
                .unwrap(),
            ),
        ])
        .unwrap();
        let bytes = record.canonical_bytes().unwrap();
        let decoded = FxUniformRecord::decode_canonical_v1(&bytes).unwrap();
        assert_eq!(decoded, record);
        assert_eq!(decoded.canonical_bytes().unwrap(), bytes);
    }

    fn one_bool_field() -> Vec<u8> {
        let mut bytes = PREFIX.to_vec();
        bytes.extend_from_slice(&[0, 1, 1, 1, b'x', 0, 0, 1]);
        bytes
    }

    #[test]
    fn canonical_uniform_decode_rejects_unknown_tags_order_and_noncanonical_float() {
        let value_tag = PREFIX.len() + 5;
        let type_tag = value_tag + 1;

        let mut unknown_value = one_bool_field();
        unknown_value[value_tag] = 0xff;
        assert_eq!(
            FxUniformRecord::decode_canonical_v1(&unknown_value),
            Err(FxUniformRecordDecodeError::UnknownValueTag(0xff))
        );

        let mut unknown_type = one_bool_field();
        unknown_type[type_tag] = 0xff;
        assert_eq!(
            FxUniformRecord::decode_canonical_v1(&unknown_type),
            Err(FxUniformRecordDecodeError::UnknownUniformType(0xff))
        );

        let mut out_of_order = PREFIX.to_vec();
        out_of_order.extend_from_slice(&[0, 1, 2, 1, b'b', 0, 0, 1, 1, b'a', 0, 0, 1]);
        assert_eq!(
            FxUniformRecord::decode_canonical_v1(&out_of_order),
            Err(FxUniformRecordDecodeError::NonCanonicalFieldOrder {
                previous: "b".to_owned(),
                current: "a".to_owned()
            })
        );

        let mut negative_zero = PREFIX.to_vec();
        negative_zero.extend_from_slice(&[0, 1, 1, 1, b'x', 0, 2]);
        negative_zero.extend_from_slice(&(-0.0_f32).to_bits().to_le_bytes());
        assert_eq!(
            FxUniformRecord::decode_canonical_v1(&negative_zero),
            Err(FxUniformRecordDecodeError::NonCanonicalFloat {
                bits: (-0.0_f32).to_bits()
            })
        );
    }

    #[test]
    fn canonical_uniform_decode_rejects_nested_type_mismatch_and_primitive_tamper() {
        let sampler = FxSamplerProgram::validate(
            ValueProgramSchema::new(Vec::new(), Vec::new(), FxRuntimeType::F32),
            vec![
                ValueInstruction::Constant {
                    value: FxRuntimeValue::F32(FiniteF32::ONE),
                },
                ValueInstruction::Return,
            ],
        )
        .unwrap();
        let sampler_bytes = sampler.canonical_v1_bytes().unwrap();
        let mut mismatch = PREFIX.to_vec();
        mismatch.extend_from_slice(&[0, 1, 1, 1, b'x', 2, 0]);
        mismatch.push(u8::try_from(sampler_bytes.len()).expect("focused sampler length"));
        mismatch.extend_from_slice(&sampler_bytes);
        assert_eq!(
            FxUniformRecord::decode_canonical_v1(&mismatch),
            Err(FxUniformRecordDecodeError::ValueTypeMismatch {
                declared: FxUniformType::Bool,
                actual: FxUniformType::F32
            })
        );

        let mut invalid_bool = one_bool_field();
        *invalid_bool.last_mut().unwrap() = 2;
        assert_eq!(
            FxUniformRecord::decode_canonical_v1(&invalid_bool),
            Err(FxUniformRecordDecodeError::Canonical(
                FxCanonicalDecodeError::InvalidBool(2)
            ))
        );

        let mut overlong = one_bool_field();
        let field_count = PREFIX.len() + 2;
        overlong.splice(field_count..=field_count, [0x81, 0]);
        assert_eq!(
            FxUniformRecord::decode_canonical_v1(&overlong),
            Err(FxUniformRecordDecodeError::Canonical(
                FxCanonicalDecodeError::NonCanonicalVarint
            ))
        );

        let mut truncated = one_bool_field();
        truncated.pop();
        assert_eq!(
            FxUniformRecord::decode_canonical_v1(&truncated),
            Err(FxUniformRecordDecodeError::Canonical(
                FxCanonicalDecodeError::Truncated
            ))
        );

        let mut trailing = one_bool_field();
        trailing.push(0);
        assert_eq!(
            FxUniformRecord::decode_canonical_v1(&trailing),
            Err(FxUniformRecordDecodeError::Canonical(
                FxCanonicalDecodeError::TrailingBytes(1)
            ))
        );
    }

    #[test]
    fn canonical_uniform_decode_rejects_first_byte_beyond_owner_budget() {
        assert_eq!(
            FxUniformRecord::decode_canonical_v1(&vec![0; 65_537]),
            Err(FxUniformRecordDecodeError::OwnerLimit {
                kind: "byte",
                actual: 65_537,
                limit: FX_MAX_UNIFORM_RECORD_CANONICAL_BYTES
            })
        );
    }
}
