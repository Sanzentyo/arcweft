#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use arcweft_data::{
    DataError, DataErrorKind, FieldShape, Number, RecordPolicy, Result, TypeShape, Value,
    encode_with_shape,
};

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
    pub source: Option<String>,
    pub value: Value,
}

impl ConfigLayer {
    #[must_use]
    pub fn new(kind: ConfigLayerKind, value: Value) -> Self {
        Self {
            kind,
            source: None,
            value,
        }
    }

    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }
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

#[derive(Clone, Debug, PartialEq)]
pub struct ConfigMergeReport {
    pub value: Value,
    pub provenance: BTreeMap<String, ConfigFieldProvenance>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigFieldProvenance {
    pub path: String,
    pub layer_index: usize,
    pub layer_kind: ConfigLayerKind,
    pub source: Option<String>,
}

struct MergeContext<'a> {
    policy: &'a ConfigMergePolicy,
    provenance: BTreeMap<String, ConfigFieldProvenance>,
    layer_index: usize,
    layer_kind: ConfigLayerKind,
    source: Option<String>,
}

pub fn merge_config_layers(
    layers: impl IntoIterator<Item = ConfigLayer>,
    shape: &TypeShape,
    policy: &ConfigMergePolicy,
) -> Result<ConfigMergeReport> {
    let mut provenance = BTreeMap::new();
    let value = layers
        .into_iter()
        .enumerate()
        .try_fold(None, |base, (layer_index, layer)| {
            let mut context = MergeContext {
                policy,
                provenance: BTreeMap::new(),
                layer_index,
                layer_kind: layer.kind,
                source: layer.source,
            };
            let merged = merge_value(base, layer.value, shape, &mut Vec::new(), &mut context)?;
            provenance.extend(context.provenance);
            Ok::<_, DataError>(Some(merged))
        })?
        .unwrap_or(empty_value(shape)?);
    let mut value = value;
    finalize_value(&mut value, shape, &mut Vec::new())?;
    Ok(ConfigMergeReport { value, provenance })
}

fn merge_value(
    base: Option<Value>,
    incoming: Value,
    shape: &TypeShape,
    path: &mut Vec<String>,
    context: &mut MergeContext<'_>,
) -> Result<Value> {
    match shape {
        TypeShape::Record { fields, policy, .. } => {
            merge_record(base, incoming, fields, *policy, path, context)
        }
        TypeShape::Map { key, value } => merge_map(base, incoming, key, value, path, context),
        TypeShape::Seq(item_shape) => merge_sequence(base, incoming, item_shape, path, context),
        _ => {
            validate_scalar(&incoming, shape)?;
            record_provenance(path, context);
            Ok(incoming)
        }
    }
}

fn merge_record(
    base: Option<Value>,
    incoming: Value,
    fields: &[FieldShape],
    record_policy: RecordPolicy,
    path: &mut Vec<String>,
    context: &mut MergeContext<'_>,
) -> Result<Value> {
    let incoming_fields = match incoming {
        Value::Record(fields) => fields,
        other => return Err(DataError::invalid_type("record config", other.type_name())),
    };
    reject_unknown_record_fields(&incoming_fields, fields, record_policy, context)?;
    let mut merged = match base {
        Some(Value::Record(fields)) => fields,
        Some(other) => return Err(DataError::invalid_type("record config", other.type_name())),
        None => BTreeMap::new(),
    };
    incoming_fields.into_iter().try_for_each(|(key, value)| {
        let field = field_shape(fields, &key)?;
        path.push(key.clone());
        let current = merged.remove(&key);
        let merged_value = merge_value(current, value, &field.value_shape(), path, context)
            .map_err(|error| error.at_field(key.clone()))?;
        path.pop();
        merged.insert(key, merged_value);
        Ok(())
    })?;
    Ok(Value::Record(merged))
}

