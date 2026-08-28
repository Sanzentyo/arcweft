//! Canonical runtime-plan type projection graph.

use crate::entry::{RuntimeNominalTypeId, TypeLayoutHash};
use crate::pattern::{
    RuntimeBuiltinVariantIdentity, RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeProducerId,
};
use crate::value::{
    RuntimeOpaquePersistence, RuntimeOpaqueValueClass, RuntimeSignedIntWidth,
    RuntimeUnsignedIntWidth,
};
use serde::{Deserialize, Serialize};

/// Closed top-level runtime families owned by the Agent Prelude.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u8)]
pub enum RuntimeAgentOperationalType {
    DebugStatePath = 0,
    ObservationFieldPath = 1,
    Probe = 2,
    Predicate = 3,
    Observation = 4,
    ObservedObject = 5,
    BoundingBox = 6,
    ActionName = 7,
    ActionTarget = 8,
    ActionResult = 9,
    DataFormat = 11,
    DataShape = 12,
    EntityMetadata = 13,
    SourceAnchor = 14,
    SourcePosition = 31,
    ProjectGraphNeighborhood = 15,
    ProjectGraphSymbol = 16,
    ProjectGraphEdge = 17,
    ProjectFlowControlSummary = 32,
    ProjectGraphSummary = 33,
    CaptureTarget = 18,
    CaptureReference = 19,
    Resource = 20,
    RagContextPack = 22,
    ObservedObjectId = 23,
    CaptureFormat = 24,
    CaptureKind = 25,
    Diagnostics = 26,
    WaitError = 27,
    ViewportPoint = 28,
    PointerButton = 29,
    RagError = 30,
    BinaryResourceBody = 34,
    BinaryData = 35,
}

impl RuntimeAgentOperationalType {
    /// Stable kind tag shared by Agent DTO type and snapshot evidence.
    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_semantic_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            0 => Self::DebugStatePath,
            1 => Self::ObservationFieldPath,
            2 => Self::Probe,
            3 => Self::Predicate,
            4 => Self::Observation,
            5 => Self::ObservedObject,
            6 => Self::BoundingBox,
            7 => Self::ActionName,
            8 => Self::ActionTarget,
            9 => Self::ActionResult,
            11 => Self::DataFormat,
            12 => Self::DataShape,
            13 => Self::EntityMetadata,
            14 => Self::SourceAnchor,
            15 => Self::ProjectGraphNeighborhood,
            16 => Self::ProjectGraphSymbol,
            17 => Self::ProjectGraphEdge,
            18 => Self::CaptureTarget,
            19 => Self::CaptureReference,
            20 => Self::Resource,
            22 => Self::RagContextPack,
            23 => Self::ObservedObjectId,
            24 => Self::CaptureFormat,
            25 => Self::CaptureKind,
            26 => Self::Diagnostics,
            27 => Self::WaitError,
            28 => Self::ViewportPoint,
            29 => Self::PointerButton,
            30 => Self::RagError,
            31 => Self::SourcePosition,
            32 => Self::ProjectFlowControlSummary,
            33 => Self::ProjectGraphSummary,
            34 => Self::BinaryResourceBody,
            35 => Self::BinaryData,
            _ => return None,
        })
    }

    /// Digest of the core-owned DTO snapshot contract for this closed kind.
    #[must_use]
    pub fn snapshot_contract_digest(self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.agent.dto-snapshot-contract.v1\0");
        hasher.update(&[self.semantic_tag()]);
        *hasher.finalize().as_bytes()
    }

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
                | Self::SourcePosition
                | Self::ProjectGraphNeighborhood
                | Self::ProjectGraphSymbol
                | Self::ProjectGraphEdge
                | Self::ProjectFlowControlSummary
                | Self::ProjectGraphSummary
                | Self::CaptureReference
                | Self::Resource
                | Self::BinaryResourceBody
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
    Record,
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
    AgentValue,
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
        value_payload: R,
        error_payload: R,
    },
    Option {
        item: R,
        some_payload: R,
    },
    BuiltinVariant {
        owner: RuntimeBuiltinVariantIdentity,
        cases: Box<[Option<R>]>,
    },
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
    Record(Box<[RuntimePlanRecordField<R>]>),
    Choice(Box<[R]>),
    Opaque {
        producer: RuntimeOpaqueTypeProducerId,
        admission: RuntimeOpaqueTypeAdmission,
        value_class: RuntimeOpaqueValueClass,
        persistence: RuntimeOpaquePersistence,
        arguments: Box<[R]>,
    },
    Agent(RuntimeAgentTypeProjection<R>),
}

