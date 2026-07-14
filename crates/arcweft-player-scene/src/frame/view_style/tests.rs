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
use arcweft_runtime_driver::display::BundlePresentationSnapshot;
use arcweft_runtime_driver::presentation_handles::PresentationHandleId;
use arcweft_runtime_driver::view_runtime::{
    BundleViewInstancePath, BundleViewInstancePathSegment, BundleViewMountOutput,
    BundleViewStyleNode, BundleViewStyleNodeKind, BundleViewTextOutput, BundleViewTextTarget,
    BundleViewTextValue,
};
use arcweft_view::style::{
    ComputedViewStyle, ComputedViewStyleBuilder, ComputedViewStyleRevision, ViewColorValue,
    ViewElementState, ViewInteractionSelector, ViewLengthMilli, ViewOverflow, ViewPartName,
    ViewPropertyKind, ViewScalarMilli, ViewSpecifiedValue, ViewStyleApplication,
    ViewStyleApplicationTarget, ViewStyleAssignOp, ViewStyleBoundaryFacts, ViewStyleContribution,
    ViewStyleContributionSource, ViewStyleDeclaration, ViewStylePatchId, ViewStylePriority,
    ViewStyleProgram, ViewStyleRule, ViewStyleScopeId, ViewStyleSelector,
    ViewStyleSelectorSequence, ViewStyleSheet, ViewStyleSheetId, ViewStyleSourceId,
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
fn inherited_style_resolves_across_a_live_call_view_mount_boundary() {
    let expected = PresentationColor::rgb(17, 34, 51);
    let (program, parent_application, child_application) = cross_mount_style_program(expected);
    let child_path: BundleViewInstancePath = serde_json::from_value(serde_json::json!([
        { "kind": "call", "instruction": 0 }
    ]))
    .unwrap();

    let mut parent = empty_mount();
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
    child.view = "view.Child".to_owned();
    child.path = child_path.clone();
    child.text = vec![BundleViewTextOutput {
        source_id: "text.child".to_owned(),
        targets: vec![BundleViewTextTarget {
            public_id: "text.child.target".to_owned(),
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
            value: "child".to_owned(),
        },
        classification: ViewObserveClassification::default(),
        replacement: None,
    }];
    child.style_nodes = vec![BundleViewStyleNode {
        path: child_path,
        instruction: 0,
        parent: None,
        kind: BundleViewStyleNodeKind::Text {
            text_source: "text.child".to_owned(),
        },
        part: None,
        exported_part: None,
        applications: vec![child_application],
    }];
    let mut presentation = BundlePresentationSnapshot::default();
    presentation.view.mounts = vec![parent, child];

    let frame = super::PlayerViewStyleState::default()
        .resolve(
            &InputController::default(),
            &presentation,
            Some(&program),
            &PresentationEnvironment::ENGINE_DEFAULT,
            &SystemPaletteSet::ENGINE_DEFAULT,
        )
        .unwrap();

    assert_eq!(
        frame
            .text("view_mount_2.text.child.target")
            .unwrap()
            .visual()
            .text,
        Some(expected)
    );
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
) -> (ViewStyleProgram, ViewStyleApplication, ViewStyleApplication) {
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
    let program = ViewStyleProgram::try_new(vec![sheet], Vec::new()).unwrap();
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
    (program, parent, child)
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
        mount: 1,
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
    BundleViewMountOutput {
        handle: PresentationHandleId::try_new("handle.test").unwrap(),
        mount: ViewMountId::from_raw(1),
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

fn interaction_target(id: &str) -> InteractionTarget {
    InteractionTarget::new(PublicId::try_new(id).unwrap())
}
