use crate::input::InputController;
use arcweft_bundle::resource_codec::ui::{
    EnterKeyHint as UiEnterKeyHint, TextAssistPolicy as UiTextAssistPolicy,
    TextCapitalization as UiTextCapitalization, UiInputKind, UiInputPurpose, UiRuntimeTextControl,
    UiRuntimeTextControlBounds, UiRuntimeTextSelection,
};
use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::semantic::SemanticRole;
use arcweft_presentation::text_input::{
    Capitalization, EnterKeyHint, TextAssistPolicy, TextByteOffset, TextInputOptions,
    TextInputPurpose, TextInputSessionId, TextRange,
};
use arcweft_render_wgpu::geometry::{FramePlanError, RenderTextInputControl};
use num_traits::ToPrimitive;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeTextControlLowerer;

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RuntimeTextControlLoweringError {
    #[error("invalid runtime text-control target `{target}`")]
    InvalidTarget { target: String },
    #[error("failed to activate focused runtime text-control: {source}")]
    FocusActivation { source: FramePlanError },
}

impl RuntimeTextControlLowerer {
    /// Lowers product/runtime text controls through the shared player-owned
    /// editor path before renderer frame preparation.
    pub fn lower_for_frame(
        input: &mut InputController,
        controls: &[UiRuntimeTextControl],
    ) -> Result<Vec<RenderTextInputControl>, RuntimeTextControlLoweringError> {
        let controls = Self::lower_controls(controls)?;
        Self::activate_focused(input, &controls)?;
        Ok(controls
            .into_iter()
            .map(|control| input.apply_live_text_control_state(control))
            .collect())
    }

    pub fn lower_controls(
        controls: &[UiRuntimeTextControl],
    ) -> Result<Vec<RenderTextInputControl>, RuntimeTextControlLoweringError> {
        controls.iter().map(Self::lower_control).collect()
    }

    pub fn lower_control(
        control: &UiRuntimeTextControl,
    ) -> Result<RenderTextInputControl, RuntimeTextControlLoweringError> {
        let target = lower_target(&control.target)?;
        let selection = lower_selection(control.selection.clamped_to_text(&control.value));
        let options = lower_options(control);
        let role = lower_role(control.kind);
        let mut render = RenderTextInputControl::new(
            target,
            TextInputSessionId(control.session),
            control.value.clone(),
            selection,
            options,
            role,
            lower_bounds(control.bounds),
        );
        if let Some(label) = &control.label {
            render = render.with_label(label.clone());
        }
        Ok(render)
    }

    fn activate_focused(
        input: &mut InputController,
        controls: &[RenderTextInputControl],
    ) -> Result<(), RuntimeTextControlLoweringError> {
        let focused = input.visual_state().focused;
        let Some(focused) = focused.as_ref() else {
            return Ok(());
        };
        let Some(control) = controls.iter().find(|control| &control.target == focused) else {
            return Ok(());
        };
        input
            .activate_text_control(control)
            .map(|_| ())
            .map_err(|source| RuntimeTextControlLoweringError::FocusActivation { source })
    }
}

fn lower_target(target: &str) -> Result<InteractionTarget, RuntimeTextControlLoweringError> {
    PublicId::try_new(target)
        .map(InteractionTarget::new)
        .map_err(|_| RuntimeTextControlLoweringError::InvalidTarget {
            target: target.to_owned(),
        })
}

fn lower_selection(selection: UiRuntimeTextSelection) -> TextRange<TextByteOffset> {
    TextRange::new(
        TextByteOffset(selection.start),
        TextByteOffset(selection.end),
    )
}

fn lower_bounds(bounds: UiRuntimeTextControlBounds) -> HitRect {
    HitRect::new(
        milli_i32_to_f32(bounds.x_milli),
        milli_i32_to_f32(bounds.y_milli),
        milli_u32_to_f32(bounds.width_milli),
        milli_u32_to_f32(bounds.height_milli),
    )
}

fn lower_role(kind: UiInputKind) -> SemanticRole {
    match kind {
        UiInputKind::TextField => SemanticRole::TextField,
        UiInputKind::TextArea => SemanticRole::TextArea,
        UiInputKind::SecureField => SemanticRole::SecureTextField,
    }
}

fn lower_options(control: &UiRuntimeTextControl) -> TextInputOptions {
    TextInputOptions::default()
        .with_purpose(lower_purpose(control.options.purpose))
        .with_autocorrect(lower_assist(control.options.autocorrect))
        .with_spellcheck(lower_assist(control.options.spellcheck))
        .with_capitalization(lower_capitalization(control.options.capitalization))
        .with_enter_key(lower_enter_key(control.options.enter_key))
        .multiline(control.options.multiline || control.kind.is_multiline())
        .secure(control.options.secure_policy.is_secure() || control.kind.is_secure())
}

fn lower_purpose(purpose: UiInputPurpose) -> TextInputPurpose {
    match purpose {
        UiInputPurpose::Text => TextInputPurpose::Text,
        UiInputPurpose::Search => TextInputPurpose::Search,
        UiInputPurpose::Name => TextInputPurpose::Name,
        UiInputPurpose::Email => TextInputPurpose::Email,
        UiInputPurpose::Url => TextInputPurpose::Url,
        UiInputPurpose::Telephone => TextInputPurpose::Telephone,
        UiInputPurpose::Number => TextInputPurpose::Number,
        UiInputPurpose::Decimal => TextInputPurpose::Decimal,
        UiInputPurpose::Password => TextInputPurpose::Password,
        UiInputPurpose::Pin => TextInputPurpose::Pin,
        UiInputPurpose::Terminal => TextInputPurpose::Terminal,
    }
}

fn lower_assist(policy: UiTextAssistPolicy) -> TextAssistPolicy {
    match policy {
        UiTextAssistPolicy::PlatformDefault => TextAssistPolicy::PlatformDefault,
        UiTextAssistPolicy::Enabled => TextAssistPolicy::Enabled,
        UiTextAssistPolicy::Disabled => TextAssistPolicy::Disabled,
    }
}

fn lower_capitalization(capitalization: UiTextCapitalization) -> Capitalization {
    match capitalization {
        UiTextCapitalization::None => Capitalization::None,
        UiTextCapitalization::Sentences => Capitalization::Sentences,
        UiTextCapitalization::Words => Capitalization::Words,
        UiTextCapitalization::Characters => Capitalization::Characters,
    }
}

fn lower_enter_key(enter_key: UiEnterKeyHint) -> EnterKeyHint {
    match enter_key {
        UiEnterKeyHint::Default => EnterKeyHint::Default,
        UiEnterKeyHint::Enter => EnterKeyHint::Enter,
        UiEnterKeyHint::Done => EnterKeyHint::Done,
        UiEnterKeyHint::Go => EnterKeyHint::Go,
        UiEnterKeyHint::Next => EnterKeyHint::Next,
        UiEnterKeyHint::Search => EnterKeyHint::Search,
        UiEnterKeyHint::Send => EnterKeyHint::Send,
    }
}

fn milli_i32_to_f32(value: i32) -> f32 {
    value.to_f32().unwrap_or_else(|| {
        if value.is_negative() {
            f32::MIN
        } else {
            f32::MAX
        }
    }) / 1_000.0
}

fn milli_u32_to_f32(value: u32) -> f32 {
    value.to_f32().unwrap_or(f32::MAX) / 1_000.0
}
