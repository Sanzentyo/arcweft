//! Dialogue-owned inline interpolation failure policy.

use crate::CharacterDialogueStyleValue;
use serde::{Deserialize, Deserializer, Serialize};

/// Failure handling policy for one runtime interpolation expression.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InlineFailurePolicy {
    #[default]
    FailLine,
    Discard,
    Fallback {
        fallback: InlineFallback,
    },
}

/// Fallback rendering strategy for a failed runtime interpolation expression.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
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

impl InlineFailurePolicy {
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

#[cfg(test)]
mod tests {
    use super::InlineFailurePolicy;

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
}
