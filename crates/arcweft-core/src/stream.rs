use crate::pattern::RuntimePattern;
use crate::source::SourceEventKind;
use crate::task::TaskSequence;
use crate::value::RuntimeExpr;
use std::collections::VecDeque;

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StreamRuntimeId(pub String);

/// Lowered stream transform state machine.
///
/// The core runtime keeps this as deterministic data. Host adapters may execute
/// the state machine or replace it with an equivalent backend implementation,
/// but device acquisition never happens inside this plan.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StreamPlan {
    pub id: StreamRuntimeId,
    pub item_ty: String,
    pub error_ty: String,
    pub ops: Vec<StreamOp>,
}

/// One operation in a lowered stream transform.
#[derive(Clone, Debug, Eq, PartialEq)]
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
    Noop,
}

/// One stream `match` arm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamMatchArm {
    pub pattern: RuntimePattern,
    pub guard: Option<RuntimeExpr>,
    pub ops: Vec<StreamOp>,
}

/// Lowered live source declaration.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamRuntimeState {
    pub id: StreamRuntimeId,
    pub queue: VecDeque<String>,
    pub closed: bool,
    pub emitted_count: u64,
}

/// Runtime stack frame used to make scope exit and loop transfer explicit.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamEvent<T, E> {
    pub stream: StreamRuntimeId,
    pub sequence: TaskSequence,
    pub kind: SourceEventKind<T, E>,
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

    pub fn push_item(&mut self, item: String) -> TaskSequence {
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
}
