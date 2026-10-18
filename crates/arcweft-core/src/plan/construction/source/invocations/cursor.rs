//! Borrowed declaration headers. Runtime path validation belongs to its
//! original owner and preserves stored segments independently of selectors.

use super::{Counts, DOMAIN, Error, RuntimeInvocationSourceLimits, reference};
use crate::entry::{
    CallableContractHash, FlowContractHash, FlowParameterCoordinate, RuntimeCallableId,
    RuntimeCallableRole, RuntimeFlowExecutableParameter, RuntimeFlowParameterMode,
};
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    FlowRuntimeId, RuntimeFunctionParameterIdentity, RuntimeFunctionParameterPassing,
};
use crate::runtime_id::{RuntimeIdFamily, RuntimeIdPath};

pub(super) struct Flow<'a> {
    segments: &'a [u8],
    count: usize,
    label: &'a str,
}
impl Flow<'_> {
    pub fn materialize(
        &self,
        limits: RuntimeInvocationSourceLimits,
    ) -> Result<FlowRuntimeId, Error> {
        let mut reader = Cursor::new(self.segments, limits);
        let mut segments = Vec::with_capacity(self.count);
        for index in 0..self.count {
            let text = reader.string()?;
            segments.push(
                RuntimeIdPath::preflight_stored_source_segment(
                    RuntimeIdFamily::Flow,
                    index,
                    self.count,
                    text,
                )?
                .materialize(),
            );
        }
        reader.finish()?;
        Ok(FlowRuntimeId::from_source_path(
            RuntimeIdPath::from_source_segment_storage(RuntimeIdFamily::Flow, segments)?,
            self.label.to_owned(),
        ))
    }
}
pub(super) struct Role<'a> {
    callable: &'a str,
    contract: CallableContractHash,
}
impl Role<'_> {
    pub fn materialize(self) -> Result<RuntimeCallableRole, Error> {
        Ok(RuntimeCallableRole {
            callable: RuntimeCallableId::try_new(self.callable)?,
            contract: self.contract,
        })
    }
}
pub(super) struct Root<'a> {
    pub flow: Flow<'a>,
    pub function: u32,
    pub parameters: usize,
}
pub(super) struct Schema<'a> {
    pub flow: Flow<'a>,
    pub parameters: &'a [u8],
    pub parameter_count: usize,
}
pub(super) struct FlowExecutable<'a> {
    pub flow: Flow<'a>,
    pub contract: FlowContractHash,
    pub controller: Option<Role<'a>>,
}
pub(super) enum Code<'a> {
    Function(u32),
    Flow(Flow<'a>),
}
pub(super) struct Callable<'a> {
    pub role: Role<'a>,
    pub code: Code<'a>,
}
pub(super) struct Parameter<'a> {
    identity: RuntimeFunctionParameterIdentity,
    coordinate: FlowParameterCoordinate,
    name: &'a str,
    mode: RuntimeFlowParameterMode,
    passing: RuntimeFunctionParameterPassing,
    semantic_identity: RuntimeSemanticTypeId,
}
impl Parameter<'_> {
    pub fn materialize(self) -> RuntimeFlowExecutableParameter {
        RuntimeFlowExecutableParameter {
            identity: self.identity,
            coordinate: self.coordinate,
            name: self.name.to_owned(),
            mode: self.mode,
            passing: self.passing,
            semantic_identity: self.semantic_identity,
        }
    }
}
pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
    limits: RuntimeInvocationSourceLimits,
}
impl<'a> Cursor<'a> {
    pub fn new(bytes: &'a [u8], limits: RuntimeInvocationSourceLimits) -> Self {
        Self {
            bytes,
            offset: 0,
            limits,
        }
    }
    fn slice(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(length).ok_or(Error::Arithmetic)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(Error::Truncated {
            offset: self.offset,
        })?;
        self.offset = end;
        Ok(bytes)
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.slice(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.slice(4)?.try_into().expect("source u32 width"),
        ))
    }
    fn digest(&mut self) -> Result<[u8; 32], Error> {
        Ok(self.slice(32)?.try_into().expect("source identity width"))
    }
    fn string(&mut self) -> Result<&'a str, Error> {
        let n = self.u32()? as usize;
        Ok(std::str::from_utf8(self.slice(n)?)?)
    }
    fn text(&mut self, counts: &mut Counts) -> Result<&'a str, Error> {
        let text = self.string()?;
        counts.text(text.len())?;
        Ok(text)
    }
    fn flag(&mut self) -> Result<bool, Error> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(Error::Tag { kind: "flag", tag }),
        }
    }
    pub fn function(&mut self) -> Result<u32, Error> {
        let id = self.u32()?;
        reference("function", id, self.limits.function_count)?;
        Ok(id)
    }
    fn flow(&mut self, counts: &mut Counts) -> Result<Flow<'a>, Error> {
        let count = self.u32()? as usize;
        if count == 0 {
            return Err(crate::runtime_id::RuntimeIdError::Empty {
                family: RuntimeIdFamily::Flow,
            }
            .into());
        }
        counts.segments(count)?;
        let start = self.offset;
        for index in 0..count {
            let text = self.text(counts)?;
            RuntimeIdPath::preflight_stored_source_segment(
                RuntimeIdFamily::Flow,
                index,
                count,
                text,
            )?;
        }
        let segments = &self.bytes[start..self.offset];
        let label = self.text(counts)?;
        Ok(Flow {
            segments,
            count,
            label,
        })
    }
    fn role(&mut self, counts: &mut Counts) -> Result<Role<'a>, Error> {
        let callable = self.text(counts)?;
        RuntimeCallableId::validate_source(callable)?;
        Ok(Role {
            callable,
            contract: CallableContractHash::from_bytes(self.digest()?),
        })
    }
    pub fn header(&mut self, counts: &mut Counts) -> Result<usize, Error> {
        if self.slice(DOMAIN.len())? != DOMAIN {
            return Err(Error::Domain);
        }
        let actual = self.byte()?;
        if actual != 1 {
            return Err(Error::Version { actual });
        }
        let n = self.u32()? as usize;
        counts.roots(n)?;
        Ok(n)
    }
    pub fn root(&mut self, counts: &mut Counts) -> Result<Root<'a>, Error> {
        let flow = self.flow(counts)?;
        let function = self.function()?;
        let parameters = self.u32()? as usize;
        counts.parameters(parameters)?;
        Ok(Root {
            flow,
            function,
            parameters,
        })
    }
    pub fn schema_count(&mut self, counts: &mut Counts) -> Result<usize, Error> {
        let n = self.u32()? as usize;
        counts.schemas(n)?;
        Ok(n)
    }
    pub fn schema(&mut self, counts: &mut Counts) -> Result<Schema<'a>, Error> {
        let flow = self.flow(counts)?;
        let parameter_count = self.u32()? as usize;
        counts.schema_parameters(parameter_count)?;
        let start = self.offset;
        for _ in 0..parameter_count {
            let parameter = self.parameter()?;
            counts.text(parameter.name.len())?;
        }
        Ok(Schema {
            flow,
            parameter_count,
            parameters: &self.bytes[start..self.offset],
        })
    }
    pub fn parameter(&mut self) -> Result<Parameter<'a>, Error> {
        let identity = RuntimeFunctionParameterIdentity::from_accepted_identity(self.digest()?);
        let coordinate = FlowParameterCoordinate::from_position(self.u32()?);
        let name = self.string()?;
        let mode = match self.byte()? {
            0 => RuntimeFlowParameterMode::Owned,
            1 => RuntimeFlowParameterMode::Shared,
            2 => RuntimeFlowParameterMode::Mutable,
            tag => {
                return Err(Error::Tag {
                    kind: "Flow parameter mode",
                    tag,
                });
            }
        };
        let tag = self.byte()?;
        let passing =
            RuntimeFunctionParameterPassing::from_semantic_tag(tag).ok_or(Error::Tag {
                kind: "parameter passing",
                tag,
            })?;
        let semantic_identity = RuntimeSemanticTypeId::from_bytes(self.digest()?);
        Ok(Parameter {
            identity,
            coordinate,
            name,
            mode,
            passing,
            semantic_identity,
        })
    }
    pub fn flow_executable_count(&mut self, counts: &mut Counts) -> Result<usize, Error> {
        let n = self.u32()? as usize;
        counts.flow_executables(n)?;
        Ok(n)
    }
    pub fn flow_executable(&mut self, counts: &mut Counts) -> Result<FlowExecutable<'a>, Error> {
        let flow = self.flow(counts)?;
        let contract = FlowContractHash::from_bytes(self.digest()?);
        let controller = if self.flag()? {
            Some(self.role(counts)?)
        } else {
            None
        };
        Ok(FlowExecutable {
            flow,
            contract,
            controller,
        })
    }
    pub fn callable_count(&mut self, counts: &mut Counts) -> Result<usize, Error> {
        let n = self.u32()? as usize;
        counts.callables(n)?;
        Ok(n)
    }
    pub fn callable(&mut self, counts: &mut Counts) -> Result<Callable<'a>, Error> {
        let role = self.role(counts)?;
        let code = match self.byte()? {
            1 => Code::Function(self.function()?),
            2 => Code::Flow(self.flow(counts)?),
            tag => {
                return Err(Error::Tag {
                    kind: "callable executable",
                    tag,
                });
            }
        };
        Ok(Callable { role, code })
    }
    pub fn defer_count(&mut self, counts: &mut Counts) -> Result<usize, Error> {
        let n = self.u32()? as usize;
        counts.defers(n)?;
        Ok(n)
    }
    pub fn finish(&self) -> Result<(), Error> {
        if self.offset != self.bytes.len() {
            return Err(Error::Trailing {
                offset: self.offset,
            });
        }
        Ok(())
    }
}
