//! Runtime binding resolution into a dialogue display frame.

use arcweft_core::plan::{RuntimeDialogueValueBinding, RuntimeLineId};
use arcweft_core::runtime_id::RuntimeDialogueValueSlotId;
use arcweft_core::value::RuntimeDialogueContentBinding;
use arcweft_core::value::RuntimeDialogueContentValue;
use arcweft_dialogue::{
    InlineFailurePolicy, InlineFailureSelection, InlineFallback, InlineTextFailure,
};
use arcweft_text_model::{
    CharacterDialoguePresentationConfig, DialogueContentFragmentTemplate, DialogueContentSpec,
    DialogueHostEvent, DialoguePresentationCharacter, LineDisplayFrame,
    MaterializedDialogueContent, ResolvedRichTextNode, RichTextControl, RichTextControlMarker,
    RichTextDisplayMap, RichTextDocument, RichTextHostEventMarker, RichTextNode, RichTextNodeCount,
    RichTextNodeIndex, RichTextNodeRange, RichTextRange, RichTextRubyAnnotation, RichTextStyle,
    RichTextStyleContribution, RichTextTextRun, RichTextTextRunRange, RichTextTextSource,
    presentation_from_styles,
};
use thiserror::Error;

/// Dialogue line context captured by the runtime at display time.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeLineContext {
    values: Vec<RuntimeDialogueValueBinding>,
    character: DialoguePresentationCharacter,
    effective: CharacterDialoguePresentationConfig,
    base_styles: Vec<RichTextStyle>,
    style_contributions: Vec<RichTextStyleContribution>,
    materialized_bindings: Vec<RuntimeDialogueContentBinding>,
    has_materialized_bindings: bool,
}

/// Error raised when a line chooses fail-fast interpolation behavior.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("inline dialogue expression `{expr}` failed: {reason}")]
pub struct LineDisplayError {
    pub line: RuntimeLineId,
    pub expr: String,
    pub reason: String,
}

impl RuntimeLineContext {
    /// Creates context from visible bindings and one checked dynamic
    /// `CharacterDialogue` presentation value.
    #[must_use]
    pub fn new(
        values: Vec<RuntimeDialogueValueBinding>,
        character: DialoguePresentationCharacter,
        effective: CharacterDialoguePresentationConfig,
        base_styles: Vec<RichTextStyle>,
        style_contributions: Vec<RichTextStyleContribution>,
    ) -> Self {
        Self {
            values,
            character,
            effective,
            base_styles,
            style_contributions,
            materialized_bindings: Vec::new(),
            has_materialized_bindings: false,
        }
    }

    /// Attaches the closed text-model bindings produced by Content
    /// materialization. Once attached, interpolation and condition lookup do
    /// not inspect raw `RuntimeValue` carriers.
    #[must_use]
    pub fn with_materialized_bindings(
        mut self,
        bindings: &[RuntimeDialogueContentBinding],
    ) -> Self {
        self.materialized_bindings = bindings.to_vec();
        self.has_materialized_bindings = true;
        self
    }

    /// Returns the effective CharacterDialogue policy used to resolve
    /// inherited inline and Content insertion failures.
    #[must_use]
    pub const fn inline_failure_policy(&self) -> &InlineFailurePolicy {
        &self.effective.inline_failure
    }

    fn inline_text(&self, slot: RuntimeDialogueValueSlotId) -> Result<Option<String>, String> {
        if self.has_materialized_bindings {
            let Some(binding) = self
                .materialized_bindings
                .iter()
                .find(|binding| binding.slot() == slot)
            else {
                return Ok(None);
            };
            return match binding {
                RuntimeDialogueContentBinding::Interpolation { value, .. } => {
                    Ok(Some(value.text().to_owned()))
                }
                _ => Err(format!(
                    "checked dialogue interpolation slot {slot} did not carry a typed Interpolation value"
                )),
            };
        }
        Err(format!(
            "typed materialized dialogue bindings are required for interpolation slot {slot}"
        ))
    }
}

/// Resolves a display spec against runtime context for a headless/native frame.
pub fn resolve_frame(
    spec: &DialogueContentSpec,
    _context: &RuntimeLineContext,
) -> Result<LineDisplayFrame, LineDisplayError> {
    Err(LineDisplayError {
        line: spec.line().clone(),
        expr: String::new(),
        reason: "immutable dialogue content template authority is required; use resolve_frame_with_template"
            .to_owned(),
    })
}

