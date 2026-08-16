use crate::pattern::RuntimePattern;
use crate::runtime_id::RuntimePlanTypeId;
use crate::runtime_id::{RuntimeIdError, RuntimeIdFamily, RuntimeIdPath, RuntimePublicLabel};
use crate::task::TaskSequence;
use crate::value::{RuntimeExpr, RuntimePayload};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct StreamRuntimeId {
    path: RuntimeIdPath,
}

/// Lowered stream transform state machine.
///
/// The core runtime keeps this as deterministic data. Host adapters may execute
/// the state machine or replace it with an equivalent backend implementation,
/// but device acquisition never happens inside this plan.
#[derive(Clone, Debug, PartialEq)]
pub struct StreamPlan {
    id: StreamRuntimeId,
    item_ty: RuntimePlanTypeId,
    error_ty: RuntimePlanTypeId,
    ops: Vec<StreamOp>,
}

impl StreamPlan {
    pub(crate) fn from_admitted_parts(
        id: StreamRuntimeId,
        item_ty: RuntimePlanTypeId,
        error_ty: RuntimePlanTypeId,
        ops: Vec<StreamOp>,
    ) -> Self {
        Self {
            id,
            item_ty,
            error_ty,
            ops,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &StreamRuntimeId {
        &self.id
    }

    #[must_use]
    pub const fn item_ty(&self) -> RuntimePlanTypeId {
        self.item_ty
    }

    #[must_use]
    pub const fn error_ty(&self) -> RuntimePlanTypeId {
        self.error_ty
    }

    #[must_use]
    pub fn ops(&self) -> &[StreamOp] {
        &self.ops
    }
}

/// One operation in a lowered stream transform.
#[derive(Clone, Debug, PartialEq)]
pub enum StreamOp {
    Let {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
    },
    ForNext {
        pattern: RuntimePattern,
        source: RuntimeExpr,
        body: Vec<StreamOp>,
    },
    Yield {
        expr: RuntimeExpr,
    },
    If {
        condition: RuntimeExpr,
        then_ops: Vec<StreamOp>,
        else_ops: Vec<StreamOp>,
    },
    Match {
        scrutinee: RuntimeExpr,
        arms: Vec<StreamMatchArm>,
    },
    Close {
        source: RuntimeExpr,
    },
    Return,
}

/// One stream `match` arm.
#[derive(Clone, Debug, PartialEq)]
pub struct StreamMatchArm {
    pub pattern: RuntimePattern,
    pub guard: Option<RuntimeExpr>,
    pub ops: Vec<StreamOp>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StreamRuntimeState {
    pub id: StreamRuntimeId,
    pub queue: VecDeque<RuntimePayload>,
    pub closed: bool,
    pub emitted_count: u64,
}

/// Event emitted by an ordinary callable-backed stream.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum StreamEventKind<T, E> {
    Item(T),
    Error(E),
    End,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StreamEvent<T, E> {
    pub stream: StreamRuntimeId,
    pub sequence: TaskSequence,
    pub kind: StreamEventKind<T, E>,
}

pub type RuntimeStreamEvent = StreamEvent<RuntimePayload, RuntimePayload>;

impl StreamRuntimeId {
    pub fn canonical(value: &str) -> Result<Self, RuntimeIdError> {
        RuntimeIdPath::from_canonical_str(RuntimeIdFamily::Stream, value).map(|path| Self { path })
    }

    pub fn from_source_entity_body(value: &str) -> Result<Self, RuntimeIdError> {
        RuntimeIdPath::from_source_entity_body(
            RuntimeIdFamily::Stream,
            value,
            RuntimeIdFamily::Stream.source_families(),
        )
        .map(|path| Self { path })
    }

    pub fn from_runtime_target_value(value: &str) -> Result<Self, RuntimeIdError> {
        let Some((family, _)) = value.split_once('.') else {
            return Self::canonical(value);
        };
        if RuntimeIdFamily::Stream.source_families().contains(&family) {
            Self::from_source_entity_body(value)
        } else {
            Self::canonical(value)
        }
    }

    #[must_use]
    pub const fn path(&self) -> &RuntimeIdPath {
        &self.path
    }

    #[must_use]
    pub fn canonical_label(&self) -> String {
        self.path.label()
    }

    #[must_use]
    pub fn public_label(&self) -> RuntimePublicLabel {
        RuntimePublicLabel::for_family(RuntimeIdFamily::Stream, &self.path)
    }
}

impl StreamRuntimeState {
    pub fn new(id: StreamRuntimeId) -> Self {
        Self {
            id,
            queue: VecDeque::new(),
            closed: false,
            emitted_count: 0,
        }
    }

    pub fn push_item(&mut self, item: RuntimePayload) -> TaskSequence {
        let sequence = TaskSequence(self.emitted_count);
        self.emitted_count += 1;
        if !self.closed {
            self.queue.push_back(item);
        }
        sequence
    }

    pub fn close(&mut self) {
        self.closed = true;
        self.queue.clear();
    }

    pub fn close_with_sequence(&mut self) -> Option<TaskSequence> {
        if self.closed {
            return None;
        }
        let sequence = TaskSequence(self.emitted_count);
        self.emitted_count += 1;
        self.close();
        Some(sequence)
    }
}
