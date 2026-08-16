//! Closed Agent expression and runtime-value algebra.
//!
//! Deterministic Agent constructors use this module as their sole executable
//! representation. Host adapters may project these values into protocol
//! types, but generic records and source labels are not an alternate Agent
//! authority.

use super::ownership::RuntimeValueOwnership;
use super::{RuntimeExpr, RuntimeValue};
use crate::entry::RuntimeCommandTargetId;
use crate::plan::RuntimeAgentOperationalType;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// One deterministic Agent expression retained by the runtime plan.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeAgentExpr {
    ChoiceAction {
        choice: RuntimeCommandTargetId,
    },
    Target(RuntimeAgentTargetExpr),
    Path(RuntimeAgentPathExpr),
    Probe(RuntimeAgentProbeExpr),
    Predicate(RuntimeAgentPredicateExpr),
    ViewportPoint {
        x: Box<RuntimeExpr>,
        y: Box<RuntimeExpr>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeAgentTargetExpr {
    Viewport,
    Layer { target: Box<RuntimeExpr> },
    Object { target: Box<RuntimeExpr> },
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeAgentPathExpr {
    State { path: Box<RuntimeExpr> },
    Observation { path: Box<RuntimeExpr> },
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeAgentProbeExpr {
    Signal { target: Box<RuntimeExpr> },
    Metric { target: Box<RuntimeExpr> },
    State { path: Box<RuntimeExpr> },
    Observation { path: Box<RuntimeExpr> },
    Diagnostics,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeAgentPredicateExpr {
    Compare {
        probe: Box<RuntimeExpr>,
        op: RuntimeAgentCompareOp,
        value: Box<RuntimeExpr>,
    },
    Exists {
        probe: Box<RuntimeExpr>,
    },
    ActionEnabled {
        target: Box<RuntimeExpr>,
    },
    DiagnosticsHasError {
        diagnostics: Box<RuntimeExpr>,
    },
    All {
        predicates: Vec<RuntimeExpr>,
    },
    Any {
        predicates: Vec<RuntimeExpr>,
    },
    Not {
        predicate: Box<RuntimeExpr>,
    },
}

/// Closed comparison metadata shared by expressions and Agent values.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeAgentCompareOp {
    Eq,
    NotEq,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
}

/// Closed field coordinate for Agent-owned runtime values.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeAgentField {
    ObservationTick,
    ObservationFrameId,
    ObservationStateHash,
    ObservationRenderHash,
    ObservationActions,
    ObservationObjects,
    ObservationSignals,
    ObservedObjectId,
    ObservedObjectParentId,
    ObservedObjectEntity,
    ObservedObjectLayer,
    ObservedObjectRole,
    ObservedObjectText,
    ObservedObjectVisible,
    ObservedObjectEnabled,
    ObservedObjectBoundingBox,
    BoundingBoxSpace,
    BoundingBoxX,
    BoundingBoxY,
    BoundingBoxWidth,
    BoundingBoxHeight,
    ActionId,
    ActionTarget,
    ActionName,
    ActionKind,
    ActionEnabled,
    ActionResultAccepted,
    ActionResultBeforeTick,
    ActionResultAfterTick,
    ActionResultBeforeStateHash,
    ActionResultAfterStateHash,
    CaptureReferenceUri,
    CaptureReferenceContentHash,
    CaptureReferenceMediaType,
    CaptureReferenceByteLen,
    ResourceUri,
    ResourceKind,
    ResourceMimeType,
    ResourceHash,
    ResourceBody,
    ResourceBodyKind,
    ResourceBodyJson,
    ResourceBodyText,
    ResourceBodyBase64,
    ResourceBodyEncoding,
    ResourceBodyValue,
    EntityMetadataId,
    EntityMetadataKind,
    EntityMetadataSemanticHash,
    EntityMetadataSource,
    SourceAnchorHasSource,
    SourceAnchorPath,
    SourceAnchorStartByte,
    SourceAnchorEndByte,
    SourceAnchorStartLine,
    SourceAnchorStartColumn,
    SourceAnchorEndLine,
    SourceAnchorEndColumn,
    ProjectGraphNeighborhoodRoot,
    ProjectGraphNeighborhoodNodeCount,
    ProjectGraphNeighborhoodEdgeCount,
    ProjectGraphNeighborhoodSymbols,
    ProjectGraphNeighborhoodEdges,
    ProjectGraphSymbolSymbolId,
    ProjectGraphSymbolId,
    ProjectGraphSymbolKind,
    ProjectGraphSymbolSemanticHash,
    ProjectGraphSymbolSummary,
    ProjectGraphSymbolHasEntity,
    ProjectGraphSymbolHasSemanticHash,
    ProjectGraphSymbolHasFlowControl,
    ProjectGraphSymbolHasDynamicControl,
    ProjectGraphSymbolHasProjectSummary,
    ProjectGraphSymbolEntityCount,
    ProjectGraphSymbolAgentActionCount,
    ProjectGraphSymbolProjectCallableCount,
    ProjectGraphSymbolRelationCount,
    ProjectGraphSymbolDependencyEdgeCount,
    ProjectGraphSymbolDynamicControlFlowCount,
    ProjectGraphSymbolDebugQueryCount,
    ProjectGraphSymbolStaticGotoCount,
    ProjectGraphSymbolDynamicGotoCount,
    ProjectGraphSymbolBranchCount,
    ProjectGraphSymbolLoopCount,
    ProjectGraphSymbolAwaitCount,
    ProjectGraphSymbolThreadCount,
    ProjectGraphSymbolSelectBranchCount,
    ProjectGraphEdgeFromSymbolId,
    ProjectGraphEdgeToSymbolId,
    ProjectGraphEdgeKind,
    ReferenceId,
    ReferenceFamily,
    ReferenceName,
    ViewportX,
    ViewportY,
}

/// Closed owner family for one Agent protocol field coordinate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeAgentFieldOwner {
    Agent(RuntimeAgentOperationalType),
    Reference,
}

/// Closed result family for one Agent protocol field coordinate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeAgentFieldResult {
    Bool,
    String,
    U32,
    U64,
    Agent(RuntimeAgentOperationalType),
    VecAgent(RuntimeAgentOperationalType),
    AgentValueMap,
}

impl RuntimeAgentField {
    /// Whether this coordinate is represented by an admitted Agent protocol
    /// record rather than an executable Agent value.
    #[must_use]
    pub const fn permits_protocol_record(self) -> bool {
        !matches!(
            self.owner(),
            RuntimeAgentFieldOwner::Agent(
                RuntimeAgentOperationalType::ActionTarget
                    | RuntimeAgentOperationalType::ViewportPoint
            ) | RuntimeAgentFieldOwner::Reference
        )
    }

    #[must_use]
    pub const fn owner(self) -> RuntimeAgentFieldOwner {
        if let Some(owner) = self.graph_owner() {
            return owner;
        }
        match self {
            Self::ObservationTick
            | Self::ObservationFrameId
            | Self::ObservationStateHash
            | Self::ObservationRenderHash
            | Self::ObservationActions
            | Self::ObservationObjects
            | Self::ObservationSignals => agent_owner(RuntimeAgentOperationalType::Observation),
            Self::ObservedObjectId
            | Self::ObservedObjectParentId
            | Self::ObservedObjectEntity
            | Self::ObservedObjectLayer
            | Self::ObservedObjectRole
            | Self::ObservedObjectText
            | Self::ObservedObjectVisible
            | Self::ObservedObjectEnabled
            | Self::ObservedObjectBoundingBox => {
                agent_owner(RuntimeAgentOperationalType::ObservedObject)
            }
            Self::BoundingBoxSpace
            | Self::BoundingBoxX
            | Self::BoundingBoxY
            | Self::BoundingBoxWidth
            | Self::BoundingBoxHeight => agent_owner(RuntimeAgentOperationalType::BoundingBox),
            Self::ActionId
            | Self::ActionTarget
            | Self::ActionName
            | Self::ActionKind
            | Self::ActionEnabled => agent_owner(RuntimeAgentOperationalType::ActionTarget),
            Self::ActionResultAccepted
            | Self::ActionResultBeforeTick
            | Self::ActionResultAfterTick
            | Self::ActionResultBeforeStateHash
            | Self::ActionResultAfterStateHash => {
                agent_owner(RuntimeAgentOperationalType::ActionResult)
            }
            Self::CaptureReferenceUri
            | Self::CaptureReferenceContentHash
            | Self::CaptureReferenceMediaType
            | Self::CaptureReferenceByteLen => {
                agent_owner(RuntimeAgentOperationalType::CaptureReference)
            }
            Self::ResourceUri
            | Self::ResourceKind
            | Self::ResourceMimeType
            | Self::ResourceHash
            | Self::ResourceBody => agent_owner(RuntimeAgentOperationalType::Resource),
            Self::ResourceBodyKind
            | Self::ResourceBodyJson
            | Self::ResourceBodyText
            | Self::ResourceBodyBase64
            | Self::ResourceBodyEncoding
            | Self::ResourceBodyValue => agent_owner(RuntimeAgentOperationalType::ResourceBody),
            Self::EntityMetadataId
            | Self::EntityMetadataKind
            | Self::EntityMetadataSemanticHash
            | Self::EntityMetadataSource => {
                agent_owner(RuntimeAgentOperationalType::EntityMetadata)
            }
            Self::SourceAnchorHasSource
            | Self::SourceAnchorPath
            | Self::SourceAnchorStartByte
            | Self::SourceAnchorEndByte
            | Self::SourceAnchorStartLine
            | Self::SourceAnchorStartColumn
            | Self::SourceAnchorEndLine
            | Self::SourceAnchorEndColumn => agent_owner(RuntimeAgentOperationalType::SourceAnchor),
            Self::ViewportX | Self::ViewportY => {
                agent_owner(RuntimeAgentOperationalType::ViewportPoint)
            }
            _ => unreachable!(),
        }
    }

