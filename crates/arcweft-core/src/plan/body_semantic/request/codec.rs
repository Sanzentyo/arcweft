//! Private version-one projection of the static request definition. Decoded
//! roles are data; the actual F endpoint and type owner must recompute Q.

use super::{
    RuntimeRequestArgument, RuntimeRequestArgumentRole, RuntimeRequestField,
    RuntimeRequestFieldRole, RuntimeRequestPathStep, RuntimeRequestRoleIdentity,
    RuntimeRequestValueSource, RuntimeTaskRequestSource, RuntimeTaskRequestTemplate,
};
use crate::runtime_id::RuntimePlanTypeId;
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use std::num::NonZeroU32;

#[derive(Clone, Copy)]
pub(crate) struct RuntimeTaskRequestCodecLimits {
    pub(crate) roles: u32,
    pub(crate) path_steps: u32,
    pub(crate) encoded_bytes: usize,
    pub(crate) decoded_bytes: usize,
    pub(crate) type_count: usize,
}

/// A complete, allocation-free check of immutable source bytes. This carries
/// storage requirements, never a request digest or an executable capability.
pub(crate) struct ValidatedRuntimeTaskRequestSource<'a> {
    bytes: &'a [u8],
    limits: RuntimeTaskRequestCodecLimits,
    storage_bytes: usize,
    arguments: usize,
    fields: usize,
}

/// Borrows the exact original definition after its complete encoding count.
/// An aggregate writer can append it into its already reserved buffer.
pub(crate) struct PreparedRuntimeTaskRequestEncoding<'a> {
    definition: &'a RuntimeTaskRequestTemplate,
    limits: RuntimeTaskRequestCodecLimits,
    encoded_bytes: usize,
    storage_bytes: usize,
}

impl PreparedRuntimeTaskRequestEncoding<'_> {
    pub(crate) const fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }

    pub(crate) const fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }

    /// Includes the old and mapped role arrays; their paths move only once.
    pub(crate) fn mapped_type_storage_bytes<Next>(
        &self,
    ) -> Result<usize, RuntimeTaskRequestCodecError> {
        mapping_storage_bytes::<Next>(
            self.storage_bytes,
            self.definition.arguments.len(),
            self.definition.fields.len(),
            self.limits,
        )
    }

    pub(crate) fn append_to(
        self,
        output: &mut Vec<u8>,
    ) -> Result<(), RuntimeTaskRequestCodecError> {
        let remaining = output.capacity() - output.len();
        if self.encoded_bytes > remaining {
            return Err(RuntimeTaskRequestCodecError::OutputCapacity {
                required: self.encoded_bytes,
                remaining,
            });
        }
        let mut writer = Writer {
            bytes: Some(output),
            meter: None,
            length: 0,
            storage_bytes: 0,
            limits: self.limits,
        };
        self.definition.write_source(&mut writer)?;
        debug_assert_eq!(writer.length, self.encoded_bytes);
        debug_assert_eq!(writer.storage_bytes, self.storage_bytes);
        Ok(())
    }
}