fn merge_map(
    base: Option<Value>,
    incoming: Value,
    key_shape: &TypeShape,
    value_shape: &TypeShape,
    path: &mut Vec<String>,
    context: &mut MergeContext<'_>,
) -> Result<Value> {
    if !matches!(key_shape, TypeShape::String) {
        return Err(DataError::unsupported(
            "config merge supports string map keys only",
        ));
    }
    let incoming_entries = match incoming {
        Value::Map(entries) => entries,
        other => return Err(DataError::invalid_type("map config", other.type_name())),
    };
    let mut merged = match base {
        Some(Value::Map(entries)) => entries,
        Some(other) => return Err(DataError::invalid_type("map config", other.type_name())),
        None => BTreeMap::new(),
    };
    incoming_entries.into_iter().try_for_each(|(key, value)| {
        path.push(key.clone());
        let current = merged.remove(&key);
        let merged_value = merge_value(current, value, value_shape, path, context)
            .map_err(|error| error.at_field(key.clone()))?;
        path.pop();
        merged.insert(key, merged_value);
        Ok(())
    })?;
    Ok(Value::Map(merged))
}

fn merge_sequence(
    base: Option<Value>,
    incoming: Value,
    item_shape: &TypeShape,
    path: &mut Vec<String>,
    context: &mut MergeContext<'_>,
) -> Result<Value> {
    let incoming_items = match incoming {
        Value::Seq(items) => items,
        other => {
            return Err(DataError::invalid_type(
                "sequence config",
                other.type_name(),
            ));
        }
    };
    let mut merged = match (base, context.policy.list_strategy) {
        (Some(Value::Seq(items)), ListMergeStrategy::Append) => items,
        (Some(Value::Seq(_)), ListMergeStrategy::Replace) | (None, _) => Vec::new(),
        (Some(other), _) => {
            return Err(DataError::invalid_type(
                "sequence config",
                other.type_name(),
            ));
        }
    };
    let start_index = merged.len();
    incoming_items
        .into_iter()
        .enumerate()
        .try_for_each(|(offset, value)| {
            let index = start_index + offset;
            path.push(format!("[{index}]"));
            validate_scalar_or_nested(&value, item_shape, path, context)
                .map_err(|error| error.at_index(index))?;
            path.pop();
            merged.push(value);
            Ok(())
        })?;
    record_provenance(path, context);
    Ok(Value::Seq(merged))
}

fn validate_scalar_or_nested(
    value: &Value,
    shape: &TypeShape,
    path: &mut Vec<String>,
    context: &mut MergeContext<'_>,
) -> Result<()> {
    match shape {
        TypeShape::Record { .. } | TypeShape::Map { .. } | TypeShape::Seq(_) => {
            let _ = merge_value(None, value.clone(), shape, path, context)?;
            Ok(())
        }
        _ => validate_scalar(value, shape).map(|()| record_provenance(path, context)),
    }
}

fn reject_unknown_record_fields(
    values: &BTreeMap<String, Value>,
    fields: &[FieldShape],
    record_policy: RecordPolicy,
    context: &MergeContext<'_>,
) -> Result<()> {
    if !(context.policy.deny_unknown_fields || record_policy.deny_unknown_fields) {
        return Ok(());
    }
    let known = fields
        .iter()
        .filter(|field| !field.skip)
        .map(|field| field.wire_name.as_str())
        .collect::<BTreeSet<_>>();
    values
        .keys()
        .find(|key| !known.contains(key.as_str()))
        .map(|key| {
            DataError::new(
                DataErrorKind::UnknownField,
                format!("unknown config field `{key}`"),
            )
            .at_field(key.clone())
        })
        .map_or(Ok(()), Err)
}

fn field_shape<'a>(fields: &'a [FieldShape], key: &str) -> Result<&'a FieldShape> {
    fields
        .iter()
        .find(|field| !field.skip && field.wire_name == key)
        .ok_or_else(|| {
            DataError::new(
                DataErrorKind::UnknownField,
                format!("unknown config field `{key}`"),
            )
            .at_field(key.to_owned())
        })
}

