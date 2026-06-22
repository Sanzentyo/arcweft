use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::hover::HoverPath;
use arcweft_presentation::input::{InputEpoch, InputEvent, InteractionTarget, PointerId};
use arcweft_presentation::interaction::{FocusState, InteractionState, PressedTarget};
use arcweft_presentation::layer::LayerId;
use arcweft_presentation::semantic::SemanticRole;
use arcweft_ui::{
    EventBinding, EventKind, FragmentKind, HandlerId, LayoutBox, LayoutLength, LayoutPoint,
    LayoutResults, LayoutSize, LayoutTree, Milli, NodeKey, Rgba8, RichTextSourceId, SemanticSpecId,
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

fn fragment_and_layout() -> (arcweft_ui::ViewFragment, LayoutResults) {
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
    ui: &LayerId,
    button: &InteractionTarget,
    enabled: bool,
) -> arcweft_ui::UiSemanticFragment {
    let mut semantics = UiSemanticFragmentBuilder::default();
    semantics
        .push(
            UiSemanticNode::new(
                NodeKey(1),
                ui.clone(),
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

fn interaction_styles() -> UiStyleTable {
    let idle = Rgba8::new(20, 30, 40, 255);
    let hovered = Rgba8::new(40, 80, 140, 255);
    let pressed = Rgba8::new(80, 120, 180, 255);
    let disabled = Rgba8::new(30, 30, 30, 180);
    let mut style = UiStyle::default();
    style
        .set_base(
            UiPropertyKind::BackgroundColor,
            UiPropertyValue::Color(idle),
        )
        .unwrap();
    style
        .set_rule(
            UiInteractionSelector::Hovered,
            UiPropertyKind::BackgroundColor,
            UiPropertyValue::Color(hovered),
        )
        .unwrap();
    style
        .set_rule(
            UiInteractionSelector::Focused,
            UiPropertyKind::OutlineColor,
            UiPropertyValue::Color(Rgba8::new(120, 210, 255, 255)),
        )
        .unwrap();
    style
        .set_rule(
            UiInteractionSelector::Focused,
            UiPropertyKind::OutlineWidth,
            UiPropertyValue::Milli(Milli::new(3_000)),
        )
        .unwrap();
    style
        .set_rule(
            UiInteractionSelector::Pressed,
            UiPropertyKind::BackgroundColor,
            UiPropertyValue::Color(pressed),
        )
        .unwrap();
    style
        .set_rule(
            UiInteractionSelector::Pressed,
            UiPropertyKind::Scale,
            UiPropertyValue::Milli(Milli::new(970)),
        )
        .unwrap();
    style
        .set_rule(
            UiInteractionSelector::Disabled,
            UiPropertyKind::BackgroundColor,
            UiPropertyValue::Color(disabled),
        )
        .unwrap();
    let mut styles = UiStyleTable::default();
    styles.insert(StyleId(7), style).unwrap();
    styles
}

fn output(enabled: bool) -> (UiLayerOutput, LayerId, InteractionTarget) {
    let ui = layer("ui");
    let button = target("button.confirm");
    let (fragment, layouts) = fragment_and_layout();
    let semantics = semantic_fragment(&ui, &button, enabled);
    let styles = interaction_styles();

    (
        UiLayerOutput::from_fragment_with_styles(&fragment, &layouts, semantics, styles).unwrap(),
        ui,
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
    let (output, ui, button) = output(true);
    let mut interaction = InteractionState::default();
    let _ = interaction.set_hover_path(HoverPath::new(PointerId(0), vec![button.clone()]));
    interaction.set_focus(FocusState::new(ui.clone(), button.clone()));
    interaction.press_pointer(PressedTarget::new(PointerId(0), ui, button));

    let resolved = output
        .display()
        .resolve_interaction_styles(output.semantics(), output.styles(), &interaction)
        .unwrap();
    let style = resolved.as_slice()[0].style();
    assert_eq!(
        style.color(UiPropertyKind::BackgroundColor),
        Some(Rgba8::new(80, 120, 180, 255))
    );
    assert_eq!(style.scale(), Milli::new(970));
    assert_eq!(
        style.color(UiPropertyKind::OutlineColor),
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
            .color(UiPropertyKind::BackgroundColor),
        Some(Rgba8::new(30, 30, 30, 180))
    );
}