impl ValidatedRuntimeTaskRequestSource<'_> {
    pub(crate) const fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }

    /// Admits type mapping before the borrowed roles or their paths are copied.
    pub(crate) fn mapped_type_storage_bytes<Next>(
        &self,
    ) -> Result<usize, RuntimeTaskRequestCodecError> {
        mapping_storage_bytes::<Next>(self.storage_bytes, self.arguments, self.fields, self.limits)
    }

    pub(crate) fn materialize(
        self,
    ) -> Result<RuntimeTaskRequestTemplate, RuntimeTaskRequestCodecError> {
        let mut reader = Reader {
            bytes: self.bytes,
            offset: 0,
            limits: self.limits,
            storage_bytes: 0,
            arguments: 0,
            fields: 0,
        };
        let template = reader
            .request(true)?
            .expect("materializing a validated request retains its rows");
        debug_assert_eq!(reader.storage_bytes, self.storage_bytes);
        Ok(template)
    }
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum RuntimeTaskRequestCodecError {
    #[error("static request resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("unsupported static request version {actual}")]
    Version { actual: u8 },
    #[error("unknown {kind} tag {tag}")]
    Tag { kind: &'static str, tag: u8 },
    #[error("noncanonical request option {actual}")]
    Option { actual: u8 },
    #[error("request source ordinal {actual} differs from {expected}")]
    Ordinal { expected: u32, actual: u32 },
    #[error("request type coordinate is zero")]
    ZeroType,
    #[error("static request {kind} count {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("static request codec count arithmetic overflow")]
    Arithmetic,
    #[error("static request codec input is truncated")]
    Truncated,
    #[error("static request codec has trailing data")]
    Trailing,
    #[error("static request needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}

impl RuntimeTaskRequestTemplate {
    pub(crate) fn encode(
        &self,
        limits: RuntimeTaskRequestCodecLimits,
    ) -> Result<Vec<u8>, RuntimeTaskRequestCodecError> {
        let prepared = self.prepare_encoding(limits)?;
        let mut output = Vec::with_capacity(prepared.encoded_bytes());
        prepared.append_to(&mut output)?;
        Ok(output)
    }

    pub(crate) fn prepare_encoding(
        &self,
        limits: RuntimeTaskRequestCodecLimits,
    ) -> Result<PreparedRuntimeTaskRequestEncoding<'_>, RuntimeTaskRequestCodecError> {
        self.count_encoding(limits, None)
    }

    pub(crate) fn prepare_encoding_with_meter(
        &self,
        limits: RuntimeTaskRequestCodecLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeTaskRequestEncoding<'_>, RuntimeTaskRequestCodecError> {
        meter
            .status()
            .map_err(RuntimeTaskRequestCodecError::Resource)?;
        self.count_encoding(limits, Some(meter))
    }

    fn count_encoding(
        &self,
        limits: RuntimeTaskRequestCodecLimits,
        meter: Option<&mut TaskSemanticMeter>,
    ) -> Result<PreparedRuntimeTaskRequestEncoding<'_>, RuntimeTaskRequestCodecError> {
        let mut count = Writer {
            bytes: None,
            meter,
            length: 0,
            storage_bytes: 0,
            limits,
        };
        self.write_source(&mut count)?;
        Ok(PreparedRuntimeTaskRequestEncoding {
            definition: self,
            limits,
            encoded_bytes: count.length,
            storage_bytes: count.storage_bytes,
        })
    }

    fn write_source(&self, writer: &mut Writer) -> Result<(), RuntimeTaskRequestCodecError> {
        let limits = writer.limits;
        check_roles(self.arguments.len(), self.fields.len(), limits)?;
        charge_storage::<RuntimeRequestArgument>(
            &mut writer.storage_bytes,
            self.arguments.len(),
            limits,
        )?;
        charge_storage::<RuntimeRequestField>(
            &mut writer.storage_bytes,
            self.fields.len(),
            limits,
        )?;
        writer.put(&[1])?;
        match self.source {
            RuntimeTaskRequestSource::FunctionRoot => writer.put(&[0])?,
            RuntimeTaskRequestSource::Operation(ordinal) => {
                writer.put(&[1])?;
                writer.u32(ordinal)?;
            }
        }
        writer.count(self.arguments.len())?;
        for (ordinal, argument) in self.arguments.iter().enumerate() {
            writer.count(ordinal)?;
            writer.put(&[
                argument.role.semantic_tag(),
                u8::from(argument.identity.is_some()),
            ])?;
            if let Some(identity) = argument.identity {
                writer.put(identity.as_bytes())?;
            }
            writer.ty(argument.ty)?;
            writer.put(&[argument.source.semantic_tag()])?;
            writer.path(&argument.path)?;
        }
        writer.count(self.fields.len())?;
        for (ordinal, field) in self.fields.iter().enumerate() {
            writer.count(ordinal)?;
            writer.put(field.identity.as_bytes())?;
            writer.put(&[field.role.semantic_tag()])?;
            writer.ty(field.ty)?;
            writer.path(&field.path)?;
        }
        Ok(())
    }

    pub(crate) fn decode(
        bytes: &[u8],
        limits: RuntimeTaskRequestCodecLimits,
    ) -> Result<Self, RuntimeTaskRequestCodecError> {
        Self::preflight_source(bytes, limits)?.materialize()
    }

    pub(crate) fn preflight_source(
        bytes: &[u8],
        limits: RuntimeTaskRequestCodecLimits,
    ) -> Result<ValidatedRuntimeTaskRequestSource<'_>, RuntimeTaskRequestCodecError> {
        check("bytes", bytes.len(), limits.encoded_bytes)?;
        let mut reader = Reader {
            bytes,
            offset: 0,
            limits,
            storage_bytes: 0,
            arguments: 0,
            fields: 0,
        };
        reader.request(false)?;
        Ok(ValidatedRuntimeTaskRequestSource {
            bytes,
            limits,
            storage_bytes: reader.storage_bytes,
            arguments: reader.arguments,
            fields: reader.fields,
        })
    }
}

