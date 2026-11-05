//! Authored rich-text nodes and dialogue-local control data.

use crate::RichTextStyle;
use arcweft_core::runtime_id::{
    RuntimeDialogueEffectSiteId, RuntimeDialogueMarkId, RuntimeDialogueValueSlotId,
};
use arcweft_dialogue::InlineFailureSelection;
use arcweft_source::{ProductSourceIdentityError, ProductSourceRef, SourceSpan};
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

/// Origin of the style on one lexical scope, retained with that scope through
/// Content expansion and frame resolution. Generated formatting scopes have
/// no authored source location.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RichTextStyleOrigin {
    Authored { source: Box<RichTextStyleSource> },
    Generated,
}

/// Bounded byte coordinates in the exact product source revision that authored
/// a style scope. The source text itself is never copied into the product.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RichTextStyleSource {
    source: ProductSourceRef,
    start: u64,
    end: u64,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RichTextStyleSourceError {
    #[error(transparent)]
    Identity(#[from] ProductSourceIdentityError),
    #[error("style source range must satisfy start <= end <= source length")]
    InvalidRange,
}

impl RichTextStyleSource {
    pub const fn source(&self) -> &ProductSourceRef {
        &self.source
    }

    pub const fn start(&self) -> u64 {
        self.start
    }

    pub const fn end(&self) -> u64 {
        self.end
    }
}

impl TryFrom<&SourceSpan> for RichTextStyleSource {
    type Error = RichTextStyleSourceError;

    fn try_from(span: &SourceSpan) -> Result<Self, Self::Error> {
        Ok(Self {
            source: ProductSourceRef::try_for_identity(span.source())?,
            start: u64::try_from(span.range().start())
                .map_err(|_| RichTextStyleSourceError::InvalidRange)?,
            end: u64::try_from(span.range().end())
                .map_err(|_| RichTextStyleSourceError::InvalidRange)?,
        })
    }
}

impl<'de> Deserialize<'de> for RichTextStyleSource {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            source: ProductSourceRef,
            start: u64,
            end: u64,
        }
        let wire = Wire::deserialize(deserializer)?;
        if wire.start > wire.end || wire.end > wire.source.source_len() {
            return Err(serde::de::Error::custom(
                RichTextStyleSourceError::InvalidRange,
            ));
        }
        Ok(Self {
            source: wire.source,
            start: wire.start,
            end: wire.end,
        })
    }
}

/// Ordered rich-text document used by source resolvers.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct RichTextDocument {
    pub nodes: Vec<RichTextNode>,
    resolved_text: String,
}

/// One rich-text node in authored order.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextNode {
    Text {
        text: String,
    },
    /// A lexical modifier with an owned, recursively typed body.
    ///
    /// Scope boundaries are structural; there is no separate close node or
    /// balance stack for a valid document.
    Scope {
        style: Box<RichTextStyle>,
        origin: RichTextStyleOrigin,
        body: Vec<RichTextNode>,
    },
    Ruby {
        body: Vec<RichTextNode>,
        ruby: String,
    },
    /// Source-authored literal text that must not be interpreted as controls.
    Raw {
        text: String,
    },
    Control {
        control: RichTextControl,
    },
    Interpolation {
        /// Document-local value supplied by the owning runtime dialogue plan.
        slot: RuntimeDialogueValueSlotId,
        /// Stable authored label used only for diagnostics and fallback text.
        label: String,
        on_error: InlineFailureSelection,
    },
    /// Inserts the immutable fragment selected by a typed Content binding.
    ///
    /// Expansion is owned by [`crate::DialogueContentMaterializer`]; a
    /// renderer receives the resulting node tree and never decodes a raw
    /// runtime value for this node.
    ContentInsert {
        slot: RuntimeDialogueValueSlotId,
        on_error: InlineFailureSelection,
    },
    /// Inserts the checked result of a rich-text formatting operation.
    ///
    /// Source strings are retained exactly for recoverable expression/call
    /// fallback policies. Formatting outcome and the selected dynamic policy
    /// operand come from the matching typed Content binding.
    FormattedInsert {
        slot: RuntimeDialogueValueSlotId,
        call_source: String,
        value_source: String,
    },
    HostEvent {
        event: DialogueHostEvent,
    },
}

/// Text-container-local control instruction.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextControl {
    Page,
    LineWait,
    HardBreak,
    TimedWait {
        duration_millis: u64,
    },
    Clear,
    Reset,
    /// Changes the reveal rate for following source-ordered content.
    RevealRate {
        milli_cps: crate::Milli,
    },
    Mark {
        mark: RuntimeDialogueMarkId,
        diagnostic_name: String,
    },
    /// Typed source-ordered inline effect boundary. The runtime content plan
    /// owns the matching operation; renderers never interpret a synthetic
    /// mark label.
    Effect {
        site: RuntimeDialogueEffectSiteId,
    },
}

