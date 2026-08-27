use crate::audio::RuntimeAudioCommand;
use crate::line_task::LineOutRequest;
use crate::time::LogicalDuration;
use crate::value::{RuntimeExpr, RuntimeValue, runtime_value_label};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod assertion_identity;

pub use assertion_identity::{
    RuntimeArtifactFingerprint, RuntimeAssertionGuardId, RuntimeIdentityDecodeError,
};

#[derive(Clone, Debug, PartialEq)]
pub enum LineEffectRequest {
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

/// Effect request whose value arguments are evaluated at the owning runtime
/// instruction instead of being preserved as source-text labels.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeEffectExpr {
    Log {
        level: String,
        message: RuntimeExpr,
        fields: Vec<RuntimeEffectFieldExpr>,
    },
    SignalWrite {
        target: RuntimeExpr,
        value: RuntimeExpr,
    },
    MetricWrite {
        target: RuntimeExpr,
        value: RuntimeExpr,
    },
    EmitEvent {
        event: RuntimeExpr,
        fields: Vec<RuntimeEffectFieldExpr>,
    },
    Panic(RuntimeExpr),
    Fail(RuntimeExpr),
    Bail(RuntimeExpr),
    Ensure {
        condition: RuntimeExpr,
        message: RuntimeExpr,
    },
    Drop {
        target: RuntimeExpr,
        policy: RuntimeDropPolicyExpr,
    },
    Assert {
        guard: RuntimeAssertionGuardId,
        condition: RuntimeExpr,
        message: String,
        profile: RuntimeAssertionProfile,
    },
}

/// Named value argument of a typed log or event effect.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeEffectFieldExpr {
    pub name: String,
    pub value: RuntimeExpr,
}

/// Failure to materialize a typed effect from evaluated runtime values.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeEffectMaterializeError {
    #[error("runtime effect expected {expected} evaluated arguments, found {actual}")]
    ArgumentCount { expected: usize, actual: usize },
    #[error("runtime assertion condition must evaluate to Bool")]
    AssertionConditionNotBool,
}

/// Materialized policy applied to every affine handle leaf in one dropped
/// value graph.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeDropPolicy {
    Default,
    Cancel,
    Stop { fade: LogicalDuration },
    Finish,
    Release,
    Detach,
}

impl RuntimeDropPolicy {
    #[must_use]
    pub const fn kind(self) -> RuntimeDropPolicyKind {
        match self {
            Self::Default => RuntimeDropPolicyKind::Default,
            Self::Cancel => RuntimeDropPolicyKind::Cancel,
            Self::Stop { .. } => RuntimeDropPolicyKind::Stop,
            Self::Finish => RuntimeDropPolicyKind::Finish,
            Self::Release => RuntimeDropPolicyKind::Release,
            Self::Detach => RuntimeDropPolicyKind::Detach,
        }
    }

    pub const fn kind_label(self) -> &'static str {
        self.kind().label()
    }
}

/// Payload-independent identity used for typed drop-policy projections.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeDropPolicyKind {
    Default,
    Cancel,
    Stop,
    Finish,
    Release,
    Detach,
}

impl RuntimeDropPolicyKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Cancel => "cancel",
            Self::Stop => "stop",
            Self::Finish => "finish",
            Self::Release => "release",
            Self::Detach => "detach",
        }
    }
}

/// Evaluated policy form retained in a runtime plan before a dynamic Stop fade
/// is materialized.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeDropPolicyExpr {
    Default,
    Cancel,
    Stop { fade: RuntimeExpr },
    Finish,
    Release,
    Detach,
}

impl RuntimeDropPolicyExpr {
    #[must_use]
    pub const fn kind(&self) -> RuntimeDropPolicyKind {
        match self {
            Self::Default => RuntimeDropPolicyKind::Default,
            Self::Cancel => RuntimeDropPolicyKind::Cancel,
            Self::Stop { .. } => RuntimeDropPolicyKind::Stop,
            Self::Finish => RuntimeDropPolicyKind::Finish,
            Self::Release => RuntimeDropPolicyKind::Release,
            Self::Detach => RuntimeDropPolicyKind::Detach,
        }
    }

    pub const fn kind_label(&self) -> &'static str {
        self.kind().label()
    }
}