#[derive(Clone, Debug)]
pub struct RuntimePlanRecordField<R> {
    diagnostic_name: String,
    ty: R,
}

impl<R: PartialEq> PartialEq for RuntimePlanRecordField<R> {
    fn eq(&self, other: &Self) -> bool {
        self.ty == other.ty
    }
}

impl<R: Eq> Eq for RuntimePlanRecordField<R> {}

impl<R> RuntimePlanRecordField<R> {
    pub fn new(diagnostic_name: impl Into<String>, ty: R) -> Self {
        Self {
            diagnostic_name: diagnostic_name.into(),
            ty,
        }
    }

    pub fn diagnostic_name(&self) -> &str {
        &self.diagnostic_name
    }

    pub const fn ty(&self) -> &R {
        &self.ty
    }

    fn try_map<T, E>(
        self,
        map: &mut impl FnMut(R) -> Result<T, E>,
    ) -> Result<RuntimePlanRecordField<T>, E> {
        Ok(RuntimePlanRecordField {
            diagnostic_name: self.diagnostic_name,
            ty: map(self.ty)?,
        })
    }
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
    DataFormat,
    DataShape,
    EntityMetadata,
    SourceAnchor,
    SourcePosition,
    ProjectGraphNeighborhood,
    ProjectGraphSymbol,
    ProjectGraphEdge,
    ProjectFlowControlSummary,
    ProjectGraphSummary,
    CaptureTarget,
    CaptureReference,
    Resource,
    RagContextPack,
    ObservedObjectId,
    CaptureFormat,
    CaptureKind,
    Diagnostics,
    WaitError,
    ViewportPoint,
    PointerButton,
    RagError,
    BinaryResourceBody,
    BinaryData,
}