/// Resolves a display spec against its catalog-owned immutable template.
pub fn resolve_frame_with_template(
    spec: &DialogueContentSpec,
    template: &DialogueContentFragmentTemplate,
    content: &RuntimeDialogueContentValue,
    context: &RuntimeLineContext,
) -> Result<LineDisplayFrame, LineDisplayError> {
    if spec.template_id() != template.id() || spec.template_digest() != template.digest() {
        return Err(LineDisplayError {
            line: spec.line().clone(),
            expr: String::new(),
            reason: "dialogue content spec and immutable template identity disagree".to_owned(),
        });
    }
    if content.template() != template.id() || content.template_digest() != template.digest() {
        return Err(LineDisplayError {
            line: spec.line().clone(),
            expr: String::new(),
            reason: "runtime Content envelope and immutable template identity disagree".to_owned(),
        });
    }
    LineDisplayFrameResolver::new(spec, template.content(), content, context).resolve()
}

/// Resolves a materialized Content document. ContentInsert nodes must have
/// been expanded by the text-model materializer before this boundary.
pub fn resolve_materialized_frame(
    spec: &DialogueContentSpec,
    materialized: &MaterializedDialogueContent,
    content: &RuntimeDialogueContentValue,
    context: &RuntimeLineContext,
) -> Result<LineDisplayFrame, LineDisplayError> {
    if content.template() != spec.template_id()
        || content.template_digest() != spec.template_digest()
    {
        return Err(LineDisplayError {
            line: spec.line().clone(),
            expr: String::new(),
            reason: "runtime Content envelope and dialogue spec identity disagree".to_owned(),
        });
    }
    let context = context
        .clone()
        .with_materialized_bindings(materialized.bindings());
    LineDisplayFrameResolver::new(spec, materialized.document(), content, &context).resolve()
}

struct LineDisplayFrameResolver<'a> {
    spec: &'a DialogueContentSpec,
    document: &'a RichTextDocument,
    content: &'a RuntimeDialogueContentValue,
    context: &'a RuntimeLineContext,
    text: String,
    nodes: Vec<ResolvedRichTextNode>,
    display_map: RichTextDisplayMap,
    host_events: Vec<DialogueHostEvent>,
    inline_failures: Vec<InlineTextFailure>,
    unresolved: Vec<String>,
}

impl<'a> LineDisplayFrameResolver<'a> {
    fn new(
        spec: &'a DialogueContentSpec,
        document: &'a RichTextDocument,
        content: &'a RuntimeDialogueContentValue,
        context: &'a RuntimeLineContext,
    ) -> Self {
        Self {
            spec,
            document,
            content,
            context,
            text: String::new(),
            nodes: Vec::new(),
            display_map: RichTextDisplayMap::default(),
            host_events: Vec::new(),
            inline_failures: Vec::new(),
            unresolved: Vec::new(),
        }
    }

    fn resolve(mut self) -> Result<LineDisplayFrame, LineDisplayError> {
        let mut node_index = 0;
        let mut nodes = Vec::new();
        self.resolve_nodes(
            &self.document.nodes,
            &[],
            &mut node_index,
            false,
            &mut nodes,
        )?;
        self.display_map.source_node_count =
            RichTextNodeCount::try_from_len(node_index).map_err(|_| LineDisplayError {
                line: self.spec.line().clone(),
                expr: String::new(),
                reason: "rich-text node count exceeds u32::MAX".to_owned(),
            })?;
        // Nested traversal naturally completes inner annotations before their
        // wrappers. The display-map authority is nevertheless canonicalized
        // by the wrapper's preorder owner identity.
        self.display_map
            .ruby_annotations
            .sort_by_key(|annotation| annotation.owner_node);
        self.nodes = nodes;
        let frame = self.finish();
        frame.validate().map_err(|error| LineDisplayError {
            line: frame.line.clone(),
            expr: String::new(),
            reason: format!("resolved rich-text display map is invalid: {error}"),
        })?;
        Ok(frame)
    }

