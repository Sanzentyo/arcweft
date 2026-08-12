//! Role-specific wrappers for canonical runtime-typed dialogue values.

use super::{
    CharacterDialogueConfig, CharacterDialogueValueError, PRODUCTION_CHARACTER_DIALOGUE_LIMITS,
    limits::MAX_TYPED_AGGREGATE_BYTES,
};
use crate::{FallbackStylePolicy, InlineFailurePolicy, InlineFallback};
use arcweft_core::{
    entry::{RuntimeNominalTypeId, RuntimeSchemaError, TypeLayoutHash},
    value::{MAX_RUNTIME_VALUE_NESTING_DEPTH, RuntimeNominalRecordValue, RuntimeSeq, RuntimeValue},
};
use core::hash::{Hash, Hasher};
use serde::{Deserialize, Deserializer, Serialize};

/// Checked lower-layer carrier for one runtime-typed configuration value.
#[derive(Clone, Debug, Serialize)]
pub struct CharacterDialogueTypedValue {
    nominal_type: Option<RuntimeNominalTypeId>,
    layout: TypeLayoutHash,
    value: RuntimeValue,
}

impl<'de> Deserialize<'de> for CharacterDialogueTypedValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SerializedTypedValue {
            nominal_type: Option<RuntimeNominalTypeId>,
            layout: TypeLayoutHash,
            value: RuntimeValue,
        }

        let serialized = SerializedTypedValue::deserialize(deserializer)?;
        Self::try_new(serialized.nominal_type, serialized.layout, serialized.value)
            .map_err(serde::de::Error::custom)
    }
}

impl CharacterDialogueTypedValue {
    /// Validates identity correlation and deterministic canonical encoding.
    pub fn try_new(
        nominal_type: Option<RuntimeNominalTypeId>,
        layout: TypeLayoutHash,
        value: RuntimeValue,
    ) -> Result<Self, CharacterDialogueValueError> {
        let value = normalize_runtime_value(value)?;
        match (&nominal_type, &value) {
            (Some(expected), RuntimeValue::NominalRecord(record))
                if expected == record.type_id() && layout == record.layout() => {}
            (None, RuntimeValue::NominalRecord(record)) => {
                return Err(CharacterDialogueValueError::Field {
                    field: "typed_value",
                    reason: format!(
                        "nominal value `{}` requires its nominal identity",
                        record.type_id().as_str()
                    ),
                });
            }
            (Some(expected), RuntimeValue::NominalRecord(record)) => {
                return Err(CharacterDialogueValueError::Field {
                    field: "typed_value",
                    reason: format!(
                        "declared nominal identity `{}` or layout does not match `{}`",
                        expected.as_str(),
                        record.type_id().as_str()
                    ),
                });
            }
            (Some(expected), _) => {
                return Err(CharacterDialogueValueError::Field {
                    field: "typed_value",
                    reason: format!(
                        "declared nominal identity `{}` requires a nominal record",
                        expected.as_str()
                    ),
                });
            }
            (None, _) => {}
        }
        value
            .validate_nesting_depth(MAX_RUNTIME_VALUE_NESTING_DEPTH)
            .map_err(|_| CharacterDialogueValueError::Limit {
                limit: "runtime_value_nesting_depth",
                maximum: MAX_RUNTIME_VALUE_NESTING_DEPTH,
            })?;
        validate_config_strings(&value)?;
        value.try_canonical_bytes(
            PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_config_encoded_bytes as usize,
        )?;
        Ok(Self {
            nominal_type,
            layout,
            value,
        })
    }

    #[must_use]
    pub const fn nominal_type(&self) -> Option<&RuntimeNominalTypeId> {
        self.nominal_type.as_ref()
    }

    #[must_use]
    pub const fn layout(&self) -> TypeLayoutHash {
        self.layout
    }

    #[must_use]
    pub const fn value(&self) -> &RuntimeValue {
        &self.value
    }

    #[must_use]
    pub fn into_value(self) -> RuntimeValue {
        self.value
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        self.value
            .try_canonical_bytes(
                PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_config_encoded_bytes as usize,
            )
            .expect("validated typed dialogue value remains canonically encodable")
    }
}

impl PartialEq for CharacterDialogueTypedValue {
    fn eq(&self, other: &Self) -> bool {
        self.nominal_type == other.nominal_type
            && self.layout == other.layout
            && self.canonical_bytes() == other.canonical_bytes()
    }
}

impl Eq for CharacterDialogueTypedValue {}

impl Hash for CharacterDialogueTypedValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.nominal_type.hash(state);
        self.layout.hash(state);
        self.canonical_bytes().hash(state);
    }
}

