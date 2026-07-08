use arcweft_bundle::resource_codec::view::{
    CompositionOnBlurPolicy, EnterKeyHint, TextAssistPolicy, TextCapitalization, ViewInputKind,
    ViewInputOptions, ViewInputPurpose, ViewInputResource, ViewLayoutBoundsResource,
    ViewLogicalRect, ViewProgramResource, ViewRuntimeTextControlBounds, ViewRuntimeTextSelection,
    ViewSecureInputPolicy, ViewSemanticTarget, ViewTextResource, ViewTextSelectionPolicy,
    ViewTextShortcutPolicy, ViewTextSourceKind, ViewTextSourceRecord, ViewTextTabPolicy,
    ViewTextVerticalNavigationPolicy,
};

#[test]
fn view_input_resource_emits_runtime_text_control_shape() {
    let view_input = ViewInputResource {
        options: vec![text_input_option(
            "input.player_name",
            ViewInputKind::TextField,
            "text.value.name",
            Some("text.placeholder.name"),
        )],
        adapter_requirements: Vec::new(),
    };
    let view_text = ViewTextResource {
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
    let program = ViewProgramResource {
        program_id: "view.program.main".to_owned(),
        root_view: "view.root".to_owned(),
        instructions: Vec::new(),
        child_spans: Vec::new(),
        handlers: Vec::new(),
        state_schema_hashes: Vec::new(),
        exported_parts: Vec::new(),
        semantic_targets: vec![ViewSemanticTarget {
            public_id: "target.player_name".to_owned(),
            target: "input.player_name".to_owned(),
            view: None,
            label_text_source: Some("text.label.name".to_owned()),
            source: None,
        }],
        layout_bounds: Vec::new(),
        scroll_regions: Vec::new(),
        surfaces: Vec::new(),
        text_blocks: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        adapter_requirements: Vec::new(),
    };

    let controls = view_input.runtime_text_controls(Some(&view_text), Some(&program));

    assert_eq!(controls.len(), 1);
    let control = &controls[0];
    assert_eq!(control.public_id, "input.player_name");
    assert_eq!(control.target, "input.player_name");
    assert_ne!(control.session, 0);
    assert_eq!(control.value, "Ada");
    assert_eq!(control.selection, ViewRuntimeTextSelection::new(3, 3));
    assert_eq!(control.kind, ViewInputKind::TextField);
    assert_eq!(control.label.as_deref(), Some("Player name"));
    assert_eq!(
        control.bounds,
        ViewRuntimeTextControlBounds::from_px(48, 48, 420, 48)
    );
}

#[test]
fn runtime_text_control_session_is_stable_across_reorder() {
    let left = text_input_option(
        "input.left",
        ViewInputKind::TextField,
        "text.value.left",
        None,
    );
    let right = text_input_option(
        "input.right",
        ViewInputKind::TextArea,
        "text.value.right",
        None,
    );
    let text = ViewTextResource {
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
    let first = ViewInputResource {
        options: vec![left.clone(), right.clone()],
        adapter_requirements: Vec::new(),
    };
    let second = ViewInputResource {
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
fn view_input_resource_stacks_default_text_control_bounds_by_height() {
    let view_input = ViewInputResource {
        options: vec![
            text_input_option("input.title", ViewInputKind::TextField, "text.title", None),
            text_input_option("input.body", ViewInputKind::TextArea, "text.body", None),
            text_input_option(
                "input.secret",
                ViewInputKind::SecureField,
                "text.secret",
                None,
            ),
        ],
        adapter_requirements: Vec::new(),
    };
    let text = ViewTextResource {
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

    let controls = view_input.runtime_text_controls(Some(&text), None);

    assert_eq!(controls.len(), 3);
    assert_eq!(
        controls[0].bounds,
        ViewRuntimeTextControlBounds::from_px(48, 48, 420, 48)
    );
    assert_eq!(
        controls[1].bounds,
        ViewRuntimeTextControlBounds::from_px(48, 112, 420, 136)
    );
    assert_eq!(
        controls[2].bounds,
        ViewRuntimeTextControlBounds::from_px(48, 264, 420, 48)
    );
}

#[test]
fn view_program_layout_bounds_override_stacked_runtime_text_control_fallback() {
    let view_input = ViewInputResource {
        options: vec![
            text_input_option("input.title", ViewInputKind::TextField, "text.title", None),
            text_input_option("input.body", ViewInputKind::TextArea, "text.body", None),
        ],
        adapter_requirements: Vec::new(),
    };
    let text = ViewTextResource {
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
    let program = ViewProgramResource {
        program_id: "view.program.feedback".to_owned(),
        root_view: "view.feedback".to_owned(),
        instructions: Vec::new(),
        child_spans: Vec::new(),
        handlers: Vec::new(),
        state_schema_hashes: Vec::new(),
        exported_parts: Vec::new(),
        semantic_targets: Vec::new(),
        layout_bounds: vec![
            ViewLayoutBoundsResource::text_control(
                "input.title",
                ViewLogicalRect::from_px(80, 64, 360, 48),
            ),
            ViewLayoutBoundsResource::text_control(
                "input.body",
                ViewLogicalRect::from_px(80, 128, 480, 136),
            ),
            ViewLayoutBoundsResource::semantic_target(
                "input.title",
                ViewLogicalRect::from_px(80, 64, 360, 48),
            ),
            ViewLayoutBoundsResource::semantic_target(
                "input.body",
                ViewLogicalRect::from_px(80, 128, 480, 136),
            ),
        ],
        scroll_regions: Vec::new(),
        surfaces: Vec::new(),
        text_blocks: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: Vec::new(),
        focus_navigation: Vec::new(),
        adapter_requirements: Vec::new(),
    };

    let controls = view_input.runtime_text_controls(Some(&text), Some(&program));

    assert_eq!(
        controls[0].bounds,
        ViewRuntimeTextControlBounds::from_px(80, 64, 360, 48)
    );
    assert_eq!(
        controls[1].bounds,
        ViewRuntimeTextControlBounds::from_px(80, 128, 480, 136)
    );
    assert_eq!(
        program.semantic_target_bounds_for("input.body"),
        Some(ViewRuntimeTextControlBounds::from_px(80, 128, 480, 136))
    );
}

fn text_input_option(
    public_id: &str,
    kind: ViewInputKind,
    value_text_source: &str,
    placeholder_text_source: Option<&str>,
) -> ViewInputOptions {
    ViewInputOptions {
        public_id: public_id.to_owned(),
        view: None,
        containing_scroll_region: None,
        kind,
        value_text_source: value_text_source.to_owned(),
        placeholder_text_source: placeholder_text_source.map(ToOwned::to_owned),
        purpose: ViewInputPurpose::Text,
        autocorrect: TextAssistPolicy::PlatformDefault,
        spellcheck: TextAssistPolicy::PlatformDefault,
        capitalization: TextCapitalization::None,
        enter_key: EnterKeyHint::Default,
        multiline: kind.is_multiline(),
        selection_policy: ViewTextSelectionPolicy::Enabled,
        shortcut_policy: ViewTextShortcutPolicy::Enabled,
        tab_policy: ViewTextTabPolicy::FocusNavigation,
        vertical_navigation_policy: ViewTextVerticalNavigationPolicy::LogicalLine,
        secure_policy: ViewSecureInputPolicy::Plain,
        composition_on_blur: CompositionOnBlurPolicy::Commit,
        submit_handler: None,
        change_handler: None,
        adapter_requirements: Vec::new(),
    }
}

fn literal_source(public_id: &str, value: &str) -> ViewTextSourceRecord {
    ViewTextSourceRecord {
        public_id: public_id.to_owned(),
        kind: ViewTextSourceKind::Literal {
            value: value.to_owned(),
        },
        source: None,
    }
}
