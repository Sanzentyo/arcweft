//! Borrowed Entry rows retain exact path segments and schema extents.

use super::{
    Counts, DOMAIN, Error, Footprint, RuntimeEntrySourceLimits, RuntimePlanSourceConstruction,
};
use crate::entry::schema::source::PreparedRuntimeSchemaSource;
use crate::entry::{
    AgentBudget, AgentPolicyHash, CallableContractHash, EntryBindingIdentity, FlowContractHash,
    FlowParameterCoordinate, RootExecutionLimits, RuntimeAgentEntryRoles, RuntimeCallableId,
    RuntimeCallableRole, RuntimeCommandConstructorId, RuntimeCommandContract, RuntimeCommandPolicy,
    RuntimeCommandTargetId, RuntimeEntryRoles, RuntimeFlowRole, RuntimeNominalRole,
    RuntimeNominalTypeId, RuntimeSchemaLimits, RuntimeStatefulEntryRoles, TypeLayoutHash,
};
use crate::plan::{
    EntryRuntimeId, FlowRuntimeId, RouteCaptureCoordinate, RuntimeEntryKind, RuntimeEntrySpec,
    RuntimeEntryTarget, RuntimeHttpMethod, RuntimeRouteBinding, RuntimeRouteBindingSource,
    RuntimeRoutePath, RuntimeRoutePathSegment, RuntimeRouteSpec,
};
use crate::runtime_id::{RuntimeIdFamily, RuntimeIdPath, RuntimeIdSegment};
use crate::task::semantic::TaskSemanticMeter;

struct Path<'a> {
    family: RuntimeIdFamily,
    bytes: &'a [u8],
    count: usize,
    label: Option<&'a str>,
}
impl Path<'_> {
    fn materialize(&self) -> Result<RuntimeIdPath, Error> {
        let mut reader = Reader::new(self.bytes);
        let mut segments = Vec::with_capacity(self.count);
        for index in 0..self.count {
            segments.push(
                RuntimeIdPath::preflight_stored_source_segment(
                    self.family,
                    index,
                    self.count,
                    reader.string()?,
                )?
                .materialize(),
            );
        }
        reader.finish()?;
        Ok(RuntimeIdPath::from_source_segment_storage(
            self.family,
            segments,
        )?)
    }
    fn flow(self) -> Result<FlowRuntimeId, Error> {
        Ok(FlowRuntimeId::from_source_path(
            self.materialize()?,
            self.label
                .expect("Flow source retains its label")
                .to_owned(),
        ))
    }
}
enum Kind<'a> {
    Known(RuntimeEntryKind),
    Custom(&'a str),
}
enum Target<'a> {
    Flow(Path<'a>),
    Controller(Path<'a>),
    Routes {
        bytes: &'a [u8],
        count: usize,
        prefix: Footprint,
    },
}
struct Callable<'a> {
    identity: &'a str,
    contract: CallableContractHash,
}
impl Callable<'_> {
    fn materialize(self) -> Result<RuntimeCallableRole, Error> {
        Ok(RuntimeCallableRole {
            callable: RuntimeCallableId::try_new(self.identity)?,
            contract: self.contract,
        })
    }
}
struct Nominal<'a> {
    identity: &'a str,
    semantic: crate::pattern::RuntimeSemanticTypeId,
    layout: TypeLayoutHash,
}
impl Nominal<'_> {
    fn materialize(self) -> Result<RuntimeNominalRole, Error> {
        Ok(RuntimeNominalRole {
            identity: RuntimeNominalTypeId::try_new(self.identity)?,
            semantic_identity: self.semantic,
            layout: self.layout,
        })
    }
}
struct Stateful<'a> {
    binding: EntryBindingIdentity,
    state: Nominal<'a>,
    event: Nominal<'a>,
    initializer: Callable<'a>,
    reducer: Callable<'a>,
    flow: Path<'a>,
    contract: FlowContractHash,
    commands: &'a [u8],
    count: usize,
    prefix: Footprint,
    limits: RootExecutionLimits,
}
enum Roles<'a> {
    None,
    Stateful {
        bytes: &'a [u8],
        prefix: Footprint,
    },
    Agent {
        binding: EntryBindingIdentity,
        controller: Callable<'a>,
        policy: AgentPolicyHash,
        budget: AgentBudget,
    },
}
struct Entry<'a> {
    id: Path<'a>,
    kind: Kind<'a>,
    binding: EntryBindingIdentity,
    target: Target<'a>,
    roles: Roles<'a>,
}
struct Route<'a> {
    method: RuntimeHttpMethod,
    segments: &'a [u8],
    count: usize,
    target: Path<'a>,
    bindings: &'a [u8],
    binding_count: usize,
}
struct Command<'a> {
    constructor: &'a str,
    target: &'a str,
    layout: TypeLayoutHash,
    schema: PreparedRuntimeSchemaSource<'a>,
}