impl Reader<'_> {
    fn request(
        &mut self,
        materialize: bool,
    ) -> Result<Option<RuntimeTaskRequestTemplate>, RuntimeTaskRequestCodecError> {
        let limits = self.limits;
        let version = self.tag()?;
        if version != 1 {
            return Err(RuntimeTaskRequestCodecError::Version { actual: version });
        }
        let source = match self.tag()? {
            0 => RuntimeTaskRequestSource::FunctionRoot,
            1 => RuntimeTaskRequestSource::Operation(self.u32()?),
            tag => {
                return Err(RuntimeTaskRequestCodecError::Tag {
                    kind: "request source",
                    tag,
                });
            }
        };
        let count = self.count("roles", limits.roles as usize, 15)?;
        charge_storage::<RuntimeRequestArgument>(&mut self.storage_bytes, count, limits)?;
        self.arguments = count;
        let mut arguments = materialize.then(|| Vec::with_capacity(count));
        for ordinal in 0..count {
            self.ordinal(ordinal)?;
            let role = RuntimeRequestArgumentRole::decode_tag(self.tag()?)?;
            let identity = match self.tag()? {
                0 => None,
                1 => Some(self.identity()?),
                actual => return Err(RuntimeTaskRequestCodecError::Option { actual }),
            };
            let ty = self.ty()?;
            let source = RuntimeRequestValueSource::decode_tag(self.tag()?)?;
            let path = self.path(materialize)?;
            if let Some(arguments) = &mut arguments {
                arguments.push(RuntimeRequestArgument {
                    role,
                    identity,
                    ty,
                    source,
                    path: path.expect("materializing an argument retains its path"),
                });
            }
        }
        let fields_count = self.u32()? as usize;
        check_roles(count, fields_count, limits)?;
        self.minimum_bytes(fields_count, 45)?;
        charge_storage::<RuntimeRequestField>(&mut self.storage_bytes, fields_count, limits)?;
        self.fields = fields_count;
        let mut fields = materialize.then(|| Vec::with_capacity(fields_count));
        for ordinal in 0..fields_count {
            self.ordinal(ordinal)?;
            let identity = self.identity()?;
            let role = RuntimeRequestFieldRole::decode_tag(self.tag()?)?;
            let ty = self.ty()?;
            let path = self.path(materialize)?;
            if let Some(fields) = &mut fields {
                fields.push(RuntimeRequestField {
                    identity,
                    role,
                    ty,
                    path: path.expect("materializing a field retains its path"),
                });
            }
        }
        if self.offset != self.bytes.len() {
            return Err(RuntimeTaskRequestCodecError::Trailing);
        }
        Ok(arguments
            .zip(fields)
            .map(|(arguments, fields)| RuntimeTaskRequestTemplate {
                source,
                arguments: arguments.into_boxed_slice(),
                fields: fields.into_boxed_slice(),
            }))
    }
}

