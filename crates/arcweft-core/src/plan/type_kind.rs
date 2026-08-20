//! Canonical runtime-plan type projection graph.

use crate::entry::{RuntimeNominalTypeId, TypeLayoutHash};
use crate::pattern::{RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeProducerId};
use crate::value::{RuntimeSignedIntWidth, RuntimeUnsignedIntWidth};
use serde::{Deserialize, Serialize};

/// Closed top-level runtime families owned by the Agent Prelude.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeAgentOperationalType {
    DebugStatePath,
    ObservationFieldPath,
    Probe,
    Predicate,
    Observation,
    ObservedObject,
    BoundingBox,
    ActionName,
    ActionTarget,
    ActionResult,
    AgentValue,
    DataFormat,
    DataShape,
    EntityMetadata,
    SourceAnchor,
    ProjectGraphNeighborhood,
    ProjectGraphSymbol,
    ProjectGraphEdge,
    CaptureTarget,
    CaptureReference,
    Resource,
    ResourceBody,
    RagContextPack,
    ObservedObjectId,
    CaptureFormat,
    CaptureKind,
    Diagnostics,
    WaitError,
    ViewportPoint,
    PointerButton,
    RagError,
}

impl RuntimeAgentOperationalType {
    /// Returns whether this semantic Agent family uses the closed protocol
    /// record carrier at host/runtime boundaries.
    #[must_use]
    pub const fn accepts_protocol_record(self) -> bool {
        matches!(
            self,
            Self::Observation
                | Self::ObservedObject
                | Self::BoundingBox
                | Self::ActionResult
                | Self::EntityMetadata
                | Self::SourceAnchor
                | Self::ProjectGraphNeighborhood
                | Self::ProjectGraphSymbol
                | Self::ProjectGraphEdge
                | Self::CaptureReference
                | Self::Resource
                | Self::ResourceBody
                | Self::RagContextPack
                | Self::Diagnostics
                | Self::WaitError
                | Self::RagError
        )
    }
}

/// Top-level execution family derived from a complete type projection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeOperationalType {
    Range,
    Iterator,
    Sequence,
    Tuple,
    Choice,
    Result,
    Option,
    Map,
    Need,
    Stream,
    ThreadHandle,
    Shared,
    Reference,
    Function,
    Agent(RuntimeAgentOperationalType),
}

/// Derived execution class. The checked predicate is resolved separately from
/// the owning table and is never copied into a declaration row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimePlanTypeClass {
    Checked,
    Operational(RuntimeOperationalType),
}

/// Accepted source sequence family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimePlanSequenceKind {
    Vec,
    Array,
    Slice,
    Seq,
}

/// The single runtime-plan type algebra, parameterized by its child-reference
/// domain.
///
/// Pre-admission rows use semantic identities and sealed rows use plan-local
/// type IDs. Keeping one algebra prevents seed and executable projections from
/// acquiring different shapes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimePlanTypeProjection<R> {
    Never,
    Unit,
    Bool,
    Signed(RuntimeSignedIntWidth),
    Unsigned(RuntimeUnsignedIntWidth),
    F32,
    F64,
    String,
    Char,
    Bytes,
    Duration,
    Progress,
    EntityReference,
    Range(R),
    Iterator(R),
    Sequence {
        kind: RuntimePlanSequenceKind,
        item: R,
    },
    Array {
        item: R,
        length: u64,
    },
    Map {
        key: R,
        value: R,
    },
    Need(R),
    Stream {
        item: R,
        error: R,
    },
    Result {
        value: R,
        error: R,
    },
    Option(R),
    ThreadHandle(R),
    Shared(R),
    Reference(R),
    Function {
        parameters: Box<[R]>,
        result: R,
    },
    ProjectNominal {
        nominal: RuntimeNominalTypeId,
        layout: TypeLayoutHash,
        arguments: Box<[R]>,
    },
    Tuple(Box<[R]>),
    Choice(Box<[R]>),
    Opaque {
        producer: RuntimeOpaqueTypeProducerId,
        admission: RuntimeOpaqueTypeAdmission,
        arguments: Box<[R]>,
    },
    Agent(RuntimeAgentTypeProjection<R>),
}