pub(super) fn inspect(
    bytes: &[u8],
    limits: RuntimeEntrySourceLimits,
    meter: &mut TaskSemanticMeter,
) -> Result<Footprint, Error> {
    let mut reader = Reader::new(bytes);
    let mut counts = Counts::new(limits);
    let count = reader.header(&mut counts)?;
    for _ in 0..count {
        reader.entry(&mut counts, meter)?;
    }
    reader.finish()?;
    Ok(counts.footprint)
}
pub(super) fn materialize(
    bytes: &[u8],
    limits: RuntimeEntrySourceLimits,
    source: &mut RuntimePlanSourceConstruction,
) -> Result<(), Error> {
    let mut reader = Reader::new(bytes);
    let mut counts = Counts::new(limits);
    let count = reader.header(&mut counts)?;
    for _ in 0..count {
        let row = reader.entry(&mut counts, &mut source.meter)?;
        let entry = row.materialize(limits, &mut source.meter)?;
        source.builder.push_entry(entry)?;
    }
    reader.finish()
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn raw(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(count).ok_or(Error::Arithmetic)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(Error::Truncated {
            offset: self.offset,
        })?;
        self.offset = end;
        Ok(bytes)
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.raw(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.raw(4)?.try_into().expect("source u32 width"),
        ))
    }
    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(
            self.raw(8)?.try_into().expect("source u64 width"),
        ))
    }
    fn digest(&mut self) -> Result<[u8; 32], Error> {
        Ok(self.raw(32)?.try_into().expect("source digest width"))
    }
    fn string(&mut self) -> Result<&'a str, Error> {
        let count = self.u32()? as usize;
        Ok(std::str::from_utf8(self.raw(count)?)?)
    }
    fn text(&mut self, counts: &mut Counts) -> Result<&'a str, Error> {
        let text = self.string()?;
        counts.text(text)?;
        Ok(text)
    }
    fn finish(&self) -> Result<(), Error> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(Error::Trailing {
                offset: self.offset,
            })
        }
    }
    fn header(&mut self, counts: &mut Counts) -> Result<usize, Error> {
        if self.raw(DOMAIN.len())? != DOMAIN {
            return Err(Error::Domain);
        }
        let actual = self.byte()?;
        if actual != 1 {
            return Err(Error::Version { actual });
        }
        let count = self.u32()? as usize;
        counts.entries(count)?;
        Ok(count)
    }
    fn path(&mut self, family: RuntimeIdFamily, counts: &mut Counts) -> Result<Path<'a>, Error> {
        let count = self.u32()? as usize;
        if count == 0 {
            return Err(crate::runtime_id::RuntimeIdError::Empty { family }.into());
        }
        counts.array::<RuntimeIdSegment>(count)?;
        let start = self.offset;
        for index in 0..count {
            RuntimeIdPath::preflight_stored_source_segment(
                family,
                index,
                count,
                self.text(counts)?,
            )?;
        }
        let bytes = &self.bytes[start..self.offset];
        let label = if family == RuntimeIdFamily::Flow {
            Some(self.text(counts)?)
        } else {
            None
        };
        Ok(Path {
            family,
            bytes,
            count,
            label,
        })
    }
    fn callable(&mut self, counts: &mut Counts) -> Result<Callable<'a>, Error> {
        let identity = self.text(counts)?;
        RuntimeCallableId::validate_source(identity)?;
        Ok(Callable {
            identity,
            contract: CallableContractHash::from_bytes(self.digest()?),
        })
    }
    fn nominal(&mut self, counts: &mut Counts) -> Result<Nominal<'a>, Error> {
        let identity = self.text(counts)?;
        RuntimeNominalTypeId::validate_source(identity)?;
        Ok(Nominal {
            identity,
            semantic: crate::pattern::RuntimeSemanticTypeId::from_bytes(self.digest()?),
            layout: TypeLayoutHash::from_bytes(self.digest()?),
        })
    }
    fn entry(
        &mut self,
        counts: &mut Counts,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Entry<'a>, Error> {
        let id = self.path(RuntimeIdFamily::Entry, counts)?;
        let kind = match self.byte()? {
            0 => Kind::Known(RuntimeEntryKind::Game),
            1 => Kind::Known(RuntimeEntryKind::Editor),
            2 => Kind::Known(RuntimeEntryKind::Cli),
            3 => Kind::Known(RuntimeEntryKind::Server),
            4 => Kind::Known(RuntimeEntryKind::Activity),
            5 => Kind::Known(RuntimeEntryKind::Test),
            6 => Kind::Known(RuntimeEntryKind::Bench),
            7 => Kind::Known(RuntimeEntryKind::Agent),
            8 => Kind::Custom(self.text(counts)?),
            tag => {
                return Err(Error::Tag {
                    kind: "Entry kind",
                    tag,
                });
            }
        };
        let binding = EntryBindingIdentity::from_bytes(self.digest()?);
        let target = match self.byte()? {
            0 => Target::Flow(self.path(RuntimeIdFamily::Flow, counts)?),
            1 => {
                let count = self.u32()? as usize;
                counts.routes(count)?;
                let prefix = counts.footprint;
                let start = self.offset;
                for _ in 0..count {
                    self.route(counts)?;
                }
                Target::Routes {
                    bytes: &self.bytes[start..self.offset],
                    count,
                    prefix,
                }
            }
            2 => Target::Controller(self.path(RuntimeIdFamily::Flow, counts)?),
            tag => {
                return Err(Error::Tag {
                    kind: "Entry target",
                    tag,
                });
            }
        };
        let roles = match self.byte()? {
            0 => Roles::None,
            1 => {
                let prefix = counts.footprint;
                let start = self.offset;
                self.stateful(counts, meter)?;
                Roles::Stateful {
                    bytes: &self.bytes[start..self.offset],
                    prefix,
                }
            }
            2 => {
                counts.storage(size_of::<RuntimeAgentEntryRoles>())?;
                Roles::Agent {
                    binding: EntryBindingIdentity::from_bytes(self.digest()?),
                    controller: self.callable(counts)?,
                    policy: AgentPolicyHash::from_bytes(self.digest()?),
                    budget: self.agent_budget()?,
                }
            }
            tag => {
                return Err(Error::Tag {
                    kind: "Entry roles",
                    tag,
                });
            }
        };
        Ok(Entry {
            id,
            kind,
            binding,
            target,
            roles,
        })
    }
    fn stateful(
        &mut self,
        counts: &mut Counts,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Stateful<'a>, Error> {
        counts.storage(size_of::<RuntimeStatefulEntryRoles>())?;
        let binding = EntryBindingIdentity::from_bytes(self.digest()?);
        let state = self.nominal(counts)?;
        let event = self.nominal(counts)?;
        let initializer = self.callable(counts)?;
        let reducer = self.callable(counts)?;
        let flow = self.path(RuntimeIdFamily::Flow, counts)?;
        let contract = FlowContractHash::from_bytes(self.digest()?);
        let count = self.u32()? as usize;
        counts.commands(count)?;
        let prefix = counts.footprint;
        let start = self.offset;
        for _ in 0..count {
            self.command(counts, meter)?;
        }
        let commands = &self.bytes[start..self.offset];
        let limits = self.root_limits()?;
        Ok(Stateful {
            binding,
            state,
            event,
            initializer,
            reducer,
            flow,
            contract,
            commands,
            count,
            prefix,
            limits,
        })
    }
    fn route(&mut self, counts: &mut Counts) -> Result<Route<'a>, Error> {
        let method = match self.byte()? {
            0 => RuntimeHttpMethod::Get,
            1 => RuntimeHttpMethod::Post,
            2 => RuntimeHttpMethod::Put,
            3 => RuntimeHttpMethod::Patch,
            4 => RuntimeHttpMethod::Delete,
            5 => RuntimeHttpMethod::Head,
            6 => RuntimeHttpMethod::Options,
            tag => {
                return Err(Error::Tag {
                    kind: "HTTP method",
                    tag,
                });
            }
        };
        let count = self.u32()? as usize;
        counts.array::<RuntimeRoutePathSegment>(count)?;
        let start = self.offset;
        let mut captures = 0usize;
        for _ in 0..count {
            match self.byte()? {
                0 => RuntimeRoutePath::validate_source_literal(self.text(counts)?)?,
                1 => {
                    let capture = RouteCaptureCoordinate::from_position(self.u32()?);
                    RuntimeRoutePath::validate_source_capture(capture, captures)?;
                    captures = captures.checked_add(1).ok_or(Error::Arithmetic)?;
                }
                tag => {
                    return Err(Error::Tag {
                        kind: "route segment",
                        tag,
                    });
                }
            }
        }
        let segments = &self.bytes[start..self.offset];
        let target = self.path(RuntimeIdFamily::Flow, counts)?;
        let binding_count = self.u32()? as usize;
        counts.array::<RuntimeRouteBinding>(binding_count)?;
        let start = self.offset;
        for _ in 0..binding_count {
            self.route_binding()?;
        }
        Ok(Route {
            method,
            segments,
            count,
            target,
            bindings: &self.bytes[start..self.offset],
            binding_count,
        })
    }
    fn route_binding(&mut self) -> Result<RuntimeRouteBinding, Error> {
        let parameter = FlowParameterCoordinate::from_position(self.u32()?);
        let source = match self.byte()? {
            0 => RuntimeRouteBindingSource::PathCapture(RouteCaptureCoordinate::from_position(
                self.u32()?,
            )),
            tag => {
                return Err(Error::Tag {
                    kind: "route binding",
                    tag,
                });
            }
        };
        Ok(RuntimeRouteBinding { parameter, source })
    }
    fn command(
        &mut self,
        counts: &mut Counts,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Command<'a>, Error> {
        let constructor = self.text(counts)?;
        RuntimeCommandConstructorId::validate_source(constructor)?;
        let target = self.text(counts)?;
        RuntimeCommandTargetId::validate_source(target)?;
        let layout = TypeLayoutHash::from_bytes(self.digest()?);
        let length = self.u32()? as usize;
        let bytes = self.raw(length)?;
        let schema = PreparedRuntimeSchemaSource::preflight(bytes, counts.schema_limits()?, meter)?;
        counts.schema(schema.nodes(), schema.elements(), schema.storage_bytes())?;
        Ok(Command {
            constructor,
            target,
            layout,
            schema,
        })
    }
    fn root_limits(&mut self) -> Result<RootExecutionLimits, Error> {
        Ok(RootExecutionLimits {
            schema: RuntimeSchemaLimits {
                max_depth: self.u32()?,
                max_nodes: self.u32()?,
                max_sequence_items: self.u32()?,
                max_string_bytes: self.u64()?,
                max_encoded_bytes: self.u64()?,
                max_validation_work: self.u64()?,
            },
            max_commands_per_transition: self.u32()?,
            max_command_bytes_per_transition: self.u64()?,
            max_pending_events: self.u32()?,
            max_pending_commands: self.u32()?,
        })
    }
    fn agent_budget(&mut self) -> Result<AgentBudget, Error> {
        Ok(AgentBudget {
            logical_timeout_millis: self.u64()?,
            max_vm_steps: self.u64()?,
            max_host_calls: self.u32()?,
            max_observations: self.u32()?,
            max_captures: self.u32()?,
            max_capture_bytes: self.u64()?,
            max_rag_queries: self.u32()?,
            max_context_bytes: self.u64()?,
        })
    }
}

