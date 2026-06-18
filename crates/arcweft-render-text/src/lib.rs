//! Sans I/O rich-text display model for Arcweft dialogue.

use arcweft_core::plan::RuntimeLineId;
use arcweft_core::value::{RuntimeBinding, RuntimeValue};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod rich_effects;

pub use rich_effects::{
    Milli, RichTextAngle, RichTextEffectDescriptor, RichTextEffectPhase, RichTextEffectTarget,
    RichTextInlineDirection, RichTextJlreqStrictness, RichTextLayout, RichTextParam,
    RichTextPresentation, RichTextRubyPosition, RichTextShaderRef, RichTextStateScope,
    RichTextTransform, RichTextTransformOrigin, RichTextVec2, RichTextVerticalLatinMode,
    RichTextWritingMode, parse_decimal_milli, parse_milli_token,
};

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
    pub text_key: Option<String>,
    pub window: Option<String>,
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
    DialogueWindowTheme,
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

/// Ordered rich-text document used by native/browser/headless players.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct RichTextDocument {
    pub nodes: Vec<RichTextNode>,
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
    Control(RichTextControl),
    Interpolation {
        expr: String,
        fallback_source: String,
        on_error: InlineFailurePolicy,
    },
    HostEvent(DialogueHostEvent),
}

/// Failure handling policy for one runtime interpolation expression.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InlineFailurePolicy {
    FailLine,
    Discard,
    Fallback { fallback: InlineFallback },
}

/// Fallback rendering strategy for a failed runtime interpolation expression.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FallbackStylePolicy {
    Plain,
    InheritSurrounding,
    Apply { styles: Vec<RichTextStyle> },
}

/// Inline style span applied until the matching `StyleEnd`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextStyle {
    Em { attrs: String },
    Strong { attrs: String },
    Italic { attrs: String },
    Oblique { angle: RichTextAngle, raw: String },
    Color { value: RichTextColor },
    Font { family: RichTextFontFamily },
    Size { points: Option<u16>, raw: String },
    Speed { value: String },
    Layout { layout: RichTextLayout },
    Transform { transform: RichTextTransform },
    Effect { effect: RichTextEffectDescriptor },
    Shader { shader: RichTextShaderRef },
    Unknown { name: String, attrs: String },
}

/// Inline text color.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextColor {
    Rgb { red: u8, green: u8, blue: u8 },
    Named { name: String },
}

/// Font family requested by authored rich text.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextFontFamily {
    Serif,
    SansSerif,
    Monospace,
    Cursive,
    Fantasy,
    Named { name: String },
}

/// Textbox-local control instruction.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextControl {
    Page,
    LineWait,
    HardBreak,
    TimedWait { value: String },
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
    Conditional { name: String, attrs: String },
}

/// Dialogue line context captured by the runtime at display time.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct RuntimeLineContext {
    pub bindings: Vec<RuntimeBinding>,
}

/// One resolved frame generated by a player adapter.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LineDisplayFrame {
    pub line: RuntimeLineId,
    pub callee: String,
    pub text: String,
    pub base_styles: Vec<RichTextStyle>,
    pub default_inline_failure_policy: Option<InlineFailurePolicy>,
    #[serde(default)]
    pub style_contributions: Vec<RichTextStyleContribution>,
    pub nodes: Vec<RichTextNode>,
    pub display_map: RichTextDisplayMap,
    pub host_events: Vec<DialogueHostEvent>,
    pub inline_failures: Vec<InlineTextFailure>,
    pub unresolved: Vec<String>,
}

/// Mapping from resolved display text back to authored rich-text nodes.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextDisplayMap {
    pub text_runs: Vec<RichTextTextRun>,
    pub ruby_annotations: Vec<RichTextRubyAnnotation>,
    pub controls: Vec<RichTextControlMarker>,
    pub host_events: Vec<RichTextHostEventMarker>,
}

/// Half-open byte range in `LineDisplayFrame::text`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextRange {
    pub start: usize,
    pub end: usize,
}

impl RichTextRange {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// One resolved visible text run.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextTextRun {
    pub range: RichTextRange,
    pub source: RichTextTextSource,
    pub node_index: usize,
    pub styles: Vec<RichTextStyle>,
    #[serde(default)]
    pub presentation: RichTextPresentation,
}

/// Source category for a resolved visible text run.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RichTextTextSource {
    Text,
    Interpolation,
    InterpolationFallback,
    RubyBase,
    ControlHardBreak,
    ControlRaw,
}

/// Ruby annotation attached to a resolved base text range.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextRubyAnnotation {
    pub base_range: RichTextRange,
    pub ruby: String,
    pub node_index: usize,
    pub styles: Vec<RichTextStyle>,
    #[serde(default)]
    pub presentation: RichTextPresentation,
}