    const fn graph_owner(self) -> Option<RuntimeAgentFieldOwner> {
        match self {
            Self::ProjectGraphNeighborhoodRoot
            | Self::ProjectGraphNeighborhoodNodeCount
            | Self::ProjectGraphNeighborhoodEdgeCount
            | Self::ProjectGraphNeighborhoodSymbols
            | Self::ProjectGraphNeighborhoodEdges => Some(agent_owner(
                RuntimeAgentOperationalType::ProjectGraphNeighborhood,
            )),
            Self::ProjectGraphSymbolSymbolId
            | Self::ProjectGraphSymbolId
            | Self::ProjectGraphSymbolKind
            | Self::ProjectGraphSymbolSemanticHash
            | Self::ProjectGraphSymbolSummary
            | Self::ProjectGraphSymbolHasEntity
            | Self::ProjectGraphSymbolHasSemanticHash
            | Self::ProjectGraphSymbolHasFlowControl
            | Self::ProjectGraphSymbolHasDynamicControl
            | Self::ProjectGraphSymbolHasProjectSummary
            | Self::ProjectGraphSymbolEntityCount
            | Self::ProjectGraphSymbolAgentActionCount
            | Self::ProjectGraphSymbolProjectCallableCount
            | Self::ProjectGraphSymbolRelationCount
            | Self::ProjectGraphSymbolDependencyEdgeCount
            | Self::ProjectGraphSymbolDynamicControlFlowCount
            | Self::ProjectGraphSymbolDebugQueryCount
            | Self::ProjectGraphSymbolStaticGotoCount
            | Self::ProjectGraphSymbolDynamicGotoCount
            | Self::ProjectGraphSymbolBranchCount
            | Self::ProjectGraphSymbolLoopCount
            | Self::ProjectGraphSymbolAwaitCount
            | Self::ProjectGraphSymbolThreadCount
            | Self::ProjectGraphSymbolSelectBranchCount => Some(project_graph_symbol_owner()),
            Self::ProjectGraphEdgeFromSymbolId
            | Self::ProjectGraphEdgeToSymbolId
            | Self::ProjectGraphEdgeKind => {
                Some(agent_owner(RuntimeAgentOperationalType::ProjectGraphEdge))
            }
            Self::ReferenceId | Self::ReferenceFamily | Self::ReferenceName => {
                Some(RuntimeAgentFieldOwner::Reference)
            }
            _ => None,
        }
    }

    #[must_use]
    pub const fn result(self) -> RuntimeAgentFieldResult {
        if let Some(result) = self.u32_result() {
            return result;
        }
        match self {
            Self::ObservationTick
            | Self::ActionResultBeforeTick
            | Self::ActionResultAfterTick
            | Self::CaptureReferenceByteLen
            | Self::SourceAnchorStartByte
            | Self::SourceAnchorEndByte => RuntimeAgentFieldResult::U64,
            Self::ObservationActions => {
                RuntimeAgentFieldResult::VecAgent(RuntimeAgentOperationalType::ActionTarget)
            }
            Self::ObservationObjects => {
                RuntimeAgentFieldResult::VecAgent(RuntimeAgentOperationalType::ObservedObject)
            }
            Self::ObservationSignals => RuntimeAgentFieldResult::AgentValueMap,
            Self::ObservedObjectId => {
                RuntimeAgentFieldResult::Agent(RuntimeAgentOperationalType::ObservedObjectId)
            }
            Self::ObservedObjectVisible
            | Self::ObservedObjectEnabled
            | Self::ActionEnabled
            | Self::ActionResultAccepted
            | Self::SourceAnchorHasSource
            | Self::ProjectGraphSymbolHasEntity
            | Self::ProjectGraphSymbolHasSemanticHash
            | Self::ProjectGraphSymbolHasFlowControl
            | Self::ProjectGraphSymbolHasDynamicControl
            | Self::ProjectGraphSymbolHasProjectSummary => RuntimeAgentFieldResult::Bool,
            Self::ObservedObjectBoundingBox => {
                RuntimeAgentFieldResult::Agent(RuntimeAgentOperationalType::BoundingBox)
            }
            Self::ActionName => {
                RuntimeAgentFieldResult::Agent(RuntimeAgentOperationalType::ActionName)
            }
            Self::ResourceBody => {
                RuntimeAgentFieldResult::Agent(RuntimeAgentOperationalType::ResourceBody)
            }
            Self::ResourceBodyValue => {
                RuntimeAgentFieldResult::Agent(RuntimeAgentOperationalType::AgentValue)
            }
            Self::EntityMetadataSource => {
                RuntimeAgentFieldResult::Agent(RuntimeAgentOperationalType::SourceAnchor)
            }
            Self::ProjectGraphNeighborhoodSymbols => {
                RuntimeAgentFieldResult::VecAgent(RuntimeAgentOperationalType::ProjectGraphSymbol)
            }
            Self::ProjectGraphNeighborhoodEdges => {
                RuntimeAgentFieldResult::VecAgent(RuntimeAgentOperationalType::ProjectGraphEdge)
            }
            Self::ObservationFrameId
            | Self::ObservationStateHash
            | Self::ObservationRenderHash
            | Self::ObservedObjectParentId
            | Self::ObservedObjectEntity
            | Self::ObservedObjectLayer
            | Self::ObservedObjectRole
            | Self::ObservedObjectText
            | Self::BoundingBoxSpace
            | Self::ActionId
            | Self::ActionTarget
            | Self::ActionKind
            | Self::ActionResultBeforeStateHash
            | Self::ActionResultAfterStateHash
            | Self::CaptureReferenceUri
            | Self::CaptureReferenceContentHash
            | Self::CaptureReferenceMediaType
            | Self::ResourceUri
            | Self::ResourceKind
            | Self::ResourceMimeType
            | Self::ResourceHash
            | Self::ResourceBodyKind
            | Self::ResourceBodyJson
            | Self::ResourceBodyText
            | Self::ResourceBodyBase64
            | Self::ResourceBodyEncoding
            | Self::EntityMetadataId
            | Self::EntityMetadataKind
            | Self::EntityMetadataSemanticHash
            | Self::SourceAnchorPath
            | Self::ProjectGraphNeighborhoodRoot
            | Self::ProjectGraphSymbolSymbolId
            | Self::ProjectGraphSymbolId
            | Self::ProjectGraphSymbolKind
            | Self::ProjectGraphSymbolSemanticHash
            | Self::ProjectGraphSymbolSummary
            | Self::ProjectGraphEdgeFromSymbolId
            | Self::ProjectGraphEdgeToSymbolId
            | Self::ProjectGraphEdgeKind
            | Self::ReferenceId
            | Self::ReferenceFamily
            | Self::ReferenceName => RuntimeAgentFieldResult::String,
            _ => unreachable!(),
        }
    }