/// Agent-owned projection with the generic `Probe<T>` descendant preserved.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeAgentTypeProjection<R> {
    DebugStatePath,
    ObservationFieldPath,
    Probe(R),
    Predicate,
    Observation,
    ObservedObject,
    BoundingBox,
    ActionName,
    ActionTarget,
    ActionResult,
    AgentValue,
    DataFormat,
    DataShape,
    EntityMetadata,
    SourceAnchor,
    ProjectGraphNeighborhood,
    ProjectGraphSymbol,
    ProjectGraphEdge,
    CaptureTarget,
    CaptureReference,
    Resource,
    ResourceBody,
    RagContextPack,
    ObservedObjectId,
    CaptureFormat,
    CaptureKind,
    Diagnostics,
    WaitError,
    ViewportPoint,
    PointerButton,
    RagError,
}

impl<R> RuntimePlanTypeProjection<R> {
    /// Child references in canonical declaration order.
    pub fn children(&self) -> Box<[&R]> {
        match self {
            Self::Range(child)
            | Self::Iterator(child)
            | Self::Option(child)
            | Self::ThreadHandle(child)
            | Self::Shared(child)
            | Self::Reference(child)
            | Self::Need(child)
            | Self::Sequence { item: child, .. }
            | Self::Array { item: child, .. }
            | Self::Agent(RuntimeAgentTypeProjection::Probe(child)) => Box::new([child]),
            Self::Map { key, value }
            | Self::Stream {
                item: key,
                error: value,
            }
            | Self::Result {
                value: key,
                error: value,
            } => Box::new([key, value]),
            Self::Function { parameters, result } => parameters
                .iter()
                .chain(std::iter::once(result))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            Self::ProjectNominal { arguments, .. }
            | Self::Opaque { arguments, .. }
            | Self::Tuple(arguments)
            | Self::Choice(arguments) => arguments.iter().collect::<Vec<_>>().into_boxed_slice(),
            Self::Never
            | Self::Unit
            | Self::Bool
            | Self::Signed(_)
            | Self::Unsigned(_)
            | Self::F32
            | Self::F64
            | Self::String
            | Self::Char
            | Self::Bytes
            | Self::Duration
            | Self::Progress
            | Self::EntityReference
            | Self::Agent(_) => Box::new([]),
        }
    }

    /// Rewrites every child reference while retaining the exact projection.
    pub fn try_map<T, E>(
        self,
        mut map: impl FnMut(R) -> Result<T, E>,
    ) -> Result<RuntimePlanTypeProjection<T>, E> {
        Ok(match self {
            Self::Never => RuntimePlanTypeProjection::Never,
            Self::Unit => RuntimePlanTypeProjection::Unit,
            Self::Bool => RuntimePlanTypeProjection::Bool,
            Self::Signed(width) => RuntimePlanTypeProjection::Signed(width),
            Self::Unsigned(width) => RuntimePlanTypeProjection::Unsigned(width),
            Self::F32 => RuntimePlanTypeProjection::F32,
            Self::F64 => RuntimePlanTypeProjection::F64,
            Self::String => RuntimePlanTypeProjection::String,
            Self::Char => RuntimePlanTypeProjection::Char,
            Self::Bytes => RuntimePlanTypeProjection::Bytes,
            Self::Duration => RuntimePlanTypeProjection::Duration,
            Self::Progress => RuntimePlanTypeProjection::Progress,
            Self::EntityReference => RuntimePlanTypeProjection::EntityReference,
            Self::Range(child) => RuntimePlanTypeProjection::Range(map(child)?),
            Self::Iterator(child) => RuntimePlanTypeProjection::Iterator(map(child)?),
            Self::Sequence { kind, item } => RuntimePlanTypeProjection::Sequence {
                kind,
                item: map(item)?,
            },
            Self::Array { item, length } => RuntimePlanTypeProjection::Array {
                item: map(item)?,
                length,
            },
            Self::Map { key, value } => RuntimePlanTypeProjection::Map {
                key: map(key)?,
                value: map(value)?,
            },
            Self::Need(item) => RuntimePlanTypeProjection::Need(map(item)?),
            Self::Stream { item, error } => RuntimePlanTypeProjection::Stream {
                item: map(item)?,
                error: map(error)?,
            },
            Self::Result { value, error } => RuntimePlanTypeProjection::Result {
                value: map(value)?,
                error: map(error)?,
            },
            Self::Option(child) => RuntimePlanTypeProjection::Option(map(child)?),
            Self::ThreadHandle(child) => RuntimePlanTypeProjection::ThreadHandle(map(child)?),
            Self::Shared(child) => RuntimePlanTypeProjection::Shared(map(child)?),
            Self::Reference(child) => RuntimePlanTypeProjection::Reference(map(child)?),
            Self::Function { parameters, result } => RuntimePlanTypeProjection::Function {
                parameters: try_map_boxed(parameters, &mut map)?,
                result: map(result)?,
            },
            Self::ProjectNominal {
                nominal,
                layout,
                arguments,
            } => RuntimePlanTypeProjection::ProjectNominal {
                nominal,
                layout,
                arguments: try_map_boxed(arguments, &mut map)?,
            },
            Self::Tuple(items) => RuntimePlanTypeProjection::Tuple(try_map_boxed(items, &mut map)?),
            Self::Choice(items) => {
                RuntimePlanTypeProjection::Choice(try_map_boxed(items, &mut map)?)
            }
            Self::Opaque {
                producer,
                admission,
                arguments,
            } => RuntimePlanTypeProjection::Opaque {
                producer,
                admission,
                arguments: try_map_boxed(arguments, &mut map)?,
            },
            Self::Agent(agent) => RuntimePlanTypeProjection::Agent(agent.try_map(map)?),
        })
    }

