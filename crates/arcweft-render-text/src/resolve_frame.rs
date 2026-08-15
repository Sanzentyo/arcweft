//! Runtime binding resolution into a dialogue display frame.

use arcweft_core::plan::RuntimeLineId;
use arcweft_core::{
    pure::{PureFunctionBackend, PureFunctionRequest, VmPureFunctionBackend},
    value::{RuntimeBinding, RuntimeExpr, RuntimeValue},
};
use arcweft_dialogue::{InlineFailurePolicy, InlineFallback, InlineTextFailure};
use arcweft_text_model::{
    CharacterDialoguePresentationConfig, DialogueContentSpec, DialogueHostEvent,
    DialoguePresentationCharacter, LineDisplayFrame, RichTextControl, RichTextControlMarker,
    RichTextDisplayMap, RichTextHostEventMarker, RichTextNode, RichTextRange,
    RichTextRubyAnnotation, RichTextSpanKind, RichTextStyle, RichTextStyleContribution,
    RichTextTextRun, RichTextTextSource, presentation_from_styles,
};
use thiserror::Error;

/// Dialogue line context captured by the runtime at display time.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeLineContext {
    bindings: Vec<RuntimeBinding>,
    character: DialoguePresentationCharacter,
    effective: CharacterDialoguePresentationConfig,
    base_styles: Vec<RichTextStyle>,
    style_contributions: Vec<RichTextStyleContribution>,
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
        bindings: Vec<RuntimeBinding>,
        character: DialoguePresentationCharacter,
        effective: CharacterDialoguePresentationConfig,
        base_styles: Vec<RichTextStyle>,
        style_contributions: Vec<RichTextStyleContribution>,
    ) -> Self {
        Self {
            bindings,
            character,
            effective,
            base_styles,
            style_contributions,
        }
    }

    fn condition_is_truthy(&self, expr: &RuntimeExpr) -> Result<bool, String> {
        match self.evaluate_expr("rich_text_condition", expr) {
            Ok(RuntimeValue::Bool(value)) => Ok(value),
            Ok(value) => Err(format!(
                "checked dialogue condition produced {}, expected Bool",
                display_runtime_value(&value)
            )),
            Err(error) => Err(error.to_string()),
        }
    }

    fn evaluate_expr(
        &self,
        request_name: &str,
        expr: &RuntimeExpr,
    ) -> Result<RuntimeValue, arcweft_core::value::RuntimeEvalError> {
        VmPureFunctionBackend
            .evaluate(&PureFunctionRequest::new(
                request_name,
                expr.clone(),
                self.bindings.clone(),
            ))
            .map(|result| result.value)
    }
}

/// Resolves a display spec against runtime context for a headless/native frame.
pub fn resolve_frame(
    spec: &DialogueContentSpec,
    context: &RuntimeLineContext,
) -> Result<LineDisplayFrame, LineDisplayError> {
    LineDisplayFrameResolver::new(spec, context).resolve()
}

