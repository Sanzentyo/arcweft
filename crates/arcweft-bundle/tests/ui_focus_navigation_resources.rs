use arcweft_bundle::resource_codec::{
    UiFocusDirection, UiFocusGroupPolicy, UiFocusGroupResource, UiFocusInitialPolicy,
    UiFocusNavigationEdge, UiFocusNavigationResource, UiFocusSkipPolicy, UiFocusTargetResolution,
    UiFocusWrapPolicy, UiProgramResource,
};

fn program() -> UiProgramResource {
    UiProgramResource {
        program_id: "ui.program.focus".to_owned(),
        root_component: "component.focus".to_owned(),
        instructions: Vec::new(),
        child_spans: Vec::new(),
        handlers: Vec::new(),
        state_schema_hashes: Vec::new(),
        exported_parts: Vec::new(),
        semantic_targets: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: vec![UiFocusGroupResource {
            public_id: "group.settings".to_owned(),
            parent: None,
            policy: UiFocusGroupPolicy::Modal,
            initial: UiFocusInitialPolicy::Explicit {
                target: "button.apply".to_owned(),
            },
            wrap: UiFocusWrapPolicy::NoWrap,
            disabled_skip: UiFocusSkipPolicy::Skip,
            hidden_skip: UiFocusSkipPolicy::Skip,
            source: None,
        }],
        focus_navigation: vec![
            UiFocusNavigationResource {
                public_id: "button.apply".to_owned(),
                group: Some("group.settings".to_owned()),
                edges: vec![UiFocusNavigationEdge {
                    direction: UiFocusDirection::Left,
                    target: UiFocusTargetResolution::Explicit {
                        target: "button.back".to_owned(),
                    },
                    source: None,
                }],
                source: None,
            },
            UiFocusNavigationResource {
                public_id: "button.back".to_owned(),
                group: Some("group.settings".to_owned()),
                edges: vec![UiFocusNavigationEdge {
                    direction: UiFocusDirection::Right,
                    target: UiFocusTargetResolution::Explicit {
                        target: "button.apply".to_owned(),
                    },
                    source: None,
                }],
                source: None,
            },
        ],
        adapter_requirements: Vec::new(),
    }
}

#[test]
fn ui_focus_navigation_compact_round_trip_is_deterministic() {
    let encoded = program().encode_canonical_section().unwrap();
    let decoded = UiProgramResource::decode_canonical_section(&encoded).unwrap();
    assert_eq!(decoded, program());
    assert_eq!(encoded, decoded.encode_canonical_section().unwrap());
}

#[test]
fn ui_focus_navigation_missing_explicit_target_is_rejected() {
    let mut invalid = program();
    invalid.focus_navigation[0].edges[0].target = UiFocusTargetResolution::Explicit {
        target: "button.missing".to_owned(),
    };
    assert!(invalid.encode_canonical_section().is_err());
}
