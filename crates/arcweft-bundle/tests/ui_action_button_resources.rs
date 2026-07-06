use arcweft_bundle::resource_codec::ui::{
    UiTextResource, UiTextSourceKind, UiTextSourceRecord, ViewActionButtonActionResource,
    ViewActionButtonResource, ViewActionPayloadResource, ViewActionTextControlPayloadField,
    ViewProgramResource, ViewRuntimeActionButtonAction, ViewRuntimeButtonBounds,
};

#[test]
fn runtime_action_button_resolves_label_and_noop_action() {
    let program = ViewProgramResource {
        program_id: "ui.program.test".to_owned(),
        root_view: "ui.view.test".to_owned(),
        instructions: Vec::new(),
        child_spans: Vec::new(),
        handlers: Vec::new(),
        state_schema_hashes: Vec::new(),
        exported_parts: Vec::new(),
        semantic_targets: Vec::new(),
        layout_bounds: Vec::new(),
        scroll_regions: Vec::new(),
        text_blocks: Vec::new(),
        action_buttons: vec![ViewActionButtonResource {
            public_id: "button.submit_feedback".to_owned(),
            view: None,
            containing_scroll_region: None,
            label_text_source: "text.label.submit_feedback".to_owned(),
            enabled: true,
            action: ViewActionButtonActionResource::Noop,
            bounds: ViewRuntimeButtonBounds::new(484_000, 48_000, 128_000, 48_000),
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
    assert_eq!(buttons[0].action, ViewRuntimeActionButtonAction::Noop);
}

#[test]
fn runtime_action_button_resolves_action_invoke_action() {
    let program = ViewProgramResource {
        program_id: "ui.program.test".to_owned(),
        root_view: "ui.view.test".to_owned(),
        instructions: Vec::new(),
        child_spans: Vec::new(),
        handlers: Vec::new(),
        state_schema_hashes: Vec::new(),
        exported_parts: Vec::new(),
        semantic_targets: Vec::new(),
        layout_bounds: Vec::new(),
        scroll_regions: Vec::new(),
        text_blocks: Vec::new(),
        action_buttons: vec![ViewActionButtonResource {
            public_id: "button.continue".to_owned(),
            view: None,
            containing_scroll_region: None,
            label_text_source: "text.label.continue".to_owned(),
            enabled: true,
            action: ViewActionButtonActionResource::ActionInvoke {
                action: "action.feedback.submit_name".to_owned(),
                payload: Some(ViewActionPayloadResource::TextControlProjection {
                    input: "input.visitor_name".to_owned(),
                    field: ViewActionTextControlPayloadField::Text,
                }),
            },
            bounds: ViewRuntimeButtonBounds::new(484_000, 48_000, 128_000, 48_000),
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
        ViewRuntimeActionButtonAction::ActionInvoke { action, payload }
            if action == "action.feedback.submit_name"
                && payload == &Some(ViewActionPayloadResource::TextControlProjection {
                    input: "input.visitor_name".to_owned(),
                    field: ViewActionTextControlPayloadField::Text,
                })
    ));
}
