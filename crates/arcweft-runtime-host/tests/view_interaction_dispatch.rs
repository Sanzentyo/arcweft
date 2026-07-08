use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::hover::HoverPath;
use arcweft_presentation::input::{InputEpoch, InputEvent, InteractionTarget, PointerId};
use arcweft_presentation::interaction::InteractionState;
use arcweft_presentation::layer::{
    LayerId, LayerInputPolicy, LayerKind, LayerNode, LayerOrder, LayerTree, RenderPhase,
};
use arcweft_presentation::semantic::SemanticRole;
use arcweft_runtime_host::ViewFrameCommitBuilder;
use arcweft_view::{
    EventBinding, EventKind, FragmentKind, HandlerId, LayoutBox, LayoutLength, LayoutPoint,
    LayoutResults, LayoutSize, LayoutTree, NodeKey, Rgba8, RichTextSourceId, SemanticSpecId,
    StyleId, ViewFragmentBuilder, ViewInteractionSelector, ViewLayerOutput, ViewPropertyKind,
    ViewPropertyValue, ViewSemanticFragmentBuilder, ViewSemanticNode, ViewStyle, ViewStyleTable,
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
    let view = layer("view");
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
                view.clone(),
                LayerKind::GameView,
                LayerOrder {
                    phase: RenderPhase::GameView,
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
    let mut semantics = ViewSemanticFragmentBuilder::default();
    semantics
        .push(
            ViewSemanticNode::new(
                NodeKey(1),
                view.clone(),
                button.clone(),
                SemanticRole::Button,
                HitRect::new(0.0, 0.0, 120.0, 40.0),
            )
            .with_action(public_id("action.confirm")),
        )
        .unwrap();
    let mut style = ViewStyle::default();
    style
        .set_base(
            ViewPropertyKind::BackgroundColor,
            ViewPropertyValue::Color(Rgba8::new(10, 20, 30, 255)),
        )
        .unwrap();
    style
        .set_rule(
            ViewInteractionSelector::Hovered,
            ViewPropertyKind::BackgroundColor,
            ViewPropertyValue::Color(Rgba8::new(40, 80, 120, 255)),
        )
        .unwrap();
    let mut styles = ViewStyleTable::default();
    styles.insert(StyleId(1), style).unwrap();
    let output =
        ViewLayerOutput::from_fragment_with_styles(&fragment, &layouts, semantics.finish(), styles)
            .unwrap();

    let mut builder = ViewFrameCommitBuilder::new(&layers);
    builder.push_layer(view.clone(), output).unwrap();
    let commit = builder.finish();
    let input = InputEvent::activate(InputEpoch(2), button.clone());
    assert_eq!(commit.dispatch_input(&input)[0].handler(), HandlerId(5));

    let mut interaction = InteractionState::default();
    let _ = interaction.set_hover_path(HoverPath::new(PointerId(0), vec![button]));
    let resolved = commit.resolve_interaction_styles(&interaction).unwrap();
    assert_eq!(resolved[0].layer(), &view);
    assert_eq!(
        resolved[0].display().as_slice()[0]
            .style()
            .color(ViewPropertyKind::BackgroundColor),
        Some(Rgba8::new(40, 80, 120, 255))
    );
}
