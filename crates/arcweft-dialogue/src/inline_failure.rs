//! Dialogue-owned inline interpolation failure policy.

use crate::{
    CharacterDialoguePolicyTypeGraph, CharacterDialoguePolicyVariantOwner,
    CharacterDialogueStyleValue, CharacterDialogueTypedValue,
};
use arcweft_core::value::RuntimeValue;
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

/// Failure handling policy for one runtime interpolation expression.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InlineFailurePolicy {
    #[default]
    FailLine,
    Discard,
    Fallback {
        fallback: InlineFallback,
    },
}

/// Selects the failure policy for one inline or Content insertion.
///
/// `InheritCharacterDialogue` defers policy selection until the effective
/// `CharacterDialogue` value is known. `Explicit` retains an authored policy
/// without allowing a renderer or materializer to infer one from a raw value.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InlineFailureSelection {
    #[default]
    InheritCharacterDialogue,
    Explicit {
        policy: InlineFailurePolicy,
    },
}

/// Fallback rendering strategy for a failed runtime interpolation expression.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InlineFallback {
    Text {
        text: String,
        style: FallbackStylePolicy,
    },
    ExprSource {
        style: FallbackStylePolicy,
    },
    CallSource {
        style: FallbackStylePolicy,
    },
    ValuePlain,
}

/// Style behavior for fallback rendering.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FallbackStylePolicy {
    Plain,
    InheritSurrounding,
    Apply {
        styles: Vec<CharacterDialogueStyleValue>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
enum StrictInlineFailurePolicy {
    FailLine {},
    Discard {},
    Fallback { fallback: InlineFallback },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
enum StrictInlineFailureSelection {
    InheritCharacterDialogue {},
    Explicit { policy: InlineFailurePolicy },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
enum StrictInlineFallback {
    Text {
        text: String,
        style: FallbackStylePolicy,
    },
    ExprSource {
        style: FallbackStylePolicy,
    },
    CallSource {
        style: FallbackStylePolicy,
    },
    ValuePlain {},
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
enum StrictFallbackStylePolicy {
    Plain {},
    InheritSurrounding {},
    Apply {
        styles: Vec<CharacterDialogueStyleValue>,
    },
}

impl<'de> Deserialize<'de> for InlineFailurePolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(
            match StrictInlineFailurePolicy::deserialize(deserializer)? {
                StrictInlineFailurePolicy::FailLine {} => Self::FailLine,
                StrictInlineFailurePolicy::Discard {} => Self::Discard,
                StrictInlineFailurePolicy::Fallback { fallback } => Self::Fallback { fallback },
            },
        )
    }
}

impl<'de> Deserialize<'de> for InlineFailureSelection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(
            match StrictInlineFailureSelection::deserialize(deserializer)? {
                StrictInlineFailureSelection::InheritCharacterDialogue {} => {
                    Self::InheritCharacterDialogue
                }
                StrictInlineFailureSelection::Explicit { policy } => Self::Explicit { policy },
            },
        )
    }
}

impl<'de> Deserialize<'de> for InlineFallback {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(match StrictInlineFallback::deserialize(deserializer)? {
            StrictInlineFallback::Text { text, style } => Self::Text { text, style },
            StrictInlineFallback::ExprSource { style } => Self::ExprSource { style },
            StrictInlineFallback::CallSource { style } => Self::CallSource { style },
            StrictInlineFallback::ValuePlain {} => Self::ValuePlain,
        })
    }
}

impl<'de> Deserialize<'de> for FallbackStylePolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(
            match StrictFallbackStylePolicy::deserialize(deserializer)? {
                StrictFallbackStylePolicy::Plain {} => Self::Plain,
                StrictFallbackStylePolicy::InheritSurrounding {} => Self::InheritSurrounding,
                StrictFallbackStylePolicy::Apply { styles } => Self::Apply { styles },
            },
        )
    }
}

/// Runtime interpolation failure retained by the display frame.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InlineTextFailure {
    pub expr: String,
    pub reason: String,
    pub policy: InlineFailurePolicy,
}

/// Failure to decode a dynamic `InlineFailure` runtime operand.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum InlineFailurePolicyDecodeError {
    #[error("runtime InlineFailure operand has an invalid typed shape at {field}")]
    InvalidValue { field: &'static str },
    #[error("runtime InlineFailure fallback text exceeds the configured size limit")]
    FallbackTextLimit,
}

impl InlineFailurePolicy {
    /// Strictly decodes the closed runtime `InlineFailure` policy shape.
    ///
    /// The executing program's type admission checks the full variant layout
    /// hash. This domain decoder independently checks the canonical nominal
    /// owner, case identity, and exact payload structure before applying it.
    pub fn try_from_runtime_value(
        value: &RuntimeValue,
    ) -> Result<Self, InlineFailurePolicyDecodeError> {
        let (case, payload) = decode_policy_variant(
            value,
            CharacterDialoguePolicyVariantOwner::InlineFailure,
            "inline_failure",
        )?;
        match (case.language_name(), payload) {
            ("fail", None) => Ok(Self::FailLine),
            ("discard", None) => Ok(Self::Discard),
            ("fallback", Some(value)) => Ok(Self::Fallback {
                fallback: decode_fallback(value)?,
            }),
            _ => Err(InlineFailurePolicyDecodeError::InvalidValue {
                field: "inline_failure",
            }),
        }
    }

