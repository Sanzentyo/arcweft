use super::super::PlayerFrameError;
use super::{
    NodeBinding, ResolvedLayoutNode, RuntimeNodeId, StyleConsumer, StyleTargetKey, StyleTargetKind,
    box_style, encode_path, interaction_states, node_bindings, node_facts, resolve_layout_offsets,
    validate_consumer_properties, validate_supported_properties,
};
use crate::input::InputController;
use arcweft_bundle::resource_codec::view::{ViewObserveClassification, ViewTextSelectionPolicy};
use arcweft_bundle::resource_codec::{
    ViewRuntimeControlVisualStyle, ViewRuntimeNodeStyle, ViewRuntimeSurface,
    ViewRuntimeSurfaceBounds, ViewTextBlockBounds,
};
use arcweft_id::PublicId;
use arcweft_presentation::appearance::{
    PresentationColor, PresentationEnvironment, SystemPaletteSet,
};
use arcweft_presentation::hover::HoverPath;
use arcweft_presentation::input::{InteractionTarget, PointerId};
use arcweft_presentation::interaction::{FocusState, InteractionState};
use arcweft_presentation::layer::LayerId;
use arcweft_render_text::{
    RichTextDocument, RichTextInlineDirection, RichTextLayout, RichTextNode, RichTextStyle,
};
use arcweft_runtime_driver::display::BundlePresentationSnapshot;
use arcweft_runtime_driver::presentation_handles::PresentationHandleId;
use arcweft_runtime_driver::view_runtime::{
    BundleViewInstancePath, BundleViewInstancePathSegment, BundleViewMountOutput,
    BundleViewStyleNode, BundleViewStyleNodeId, BundleViewStyleNodeKind, BundleViewTextOutput,
    BundleViewTextTarget, BundleViewTextValue,
};
use arcweft_view::style::{
    ComputedViewStyle, ComputedViewStyleBuilder, ComputedViewStyleRevision,
    ViewAxisProviderParticipation, ViewBoxAxisHostSeed, ViewBoxAxisMode, ViewBoxAxisSeedGeneration,
    ViewColorValue, ViewElementState, ViewInheritedBoxAxes, ViewInteractionSelector,
    ViewLengthMilli, ViewOverflow, ViewPartName, ViewPropertyKind, ViewScalarMilli,
    ViewSpecifiedValue, ViewStyleApplication, ViewStyleApplicationTarget, ViewStyleAssignOp,
    ViewStyleBoundaryFacts, ViewStyleContribution, ViewStyleContributionSource,
    ViewStyleDeclaration, ViewStyleNodeFacts, ViewStyleNodeKey, ViewStylePatchId,
    ViewStylePriority, ViewStyleProgram, ViewStyleResolveContext, ViewStyleRevisionSet,
    ViewStyleRule, ViewStyleScopeId, ViewStyleSelector, ViewStyleSelectorSequence, ViewStyleSheet,
    ViewStyleSheetId, ViewStyleSourceId, ViewStyleTraceMode,
};
use arcweft_view::{ViewElementKind, ViewMountId};

#[test]
fn node_path_encoding_distinguishes_all_segment_families_and_key_presence() {
    let call_none = [BundleViewInstancePathSegment::Call {
        instruction: 7,
        authored_key: None,
    }];
    let call_zero = [BundleViewInstancePathSegment::Call {
        instruction: 7,
        authored_key: Some(0),
    }];
    let repeat = [BundleViewInstancePathSegment::Repeat {
        instruction: 7,
        key: 0,
    }];
    let repeat_negative = [BundleViewInstancePathSegment::Repeat {
        instruction: 7,
        key: -1,
    }];

    assert_ne!(encode_path(&call_none), encode_path(&call_zero));
    assert_ne!(encode_path(&call_none), encode_path(&repeat));
    assert_ne!(encode_path(&repeat), encode_path(&repeat_negative));
}

#[test]
fn box_style_consumes_only_canonical_physical_geometry() {
    let style = projected_style([
        length(ViewPropertyKind::Width, 120_000),
        length(ViewPropertyKind::Height, 44_000),
        length(ViewPropertyKind::TranslateX, 7_000),
        length(ViewPropertyKind::TranslateY, -3_000),
        (
            ViewPropertyKind::Scale,
            ViewSpecifiedValue::Scalar {
                value: ViewScalarMilli::new(875),
            },
        ),
        (
            ViewPropertyKind::OverflowX,
            ViewSpecifiedValue::Overflow {
                value: ViewOverflow::Clip,
            },
        ),
        (
            ViewPropertyKind::OverflowY,
            ViewSpecifiedValue::Overflow {
                value: ViewOverflow::Auto,
            },
        ),
    ]);

    let style = box_style(&style);
    assert_eq!(style.width, Some(120_000));
    assert_eq!(style.height, Some(44_000));
    assert_eq!(style.translate_x, 7_000);
    assert_eq!(style.translate_y, -3_000);
    assert_eq!(style.scale_milli, 875);
    assert_eq!(style.overflow_x, ViewOverflow::Clip);
    assert_eq!(style.overflow_y, ViewOverflow::Auto);
}

