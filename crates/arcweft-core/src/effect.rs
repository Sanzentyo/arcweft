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

/// Effect request whose value arguments are evaluated at the owning runtime
/// instruction instead of being preserved as source-text labels.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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
    Assert {
        condition: RuntimeExpr,
        message: RuntimeExpr,
        profile: RuntimeAssertionProfile,
    },
}

/// Named value argument of a typed log or event effect.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeEffectFieldExpr {
    pub name: String,
    pub value: RuntimeExpr,
}

/// Failure to materialize a typed effect from evaluated runtime values.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("runtime effect expected {expected} evaluated arguments, found {actual}")]
pub struct RuntimeEffectMaterializeError {
    expected: usize,
    actual: usize,
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
            Self::Ensure { condition, message }
            | Self::Assert {
                condition, message, ..
            } => vec![condition, message],
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
            Self::Ensure { condition, message }
            | Self::Assert {
                condition, message, ..
            } => vec![condition, message],
        }
    }

    /// Static descriptor interned by structured and AWBC runtimes. Dynamic
    /// values are deliberately absent and travel through the argument ABI.
    pub fn descriptor(&self) -> LineEffectRequest {
        match self {
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
            Self::Assert { profile, .. } => LineEffectRequest::Assert(RuntimeAssertion {
                condition: String::new(),
                message: String::new(),
                profile: *profile,
            }),
        }
    }

    /// Materializes the host-facing effect after its arguments have been
    /// evaluated by the current fiber.
    pub fn materialize(
        &self,
        values: &[RuntimeValue],
    ) -> Result<LineEffectRequest, RuntimeEffectMaterializeError> {
        let expected = self.argument_exprs().len();
        if values.len() != expected {
            return Err(RuntimeEffectMaterializeError {
                expected,
                actual: values.len(),
            });
        }
        let labels = values.iter().map(runtime_value_label).collect::<Vec<_>>();
        Ok(match self {
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
            Self::Assert { profile, .. } => LineEffectRequest::Assert(RuntimeAssertion {
                condition: labels[0].clone(),
                message: labels[1].clone(),
                profile: *profile,
            }),
        })
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
