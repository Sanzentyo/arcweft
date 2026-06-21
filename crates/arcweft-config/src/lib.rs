#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use arcweft_data::{DataError, DataErrorKind, Result, Value};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigLayerKind {
    Defaults,
    File,
    Environment,
    CommandLine,
    Remote,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConfigLayer {
    pub kind: ConfigLayerKind,
    pub value: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigMergePolicy {
    pub deny_unknown_fields: bool,
    pub list_strategy: ListMergeStrategy,
    pub redact_keys: BTreeSet<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ListMergeStrategy {
    #[default]
    Replace,
    Append,
}

impl Default for ConfigMergePolicy {
    fn default() -> Self {
        Self {
            deny_unknown_fields: true,
            list_strategy: ListMergeStrategy::Replace,
            redact_keys: ["token", "secret", "password", "key"]
                .into_iter()
                .map(String::from)
                .collect(),
        }
    }
}

pub fn merge_config_layers(
    layers: impl IntoIterator<Item = ConfigLayer>,
    policy: &ConfigMergePolicy,
) -> Result<Value> {
    layers
        .into_iter()
        .try_fold(Value::Record(BTreeMap::new()), |base, layer| {
            merge_values(base, layer.value, policy)
        })
}

pub fn merge_values(
    base: Value,
    override_value: Value,
    policy: &ConfigMergePolicy,
) -> Result<Value> {
    match (base, override_value) {
        (Value::Record(mut base), Value::Record(override_map)) => {
            override_map.into_iter().try_for_each(|(key, value)| {
                let merged = match base.remove(&key) {
                    Some(current) => merge_values(current, value, policy)?,
                    None => value,
                };
                base.insert(key, merged);
                Ok(())
            })?;
            Ok(Value::Record(base))
        }
        (Value::Map(mut base), Value::Map(override_map)) => {
            override_map.into_iter().try_for_each(|(key, value)| {
                let merged = match base.remove(&key) {
                    Some(current) => merge_values(current, value, policy)?,
                    None => value,
                };
                base.insert(key, merged);
                Ok(())
            })?;
            Ok(Value::Map(base))
        }
        (Value::Seq(mut base), Value::Seq(mut override_seq))
            if policy.list_strategy == ListMergeStrategy::Append =>
        {
            base.append(&mut override_seq);
            Ok(Value::Seq(base))
        }
        (_, override_value) => Ok(override_value),
    }
}

pub fn redact(value: &Value, policy: &ConfigMergePolicy) -> Value {
    match value {
        Value::Record(fields) => Value::Record(redact_map(fields, policy)),
        Value::Map(fields) => Value::Map(redact_map(fields, policy)),
        Value::Seq(values) => {
            Value::Seq(values.iter().map(|value| redact(value, policy)).collect())
        }
        other => other.clone(),
    }
}

fn redact_map(
    fields: &BTreeMap<String, Value>,
    policy: &ConfigMergePolicy,
) -> BTreeMap<String, Value> {
    fields
        .iter()
        .map(|(key, value)| {
            let value = if policy
                .redact_keys
                .iter()
                .any(|needle| key.to_ascii_lowercase().contains(needle))
            {
                Value::String("<redacted>".to_owned())
            } else {
                redact(value, policy)
            };
            (key.clone(), value)
        })
        .collect()
}

pub fn require_record(value: &Value) -> Result<&BTreeMap<String, Value>> {
    match value {
        Value::Record(fields) => Ok(fields),
        other => Err(DataError::new(
            DataErrorKind::InvalidType,
            format!("expected record config, found {}", other.type_name()),
        )),
    }
}
