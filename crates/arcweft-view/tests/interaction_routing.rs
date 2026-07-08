use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::hover::HoverPath;
use arcweft_presentation::input::{InputEpoch, InputEvent, InteractionTarget, PointerId};
use arcweft_presentation::interaction::{FocusState, InteractionState, PressedTarget};
use arcweft_presentation::layer::LayerId;
use arcweft_presentation::semantic::SemanticRole;
use arcweft_view::{
    EventBinding, EventKind, FragmentKind, HandlerId, LayoutBox, LayoutLength, LayoutPoint,
    LayoutResults, LayoutSize, LayoutTree, Milli, NodeKey, Rgba8, RichTextSourceId, SemanticSpecId,
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

fn fragment_and_layout() -> (arcweft_view::ViewFragment, LayoutResults) {
    let mut fragment = ViewFragmentBuilder::default();
    let node = fragment
        .push_node(
            NodeKey(1),
            FragmentKind::RichText(RichTextSourceId(1)),
            StyleId(7),
            &[],
            &[EventBinding::new(EventKind::Activate, HandlerId(11))],
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
                LayoutPoint::new(LayoutLength::px(20), LayoutLength::px(30)),
                LayoutSize::new(LayoutLength::px(160), LayoutLength::px(48)),
            ),
        )
        .unwrap();
    (fragment, layouts)
}

fn semantic_fragment(
    view: &LayerId,
    button: &InteractionTarget,
    enabled: bool,
) -> arcweft_view::ViewSemanticFragment {
    let mut semantics = ViewSemanticFragmentBuilder::default();
    semantics
        .push(
            ViewSemanticNode::new(
                NodeKey(1),
                view.clone(),
                button.clone(),
                SemanticRole::Button,
                HitRect::new(20.0, 30.0, 160.0, 48.0),
            )
            .with_label("Confirm")
            .with_enabled(enabled)
            .with_action(public_id("action.confirm")),
        )
        .unwrap();
    semantics.finish()
}

fn interaction_styles() -> ViewStyleTable {
    let idle = Rgba8::new(20, 30, 40, 255);
    let hovered = Rgba8::new(40, 80, 140, 255);
    let pressed = Rgba8::new(80, 120, 180, 255);
    let disabled = Rgba8::new(30, 30, 30, 180);
    let mut style = ViewStyle::default();
    style
        .set_base(
            ViewPropertyKind::BackgroundColor,
            ViewPropertyValue::Color(idle),
        )
        .unwrap();
    style
        .set_rule(
            ViewInteractionSelector::Hovered,
            ViewPropertyKind::BackgroundColor,
            ViewPropertyValue::Color(hovered),
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
    style
        .set_rule(
            ViewInteractionSelector::Pressed,
            ViewPropertyKind::BackgroundColor,
            ViewPropertyValue::Color(pressed),
        )
        .unwrap();
    style
        .set_rule(
            ViewInteractionSelector::Pressed,
            ViewPropertyKind::Scale,
            ViewPropertyValue::Milli(Milli::new(970)),
        )
        .unwrap();
    style
        .set_rule(
            ViewInteractionSelector::Disabled,
            ViewPropertyKind::BackgroundColor,
            ViewPropertyValue::Color(disabled),
        )
        .unwrap();
    let mut styles = ViewStyleTable::default();
    styles.insert(StyleId(7), style).unwrap();
    styles
}

fn output(enabled: bool) -> (ViewLayerOutput, LayerId, InteractionTarget) {
    let view = layer("view");
    let button = target("button.confirm");
    let (fragment, layouts) = fragment_and_layout();
    let semantics = semantic_fragment(&view, &button, enabled);
    let styles = interaction_styles();

    (
        ViewLayerOutput::from_fragment_with_styles(&fragment, &layouts, semantics, styles).unwrap(),
        view,
        button,
    )
}

#[test]
fn routed_activate_selects_handler_by_stable_target() {
    let (output, _, button) = output(true);
    let invocations = output
        .handlers()
        .dispatch_input(&InputEvent::activate(InputEpoch(4), button.clone()));

    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0].target(), &button);
    assert_eq!(invocations[0].event(), EventKind::Activate);
    assert_eq!(invocations[0].handler(), HandlerId(11));
}

#[test]
fn interaction_cascade_resolves_hover_focus_and_pressed_without_backend_matching() {
    let (output, view, button) = output(true);
    let mut interaction = InteractionState::default();
    let _ = interaction.set_hover_path(HoverPath::new(PointerId(0), vec![button.clone()]));
    interaction.set_focus(FocusState::new(view.clone(), button.clone()));
    interaction.press_pointer(PressedTarget::new(PointerId(0), view, button));

    let resolved = output
        .display()
        .resolve_interaction_styles(output.semantics(), output.styles(), &interaction)
        .unwrap();
    let style = resolved.as_slice()[0].style();
    assert_eq!(
        style.color(ViewPropertyKind::BackgroundColor),
        Some(Rgba8::new(80, 120, 180, 255))
    );
    assert_eq!(style.scale(), Milli::new(970));
    assert_eq!(
        style.color(ViewPropertyKind::OutlineColor),
        Some(Rgba8::new(120, 210, 255, 255))
    );
}

#[test]
fn disabled_rule_has_final_precedence() {
    let (output, _, _) = output(false);
    let resolved = output
        .display()
        .resolve_interaction_styles(
            output.semantics(),
            output.styles(),
            &InteractionState::default(),
        )
        .unwrap();
    assert_eq!(
        resolved.as_slice()[0]
            .style()
            .color(ViewPropertyKind::BackgroundColor),
        Some(Rgba8::new(30, 30, 30, 180))
    );
}