fn validate_scalar(value: &Value, shape: &TypeShape) -> Result<()> {
    match shape {
        TypeShape::Option(inner) => match value {
            Value::Unit => Ok(()),
            other => validate_scalar(other, inner),
        },
        TypeShape::F32 => finite_float(value, "f32"),
        TypeShape::F64 => finite_float(value, "f64"),
        _ => encode_with_shape(value, shape).map(|_| ()),
    }
}

fn finite_float(value: &Value, expected: &str) -> Result<()> {
    match value {
        Value::Number(Number::F32(value)) if expected == "f32" && value.is_finite() => Ok(()),
        Value::Number(Number::F64(value)) if expected == "f64" && value.is_finite() => Ok(()),
        Value::Number(Number::F32(_) | Number::F64(_)) => Err(DataError::new(
            DataErrorKind::InvalidEncoding,
            "config floats must be finite",
        )),
        other => Err(DataError::invalid_type(expected, other.type_name())),
    }
}

fn finalize_value(value: &mut Value, shape: &TypeShape, path: &mut Vec<String>) -> Result<()> {
    match (value, shape) {
        (Value::Record(values), TypeShape::Record { fields, .. }) => fields
            .iter()
            .filter(|field| !field.skip)
            .try_for_each(|field| {
                path.push(field.wire_name.clone());
                if let Some(value) = values.get_mut(&field.wire_name) {
                    finalize_value(value, &field.value_shape(), path)
                        .map_err(|error| error.at_field(field.wire_name.clone()))?;
                    path.pop();
                    return Ok(());
                }
                let field_shape = field.value_shape();
                if matches!(field_shape, TypeShape::Option(_)) {
                    values.insert(field.wire_name.clone(), Value::Unit);
                    path.pop();
                    return Ok(());
                }
                if field.has_default {
                    path.pop();
                    return Ok(());
                }
                path.pop();
                Err(DataError::new(
                    DataErrorKind::MissingField,
                    format!("missing config field `{}`", field.wire_name),
                )
                .at_field(field.wire_name.clone()))
            }),
        (Value::Map(values), TypeShape::Map { value: shape, .. }) => {
            values.iter_mut().try_for_each(|(key, value)| {
                path.push(key.clone());
                let result =
                    finalize_value(value, shape, path).map_err(|error| error.at_field(key.clone()));
                path.pop();
                result
            })
        }
        (Value::Seq(values), TypeShape::Seq(shape)) => {
            values
                .iter_mut()
                .enumerate()
                .try_for_each(|(index, value)| {
                    path.push(format!("[{index}]"));
                    let result =
                        finalize_value(value, shape, path).map_err(|error| error.at_index(index));
                    path.pop();
                    result
                })
        }
        (value, shape) => validate_scalar(value, shape),
    }
}

fn empty_value(shape: &TypeShape) -> Result<Value> {
    match shape {
        TypeShape::Record { .. } => Ok(Value::Record(BTreeMap::new())),
        TypeShape::Map { .. } => Ok(Value::Map(BTreeMap::new())),
        TypeShape::Seq(_) => Ok(Value::Seq(Vec::new())),
        TypeShape::Option(_) | TypeShape::Unit => Ok(Value::Unit),
        other => Err(DataError::new(
            DataErrorKind::MissingField,
            format!("missing required config value for {}", other.type_name()),
        )),
    }
}

fn record_provenance(path: &[String], context: &mut MergeContext<'_>) {
    let path = config_path(path);
    context.provenance.insert(
        path.clone(),
        ConfigFieldProvenance {
            path,
            layer_index: context.layer_index,
            layer_kind: context.layer_kind.clone(),
            source: context.source.clone(),
        },
    );
}

fn config_path(path: &[String]) -> String {
    if path.is_empty() {
        return "$".to_owned();
    }
    let mut out = "$".to_owned();
    for segment in path {
        if segment.starts_with('[') {
            out.push_str(segment);
        } else {
            out.push('.');
            out.push_str(segment);
        }
    }
    out
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