/// Control node marker in the resolved display stream.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextControlMarker {
    pub node_index: usize,
    pub control: RichTextControl,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<RichTextRange>,
}

/// Host event marker in authored node order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextHostEventMarker {
    pub node_index: usize,
    pub event_index: usize,
    pub event: DialogueHostEvent,
}

/// Runtime interpolation failure retained by the display frame.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InlineTextFailure {
    pub expr: String,
    pub reason: String,
    pub policy: InlineFailurePolicy,
}

/// Error raised when a line chooses fail-fast interpolation behavior.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("inline dialogue expression `{expr}` failed: {reason}")]
pub struct LineDisplayError {
    pub line: RuntimeLineId,
    pub expr: String,
    pub reason: String,
}

impl LineDisplayCatalog {
    /// Creates a catalog from display specs in runtime order.
    pub fn new(lines: Vec<LineDisplaySpec>) -> Self {
        Self { lines }
    }

    /// Appends one display spec.
    pub fn push(&mut self, spec: LineDisplaySpec) {
        self.lines.push(spec);
    }

    /// Display specs in runtime order.
    pub fn lines(&self) -> &[LineDisplaySpec] {
        &self.lines
    }

    /// Finds a line display spec by runtime line id.
    pub fn find(&self, line: &RuntimeLineId) -> Option<&LineDisplaySpec> {
        self.lines.iter().find(|spec| &spec.line == line)
    }
}

impl RichTextDocument {
    /// Creates a rich text document from nodes in authored order.
    pub fn new(nodes: Vec<RichTextNode>) -> Self {
        Self { nodes }
    }
}

impl InlineFailurePolicy {
    pub fn fallback_text(text: impl Into<String>) -> Self {
        Self::Fallback {
            fallback: InlineFallback::Text {
                text: text.into(),
                style: FallbackStylePolicy::Plain,
            },
        }
    }

    pub const fn fallback_expr_source(style: FallbackStylePolicy) -> Self {
        Self::Fallback {
            fallback: InlineFallback::ExprSource { style },
        }
    }

    pub const fn fallback_call_source(style: FallbackStylePolicy) -> Self {
        Self::Fallback {
            fallback: InlineFallback::CallSource { style },
        }
    }

    pub const fn fallback_value_plain() -> Self {
        Self::Fallback {
            fallback: InlineFallback::ValuePlain,
        }
    }
}

impl RichTextStyle {
    /// Creates a typed style from an authored tag name and raw attribute text.
    pub fn from_tag(name: &str, attrs: &str) -> Self {
        let attrs = attrs.trim();
        match name {
            "em" => Self::Em {
                attrs: attrs.to_owned(),
            },
            "strong" => Self::Strong {
                attrs: attrs.to_owned(),
            },
            "i" | "italic" => Self::Italic {
                attrs: attrs.to_owned(),
            },
            "oblique" | "slant" => Self::Oblique {
                angle: RichTextAngle {
                    degrees: parse_milli_token(attrs),
                },
                raw: attrs.to_owned(),
            },
            "color" => Self::Color {
                value: RichTextColor::from_attrs(attrs),
            },
            "font" => Self::Font {
                family: RichTextFontFamily::from_attrs(attrs),
            },
            "size" => Self::Size {
                points: attrs
                    .split_whitespace()
                    .next()
                    .and_then(|value| value.parse::<u16>().ok()),
                raw: attrs.to_owned(),
            },
            "speed" => Self::Speed {
                value: attrs.to_owned(),
            },
            name => Self::Unknown {
                name: name.to_owned(),
                attrs: attrs.to_owned(),
            },
        }
    }

    /// Authored tag name used to match the corresponding end tag.
    pub fn tag_name(&self) -> &str {
        match self {
            Self::Em { .. } => "em",
            Self::Strong { .. } => "strong",
            Self::Italic { .. } | Self::Oblique { .. } => "style",
            Self::Color { .. } => "color",
            Self::Font { .. } => "font",
            Self::Size { .. } => "size",
            Self::Speed { .. } => "speed",
            Self::Layout { .. } => "layout",
            Self::Transform { .. } => "transform",
            Self::Effect { .. } | Self::Shader { .. } => "effect",
            Self::Unknown { name, .. } => name,
        }
    }
}

impl RichTextColor {
    /// Parses authored color attributes into a deterministic color token.
    pub fn from_attrs(attrs: &str) -> Self {
        parse_hex_color(attrs).unwrap_or_else(|| Self::Named {
            name: trim_quoted(attrs).to_owned(),
        })
    }
}

impl RichTextFontFamily {
    /// Parses authored font attributes into a typed font family request.
    pub fn from_attrs(attrs: &str) -> Self {
        match trim_quoted(attrs).trim().to_ascii_lowercase().as_str() {
            "" | "sans" | "sans-serif" | "sans_serif" | "ui-sans" => Self::SansSerif,
            "serif" | "ui-serif" => Self::Serif,
            "mono" | "monospace" | "ui-monospace" => Self::Monospace,
            "cursive" => Self::Cursive,
            "fantasy" => Self::Fantasy,
            _ => Self::Named {
                name: trim_quoted(attrs).to_owned(),
            },
        }
    }
}

