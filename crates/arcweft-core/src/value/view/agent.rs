//! One Agent/predicate grammar over live and inert payload storage.

use super::super::{
    RuntimeAgentActionTarget, RuntimeAgentCaptureTarget, RuntimeAgentCompareOp, RuntimeAgentPath,
    RuntimeAgentPredicate, RuntimeAgentProbe, RuntimeAgentValue, RuntimeDataShape,
    awbc_save::{AwbcRuntimeAgentPredicateSnapshot, AwbcRuntimeAgentSnapshot},
};
use super::RuntimeValueSource;
use crate::{
    entry::RuntimeCommandTargetId, pattern::RuntimeSemanticTypeId,
    plan::RuntimeAgentOperationalType, program_types::RuntimeProgramTypes,
};

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeAgentView<'a> {
    Live(&'a RuntimeAgentValue),
    Snapshot(&'a AwbcRuntimeAgentSnapshot),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeAgentKind<'a> {
    ActionTarget(&'a RuntimeAgentActionTarget),
    CaptureTarget(&'a RuntimeAgentCaptureTarget),
    DebugStatePath(&'a RuntimeAgentPath),
    ObservationFieldPath(&'a RuntimeAgentPath),
    Probe(&'a RuntimeAgentProbe),
    Diagnostics,
    Predicate(RuntimePredicateView<'a>),
    ViewportPoint { x: u32, y: u32 },
    BinaryData(&'a str),
    DataShape(RuntimeDataShapeView<'a>),
}

impl<'a> RuntimeAgentView<'a> {
    pub(crate) fn kind(self) -> RuntimeAgentKind<'a> {
        use AwbcRuntimeAgentSnapshot as Snap;
        use RuntimeAgentKind as Kind;
        use RuntimeAgentValue as Live;
        match self {
            Self::Live(Live::ActionTarget(v)) | Self::Snapshot(Snap::ActionTarget(v)) => {
                Kind::ActionTarget(v)
            }
            Self::Live(Live::CaptureTarget(v)) | Self::Snapshot(Snap::CaptureTarget(v)) => {
                Kind::CaptureTarget(v)
            }
            Self::Live(Live::DebugStatePath(v)) | Self::Snapshot(Snap::DebugStatePath(v)) => {
                Kind::DebugStatePath(v)
            }
            Self::Live(Live::ObservationFieldPath(v))
            | Self::Snapshot(Snap::ObservationFieldPath(v)) => Kind::ObservationFieldPath(v),
            Self::Live(Live::Probe(v)) | Self::Snapshot(Snap::Probe(v)) => Kind::Probe(v),
            Self::Live(Live::Diagnostics) | Self::Snapshot(Snap::Diagnostics) => Kind::Diagnostics,
            Self::Live(Live::Predicate(v)) => Kind::Predicate(RuntimePredicateView::Live(v)),
            Self::Snapshot(Snap::Predicate(v)) => {
                Kind::Predicate(RuntimePredicateView::Snapshot(v))
            }
            Self::Live(Live::ViewportPoint { x, y })
            | Self::Snapshot(Snap::ViewportPoint { x, y }) => Kind::ViewportPoint { x: *x, y: *y },
            Self::Live(Live::BinaryData(v)) | Self::Snapshot(Snap::BinaryData(v)) => {
                Kind::BinaryData(v)
            }
            Self::Live(Live::DataShape(v)) => Kind::DataShape(RuntimeDataShapeView::Live(v)),
            Self::Snapshot(Snap::DataShape {
                shape_type,
                value_type,
            }) => Kind::DataShape(RuntimeDataShapeView::Snapshot {
                shape_type: *shape_type,
                value_type: *value_type,
            }),
        }
    }
    pub(crate) fn operational_type(self) -> RuntimeAgentOperationalType {
        match self.kind() {
            RuntimeAgentKind::ActionTarget(_) => RuntimeAgentOperationalType::ActionTarget,
            RuntimeAgentKind::CaptureTarget(_) => RuntimeAgentOperationalType::CaptureTarget,
            RuntimeAgentKind::DebugStatePath(_) => RuntimeAgentOperationalType::DebugStatePath,
            RuntimeAgentKind::ObservationFieldPath(_) => {
                RuntimeAgentOperationalType::ObservationFieldPath
            }
            RuntimeAgentKind::Probe(_) => RuntimeAgentOperationalType::Probe,
            RuntimeAgentKind::Diagnostics => RuntimeAgentOperationalType::Diagnostics,
            RuntimeAgentKind::Predicate(_) => RuntimeAgentOperationalType::Predicate,
            RuntimeAgentKind::ViewportPoint { .. } => RuntimeAgentOperationalType::ViewportPoint,
            RuntimeAgentKind::BinaryData(_) => RuntimeAgentOperationalType::BinaryData,
            RuntimeAgentKind::DataShape(_) => RuntimeAgentOperationalType::DataShape,
        }
    }
    pub(crate) const fn live_value(self) -> Option<&'a RuntimeAgentValue> {
        match self {
            Self::Live(v) => Some(v),
            Self::Snapshot(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeDataShapeView<'a> {
    Live(&'a RuntimeDataShape),
    Snapshot {
        shape_type: RuntimeSemanticTypeId,
        value_type: RuntimeSemanticTypeId,
    },
}

impl RuntimeDataShapeView<'_> {
    pub(crate) fn shape_type(self) -> RuntimeSemanticTypeId {
        match self {
            Self::Live(v) => v.shape_type(),
            Self::Snapshot { shape_type, .. } => shape_type,
        }
    }
    pub(crate) fn value_type(self) -> RuntimeSemanticTypeId {
        match self {
            Self::Live(v) => v.value_type(),
            Self::Snapshot { value_type, .. } => value_type,
        }
    }
    pub(crate) fn matches_plan(
        self,
        plan: &crate::plan::RuntimePlanInventory,
        expected: RuntimeSemanticTypeId,
    ) -> bool {
        self.shape_type() == expected
            && RuntimeProgramTypes::Plan(plan)
                .data_shape_value_type(expected)
                .ok()
                == Some(self.value_type())
            && match self {
                Self::Live(v) => v.matches_plan(plan, expected),
                Self::Snapshot { .. } => true,
            }
    }
    pub(crate) fn matches_awbc(
        self,
        program: &crate::awbc::schema::AwbcProgram,
        expected: RuntimeSemanticTypeId,
    ) -> bool {
        self.shape_type() == expected
            && RuntimeProgramTypes::Awbc(program)
                .data_shape_value_type(expected)
                .ok()
                == Some(self.value_type())
            && match self {
                Self::Live(v) => v.matches_awbc(program, expected),
                Self::Snapshot { .. } => true,
            }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimePredicateView<'a> {
    Live(&'a RuntimeAgentPredicate),
    Snapshot(&'a AwbcRuntimeAgentPredicateSnapshot),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimePredicateKind<'a> {
    Compare {
        probe: &'a RuntimeAgentProbe,
        op: RuntimeAgentCompareOp,
        value: RuntimeValueSource<'a>,
    },
    Exists {
        probe: &'a RuntimeAgentProbe,
    },
    ActionEnabled {
        target: &'a RuntimeCommandTargetId,
    },
    DiagnosticsHasError,
    All(RuntimePredicateListView<'a>),
    Any(RuntimePredicateListView<'a>),
    Not(RuntimePredicateView<'a>),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimePredicateListView<'a> {
    Live(&'a [RuntimeAgentPredicate]),
    Snapshot(&'a [AwbcRuntimeAgentPredicateSnapshot]),
}

impl<'a> RuntimePredicateView<'a> {
    pub(crate) fn kind(self) -> RuntimePredicateKind<'a> {
        use AwbcRuntimeAgentPredicateSnapshot as Snap;
        use RuntimeAgentPredicate as Live;
        use RuntimePredicateKind as Kind;
        match self {
            Self::Live(Live::Compare { probe, op, value }) => Kind::Compare {
                probe,
                op: *op,
                value: RuntimeValueSource::Live(value),
            },
            Self::Snapshot(Snap::Compare { probe, op, value }) => Kind::Compare {
                probe,
                op: *op,
                value: RuntimeValueSource::Snapshot(value),
            },
            Self::Live(Live::Exists { probe }) | Self::Snapshot(Snap::Exists { probe }) => {
                Kind::Exists { probe }
            }
            Self::Live(Live::ActionEnabled { target })
            | Self::Snapshot(Snap::ActionEnabled { target }) => Kind::ActionEnabled { target },
            Self::Live(Live::DiagnosticsHasError) | Self::Snapshot(Snap::DiagnosticsHasError) => {
                Kind::DiagnosticsHasError
            }
            Self::Live(Live::All { predicates }) => {
                Kind::All(RuntimePredicateListView::Live(predicates.as_slice()))
            }
            Self::Snapshot(Snap::All { predicates }) => {
                Kind::All(RuntimePredicateListView::Snapshot(predicates.as_slice()))
            }
            Self::Live(Live::Any { predicates }) => {
                Kind::Any(RuntimePredicateListView::Live(predicates.as_slice()))
            }
            Self::Snapshot(Snap::Any { predicates }) => {
                Kind::Any(RuntimePredicateListView::Snapshot(predicates.as_slice()))
            }
            Self::Live(Live::Not { predicate }) => Kind::Not(Self::Live(predicate)),
            Self::Snapshot(Snap::Not { predicate }) => Kind::Not(Self::Snapshot(predicate)),
        }
    }
}

impl<'a> RuntimePredicateListView<'a> {
    pub(crate) fn len(self) -> usize {
        match self {
            Self::Live(v) => v.len(),
            Self::Snapshot(v) => v.len(),
        }
    }
    pub(crate) fn get(self, index: usize) -> Option<RuntimePredicateView<'a>> {
        match self {
            Self::Live(v) => v.get(index).map(RuntimePredicateView::Live),
            Self::Snapshot(v) => v.get(index).map(RuntimePredicateView::Snapshot),
        }
    }
}
