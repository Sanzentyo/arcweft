//! Runtime text-control write-back projection.

use super::{
    BundleSessionError, RuntimeTextControlWriteBack, TextControlWriteBack, ViewRuntimeTextControl,
    ViewRuntimeTextSelection,
};

pub(super) fn apply_text_control_write_back_to_controls(
    text_inputs: &mut [ViewRuntimeTextControl],
    write_back: &TextControlWriteBack,
) -> Result<RuntimeTextControlWriteBack, BundleSessionError> {
    let target = write_back.target().id().as_str().to_owned();
    let session = write_back.session().0;
    let Some(control) = text_inputs
        .iter_mut()
        .find(|control| control.target == target && control.session == session)
    else {
        return Err(BundleSessionError::UnknownTextControlWriteBackTarget { target, session });
    };
    write_back.value().as_str().clone_into(&mut control.value);
    control.selection = ViewRuntimeTextSelection::new(
        write_back.selection().start().get(),
        write_back.selection().end().get(),
    );
    Ok(RuntimeTextControlWriteBack::from_control(
        write_back, control,
    ))
}

#[cfg(test)]
mod tests {
    use super::apply_text_control_write_back_to_controls;
    use crate::session::{TextControlWriteBack, ViewRuntimeTextControl, ViewRuntimeTextSelection};
    use arcweft_bundle::resource_codec::view::{
        CompositionOnBlurPolicy, EnterKeyHint, TextAssistPolicy, TextCapitalization, ViewInputKind,
        ViewInputPurpose, ViewRuntimeControlVisualStyle, ViewRuntimeTextControlBounds,
        ViewRuntimeTextControlHandlers, ViewRuntimeTextControlOptions, ViewSecureInputPolicy,
        ViewTextSelectionPolicy, ViewTextShortcutPolicy, ViewTextTabPolicy,
        ViewTextVerticalNavigationPolicy,
    };
    use arcweft_id::PublicId;
    use arcweft_presentation::input::InteractionTarget as PresentationTarget;
    use arcweft_presentation::text_input::{
        TextByteOffset, TextControlValue, TextInputSessionId, TextRange, TextRevision,
    };

    fn runtime_control(target: &str, session: u64, value: &str) -> ViewRuntimeTextControl {
        ViewRuntimeTextControl {
            public_id: target.to_owned(),
            target: target.to_owned(),
            view: None,
            containing_scroll_region: None,
            session,
            value: value.to_owned(),
            selection: ViewRuntimeTextSelection::collapsed_at_end(value),
            options: ViewRuntimeTextControlOptions {
                purpose: ViewInputPurpose::Text,
                autocorrect: TextAssistPolicy::PlatformDefault,
                spellcheck: TextAssistPolicy::PlatformDefault,
                capitalization: TextCapitalization::None,
                enter_key: EnterKeyHint::Default,
                multiline: false,
                selection_policy: ViewTextSelectionPolicy::Enabled,
                shortcut_policy: ViewTextShortcutPolicy::Enabled,
                tab_policy: ViewTextTabPolicy::FocusNavigation,
                vertical_navigation_policy: ViewTextVerticalNavigationPolicy::LogicalLine,
                secure_policy: ViewSecureInputPolicy::Plain,
                composition_on_blur: CompositionOnBlurPolicy::Commit,
            },
            kind: ViewInputKind::TextField,
            bounds: ViewRuntimeTextControlBounds::from_px(0, 0, 100, 24),
            label: None,
            handlers: ViewRuntimeTextControlHandlers::default(),
            style: ViewRuntimeControlVisualStyle::default(),
        }
    }

    #[test]
    fn write_back_updates_runtime_overlay_and_returns_typed_event() {
        let mut controls = vec![runtime_control("field.name", 7, "old")];
        let write_back = TextControlWriteBack::change(
            PresentationTarget::new(PublicId::try_new("field.name").unwrap()),
            TextInputSessionId(7),
            TextControlValue::plain("new"),
            TextRange::new(TextByteOffset(3), TextByteOffset(3)),
            TextRevision(1),
        );

        let event = apply_text_control_write_back_to_controls(&mut controls, &write_back).unwrap();

        assert_eq!(controls[0].value, "new");
        assert_eq!(event.value().as_str(), "new");
        assert!(event.is_change());
    }
}
