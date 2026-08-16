//! Authored rich-text nodes and dialogue-local control data.

use crate::{RichTextSpanKind, RichTextStyle};
use arcweft_core::runtime_id::RuntimeDialogueValueSlotId;
use arcweft_dialogue::InlineFailurePolicy;
use serde::{Deserialize, Serialize};

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
    Ruby {
        base: String,
        ruby: String,
    },
    StyleStart {
        style: Box<RichTextStyle>,
    },
    StyleEnd {
        span: RichTextSpanKind,
    },
    Control {
        control: RichTextControl,
    },
    Interpolation {
        /// Document-local value supplied by the owning runtime dialogue plan.
        slot: RuntimeDialogueValueSlotId,
        /// Stable authored label used only for diagnostics and fallback text.
        label: String,
        on_error: InlineFailurePolicy,
    },
    HostEvent {
        event: DialogueHostEvent,
    },
    ConditionalStart {
        condition: RuntimeDialogueValueSlotId,
    },
    ConditionalElse,
    ConditionalEnd,
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
            | Self::HostEvent { .. }
            | Self::ConditionalStart { .. }
            | Self::ConditionalElse
            | Self::ConditionalEnd => None,
        }
    }
}