impl<R> RuntimePlanTypeProjection<R> {
    /// Child references in canonical declaration order.
    pub fn children(&self) -> Box<[&R]> {
        match self {
            Self::Range(child)
            | Self::Iterator(child)
            | Self::ThreadHandle(child)
            | Self::Shared(child)
            | Self::Reference(child)
            | Self::Need(child)
            | Self::Sequence { item: child, .. }
            | Self::Array { item: child, .. }
            | Self::Agent(RuntimeAgentTypeProjection::Probe(child)) => Box::new([child]),
            Self::BuiltinVariant { cases, .. } => cases.iter().filter_map(Option::as_ref).collect(),
            Self::Map { key, value }
            | Self::Stream {
                item: key,
                error: value,
            } => Box::new([key, value]),
            Self::Result {
                value: key,
                error: value,
                value_payload,
                error_payload,
            } => Box::new([key, value, value_payload, error_payload]),
            Self::Option { item, some_payload } => Box::new([item, some_payload]),
            Self::Function { parameters, result } => parameters
                .iter()
                .chain(std::iter::once(result))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            Self::ProjectNominal { arguments, .. }
            | Self::Opaque { arguments, .. }
            | Self::Tuple(arguments)
            | Self::Choice(arguments) => arguments.iter().collect::<Vec<_>>().into_boxed_slice(),
            Self::Record(fields) => fields
                .iter()
                .map(RuntimePlanRecordField::ty)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
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
            | Self::AgentValue
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
            Self::AgentValue => RuntimePlanTypeProjection::AgentValue,
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
            Self::Result {
                value,
                error,
                value_payload,
                error_payload,
            } => RuntimePlanTypeProjection::Result {
                value: map(value)?,
                error: map(error)?,
                value_payload: map(value_payload)?,
                error_payload: map(error_payload)?,
            },
            Self::Option { item, some_payload } => RuntimePlanTypeProjection::Option {
                item: map(item)?,
                some_payload: map(some_payload)?,
            },
            Self::BuiltinVariant { owner, cases } => RuntimePlanTypeProjection::BuiltinVariant {
                owner,
                cases: cases
                    .into_vec()
                    .into_iter()
                    .map(|payload| payload.map(&mut map).transpose())
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            },
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
            Self::Record(fields) => {
                RuntimePlanTypeProjection::Record(try_map_record_fields(fields, &mut map)?)
            }
            Self::Choice(items) => {
                RuntimePlanTypeProjection::Choice(try_map_boxed(items, &mut map)?)
            }
            Self::Opaque {
                producer,
                admission,
                value_class,
                persistence,
                arguments,
            } => RuntimePlanTypeProjection::Opaque {
                producer,
                admission,
                value_class,
                persistence,
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
            Self::Record(_) => Some(RuntimeOperationalType::Record),
            Self::Choice(_) => Some(RuntimeOperationalType::Choice),
            Self::Result { .. } => Some(RuntimeOperationalType::Result),
            Self::Option { .. } => Some(RuntimeOperationalType::Option),
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
            | Self::AgentValue
            | Self::BuiltinVariant { .. }
            | Self::ProjectNominal { .. }
            | Self::Opaque { .. } => None,
        }
    }
}

fn try_map_record_fields<R, T, E>(
    fields: Box<[RuntimePlanRecordField<R>]>,
    map: &mut impl FnMut(R) -> Result<T, E>,
) -> Result<Box<[RuntimePlanRecordField<T>]>, E> {
    fields
        .into_vec()
        .into_iter()
        .map(|field| field.try_map(map))
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
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
            Self::DataFormat => RuntimeAgentTypeProjection::DataFormat,
            Self::DataShape => RuntimeAgentTypeProjection::DataShape,
            Self::EntityMetadata => RuntimeAgentTypeProjection::EntityMetadata,
            Self::SourceAnchor => RuntimeAgentTypeProjection::SourceAnchor,
            Self::SourcePosition => RuntimeAgentTypeProjection::SourcePosition,
            Self::ProjectGraphNeighborhood => RuntimeAgentTypeProjection::ProjectGraphNeighborhood,
            Self::ProjectGraphSymbol => RuntimeAgentTypeProjection::ProjectGraphSymbol,
            Self::ProjectGraphEdge => RuntimeAgentTypeProjection::ProjectGraphEdge,
            Self::ProjectFlowControlSummary => {
                RuntimeAgentTypeProjection::ProjectFlowControlSummary
            }
            Self::ProjectGraphSummary => RuntimeAgentTypeProjection::ProjectGraphSummary,
            Self::CaptureTarget => RuntimeAgentTypeProjection::CaptureTarget,
            Self::CaptureReference => RuntimeAgentTypeProjection::CaptureReference,
            Self::Resource => RuntimeAgentTypeProjection::Resource,
            Self::RagContextPack => RuntimeAgentTypeProjection::RagContextPack,
            Self::ObservedObjectId => RuntimeAgentTypeProjection::ObservedObjectId,
            Self::CaptureFormat => RuntimeAgentTypeProjection::CaptureFormat,
            Self::CaptureKind => RuntimeAgentTypeProjection::CaptureKind,
            Self::Diagnostics => RuntimeAgentTypeProjection::Diagnostics,
            Self::WaitError => RuntimeAgentTypeProjection::WaitError,
            Self::ViewportPoint => RuntimeAgentTypeProjection::ViewportPoint,
            Self::PointerButton => RuntimeAgentTypeProjection::PointerButton,
            Self::RagError => RuntimeAgentTypeProjection::RagError,
            Self::BinaryResourceBody => RuntimeAgentTypeProjection::BinaryResourceBody,
            Self::BinaryData => RuntimeAgentTypeProjection::BinaryData,
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
            Self::DataFormat => RuntimeAgentOperationalType::DataFormat,
            Self::DataShape => RuntimeAgentOperationalType::DataShape,
            Self::EntityMetadata => RuntimeAgentOperationalType::EntityMetadata,
            Self::SourceAnchor => RuntimeAgentOperationalType::SourceAnchor,
            Self::SourcePosition => RuntimeAgentOperationalType::SourcePosition,
            Self::ProjectGraphNeighborhood => RuntimeAgentOperationalType::ProjectGraphNeighborhood,
            Self::ProjectGraphSymbol => RuntimeAgentOperationalType::ProjectGraphSymbol,
            Self::ProjectGraphEdge => RuntimeAgentOperationalType::ProjectGraphEdge,
            Self::ProjectFlowControlSummary => {
                RuntimeAgentOperationalType::ProjectFlowControlSummary
            }
            Self::ProjectGraphSummary => RuntimeAgentOperationalType::ProjectGraphSummary,
            Self::CaptureTarget => RuntimeAgentOperationalType::CaptureTarget,
            Self::CaptureReference => RuntimeAgentOperationalType::CaptureReference,
            Self::Resource => RuntimeAgentOperationalType::Resource,
            Self::RagContextPack => RuntimeAgentOperationalType::RagContextPack,
            Self::ObservedObjectId => RuntimeAgentOperationalType::ObservedObjectId,
            Self::CaptureFormat => RuntimeAgentOperationalType::CaptureFormat,
            Self::CaptureKind => RuntimeAgentOperationalType::CaptureKind,
            Self::Diagnostics => RuntimeAgentOperationalType::Diagnostics,
            Self::WaitError => RuntimeAgentOperationalType::WaitError,
            Self::ViewportPoint => RuntimeAgentOperationalType::ViewportPoint,
            Self::PointerButton => RuntimeAgentOperationalType::PointerButton,
            Self::RagError => RuntimeAgentOperationalType::RagError,
            Self::BinaryResourceBody => RuntimeAgentOperationalType::BinaryResourceBody,
            Self::BinaryData => RuntimeAgentOperationalType::BinaryData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeAgentOperationalType;

    #[test]
    fn agent_operational_type_tags_are_stable_sparse_and_closed() {
        let cases = [
            (RuntimeAgentOperationalType::DebugStatePath, 0),
            (RuntimeAgentOperationalType::ObservationFieldPath, 1),
            (RuntimeAgentOperationalType::Probe, 2),
            (RuntimeAgentOperationalType::Predicate, 3),
            (RuntimeAgentOperationalType::Observation, 4),
            (RuntimeAgentOperationalType::ObservedObject, 5),
            (RuntimeAgentOperationalType::BoundingBox, 6),
            (RuntimeAgentOperationalType::ActionName, 7),
            (RuntimeAgentOperationalType::ActionTarget, 8),
            (RuntimeAgentOperationalType::ActionResult, 9),
            (RuntimeAgentOperationalType::DataFormat, 11),
            (RuntimeAgentOperationalType::DataShape, 12),
            (RuntimeAgentOperationalType::EntityMetadata, 13),
            (RuntimeAgentOperationalType::SourceAnchor, 14),
            (RuntimeAgentOperationalType::ProjectGraphNeighborhood, 15),
            (RuntimeAgentOperationalType::ProjectGraphSymbol, 16),
            (RuntimeAgentOperationalType::ProjectGraphEdge, 17),
            (RuntimeAgentOperationalType::CaptureTarget, 18),
            (RuntimeAgentOperationalType::CaptureReference, 19),
            (RuntimeAgentOperationalType::Resource, 20),
            (RuntimeAgentOperationalType::RagContextPack, 22),
            (RuntimeAgentOperationalType::ObservedObjectId, 23),
            (RuntimeAgentOperationalType::CaptureFormat, 24),
            (RuntimeAgentOperationalType::CaptureKind, 25),
            (RuntimeAgentOperationalType::Diagnostics, 26),
            (RuntimeAgentOperationalType::WaitError, 27),
            (RuntimeAgentOperationalType::ViewportPoint, 28),
            (RuntimeAgentOperationalType::PointerButton, 29),
            (RuntimeAgentOperationalType::RagError, 30),
            (RuntimeAgentOperationalType::SourcePosition, 31),
            (RuntimeAgentOperationalType::ProjectFlowControlSummary, 32),
            (RuntimeAgentOperationalType::ProjectGraphSummary, 33),
            (RuntimeAgentOperationalType::BinaryResourceBody, 34),
            (RuntimeAgentOperationalType::BinaryData, 35),
        ];

        for (kind, tag) in cases {
            assert_eq!(kind.semantic_tag(), tag);
            assert_eq!(
                RuntimeAgentOperationalType::from_semantic_tag(tag),
                Some(kind)
            );
        }
        assert_eq!(RuntimeAgentOperationalType::from_semantic_tag(10), None);
        assert_eq!(RuntimeAgentOperationalType::from_semantic_tag(21), None);
        assert_eq!(RuntimeAgentOperationalType::from_semantic_tag(36), None);
        assert_eq!(
            RuntimeAgentOperationalType::from_semantic_tag(u8::MAX),
            None
        );
    }
}