impl RuntimeEffectExpr {
    /// Returns value expressions in the stable ABI order consumed by
    /// `materialize` and AWBC `EmitEffect.args`.
    pub fn argument_exprs(&self) -> Vec<&RuntimeExpr> {
        match self {
            Self::Log {
                message, fields, ..
            } => std::iter::once(message)
                .chain(fields.iter().map(|field| &field.value))
                .collect(),
            Self::SignalWrite { target, value } | Self::MetricWrite { target, value } => {
                vec![target, value]
            }
            Self::EmitEvent { event, fields } => std::iter::once(event)
                .chain(fields.iter().map(|field| &field.value))
                .collect(),
            Self::Panic(message) | Self::Fail(message) | Self::Bail(message) => vec![message],
            Self::Ensure { condition, message } => vec![condition, message],
            Self::Drop { target, policy } => {
                let mut expressions = vec![target];
                if let RuntimeDropPolicyExpr::Stop { fade } = policy {
                    expressions.push(fade);
                }
                expressions
            }
            Self::Assert { condition, .. } => vec![condition],
        }
    }

    /// Mutable counterpart of `argument_exprs`, preserving the same ABI order.
    pub fn argument_exprs_mut(&mut self) -> Vec<&mut RuntimeExpr> {
        match self {
            Self::Log {
                message, fields, ..
            } => {
                let mut expressions = Vec::with_capacity(fields.len().saturating_add(1));
                expressions.push(message);
                expressions.extend(fields.iter_mut().map(|field| &mut field.value));
                expressions
            }
            Self::SignalWrite { target, value } | Self::MetricWrite { target, value } => {
                vec![target, value]
            }
            Self::EmitEvent { event, fields } => {
                let mut expressions = Vec::with_capacity(fields.len().saturating_add(1));
                expressions.push(event);
                expressions.extend(fields.iter_mut().map(|field| &mut field.value));
                expressions
            }
            Self::Panic(message) | Self::Fail(message) | Self::Bail(message) => vec![message],
            Self::Ensure { condition, message } => vec![condition, message],
            Self::Drop { target, policy } => {
                let mut expressions = vec![target];
                if let RuntimeDropPolicyExpr::Stop { fade } = policy {
                    expressions.push(fade);
                }
                expressions
            }
            Self::Assert { condition, .. } => vec![condition],
        }
    }

    /// Returns the host-effect descriptor for expressions that use the host
    /// request ABI. Typed `Drop` expressions are executed by the owning
    /// runtime instruction and therefore have no host-effect descriptor.
    pub fn host_descriptor(&self) -> Option<LineEffectRequest> {
        let descriptor = match self {
            Self::Drop { .. } => return None,
            Self::Log { level, fields, .. } => LineEffectRequest::Log(RuntimeLog {
                level: level.clone(),
                message: String::new(),
                fields: empty_runtime_fields(fields),
            }),
            Self::SignalWrite { .. } => LineEffectRequest::SignalWrite(empty_runtime_assignment()),
            Self::MetricWrite { .. } => LineEffectRequest::MetricWrite(empty_runtime_assignment()),
            Self::EmitEvent { fields, .. } => LineEffectRequest::EmitEvent(RuntimeEvent {
                event: String::new(),
                fields: empty_runtime_fields(fields),
            }),
            Self::Panic(_) => LineEffectRequest::Panic(String::new()),
            Self::Fail(_) => LineEffectRequest::Fail(String::new()),
            Self::Bail(_) => LineEffectRequest::Bail(String::new()),
            Self::Ensure { .. } => LineEffectRequest::Ensure {
                condition: String::new(),
                message: String::new(),
            },
            Self::Assert {
                guard,
                message,
                profile,
                ..
            } => LineEffectRequest::Assert(RuntimeAssertion::new(
                *guard,
                String::new(),
                message.clone(),
                *profile,
            )),
        };
        Some(descriptor)
    }

