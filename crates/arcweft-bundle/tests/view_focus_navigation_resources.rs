use arcweft_bundle::resource_codec::{
    ViewFocusDirection, ViewFocusGroupPolicy, ViewFocusGroupResource, ViewFocusInitialPolicy,
    ViewFocusNavigationEdge, ViewFocusNavigationResource, ViewFocusSkipPolicy,
    ViewFocusTargetResolution, ViewFocusWrapPolicy, ViewProgramResource,
};

fn program() -> ViewProgramResource {
    ViewProgramResource {
        program_id: arcweft_view::ViewProgramId::try_new("view.program.focus").unwrap(),
        source_refs: Vec::new(),
        definitions: Vec::new(),
        value_programs: Vec::new(),
        value_inputs: Vec::new(),
        instructions: Vec::new(),
        handlers: Vec::new(),
        exported_parts: Vec::new(),
        semantic_targets: Vec::new(),
        layout_bounds: Vec::new(),
        scroll_regions: Vec::new(),
        surfaces: Vec::new(),
        text_blocks: Vec::new(),
        action_buttons: Vec::new(),
        focus_groups: vec![ViewFocusGroupResource {
            public_id: "group.settings".to_owned(),
            view: Some("view.focus".to_owned()),
            parent: None,
            policy: ViewFocusGroupPolicy::Modal,
            initial: ViewFocusInitialPolicy::Explicit {
                target: "button.apply".to_owned(),
            },
            wrap: ViewFocusWrapPolicy::NoWrap,
            disabled_skip: ViewFocusSkipPolicy::Skip,
            hidden_skip: ViewFocusSkipPolicy::Skip,
            source: None,
        }],
        focus_navigation: vec![
            ViewFocusNavigationResource {
                public_id: "button.apply".to_owned(),
                view: Some("view.focus".to_owned()),
                group: Some("group.settings".to_owned()),
                edges: vec![ViewFocusNavigationEdge {
                    direction: ViewFocusDirection::Left,
                    target: ViewFocusTargetResolution::Explicit {
                        target: "button.back".to_owned(),
                    },
                    source: None,
                }],
                source: None,
            },
            ViewFocusNavigationResource {
                public_id: "button.back".to_owned(),
                view: Some("view.focus".to_owned()),
                group: Some("group.settings".to_owned()),
                edges: vec![ViewFocusNavigationEdge {
                    direction: ViewFocusDirection::Right,
                    target: ViewFocusTargetResolution::Explicit {
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
fn view_focus_navigation_compact_round_trip_is_deterministic() {
    let encoded = program().encode_canonical_section().unwrap();
    let decoded = ViewProgramResource::decode_canonical_section(&encoded).unwrap();
    assert_eq!(decoded, program());
    assert_eq!(encoded, decoded.encode_canonical_section().unwrap());
}

#[test]
fn view_focus_navigation_missing_explicit_target_is_rejected() {
    let mut invalid = program();
    invalid.focus_navigation[0].edges[0].target = ViewFocusTargetResolution::Explicit {
        target: "button.missing".to_owned(),
    };
    assert!(invalid.encode_canonical_section().is_err());
}