fn check(
    kind: &'static str,
    actual: usize,
    maximum: usize,
) -> Result<(), RuntimeTaskRequestCodecError> {
    if actual > maximum {
        Err(RuntimeTaskRequestCodecError::Limit {
            kind,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}
fn check_roles(
    arguments: usize,
    fields: usize,
    limits: RuntimeTaskRequestCodecLimits,
) -> Result<(), RuntimeTaskRequestCodecError> {
    let actual = u32::try_from(arguments)
        .ok()
        .and_then(|count| {
            u32::try_from(fields)
                .ok()
                .and_then(|fields| count.checked_add(fields))
        })
        .ok_or(RuntimeTaskRequestCodecError::Arithmetic)?;
    check("roles", actual as usize, limits.roles as usize)
}

fn charge_storage<T>(
    total: &mut usize,
    count: usize,
    limits: RuntimeTaskRequestCodecLimits,
) -> Result<(), RuntimeTaskRequestCodecError> {
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|bytes| total.checked_add(bytes))
        .ok_or(RuntimeTaskRequestCodecError::Arithmetic)?;
    check("decoded bytes", bytes, limits.decoded_bytes)?;
    *total = bytes;
    Ok(())
}

fn mapping_storage_bytes<Next>(
    mut storage_bytes: usize,
    arguments: usize,
    fields: usize,
    limits: RuntimeTaskRequestCodecLimits,
) -> Result<usize, RuntimeTaskRequestCodecError> {
    charge_storage::<RuntimeRequestArgument<Next>>(&mut storage_bytes, arguments, limits)?;
    charge_storage::<RuntimeRequestField<Next>>(&mut storage_bytes, fields, limits)?;
    Ok(storage_bytes)
}

struct Writer<'a> {
    bytes: Option<&'a mut Vec<u8>>,
    meter: Option<&'a mut TaskSemanticMeter>,
    length: usize,
    storage_bytes: usize,
    limits: RuntimeTaskRequestCodecLimits,
}
impl Writer<'_> {
    fn put(&mut self, bytes: &[u8]) -> Result<(), RuntimeTaskRequestCodecError> {
        if let Some(meter) = &mut self.meter {
            meter
                .charge_work(bytes.len() as u64)
                .map_err(RuntimeTaskRequestCodecError::Resource)?;
        }
        let length = self
            .length
            .checked_add(bytes.len())
            .ok_or(RuntimeTaskRequestCodecError::Arithmetic)?;
        check("bytes", length, self.limits.encoded_bytes)?;
        if let Some(output) = &mut self.bytes {
            output.extend_from_slice(bytes);
        }
        self.length = length;
        Ok(())
    }
    fn u32(&mut self, value: u32) -> Result<(), RuntimeTaskRequestCodecError> {
        self.put(&value.to_le_bytes())
    }
    fn count(&mut self, count: usize) -> Result<(), RuntimeTaskRequestCodecError> {
        self.u32(u32::try_from(count).map_err(|_| RuntimeTaskRequestCodecError::Arithmetic)?)
    }
    fn ty(&mut self, ty: RuntimePlanTypeId) -> Result<(), RuntimeTaskRequestCodecError> {
        check(
            "type coordinate",
            ty.get().get() as usize,
            self.limits.type_count,
        )?;
        self.u32(ty.get().get())
    }
    fn path(
        &mut self,
        steps: &[RuntimeRequestPathStep],
    ) -> Result<(), RuntimeTaskRequestCodecError> {
        check("path steps", steps.len(), self.limits.path_steps as usize)?;
        charge_storage::<RuntimeRequestPathStep>(
            &mut self.storage_bytes,
            steps.len(),
            self.limits,
        )?;
        self.count(steps.len())?;
        for step in steps {
            step.write_wire(self)?;
        }
        Ok(())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
    limits: RuntimeTaskRequestCodecLimits,
    storage_bytes: usize,
    arguments: usize,
    fields: usize,
}
impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], RuntimeTaskRequestCodecError> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(RuntimeTaskRequestCodecError::Arithmetic)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(RuntimeTaskRequestCodecError::Truncated)?;
        self.offset = end;
        bytes
            .try_into()
            .map_err(|_| RuntimeTaskRequestCodecError::Truncated)
    }
    fn tag(&mut self) -> Result<u8, RuntimeTaskRequestCodecError> {
        Ok(self.take::<1>()?[0])
    }
    fn u32(&mut self) -> Result<u32, RuntimeTaskRequestCodecError> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn ordinal(&mut self, expected: usize) -> Result<(), RuntimeTaskRequestCodecError> {
        let expected =
            u32::try_from(expected).map_err(|_| RuntimeTaskRequestCodecError::Arithmetic)?;
        let actual = self.u32()?;
        if actual != expected {
            return Err(RuntimeTaskRequestCodecError::Ordinal { expected, actual });
        }
        Ok(())
    }
    fn identity(&mut self) -> Result<RuntimeRequestRoleIdentity, RuntimeTaskRequestCodecError> {
        Ok(RuntimeRequestRoleIdentity::from_accepted_identity(
            self.take()?,
        ))
    }
    fn ty(&mut self) -> Result<RuntimePlanTypeId, RuntimeTaskRequestCodecError> {
        let raw = NonZeroU32::new(self.u32()?).ok_or(RuntimeTaskRequestCodecError::ZeroType)?;
        check(
            "type coordinate",
            raw.get() as usize,
            self.limits.type_count,
        )?;
        Ok(RuntimePlanTypeId::from_accepted_ordinal(raw))
    }
    fn count(
        &mut self,
        kind: &'static str,
        maximum: usize,
        minimum_bytes: usize,
    ) -> Result<usize, RuntimeTaskRequestCodecError> {
        let count = self.u32()? as usize;
        check(kind, count, maximum)?;
        self.minimum_bytes(count, minimum_bytes)?;
        Ok(count)
    }
    fn minimum_bytes(
        &self,
        count: usize,
        minimum_bytes: usize,
    ) -> Result<(), RuntimeTaskRequestCodecError> {
        let minimum = count
            .checked_mul(minimum_bytes)
            .ok_or(RuntimeTaskRequestCodecError::Arithmetic)?;
        if minimum > self.bytes.len() - self.offset {
            return Err(RuntimeTaskRequestCodecError::Truncated);
        }
        Ok(())
    }
    fn path(
        &mut self,
        materialize: bool,
    ) -> Result<Option<Box<[RuntimeRequestPathStep]>>, RuntimeTaskRequestCodecError> {
        let count = self.count("path steps", self.limits.path_steps as usize, 1)?;
        charge_storage::<RuntimeRequestPathStep>(&mut self.storage_bytes, count, self.limits)?;
        let mut steps = materialize.then(|| Vec::with_capacity(count));
        for _ in 0..count {
            let step = RuntimeRequestPathStep::read_wire(self)?;
            if let Some(steps) = &mut steps {
                steps.push(step);
            }
        }
        Ok(steps.map(Vec::into_boxed_slice))
    }
}

