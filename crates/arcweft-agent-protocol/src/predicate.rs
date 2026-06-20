use crate::{ids::PublicId, value::AgentValue};
use serde::{Deserialize, Serialize};

/// A first-class Agent debug-state path.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct DebugStatePath(String);

impl DebugStatePath {
    pub fn new(path: impl Into<String>) -> Result<Self, String> {
        let path = path.into();
        if path.trim().is_empty() {
            Err("debug state path must not be empty".to_owned())
        } else {
            Ok(Self(path))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A first-class Agent observation-field path.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ObservationFieldPath(String);

impl ObservationFieldPath {
    pub fn new(path: impl Into<String>) -> Result<Self, String> {
        let path = path.into();
        if path.trim().is_empty() {
            Err("observation field path must not be empty".to_owned())
        } else {
            Ok(Self(path))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A typed observation value source evaluated by the Agent runner.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Probe {
    Signal { target: PublicId },
    Metric { target: PublicId },
    StatePath { path: DebugStatePath },
    ObservationField { path: ObservationFieldPath },
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

    #[test]
    fn debug_and_observation_paths_keep_string_wire_shape() {
        let predicate = Predicate::All {
            predicates: vec![
                Predicate::Exists {
                    probe: Probe::StatePath {
                        path: DebugStatePath::new("route.phase").expect("valid state path"),
                    },
                },
                Predicate::Exists {
                    probe: Probe::ObservationField {
                        path: ObservationFieldPath::new("tick")
                            .expect("valid observation field path"),
                    },
                },
            ],
        };

        let value = serde_json::to_value(predicate).expect("serializes typed paths");

        assert_eq!(
            value,
            json!({
                "kind": "all",
                "predicates": [
                    {
                        "kind": "exists",
                        "probe": {
                            "kind": "state_path",
                            "path": "route.phase"
                        }
                    },
                    {
                        "kind": "exists",
                        "probe": {
                            "kind": "observation_field",
                            "path": "tick"
                        }
                    }
                ]
            })
        );
    }

    #[test]
    fn typed_paths_reject_empty_values() {
        assert!(DebugStatePath::new(" ").is_err());
        assert!(ObservationFieldPath::new("").is_err());
    }
}