#[test]
fn column_gap_repositions_each_direct_child_subtree_from_actual_bounds() {
    let container = runtime_node(0);
    let first = runtime_node(1);
    let second = runtime_node(2);
    let first_key = control_key("control.first");
    let second_key = control_key("control.second");
    let nodes = vec![
        ResolvedLayoutNode {
            id: container.clone(),
            parent: None,
            element: Some(ViewElementKind::Column),
            keys: Vec::new(),
            style: projected_style([length(ViewPropertyKind::Gap, 14_000)]),
        },
        ResolvedLayoutNode {
            id: first,
            parent: Some(container.clone()),
            element: Some(ViewElementKind::Panel),
            keys: vec![first_key.clone()],
            style: ViewRuntimeNodeStyle::default(),
        },
        ResolvedLayoutNode {
            id: second,
            parent: Some(container),
            element: Some(ViewElementKind::Panel),
            keys: vec![second_key.clone()],
            style: ViewRuntimeNodeStyle::default(),
        },
    ];
    let presentation = BundlePresentationSnapshot {
        surfaces: vec![
            surface("control.first", 0, 20_000),
            surface("control.second", 36_000, 20_000),
        ],
        ..BundlePresentationSnapshot::default()
    };

    let offsets = resolve_layout_offsets(&presentation, &nodes);

    assert_eq!(offsets.get(&first_key), Some(&(0, 0)));
    assert_eq!(offsets.get(&second_key), Some(&(0, -2_000)));
}

#[test]
fn placeholder_shown_is_retained_as_a_typed_element_state() {
    let node = BundleViewStyleNode {
        path: BundleViewInstancePath::default(),
        instruction: 0,
        parent: None,
        kind: BundleViewStyleNodeKind::Element {
            element: ViewElementKind::TextField,
            target: Some("field.name".to_owned()),
        },
        part: None,
        exported_part: None,
        applications: Vec::new(),
    };
    let binding = NodeBinding {
        keys: Vec::new(),
        target: None,
        enabled: true,
        composing: false,
        placeholder_shown: true,
    };

    let facts = node_facts(&InputController::default(), &node, &binding).unwrap();

    assert!(
        facts
            .element_states()
            .contains(ViewElementState::PlaceholderShown)
    );
}

