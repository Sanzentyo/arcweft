use crate::effect::{LineEffectRequest, RuntimeAssignment, RuntimeLog};
use crate::pattern::RuntimePattern;
use crate::task::TaskSequence;
use crate::value::RuntimeExpr;
use std::collections::VecDeque;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourcePlan {
    pub id: SourceId,
    pub item_ty: String,
    pub error_ty: String,
    pub from: RuntimeExpr,
    pub policy: SourcePolicy,
    pub handlers: Vec<SourceHandlerPlan>,
}

/// Handler for one live source event kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceHandlerPlan {
    Item {
        pattern: RuntimePattern,
        ops: Vec<SourceOp>,
    },
    Error {
        pattern: RuntimePattern,
        ops: Vec<SourceOp>,
    },
    Progress {
        pattern: RuntimePattern,
        ops: Vec<SourceOp>,
    },
    Disconnected {
        ops: Vec<SourceOp>,
    },
    PermissionRevoked {
        ops: Vec<SourceOp>,
    },
    End {
        ops: Vec<SourceOp>,
    },
}

/// Operation inside a source handler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceOp {
    Yield(RuntimeExpr),
    Effect(LineEffectRequest),
    SignalWrite(RuntimeAssignment),
    Log(RuntimeLog),
    Close(SourceId),
    Noop,
}

/// One deterministic operation in a lowered flow program.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRuntimeState {
    pub id: SourceId,
    pub policy: SourcePolicy,
    pub queue: VecDeque<String>,
    pub closed: bool,
    pub last_error: Option<String>,
    pub overflow_count: u64,
}

/// Replay-observable state for one derived stream queue.

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceId(pub String);

pub fn normalize_source_events<T, E>(mut events: Vec<SourceEvent<T, E>>) -> Vec<SourceEvent<T, E>> {
    events.sort_by_key(|event| (event.source.clone(), event.sequence));
    events
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourcePolicy {
    pub backpressure: BackpressurePolicy,
    pub replay: ReplayPolicy,
    pub privacy: PrivacyPolicy,
    pub max_queue: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackpressurePolicy {
    LatestOnly,
    BoundedQueue {
        capacity: usize,
        on_overflow: OverflowPolicy,
    },
    BlockingNotAllowed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OverflowPolicy {
    DropOldest,
    DropNewest,
    Error,
    Coalesce,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplayPolicy {
    Full,
    HashOnly,
    Summary,
    EventOnly,
    None,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrivacyPolicy {
    Transient,
    Redacted,
    Recordable,
    Private,
}

impl Default for SourcePolicy {
    fn default() -> Self {
        Self {
            backpressure: BackpressurePolicy::LatestOnly,
            replay: ReplayPolicy::EventOnly,
            privacy: PrivacyPolicy::Transient,
            max_queue: 1,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceEvent<T, E> {
    pub source: SourceId,
    pub sequence: TaskSequence,
    pub kind: SourceEventKind<T, E>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceEventKind<T, E> {
    Item(T),
    Progress(String),
    Disconnected,
    PermissionRevoked,
    Error(E),
    End,
}

impl SourceRuntimeState {
    pub fn new(id: SourceId, policy: SourcePolicy) -> Self {
        Self {
            id,
            policy,
            queue: VecDeque::new(),
            closed: false,
            last_error: None,
            overflow_count: 0,
        }
    }

    pub fn apply_event(&mut self, event: SourceEvent<String, String>) -> Option<String> {
        match event.kind {
            SourceEventKind::Item(item) => self.push_item(item),
            SourceEventKind::Error(error) => {
                self.last_error = Some(error.clone());
                Some(format!("source {} error: {error}", self.id.0))
            }
            SourceEventKind::Disconnected
            | SourceEventKind::PermissionRevoked
            | SourceEventKind::End => {
                self.closed = true;
                None
            }
            SourceEventKind::Progress(_) => None,
        }
    }

    pub fn close(&mut self) {
        self.closed = true;
        self.queue.clear();
    }

    pub(crate) fn push_item(&mut self, item: String) -> Option<String> {
        if self.closed {
            return Some(format!("source {} received item after close", self.id.0));
        }
        let backpressure = self.policy.backpressure.clone();
        match &backpressure {
            BackpressurePolicy::LatestOnly => {
                self.queue.clear();
                self.queue.push_back(item);
                None
            }
            BackpressurePolicy::BoundedQueue {
                capacity,
                on_overflow,
            } => self.push_bounded_item(*capacity, on_overflow, item),
            BackpressurePolicy::BlockingNotAllowed => {
                if self.queue.is_empty() {
                    self.queue.push_back(item);
                    None
                } else {
                    self.overflow_count += 1;
                    Some(format!(
                        "source {} overflowed a blocking-not-allowed queue",
                        self.id.0
                    ))
                }
            }
        }
    }

    fn push_bounded_item(
        &mut self,
        capacity: usize,
        on_overflow: &OverflowPolicy,
        item: String,
    ) -> Option<String> {
        if capacity == 0 {
            self.overflow_count += 1;
            return Some(format!("source {} has zero queue capacity", self.id.0));
        }
        if self.queue.len() < capacity {
            self.queue.push_back(item);
            return None;
        }
        self.overflow_count += 1;
        match on_overflow {
            OverflowPolicy::DropOldest => {
                self.queue.pop_front();
                self.queue.push_back(item);
                None
            }
            OverflowPolicy::DropNewest => None,
            OverflowPolicy::Error => Some(format!("source {} queue overflow", self.id.0)),
            OverflowPolicy::Coalesce => {
                self.queue.pop_back();
                self.queue.push_back(item);
                None
            }
        }
    }
}