impl Entry<'_> {
    fn materialize(
        self,
        limits: RuntimeEntrySourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeEntrySpec, Error> {
        let id = EntryRuntimeId::from_source_path(self.id.materialize()?);
        let kind = match self.kind {
            Kind::Known(kind) => kind,
            Kind::Custom(name) => RuntimeEntryKind::Custom(name.to_owned()),
        };
        let target = match self.target {
            Target::Flow(flow) => RuntimeEntryTarget::Flow(flow.flow()?),
            Target::Controller(flow) => RuntimeEntryTarget::Controller(flow.flow()?),
            Target::Routes {
                bytes,
                count,
                prefix,
            } => {
                let mut reader = Reader::new(bytes);
                let mut counts = Counts {
                    limits,
                    footprint: prefix,
                };
                let mut routes = Vec::with_capacity(count);
                for _ in 0..count {
                    routes.push(reader.route(&mut counts)?.materialize()?);
                }
                reader.finish()?;
                RuntimeEntryTarget::Routes(routes)
            }
        };
        let roles = match self.roles {
            Roles::None => RuntimeEntryRoles::None,
            Roles::Agent {
                binding,
                controller,
                policy,
                budget,
            } => RuntimeEntryRoles::Agent(Box::new(RuntimeAgentEntryRoles {
                binding,
                controller: controller.materialize()?,
                policy,
                budget,
            })),
            Roles::Stateful { bytes, prefix } => {
                let mut role_reader = Reader::new(bytes);
                let mut role_counts = Counts {
                    limits,
                    footprint: prefix,
                };
                let role = role_reader.stateful(&mut role_counts, meter)?;
                role_reader.finish()?;
                let mut reader = Reader::new(role.commands);
                let mut counts = Counts {
                    limits,
                    footprint: role.prefix,
                };
                let mut admitted = Vec::with_capacity(role.count);
                for _ in 0..role.count {
                    let command = reader.command(&mut counts, meter)?;
                    admitted.push(RuntimeCommandContract {
                        constructor: RuntimeCommandConstructorId::try_new(command.constructor)?,
                        target: RuntimeCommandTargetId::try_new(command.target)?,
                        payload_layout: command.layout,
                        payload_schema: command.schema.materialize(meter)?,
                    });
                }
                reader.finish()?;
                RuntimeEntryRoles::Stateful(Box::new(RuntimeStatefulEntryRoles {
                    binding: role.binding,
                    state: role.state.materialize()?,
                    event: role.event.materialize()?,
                    initializer: role.initializer.materialize()?,
                    reducer: role.reducer.materialize()?,
                    initial_flow: RuntimeFlowRole {
                        flow: role.flow.flow()?,
                        contract: role.contract,
                    },
                    command_policy: RuntimeCommandPolicy::new(admitted, role.limits),
                }))
            }
        };
        Ok(RuntimeEntrySpec {
            id,
            kind,
            binding: self.binding,
            target,
            roles,
        })
    }
}
impl Route<'_> {
    fn materialize(self) -> Result<RuntimeRouteSpec, Error> {
        let mut reader = Reader::new(self.segments);
        let mut segments = Vec::with_capacity(self.count);
        for _ in 0..self.count {
            segments.push(match reader.byte()? {
                0 => RuntimeRoutePathSegment::Literal(reader.string()?.to_owned()),
                1 => RuntimeRoutePathSegment::Capture(RouteCaptureCoordinate::from_position(
                    reader.u32()?,
                )),
                _ => unreachable!("preflighted route segment"),
            });
        }
        reader.finish()?;
        let path = RuntimeRoutePath::try_new(segments)?;
        let target = self.target.flow()?;
        let mut reader = Reader::new(self.bindings);
        let mut bindings = Vec::with_capacity(self.binding_count);
        for _ in 0..self.binding_count {
            bindings.push(reader.route_binding()?);
        }
        reader.finish()?;
        Ok(RuntimeRouteSpec {
            method: self.method,
            path,
            target,
            bindings,
        })
    }
}
