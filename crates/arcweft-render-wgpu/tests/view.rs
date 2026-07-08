use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::interaction::{FocusState, InteractionState};
use arcweft_presentation::layer::LayerId;
use arcweft_presentation::semantic::SemanticRole;
use arcweft_render_wgpu::view::ViewPaintPlan;
use arcweft_view::{
    FragmentKind, LayoutBox, LayoutLength, LayoutPoint, LayoutResults, LayoutSize, LayoutTree,
    Milli, NodeKey, Rgba8, RichTextSourceId, SemanticSpecId, StyleId, ViewFragmentBuilder,
    ViewInteractionSelector, ViewPropertyKind, ViewPropertyValue, ViewSemanticFragmentBuilder,
    ViewSemanticNode, ViewStyle, ViewStyleTable,
};

fn public_id(value: &str) -> PublicId {
    PublicId::try_new(value).unwrap()
}

#[test]
fn resolved_focus_style_lowers_to_background_and_outline_rectangles() {
    let layer = LayerId::new(public_id("layer.view"));
    let target = InteractionTarget::new(public_id("target.button.confirm"));
    let mut fragment = ViewFragmentBuilder::default();
    let node = fragment
        .push_node(
            NodeKey(1),
            FragmentKind::RichText(RichTextSourceId(1)),
            StyleId(1),
            &[],
            &[],
            Some(SemanticSpecId(0)),
        )
        .unwrap();
    let fragment = fragment.finish();
    let tree = LayoutTree::from_fragment(&fragment).unwrap();
    let mut layouts = LayoutResults::new(&tree);
    layouts
        .set(
            node,
            LayoutBox::new(
                LayoutPoint::new(LayoutLength::px(10), LayoutLength::px(20)),
                LayoutSize::new(LayoutLength::px(100), LayoutLength::px(40)),
            ),
        )
        .unwrap();
    let display = arcweft_view::DisplayList::from_fragment(&fragment, &layouts).unwrap();
    let mut semantics = ViewSemanticFragmentBuilder::default();
    semantics
        .push(ViewSemanticNode::new(
            NodeKey(1),
            layer.clone(),
            target.clone(),
            SemanticRole::Button,
            HitRect::new(10.0, 20.0, 100.0, 40.0),
        ))
        .unwrap();
    let semantics = semantics.finish();

    let mut style = ViewStyle::default();
    style
        .set_base(
            ViewPropertyKind::BackgroundColor,
            ViewPropertyValue::Color(Rgba8::new(30, 60, 90, 255)),
        )
        .unwrap();
    style
        .set_rule(
            ViewInteractionSelector::Focused,
            ViewPropertyKind::OutlineColor,
            ViewPropertyValue::Color(Rgba8::new(120, 210, 255, 255)),
        )
        .unwrap();
    style
        .set_rule(
            ViewInteractionSelector::Focused,
            ViewPropertyKind::OutlineWidth,
            ViewPropertyValue::Milli(Milli::new(3_000)),
        )
        .unwrap();
    let mut styles = ViewStyleTable::default();
    styles.insert(StyleId(1), style).unwrap();
    let mut interaction = InteractionState::default();
    interaction.set_focus(FocusState::new(layer, target));
    let resolved = display
        .resolve_interaction_styles(&semantics, &styles, &interaction)
        .unwrap();
    let plan = ViewPaintPlan::from_resolved_display(&resolved);

    assert_eq!(plan.rectangles().len(), 5);
    assert_eq!(
        plan.rectangles()[0].bounds,
        HitRect::new(10.0, 20.0, 100.0, 40.0)
    );
    assert!((plan.rectangles()[1].bounds.height - 3.0).abs() < f32::EPSILON);
}