    #[must_use]
    pub fn fallback_text(text: impl Into<String>) -> Self {
        Self::Fallback {
            fallback: InlineFallback::Text {
                text: text.into(),
                style: FallbackStylePolicy::Plain,
            },
        }
    }

    #[must_use]
    pub const fn fallback_expr_source(style: FallbackStylePolicy) -> Self {
        Self::Fallback {
            fallback: InlineFallback::ExprSource { style },
        }
    }

    #[must_use]
    pub const fn fallback_call_source(style: FallbackStylePolicy) -> Self {
        Self::Fallback {
            fallback: InlineFallback::CallSource { style },
        }
    }

    #[must_use]
    pub const fn fallback_value_plain() -> Self {
        Self::Fallback {
            fallback: InlineFallback::ValuePlain,
        }
    }
}

fn decode_fallback(value: &RuntimeValue) -> Result<InlineFallback, InlineFailurePolicyDecodeError> {
    let (case, payload) = decode_policy_variant(
        value,
        CharacterDialoguePolicyVariantOwner::InlineFallback,
        "inline_fallback",
    )?;
    match (case.language_name(), payload) {
        ("text", Some(RuntimeValue::Tuple(fields))) if fields.len() == 2 => {
            let [RuntimeValue::String(text), style] = fields.as_slice() else {
                return Err(InlineFailurePolicyDecodeError::InvalidValue {
                    field: "inline_fallback.text",
                });
            };
            if text.len()
                > usize::try_from(
                    crate::PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_config_string_bytes,
                )
                .unwrap_or(usize::MAX)
            {
                return Err(InlineFailurePolicyDecodeError::FallbackTextLimit);
            }
            Ok(InlineFallback::Text {
                text: text.clone(),
                style: decode_fallback_style(&style)?,
            })
        }
        ("expr_source", Some(style)) => Ok(InlineFallback::ExprSource {
            style: decode_fallback_style(&style)?,
        }),
        ("call_source", Some(style)) => Ok(InlineFallback::CallSource {
            style: decode_fallback_style(&style)?,
        }),
        ("value_plain", None) => Ok(InlineFallback::ValuePlain),
        _ => Err(InlineFailurePolicyDecodeError::InvalidValue {
            field: "inline_fallback",
        }),
    }
}

fn decode_fallback_style(
    value: &RuntimeValue,
) -> Result<FallbackStylePolicy, InlineFailurePolicyDecodeError> {
    let (case, payload) = decode_policy_variant(
        value,
        CharacterDialoguePolicyVariantOwner::FallbackStyle,
        "fallback_style",
    )?;
    match (case.language_name(), payload) {
        ("plain", None) => Ok(FallbackStylePolicy::Plain),
        ("inherit_surrounding", None) => Ok(FallbackStylePolicy::InheritSurrounding),
        ("apply", Some(RuntimeValue::Seq(sequence))) => {
            let values =
                sequence
                    .as_values()
                    .ok_or(InlineFailurePolicyDecodeError::InvalidValue {
                        field: "fallback_style.apply",
                    })?;
            let styles = values
                .iter()
                .cloned()
                .map(CharacterDialogueTypedValue::try_new)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| InlineFailurePolicyDecodeError::InvalidValue {
                    field: "fallback_style.apply",
                })?
                .into_iter()
                .map(CharacterDialogueStyleValue::try_new)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| InlineFailurePolicyDecodeError::InvalidValue {
                    field: "fallback_style.apply",
                })?;
            Ok(FallbackStylePolicy::Apply { styles })
        }
        _ => Err(InlineFailurePolicyDecodeError::InvalidValue {
            field: "fallback_style",
        }),
    }
}

fn decode_policy_variant<'a>(
    value: &'a RuntimeValue,
    expected: CharacterDialoguePolicyVariantOwner,
    field: &'static str,
) -> Result<
    (
        crate::CharacterDialoguePolicyCaseSpec,
        Option<&'a RuntimeValue>,
    ),
    InlineFailurePolicyDecodeError,
> {
    let RuntimeValue::Variant {
        owner,
        ordinal,
        name,
        payload,
    } = value
    else {
        return Err(InlineFailurePolicyDecodeError::InvalidValue { field });
    };
    if !expected.matches_runtime_identity(owner) {
        return Err(InlineFailurePolicyDecodeError::InvalidValue { field });
    }
    let case = CharacterDialoguePolicyTypeGraph::case_spec_for_value(expected, *ordinal, name)
        .ok_or(InlineFailurePolicyDecodeError::InvalidValue { field })?;
    Ok((case, payload.as_deref()))
}

