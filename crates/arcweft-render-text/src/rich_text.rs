//! Authored rich-text nodes and dialogue-local control data.

use crate::RichTextStyle;
use arcweft_dialogue::InlineFailurePolicy;
use serde::{Deserialize, Serialize};

/// Ordered rich-text document used by source resolvers.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct RichTextDocument {
    pub nodes: Vec<RichTextNode>,
    #[serde(skip)]
    resolved_text: String,
}

/// One rich-text node in authored order.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextNode {
    Text {
        text: String,
    },
    Ruby {
        base: String,
        ruby: String,
    },
    StyleStart {
        style: RichTextStyle,
    },
    StyleEnd {
        name: String,
    },
    Control {
        control: RichTextControl,
    },
    Interpolation {
        expr: String,
        fallback_source: String,
        on_error: InlineFailurePolicy,
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
    TimedWait { duration_millis: u64 },
    Clear,
    Reset,
    Mark { name: String },
    Raw { text: String },
    Unknown { name: String, attrs: String },
}

/// Host-observable rich-text event for non-text presentation/audio/capability tags.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DialogueHostEvent {
    Voice { attrs: String },
    Face { attrs: String },
    Pose { attrs: String },
    Show { attrs: String },
    Hide { attrs: String },
    Move { attrs: String },
    Scale { attrs: String },
    Rotate { attrs: String },
    Anim { attrs: String },
    Shake { attrs: String },
    TimedCue { attrs: String },
    Call { attrs: String },
    Signal { attrs: String },
    Effect { id: String, attrs: String },
    Conditional { name: String, attrs: String },
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
    fn static_text(&self) -> Option<&str> {
        match self {
            Self::Text { text }
            | Self::Control {
                control: RichTextControl::Raw { text },
            } => Some(text),
            Self::Ruby { base, .. } => Some(base),
            Self::Control {
                control: RichTextControl::HardBreak,
            } => Some("\n"),
            Self::StyleStart { .. }
            | Self::StyleEnd { .. }
            | Self::Control { .. }
            | Self::Interpolation { .. }
            | Self::HostEvent { .. } => None,
        }
    }
}

impl<'de> Deserialize<'de> for RichTextDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SerializedDocument {
            nodes: Vec<RichTextNode>,
        }

        let serialized = SerializedDocument::deserialize(deserializer)?;
        Ok(Self::new(serialized.nodes))
    }
}