    fn resolve_nodes(
        &mut self,
        nodes: &[RichTextNode],
        styles: &[RichTextStyle],
        node_index: &mut usize,
        in_ruby: bool,
        resolved: &mut Vec<ResolvedRichTextNode>,
    ) -> Result<(), LineDisplayError> {
        for node in nodes {
            let current_index =
                RichTextNodeIndex::try_from_index(*node_index).map_err(|_| LineDisplayError {
                    line: self.spec.line().clone(),
                    expr: String::new(),
                    reason: "rich-text node index exceeds u32::MAX".to_owned(),
                })?;
            *node_index = node_index.checked_add(1).ok_or_else(|| LineDisplayError {
                line: self.spec.line().clone(),
                expr: String::new(),
                reason: "rich-text node index overflow".to_owned(),
            })?;
            match node {
                RichTextNode::Text { text } => {
                    self.push_visible_text(text, RichTextTextSource::Text, current_index, styles);
                    resolved.push(ResolvedRichTextNode::Text { text: text.clone() });
                }
                RichTextNode::Raw { text } => {
                    self.push_visible_text(text, RichTextTextSource::Raw, current_index, styles);
                    resolved.push(ResolvedRichTextNode::Raw { text: text.clone() });
                }
                RichTextNode::Scope { style, body } => {
                    let mut nested_styles = styles.to_vec();
                    nested_styles.push(style.as_ref().clone());
                    let mut resolved_body = Vec::new();
                    self.resolve_nodes(
                        body,
                        &nested_styles,
                        node_index,
                        in_ruby,
                        &mut resolved_body,
                    )?;
                    resolved.push(ResolvedRichTextNode::Scope {
                        style: style.clone(),
                        body: resolved_body,
                    });
                }
                RichTextNode::Ruby { body, ruby } => {
                    let start = self.text.len();
                    let base_run_start = self.display_map.text_runs.len();
                    let body_start = *node_index;
                    let mut resolved_body = Vec::new();
                    self.resolve_nodes(body, styles, node_index, true, &mut resolved_body)?;
                    let range = RichTextRange::new(start, self.text.len());
                    if range.start != range.end {
                        let current_styles = self.current_styles(styles);
                        let body_start =
                            RichTextNodeIndex::try_from_index(body_start).map_err(|_| {
                                LineDisplayError {
                                    line: self.spec.line().clone(),
                                    expr: String::new(),
                                    reason: "ruby body node range exceeds u32::MAX".to_owned(),
                                }
                            })?;
                        let body_end =
                            RichTextNodeIndex::try_from_index(*node_index).map_err(|_| {
                                LineDisplayError {
                                    line: self.spec.line().clone(),
                                    expr: String::new(),
                                    reason: "ruby body node range exceeds u32::MAX".to_owned(),
                                }
                            })?;
                        let base_run_start =
                            u32::try_from(base_run_start).map_err(|_| LineDisplayError {
                                line: self.spec.line().clone(),
                                expr: String::new(),
                                reason: "ruby base run range exceeds u32::MAX".to_owned(),
                            })?;
                        let base_run_end = u32::try_from(self.display_map.text_runs.len())
                            .map_err(|_| LineDisplayError {
                                line: self.spec.line().clone(),
                                expr: String::new(),
                                reason: "ruby base run range exceeds u32::MAX".to_owned(),
                            })?;
                        self.display_map
                            .ruby_annotations
                            .push(RichTextRubyAnnotation {
                                owner_node: current_index,
                                body_nodes: RichTextNodeRange::new(body_start, body_end),
                                base_runs: RichTextTextRunRange::new(base_run_start, base_run_end),
                                base_range: range,
                                ruby: ruby.clone(),
                                styles: current_styles.clone(),
                                presentation: presentation_from_styles(current_styles.iter()),
                            });
                    }
                    resolved.push(ResolvedRichTextNode::Ruby {
                        body: resolved_body,
                        ruby: ruby.clone(),
                    });
                }
                RichTextNode::Control { control } => {
                    if in_ruby
                        && matches!(
                            control,
                            RichTextControl::Page
                                | RichTextControl::LineWait
                                | RichTextControl::Clear
                        )
                    {
                        return Err(LineDisplayError {
                            line: self.spec.line().clone(),
                            expr: String::new(),
                            reason:
                                "page, line-wait, and clear controls are not allowed inside Ruby"
                                    .to_owned(),
                        });
                    }
                    let text_offset = self.text.len();
                    let range = push_control_text(
                        &mut self.text,
                        &mut self.display_map,
                        control,
                        current_index,
                        &self.context.base_styles,
                        styles,
                    );
                    self.display_map.controls.push(RichTextControlMarker {
                        node_index: current_index,
                        text_offset,
                        control: control.clone(),
                        range,
                    });
                    resolved.push(ResolvedRichTextNode::Control {
                        control: control.clone(),
                    });
                }
                RichTextNode::Interpolation {
                    slot,
                    label,
                    on_error,
                } => self.push_interpolation_node(
                    *slot,
                    label,
                    on_error,
                    current_index,
                    styles,
                    resolved,
                )?,
                RichTextNode::ContentInsert { .. } => {
                    return Err(LineDisplayError {
                        line: self.spec.line().clone(),
                        expr: String::new(),
                        reason: "content must be materialized before renderer resolution"
                            .to_owned(),
                    });
                }
                RichTextNode::HostEvent { event } => {
                    self.push_host_event(event, current_index);
                    resolved.push(ResolvedRichTextNode::HostEvent {
                        event: event.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    fn push_interpolation_node(
        &mut self,
        slot: RuntimeDialogueValueSlotId,
        label: &str,
        on_error: &InlineFailureSelection,
        node_index: RichTextNodeIndex,
        styles: &[RichTextStyle],
        resolved: &mut Vec<ResolvedRichTextNode>,
    ) -> Result<(), LineDisplayError> {
        if let Some(label) = self
            .context
            .inline_text(slot)
            .map_err(|reason| LineDisplayError {
                line: self.spec.line().clone(),
                expr: format!("slot {slot}"),
                reason,
            })?
        {
            self.push_visible_text(
                &label,
                RichTextTextSource::Interpolation,
                node_index,
                styles,
            );
            resolved.push(ResolvedRichTextNode::Text { text: label });
            return Ok(());
        }
        let policy = on_error.resolve(self.context.inline_failure_policy());
        self.push_unresolved_interpolation(label, &policy, node_index, styles, resolved)
    }

    fn push_unresolved_interpolation(
        &mut self,
        expr: &str,
        on_error: &InlineFailurePolicy,
        node_index: RichTextNodeIndex,
        styles: &[RichTextStyle],
        resolved: &mut Vec<ResolvedRichTextNode>,
    ) -> Result<(), LineDisplayError> {
        let reason = "runtime interpolation value was not resolved".to_owned();
        if matches!(on_error, InlineFailurePolicy::FailLine) {
            return Err(LineDisplayError {
                line: self.spec.line().clone(),
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
            && let Some(label) = fallback_text(expr, fallback)
        {
            self.push_visible_text(
                &label,
                RichTextTextSource::InterpolationFallback,
                node_index,
                styles,
            );
            resolved.push(ResolvedRichTextNode::Text { text: label });
        } else {
            resolved.push(ResolvedRichTextNode::Omitted);
        }
        Ok(())
    }

    fn push_host_event(&mut self, event: &DialogueHostEvent, node_index: RichTextNodeIndex) {
        let event_index = self.host_events.len();
        self.host_events.push(event.clone());
        self.display_map.host_events.push(RichTextHostEventMarker {
            node_index,
            text_offset: self.text.len(),
            event_index,
            event: event.clone(),
        });
    }

    fn push_visible_text(
        &mut self,
        value: &str,
        source: RichTextTextSource,
        node_index: RichTextNodeIndex,
        styles: &[RichTextStyle],
    ) -> RichTextRange {
        push_display_text_run(
            &mut self.text,
            &mut self.display_map,
            value,
            source,
            node_index,
            &self.context.base_styles,
            styles,
        )
    }

    fn current_styles(&self, styles: &[RichTextStyle]) -> Vec<RichTextStyle> {
        current_styles(&self.context.base_styles, styles)
    }

    fn finish(self) -> LineDisplayFrame {
        LineDisplayFrame {
            line: self.spec.line().clone(),
            character: self.context.character.clone(),
            text_key: self.spec.text_key().clone(),
            effective: self.context.effective.clone(),
            text: self.text,
            base_styles: self.context.base_styles.clone(),
            style_contributions: self
                .context
                .style_contributions
                .iter()
                .chain(self.spec.inline_styles())
                .cloned()
                .collect(),
            nodes: self.nodes,
            display_map: self.display_map,
            host_events: self.host_events,
            inline_failures: self.inline_failures,
            unresolved: self.unresolved,
            content: self.content.clone(),
        }
    }
}

fn push_display_text_run(
    text: &mut String,
    display_map: &mut RichTextDisplayMap,
    value: &str,
    source: RichTextTextSource,
    node_index: RichTextNodeIndex,
    base_styles: &[RichTextStyle],
    scoped_styles: &[RichTextStyle],
) -> RichTextRange {
    let start = text.len();
    text.push_str(value);
    let range = RichTextRange::new(start, text.len());
    if !value.is_empty() {
        let styles = current_styles(base_styles, scoped_styles);
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
    scoped_styles: &[RichTextStyle],
) -> Vec<RichTextStyle> {
    base_styles
        .iter()
        .chain(scoped_styles.iter())
        .cloned()
        .collect()
}

fn fallback_text(expr: &str, fallback: &InlineFallback) -> Option<String> {
    match fallback {
        InlineFallback::Text { text, .. } => Some(text.clone()),
        InlineFallback::ExprSource { .. } | InlineFallback::CallSource { .. } => {
            Some(expr.to_owned())
        }
        InlineFallback::ValuePlain => None,
    }
}

fn push_control_text(
    text: &mut String,
    display_map: &mut RichTextDisplayMap,
    control: &RichTextControl,
    node_index: RichTextNodeIndex,
    base_styles: &[RichTextStyle],
    scoped_styles: &[RichTextStyle],
) -> Option<RichTextRange> {
    match control {
        RichTextControl::HardBreak => Some(push_display_text_run(
            text,
            display_map,
            "\n",
            RichTextTextSource::ControlHardBreak,
            node_index,
            base_styles,
            scoped_styles,
        )),
        RichTextControl::Page
        | RichTextControl::LineWait
        | RichTextControl::TimedWait { .. }
        | RichTextControl::Clear
        | RichTextControl::Reset
        | RichTextControl::RevealRate { .. }
        | RichTextControl::Mark { .. }
        | RichTextControl::Effect { .. } => None,
    }
}
