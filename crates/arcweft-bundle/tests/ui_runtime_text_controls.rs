use arcweft_bundle::resource_codec::ui::{
    CompositionOnBlurPolicy, EnterKeyHint, TextAssistPolicy, TextCapitalization, UiInputKind,
    UiInputOptions, UiInputPurpose, UiInputResource, UiLayoutBoundsResource, UiLogicalRect,
    UiProgramResource, UiRuntimeTextControlBounds, UiRuntimeTextSelection, UiSecureInputPolicy,
    UiSemanticTarget, UiTextResource, UiTextSelectionPolicy, UiTextShortcutPolicy,
    UiTextSourceKind, UiTextSourceRecord, UiTextTabPolicy, UiTextVerticalNavigationPolicy,
};

#[test]
fn ui_input_resource_emits_runtime_text_control_shape() {
    let ui_input = UiInputResource {
        options: vec![text_input_option(
            "input.player_name",
            UiInputKind::TextField,
            "text.value.name",
            Some("text.placeholder.name"),
        )],
        adapter_requirements: Vec::new(),
    };
    let ui_text = UiTextResource {
        sources: vec![
            literal_source("text.value.name", "Ada"),
            literal_source("text.label.name", "Player name"),
            literal_source("text.placeholder.name", "Name"),
        ],
        display_frame_refs: Vec::new(),
        source_ranges: Vec::new(),
        reveal_policies: Vec::new(),
        cursor_policies: Vec::new(),
        redactions: Vec::new(),
    };
    let program = UiProgramResource {
        program_id: "ui.program.main".to_owned(),
        root_component: "ui.root".to_owned(),
        instructions: Vec::new(),
        child_spans: Vec::new(),
        handlers: Vec::new(),
        state_schema_hashes: Vec::new(),
        exported_parts: Vec::new(),
        semantic_targets: vec![UiSemanticTarget {
            public_id: "target.player_name".to_owned(),
            target: "input.player_name".to_owned(),
            component: None,
            label_text_source: Some("text.label.name".to_owned()),
            source: None,
        }],
        layout_bounds: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        adapter_requirements: Vec::new(),
    };

    let controls = ui_input.runtime_text_controls(Some(&ui_text), Some(&program));

    assert_eq!(controls.len(), 1);
    let control = &controls[0];
    assert_eq!(control.public_id, "input.player_name");
    assert_eq!(control.target, "input.player_name");
    assert_ne!(control.session, 0);
    assert_eq!(control.value, "Ada");
    assert_eq!(control.selection, UiRuntimeTextSelection::new(3, 3));
    assert_eq!(control.kind, UiInputKind::TextField);
    assert_eq!(control.label.as_deref(), Some("Player name"));
    assert_eq!(
        control.bounds,
        UiRuntimeTextControlBounds::from_px(48, 48, 420, 48)
    );
}

#[test]
fn runtime_text_control_session_is_stable_across_reorder() {
    let left = text_input_option(
        "input.left",
        UiInputKind::TextField,
        "text.value.left",
        None,
    );
    let right = text_input_option(
        "input.right",
        UiInputKind::TextArea,
        "text.value.right",
        None,
    );
    let text = UiTextResource {
        sources: vec![
            literal_source("text.value.left", "left"),
            literal_source("text.value.right", "right"),
        ],
        display_frame_refs: Vec::new(),
        source_ranges: Vec::new(),
        reveal_policies: Vec::new(),
        cursor_policies: Vec::new(),
        redactions: Vec::new(),
    };
    let first = UiInputResource {
        options: vec![left.clone(), right.clone()],
        adapter_requirements: Vec::new(),
    };
    let second = UiInputResource {
        options: vec![right, left],
        adapter_requirements: Vec::new(),
    };

    let left_session = first
        .runtime_text_controls(Some(&text), None)
        .into_iter()
        .find(|control| control.public_id == "input.left")
        .expect("left control exists")
        .session;
    let reordered_left_session = second
        .runtime_text_controls(Some(&text), None)
        .into_iter()
        .find(|control| control.public_id == "input.left")
        .expect("left control exists after reorder")
        .session;

    assert_eq!(left_session, reordered_left_session);
}