#[test]
fn hover_path_retains_ancestor_hover_and_simultaneous_child_focus() {
    let ancestor = interaction_target("target.ancestor");
    let child = interaction_target("target.child");
    let mut interaction = InteractionState::default();
    interaction.set_hover_path(HoverPath::new(
        PointerId(2),
        vec![ancestor.clone(), child.clone()],
    ));
    interaction.set_focus(FocusState::new(
        LayerId::new(PublicId::try_new("layer.view").unwrap()),
        child.clone(),
    ));

    let ancestor_states = interaction_states(&interaction, Some(&ancestor), true);
    let child_states = interaction_states(&interaction, Some(&child), true);

    assert!(ancestor_states.contains(ViewInteractionSelector::Hovered));
    assert!(!ancestor_states.contains(ViewInteractionSelector::Focused));
    assert!(child_states.contains(ViewInteractionSelector::Hovered));
    assert!(child_states.contains(ViewInteractionSelector::Focused));
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the live call boundary test keeps inherited root, exported no-winner, exported local barrier, descendant propagation, and three-adapter parity in one frame"
)]
fn inherited_style_resolves_across_a_live_call_view_mount_boundary() {
    let expected = PresentationColor::rgb(17, 34, 51);
    let (program, parent_application, child_application, exported_axis_application) =
        cross_mount_style_program(expected);
    let child_path: BundleViewInstancePath = serde_json::from_value(serde_json::json!([
        { "kind": "call", "instruction": 0 }
    ]))
    .unwrap();

    let mut parent = empty_mount();
    parent.host_axis_seed = Some(ViewInheritedBoxAxes::for_host_seed(
        parent.mount,
        ViewBoxAxisSeedGeneration::INITIAL,
        ViewBoxAxisHostSeed::Explicit(ViewBoxAxisMode::VerticalRl),
    ));
    parent.view = "view.Parent".to_owned();
    parent.style_nodes = vec![BundleViewStyleNode {
        path: BundleViewInstancePath::default(),
        instruction: 0,
        parent: None,
        kind: BundleViewStyleNodeKind::CallView {
            view: "view.Child".to_owned(),
        },
        part: Some("part.call".to_owned()),
        exported_part: None,
        applications: vec![parent_application],
    }];
    let mut child = empty_mount();
    child.mount = ViewMountId::from_raw(2);
    child.host_axis_seed = None;
    child.view = "view.Child".to_owned();
    child.path = child_path.clone();
    child.text = [
        ("text.child", "text.child.target", "child"),
        (
            "text.child.exported",
            "text.child.exported.target",
            "exported",
        ),
        (
            "text.child.exported.descendant",
            "text.child.exported.descendant.target",
            "descendant",
        ),
        (
            "text.child.exported.inherited",
            "text.child.exported.inherited.target",
            "inherited exported part",
        ),
    ]
    .into_iter()
    .map(|(source_id, target, value)| BundleViewTextOutput {
        source_id: source_id.to_owned(),
        targets: vec![BundleViewTextTarget {
            public_id: target.to_owned(),
            containing_scroll_region: None,
            bounds: ViewTextBlockBounds {
                x_milli: 0,
                y_milli: 0,
                width_milli: 100_000,
                height_milli: 20_000,
            },
            selection_policy: ViewTextSelectionPolicy::default(),
            style: ViewRuntimeControlVisualStyle::default(),
        }],
        value: BundleViewTextValue::Plain {
            value: value.to_owned(),
        },
        classification: ViewObserveClassification::default(),
        replacement: None,
    })
    .collect();
    child.style_nodes = vec![
        BundleViewStyleNode {
            path: child_path.clone(),
            instruction: 0,
            parent: None,
            kind: BundleViewStyleNodeKind::Text {
                text_source: "text.child".to_owned(),
            },
            part: None,
            exported_part: None,
            applications: vec![child_application.clone()],
        },
        BundleViewStyleNode {
            path: child_path.clone(),
            instruction: 1,
            parent: Some(BundleViewStyleNodeId {
                path: child_path.clone(),
                instruction: 0,
            }),
            kind: BundleViewStyleNodeKind::Text {
                text_source: "text.child.exported".to_owned(),
            },
            part: Some("part.child-exported".to_owned()),
            exported_part: Some("part.public-child".to_owned()),
            applications: vec![child_application.clone(), exported_axis_application],
        },
        BundleViewStyleNode {
            path: child_path.clone(),
            instruction: 2,
            parent: Some(BundleViewStyleNodeId {
                path: child_path.clone(),
                instruction: 1,
            }),
            kind: BundleViewStyleNodeKind::Text {
                text_source: "text.child.exported.descendant".to_owned(),
            },
            part: None,
            exported_part: None,
            applications: Vec::new(),
        },
        BundleViewStyleNode {
            path: child_path.clone(),
            instruction: 3,
            parent: Some(BundleViewStyleNodeId {
                path: child_path,
                instruction: 0,
            }),
            kind: BundleViewStyleNodeKind::Text {
                text_source: "text.child.exported.inherited".to_owned(),
            },
            part: Some("part.child-inherited-export".to_owned()),
            exported_part: Some("part.public-inherited".to_owned()),
            applications: vec![child_application],
        },
    ];
    let mut presentation = BundlePresentationSnapshot::default();
    presentation.view.mounts = vec![parent, child];

    let frames = ["native", "web", "headless"]
        .into_iter()
        .map(|_| {
            super::PlayerViewStyleState::default()
                .resolve(
                    &InputController::default(),
                    &presentation,
                    Some(&program),
                    &PresentationEnvironment::ENGINE_DEFAULT,
                    &SystemPaletteSet::ENGINE_DEFAULT,
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    for frame in &frames {
        for target in [
            "view_mount_2.text.child.target",
            "view_mount_2.text.child.exported.inherited.target",
        ] {
            assert_eq!(
                frame.text(target).unwrap().physical_box().axes,
                ViewBoxAxisMode::VerticalRl
            );
        }
        for target in [
            "view_mount_2.text.child.exported.target",
            "view_mount_2.text.child.exported.descendant.target",
        ] {
            assert_eq!(frame.text(target).unwrap().visual().text, Some(expected));
            assert_eq!(
                frame.text(target).unwrap().physical_box().axes,
                ViewBoxAxisMode::HorizontalRtl
            );
        }
    }
    for target in [
        "view_mount_2.text.child.target",
        "view_mount_2.text.child.exported.target",
        "view_mount_2.text.child.exported.descendant.target",
        "view_mount_2.text.child.exported.inherited.target",
    ] {
        assert_eq!(frames[0].text(target), frames[1].text(target));
        assert_eq!(frames[0].text(target), frames[2].text(target));
    }
}

#[test]
fn top_level_host_seed_is_required_and_explicit_modes_reach_the_shared_player_path() {
    let input = InputController::default();
    for &mode in ViewBoxAxisMode::ALL {
        let mut mount = empty_mount();
        mount.host_axis_seed = Some(ViewInheritedBoxAxes::for_host_seed(
            mount.mount,
            ViewBoxAxisSeedGeneration::INITIAL,
            ViewBoxAxisHostSeed::Explicit(mode),
        ));
        mount.style_nodes = vec![style_root_node(0, "control.axis")];
        let mut presentation = BundlePresentationSnapshot::default();
        presentation.view.mounts = vec![mount];
        let frame = super::PlayerViewStyleState::default()
            .resolve(
                &input,
                &presentation,
                Some(&ViewStyleProgram::default()),
                &PresentationEnvironment::ENGINE_DEFAULT,
                &SystemPaletteSet::ENGINE_DEFAULT,
            )
            .unwrap();
        assert_eq!(
            frame
                .control("view_mount_1.control.axis")
                .unwrap()
                .physical_box()
                .axes,
            mode
        );
    }

    let mut missing = empty_mount();
    missing.host_axis_seed = None;
    missing.style_nodes = vec![style_root_node(7, "control.missing")];
    let mut presentation = BundlePresentationSnapshot::default();
    presentation.view.mounts = vec![missing];
    assert_eq!(
        super::PlayerViewStyleState::default()
            .resolve(
                &input,
                &presentation,
                Some(&ViewStyleProgram::default()),
                &PresentationEnvironment::ENGINE_DEFAULT,
                &SystemPaletteSet::ENGINE_DEFAULT,
            )
            .unwrap_err(),
        PlayerFrameError::MissingHostAxisSeed {
            mount: ViewMountId::from_raw(1),
            instruction: 7,
        }
    );
}

#[test]
fn native_web_and_headless_style_states_match_for_default_and_every_explicit_seed() {
    let seeds = std::iter::once(ViewBoxAxisHostSeed::Default)
        .chain(
            ViewBoxAxisMode::ALL
                .iter()
                .copied()
                .map(ViewBoxAxisHostSeed::Explicit),
        )
        .collect::<Vec<_>>();
    for seed in seeds {
        let mut mount = empty_mount();
        let inherited = ViewInheritedBoxAxes::for_host_seed(
            mount.mount,
            ViewBoxAxisSeedGeneration::INITIAL,
            seed,
        );
        mount.host_axis_seed = Some(inherited);
        mount.style_nodes = vec![style_root_node(0, "control.parity")];
        let mut presentation = BundlePresentationSnapshot::default();
        presentation.view.mounts = vec![mount];

        let frames = ["native", "web", "headless"]
            .into_iter()
            .map(|_| {
                super::PlayerViewStyleState::default()
                    .resolve(
                        &InputController::default(),
                        &presentation,
                        Some(&ViewStyleProgram::default()),
                        &PresentationEnvironment::ENGINE_DEFAULT,
                        &SystemPaletteSet::ENGINE_DEFAULT,
                    )
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let target = "view_mount_1.control.parity";
        assert_eq!(frames[0].control(target), frames[1].control(target));
        assert_eq!(frames[0].control(target), frames[2].control(target));
        assert_eq!(
            frames[0].control(target).unwrap().physical_box().axes,
            seed.mode()
        );
        assert_eq!(
            serde_json::to_vec(&presentation.view.mounts[0].host_axis_seed).unwrap(),
            serde_json::to_vec(&Some(inherited)).unwrap()
        );
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the non-inference contract compares one retained node across rich-text, locale, and palette-only mutations before checking cache identity"
)]
fn locale_rich_text_direction_and_theme_never_infer_a_different_axis_provider() {
    let mut mount = empty_mount();
    let inherited = mount.host_axis_seed.unwrap();
    mount.text = vec![BundleViewTextOutput {
        source_id: "text.no-axis-inference".to_owned(),
        targets: vec![BundleViewTextTarget {
            public_id: "text.no-axis-inference.target".to_owned(),
            containing_scroll_region: None,
            bounds: ViewTextBlockBounds {
                x_milli: 0,
                y_milli: 0,
                width_milli: 100_000,
                height_milli: 20_000,
            },
            selection_policy: ViewTextSelectionPolicy::default(),
            style: ViewRuntimeControlVisualStyle::default(),
        }],
        value: BundleViewTextValue::Plain {
            value: "baseline".to_owned(),
        },
        classification: ViewObserveClassification::default(),
        replacement: None,
    }];
    mount.style_nodes = vec![BundleViewStyleNode {
        path: BundleViewInstancePath::default(),
        instruction: 0,
        parent: None,
        kind: BundleViewStyleNodeKind::Text {
            text_source: "text.no-axis-inference".to_owned(),
        },
        part: None,
        exported_part: None,
        applications: Vec::new(),
    }];
    let mut presentation = BundlePresentationSnapshot::default();
    presentation.view.mounts = vec![mount];
    let program = ViewStyleProgram::default();
    let mut state = super::PlayerViewStyleState::default();
    let baseline = state
        .resolve(
            &InputController::default(),
            &presentation,
            Some(&program),
            &PresentationEnvironment::ENGINE_DEFAULT,
            &SystemPaletteSet::ENGINE_DEFAULT,
        )
        .unwrap();

    presentation.view.mounts[0].text[0].value = BundleViewTextValue::RichTextDocument {
        document: Box::new(RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Layout {
                    layout: RichTextLayout {
                        direction: RichTextInlineDirection::Rtl,
                        ..RichTextLayout::default()
                    },
                },
            },
            RichTextNode::Text {
                text: "changed text and inline direction".to_owned(),
            },
        ])),
    };
    let changed_text = state
        .resolve(
            &InputController::default(),
            &presentation,
            Some(&program),
            &PresentationEnvironment::ENGINE_DEFAULT,
            &SystemPaletteSet::ENGINE_DEFAULT,
        )
        .unwrap();
    let target = "view_mount_1.text.no-axis-inference.target";
    assert_eq!(baseline.text(target), changed_text.text(target));

    let localized_environment = PresentationEnvironment::ENGINE_DEFAULT
        .with_locale(PublicId::try_new("locale.ja_jp").expect("locale identity is valid"));
    let localized = state
        .resolve(
            &InputController::default(),
            &presentation,
            Some(&program),
            &localized_environment,
            &SystemPaletteSet::ENGINE_DEFAULT,
        )
        .unwrap();
    assert_eq!(baseline.text(target), localized.text(target));

    let mut changed_theme = SystemPaletteSet::ENGINE_DEFAULT;
    changed_theme.light.accent = PresentationColor::rgb(250, 17, 99);
    changed_theme.dark.accent = PresentationColor::rgb(11, 222, 73);
    let themed = state
        .resolve(
            &InputController::default(),
            &presentation,
            Some(&program),
            &PresentationEnvironment::ENGINE_DEFAULT,
            &changed_theme,
        )
        .unwrap();
    assert_eq!(baseline.text(target), themed.text(target));

    let key = ViewStyleNodeKey::new(ViewMountId::from_raw(1), Vec::new(), 0);
    let facts = ViewStyleNodeFacts::new(None);
    let cached = state
        .resolver
        .resolve(
            &program,
            &ViewStyleResolveContext {
                node_key: &key,
                node: &facts,
                ancestors: &[],
                applications: &[],
                parent: None,
                parent_node_key: None,
                inherited_axes: inherited,
                axis_provider_participation: ViewAxisProviderParticipation::ProjectionOnly,
                environment: &PresentationEnvironment::ENGINE_DEFAULT,
                revisions: ViewStyleRevisionSet {
                    sheets: state.program_revision,
                    patches: state.program_revision,
                    tokens: state.program_revision,
                    applications: presentation.revision,
                    interactions: 0,
                    containers: 0,
                },
                trace: ViewStyleTraceMode::Off,
            },
        )
        .unwrap();
    assert!(
        cached.cache_hit(),
        "locale, rich-text direction/content, and palette changes must not evict the typed axis provider entry"
    );
    assert_eq!(cached.computed().axes().revision(), inherited.revision());
}

#[test]
fn nested_mount_rejects_a_host_seed_before_style_resolution() {
    let mut mount = empty_mount();
    mount.mount = ViewMountId::from_raw(9);
    mount.path = serde_json::from_value(serde_json::json!([
        { "kind": "call", "instruction": 3 }
    ]))
    .unwrap();
    mount.style_nodes = vec![style_root_node(3, "control.nested")];
    let mut presentation = BundlePresentationSnapshot::default();
    presentation.view.mounts = vec![mount.clone()];

    assert_eq!(
        super::PlayerViewStyleState::default()
            .resolve(
                &InputController::default(),
                &presentation,
                Some(&ViewStyleProgram::default()),
                &PresentationEnvironment::ENGINE_DEFAULT,
                &SystemPaletteSet::ENGINE_DEFAULT,
            )
            .unwrap_err(),
        PlayerFrameError::UnexpectedHostAxisSeed {
            mount: ViewMountId::from_raw(9),
        }
    );

    mount.host_axis_seed = None;
    presentation.view.mounts = vec![mount];
    assert_eq!(
        super::PlayerViewStyleState::default()
            .resolve(
                &InputController::default(),
                &presentation,
                Some(&ViewStyleProgram::default()),
                &PresentationEnvironment::ENGINE_DEFAULT,
                &SystemPaletteSet::ENGINE_DEFAULT,
            )
            .unwrap_err(),
        PlayerFrameError::MissingStyleParent {
            mount: 9,
            instruction: 3,
        }
    );
}

#[test]
fn disappearing_mounts_are_removed_from_the_long_lived_provider_index() {
    let mut mount = empty_mount();
    let removed_mount = mount.mount;
    mount.style_nodes = vec![style_root_node(0, "control.cleanup")];
    let mut presentation = BundlePresentationSnapshot::default();
    presentation.view.mounts = vec![mount];
    let mut state = super::PlayerViewStyleState::default();
    state
        .resolve(
            &InputController::default(),
            &presentation,
            Some(&ViewStyleProgram::default()),
            &PresentationEnvironment::ENGINE_DEFAULT,
            &SystemPaletteSet::ENGINE_DEFAULT,
        )
        .unwrap();

    state
        .resolve(
            &InputController::default(),
            &BundlePresentationSnapshot::default(),
            Some(&ViewStyleProgram::default()),
            &PresentationEnvironment::ENGINE_DEFAULT,
            &SystemPaletteSet::ENGINE_DEFAULT,
        )
        .unwrap();
    assert!(state.live_mounts.is_empty());
    assert_eq!(state.resolver.invalidate_mount(removed_mount), 0);
}

#[test]
fn generated_row_target_does_not_masquerade_as_a_surface_consumer() {
    let mount = empty_mount();
    let node = BundleViewStyleNode {
        path: BundleViewInstancePath::default(),
        instruction: 4,
        parent: None,
        kind: BundleViewStyleNodeKind::Element {
            element: ViewElementKind::Row,
            target: Some("generated.row".to_owned()),
        },
        part: None,
        exported_part: None,
        applications: Vec::new(),
    };
    let computed = computed_style([(
        ViewPropertyKind::BackgroundColor,
        ViewSpecifiedValue::Color {
            value: ViewColorValue::Literal {
                color: PresentationColor::rgb(1, 2, 3),
            },
        },
    )]);

    let error = validate_supported_properties(
        &BundlePresentationSnapshot::default(),
        &mount,
        &node,
        &[],
        &computed,
    )
    .unwrap_err();

    assert_eq!(
        error,
        PlayerFrameError::UnsupportedStyleProperty {
            mount: 1,
            instruction: 4,
            target: "structural layout",
            property: ViewPropertyKind::BackgroundColor,
        }
    );
}

#[test]
fn silent_runtime_style_drops_are_rejected_with_typed_errors() {
    let mount = empty_mount();
    let node = BundleViewStyleNode {
        path: BundleViewInstancePath::default(),
        instruction: 13,
        parent: None,
        kind: BundleViewStyleNodeKind::Custom {
            element: "test.consumer".to_owned(),
        },
        part: None,
        exported_part: None,
        applications: Vec::new(),
    };
    let cases = [
        (StyleConsumer::Control, ViewPropertyKind::OverflowX),
        (StyleConsumer::Control, ViewPropertyKind::OverflowY),
        (StyleConsumer::Image, ViewPropertyKind::OverflowX),
        (StyleConsumer::Image, ViewPropertyKind::OverflowY),
        (StyleConsumer::Text, ViewPropertyKind::ZIndex),
        (StyleConsumer::Text, ViewPropertyKind::Opacity),
        (StyleConsumer::Text, ViewPropertyKind::BoxShadow),
        (StyleConsumer::Text, ViewPropertyKind::Filter),
        (StyleConsumer::Text, ViewPropertyKind::BackdropFilter),
        (StyleConsumer::Text, ViewPropertyKind::PlaceholderColor),
        (StyleConsumer::Text, ViewPropertyKind::CaretColor),
        (
            StyleConsumer::Text,
            ViewPropertyKind::CompositionUnderlineColor,
        ),
        (
            StyleConsumer::Surface(ViewElementKind::Panel),
            ViewPropertyKind::OutlineColor,
        ),
        (
            StyleConsumer::Surface(ViewElementKind::Panel),
            ViewPropertyKind::OutlineWidth,
        ),
        (
            StyleConsumer::Surface(ViewElementKind::Panel),
            ViewPropertyKind::OutlineOffset,
        ),
        (
            StyleConsumer::Surface(ViewElementKind::Panel),
            ViewPropertyKind::FocusRingColor,
        ),
        (
            StyleConsumer::Surface(ViewElementKind::Panel),
            ViewPropertyKind::FocusRingWidth,
        ),
        (
            StyleConsumer::Surface(ViewElementKind::Panel),
            ViewPropertyKind::CornerFrameColor,
        ),
        (
            StyleConsumer::Surface(ViewElementKind::Panel),
            ViewPropertyKind::CornerFrameWidth,
        ),
        (
            StyleConsumer::Surface(ViewElementKind::Panel),
            ViewPropertyKind::CornerFrameLength,
        ),
        (
            StyleConsumer::Surface(ViewElementKind::Panel),
            ViewPropertyKind::CornerFrameOffset,
        ),
    ];

    for (consumer, property) in cases {
        let computed = computed_style([(property, unsupported_consumer_value(property))]);
        let error = validate_consumer_properties(&mount, &node, consumer, &computed)
            .expect_err("unsupported property must fail instead of being dropped");

        assert_eq!(
            error,
            PlayerFrameError::UnsupportedStyleProperty {
                mount: 1,
                instruction: 13,
                target: consumer.label(),
                property,
            }
        );
    }
}

#[test]
fn image_consumer_allows_parent_inherited_color_but_rejects_direct_color() {
    let mount = empty_mount();
    let node = BundleViewStyleNode {
        path: BundleViewInstancePath::default(),
        instruction: 14,
        parent: None,
        kind: BundleViewStyleNodeKind::Image {
            image: "image.hero".to_owned(),
            target: None,
        },
        part: None,
        exported_part: None,
        applications: Vec::new(),
    };
    let property = ViewPropertyKind::Color;
    let value = ViewSpecifiedValue::Color {
        value: ViewColorValue::Literal {
            color: PresentationColor::rgb(17, 34, 51),
        },
    };
    let inherited = inherited_computed_style(property, value.clone());

    validate_consumer_properties(&mount, &node, StyleConsumer::Image, &inherited)
        .expect("parent-inherited color crosses the image boundary without becoming direct work");

    let direct = computed_style([(property, value)]);
    let error = validate_consumer_properties(&mount, &node, StyleConsumer::Image, &direct)
        .expect_err("direct image color has no renderer consumer");
    assert_eq!(
        error,
        PlayerFrameError::UnsupportedStyleProperty {
            mount: 1,
            instruction: 14,
            target: "image",
            property,
        }
    );
}

#[test]
fn targetless_image_binds_its_mount_scoped_image_identity() {
    let mount = empty_mount();
    let node = BundleViewStyleNode {
        path: BundleViewInstancePath::default(),
        instruction: 8,
        parent: None,
        kind: BundleViewStyleNodeKind::Image {
            image: "image.hero".to_owned(),
            target: None,
        },
        part: None,
        exported_part: None,
        applications: Vec::new(),
    };

    let bindings = node_bindings(
        &BundlePresentationSnapshot::default(),
        &InputController::default(),
        &mount,
        &node,
    )
    .unwrap();

    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].keys.len(), 1);
    assert_eq!(bindings[0].keys[0].kind, StyleTargetKind::Image);
    assert_eq!(bindings[0].keys[0].id, "view_mount_1.image.hero");
}