fn parse_hex_color(value: &str) -> Option<RichTextColor> {
    let hex = value.trim().strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(RichTextColor::Rgb { red, green, blue })
}

fn trim_quoted(value: &str) -> &str {
    let trimmed = value.trim();
    trimmed
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            trimmed
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(trimmed)
}

impl RuntimeLineContext {
    /// Creates context from visible runtime bindings.
    pub fn new(bindings: Vec<RuntimeBinding>) -> Self {
        Self { bindings }
    }

    fn get(&self, name: &str) -> Option<&RuntimeValue> {
        self.bindings
            .iter()
            .rev()
            .find(|binding| binding.name == name)
            .map(|binding| &binding.value)
    }

    fn condition_is_truthy(&self, expr: &str) -> bool {
        match expr.trim() {
            "" | "false" => false,
            "true" => true,
            name => self.get(name).is_some_and(runtime_value_is_truthy),
        }
    }
}

impl LineDisplaySpec {
    /// Resolves a display spec against runtime context for a headless/native frame.
    pub fn resolve_frame(
        &self,
        context: &RuntimeLineContext,
    ) -> Result<LineDisplayFrame, LineDisplayError> {
        LineDisplayFrameResolver::new(self, context).resolve()
    }
}

struct LineDisplayFrameResolver<'a> {
    spec: &'a LineDisplaySpec,
    context: &'a RuntimeLineContext,
    text: String,
    nodes: Vec<RichTextNode>,
    display_map: RichTextDisplayMap,
    host_events: Vec<DialogueHostEvent>,
    inline_failures: Vec<InlineTextFailure>,
    unresolved: Vec<String>,
    active_styles: Vec<RichTextStyle>,
    conditional_stack: Vec<ConditionalBranch>,
}

impl<'a> LineDisplayFrameResolver<'a> {
    fn new(spec: &'a LineDisplaySpec, context: &'a RuntimeLineContext) -> Self {
        Self {
            spec,
            context,
            text: String::new(),
            nodes: Vec::new(),
            display_map: RichTextDisplayMap::default(),
            host_events: Vec::new(),
            inline_failures: Vec::new(),
            unresolved: Vec::new(),
            active_styles: Vec::new(),
            conditional_stack: Vec::new(),
        }
    }

    fn resolve(mut self) -> Result<LineDisplayFrame, LineDisplayError> {
        for (node_index, node) in self.spec.content.nodes.iter().enumerate() {
            self.resolve_node(node_index, node)?;
        }
        Ok(self.finish())
    }

    fn resolve_node(
        &mut self,
        node_index: usize,
        node: &RichTextNode,
    ) -> Result<(), LineDisplayError> {
        if let RichTextNode::HostEvent(event @ DialogueHostEvent::Conditional { .. }) = node {
            self.push_conditional_event(event, node_index);
            return Ok(());
        }

        if !self.is_conditionally_active() {
            return Ok(());
        }

        match node {
            RichTextNode::Text { text } => {
                self.push_text_node(text, node_index, node);
                Ok(())
            }
            RichTextNode::Ruby { base, ruby } => {
                self.push_ruby_node(base, ruby, node_index, node);
                Ok(())
            }
            RichTextNode::StyleStart { style } => {
                self.push_style_start(style, node);
                Ok(())
            }
            RichTextNode::StyleEnd { name } => {
                self.push_style_end(name, node);
                Ok(())
            }
            RichTextNode::Control(control) => {
                self.push_control_node(control, node_index, node);
                Ok(())
            }
            RichTextNode::Interpolation {
                expr,
                fallback_source,
                on_error,
            } => self.push_interpolation_node(expr, fallback_source, on_error, node_index),
            RichTextNode::HostEvent(event) => {
                self.push_host_event(event, node_index);
                Ok(())
            }
        }
    }

    fn push_text_node(&mut self, value: &str, node_index: usize, node: &RichTextNode) {
        self.push_visible_text(value, RichTextTextSource::Text, node_index);
        self.nodes.push(node.clone());
    }

    fn push_ruby_node(&mut self, base: &str, ruby: &str, node_index: usize, node: &RichTextNode) {
        let range = self.push_visible_text(base, RichTextTextSource::RubyBase, node_index);
        let styles = self.current_styles();
        let presentation = presentation_from_styles(styles.iter());
        self.display_map
            .ruby_annotations
            .push(RichTextRubyAnnotation {
                base_range: range,
                ruby: ruby.to_owned(),
                node_index,
                styles,
                presentation,
            });
        self.nodes.push(node.clone());
    }

