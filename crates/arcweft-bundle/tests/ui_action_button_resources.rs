use arcweft_bundle::resource_codec::ui::{
    UiActionButtonActionResource, UiActionButtonResource, UiActionPayloadResource,
    UiActionTextControlPayloadField, UiProgramResource, UiRuntimeActionButtonAction,
    UiRuntimeButtonBounds, UiTextResource, UiTextSourceKind, UiTextSourceRecord,
    UiTextSubmitImePolicy,
};

#[test]
fn runtime_action_button_resolves_label_and_typed_submit_action() {
    let program = UiProgramResource {
        program_id: "ui.program.test".to_owned(),
        root_component: "ui.component.test".to_owned(),
        instructions: Vec::new(),
        child_spans: Vec::new(),
        handlers: Vec::new(),
        state_schema_hashes: Vec::new(),
        exported_parts: Vec::new(),
        semantic_targets: Vec::new(),
        layout_bounds: Vec::new(),
        action_buttons: vec![UiActionButtonResource {
            public_id: "button.submit_feedback".to_owned(),
            label_text_source: "text.label.submit_feedback".to_owned(),
            enabled: true,
            action: UiActionButtonActionResource::TextInputSubmit {
                input: "input.feedback".to_owned(),
                ime_policy: UiTextSubmitImePolicy::Commit,
            },
            bounds: UiRuntimeButtonBounds::new(484_000, 48_000, 128_000, 48_000),
            style: None,
            source: None,
        }],
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        adapter_requirements: Vec::new(),
    };
    let text = UiTextResource {
        sources: vec![UiTextSourceRecord {
            public_id: "text.label.submit_feedback".to_owned(),
            kind: UiTextSourceKind::Literal {
                value: "Send".to_owned(),
            },
            source: None,
        }],
        ..UiTextResource::default()
    };

    let buttons = program.runtime_action_buttons(Some(&text));

    assert_eq!(buttons.len(), 1);
    assert_eq!(buttons[0].target, "button.submit_feedback");
    assert_eq!(buttons[0].label, "Send");
    assert!(matches!(
        buttons[0].action,
        UiRuntimeActionButtonAction::TextInputSubmit { ref input_target, ime_policy }
            if input_target == "input.feedback" && ime_policy == UiTextSubmitImePolicy::Commit
    ));
}

#[test]
fn runtime_action_button_resolves_action_invoke_action() {
    let program = UiProgramResource {
        program_id: "ui.program.test".to_owned(),
        root_component: "ui.component.test".to_owned(),
        instructions: Vec::new(),
        child_spans: Vec::new(),
        handlers: Vec::new(),
        state_schema_hashes: Vec::new(),
        exported_parts: Vec::new(),
        semantic_targets: Vec::new(),
        layout_bounds: Vec::new(),
        action_buttons: vec![UiActionButtonResource {
            public_id: "button.continue".to_owned(),
            label_text_source: "text.label.continue".to_owned(),
            enabled: true,
            action: UiActionButtonActionResource::ActionInvoke {
                action: "action.feedback.submit_name".to_owned(),
                payload: Some(UiActionPayloadResource::TextControlProjection {
                    input: "input.visitor_name".to_owned(),
                    field: UiActionTextControlPayloadField::Text,
                }),
            },
            bounds: UiRuntimeButtonBounds::new(484_000, 48_000, 128_000, 48_000),
            style: None,
            source: None,
        }],
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        adapter_requirements: Vec::new(),
    };
    let text = UiTextResource {
        sources: vec![UiTextSourceRecord {
            public_id: "text.label.continue".to_owned(),
            kind: UiTextSourceKind::Literal {
                value: "Continue".to_owned(),
            },
            source: None,
        }],
        ..UiTextResource::default()
    };

    let buttons = program.runtime_action_buttons(Some(&text));

    assert_eq!(buttons.len(), 1);
    assert!(matches!(
        &buttons[0].action,
        UiRuntimeActionButtonAction::ActionInvoke { action, payload }
            if action == "action.feedback.submit_name"
                && payload == &Some(UiActionPayloadResource::TextControlProjection {
                    input: "input.visitor_name".to_owned(),
                    field: UiActionTextControlPayloadField::Text,
                })
    ));
}