fn cross_mount_style_program(
    color: PresentationColor,
) -> (
    ViewStyleProgram,
    ViewStyleApplication,
    ViewStyleApplication,
    ViewStyleApplication,
) {
    let source = ViewStyleSourceId::new(0);
    let sheet_id = ViewStyleSheetId::try_new("style.cross-mount").unwrap();
    let selector = ViewStyleSelector::new(vec![
        ViewStyleSelectorSequence::new(
            None,
            None,
            Some(ViewPartName::try_new("part.call").unwrap()),
            Vec::new(),
        )
        .unwrap(),
    ])
    .unwrap();
    let declaration = ViewStyleDeclaration::new(
        ViewPropertyKind::Color,
        ViewSpecifiedValue::Color {
            value: ViewColorValue::Literal { color },
        },
        ViewStyleAssignOp::Replace,
        source,
    )
    .unwrap();
    let rule = ViewStyleRule::new(selector, vec![declaration], 0, source).unwrap();
    let sheet = ViewStyleSheet::new(sheet_id.clone(), Vec::new(), vec![rule]).unwrap();
    let axis_sheet_id = ViewStyleSheetId::try_new("style.exported-axis").unwrap();
    let axis_selector = ViewStyleSelector::new(vec![
        ViewStyleSelectorSequence::new(
            None,
            None,
            Some(ViewPartName::try_new("part.child-exported").unwrap()),
            Vec::new(),
        )
        .unwrap(),
    ])
    .unwrap();
    let axis_rule = ViewStyleRule::new(
        axis_selector,
        vec![
            ViewStyleDeclaration::new(
                ViewPropertyKind::BoxAxes,
                ViewSpecifiedValue::BoxAxes {
                    value: ViewBoxAxisMode::HorizontalRtl,
                },
                ViewStyleAssignOp::Replace,
                ViewStyleSourceId::new(1),
            )
            .unwrap(),
        ],
        0,
        ViewStyleSourceId::new(1),
    )
    .unwrap();
    let axis_sheet =
        ViewStyleSheet::new(axis_sheet_id.clone(), Vec::new(), vec![axis_rule]).unwrap();
    let program = ViewStyleProgram::try_new(vec![sheet, axis_sheet], Vec::new()).unwrap();
    let scope = ViewStyleScopeId::new(1);
    let parent = ViewStyleApplication::new(
        ViewStyleApplicationTarget::named(sheet_id.clone()),
        scope,
        0,
        0,
        ViewStyleBoundaryFacts::SAME_VIEW,
    );
    let child = ViewStyleApplication::new(
        ViewStyleApplicationTarget::named(sheet_id),
        scope,
        0,
        0,
        ViewStyleBoundaryFacts::nested_view(1, false, true),
    );
    let exported_axis = ViewStyleApplication::new(
        ViewStyleApplicationTarget::named(axis_sheet_id),
        ViewStyleScopeId::new(2),
        0,
        1,
        ViewStyleBoundaryFacts::SAME_VIEW,
    );
    (program, parent, child, exported_axis)
}