macro_rules! typed_role {
    ($name:ident, $validator:ident) => {
        #[derive(Clone, Debug, Eq, PartialEq, Serialize)]
        #[serde(transparent)]
        pub struct $name(CharacterDialogueTypedValue);

        impl $name {
            pub fn try_new(
                value: CharacterDialogueTypedValue,
            ) -> Result<Self, CharacterDialogueValueError> {
                $validator(&value)?;
                Ok(Self(value))
            }

            #[must_use]
            pub const fn typed(&self) -> &CharacterDialogueTypedValue {
                &self.0
            }

            #[must_use]
            pub fn into_typed(self) -> CharacterDialogueTypedValue {
                self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                Self::try_new(CharacterDialogueTypedValue::deserialize(deserializer)?)
                    .map_err(serde::de::Error::custom)
            }
        }
    };
}

typed_role!(CharacterDialogueStageValue, validate_field_value);
typed_role!(CharacterDialoguePortraitValue, validate_field_value);
typed_role!(CharacterDialogueFocusValue, validate_field_value);
typed_role!(CharacterDialogueCleanupValue, validate_field_value);
typed_role!(CharacterDialogueHookValue, validate_aggregate_value);
typed_role!(CharacterDialogueStyleValue, validate_structured_value);
typed_role!(CharacterDialogueRichTextValue, validate_structured_value);
typed_role!(CharacterDialogueCustomValue, validate_field_value);

pub(super) fn validate_config_value_limits(
    config: &CharacterDialogueConfig,
) -> Result<(), CharacterDialogueValueError> {
    let hook_bytes = config.hooks.iter().try_fold(0_usize, |total, hook| {
        let encoded = hook
            .typed()
            .value()
            .try_canonical_bytes(MAX_TYPED_AGGREGATE_BYTES)?;
        total
            .checked_add(encoded.len())
            .ok_or(CharacterDialogueValueError::Limit {
                limit: "hook_aggregate_bytes",
                maximum: MAX_TYPED_AGGREGATE_BYTES,
            })
    })?;
    if hook_bytes > MAX_TYPED_AGGREGATE_BYTES {
        return Err(CharacterDialogueValueError::Limit {
            limit: "hook_aggregate_bytes",
            maximum: MAX_TYPED_AGGREGATE_BYTES,
        });
    }
    validate_inline_failure(&config.inline_failure)?;
    Ok(())
}

fn validate_inline_failure(
    policy: &InlineFailurePolicy,
) -> Result<(), CharacterDialogueValueError> {
    let Some(fallback) = (match policy {
        InlineFailurePolicy::Fallback { fallback } => Some(fallback),
        InlineFailurePolicy::FailLine | InlineFailurePolicy::Discard => None,
    }) else {
        return Ok(());
    };
    let style = match fallback {
        InlineFallback::Text { text, style } => {
            let maximum = PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_config_string_bytes as usize;
            if text.len() > maximum {
                return Err(CharacterDialogueValueError::Limit {
                    limit: "inline_fallback_text_bytes",
                    maximum,
                });
            }
            style
        }
        InlineFallback::ExprSource { style } | InlineFallback::CallSource { style } => style,
        InlineFallback::ValuePlain => return Ok(()),
    };
    if let FallbackStylePolicy::Apply { styles } = style {
        for style in styles {
            validate_structured_value(style.typed())?;
        }
    }
    Ok(())
}

fn validate_config_strings(value: &RuntimeValue) -> Result<(), CharacterDialogueValueError> {
    let maximum = PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_config_string_bytes as usize;
    match value {
        RuntimeValue::String(value) | RuntimeValue::EntityRef(value) if value.len() > maximum => {
            Err(CharacterDialogueValueError::Limit {
                limit: "config_string_bytes",
                maximum,
            })
        }
        RuntimeValue::Tuple(values) => validate_config_string_values(values),
        RuntimeValue::Seq(sequence) => {
            let values = sequence.clone().into_values();
            validate_config_string_values(&values)
        }
        RuntimeValue::Record(fields) => {
            for field in fields {
                validate_config_strings(field.value())?;
            }
            Ok(())
        }
        RuntimeValue::NominalRecord(record) => validate_config_string_values(record.fields()),
        RuntimeValue::Opaque(value) => validate_config_strings(value.payload()),
        RuntimeValue::Variant {
            payload: Some(payload),
            ..
        } => validate_config_strings(payload),
        RuntimeValue::Unit
        | RuntimeValue::Bool(_)
        | RuntimeValue::Int(_)
        | RuntimeValue::UInt(_)
        | RuntimeValue::F32(_)
        | RuntimeValue::F64(_)
        | RuntimeValue::MatrixF32(_)
        | RuntimeValue::MatrixF64(_)
        | RuntimeValue::TensorF32(_)
        | RuntimeValue::TensorF64(_)
        | RuntimeValue::String(_)
        | RuntimeValue::Char(_)
        | RuntimeValue::Duration(_)
        | RuntimeValue::Range(_)
        | RuntimeValue::Iterator(_)
        | RuntimeValue::Function(_)
        | RuntimeValue::EntityRef(_)
        | RuntimeValue::Variant { payload: None, .. } => Ok(()),
    }
}

