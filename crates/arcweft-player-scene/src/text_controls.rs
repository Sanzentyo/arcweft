use crate::control_style::lower_control_style;
use crate::input::InputController;
use arcweft_bundle::resource_codec::view::{
    EnterKeyHint as ViewEnterKeyHint, TextAssistPolicy as ViewTextAssistPolicy,
    TextCapitalization as ViewTextCapitalization, ViewInputKind, ViewInputPurpose,
    ViewRuntimeTextControl, ViewRuntimeTextControlBounds, ViewRuntimeTextSelection,
    ViewTextSelectionPolicy, ViewTextShortcutPolicy, ViewTextTabPolicy,
    ViewTextVerticalNavigationPolicy,
};
use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::semantic::SemanticRole;
use arcweft_presentation::text_input::{
    Capitalization, EnterKeyHint, TextAssistPolicy, TextByteOffset, TextInputOptions,
    TextInputPurpose, TextInputSessionId, TextRange, TextSelectionPolicy, TextShortcutPolicy,
    TextTabPolicy, TextVerticalNavigationPolicy,
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
        controls: &[ViewRuntimeTextControl],
    ) -> Result<Vec<RenderTextInputControl>, RuntimeTextControlLoweringError> {
        let controls = Self::lower_controls(controls)?;
        input.retain_live_text_control_focus(&controls);
        Self::activate_focused(input, &controls)?;
        Ok(controls
            .into_iter()
            .map(|control| input.apply_live_text_control_state(control))
            .collect())
    }

    pub fn lower_controls(
        controls: &[ViewRuntimeTextControl],
    ) -> Result<Vec<RenderTextInputControl>, RuntimeTextControlLoweringError> {
        controls.iter().map(Self::lower_control).collect()
    }

    pub fn lower_control(
        control: &ViewRuntimeTextControl,
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
        if let Some(scroll_region) = &control.containing_scroll_region {
            render = render.with_containing_scroll_region(scroll_region.clone());
        }
        if let Some(label) = &control.label {
            render = render.with_label(label.clone());
        }
        render = render.with_style(lower_control_style(&control.style));
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

fn lower_selection(selection: ViewRuntimeTextSelection) -> TextRange<TextByteOffset> {
    TextRange::new(
        TextByteOffset(selection.start),
        TextByteOffset(selection.end),
    )
}

fn lower_bounds(bounds: ViewRuntimeTextControlBounds) -> HitRect {
    HitRect::new(
        milli_i32_to_f32(bounds.x_milli),
        milli_i32_to_f32(bounds.y_milli),
        milli_u32_to_f32(bounds.width_milli),
        milli_u32_to_f32(bounds.height_milli),
    )
}

fn lower_role(kind: ViewInputKind) -> SemanticRole {
    match kind {
        ViewInputKind::TextField => SemanticRole::TextField,
        ViewInputKind::TextArea => SemanticRole::TextArea,
        ViewInputKind::SecureField => SemanticRole::SecureTextField,
    }
}

fn lower_options(control: &ViewRuntimeTextControl) -> TextInputOptions {
    TextInputOptions::default()
        .with_purpose(lower_purpose(control.options.purpose))
        .with_autocorrect(lower_assist(control.options.autocorrect))
        .with_spellcheck(lower_assist(control.options.spellcheck))
        .with_capitalization(lower_capitalization(control.options.capitalization))
        .with_enter_key(lower_enter_key(control.options.enter_key))
        .multiline(control.options.multiline || control.kind.is_multiline())
        .with_selection_policy(lower_selection_policy(control.options.selection_policy))
        .with_shortcut_policy(lower_shortcut_policy(control.options.shortcut_policy))
        .with_tab_policy(lower_tab_policy(control.options.tab_policy))
        .with_vertical_navigation_policy(lower_vertical_navigation_policy(
            control.options.vertical_navigation_policy,
        ))
        .secure(control.options.secure_policy.is_secure() || control.kind.is_secure())
}

fn lower_purpose(purpose: ViewInputPurpose) -> TextInputPurpose {
    match purpose {
        ViewInputPurpose::Text => TextInputPurpose::Text,
        ViewInputPurpose::Search => TextInputPurpose::Search,
        ViewInputPurpose::Name => TextInputPurpose::Name,
        ViewInputPurpose::Email => TextInputPurpose::Email,
        ViewInputPurpose::Url => TextInputPurpose::Url,
        ViewInputPurpose::Telephone => TextInputPurpose::Telephone,
        ViewInputPurpose::Number => TextInputPurpose::Number,
        ViewInputPurpose::Decimal => TextInputPurpose::Decimal,
        ViewInputPurpose::Password => TextInputPurpose::Password,
        ViewInputPurpose::Pin => TextInputPurpose::Pin,
        ViewInputPurpose::Terminal => TextInputPurpose::Terminal,
    }
}

fn lower_assist(policy: ViewTextAssistPolicy) -> TextAssistPolicy {
    match policy {
        ViewTextAssistPolicy::PlatformDefault => TextAssistPolicy::PlatformDefault,
        ViewTextAssistPolicy::Enabled => TextAssistPolicy::Enabled,
        ViewTextAssistPolicy::Disabled => TextAssistPolicy::Disabled,
    }
}

fn lower_capitalization(capitalization: ViewTextCapitalization) -> Capitalization {
    match capitalization {
        ViewTextCapitalization::None => Capitalization::None,
        ViewTextCapitalization::Sentences => Capitalization::Sentences,
        ViewTextCapitalization::Words => Capitalization::Words,
        ViewTextCapitalization::Characters => Capitalization::Characters,
    }
}

fn lower_enter_key(enter_key: ViewEnterKeyHint) -> EnterKeyHint {
    match enter_key {
        ViewEnterKeyHint::Default => EnterKeyHint::Default,
        ViewEnterKeyHint::Enter => EnterKeyHint::Enter,
        ViewEnterKeyHint::Done => EnterKeyHint::Done,
        ViewEnterKeyHint::Go => EnterKeyHint::Go,
        ViewEnterKeyHint::Next => EnterKeyHint::Next,
        ViewEnterKeyHint::Search => EnterKeyHint::Search,
        ViewEnterKeyHint::Send => EnterKeyHint::Send,
    }
}

fn lower_selection_policy(policy: ViewTextSelectionPolicy) -> TextSelectionPolicy {
    match policy {
        ViewTextSelectionPolicy::Enabled => TextSelectionPolicy::Enabled,
        ViewTextSelectionPolicy::Disabled => TextSelectionPolicy::Disabled,
    }
}

fn lower_shortcut_policy(policy: ViewTextShortcutPolicy) -> TextShortcutPolicy {
    match policy {
        ViewTextShortcutPolicy::Enabled => TextShortcutPolicy::Enabled,
        ViewTextShortcutPolicy::Disabled => TextShortcutPolicy::Disabled,
    }
}

fn lower_tab_policy(policy: ViewTextTabPolicy) -> TextTabPolicy {
    match policy {
        ViewTextTabPolicy::FocusNavigation => TextTabPolicy::FocusNavigation,
        ViewTextTabPolicy::InsertTab => TextTabPolicy::InsertTab,
    }
}

fn lower_vertical_navigation_policy(
    policy: ViewTextVerticalNavigationPolicy,
) -> TextVerticalNavigationPolicy {
    match policy {
        ViewTextVerticalNavigationPolicy::LogicalLine => TextVerticalNavigationPolicy::LogicalLine,
        ViewTextVerticalNavigationPolicy::VisualLine => TextVerticalNavigationPolicy::VisualLine,
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
