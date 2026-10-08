//! Private version-one projection of the static request definition. Decoded
//! roles are data; the actual F endpoint and type owner must recompute Q.

use super::{
    RuntimeRequestArgument, RuntimeRequestArgumentRole, RuntimeRequestField,
    RuntimeRequestFieldRole, RuntimeRequestPathStep, RuntimeRequestRoleIdentity,
    RuntimeRequestValueSource, RuntimeTaskRequestTemplate,
};
use crate::runtime_id::RuntimePlanTypeId;
use std::num::NonZeroU32;

#[derive(Clone, Copy)]
pub(crate) struct RuntimeTaskRequestCodecLimits {
    pub(crate) roles: u32,
    pub(crate) path_steps: u32,
    pub(crate) encoded_bytes: usize,
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum RuntimeTaskRequestCodecError {
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
}

impl RuntimeTaskRequestTemplate {
    pub(crate) fn encode(
        &self,
        limits: RuntimeTaskRequestCodecLimits,
    ) -> Result<Vec<u8>, RuntimeTaskRequestCodecError> {
        check_roles(self.arguments.len(), self.fields.len(), limits)?;
        let mut writer = Writer {
            bytes: Vec::new(),
            limits,
        };
        writer.put(&[1])?;
        writer.u32(self.endpoint)?;
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
            writer.u32(argument.ty.get().get())?;
            writer.put(&[argument.source.semantic_tag()])?;
            writer.path(&argument.path)?;
        }
        writer.count(self.fields.len())?;
        for (ordinal, field) in self.fields.iter().enumerate() {
            writer.count(ordinal)?;
            writer.put(field.identity.as_bytes())?;
            writer.put(&[field.role.semantic_tag()])?;
            writer.u32(field.ty.get().get())?;
            writer.path(&field.path)?;
        }
        Ok(writer.bytes)
    }

    pub(crate) fn decode(
        bytes: &[u8],
        limits: RuntimeTaskRequestCodecLimits,
    ) -> Result<Self, RuntimeTaskRequestCodecError> {
        check("bytes", bytes.len(), limits.encoded_bytes)?;
        let mut reader = Reader {
            bytes,
            offset: 0,
            limits,
        };
        let version = reader.tag()?;
        if version != 1 {
            return Err(RuntimeTaskRequestCodecError::Version { actual: version });
        }
        let endpoint = reader.u32()?;
        let count = reader.count("roles", limits.roles as usize, 15)?;
        let mut arguments = Vec::with_capacity(count);
        for ordinal in 0..count {
            reader.ordinal(ordinal)?;
            let role = RuntimeRequestArgumentRole::decode_tag(reader.tag()?)?;
            let identity = match reader.tag()? {
                0 => None,
                1 => Some(reader.identity()?),
                actual => return Err(RuntimeTaskRequestCodecError::Option { actual }),
            };
            let ty = reader.ty()?;
            let source = RuntimeRequestValueSource::decode_tag(reader.tag()?)?;
            arguments.push(RuntimeRequestArgument {
                role,
                identity,
                ty,
                source,
                path: reader.path()?,
            });
        }
        let fields_count = reader.u32()? as usize;
        check_roles(count, fields_count, limits)?;
        reader.minimum_bytes(fields_count, 45)?;
        let mut fields = Vec::with_capacity(fields_count);
        for ordinal in 0..fields_count {
            reader.ordinal(ordinal)?;
            let identity = reader.identity()?;
            let role = RuntimeRequestFieldRole::decode_tag(reader.tag()?)?;
            let ty = reader.ty()?;
            fields.push(RuntimeRequestField {
                identity,
                role,
                ty,
                path: reader.path()?,
            });
        }
        if reader.offset != bytes.len() {
            return Err(RuntimeTaskRequestCodecError::Trailing);
        }
        Ok(Self::new(
            endpoint,
            arguments.into_boxed_slice(),
            fields.into_boxed_slice(),
        ))
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

struct Writer {
    bytes: Vec<u8>,
    limits: RuntimeTaskRequestCodecLimits,
}
impl Writer {
    fn put(&mut self, bytes: &[u8]) -> Result<(), RuntimeTaskRequestCodecError> {
        let length = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or(RuntimeTaskRequestCodecError::Arithmetic)?;
        check("bytes", length, self.limits.encoded_bytes)?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn u32(&mut self, value: u32) -> Result<(), RuntimeTaskRequestCodecError> {
        self.put(&value.to_le_bytes())
    }
    fn count(&mut self, count: usize) -> Result<(), RuntimeTaskRequestCodecError> {
        self.u32(u32::try_from(count).map_err(|_| RuntimeTaskRequestCodecError::Arithmetic)?)
    }
    fn path(
        &mut self,
        steps: &[RuntimeRequestPathStep],
    ) -> Result<(), RuntimeTaskRequestCodecError> {
        check("path steps", steps.len(), self.limits.path_steps as usize)?;
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
    fn path(&mut self) -> Result<Box<[RuntimeRequestPathStep]>, RuntimeTaskRequestCodecError> {
        let count = self.count("path steps", self.limits.path_steps as usize, 1)?;
        (0..count)
            .map(|_| RuntimeRequestPathStep::read_wire(self))
            .collect()
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