/// Host-observable rich-text event for non-text presentation/audio/capability tags.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DialogueHostEvent {
    Voice { source: DialogueVoiceSource },
    Face { expression: String },
    Pose { pose: String },
    Show { entity: String },
    Hide { entity: String },
    Move { x: crate::Milli, y: crate::Milli },
    Scale { x: crate::Milli, y: crate::Milli },
    Rotate { angle: crate::RichTextAngle },
    Anim { animation: String },
    Shake { amplitude: crate::Milli },
    Signal { signal: String },
}

/// Closed voice selection accepted by the `RichText` semantic checker.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DialogueVoiceSource {
    Auto,
    Identity { id: String },
}

impl RichTextDocument {
    /// Creates a rich text document and materializes its contiguous static text once.
    #[must_use]
    pub fn new(nodes: Vec<RichTextNode>) -> Self {
        let resolved_text = nodes.iter().filter_map(RichTextNode::static_text).collect();
        Self {
            nodes,
            resolved_text,
        }
    }

    /// Contiguous visible text retained by this source document.
    #[must_use]
    pub fn resolved_text(&self) -> &str {
        &self.resolved_text
    }
}

impl RichTextNode {
    fn static_text(&self) -> Option<String> {
        match self {
            Self::Text { text } | Self::Raw { text } => Some(text.clone()),
            Self::Scope { body, .. } | Self::Ruby { body, .. } => {
                let mut text = String::new();
                for node in body {
                    text.push_str(&node.static_text()?);
                }
                Some(text)
            }
            Self::Control {
                control: RichTextControl::HardBreak,
            } => Some("\n".to_owned()),
            Self::Control { .. }
            | Self::Interpolation { .. }
            | Self::ContentInsert { .. }
            | Self::FormattedInsert { .. }
            | Self::HostEvent { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

    #[test]
    fn style_source_round_trip_retains_revision_and_rejects_unbounded_ranges() {
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new("style.arcw").unwrap(),
            SourceName::path("style.arcw"),
            "前#color(rgb(\"#202122\"))[猫]後",
        )
        .unwrap();
        let span = document.span(SourceRange::new(3, 30)).unwrap();
        let source = RichTextStyleSource::try_from(&span).unwrap();
        let encoded = serde_json::to_value(&source).unwrap();
        assert_eq!(
            serde_json::from_value::<RichTextStyleSource>(encoded.clone()).unwrap(),
            source
        );
        assert_eq!(
            source.source(),
            &ProductSourceRef::try_for_identity(document.identity()).unwrap()
        );
        assert_eq!(source.start(), 3);
        assert_eq!(source.end(), 30);
        for (start, end) in [(31, 30), (0, document.identity().source_len() + 1)] {
            let mut invalid = encoded.clone();
            invalid["start"] = start.into();
            invalid["end"] = end.into();
            assert!(serde_json::from_value::<RichTextStyleSource>(invalid).is_err());
        }
    }

    #[test]
    fn scope_origin_is_required_and_bound_into_the_template_digest() {
        use crate::DialogueContentFragmentTemplate;
        use arcweft_core::runtime_id::RuntimeDialogueContentTemplateId;
        let template = |origin| {
            DialogueContentFragmentTemplate::try_new_canonical(
                RuntimeDialogueContentTemplateId::from_zero_based(0).unwrap(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                RichTextDocument::new(vec![RichTextNode::Scope {
                    style: Box::new(RichTextStyle::Strong),
                    origin,
                    body: vec![RichTextNode::Text {
                        text: "猫".to_owned(),
                    }],
                }]),
            )
            .unwrap()
        };
        let generated = template(RichTextStyleOrigin::Generated);
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new("style.arcw").unwrap(),
            SourceName::path("style.arcw"),
            "猫",
        )
        .unwrap();
        let authored = template(RichTextStyleOrigin::Authored {
            source: Box::new(
                RichTextStyleSource::try_from(&document.span(SourceRange::new(0, 3)).unwrap())
                    .unwrap(),
            ),
        });
        assert_ne!(authored.digest(), generated.digest());
        let encoded = serde_json::to_value(&authored).unwrap();
        assert_eq!(
            serde_json::from_value::<DialogueContentFragmentTemplate>(encoded.clone()).unwrap(),
            authored
        );
        let mut missing = encoded["content"]["nodes"][0].clone();
        missing.as_object_mut().unwrap().remove("origin");
        assert!(serde_json::from_value::<RichTextNode>(missing).is_err());
        let mut tampered = encoded;
        tampered["content"]["nodes"][0]["origin"] = serde_json::json!({"kind":"generated"});
        assert!(serde_json::from_value::<DialogueContentFragmentTemplate>(tampered).is_err());
    }
}