    const fn u32_result(self) -> Option<RuntimeAgentFieldResult> {
        match self {
            Self::ProjectGraphNeighborhoodNodeCount
            | Self::ProjectGraphNeighborhoodEdgeCount
            | Self::SourceAnchorStartLine
            | Self::SourceAnchorStartColumn
            | Self::SourceAnchorEndLine
            | Self::SourceAnchorEndColumn
            | Self::BoundingBoxX
            | Self::BoundingBoxY
            | Self::BoundingBoxWidth
            | Self::BoundingBoxHeight
            | Self::ViewportX
            | Self::ViewportY
            | Self::ProjectGraphSymbolEntityCount
            | Self::ProjectGraphSymbolAgentActionCount
            | Self::ProjectGraphSymbolProjectCallableCount
            | Self::ProjectGraphSymbolRelationCount
            | Self::ProjectGraphSymbolDependencyEdgeCount
            | Self::ProjectGraphSymbolDynamicControlFlowCount
            | Self::ProjectGraphSymbolDebugQueryCount
            | Self::ProjectGraphSymbolStaticGotoCount
            | Self::ProjectGraphSymbolDynamicGotoCount
            | Self::ProjectGraphSymbolBranchCount
            | Self::ProjectGraphSymbolLoopCount
            | Self::ProjectGraphSymbolAwaitCount
            | Self::ProjectGraphSymbolThreadCount
            | Self::ProjectGraphSymbolSelectBranchCount => Some(RuntimeAgentFieldResult::U32),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::ObservationTick => "tick",
            Self::ObservationFrameId => "frame_id",
            Self::ObservationStateHash => "state_hash",
            Self::ObservationRenderHash => "render_hash",
            Self::ObservationActions => "actions",
            Self::ObservationObjects => "objects",
            Self::ObservationSignals => "signals",
            Self::ObservedObjectId
            | Self::ActionId
            | Self::EntityMetadataId
            | Self::ProjectGraphSymbolId
            | Self::ReferenceId => "id",
            Self::ObservedObjectParentId => "parent_id",
            Self::ObservedObjectEntity => "entity",
            Self::ObservedObjectLayer => "layer",
            Self::ObservedObjectRole => "role",
            Self::ObservedObjectText | Self::ResourceBodyText => "text",
            Self::ObservedObjectVisible => "visible",
            Self::ObservedObjectEnabled | Self::ActionEnabled => "enabled",
            Self::ObservedObjectBoundingBox => "bbox",
            Self::BoundingBoxSpace => "space",
            Self::BoundingBoxX | Self::ViewportX => "x",
            Self::BoundingBoxY | Self::ViewportY => "y",
            Self::BoundingBoxWidth => "width",
            Self::BoundingBoxHeight => "height",
            Self::ActionTarget => "target",
            Self::ActionName => "action",
            Self::ActionKind
            | Self::ResourceKind
            | Self::EntityMetadataKind
            | Self::ProjectGraphSymbolKind
            | Self::ProjectGraphEdgeKind
            | Self::ResourceBodyKind => "kind",
            Self::ActionResultAccepted => "accepted",
            Self::ActionResultBeforeTick => "before_tick",
            Self::ActionResultAfterTick => "after_tick",
            Self::ActionResultBeforeStateHash => "before_state_hash",
            Self::ActionResultAfterStateHash => "after_state_hash",
            Self::CaptureReferenceUri | Self::ResourceUri => "uri",
            Self::CaptureReferenceContentHash => "content_hash",
            Self::CaptureReferenceMediaType => "media_type",
            Self::CaptureReferenceByteLen => "byte_len",
            Self::ResourceMimeType => "mime_type",
            Self::ResourceHash => "hash",
            Self::ResourceBody => "body",
            Self::ResourceBodyJson => "json",
            Self::ResourceBodyBase64 => "base64",
            Self::ResourceBodyEncoding => "encoding",
            Self::ResourceBodyValue => "value",
            Self::EntityMetadataSemanticHash | Self::ProjectGraphSymbolSemanticHash => {
                "semantic_hash"
            }
            Self::EntityMetadataSource => "source",
            Self::SourceAnchorHasSource => "has_source",
            Self::SourceAnchorPath => "path",
            Self::SourceAnchorStartByte => "start_byte",
            Self::SourceAnchorEndByte => "end_byte",
            Self::SourceAnchorStartLine => "start_line",
            Self::SourceAnchorStartColumn => "start_column",
            Self::SourceAnchorEndLine => "end_line",
            Self::SourceAnchorEndColumn => "end_column",
            Self::ProjectGraphNeighborhoodRoot => "root",
            Self::ProjectGraphNeighborhoodNodeCount => "node_count",
            Self::ProjectGraphNeighborhoodEdgeCount => "edge_count",
            Self::ProjectGraphNeighborhoodSymbols => "symbols",
            Self::ProjectGraphNeighborhoodEdges => "edges",
            Self::ProjectGraphSymbolSymbolId => "symbol_id",
            Self::ProjectGraphSymbolSummary => "summary",
            Self::ProjectGraphSymbolHasEntity => "has_entity",
            Self::ProjectGraphSymbolHasSemanticHash => "has_semantic_hash",
            Self::ProjectGraphSymbolHasFlowControl => "has_flow_control",
            Self::ProjectGraphSymbolHasDynamicControl => "has_dynamic_control",
            Self::ProjectGraphSymbolHasProjectSummary => "has_project_summary",
            Self::ProjectGraphSymbolEntityCount => "entity_count",
            Self::ProjectGraphSymbolAgentActionCount => "agent_action_count",
            Self::ProjectGraphSymbolProjectCallableCount => "project_callable_count",
            Self::ProjectGraphSymbolRelationCount => "relation_count",
            Self::ProjectGraphSymbolDependencyEdgeCount => "dependency_edge_count",
            Self::ProjectGraphSymbolDynamicControlFlowCount => "dynamic_control_flow_count",
            Self::ProjectGraphSymbolDebugQueryCount => "debug_query_count",
            Self::ProjectGraphSymbolStaticGotoCount => "static_goto_count",
            Self::ProjectGraphSymbolDynamicGotoCount => "dynamic_goto_count",
            Self::ProjectGraphSymbolBranchCount => "branch_count",
            Self::ProjectGraphSymbolLoopCount => "loop_count",
            Self::ProjectGraphSymbolAwaitCount => "await_count",
            Self::ProjectGraphSymbolThreadCount => "thread_count",
            Self::ProjectGraphSymbolSelectBranchCount => "select_branch_count",
            Self::ProjectGraphEdgeFromSymbolId => "from_symbol_id",
            Self::ProjectGraphEdgeToSymbolId => "to_symbol_id",
            Self::ReferenceFamily => "family",
            Self::ReferenceName => "name",
        }
    }
}

const fn project_graph_symbol_owner() -> RuntimeAgentFieldOwner {
    RuntimeAgentFieldOwner::Agent(RuntimeAgentOperationalType::ProjectGraphSymbol)
}

const fn agent_owner(kind: RuntimeAgentOperationalType) -> RuntimeAgentFieldOwner {
    RuntimeAgentFieldOwner::Agent(kind)
}

impl RuntimeAgentCompareOp {
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::NotEq => "not_eq",
            Self::Greater => "greater",
            Self::GreaterOrEqual => "greater_or_equal",
            Self::Less => "less",
            Self::LessOrEqual => "less_or_equal",
        }
    }
}

/// Typed operation encoded by native evaluation and AWBC `MakeAgent`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeAgentConstructor {
    ChoiceAction,
    CaptureViewport,
    CaptureLayer,
    CaptureObject,
    StatePath,
    ObservationPath,
    ProbeSignal,
    ProbeMetric,
    ProbeState,
    ProbeObservation,
    Diagnostics,
    PredicateExists,
    PredicateActionEnabled,
    PredicateDiagnosticsHasError,
    PredicateAll,
    PredicateAny,
    PredicateNot,
    PredicateEq,
    PredicateNotEq,
    PredicateGreater,
    PredicateGreaterOrEqual,
    PredicateLess,
    PredicateLessOrEqual,
    ViewportPoint,
}

impl RuntimeAgentConstructor {
    #[must_use]
    pub const fn result_type(self) -> RuntimeAgentOperationalType {
        match self {
            Self::ChoiceAction => RuntimeAgentOperationalType::ActionTarget,
            Self::CaptureViewport | Self::CaptureLayer | Self::CaptureObject => {
                RuntimeAgentOperationalType::CaptureTarget
            }
            Self::StatePath => RuntimeAgentOperationalType::DebugStatePath,
            Self::ObservationPath => RuntimeAgentOperationalType::ObservationFieldPath,
            Self::ProbeSignal | Self::ProbeMetric | Self::ProbeState | Self::ProbeObservation => {
                RuntimeAgentOperationalType::Probe
            }
            Self::Diagnostics => RuntimeAgentOperationalType::Diagnostics,
            Self::PredicateExists
            | Self::PredicateActionEnabled
            | Self::PredicateDiagnosticsHasError
            | Self::PredicateAll
            | Self::PredicateAny
            | Self::PredicateNot
            | Self::PredicateEq
            | Self::PredicateNotEq
            | Self::PredicateGreater
            | Self::PredicateGreaterOrEqual
            | Self::PredicateLess
            | Self::PredicateLessOrEqual => RuntimeAgentOperationalType::Predicate,
            Self::ViewportPoint => RuntimeAgentOperationalType::ViewportPoint,
        }
    }