    fn push_style_start(&mut self, style: &RichTextStyle, node: &RichTextNode) {
        self.active_styles.push(style.clone());
        self.nodes.push(node.clone());
    }

    fn push_style_end(&mut self, name: &str, node: &RichTextNode) {
        remove_active_style(&mut self.active_styles, name);
        self.nodes.push(node.clone());
    }

    fn push_control_node(
        &mut self,
        control: &RichTextControl,
        node_index: usize,
        node: &RichTextNode,
    ) {
        let range = push_control_text(
            &mut self.text,
            &mut self.display_map,
            control,
            node_index,
            &self.spec.base_styles,
            &self.active_styles,
        );
        self.display_map.controls.push(RichTextControlMarker {
            node_index,
            control: control.clone(),
            range,
        });
        self.nodes.push(node.clone());
        if matches!(control, RichTextControl::Reset) {
            self.active_styles.clear();
        }
    }

    fn push_interpolation_node(
        &mut self,
        expr: &str,
        fallback_source: &str,
        on_error: &InlineFailurePolicy,
        node_index: usize,
    ) -> Result<(), LineDisplayError> {
        if let Some(value) = self.context.get(expr) {
            let label = display_runtime_value(value);
            self.push_visible_text(&label, RichTextTextSource::Interpolation, node_index);
            self.nodes.push(RichTextNode::Text { text: label });
            return Ok(());
        }
        self.push_unresolved_interpolation(expr, fallback_source, on_error, node_index)
    }

    fn push_unresolved_interpolation(
        &mut self,
        expr: &str,
        fallback_source: &str,
        on_error: &InlineFailurePolicy,
        node_index: usize,
    ) -> Result<(), LineDisplayError> {
        let reason = "runtime interpolation value was not resolved".to_owned();
        if matches!(on_error, InlineFailurePolicy::FailLine) {
            return Err(LineDisplayError {
                line: self.spec.line.clone(),
                expr: expr.to_owned(),
                reason,
            });
        }
        self.unresolved.push(expr.to_owned());
        self.inline_failures.push(InlineTextFailure {
            expr: expr.to_owned(),
            reason,
            policy: on_error.clone(),
        });
        if let InlineFailurePolicy::Fallback { fallback } = on_error
            && let Some(label) = fallback_text(expr, fallback_source, fallback)
        {
            self.push_visible_text(
                &label,
                RichTextTextSource::InterpolationFallback,
                node_index,
            );
            self.nodes.push(RichTextNode::Text { text: label });
        }
        Ok(())
    }

    fn push_host_event(&mut self, event: &DialogueHostEvent, node_index: usize) {
        let event_index = self.host_events.len();
        self.host_events.push(event.clone());
        self.display_map.host_events.push(RichTextHostEventMarker {
            node_index,
            event_index,
            event: event.clone(),
        });
    }

    fn push_conditional_event(&mut self, event: &DialogueHostEvent, node_index: usize) {
        self.push_host_event(event, node_index);
        let DialogueHostEvent::Conditional { name, attrs } = event else {
            return;
        };

        match name.as_str() {
            "if" => {
                let parent_active = self.is_conditionally_active();
                self.conditional_stack.push(ConditionalBranch {
                    parent_active,
                    condition_matches: parent_active && self.context.condition_is_truthy(attrs),
                    in_else: false,
                });
            }
            "else" => {
                if let Some(branch) = self.conditional_stack.last_mut() {
                    branch.in_else = true;
                }
            }
            "endif" => {
                self.conditional_stack.pop();
            }
            _ => {}
        }
    }

    fn is_conditionally_active(&self) -> bool {
        self.conditional_stack
            .last()
            .is_none_or(|branch| branch.is_active())
    }

    fn push_visible_text(
        &mut self,
        value: &str,
        source: RichTextTextSource,
        node_index: usize,
    ) -> RichTextRange {
        push_display_text_run(
            &mut self.text,
            &mut self.display_map,
            value,
            source,
            node_index,
            &self.spec.base_styles,
            &self.active_styles,
        )
    }

    fn current_styles(&self) -> Vec<RichTextStyle> {
        current_styles(&self.spec.base_styles, &self.active_styles)
    }