fn projected_style(
    entries: impl IntoIterator<Item = (ViewPropertyKind, ViewSpecifiedValue)>,
) -> ViewRuntimeNodeStyle {
    let computed = computed_style(entries);
    ViewRuntimeNodeStyle::try_from_computed(
        &computed,
        &PresentationEnvironment::ENGINE_DEFAULT,
        &SystemPaletteSet::ENGINE_DEFAULT,
    )
    .unwrap()
}

fn computed_style(
    entries: impl IntoIterator<Item = (ViewPropertyKind, ViewSpecifiedValue)>,
) -> ComputedViewStyle {
    let mut builder = ComputedViewStyleBuilder::default();
    for (order, (property, value)) in entries.into_iter().enumerate() {
        assert!(builder.apply(ViewStyleContribution::new(
            property,
            value,
            ViewStyleAssignOp::Replace,
            ViewStylePriority::new(1, 1, 0, 0, 0, u32::try_from(order).unwrap_or(u32::MAX),),
            ViewStyleContributionSource::Patch {
                patch: ViewStylePatchId::new(0),
                declaration: ViewStyleSourceId::new(u32::try_from(order).unwrap_or(u32::MAX),),
            },
        )));
    }
    builder.finish(ComputedViewStyleRevision::new(1))
}

fn inherited_computed_style(
    property: ViewPropertyKind,
    value: ViewSpecifiedValue,
) -> ComputedViewStyle {
    let mut builder = ComputedViewStyleBuilder::default();
    assert!(builder.apply(ViewStyleContribution::new(
        property,
        value,
        ViewStyleAssignOp::Replace,
        ViewStylePriority::INHERITED,
        ViewStyleContributionSource::Inherited,
    )));
    builder.finish(ComputedViewStyleRevision::new(1))
}