    /// Operational family selected when the complete checked projection is
    /// unavailable.
    #[must_use]
    pub const fn operational_type(&self) -> Option<RuntimeOperationalType> {
        match self {
            Self::Range(_) => Some(RuntimeOperationalType::Range),
            Self::Iterator(_) => Some(RuntimeOperationalType::Iterator),
            Self::Sequence { .. } | Self::Array { .. } => Some(RuntimeOperationalType::Sequence),
            Self::Tuple(_) => Some(RuntimeOperationalType::Tuple),
            Self::Choice(_) => Some(RuntimeOperationalType::Choice),
            Self::Result { .. } => Some(RuntimeOperationalType::Result),
            Self::Option(_) => Some(RuntimeOperationalType::Option),
            Self::Map { .. } => Some(RuntimeOperationalType::Map),
            Self::Need { .. } => Some(RuntimeOperationalType::Need),
            Self::Stream { .. } => Some(RuntimeOperationalType::Stream),
            Self::ThreadHandle(_) => Some(RuntimeOperationalType::ThreadHandle),
            Self::Shared(_) => Some(RuntimeOperationalType::Shared),
            Self::Reference(_) => Some(RuntimeOperationalType::Reference),
            Self::Function { .. } => Some(RuntimeOperationalType::Function),
            Self::Agent(agent) => Some(RuntimeOperationalType::Agent(agent.operational_type())),
            Self::Never
            | Self::Unit
            | Self::Bool
            | Self::Signed(_)
            | Self::Unsigned(_)
            | Self::F32
            | Self::F64
            | Self::String
            | Self::Char
            | Self::Bytes
            | Self::Duration
            | Self::Progress
            | Self::EntityReference
            | Self::ProjectNominal { .. }
            | Self::Opaque { .. } => None,
        }
    }
}

