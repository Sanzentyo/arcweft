use crate::{ids::PublicId, value::AgentValue};
use serde::{Deserialize, Serialize};

/// A typed observation value source evaluated by the Agent runner.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Probe {
    Signal { target: PublicId },
    Metric { target: PublicId },
    StatePath { path: String },
    ObservationField { path: String },
}

/// Comparison operation for a typed probe.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompareOp {
    Eq,
    NotEq,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
}

/// Serializable predicate lowered from typed Agent Script expressions.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Predicate {
    Compare {
        probe: Probe,
        op: CompareOp,
        value: AgentValue,
    },
    Exists {
        probe: Probe,
    },
    ActionEnabled {
        target: PublicId,
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