fn length(property: ViewPropertyKind, value: i32) -> (ViewPropertyKind, ViewSpecifiedValue) {
    (
        property,
        ViewSpecifiedValue::Length {
            value: ViewLengthMilli::new(value),
        },
    )
}

fn unsupported_consumer_value(property: ViewPropertyKind) -> ViewSpecifiedValue {
    match property {
        ViewPropertyKind::Overflow
        | ViewPropertyKind::OverflowX
        | ViewPropertyKind::OverflowY
        | ViewPropertyKind::OverflowInline
        | ViewPropertyKind::OverflowBlock => ViewSpecifiedValue::Overflow {
            value: ViewOverflow::Hidden,
        },
        ViewPropertyKind::ZIndex => ViewSpecifiedValue::Integer { value: 7 },
        ViewPropertyKind::Opacity => ViewSpecifiedValue::Scalar {
            value: ViewScalarMilli::new(500),
        },
        ViewPropertyKind::BoxShadow => ViewSpecifiedValue::ShadowList { value: Vec::new() },
        ViewPropertyKind::Filter | ViewPropertyKind::BackdropFilter => {
            ViewSpecifiedValue::FilterList { value: Vec::new() }
        }
        ViewPropertyKind::PlaceholderColor
        | ViewPropertyKind::CaretColor
        | ViewPropertyKind::CompositionUnderlineColor
        | ViewPropertyKind::OutlineColor
        | ViewPropertyKind::FocusRingColor
        | ViewPropertyKind::CornerFrameColor => ViewSpecifiedValue::Color {
            value: ViewColorValue::Literal {
                color: PresentationColor::rgb(1, 2, 3),
            },
        },
        ViewPropertyKind::OutlineWidth
        | ViewPropertyKind::OutlineOffset
        | ViewPropertyKind::FocusRingWidth
        | ViewPropertyKind::CornerFrameWidth
        | ViewPropertyKind::CornerFrameLength
        | ViewPropertyKind::CornerFrameOffset => ViewSpecifiedValue::Length {
            value: ViewLengthMilli::new(1_000),
        },
        _ => unreachable!("test table only contains known unsupported consumer properties"),
    }
}