#[test]
fn ui_input_resource_stacks_default_text_control_bounds_by_height() {
    let ui_input = UiInputResource {
        options: vec![
            text_input_option("input.title", UiInputKind::TextField, "text.title", None),
            text_input_option("input.body", UiInputKind::TextArea, "text.body", None),
            text_input_option(
                "input.secret",
                UiInputKind::SecureField,
                "text.secret",
                None,
            ),
        ],
        adapter_requirements: Vec::new(),
    };
    let text = UiTextResource {
        sources: vec![
            literal_source("text.title", "line one"),
            literal_source("text.body", "Tokyo"),
            literal_source("text.secret", "secret"),
        ],
        display_frame_refs: Vec::new(),
        source_ranges: Vec::new(),
        reveal_policies: Vec::new(),
        cursor_policies: Vec::new(),
        redactions: Vec::new(),
    };

    let controls = ui_input.runtime_text_controls(Some(&text), None);

    assert_eq!(controls.len(), 3);
    assert_eq!(
        controls[0].bounds,
        UiRuntimeTextControlBounds::from_px(48, 48, 420, 48)
    );
    assert_eq!(
        controls[1].bounds,
        UiRuntimeTextControlBounds::from_px(48, 112, 420, 136)
    );
    assert_eq!(
        controls[2].bounds,
        UiRuntimeTextControlBounds::from_px(48, 264, 420, 48)
    );
}

#[test]
fn ui_program_layout_bounds_override_stacked_runtime_text_control_fallback() {
    let ui_input = UiInputResource {
        options: vec![
            text_input_option("input.title", UiInputKind::TextField, "text.title", None),
            text_input_option("input.body", UiInputKind::TextArea, "text.body", None),
        ],
        adapter_requirements: Vec::new(),
    };
    let text = UiTextResource {
        sources: vec![
            literal_source("text.title", "Title"),
            literal_source("text.body", "Body"),
        ],
        display_frame_refs: Vec::new(),
        source_ranges: Vec::new(),
        reveal_policies: Vec::new(),
        cursor_policies: Vec::new(),
        redactions: Vec::new(),
    };
    let program = UiProgramResource {
        program_id: "ui.program.feedback".to_owned(),
        root_component: "component.feedback".to_owned(),
        instructions: Vec::new(),
        child_spans: Vec::new(),
        handlers: Vec::new(),
        state_schema_hashes: Vec::new(),
        exported_parts: Vec::new(),
        semantic_targets: Vec::new(),
        layout_bounds: vec![
            UiLayoutBoundsResource::text_control(
                "input.title",
                UiLogicalRect::from_px(80, 64, 360, 48),
            ),
            UiLayoutBoundsResource::text_control(
                "input.body",
                UiLogicalRect::from_px(80, 128, 480, 136),
            ),
            UiLayoutBoundsResource::semantic_target(
                "input.title",
                UiLogicalRect::from_px(80, 64, 360, 48),
            ),
            UiLayoutBoundsResource::semantic_target(
                "input.body",
                UiLogicalRect::from_px(80, 128, 480, 136),
            ),
        ],
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        adapter_requirements: Vec::new(),
    };

    let controls = ui_input.runtime_text_controls(Some(&text), Some(&program));

    assert_eq!(
        controls[0].bounds,
        UiRuntimeTextControlBounds::from_px(80, 64, 360, 48)
    );
    assert_eq!(
        controls[1].bounds,
        UiRuntimeTextControlBounds::from_px(80, 128, 480, 136)
    );
    assert_eq!(
        program.semantic_target_bounds_for("input.body"),
        Some(UiRuntimeTextControlBounds::from_px(80, 128, 480, 136))
    );
}

fn text_input_option(
    public_id: &str,
    kind: UiInputKind,
    value_text_source: &str,
    placeholder_text_source: Option<&str>,
) -> UiInputOptions {
    UiInputOptions {
        public_id: public_id.to_owned(),
        component: None,
        kind,
        value_text_source: value_text_source.to_owned(),
        placeholder_text_source: placeholder_text_source.map(ToOwned::to_owned),
        purpose: UiInputPurpose::Text,
        autocorrect: TextAssistPolicy::PlatformDefault,
        spellcheck: TextAssistPolicy::PlatformDefault,
        capitalization: TextCapitalization::None,
        enter_key: EnterKeyHint::Default,
        multiline: kind.is_multiline(),
        selection_policy: UiTextSelectionPolicy::Enabled,
        shortcut_policy: UiTextShortcutPolicy::Enabled,
        tab_policy: UiTextTabPolicy::FocusNavigation,
        vertical_navigation_policy: UiTextVerticalNavigationPolicy::LogicalLine,
        secure_policy: UiSecureInputPolicy::Plain,
        composition_on_blur: CompositionOnBlurPolicy::Commit,
        submit_handler: None,
        change_handler: None,
        adapter_requirements: Vec::new(),
    }
}

fn literal_source(public_id: &str, value: &str) -> UiTextSourceRecord {
    UiTextSourceRecord {
        public_id: public_id.to_owned(),
        kind: UiTextSourceKind::Literal {
            value: value.to_owned(),
        },
        source: None,
    }
}