    #[must_use]
    pub const fn accepts_operand_count(self, count: usize) -> bool {
        match self {
            Self::CaptureViewport | Self::Diagnostics => count == 0,
            Self::ChoiceAction
            | Self::CaptureLayer
            | Self::CaptureObject
            | Self::StatePath
            | Self::ObservationPath
            | Self::ProbeSignal
            | Self::ProbeMetric
            | Self::ProbeState
            | Self::ProbeObservation
            | Self::PredicateExists
            | Self::PredicateActionEnabled
            | Self::PredicateDiagnosticsHasError
            | Self::PredicateNot => count == 1,
            Self::PredicateEq
            | Self::PredicateNotEq
            | Self::PredicateGreater
            | Self::PredicateGreaterOrEqual
            | Self::PredicateLess
            | Self::PredicateLessOrEqual
            | Self::ViewportPoint => count == 2,
            Self::PredicateAll | Self::PredicateAny => count >= 1,
        }
    }
}

impl RuntimeAgentExpr {
    pub(crate) fn try_from_admitted_constructor(
        constructor: RuntimeAgentConstructor,
        choice: Option<RuntimeCommandTargetId>,
        operands: Vec<RuntimeExpr>,
    ) -> Result<Self, RuntimeAgentConstructionError> {
        use RuntimeAgentConstructor as Constructor;

        if !constructor.accepts_operand_count(operands.len()) {
            return Err(RuntimeAgentConstructionError::InvalidOperandCount {
                constructor,
                actual: operands.len(),
            });
        }
        if matches!(constructor, RuntimeAgentConstructor::ChoiceAction) != choice.is_some() {
            return Err(RuntimeAgentConstructionError::InvalidExpressionChoice { constructor });
        }
        let mut operands = operands.into_iter();
        let mut next = || {
            operands.next().map(Box::new).ok_or(
                RuntimeAgentConstructionError::InvalidOperandCount {
                    constructor,
                    actual: 0,
                },
            )
        };
        Ok(match constructor {
            Constructor::ChoiceAction => Self::ChoiceAction {
                choice: choice.ok_or(RuntimeAgentConstructionError::InvalidExpressionChoice {
                    constructor,
                })?,
            },
            Constructor::CaptureViewport => Self::Target(RuntimeAgentTargetExpr::Viewport),
            Constructor::CaptureLayer => {
                Self::Target(RuntimeAgentTargetExpr::Layer { target: next()? })
            }
            Constructor::CaptureObject => {
                Self::Target(RuntimeAgentTargetExpr::Object { target: next()? })
            }
            Constructor::StatePath => Self::Path(RuntimeAgentPathExpr::State { path: next()? }),
            Constructor::ObservationPath => {
                Self::Path(RuntimeAgentPathExpr::Observation { path: next()? })
            }
            Constructor::ProbeSignal => {
                Self::Probe(RuntimeAgentProbeExpr::Signal { target: next()? })
            }
            Constructor::ProbeMetric => {
                Self::Probe(RuntimeAgentProbeExpr::Metric { target: next()? })
            }
            Constructor::ProbeState => Self::Probe(RuntimeAgentProbeExpr::State { path: next()? }),
            Constructor::ProbeObservation => {
                Self::Probe(RuntimeAgentProbeExpr::Observation { path: next()? })
            }
            Constructor::Diagnostics => Self::Probe(RuntimeAgentProbeExpr::Diagnostics),
            Constructor::PredicateExists => {
                Self::Predicate(RuntimeAgentPredicateExpr::Exists { probe: next()? })
            }
            Constructor::PredicateActionEnabled => {
                Self::Predicate(RuntimeAgentPredicateExpr::ActionEnabled { target: next()? })
            }
            Constructor::PredicateDiagnosticsHasError => {
                Self::Predicate(RuntimeAgentPredicateExpr::DiagnosticsHasError {
                    diagnostics: next()?,
                })
            }
            Constructor::PredicateAll | Constructor::PredicateAny => {
                let predicates = operands.collect::<Vec<_>>();
                if constructor == Constructor::PredicateAll {
                    Self::Predicate(RuntimeAgentPredicateExpr::All { predicates })
                } else {
                    Self::Predicate(RuntimeAgentPredicateExpr::Any { predicates })
                }
            }
            Constructor::PredicateNot => {
                Self::Predicate(RuntimeAgentPredicateExpr::Not { predicate: next()? })
            }
            Constructor::PredicateEq
            | Constructor::PredicateNotEq
            | Constructor::PredicateGreater
            | Constructor::PredicateGreaterOrEqual
            | Constructor::PredicateLess
            | Constructor::PredicateLessOrEqual => {
                let probe = next()?;
                let value = next()?;
                let op = match constructor {
                    Constructor::PredicateEq => RuntimeAgentCompareOp::Eq,
                    Constructor::PredicateNotEq => RuntimeAgentCompareOp::NotEq,
                    Constructor::PredicateGreater => RuntimeAgentCompareOp::Greater,
                    Constructor::PredicateGreaterOrEqual => RuntimeAgentCompareOp::GreaterOrEqual,
                    Constructor::PredicateLess => RuntimeAgentCompareOp::Less,
                    Constructor::PredicateLessOrEqual => RuntimeAgentCompareOp::LessOrEqual,
                    _ => {
                        return Err(RuntimeAgentConstructionError::InvalidExpressionChoice {
                            constructor,
                        });
                    }
                };
                Self::Predicate(RuntimeAgentPredicateExpr::Compare { probe, op, value })
            }
            Constructor::ViewportPoint => Self::ViewportPoint {
                x: next()?,
                y: next()?,
            },
        })
    }

    #[must_use]
    pub const fn constructor(&self) -> RuntimeAgentConstructor {
        match self {
            Self::ChoiceAction { .. } => RuntimeAgentConstructor::ChoiceAction,
            Self::Target(RuntimeAgentTargetExpr::Viewport) => {
                RuntimeAgentConstructor::CaptureViewport
            }
            Self::Target(RuntimeAgentTargetExpr::Layer { .. }) => {
                RuntimeAgentConstructor::CaptureLayer
            }
            Self::Target(RuntimeAgentTargetExpr::Object { .. }) => {
                RuntimeAgentConstructor::CaptureObject
            }
            Self::Path(RuntimeAgentPathExpr::State { .. }) => RuntimeAgentConstructor::StatePath,
            Self::Path(RuntimeAgentPathExpr::Observation { .. }) => {
                RuntimeAgentConstructor::ObservationPath
            }
            Self::Probe(RuntimeAgentProbeExpr::Signal { .. }) => {
                RuntimeAgentConstructor::ProbeSignal
            }
            Self::Probe(RuntimeAgentProbeExpr::Metric { .. }) => {
                RuntimeAgentConstructor::ProbeMetric
            }
            Self::Probe(RuntimeAgentProbeExpr::State { .. }) => RuntimeAgentConstructor::ProbeState,
            Self::Probe(RuntimeAgentProbeExpr::Observation { .. }) => {
                RuntimeAgentConstructor::ProbeObservation
            }
            Self::Probe(RuntimeAgentProbeExpr::Diagnostics) => RuntimeAgentConstructor::Diagnostics,
            Self::Predicate(RuntimeAgentPredicateExpr::Compare { op, .. }) => match op {
                RuntimeAgentCompareOp::Eq => RuntimeAgentConstructor::PredicateEq,
                RuntimeAgentCompareOp::NotEq => RuntimeAgentConstructor::PredicateNotEq,
                RuntimeAgentCompareOp::Greater => RuntimeAgentConstructor::PredicateGreater,
                RuntimeAgentCompareOp::GreaterOrEqual => {
                    RuntimeAgentConstructor::PredicateGreaterOrEqual
                }
                RuntimeAgentCompareOp::Less => RuntimeAgentConstructor::PredicateLess,
                RuntimeAgentCompareOp::LessOrEqual => RuntimeAgentConstructor::PredicateLessOrEqual,
            },
            Self::Predicate(RuntimeAgentPredicateExpr::Exists { .. }) => {
                RuntimeAgentConstructor::PredicateExists
            }
            Self::Predicate(RuntimeAgentPredicateExpr::ActionEnabled { .. }) => {
                RuntimeAgentConstructor::PredicateActionEnabled
            }
            Self::Predicate(RuntimeAgentPredicateExpr::DiagnosticsHasError { .. }) => {
                RuntimeAgentConstructor::PredicateDiagnosticsHasError
            }
            Self::Predicate(RuntimeAgentPredicateExpr::All { .. }) => {
                RuntimeAgentConstructor::PredicateAll
            }
            Self::Predicate(RuntimeAgentPredicateExpr::Any { .. }) => {
                RuntimeAgentConstructor::PredicateAny
            }
            Self::Predicate(RuntimeAgentPredicateExpr::Not { .. }) => {
                RuntimeAgentConstructor::PredicateNot
            }
            Self::ViewportPoint { .. } => RuntimeAgentConstructor::ViewportPoint,
        }
    }

