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
        value: Box<AgentValue>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compare_predicate_keeps_flat_wire_value() {
        let predicate = Predicate::Compare {
            probe: Probe::Signal {
                target: PublicId::new("signal.ready").expect("valid public id"),
            },
            op: CompareOp::Eq,
            value: Box::new(AgentValue::Bool(true)),
        };

        let value = serde_json::to_value(predicate).expect("serializes predicate");

        assert_eq!(
            value,
            json!({
                "kind": "compare",
                "probe": {
                    "kind": "signal",
                    "target": "signal.ready"
                },
                "op": "eq",
                "value": {
                    "kind": "bool",
                    "value": true
                }
            })
        );
    }
}