fn validate_config_string_values(
    values: &[RuntimeValue],
) -> Result<(), CharacterDialogueValueError> {
    for value in values {
        validate_config_strings(value)?;
    }
    Ok(())
}

fn validate_field_value(
    value: &CharacterDialogueTypedValue,
) -> Result<(), CharacterDialogueValueError> {
    validate_encoded_size(
        value,
        PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_field_value_bytes as usize,
        "field_value_bytes",
    )
}

fn validate_aggregate_value(
    value: &CharacterDialogueTypedValue,
) -> Result<(), CharacterDialogueValueError> {
    validate_encoded_size(value, MAX_TYPED_AGGREGATE_BYTES, "typed_aggregate_bytes")
}

fn validate_structured_value(
    value: &CharacterDialogueTypedValue,
) -> Result<(), CharacterDialogueValueError> {
    validate_aggregate_value(value)?;
    let mut leaves = 0_usize;
    count_structured_leaves(value.value(), 0, &mut leaves)
}

fn validate_encoded_size(
    value: &CharacterDialogueTypedValue,
    maximum: usize,
    limit: &'static str,
) -> Result<(), CharacterDialogueValueError> {
    match value.value().try_canonical_bytes(maximum) {
        Ok(_) => Ok(()),
        Err(RuntimeSchemaError::BudgetExceeded {
            budget: "encoded_bytes",
        }) => Err(CharacterDialogueValueError::Limit { limit, maximum }),
        Err(error) => Err(error.into()),
    }
}

fn count_structured_leaves(
    value: &RuntimeValue,
    depth: usize,
    leaves: &mut usize,
) -> Result<(), CharacterDialogueValueError> {
    let maximum_depth = usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_structured_depth);
    if depth > maximum_depth {
        return Err(CharacterDialogueValueError::Limit {
            limit: "structured_depth",
            maximum: maximum_depth,
        });
    }
    match value {
        RuntimeValue::Tuple(values) => {
            for value in values {
                count_structured_leaves(value, depth + 1, leaves)?;
            }
        }
        RuntimeValue::Seq(sequence) => {
            for value in sequence.clone().into_values() {
                count_structured_leaves(&value, depth + 1, leaves)?;
            }
        }
        RuntimeValue::Record(fields) => {
            for field in fields {
                count_structured_leaves(field.value(), depth + 1, leaves)?;
            }
        }
        RuntimeValue::NominalRecord(record) => {
            for field in record.fields() {
                count_structured_leaves(field, depth + 1, leaves)?;
            }
        }
        RuntimeValue::Opaque(value) => {
            count_structured_leaves(value.payload(), depth + 1, leaves)?;
        }
        RuntimeValue::Variant {
            payload: Some(payload),
            ..
        } => count_structured_leaves(payload, depth, leaves)?,
        RuntimeValue::Function(_)
        | RuntimeValue::Iterator(_)
        | RuntimeValue::MatrixF32(_)
        | RuntimeValue::MatrixF64(_)
        | RuntimeValue::TensorF32(_)
        | RuntimeValue::TensorF64(_)
        | RuntimeValue::Range(_) => {
            return Err(CharacterDialogueValueError::Field {
                field: "structured_value",
                reason: "structured style contains a runtime-only value".to_owned(),
            });
        }
        RuntimeValue::Unit
        | RuntimeValue::Bool(_)
        | RuntimeValue::Int(_)
        | RuntimeValue::UInt(_)
        | RuntimeValue::F32(_)
        | RuntimeValue::F64(_)
        | RuntimeValue::String(_)
        | RuntimeValue::Char(_)
        | RuntimeValue::Duration(_)
        | RuntimeValue::EntityRef(_)
        | RuntimeValue::Variant { payload: None, .. } => {
            *leaves = leaves
                .checked_add(1)
                .ok_or(CharacterDialogueValueError::Limit {
                    limit: "structured_leaves",
                    maximum: usize::from(
                        PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_structured_leaves,
                    ),
                })?;
            let maximum = usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_structured_leaves);
            if *leaves > maximum {
                return Err(CharacterDialogueValueError::Limit {
                    limit: "structured_leaves",
                    maximum,
                });
            }
        }
    }
    Ok(())
}