fn runtime_node(instruction: u32) -> RuntimeNodeId {
    RuntimeNodeId {
        mount: ViewMountId::from_raw(1),
        path: Vec::new(),
        instruction,
    }
}

fn control_key(id: &str) -> StyleTargetKey {
    StyleTargetKey {
        kind: StyleTargetKind::Control,
        id: id.to_owned(),
    }
}

fn surface(id: &str, y_milli: i32, height_milli: u32) -> ViewRuntimeSurface {
    ViewRuntimeSurface {
        public_id: id.to_owned(),
        target: id.to_owned(),
        view: None,
        containing_scroll_region: None,
        element: ViewElementKind::Panel,
        bounds: ViewRuntimeSurfaceBounds {
            x_milli: 0,
            y_milli,
            width_milli: 100_000,
            height_milli,
        },
        style: ViewRuntimeControlVisualStyle::default(),
    }
}

fn empty_mount() -> BundleViewMountOutput {
    let mount = ViewMountId::from_raw(1);
    BundleViewMountOutput {
        handle: PresentationHandleId::try_new("handle.test").unwrap(),
        mount,
        host_axis_seed: Some(ViewInheritedBoxAxes::for_host_seed(
            mount,
            ViewBoxAxisSeedGeneration::INITIAL,
            ViewBoxAxisHostSeed::Default,
        )),
        view: "view.Test".to_owned(),
        path: BundleViewInstancePath::default(),
        dialogue: None,
        active_targets: Vec::new(),
        active_images: Vec::new(),
        paint: Vec::new(),
        text: Vec::new(),
        fx: Vec::new(),
        style_nodes: Vec::new(),
    }
}

fn style_root_node(instruction: u32, target: &str) -> BundleViewStyleNode {
    BundleViewStyleNode {
        path: BundleViewInstancePath::default(),
        instruction,
        parent: None,
        kind: BundleViewStyleNodeKind::Element {
            element: ViewElementKind::Panel,
            target: Some(target.to_owned()),
        },
        part: None,
        exported_part: None,
        applications: Vec::new(),
    }
}

fn interaction_target(id: &str) -> InteractionTarget {
    InteractionTarget::new(PublicId::try_new(id).unwrap())
}