impl InlineFailureSelection {
    #[must_use]
    pub const fn inherit_character_dialogue() -> Self {
        Self::InheritCharacterDialogue
    }

    #[must_use]
    pub const fn explicit(policy: InlineFailurePolicy) -> Self {
        Self::Explicit { policy }
    }

    #[must_use]
    pub fn resolve(&self, inherited: &InlineFailurePolicy) -> InlineFailurePolicy {
        match self {
            Self::InheritCharacterDialogue => inherited.clone(),
            Self::Explicit { policy } => policy.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FallbackStylePolicy, InlineFailurePolicy, InlineFailurePolicyDecodeError,
        InlineFailureSelection, InlineFallback,
    };
    use crate::{CharacterDialoguePolicyTypeGraph, CharacterDialoguePolicyVariantOwner};
    use arcweft_core::{
        entry::RuntimeSchemaLimits,
        value::{RuntimeDialogueOpaqueRole, RuntimeValue},
    };

    fn policy_graph() -> CharacterDialoguePolicyTypeGraph {
        CharacterDialoguePolicyTypeGraph::try_new(
            RuntimeDialogueOpaqueRole::Content.exact_owner(),
            RuntimeSchemaLimits::engine_default(),
        )
        .expect("policy graph")
    }

    fn case_value(
        graph: &CharacterDialoguePolicyTypeGraph,
        owner: CharacterDialoguePolicyVariantOwner,
        language_name: &str,
        payload: Option<RuntimeValue>,
    ) -> RuntimeValue {
        let case = graph
            .cases(owner)
            .iter()
            .find(|case| case.language_name() == language_name)
            .expect("policy case");
        RuntimeValue::Variant {
            owner: graph.identity(owner).clone(),
            ordinal: case.ordinal(),
            name: case.name().to_owned(),
            payload: payload.map(Box::new),
        }
    }

    #[test]
    fn tagged_unit_variants_reject_unknown_fields_at_every_level() {
        for malformed in [
            r#"{"kind":"fail_line","unexpected":true}"#,
            r#"{"kind":"discard","unexpected":true}"#,
            r#"{"kind":"fallback","fallback":{"kind":"value_plain","unexpected":true}}"#,
            r#"{"kind":"fallback","fallback":{"kind":"text","text":"x","style":{"kind":"plain","unexpected":true}}}"#,
        ] {
            assert!(
                serde_json::from_str::<InlineFailurePolicy>(malformed).is_err(),
                "policy must reject {malformed}"
            );
        }
    }

    #[test]
    fn failure_selection_is_closed_and_resolves_at_display_time() {
        let inherited = InlineFailurePolicy::Discard;
        let selection = InlineFailureSelection::InheritCharacterDialogue;
        assert_eq!(selection.resolve(&inherited), inherited);
        let explicit = InlineFailureSelection::Explicit {
            policy: InlineFailurePolicy::FailLine,
        };
        assert_eq!(
            explicit.resolve(&InlineFailurePolicy::Discard),
            InlineFailurePolicy::FailLine
        );
        assert!(
            serde_json::from_str::<InlineFailureSelection>(
                r#"{"kind":"inherit_character_dialogue","extra":true}"#
            )
            .is_err()
        );
    }

    #[test]
    fn runtime_policy_decoder_accepts_only_exact_closed_policy_shapes() {
        let graph = policy_graph();
        let fallback_style = case_value(
            &graph,
            CharacterDialoguePolicyVariantOwner::FallbackStyle,
            "plain",
            None,
        );
        let fallback = case_value(
            &graph,
            CharacterDialoguePolicyVariantOwner::InlineFallback,
            "text",
            Some(RuntimeValue::Tuple(vec![
                RuntimeValue::String("fallback".to_owned()),
                fallback_style,
            ])),
        );
        let value = case_value(
            &graph,
            CharacterDialoguePolicyVariantOwner::InlineFailure,
            "fallback",
            Some(fallback),
        );
        assert_eq!(
            InlineFailurePolicy::try_from_runtime_value(&value),
            Ok(InlineFailurePolicy::Fallback {
                fallback: InlineFallback::Text {
                    text: "fallback".to_owned(),
                    style: FallbackStylePolicy::Plain,
                },
            })
        );

        let malformed = RuntimeValue::String("not a policy".to_owned());
        assert!(matches!(
            InlineFailurePolicy::try_from_runtime_value(&malformed),
            Err(InlineFailurePolicyDecodeError::InvalidValue { .. })
        ));
        let wrong_owner = case_value(
            &graph,
            CharacterDialoguePolicyVariantOwner::Voice,
            "auto",
            None,
        );
        assert!(matches!(
            InlineFailurePolicy::try_from_runtime_value(&wrong_owner),
            Err(InlineFailurePolicyDecodeError::InvalidValue { .. })
        ));
    }
}
