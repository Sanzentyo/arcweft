//! Runtime-plan dialogue display catalog.

use crate::{InlineFailurePolicy, RichTextDocument, RichTextStyle};
use arcweft_core::plan::RuntimeLineId;
use arcweft_view::ViewId;
use serde::{Deserialize, Serialize};

/// Rich-text display sidecar generated while lowering a runtime plan.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct LineDisplayCatalog {
    lines: Vec<LineDisplaySpec>,
}

/// One dialogue line's renderable text and host-observable tag events.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LineDisplaySpec {
    pub line: RuntimeLineId,
    pub callee: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_label: Option<String>,
    pub text_key: Option<String>,
    /// Stable public owner of the authored View used for this dialogue line.
    pub view: ViewId,
    pub voice: Option<String>,
    pub look: Option<String>,
    pub style: Option<String>,
    #[serde(default)]
    pub base_styles: Vec<RichTextStyle>,
    #[serde(default)]
    pub default_inline_failure_policy: Option<InlineFailurePolicy>,
    #[serde(default)]
    pub style_contributions: Vec<RichTextStyleContribution>,
    pub args: Vec<LineDisplayArg>,
    pub content: RichTextDocument,
}

/// Provenance for one style contribution in the effective dialogue cascade.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextStyleContribution {
    pub path: String,
    pub layer: RichTextCascadeLayer,
    pub source: RichTextSettingSource,
    pub op: RichTextAssignOp,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_index: Option<usize>,
    #[serde(default)]
    pub active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadowed_by: Option<usize>,
}

/// Effective style cascade layer.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RichTextCascadeLayer {
    InlineSpan,
    LineOptions,
    SpeakerPreset,
    CharacterDialogueStyle,
    DialogueViewStyle,
    DialogueDefaults,
    EngineDefaults,
}

/// Source of a style/default contribution.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextSettingSource {
    SourceFile {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        item_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        public_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        range: Option<RichTextSourceRange>,
    },
    EngineDefault {
        key: String,
    },
}

/// Assignment operator used by a style/default contribution.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RichTextAssignOp {
    Replace,
    Append,
}

/// Half-open byte range in source text.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextSourceRange {
    pub start: usize,
    pub end: usize,
}

/// Non-reserved line argument preserved for player adapters.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LineDisplayArg {
    pub name: String,
    pub value: String,
}

impl LineDisplayCatalog {
    /// Creates a catalog from display specs in runtime order.
    #[must_use]
    pub fn new(lines: Vec<LineDisplaySpec>) -> Self {
        Self { lines }
    }

    /// Appends one display spec.
    pub fn push(&mut self, spec: LineDisplaySpec) {
        self.lines.push(spec);
    }

    /// Display specs in runtime order.
    #[must_use]
    pub fn lines(&self) -> &[LineDisplaySpec] {
        &self.lines
    }

    /// Finds a line display spec by runtime line id.
    #[must_use]
    pub fn find(&self, line: &RuntimeLineId) -> Option<&LineDisplaySpec> {
        self.lines.iter().find(|spec| &spec.line == line)
    }
}