    fn finish(self) -> LineDisplayFrame {
        LineDisplayFrame {
            line: self.spec.line.clone(),
            callee: self.spec.callee.clone(),
            text: self.text,
            base_styles: self.spec.base_styles.clone(),
            default_inline_failure_policy: self.spec.default_inline_failure_policy.clone(),
            style_contributions: self.spec.style_contributions.clone(),
            nodes: self.nodes,
            display_map: self.display_map,
            host_events: self.host_events,
            inline_failures: self.inline_failures,
            unresolved: self.unresolved,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ConditionalBranch {
    parent_active: bool,
    condition_matches: bool,
    in_else: bool,
}

impl ConditionalBranch {
    const fn is_active(self) -> bool {
        self.parent_active
            && if self.in_else {
                !self.condition_matches
            } else {
                self.condition_matches
            }
    }
}

fn push_display_text_run(
    text: &mut String,
    display_map: &mut RichTextDisplayMap,
    value: &str,
    source: RichTextTextSource,
    node_index: usize,
    base_styles: &[RichTextStyle],
    active_styles: &[RichTextStyle],
) -> RichTextRange {
    let start = text.len();
    text.push_str(value);
    let range = RichTextRange::new(start, text.len());
    if !value.is_empty() {
        let styles = current_styles(base_styles, active_styles);
        display_map.text_runs.push(RichTextTextRun {
            range,
            source,
            node_index,
            presentation: presentation_from_styles(styles.iter()),
            styles,
        });
    }
    range
}

fn current_styles(
    base_styles: &[RichTextStyle],
    active_styles: &[RichTextStyle],
) -> Vec<RichTextStyle> {
    base_styles
        .iter()
        .chain(active_styles.iter())
        .cloned()
        .collect()
}

fn remove_active_style(active_styles: &mut Vec<RichTextStyle>, name: &str) {
    if name == "/" {
        active_styles.pop();
        return;
    }
    let name = canonical_style_name(name);
    if let Some(index) = active_styles
        .iter()
        .rposition(|style| style.tag_name() == name)
    {
        active_styles.remove(index);
    }
}

/// Canonicalizes style end names used by authored aliases and inferred spans.
pub fn canonical_style_name(name: &str) -> &str {
    match name {
        "" | "/" => "/",
        "i" | "italic" | "oblique" | "slant" | "style" => "style",
        "vertical"
        | "vertical_rl"
        | "vertical_lr"
        | "horizontal_tb"
        | "ruby_over"
        | "ruby_under"
        | "ruby_inter_character"
        | "layout" => "layout",
        "offset" | "pos" | "rotate" | "scale" | "transform" => "transform",
        "wave" | "shake" | "arc" | "spin" | "pulse" | "typewriter" | "jitter" | "shader"
        | "host" | "effect" | "fx" => "effect",
        other => other,
    }
}

/// Aggregates presentation metadata from active rich-text styles.
pub fn presentation_from_styles<'a>(
    styles: impl IntoIterator<Item = &'a RichTextStyle>,
) -> RichTextPresentation {
    styles
        .into_iter()
        .fold(RichTextPresentation::default(), |mut out, style| {
            match style {
                RichTextStyle::Em { .. } | RichTextStyle::Italic { .. } => out.italic = true,
                RichTextStyle::Oblique { angle, .. } => out.oblique = Some(*angle),
                RichTextStyle::Layout { layout } => merge_layout_presentation(&mut out, layout),
                RichTextStyle::Transform { transform } => {
                    out.transform = Some(transform.clone());
                }
                RichTextStyle::Effect { effect } => out.effects.push(effect.clone()),
                RichTextStyle::Shader { shader } => out.shaders.push(shader.clone()),
                RichTextStyle::Strong { .. }
                | RichTextStyle::Color { .. }
                | RichTextStyle::Font { .. }
                | RichTextStyle::Size { .. }
                | RichTextStyle::Speed { .. }
                | RichTextStyle::Unknown { .. } => {}
            }
            out
        })
}

fn merge_layout_presentation(out: &mut RichTextPresentation, layout: &RichTextLayout) {
    let mut merged = out.layout.clone().unwrap_or_default();
    if !matches!(layout.writing_mode, RichTextWritingMode::HorizontalTb)
        || out.layout.is_none()
        || matches!(layout.ruby_position, RichTextRubyPosition::Auto)
    {
        merged.writing_mode = layout.writing_mode;
    }
    if !matches!(layout.direction, RichTextInlineDirection::Auto) {
        merged.direction = layout.direction;
    }
    if !matches!(layout.vertical_latin, RichTextVerticalLatinMode::Mixed) {
        merged.vertical_latin = layout.vertical_latin;
    }
    if !matches!(layout.ruby_position, RichTextRubyPosition::Auto) {
        merged.ruby_position = layout.ruby_position;
    }
    if !matches!(layout.jlreq_strictness, RichTextJlreqStrictness::Auto) {
        merged.jlreq_strictness = layout.jlreq_strictness;
    }
    if layout.column_gap != RichTextLayout::default().column_gap {
        merged.column_gap = layout.column_gap;
    }
    if layout.ruby_font_size.is_some() {
        merged.ruby_font_size = layout.ruby_font_size;
    }
    if layout.ruby_gap.is_some() {
        merged.ruby_gap = layout.ruby_gap;
    }
    if layout.ruby_overhang.is_some() {
        merged.ruby_overhang = layout.ruby_overhang;
    }
    if layout.ruby_collision_gap.is_some() {
        merged.ruby_collision_gap = layout.ruby_collision_gap;
    }
    out.layout = Some(merged);
}

fn fallback_text(expr: &str, fallback_source: &str, fallback: &InlineFallback) -> Option<String> {
    match fallback {
        InlineFallback::Text { text, .. } => Some(text.clone()),
        InlineFallback::ExprSource { .. } => Some(fallback_source.to_owned()),
        InlineFallback::CallSource { .. } => Some(expr.to_owned()),
        InlineFallback::ValuePlain => None,
    }
}

fn push_control_text(
    text: &mut String,
    display_map: &mut RichTextDisplayMap,
    control: &RichTextControl,
    node_index: usize,
    base_styles: &[RichTextStyle],
    active_styles: &[RichTextStyle],
) -> Option<RichTextRange> {
    match control {
        RichTextControl::HardBreak => Some(push_display_text_run(
            text,
            display_map,
            "\n",
            RichTextTextSource::ControlHardBreak,
            node_index,
            base_styles,
            active_styles,
        )),
        RichTextControl::Raw { text: raw } => Some(push_display_text_run(
            text,
            display_map,
            raw,
            RichTextTextSource::ControlRaw,
            node_index,
            base_styles,
            active_styles,
        )),
        RichTextControl::Page
        | RichTextControl::LineWait
        | RichTextControl::TimedWait { .. }
        | RichTextControl::Clear
        | RichTextControl::Reset
        | RichTextControl::Mark { .. }
        | RichTextControl::Unknown { .. } => None,
    }
}

fn display_runtime_value(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Unit => "()".to_owned(),
        RuntimeValue::Bool(value) => value.to_string(),
        RuntimeValue::Int(value) => value.to_string(),
        RuntimeValue::UInt(value) => value.to_string(),
        RuntimeValue::F32(value) => value.to_string(),
        RuntimeValue::F64(value) => value.to_string(),
        RuntimeValue::MatrixF32(_) | RuntimeValue::MatrixF64(_) => "<matrix>".to_owned(),
        RuntimeValue::TensorF32(_) | RuntimeValue::TensorF64(_) => "<tensor>".to_owned(),
        RuntimeValue::String(value) => value.clone(),
        RuntimeValue::Char(value) => value.to_string(),
        RuntimeValue::Duration(value) => format!("{}ns", value.as_nanos()),
        RuntimeValue::EntityRef(value) => format!("@{value}"),
        RuntimeValue::Seq(_) => "[...]".to_owned(),
        RuntimeValue::Tuple(_) => "(...)".to_owned(),
        RuntimeValue::Record(_) => "{...}".to_owned(),
        RuntimeValue::Variant { name, .. } => format!(".{name}"),
    }
}

fn runtime_value_is_truthy(value: &RuntimeValue) -> bool {
    match value {
        RuntimeValue::Unit => false,
        RuntimeValue::Bool(value) => *value,
        RuntimeValue::Int(value) => value.try_into_i64().is_some_and(|value| value != 0),
        RuntimeValue::UInt(value) => value.try_into_i64().is_some_and(|value| value != 0),
        RuntimeValue::F32(value) => value.is_finite() && *value != 0.0,
        RuntimeValue::F64(value) => value.is_finite() && *value != 0.0,
        RuntimeValue::MatrixF32(_)
        | RuntimeValue::MatrixF64(_)
        | RuntimeValue::TensorF32(_)
        | RuntimeValue::TensorF64(_)
        | RuntimeValue::Variant { .. } => true,
        RuntimeValue::String(value) | RuntimeValue::EntityRef(value) => !value.is_empty(),
        RuntimeValue::Char(value) => *value != '\0',
        RuntimeValue::Duration(value) => value.as_nanos() != 0,
        RuntimeValue::Seq(value) => !value.is_empty(),
        RuntimeValue::Tuple(value) => !value.is_empty(),
        RuntimeValue::Record(value) => !value.is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::value::RuntimeValue;

    #[test]
    fn resolves_text_ruby_controls_and_interpolation() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.opening.001".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: vec![RichTextStyle::from_tag("font", "monospace")],
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::Text {
                    text: "Hi ".to_owned(),
                },
                RichTextNode::Interpolation {
                    expr: "player".to_owned(),
                    fallback_source: "player".to_owned(),
                    on_error: InlineFailurePolicy::FailLine,
                },
                RichTextNode::Ruby {
                    base: "夢".to_owned(),
                    ruby: "ゆめ".to_owned(),
                },
                RichTextNode::Control(RichTextControl::HardBreak),
                RichTextNode::Control(RichTextControl::Raw {
                    text: "[p]".to_owned(),
                }),
            ]),
        };
        let frame = spec
            .resolve_frame(&RuntimeLineContext::new(vec![RuntimeBinding {
                name: "player".to_owned(),
                value: RuntimeValue::String("Aoi".to_owned()),
            }]))
            .expect("frame resolves");

        assert_eq!(frame.text, "Hi Aoi夢\n[p]");
        assert_eq!(
            frame
                .display_map
                .text_runs
                .iter()
                .map(|run| (run.source, run.range))
                .collect::<Vec<_>>(),
            vec![
                (RichTextTextSource::Text, RichTextRange::new(0, 3)),
                (RichTextTextSource::Interpolation, RichTextRange::new(3, 6)),
                (RichTextTextSource::RubyBase, RichTextRange::new(6, 9)),
                (
                    RichTextTextSource::ControlHardBreak,
                    RichTextRange::new(9, 10)
                ),
                (RichTextTextSource::ControlRaw, RichTextRange::new(10, 13)),
            ]
        );
        assert_eq!(
            frame.display_map.ruby_annotations,
            vec![RichTextRubyAnnotation {
                base_range: RichTextRange::new(6, 9),
                ruby: "ゆめ".to_owned(),
                node_index: 2,
                styles: vec![RichTextStyle::Font {
                    family: RichTextFontFamily::Monospace
                }],
                presentation: RichTextPresentation::default(),
            }]
        );
        assert_eq!(
            frame.display_map.controls,
            vec![
                RichTextControlMarker {
                    node_index: 3,
                    control: RichTextControl::HardBreak,
                    range: Some(RichTextRange::new(9, 10)),
                },
                RichTextControlMarker {
                    node_index: 4,
                    control: RichTextControl::Raw {
                        text: "[p]".to_owned()
                    },
                    range: Some(RichTextRange::new(10, 13)),
                },
            ]
        );
        assert_eq!(
            frame.base_styles,
            vec![RichTextStyle::Font {
                family: RichTextFontFamily::Monospace
            }]
        );
        assert!(frame.unresolved.is_empty());
        assert!(frame.inline_failures.is_empty());
    }

    #[test]
    fn interpolation_failure_policy_can_discard_or_fallback() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.opening.002".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::Text {
                    text: "A".to_owned(),
                },
                RichTextNode::Interpolation {
                    expr: "missing_discard".to_owned(),
                    fallback_source: "missing_discard".to_owned(),
                    on_error: InlineFailurePolicy::Discard,
                },
                RichTextNode::Interpolation {
                    expr: "missing_fallback".to_owned(),
                    fallback_source: "missing_fallback".to_owned(),
                    on_error: InlineFailurePolicy::fallback_text("?"),
                },
            ]),
        };
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves with non-failing policies");

        assert_eq!(frame.text, "A?");
        assert_eq!(
            frame.unresolved,
            vec!["missing_discard", "missing_fallback"]
        );
        assert_eq!(
            frame.inline_failures,
            vec![
                InlineTextFailure {
                    expr: "missing_discard".to_owned(),
                    reason: "runtime interpolation value was not resolved".to_owned(),
                    policy: InlineFailurePolicy::Discard
                },
                InlineTextFailure {
                    expr: "missing_fallback".to_owned(),
                    reason: "runtime interpolation value was not resolved".to_owned(),
                    policy: InlineFailurePolicy::fallback_text("?")
                }
            ]
        );
    }

    #[test]
    fn interpolation_failure_policy_can_fail_line() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.opening.003".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![RichTextNode::Interpolation {
                expr: "missing".to_owned(),
                fallback_source: "missing".to_owned(),
                on_error: InlineFailurePolicy::FailLine,
            }]),
        };

        let error = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect_err("line fails");

        assert_eq!(error.line, RuntimeLineId("say.opening.003".to_owned()));
        assert_eq!(error.expr, "missing");
    }

    #[test]
    fn interpolation_fallback_can_render_expr_or_call_source() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.opening.004".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::Interpolation {
                    expr: "fmt(score, style = \"number\")".to_owned(),
                    fallback_source: "score".to_owned(),
                    on_error: InlineFailurePolicy::fallback_expr_source(FallbackStylePolicy::Plain),
                },
                RichTextNode::Text {
                    text: "|".to_owned(),
                },
                RichTextNode::Interpolation {
                    expr: "fmt(score, style = \"number\")".to_owned(),
                    fallback_source: "score".to_owned(),
                    on_error: InlineFailurePolicy::fallback_call_source(FallbackStylePolicy::Plain),
                },
            ]),
        };

        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("fallback source frame resolves");

        assert_eq!(frame.text, "score|fmt(score, style = \"number\")");
    }

    #[test]
    fn local_text_conditionals_render_only_selected_branch() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.opening.conditional".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::Text {
                    text: "A".to_owned(),
                },
                RichTextNode::HostEvent(DialogueHostEvent::Conditional {
                    name: "if".to_owned(),
                    attrs: "flag".to_owned(),
                }),
                RichTextNode::Text {
                    text: "yes".to_owned(),
                },
                RichTextNode::HostEvent(DialogueHostEvent::Conditional {
                    name: "else".to_owned(),
                    attrs: String::new(),
                }),
                RichTextNode::Text {
                    text: "no".to_owned(),
                },
                RichTextNode::HostEvent(DialogueHostEvent::Conditional {
                    name: "endif".to_owned(),
                    attrs: String::new(),
                }),
                RichTextNode::Text {
                    text: "Z".to_owned(),
                },
            ]),
        };

        let true_frame = spec
            .resolve_frame(&RuntimeLineContext::new(vec![RuntimeBinding {
                name: "flag".to_owned(),
                value: RuntimeValue::Bool(true),
            }]))
            .expect("true branch resolves");
        let false_frame = spec
            .resolve_frame(&RuntimeLineContext::new(vec![RuntimeBinding {
                name: "flag".to_owned(),
                value: RuntimeValue::Bool(false),
            }]))
            .expect("false branch resolves");
        let missing_frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("missing condition resolves as false");

        assert_eq!(true_frame.text, "AyesZ");
        assert_eq!(false_frame.text, "AnoZ");
        assert_eq!(missing_frame.text, "AnoZ");
        assert_eq!(true_frame.host_events.len(), 3);
        assert_eq!(false_frame.host_events.len(), 3);
    }

    #[test]
    fn inactive_conditional_branch_suppresses_styles_interpolation_and_host_events() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.opening.conditional_gated".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::HostEvent(DialogueHostEvent::Conditional {
                    name: "if".to_owned(),
                    attrs: "flag".to_owned(),
                }),
                RichTextNode::StyleStart {
                    style: RichTextStyle::from_tag("color", "#ff0000"),
                },
                RichTextNode::HostEvent(DialogueHostEvent::Voice {
                    attrs: "hidden".to_owned(),
                }),
                RichTextNode::Interpolation {
                    expr: "missing".to_owned(),
                    fallback_source: "missing".to_owned(),
                    on_error: InlineFailurePolicy::FailLine,
                },
                RichTextNode::HostEvent(DialogueHostEvent::Conditional {
                    name: "else".to_owned(),
                    attrs: String::new(),
                }),
                RichTextNode::Text {
                    text: "shown".to_owned(),
                },
                RichTextNode::HostEvent(DialogueHostEvent::Conditional {
                    name: "endif".to_owned(),
                    attrs: String::new(),
                }),
                RichTextNode::Text {
                    text: " plain".to_owned(),
                },
            ]),
        };

        let frame = spec
            .resolve_frame(&RuntimeLineContext::new(vec![RuntimeBinding {
                name: "flag".to_owned(),
                value: RuntimeValue::Bool(false),
            }]))
            .expect("inactive interpolation does not fail the line");

        assert_eq!(frame.text, "shown plain");
        assert!(
            frame
                .host_events
                .iter()
                .all(|event| matches!(event, DialogueHostEvent::Conditional { .. }))
        );
        assert!(frame.inline_failures.is_empty());
        assert!(frame.unresolved.is_empty());
        assert!(frame.display_map.text_runs.iter().all(|run| {
            !run.styles
                .iter()
                .any(|style| matches!(style, RichTextStyle::Color { .. }))
        }));
    }

    #[test]
    fn rich_text_style_parses_font_families_without_renderer_context() {
        assert_eq!(
            RichTextStyle::from_tag("font", "monospace"),
            RichTextStyle::Font {
                family: RichTextFontFamily::Monospace
            }
        );
        assert_eq!(
            RichTextStyle::from_tag("font", r#""Noto Sans JP""#),
            RichTextStyle::Font {
                family: RichTextFontFamily::Named {
                    name: "Noto Sans JP".to_owned()
                }
            }
        );
    }

    #[test]
    fn reset_control_clears_active_inline_styles_for_following_runs() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.opening.005".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::StyleStart {
                    style: RichTextStyle::from_tag("color", "#80c0ff"),
                },
                RichTextNode::Text {
                    text: "blue".to_owned(),
                },
                RichTextNode::Control(RichTextControl::Reset),
                RichTextNode::Text {
                    text: "plain".to_owned(),
                },
            ]),
        };
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves");

        assert_eq!(frame.text, "blueplain");
        assert_eq!(frame.display_map.text_runs.len(), 2);
        assert!(
            frame.display_map.text_runs[0]
                .styles
                .iter()
                .any(|style| matches!(style, RichTextStyle::Color { .. }))
        );
        assert!(frame.display_map.text_runs[1].styles.is_empty());
        assert_eq!(
            frame.display_map.controls,
            vec![RichTextControlMarker {
                node_index: 2,
                control: RichTextControl::Reset,
                range: None,
            }]
        );
    }
}