    /// Returns runtime-authored operands in semantic order. `ChoiceAction`
    /// carries its accepted identity directly and therefore has no authored
    /// runtime operand.
    #[must_use]
    pub fn operands(&self) -> Vec<&RuntimeExpr> {
        match self {
            Self::ChoiceAction { .. }
            | Self::Target(RuntimeAgentTargetExpr::Viewport)
            | Self::Probe(RuntimeAgentProbeExpr::Diagnostics) => Vec::new(),
            Self::Target(
                RuntimeAgentTargetExpr::Layer { target }
                | RuntimeAgentTargetExpr::Object { target },
            )
            | Self::Predicate(RuntimeAgentPredicateExpr::ActionEnabled { target }) => {
                vec![target]
            }
            Self::Path(
                RuntimeAgentPathExpr::State { path } | RuntimeAgentPathExpr::Observation { path },
            )
            | Self::Probe(
                RuntimeAgentProbeExpr::State { path } | RuntimeAgentProbeExpr::Observation { path },
            ) => vec![path],
            Self::Probe(
                RuntimeAgentProbeExpr::Signal { target } | RuntimeAgentProbeExpr::Metric { target },
            ) => vec![target],
            Self::Predicate(RuntimeAgentPredicateExpr::Exists { probe }) => vec![probe],
            Self::Predicate(RuntimeAgentPredicateExpr::DiagnosticsHasError { diagnostics }) => {
                vec![diagnostics]
            }
            Self::Predicate(RuntimeAgentPredicateExpr::Not { predicate }) => vec![predicate],
            Self::Predicate(
                RuntimeAgentPredicateExpr::All { predicates }
                | RuntimeAgentPredicateExpr::Any { predicates },
            ) => predicates.iter().collect(),
            Self::Predicate(RuntimeAgentPredicateExpr::Compare { probe, value, .. }) => {
                vec![probe, value]
            }
            Self::ViewportPoint { x, y } => vec![x, y],
        }
    }

    #[must_use]
    pub const fn choice(&self) -> Option<&RuntimeCommandTargetId> {
        match self {
            Self::ChoiceAction { choice } => Some(choice),
            _ => None,
        }
    }
}

/// Core-owned typed Agent runtime value.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeAgentValue {
    ActionTarget(RuntimeAgentActionTarget),
    CaptureTarget(RuntimeAgentCaptureTarget),
    DebugStatePath(RuntimeAgentPath),
    ObservationFieldPath(RuntimeAgentPath),
    Probe(RuntimeAgentProbe),
    Diagnostics,
    Predicate(RuntimeAgentPredicate),
    ViewportPoint { x: u32, y: u32 },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeAgentActionTarget {
    id: RuntimeCommandTargetId,
    target: RuntimeCommandTargetId,
    action: RuntimeAgentAction,
    dispatch: RuntimeAgentActionDispatch,
    enabled: bool,
}

impl RuntimeAgentActionTarget {
    pub const fn new(
        id: RuntimeCommandTargetId,
        target: RuntimeCommandTargetId,
        action: RuntimeAgentAction,
        dispatch: RuntimeAgentActionDispatch,
        enabled: bool,
    ) -> Self {
        Self {
            id,
            target,
            action,
            dispatch,
            enabled,
        }
    }

    pub const fn id(&self) -> &RuntimeCommandTargetId {
        &self.id
    }

    pub const fn target(&self) -> &RuntimeCommandTargetId {
        &self.target
    }

    pub const fn action(&self) -> RuntimeAgentAction {
        self.action
    }

