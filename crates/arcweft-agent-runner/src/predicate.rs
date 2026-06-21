use std::collections::BTreeMap;

use arcweft_agent_protocol::{
    predicate::{CompareOp, Predicate, Probe},
    protocol::ObservationEnvelope,
    value::AgentValue,
};

pub(crate) fn predicate_matches(predicate: &Predicate, observation: &ObservationEnvelope) -> bool {
    match predicate {
        Predicate::Compare { probe, op, value } => observation_value(probe, observation)
            .is_some_and(|actual| compare_values(&actual, *op, value)),
        Predicate::Exists { probe } => observation_value(probe, observation).is_some(),
        Predicate::ActionEnabled { target } => observation
            .actions
            .iter()
            .any(|action| action.enabled && action.target == target.as_str()),
        Predicate::DiagnosticsHasError => diagnostics_has_error(observation),
        Predicate::All { predicates } => predicates
            .iter()
            .all(|predicate| predicate_matches(predicate, observation)),
        Predicate::Any { predicates } => predicates
            .iter()
            .any(|predicate| predicate_matches(predicate, observation)),
        Predicate::Not { predicate } => !predicate_matches(predicate, observation),
    }
}

fn diagnostics_has_error(observation: &ObservationEnvelope) -> bool {
    observation
        .payload
        .get("diagnostics")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|diagnostics| {
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .get("severity")
                    .and_then(serde_json::Value::as_str)
                    == Some("error")
            })
        })
}

fn observation_value(probe: &Probe, observation: &ObservationEnvelope) -> Option<AgentValue> {
    match probe {
        Probe::Signal { target } | Probe::Metric { target } => {
            observation.signals.get(target.as_str()).cloned()
        }
        Probe::StatePath { path } => observation
            .payload
            .get("state")
            .and_then(|state| json_path_value(state, path.as_str()))
            .and_then(agent_value_from_json),
        Probe::ObservationField { path } if path.as_str() == "tick" => {
            Some(AgentValue::I64(i64::try_from(observation.tick).ok()?))
        }
        Probe::ObservationField { path } if path.as_str() == "frame_id" => {
            Some(AgentValue::String(observation.frame_id.clone()))
        }
        Probe::ObservationField { path } if path.as_str() == "state_hash" => {
            Some(AgentValue::String(observation.state_hash.clone()))
        }
        Probe::ObservationField { path } if path.as_str() == "render_hash" => {
            Some(AgentValue::String(observation.render_hash.clone()))
        }
        Probe::ObservationField { path } => path
            .as_str()
            .strip_prefix("signals.")
            .and_then(|signal| observation.signals.get(signal).cloned())
            .or_else(|| {
                json_path_value(&observation.payload, path.as_str()).and_then(agent_value_from_json)
            }),
    }
}

fn json_path_value<'a>(root: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    if let Some(value) = root.get(path) {
        return Some(value);
    }
    path.split('.')
        .try_fold(root, |value, segment| value.get(segment))
}

fn agent_value_from_json(value: &serde_json::Value) -> Option<AgentValue> {
    Some(match value {
        serde_json::Value::Null => AgentValue::Null,
        serde_json::Value::Bool(value) => AgentValue::Bool(*value),
        serde_json::Value::Number(value) => value
            .as_i64()
            .map(AgentValue::I64)
            .or_else(|| value.as_u64().map(AgentValue::U64))
            .or_else(|| value.as_f64().map(AgentValue::F64))?,
        serde_json::Value::String(value) => AgentValue::String(value.clone()),
        serde_json::Value::Array(values) => AgentValue::List(
            values
                .iter()
                .map(agent_value_from_json)
                .collect::<Option<Vec<_>>>()?,
        ),
        serde_json::Value::Object(values) => AgentValue::Map(
            values
                .iter()
                .map(|(key, value)| Some((key.clone(), agent_value_from_json(value)?)))
                .collect::<Option<BTreeMap<_, _>>>()?,
        ),
    })
}

fn compare_values(actual: &AgentValue, op: CompareOp, expected: &AgentValue) -> bool {
    match op {
        CompareOp::Eq => agent_values_equal(actual, expected),
        CompareOp::NotEq => !agent_values_equal(actual, expected),
        CompareOp::Greater => {
            compare_numeric_values(actual, expected).is_some_and(i32::is_positive)
        }
        CompareOp::GreaterOrEqual => {
            compare_numeric_values(actual, expected).is_some_and(|order| order >= 0)
        }
        CompareOp::Less => compare_numeric_values(actual, expected).is_some_and(i32::is_negative),
        CompareOp::LessOrEqual => {
            compare_numeric_values(actual, expected).is_some_and(|order| order <= 0)
        }
    }
}

fn agent_values_equal(left: &AgentValue, right: &AgentValue) -> bool {
    match (left, right) {
        (AgentValue::Entity(left), AgentValue::String(right))
        | (AgentValue::String(right), AgentValue::Entity(left)) => left.as_str() == right,
        _ => left == right,
    }
}

fn compare_numeric_values(left: &AgentValue, right: &AgentValue) -> Option<i32> {
    Some(match (left, right) {
        (AgentValue::I64(left), AgentValue::I64(right)) => compare_order(left.cmp(right)),
        (AgentValue::U64(left), AgentValue::U64(right)) => compare_order(left.cmp(right)),
        (AgentValue::I64(left), AgentValue::U64(right)) => {
            if *left < 0 {
                -1
            } else {
                compare_order(u64::try_from(*left).ok()?.cmp(right))
            }
        }
        (AgentValue::U64(left), AgentValue::I64(right)) => {
            if *right < 0 {
                1
            } else {
                compare_order(left.cmp(&u64::try_from(*right).ok()?))
            }
        }
        (AgentValue::F64(left), AgentValue::F64(right)) => compare_order(left.partial_cmp(right)?),
        _ => return None,
    })
}

fn compare_order(order: std::cmp::Ordering) -> i32 {
    match order {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}