fn normalize_runtime_value(
    value: RuntimeValue,
) -> Result<RuntimeValue, CharacterDialogueValueError> {
    match value {
        RuntimeValue::F32(value) => {
            if !value.is_finite() {
                return Err(RuntimeSchemaError::NonFinite {
                    path: "$".to_owned(),
                    kind: "f32",
                }
                .into());
            }
            Ok(RuntimeValue::F32(if value == 0.0 { 0.0 } else { value }))
        }
        RuntimeValue::F64(value) => {
            if !value.is_finite() {
                return Err(RuntimeSchemaError::NonFinite {
                    path: "$".to_owned(),
                    kind: "f64",
                }
                .into());
            }
            Ok(RuntimeValue::F64(if value == 0.0 { 0.0 } else { value }))
        }
        RuntimeValue::Tuple(values) => values
            .into_iter()
            .map(normalize_runtime_value)
            .collect::<Result<Vec<_>, _>>()
            .map(RuntimeValue::Tuple),
        RuntimeValue::Seq(sequence) => sequence
            .into_values()
            .into_iter()
            .map(normalize_runtime_value)
            .collect::<Result<Vec<_>, _>>()
            .map(RuntimeSeq::values)
            .map(RuntimeValue::Seq),
        RuntimeValue::Record(fields) => {
            if let Some(pair) = fields
                .windows(2)
                .find(|pair| pair[0].name() >= pair[1].name())
            {
                return Err(CharacterDialogueValueError::Field {
                    field: "typed_value",
                    reason: format!(
                        "anonymous record fields are not in canonical order near `{}`",
                        pair[1].name()
                    ),
                });
            }
            let fields = fields
                .into_iter()
                .map(|field| {
                    let name = field.name().to_owned();
                    normalize_runtime_value(field.value().clone()).map(|value| (name, value))
                })
                .collect::<Result<Vec<_>, _>>()?;
            RuntimeValue::try_record(fields).map_err(|error| CharacterDialogueValueError::Field {
                field: "typed_value",
                reason: error.to_string(),
            })
        }
        RuntimeValue::NominalRecord(record) => {
            let type_id = record.type_id().clone();
            let layout = record.layout();
            record
                .into_fields()
                .into_iter()
                .map(normalize_runtime_value)
                .collect::<Result<Vec<_>, _>>()
                .map(|fields| {
                    RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
                        type_id, layout, fields,
                    ))
                })
        }
        RuntimeValue::Variant {
            owner,
            ordinal,
            name,
            payload,
        } => payload
            .map(|payload| normalize_runtime_value(*payload).map(Box::new))
            .transpose()
            .map(|payload| RuntimeValue::Variant {
                owner,
                ordinal,
                name,
                payload,
            }),
        value => Ok(value),
    }
}

pub(super) fn empty_like(
    value: &CharacterDialogueTypedValue,
) -> Result<CharacterDialogueTypedValue, CharacterDialogueValueError> {
    let empty = empty_runtime_value(value.value())?;
    Ok(CharacterDialogueTypedValue {
        nominal_type: value.nominal_type.clone(),
        layout: value.layout,
        value: empty,
    })
}

pub(super) fn replace_runtime_value(
    value: CharacterDialogueTypedValue,
    runtime: RuntimeValue,
) -> CharacterDialogueTypedValue {
    CharacterDialogueTypedValue {
        value: runtime,
        ..value
    }
}

pub(super) fn empty_runtime_value(
    value: &RuntimeValue,
) -> Result<RuntimeValue, CharacterDialogueValueError> {
    if let Some(none) = value.option_none_with_same_owner() {
        return Ok(none);
    }
    match value {
        RuntimeValue::Unit => Ok(RuntimeValue::Unit),
        RuntimeValue::Tuple(values) => values
            .iter()
            .map(empty_runtime_value)
            .collect::<Result<Vec<_>, _>>()
            .map(RuntimeValue::Tuple),
        RuntimeValue::Seq(_) => Ok(RuntimeValue::Seq(RuntimeSeq::values(Vec::new()))),
        RuntimeValue::Record(_) => Ok(RuntimeValue::Record(Vec::new())),
        RuntimeValue::NominalRecord(record) => record
            .fields()
            .iter()
            .map(empty_runtime_value)
            .collect::<Result<Vec<_>, _>>()
            .map(|fields| {
                RuntimeValue::NominalRecord(arcweft_core::value::RuntimeNominalRecordValue::new(
                    record.type_id().clone(),
                    record.layout(),
                    fields,
                ))
            }),
        _ => Err(CharacterDialogueValueError::Field {
            field: "structured_patch",
            reason: "clear requires an optional or structurally empty runtime field".to_owned(),
        }),
    }
}