impl RuntimeRequestArgumentRole {
    fn decode_tag(tag: u8) -> Result<Self, RuntimeTaskRequestCodecError> {
        Ok(match tag {
            0 => Self::Positional,
            1 => Self::Named,
            2 => Self::Spread,
            3 => Self::Capture,
            4 => Self::AwaitManyItem,
            5 => Self::TimeoutSource,
            6 => Self::TimeoutLimit,
            7 => Self::LineInput,
            _ => {
                return Err(RuntimeTaskRequestCodecError::Tag {
                    kind: "argument role",
                    tag,
                });
            }
        })
    }
}
impl RuntimeRequestValueSource {
    fn decode_tag(tag: u8) -> Result<Self, RuntimeTaskRequestCodecError> {
        Ok(match tag {
            0 => Self::Literal,
            1 => Self::Local,
            2 => Self::Capture,
            3 => Self::Projection,
            4 => Self::CallResult,
            5 => Self::AggregateItem,
            6 => Self::NeedHandle,
            _ => {
                return Err(RuntimeTaskRequestCodecError::Tag {
                    kind: "value source",
                    tag,
                });
            }
        })
    }
}
impl RuntimeRequestFieldRole {
    fn decode_tag(tag: u8) -> Result<Self, RuntimeTaskRequestCodecError> {
        Ok(match tag {
            0 => Self::Required,
            1 => Self::Optional,
            2 => Self::Repeated,
            3 => Self::NamedOnly,
            4 => Self::PositionalOnly,
            _ => {
                return Err(RuntimeTaskRequestCodecError::Tag {
                    kind: "field role",
                    tag,
                });
            }
        })
    }
}
impl RuntimeRequestPathStep {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), RuntimeTaskRequestCodecError> {
        writer.put(&[self.semantic_tag()])?;
        match self {
            Self::Operand(value)
            | Self::Tuple(value)
            | Self::CallArgument(value)
            | Self::SpreadArgument(value)
            | Self::Capture(value)
            | Self::LineChild(value) => writer.u32(*value),
            Self::Record(identity) | Self::Variant(identity) | Self::NamedArgument(identity) => {
                writer.put(identity.as_bytes())
            }
            Self::AwaitManyItem | Self::TimeoutSource | Self::TimeoutLimit => Ok(()),
        }
    }
    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, RuntimeTaskRequestCodecError> {
        Ok(match reader.tag()? {
            0 => Self::Operand(reader.u32()?),
            1 => Self::Tuple(reader.u32()?),
            2 => Self::Record(reader.identity()?),
            3 => Self::Variant(reader.identity()?),
            4 => Self::CallArgument(reader.u32()?),
            5 => Self::NamedArgument(reader.identity()?),
            6 => Self::SpreadArgument(reader.u32()?),
            7 => Self::Capture(reader.u32()?),
            8 => Self::AwaitManyItem,
            9 => Self::TimeoutSource,
            10 => Self::TimeoutLimit,
            11 => Self::LineChild(reader.u32()?),
            tag => {
                return Err(RuntimeTaskRequestCodecError::Tag {
                    kind: "path step",
                    tag,
                });
            }
        })
    }
}

#[cfg(test)]
mod tests;