    /// Materializes the host-facing effect after its arguments have been
    /// evaluated by the current fiber.
    ///
    /// Typed `Drop` expressions produce no host request because their owning
    /// instruction performs the drop. A successful assertion likewise
    /// produces no host request. A failed assertion is the only path that
    /// materializes `LineEffectRequest::Assert`; hosts can therefore
    /// construct failure data without parsing the condition label.
    pub fn materialize(
        &self,
        values: &[RuntimeValue],
    ) -> Result<Option<LineEffectRequest>, RuntimeEffectMaterializeError> {
        if self.host_descriptor().is_none() {
            return Ok(None);
        }
        let expected = self.argument_exprs().len();
        if values.len() != expected {
            return Err(RuntimeEffectMaterializeError::ArgumentCount {
                expected,
                actual: values.len(),
            });
        }
        if matches!(self, Self::Assert { .. }) {
            let RuntimeValue::Bool(condition) = &values[0] else {
                return Err(RuntimeEffectMaterializeError::AssertionConditionNotBool);
            };
            if *condition {
                return Ok(None);
            }
        }
        let labels = values.iter().map(runtime_value_label).collect::<Vec<_>>();
        Ok(Some(match self {
            Self::Log { level, fields, .. } => LineEffectRequest::Log(RuntimeLog {
                level: level.clone(),
                message: labels[0].clone(),
                fields: materialized_fields(fields, &labels[1..]),
            }),
            Self::SignalWrite { .. } => LineEffectRequest::SignalWrite(RuntimeAssignment {
                target: labels[0].clone(),
                value: labels[1].clone(),
            }),
            Self::MetricWrite { .. } => LineEffectRequest::MetricWrite(RuntimeAssignment {
                target: labels[0].clone(),
                value: labels[1].clone(),
            }),
            Self::EmitEvent { fields, .. } => LineEffectRequest::EmitEvent(RuntimeEvent {
                event: labels[0].clone(),
                fields: materialized_fields(fields, &labels[1..]),
            }),
            Self::Panic(_) => LineEffectRequest::Panic(labels[0].clone()),
            Self::Fail(_) => LineEffectRequest::Fail(labels[0].clone()),
            Self::Bail(_) => LineEffectRequest::Bail(labels[0].clone()),
            Self::Ensure { .. } => LineEffectRequest::Ensure {
                condition: labels[0].clone(),
                message: labels[1].clone(),
            },
            Self::Drop { .. } => return Ok(None),
            Self::Assert {
                guard,
                message,
                profile,
                ..
            } => LineEffectRequest::Assert(RuntimeAssertion::new(
                *guard,
                labels[0].clone(),
                message.clone(),
                *profile,
            )),
        }))
    }
}

fn empty_runtime_assignment() -> RuntimeAssignment {
    RuntimeAssignment {
        target: String::new(),
        value: String::new(),
    }
}

fn empty_runtime_fields(fields: &[RuntimeEffectFieldExpr]) -> Vec<RuntimeField> {
    fields
        .iter()
        .map(|field| RuntimeField {
            name: field.name.clone(),
            value: String::new(),
        })
        .collect()
}

fn materialized_fields(fields: &[RuntimeEffectFieldExpr], labels: &[String]) -> Vec<RuntimeField> {
    fields
        .iter()
        .zip(labels)
        .map(|(field, value)| RuntimeField {
            name: field.name.clone(),
            value: value.clone(),
        })
        .collect()
}

/// Failed runtime assertion request emitted by ordinary `assert(...)` calls.
///
/// Typed condition evaluation happens before this host boundary. The core
/// remains Sans I/O, while the host/test runner chooses how the failure is
/// logged, traced, or surfaced.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeAssertion {
    guard: RuntimeAssertionGuardId,
    condition: String,
    message: String,
    profile: RuntimeAssertionProfile,
}

impl RuntimeAssertion {
    pub fn new(
        guard: RuntimeAssertionGuardId,
        condition: String,
        message: String,
        profile: RuntimeAssertionProfile,
    ) -> Self {
        Self {
            guard,
            condition,
            message,
            profile,
        }
    }

    pub const fn guard(&self) -> RuntimeAssertionGuardId {
        self.guard
    }

    pub fn condition(&self) -> &str {
        &self.condition
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn profile(&self) -> RuntimeAssertionProfile {
        self.profile
    }
}

/// Persistable assertion failure produced by a runtime host.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeAssertionFailure {
    assertion: RuntimeAssertion,
}

impl RuntimeAssertionFailure {
    pub const fn new(assertion: RuntimeAssertion) -> Self {
        Self { assertion }
    }

    pub const fn assertion(&self) -> &RuntimeAssertion {
        &self.assertion
    }

    pub fn into_assertion(self) -> RuntimeAssertion {
        self.assertion
    }
}

/// Profile policy for runtime assertions.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
