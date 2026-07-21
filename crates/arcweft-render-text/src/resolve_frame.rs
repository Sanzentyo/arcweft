//! Runtime binding resolution into a dialogue display frame.

use crate::{
    DialogueHostEvent, LineDisplayFrame, LineDisplaySpec, RichTextControl, RichTextControlMarker,
    RichTextDisplayMap, RichTextHostEventMarker, RichTextNode, RichTextRange,
    RichTextRubyAnnotation, RichTextStyle, RichTextTextRun, RichTextTextSource,
    presentation_from_styles,
};
use arcweft_core::plan::RuntimeLineId;
use arcweft_core::value::{RuntimeBinding, RuntimeValue};
use arcweft_dialogue::{InlineFailurePolicy, InlineFallback, InlineTextFailure};
use arcweft_presentation::rich_text::canonical_tag_name;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Dialogue line context captured by the runtime at display time.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct RuntimeLineContext {
    pub bindings: Vec<RuntimeBinding>,
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
    /// Creates context from visible runtime bindings.
    #[must_use]
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
        if let RichTextNode::HostEvent {
            event: event @ DialogueHostEvent::Conditional { .. },
        } = node
        {
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
            RichTextNode::Control { control } => {
                self.push_control_node(control, node_index, node);
                Ok(())
            }
            RichTextNode::Interpolation {
                expr,
                fallback_source,
                on_error,
            } => self.push_interpolation_node(expr, fallback_source, on_error, node_index),
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
        let text_offset = self.text.len();
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
            text_offset: self.text.len(),
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
            speaker_label: self.spec.speaker_label.clone(),
            text: self.text,
            base_styles: self.spec.base_styles.clone(),
            profile_style: self.spec.profile_style.clone(),
            dialogue_revision: self.spec.dialogue_revision.clone(),
            inline_failure: self.spec.inline_failure.clone(),
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
    let name = canonical_tag_name(name);
    if let Some(index) = active_styles
        .iter()
        .rposition(|style| style.tag_name() == name)
    {
        active_styles.remove(index);
    }
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
        RuntimeValue::Range(value) => value.label(),
        RuntimeValue::Iterator(_) => "<iterator>".to_owned(),
        RuntimeValue::EntityRef(value) => format!("@{value}"),
        RuntimeValue::Seq(_) => "[...]".to_owned(),
        RuntimeValue::Tuple(_) => "(...)".to_owned(),
        RuntimeValue::Record(_) => "{...}".to_owned(),
        RuntimeValue::NominalRecord(record) => {
            format!("<{}>", record.type_id().as_str())
        }
        RuntimeValue::Function(function) => format!("<function/{}>", function.arity()),
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
        | RuntimeValue::Range(_)
        | RuntimeValue::Iterator(_)
        | RuntimeValue::NominalRecord(_)
        | RuntimeValue::Function(_)
        | RuntimeValue::Variant { .. } => true,
        RuntimeValue::String(value) | RuntimeValue::EntityRef(value) => !value.is_empty(),
        RuntimeValue::Char(value) => *value != '\0',
        RuntimeValue::Duration(value) => value.as_nanos() != 0,
        RuntimeValue::Seq(value) => !value.is_empty(),
        RuntimeValue::Tuple(value) => !value.is_empty(),
        RuntimeValue::Record(value) => !value.is_empty(),
    }
}
