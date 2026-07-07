use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::hover::HoverPath;
use arcweft_presentation::input::{InputEpoch, InputEvent, InteractionTarget, PointerId};
use arcweft_presentation::interaction::InteractionState;
use arcweft_presentation::layer::{
    LayerId, LayerInputPolicy, LayerKind, LayerNode, LayerOrder, LayerTree, RenderPhase,
};
use arcweft_presentation::semantic::SemanticRole;
use arcweft_runtime_host::UiFrameCommitBuilder;
use arcweft_view::{
    EventBinding, EventKind, FragmentKind, HandlerId, LayoutBox, LayoutLength, LayoutPoint,
    LayoutResults, LayoutSize, LayoutTree, NodeKey, Rgba8, RichTextSourceId, SemanticSpecId,
    StyleId, UiInteractionSelector, UiLayerOutput, UiPropertyKind, UiPropertyValue,
    UiSemanticFragmentBuilder, UiSemanticNode, UiStyle, UiStyleTable, ViewFragmentBuilder,
};

fn public_id(value: &str) -> PublicId {
    PublicId::try_new(value).unwrap()
}

fn layer(value: &str) -> LayerId {
    LayerId::new(public_id(&format!("layer.{value}")))
}

fn target(value: &str) -> InteractionTarget {
    InteractionTarget::new(public_id(&format!("target.{value}")))
}

#[test]
fn frame_commit_preserves_handler_dispatch_and_resolved_interaction_style() {
    let root = layer("root");
    let ui = layer("ui");
    let button = target("button.confirm");
    let mut layers = LayerTree::new(LayerNode::new(
        root.clone(),
        LayerKind::Root,
        LayerOrder {
            phase: RenderPhase::Background,
            z: 0,
            stable_index: 0,
        },
    ));
    layers
        .insert(
            LayerNode::new(
                ui.clone(),
                LayerKind::GameUi,
                LayerOrder {
                    phase: RenderPhase::GameUi,
                    z: 0,
                    stable_index: 0,
                },
            )
            .with_parent(root)
            .with_input_policy(LayerInputPolicy::HitTest),
        )
        .unwrap();

    let mut fragment = ViewFragmentBuilder::default();
    let node = fragment
        .push_node(
            NodeKey(1),
            FragmentKind::RichText(RichTextSourceId(1)),
            StyleId(1),
            &[],
            &[EventBinding::new(EventKind::Activate, HandlerId(5))],
            Some(SemanticSpecId(0)),
        )
        .unwrap();
    let fragment = fragment.finish();
    let layout_tree = LayoutTree::from_fragment(&fragment).unwrap();
    let mut layouts = LayoutResults::new(&layout_tree);
    layouts
        .set(
            node,
            LayoutBox::new(
                LayoutPoint::new(LayoutLength::px(0), LayoutLength::px(0)),
                LayoutSize::new(LayoutLength::px(120), LayoutLength::px(40)),
            ),
        )
        .unwrap();
    let mut semantics = UiSemanticFragmentBuilder::default();
    semantics
        .push(
            UiSemanticNode::new(
                NodeKey(1),
                ui.clone(),
                button.clone(),
                SemanticRole::Button,
                HitRect::new(0.0, 0.0, 120.0, 40.0),
            )
            .with_action(public_id("action.confirm")),
        )
        .unwrap();
    let mut style = UiStyle::default();
    style
        .set_base(
            UiPropertyKind::BackgroundColor,
            UiPropertyValue::Color(Rgba8::new(10, 20, 30, 255)),
        )
        .unwrap();
    style
        .set_rule(
            UiInteractionSelector::Hovered,
            UiPropertyKind::BackgroundColor,
            UiPropertyValue::Color(Rgba8::new(40, 80, 120, 255)),
        )
        .unwrap();
    let mut styles = UiStyleTable::default();
    styles.insert(StyleId(1), style).unwrap();
    let output =
        UiLayerOutput::from_fragment_with_styles(&fragment, &layouts, semantics.finish(), styles)
            .unwrap();

    let mut builder = UiFrameCommitBuilder::new(&layers);
    builder.push_layer(ui.clone(), output).unwrap();
    let commit = builder.finish();
    let input = InputEvent::activate(InputEpoch(2), button.clone());
    assert_eq!(commit.dispatch_input(&input)[0].handler(), HandlerId(5));

    let mut interaction = InteractionState::default();
    let _ = interaction.set_hover_path(HoverPath::new(PointerId(0), vec![button]));
    let resolved = commit.resolve_interaction_styles(&interaction).unwrap();
    assert_eq!(resolved[0].layer(), &ui);
    assert_eq!(
        resolved[0].display().as_slice()[0]
            .style()
            .color(UiPropertyKind::BackgroundColor),
        Some(Rgba8::new(40, 80, 120, 255))
    );
}
