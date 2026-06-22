use crate::audio::RuntimeAudioCommand;
use crate::line_task::LineOutRequest;
use crate::time::LogicalDuration;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum LineEffectRequest {
    RegisterHandle {
        key: String,
        handle: String,
    },
    DropHandle {
        key: String,
    },
    Wait(RuntimeWaitTarget),
    Audio(Box<RuntimeAudioCommand>),
    Call(RuntimeCall),
    Log(RuntimeLog),
    SignalWrite(RuntimeAssignment),
    MetricWrite(RuntimeAssignment),
    EmitEvent(RuntimeEvent),
    Out(LineOutRequest),
    Return(String),
    Goto(String),
    Panic(String),
    Fail(String),
    Bail(String),
    Ensure {
        condition: String,
        message: String,
    },
    Assert(RuntimeAssertion),
    Close(String),
    Select(String),
    Break {
        label: Option<String>,
        value: Option<String>,
    },
    Continue {
        label: Option<String>,
    },
}

impl LineEffectRequest {
    pub const fn is_audio(&self) -> bool {
        matches!(self, Self::Audio(_))
    }

    pub fn audio(&self) -> Option<&RuntimeAudioCommand> {
        match self {
            Self::Audio(command) => Some(command.as_ref()),
            _ => None,
        }
    }
}

/// Runtime assertion request emitted by ordinary `assert(...)` calls.
///
/// The core remains Sans I/O: this data says when an assertion should be
/// enforced, while the host/test runner chooses how assertion failures are
/// logged, traced, or surfaced.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeAssertion {
    pub condition: String,
    pub message: String,
    pub profile: RuntimeAssertionProfile,
}

/// Profile policy for runtime assertions.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeAssertionProfile {
    Always,
    DebugOnly,
}

/// Access information used by static conflict checks for parallel regions.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResourceAccess {
    pub key: String,
    pub mode: ResourceAccessMode,
    pub policy: ConflictPolicy,
}

/// Resource access kind.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum ResourceAccessMode {
    Read,
    Write,
    Drop,
    Append,
    Control,
}

/// Conflict resolution policy for resource accesses in a parallel region.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum ConflictPolicy {
    Error,
    Append,
    LastWriterWins { priority: i32 },
    MergePatch,
    Reduce { op: ReduceOp },
}

/// Deterministic reduce operator for mergeable parallel writes.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum ReduceOp {
    Sum,
    Min,
    Max,
    And,
    Or,
}

/// Input event placeholder kept as Sans I/O data.

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeCall {
    pub callee: String,
    pub args: Vec<String>,
}

/// Structured target for an ordinary `wait(...)` effect.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeWaitTarget {
    Duration(LogicalDuration),
    Mark(String),
    Expr(String),
}

/// Structured log request preserved for defmt-style template interning later.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeLog {
    pub level: String,
    pub message: String,
    pub fields: Vec<RuntimeField>,
}

/// Assignment-like runtime request used by signal and metric updates.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeAssignment {
    pub target: String,
    pub value: String,
}

/// Structured event emission request.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeEvent {
    pub event: String,
    pub fields: Vec<RuntimeField>,
}

/// Named expression payload preserved in runtime IR without performing I/O.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeField {
    pub name: String,
    pub value: String,
}