struct LineDisplayFrameResolver<'a> {
    spec: &'a DialogueContentSpec,
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
    fn new(spec: &'a DialogueContentSpec, context: &'a RuntimeLineContext) -> Self {
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
        for (node_index, node) in self.spec.content().nodes.iter().enumerate() {
            self.resolve_node(node_index, node)?;
        }
        Ok(self.finish())
    }

    fn resolve_node(
        &mut self,
        node_index: usize,
        node: &RichTextNode,
    ) -> Result<(), LineDisplayError> {
        if let RichTextNode::HostEvent {
            event:
                event @ (DialogueHostEvent::ConditionalStart { .. }
                | DialogueHostEvent::ConditionalElse
                | DialogueHostEvent::ConditionalEnd),
        } = node
        {
            return self.push_conditional_event(event, node_index);
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
            RichTextNode::StyleEnd { span } => {
                self.push_style_end(*span, node);
                Ok(())
            }
            RichTextNode::Control { control } => {
                self.push_control_node(control, node_index, node);
                Ok(())
            }
            RichTextNode::Interpolation {
                expr,
                label,
                on_error,
            } => self.push_interpolation_node(expr, label, on_error, node_index),
            RichTextNode::HostEvent { event } => {
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
        let range = push_display_text_run(
            &mut self.text,
            &mut self.display_map,
            base,
            RichTextTextSource::RubyBase,
            node_index,
            &self.context.base_styles,
            &self.active_styles,
        );
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

    fn push_style_end(&mut self, kind: RichTextSpanKind, node: &RichTextNode) {
        remove_active_style(&mut self.active_styles, kind);
        self.nodes.push(node.clone());
    }

    fn push_control_node(
        &mut self,
        control: &RichTextControl,
        node_index: usize,
        node: &RichTextNode,
    ) {
        let text_offset = self.text.len();
        let range = push_control_text(
            &mut self.text,
            &mut self.display_map,
            control,
            node_index,
            &self.context.base_styles,
            &self.active_styles,
        );
        self.display_map.controls.push(RichTextControlMarker {
            node_index,
            text_offset,
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
        expr: &RuntimeExpr,
        label: &str,
        on_error: &InlineFailurePolicy,
        node_index: usize,
    ) -> Result<(), LineDisplayError> {
        if let Ok(value) = self.context.evaluate_expr("rich_text_interpolation", expr) {
            let label = display_runtime_value(&value);
            self.push_visible_text(&label, RichTextTextSource::Interpolation, node_index);
            self.nodes.push(RichTextNode::Text { text: label });
            return Ok(());
        }
        self.push_unresolved_interpolation(label, on_error, node_index)
    }

    fn push_unresolved_interpolation(
        &mut self,
        expr: &str,
        on_error: &InlineFailurePolicy,
        node_index: usize,
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
            text_offset: self.text.len(),
            event_index,
            event: event.clone(),
        });
    }

    fn push_conditional_event(
        &mut self,
        event: &DialogueHostEvent,
        node_index: usize,
    ) -> Result<(), LineDisplayError> {
        self.push_host_event(event, node_index);
        match event {
            DialogueHostEvent::ConditionalStart { condition } => {
                let parent_active = self.is_conditionally_active();
                let condition_matches = if parent_active {
                    self.context
                        .condition_is_truthy(condition)
                        .map_err(|reason| LineDisplayError {
                            line: self.spec.line().clone(),
                            expr: condition.to_string(),
                            reason,
                        })?
                } else {
                    false
                };
                self.conditional_stack.push(ConditionalBranch {
                    parent_active,
                    condition_matches,
                    in_else: false,
                });
            }
            DialogueHostEvent::ConditionalElse => {
                if let Some(branch) = self.conditional_stack.last_mut() {
                    branch.in_else = true;
                }
            }
            DialogueHostEvent::ConditionalEnd => {
                self.conditional_stack.pop();
            }
            DialogueHostEvent::Voice { .. }
            | DialogueHostEvent::Face { .. }
            | DialogueHostEvent::Pose { .. }
            | DialogueHostEvent::Show { .. }
            | DialogueHostEvent::Hide { .. }
            | DialogueHostEvent::Move { .. }
            | DialogueHostEvent::Scale { .. }
            | DialogueHostEvent::Rotate { .. }
            | DialogueHostEvent::Anim { .. }
            | DialogueHostEvent::Shake { .. }
            | DialogueHostEvent::TimedCue { .. }
            | DialogueHostEvent::Call { .. }
            | DialogueHostEvent::Signal { .. } => {}
        }
        Ok(())
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
        let range = push_display_text_run(
            &mut self.text,
            &mut self.display_map,
            value,
            source,
            node_index,
            &self.context.base_styles,
            &self.active_styles,
        );
        if range.start != range.end
            && let Some(annotation) =
                self.active_styles
                    .iter()
                    .rev()
                    .find_map(|style| match style {
                        RichTextStyle::Ruby { annotation } => Some(annotation),
                        _ => None,
                    })
        {
            let styles = self.current_styles();
            self.display_map
                .ruby_annotations
                .push(RichTextRubyAnnotation {
                    base_range: range,
                    ruby: annotation.clone(),
                    node_index,
                    presentation: presentation_from_styles(styles.iter()),
                    styles,
                });
        }
        range
    }

    fn current_styles(&self) -> Vec<RichTextStyle> {
        current_styles(&self.context.base_styles, &self.active_styles)
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

fn remove_active_style(active_styles: &mut Vec<RichTextStyle>, kind: RichTextSpanKind) {
    if let Some(index) = active_styles
        .iter()
        .rposition(|style| style.span_kind() == kind)
    {
        active_styles.remove(index);
    }
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
        | RichTextControl::Mark { .. } => None,
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
        RuntimeValue::Range(value) => value.label(),
        RuntimeValue::Iterator(_) => "<iterator>".to_owned(),
        RuntimeValue::EntityRef(value) => format!("@{value}"),
        RuntimeValue::Seq(_) => "[...]".to_owned(),
        RuntimeValue::Tuple(_) => "(...)".to_owned(),
        RuntimeValue::Record(_) => "{...}".to_owned(),
        RuntimeValue::NominalRecord(record) => {
            format!("<{}>", record.type_id().as_str())
        }
        RuntimeValue::Opaque(value) => format!("<opaque:{}>", value.producer().as_str()),
        RuntimeValue::Agent(value) => format!("<{}>", value.label()),
        RuntimeValue::Function(function) => format!("<function/{}>", function.arity()),
        RuntimeValue::Variant { name, .. } => format!(".{name}"),
    }
}