fn try_map_boxed<R, T, E>(
    values: Box<[R]>,
    map: &mut impl FnMut(R) -> Result<T, E>,
) -> Result<Box<[T]>, E> {
    values
        .into_vec()
        .into_iter()
        .map(map)
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

impl<R> RuntimeAgentTypeProjection<R> {
    fn try_map<T, E>(
        self,
        mut map: impl FnMut(R) -> Result<T, E>,
    ) -> Result<RuntimeAgentTypeProjection<T>, E> {
        Ok(match self {
            Self::DebugStatePath => RuntimeAgentTypeProjection::DebugStatePath,
            Self::ObservationFieldPath => RuntimeAgentTypeProjection::ObservationFieldPath,
            Self::Probe(value) => RuntimeAgentTypeProjection::Probe(map(value)?),
            Self::Predicate => RuntimeAgentTypeProjection::Predicate,
            Self::Observation => RuntimeAgentTypeProjection::Observation,
            Self::ObservedObject => RuntimeAgentTypeProjection::ObservedObject,
            Self::BoundingBox => RuntimeAgentTypeProjection::BoundingBox,
            Self::ActionName => RuntimeAgentTypeProjection::ActionName,
            Self::ActionTarget => RuntimeAgentTypeProjection::ActionTarget,
            Self::ActionResult => RuntimeAgentTypeProjection::ActionResult,
            Self::AgentValue => RuntimeAgentTypeProjection::AgentValue,
            Self::DataFormat => RuntimeAgentTypeProjection::DataFormat,
            Self::DataShape => RuntimeAgentTypeProjection::DataShape,
            Self::EntityMetadata => RuntimeAgentTypeProjection::EntityMetadata,
            Self::SourceAnchor => RuntimeAgentTypeProjection::SourceAnchor,
            Self::ProjectGraphNeighborhood => RuntimeAgentTypeProjection::ProjectGraphNeighborhood,
            Self::ProjectGraphSymbol => RuntimeAgentTypeProjection::ProjectGraphSymbol,
            Self::ProjectGraphEdge => RuntimeAgentTypeProjection::ProjectGraphEdge,
            Self::CaptureTarget => RuntimeAgentTypeProjection::CaptureTarget,
            Self::CaptureReference => RuntimeAgentTypeProjection::CaptureReference,
            Self::Resource => RuntimeAgentTypeProjection::Resource,
            Self::ResourceBody => RuntimeAgentTypeProjection::ResourceBody,
            Self::RagContextPack => RuntimeAgentTypeProjection::RagContextPack,
            Self::ObservedObjectId => RuntimeAgentTypeProjection::ObservedObjectId,
            Self::CaptureFormat => RuntimeAgentTypeProjection::CaptureFormat,
            Self::CaptureKind => RuntimeAgentTypeProjection::CaptureKind,
            Self::Diagnostics => RuntimeAgentTypeProjection::Diagnostics,
            Self::WaitError => RuntimeAgentTypeProjection::WaitError,
            Self::ViewportPoint => RuntimeAgentTypeProjection::ViewportPoint,
            Self::PointerButton => RuntimeAgentTypeProjection::PointerButton,
            Self::RagError => RuntimeAgentTypeProjection::RagError,
        })
    }

    #[must_use]
    pub const fn operational_type(&self) -> RuntimeAgentOperationalType {
        match self {
            Self::DebugStatePath => RuntimeAgentOperationalType::DebugStatePath,
            Self::ObservationFieldPath => RuntimeAgentOperationalType::ObservationFieldPath,
            Self::Probe(_) => RuntimeAgentOperationalType::Probe,
            Self::Predicate => RuntimeAgentOperationalType::Predicate,
            Self::Observation => RuntimeAgentOperationalType::Observation,
            Self::ObservedObject => RuntimeAgentOperationalType::ObservedObject,
            Self::BoundingBox => RuntimeAgentOperationalType::BoundingBox,
            Self::ActionName => RuntimeAgentOperationalType::ActionName,
            Self::ActionTarget => RuntimeAgentOperationalType::ActionTarget,
            Self::ActionResult => RuntimeAgentOperationalType::ActionResult,
            Self::AgentValue => RuntimeAgentOperationalType::AgentValue,
            Self::DataFormat => RuntimeAgentOperationalType::DataFormat,
            Self::DataShape => RuntimeAgentOperationalType::DataShape,
            Self::EntityMetadata => RuntimeAgentOperationalType::EntityMetadata,
            Self::SourceAnchor => RuntimeAgentOperationalType::SourceAnchor,
            Self::ProjectGraphNeighborhood => RuntimeAgentOperationalType::ProjectGraphNeighborhood,
            Self::ProjectGraphSymbol => RuntimeAgentOperationalType::ProjectGraphSymbol,
            Self::ProjectGraphEdge => RuntimeAgentOperationalType::ProjectGraphEdge,
            Self::CaptureTarget => RuntimeAgentOperationalType::CaptureTarget,
            Self::CaptureReference => RuntimeAgentOperationalType::CaptureReference,
            Self::Resource => RuntimeAgentOperationalType::Resource,
            Self::ResourceBody => RuntimeAgentOperationalType::ResourceBody,
            Self::RagContextPack => RuntimeAgentOperationalType::RagContextPack,
            Self::ObservedObjectId => RuntimeAgentOperationalType::ObservedObjectId,
            Self::CaptureFormat => RuntimeAgentOperationalType::CaptureFormat,
            Self::CaptureKind => RuntimeAgentOperationalType::CaptureKind,
            Self::Diagnostics => RuntimeAgentOperationalType::Diagnostics,
            Self::WaitError => RuntimeAgentOperationalType::WaitError,
            Self::ViewportPoint => RuntimeAgentOperationalType::ViewportPoint,
            Self::PointerButton => RuntimeAgentOperationalType::PointerButton,
            Self::RagError => RuntimeAgentOperationalType::RagError,
        }
    }
}