    pub const fn dispatch(&self) -> RuntimeAgentActionDispatch {
        self.dispatch
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeAgentAction {
    AdvanceText,
    SelectChoice,
    Invoke,
    Scroll,
    PointerClick,
}

impl RuntimeAgentAction {
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::AdvanceText => "advance_text",
            Self::SelectChoice => "select_choice",
            Self::Invoke => "invoke",
            Self::Scroll => "scroll",
            Self::PointerClick => "pointer_click",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeAgentActionDispatch {
    Semantic,
    Physical,
}

impl RuntimeAgentActionDispatch {
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Semantic => "semantic",
            Self::Physical => "physical",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeAgentCaptureTarget {
    Viewport,
    Layer { target: RuntimeCommandTargetId },
    Object { target: RuntimeCommandTargetId },
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct RuntimeAgentPath(String);

impl RuntimeAgentPath {
    pub fn try_new(path: impl Into<String>) -> Result<Self, RuntimeAgentConstructionError> {
        let path = path.into();
        if path.trim().is_empty() {
            return Err(RuntimeAgentConstructionError::InvalidPath);
        }
        if path.chars().any(char::is_control) {
            return Err(RuntimeAgentConstructionError::InvalidPath);
        }
        Ok(Self(path))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeAgentProbe {
    Signal { target: RuntimeCommandTargetId },
    Metric { target: RuntimeCommandTargetId },
    StatePath { path: RuntimeAgentPath },
    ObservationField { path: RuntimeAgentPath },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeAgentPredicate {
    Compare {
        probe: RuntimeAgentProbe,
        op: RuntimeAgentCompareOp,
        value: Box<RuntimeValue>,
    },
    Exists {
        probe: RuntimeAgentProbe,
    },
    ActionEnabled {
        target: RuntimeCommandTargetId,
    },
    DiagnosticsHasError,
    All {
        predicates: Vec<Self>,
    },
    Any {
        predicates: Vec<Self>,
    },
    Not {
        predicate: Box<Self>,
    },
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RuntimeAgentConstructionError {
    #[error("Agent constructor {constructor:?} does not accept {actual} operand(s)")]
    InvalidOperandCount {
        constructor: RuntimeAgentConstructor,
        actual: usize,
    },
    #[error("Agent constructor {constructor:?} expected {expected}, found {actual}")]
    InvalidOperand {
        constructor: RuntimeAgentConstructor,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("Agent expression constructor {constructor:?} has invalid accepted choice metadata")]
    InvalidExpressionChoice {
        constructor: RuntimeAgentConstructor,
    },
    #[error("Agent path must be non-empty and contain no control characters")]
    InvalidPath,
    #[error("Agent target identity is invalid: {0}")]
    InvalidTarget(String),
}

impl RuntimeAgentValue {
    /// Materializes one typed Agent value from an admitted closed constructor.
    pub fn try_construct(
        constructor: RuntimeAgentConstructor,
        operands: Vec<RuntimeValue>,
    ) -> Result<Self, RuntimeAgentConstructionError> {
        use RuntimeAgentConstructor as Constructor;

        if !constructor.accepts_operand_count(operands.len()) {
            return Err(RuntimeAgentConstructionError::InvalidOperandCount {
                constructor,
                actual: operands.len(),
            });
        }
        let mut operands = operands.into_iter();
        Ok(match constructor {
            Constructor::ChoiceAction => construct_choice_action(constructor, &mut operands)?,
            Constructor::CaptureViewport => {
                Self::CaptureTarget(RuntimeAgentCaptureTarget::Viewport)
            }
            Constructor::CaptureLayer => Self::CaptureTarget(RuntimeAgentCaptureTarget::Layer {
                target: target_operand(constructor, next(&mut operands, constructor)?)?,
            }),
            Constructor::CaptureObject => Self::CaptureTarget(RuntimeAgentCaptureTarget::Object {
                target: target_operand(constructor, next(&mut operands, constructor)?)?,
            }),
            Constructor::StatePath => Self::DebugStatePath(path_operand(
                constructor,
                next(&mut operands, constructor)?,
            )?),
            Constructor::ObservationPath => Self::ObservationFieldPath(path_operand(
                constructor,
                next(&mut operands, constructor)?,
            )?),
            Constructor::ProbeSignal => Self::Probe(RuntimeAgentProbe::Signal {
                target: target_operand(constructor, next(&mut operands, constructor)?)?,
            }),
            Constructor::ProbeMetric => Self::Probe(RuntimeAgentProbe::Metric {
                target: target_operand(constructor, next(&mut operands, constructor)?)?,
            }),
            Constructor::ProbeState => Self::Probe(RuntimeAgentProbe::StatePath {
                path: debug_path_operand(constructor, next(&mut operands, constructor)?)?,
            }),
            Constructor::ProbeObservation => Self::Probe(RuntimeAgentProbe::ObservationField {
                path: observation_path_operand(constructor, next(&mut operands, constructor)?)?,
            }),
            Constructor::Diagnostics => Self::Diagnostics,
            Constructor::PredicateExists => Self::Predicate(RuntimeAgentPredicate::Exists {
                probe: probe_operand(constructor, next(&mut operands, constructor)?)?,
            }),
            Constructor::PredicateActionEnabled => {
                construct_action_enabled_predicate(constructor, &mut operands)?
            }
            Constructor::PredicateDiagnosticsHasError => {
                diagnostics_operand(constructor, next(&mut operands, constructor)?)?;
                Self::Predicate(RuntimeAgentPredicate::DiagnosticsHasError)
            }
            Constructor::PredicateAll | Constructor::PredicateAny => {
                construct_predicate_collection(constructor, operands.collect())?
            }
            Constructor::PredicateNot => Self::Predicate(RuntimeAgentPredicate::Not {
                predicate: Box::new(predicate_operand(
                    constructor,
                    next(&mut operands, constructor)?,
                )?),
            }),
            Constructor::PredicateEq => {
                construct_compare_predicate(constructor, RuntimeAgentCompareOp::Eq, &mut operands)?
            }
            Constructor::PredicateNotEq => construct_compare_predicate(
                constructor,
                RuntimeAgentCompareOp::NotEq,
                &mut operands,
            )?,
            Constructor::PredicateGreater => construct_compare_predicate(
                constructor,
                RuntimeAgentCompareOp::Greater,
                &mut operands,
            )?,
            Constructor::PredicateGreaterOrEqual => construct_compare_predicate(
                constructor,
                RuntimeAgentCompareOp::GreaterOrEqual,
                &mut operands,
            )?,
            Constructor::PredicateLess => construct_compare_predicate(
                constructor,
                RuntimeAgentCompareOp::Less,
                &mut operands,
            )?,
            Constructor::PredicateLessOrEqual => construct_compare_predicate(
                constructor,
                RuntimeAgentCompareOp::LessOrEqual,
                &mut operands,
            )?,
            Constructor::ViewportPoint => construct_viewport_point(constructor, &mut operands)?,
        })
    }

    #[must_use]
    pub const fn operational_type(&self) -> RuntimeAgentOperationalType {
        match self {
            Self::ActionTarget(_) => RuntimeAgentOperationalType::ActionTarget,
            Self::CaptureTarget(_) => RuntimeAgentOperationalType::CaptureTarget,
            Self::DebugStatePath(_) => RuntimeAgentOperationalType::DebugStatePath,
            Self::ObservationFieldPath(_) => RuntimeAgentOperationalType::ObservationFieldPath,
            Self::Probe(_) => RuntimeAgentOperationalType::Probe,
            Self::Diagnostics => RuntimeAgentOperationalType::Diagnostics,
            Self::Predicate(_) => RuntimeAgentOperationalType::Predicate,
            Self::ViewportPoint { .. } => RuntimeAgentOperationalType::ViewportPoint,
        }
    }

    #[must_use]
    pub fn project_typed_field(&self, field: RuntimeAgentField) -> Option<RuntimeValue> {
        match (self, field) {
            (Self::ActionTarget(target), RuntimeAgentField::ActionId) => {
                Some(RuntimeValue::String(target.id().as_str().to_owned()))
            }
            (Self::ActionTarget(target), RuntimeAgentField::ActionTarget) => {
                Some(RuntimeValue::String(target.target().as_str().to_owned()))
            }
            (Self::ActionTarget(target), RuntimeAgentField::ActionName) => {
                Some(RuntimeValue::String(target.action().as_label().to_owned()))
            }
            (Self::ActionTarget(target), RuntimeAgentField::ActionKind) => Some(
                RuntimeValue::String(target.dispatch().as_label().to_owned()),
            ),
            (Self::ActionTarget(target), RuntimeAgentField::ActionEnabled) => {
                Some(RuntimeValue::Bool(target.enabled()))
            }
            (Self::ViewportPoint { x, .. }, RuntimeAgentField::ViewportX) => {
                Some(RuntimeValue::u32(*x))
            }
            (Self::ViewportPoint { y, .. }, RuntimeAgentField::ViewportY) => {
                Some(RuntimeValue::u32(*y))
            }
            _ => None,
        }
    }

    pub(crate) fn project_field_label(&self, field: &str) -> Option<RuntimeValue> {
        let field = match (self.operational_type(), field) {
            (RuntimeAgentOperationalType::ActionTarget, "id") => RuntimeAgentField::ActionId,
            (RuntimeAgentOperationalType::ActionTarget, "target") => {
                RuntimeAgentField::ActionTarget
            }
            (RuntimeAgentOperationalType::ActionTarget, "action") => RuntimeAgentField::ActionName,
            (RuntimeAgentOperationalType::ActionTarget, "kind") => RuntimeAgentField::ActionKind,
            (RuntimeAgentOperationalType::ActionTarget, "enabled") => {
                RuntimeAgentField::ActionEnabled
            }
            (RuntimeAgentOperationalType::ViewportPoint, "x") => RuntimeAgentField::ViewportX,
            (RuntimeAgentOperationalType::ViewportPoint, "y") => RuntimeAgentField::ViewportY,
            _ => return None,
        };
        self.project_typed_field(field)
    }

    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::ActionTarget(_) => "agent/action_target",
            Self::CaptureTarget(_) => "agent/capture_target",
            Self::DebugStatePath(_) => "agent/debug_state_path",
            Self::ObservationFieldPath(_) => "agent/observation_field_path",
            Self::Probe(_) => "agent/probe",
            Self::Diagnostics => "agent/diagnostics",
            Self::Predicate(_) => "agent/predicate",
            Self::ViewportPoint { .. } => "agent/viewport_point",
        }
    }

    pub(crate) fn ownership(&self) -> RuntimeValueOwnership {
        match self {
            Self::Predicate(predicate) => predicate_ownership(predicate),
            _ => RuntimeValueOwnership::Unrestricted,
        }
    }

    pub(crate) fn structural_nesting_depth(&self) -> usize {
        match self {
            Self::Predicate(predicate) => predicate_structural_depth(predicate),
            _ => 0,
        }
    }

    /// Returns every general runtime value embedded by this Agent value and
    /// its structural depth relative to the Agent value.
    #[must_use]
    pub fn nested_runtime_values_with_depth(&self) -> Vec<(usize, &RuntimeValue)> {
        let mut values = Vec::new();
        if let Self::Predicate(predicate) = self {
            collect_predicate_values(predicate, 0, &mut values);
        }
        values
    }

    /// Returns every text-bearing identity or path owned by this Agent value.
    #[must_use]
    pub fn text_values(&self) -> Vec<&str> {
        let mut strings = Vec::new();
        match self {
            Self::ActionTarget(target) => {
                strings.push(target.id().as_str());
                strings.push(target.target().as_str());
            }
            Self::CaptureTarget(RuntimeAgentCaptureTarget::Viewport)
            | Self::Diagnostics
            | Self::ViewportPoint { .. } => {}
            Self::CaptureTarget(
                RuntimeAgentCaptureTarget::Layer { target }
                | RuntimeAgentCaptureTarget::Object { target },
            ) => strings.push(target.as_str()),
            Self::DebugStatePath(path) | Self::ObservationFieldPath(path) => {
                strings.push(path.as_str());
            }
            Self::Probe(probe) => collect_probe_strings(probe, &mut strings),
            Self::Predicate(predicate) => collect_predicate_strings(predicate, &mut strings),
        }
        strings
    }

    pub(crate) fn predicate_collection_lengths(&self) -> Vec<usize> {
        let mut lengths = Vec::new();
        if let Self::Predicate(predicate) = self {
            collect_predicate_collection_lengths(predicate, &mut lengths);
        }
        lengths
    }

    pub(crate) fn additional_structural_node_count(&self) -> usize {
        match self {
            Self::Predicate(predicate) => predicate_node_count(predicate).saturating_sub(1),
            _ => 0,
        }
    }
}

fn predicate_ownership(predicate: &RuntimeAgentPredicate) -> RuntimeValueOwnership {
    match predicate {
        RuntimeAgentPredicate::Compare { value, .. } => value.ownership(),
        RuntimeAgentPredicate::All { predicates } | RuntimeAgentPredicate::Any { predicates } => {
            predicates.iter().fold(
                RuntimeValueOwnership::Unrestricted,
                |ownership, predicate| ownership.join(predicate_ownership(predicate)),
            )
        }
        RuntimeAgentPredicate::Not { predicate } => predicate_ownership(predicate),
        RuntimeAgentPredicate::Exists { .. }
        | RuntimeAgentPredicate::ActionEnabled { .. }
        | RuntimeAgentPredicate::DiagnosticsHasError => RuntimeValueOwnership::Unrestricted,
    }
}

fn predicate_structural_depth(predicate: &RuntimeAgentPredicate) -> usize {
    match predicate {
        RuntimeAgentPredicate::All { predicates } | RuntimeAgentPredicate::Any { predicates } => {
            predicates
                .iter()
                .map(predicate_structural_depth)
                .max()
                .unwrap_or(0)
                .saturating_add(1)
        }
        RuntimeAgentPredicate::Not { predicate } => {
            predicate_structural_depth(predicate).saturating_add(1)
        }
        RuntimeAgentPredicate::Compare { .. }
        | RuntimeAgentPredicate::Exists { .. }
        | RuntimeAgentPredicate::ActionEnabled { .. }
        | RuntimeAgentPredicate::DiagnosticsHasError => 0,
    }
}

fn collect_predicate_values<'a>(
    predicate: &'a RuntimeAgentPredicate,
    predicate_depth: usize,
    values: &mut Vec<(usize, &'a RuntimeValue)>,
) {
    match predicate {
        RuntimeAgentPredicate::Compare { value, .. } => {
            values.push((predicate_depth.saturating_add(1), value));
        }
        RuntimeAgentPredicate::All { predicates } | RuntimeAgentPredicate::Any { predicates } => {
            for predicate in predicates {
                collect_predicate_values(predicate, predicate_depth.saturating_add(1), values);
            }
        }
        RuntimeAgentPredicate::Not { predicate } => {
            collect_predicate_values(predicate, predicate_depth.saturating_add(1), values);
        }
        RuntimeAgentPredicate::Exists { .. }
        | RuntimeAgentPredicate::ActionEnabled { .. }
        | RuntimeAgentPredicate::DiagnosticsHasError => {}
    }
}

fn collect_probe_strings<'a>(probe: &'a RuntimeAgentProbe, strings: &mut Vec<&'a str>) {
    match probe {
        RuntimeAgentProbe::Signal { target } | RuntimeAgentProbe::Metric { target } => {
            strings.push(target.as_str());
        }
        RuntimeAgentProbe::StatePath { path } | RuntimeAgentProbe::ObservationField { path } => {
            strings.push(path.as_str());
        }
    }
}

fn collect_predicate_strings<'a>(predicate: &'a RuntimeAgentPredicate, strings: &mut Vec<&'a str>) {
    match predicate {
        RuntimeAgentPredicate::Compare { probe, .. } | RuntimeAgentPredicate::Exists { probe } => {
            collect_probe_strings(probe, strings);
        }
        RuntimeAgentPredicate::ActionEnabled { target } => strings.push(target.as_str()),
        RuntimeAgentPredicate::All { predicates } | RuntimeAgentPredicate::Any { predicates } => {
            for predicate in predicates {
                collect_predicate_strings(predicate, strings);
            }
        }
        RuntimeAgentPredicate::Not { predicate } => {
            collect_predicate_strings(predicate, strings);
        }
        RuntimeAgentPredicate::DiagnosticsHasError => {}
    }
}

fn collect_predicate_collection_lengths(
    predicate: &RuntimeAgentPredicate,
    lengths: &mut Vec<usize>,
) {
    match predicate {
        RuntimeAgentPredicate::All { predicates } | RuntimeAgentPredicate::Any { predicates } => {
            lengths.push(predicates.len());
            for predicate in predicates {
                collect_predicate_collection_lengths(predicate, lengths);
            }
        }
        RuntimeAgentPredicate::Not { predicate } => {
            collect_predicate_collection_lengths(predicate, lengths);
        }
        RuntimeAgentPredicate::Compare { .. }
        | RuntimeAgentPredicate::Exists { .. }
        | RuntimeAgentPredicate::ActionEnabled { .. }
        | RuntimeAgentPredicate::DiagnosticsHasError => {}
    }
}

fn predicate_node_count(predicate: &RuntimeAgentPredicate) -> usize {
    match predicate {
        RuntimeAgentPredicate::All { predicates } | RuntimeAgentPredicate::Any { predicates } => {
            predicates.iter().fold(1_usize, |nodes, predicate| {
                nodes.saturating_add(predicate_node_count(predicate))
            })
        }
        RuntimeAgentPredicate::Not { predicate } => {
            1_usize.saturating_add(predicate_node_count(predicate))
        }
        RuntimeAgentPredicate::Compare { .. }
        | RuntimeAgentPredicate::Exists { .. }
        | RuntimeAgentPredicate::ActionEnabled { .. }
        | RuntimeAgentPredicate::DiagnosticsHasError => 1,
    }
}

fn next(
    operands: &mut impl Iterator<Item = RuntimeValue>,
    constructor: RuntimeAgentConstructor,
) -> Result<RuntimeValue, RuntimeAgentConstructionError> {
    operands
        .next()
        .ok_or(RuntimeAgentConstructionError::InvalidOperandCount {
            constructor,
            actual: 0,
        })
}

fn target_operand(
    constructor: RuntimeAgentConstructor,
    value: RuntimeValue,
) -> Result<RuntimeCommandTargetId, RuntimeAgentConstructionError> {
    let value = match value {
        RuntimeValue::String(value) | RuntimeValue::EntityRef(value) => value,
        value => return invalid_operand(constructor, "string or entity target", &value),
    };
    RuntimeCommandTargetId::try_new(value)
        .map_err(|error| RuntimeAgentConstructionError::InvalidTarget(error.to_string()))
}

fn path_operand(
    constructor: RuntimeAgentConstructor,
    value: RuntimeValue,
) -> Result<RuntimeAgentPath, RuntimeAgentConstructionError> {
    match value {
        RuntimeValue::String(value) => RuntimeAgentPath::try_new(value),
        value => invalid_operand(constructor, "string path", &value),
    }
}

fn debug_path_operand(
    constructor: RuntimeAgentConstructor,
    value: RuntimeValue,
) -> Result<RuntimeAgentPath, RuntimeAgentConstructionError> {
    match value {
        RuntimeValue::Agent(RuntimeAgentValue::DebugStatePath(path)) => Ok(path),
        value => invalid_operand(constructor, "DebugStatePath", &value),
    }
}

fn observation_path_operand(
    constructor: RuntimeAgentConstructor,
    value: RuntimeValue,
) -> Result<RuntimeAgentPath, RuntimeAgentConstructionError> {
    match value {
        RuntimeValue::Agent(RuntimeAgentValue::ObservationFieldPath(path)) => Ok(path),
        value => invalid_operand(constructor, "ObservationFieldPath", &value),
    }
}

fn probe_operand(
    constructor: RuntimeAgentConstructor,
    value: RuntimeValue,
) -> Result<RuntimeAgentProbe, RuntimeAgentConstructionError> {
    match value {
        RuntimeValue::Agent(RuntimeAgentValue::Probe(probe)) => Ok(probe),
        value => invalid_operand(constructor, "Agent probe", &value),
    }
}

fn action_target_operand(
    constructor: RuntimeAgentConstructor,
    value: RuntimeValue,
) -> Result<RuntimeAgentActionTarget, RuntimeAgentConstructionError> {
    match value {
        RuntimeValue::Agent(RuntimeAgentValue::ActionTarget(target)) => Ok(target),
        value => invalid_operand(constructor, "Agent action target", &value),
    }
}

fn diagnostics_operand(
    constructor: RuntimeAgentConstructor,
    value: RuntimeValue,
) -> Result<(), RuntimeAgentConstructionError> {
    match value {
        RuntimeValue::Agent(RuntimeAgentValue::Diagnostics) => Ok(()),
        value => invalid_operand(constructor, "Agent diagnostics", &value),
    }
}

fn predicate_operand(
    constructor: RuntimeAgentConstructor,
    value: RuntimeValue,
) -> Result<RuntimeAgentPredicate, RuntimeAgentConstructionError> {
    match value {
        RuntimeValue::Agent(RuntimeAgentValue::Predicate(predicate)) => Ok(predicate),
        value => invalid_operand(constructor, "Agent predicate", &value),
    }
}

fn predicate_operands(
    constructor: RuntimeAgentConstructor,
    values: Vec<RuntimeValue>,
) -> Result<Vec<RuntimeAgentPredicate>, RuntimeAgentConstructionError> {
    let mut predicates = Vec::new();
    for value in values {
        match value {
            RuntimeValue::Seq(values) => {
                for value in values.into_values() {
                    predicates.push(predicate_operand(constructor, value)?);
                }
            }
            RuntimeValue::Tuple(values) => {
                for value in values {
                    predicates.push(predicate_operand(constructor, value)?);
                }
            }
            value => predicates.push(predicate_operand(constructor, value)?),
        }
    }
    if predicates.is_empty() {
        return Err(RuntimeAgentConstructionError::InvalidOperandCount {
            constructor,
            actual: 0,
        });
    }
    Ok(predicates)
}

fn construct_choice_action(
    constructor: RuntimeAgentConstructor,
    operands: &mut impl Iterator<Item = RuntimeValue>,
) -> Result<RuntimeAgentValue, RuntimeAgentConstructionError> {
    let target = target_operand(constructor, next(operands, constructor)?)?;
    let id = RuntimeCommandTargetId::try_new(format!("action.select_choice.{}", target.as_str()))
        .map_err(|error| RuntimeAgentConstructionError::InvalidTarget(error.to_string()))?;
    Ok(RuntimeAgentValue::ActionTarget(
        RuntimeAgentActionTarget::new(
            id,
            target,
            RuntimeAgentAction::SelectChoice,
            RuntimeAgentActionDispatch::Semantic,
            true,
        ),
    ))
}

fn construct_action_enabled_predicate(
    constructor: RuntimeAgentConstructor,
    operands: &mut impl Iterator<Item = RuntimeValue>,
) -> Result<RuntimeAgentValue, RuntimeAgentConstructionError> {
    let target = action_target_operand(constructor, next(operands, constructor)?)?;
    Ok(RuntimeAgentValue::Predicate(
        RuntimeAgentPredicate::ActionEnabled {
            target: target.target().clone(),
        },
    ))
}

fn construct_predicate_collection(
    constructor: RuntimeAgentConstructor,
    values: Vec<RuntimeValue>,
) -> Result<RuntimeAgentValue, RuntimeAgentConstructionError> {
    let predicates = predicate_operands(constructor, values)?;
    if constructor == RuntimeAgentConstructor::PredicateAll {
        Ok(RuntimeAgentValue::Predicate(RuntimeAgentPredicate::All {
            predicates,
        }))
    } else {
        Ok(RuntimeAgentValue::Predicate(RuntimeAgentPredicate::Any {
            predicates,
        }))
    }
}

fn construct_compare_predicate(
    constructor: RuntimeAgentConstructor,
    op: RuntimeAgentCompareOp,
    operands: &mut impl Iterator<Item = RuntimeValue>,
) -> Result<RuntimeAgentValue, RuntimeAgentConstructionError> {
    let probe = probe_operand(constructor, next(operands, constructor)?)?;
    let value = next(operands, constructor)?;
    Ok(RuntimeAgentValue::Predicate(
        RuntimeAgentPredicate::Compare {
            probe,
            op,
            value: Box::new(value),
        },
    ))
}

fn construct_viewport_point(
    constructor: RuntimeAgentConstructor,
    operands: &mut impl Iterator<Item = RuntimeValue>,
) -> Result<RuntimeAgentValue, RuntimeAgentConstructionError> {
    let x = next(operands, constructor)?;
    let x = u32_operand(constructor, &x)?;
    let y = next(operands, constructor)?;
    let y = u32_operand(constructor, &y)?;
    Ok(RuntimeAgentValue::ViewportPoint { x, y })
}

fn u32_operand(
    constructor: RuntimeAgentConstructor,
    value: &RuntimeValue,
) -> Result<u32, RuntimeAgentConstructionError> {
    value
        .try_u64()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| RuntimeAgentConstructionError::InvalidOperand {
            constructor,
            expected: "u32",
            actual: value_kind(value),
        })
}

fn invalid_operand<T>(
    constructor: RuntimeAgentConstructor,
    expected: &'static str,
    value: &RuntimeValue,
) -> Result<T, RuntimeAgentConstructionError> {
    Err(RuntimeAgentConstructionError::InvalidOperand {
        constructor,
        expected,
        actual: value_kind(value),
    })
}

const fn value_kind(value: &RuntimeValue) -> &'static str {
    match value {
        RuntimeValue::Unit => "unit",
        RuntimeValue::Bool(_) => "bool",
        RuntimeValue::Int(_) => "signed integer",
        RuntimeValue::UInt(_) => "unsigned integer",
        RuntimeValue::F32(_) => "f32",
        RuntimeValue::F64(_) => "f64",
        RuntimeValue::MatrixF32(_) => "matrix f32",
        RuntimeValue::MatrixF64(_) => "matrix f64",
        RuntimeValue::TensorF32(_) => "tensor f32",
        RuntimeValue::TensorF64(_) => "tensor f64",
        RuntimeValue::String(_) => "string",
        RuntimeValue::Char(_) => "char",
        RuntimeValue::Duration(_) => "duration",
        RuntimeValue::Range(_) => "range",
        RuntimeValue::Iterator(_) => "iterator",
        RuntimeValue::EntityRef(_) => "entity reference",
        RuntimeValue::Tuple(_) => "tuple",
        RuntimeValue::Seq(_) => "sequence",
        RuntimeValue::Record(_) => "record",
        RuntimeValue::NominalRecord(_) => "nominal record",
        RuntimeValue::Opaque(_) => "opaque",
        RuntimeValue::Reduction(_) => "Reduction",
        RuntimeValue::Agent(_) => "Agent value",
        RuntimeValue::Function(_) => "function",
        RuntimeValue::Variant { .. } => "variant",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choice_action_materializes_one_typed_action_target() {
        let value = RuntimeAgentValue::try_construct(
            RuntimeAgentConstructor::ChoiceAction,
            vec![RuntimeValue::EntityRef("choice.opening.listen".to_owned())],
        )
        .expect("accepted choice identity constructs an action target");
        let RuntimeAgentValue::ActionTarget(target) = value else {
            panic!("choice_action must construct ActionTarget");
        };
        assert_eq!(target.target().as_str(), "choice.opening.listen");
        assert_eq!(
            target.id().as_str(),
            "action.select_choice.choice.opening.listen"
        );
        assert_eq!(target.action(), RuntimeAgentAction::SelectChoice);
        assert_eq!(target.dispatch(), RuntimeAgentActionDispatch::Semantic);
        assert!(target.enabled());
    }

    #[test]
    fn agent_constructor_rejects_generic_record_fallback() {
        let raw_probe = RuntimeValue::try_record(vec![(
            "kind".to_owned(),
            RuntimeValue::String("signal".to_owned()),
        )])
        .expect("test record is structurally valid");
        assert!(matches!(
            RuntimeAgentValue::try_construct(
                RuntimeAgentConstructor::PredicateExists,
                vec![raw_probe]
            ),
            Err(RuntimeAgentConstructionError::InvalidOperand { .. })
        ));
    }

    #[test]
    fn recursive_predicate_depth_participates_in_runtime_value_nesting() {
        let mut predicate = RuntimeAgentPredicate::DiagnosticsHasError;
        for _ in 0..65 {
            predicate = RuntimeAgentPredicate::Not {
                predicate: Box::new(predicate),
            };
        }
        let value = RuntimeValue::Agent(RuntimeAgentValue::Predicate(predicate));
        assert!(value.validate_nesting_depth(64).is_err());
    }

    #[test]
    fn protocol_field_coordinates_own_their_label_and_runtime_family() {
        let field = RuntimeAgentField::ProjectGraphNeighborhoodSymbols;
        assert_eq!(field.as_label(), "symbols");
        assert_eq!(
            field.owner(),
            RuntimeAgentFieldOwner::Agent(RuntimeAgentOperationalType::ProjectGraphNeighborhood)
        );
        assert_eq!(
            field.result(),
            RuntimeAgentFieldResult::VecAgent(RuntimeAgentOperationalType::ProjectGraphSymbol)
        );
        assert!(field.permits_protocol_record());
        assert!(!RuntimeAgentField::ActionId.permits_protocol_record());
        assert!(!RuntimeAgentField::ViewportX.permits_protocol_record());
    }
}
