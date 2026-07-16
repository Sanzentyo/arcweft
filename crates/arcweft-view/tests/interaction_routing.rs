use arcweft_id::PublicId;
use arcweft_presentation::appearance::{PresentationColor, PresentationEnvironment};
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::hover::HoverPath;
use arcweft_presentation::input::{InputEpoch, InputEvent, InteractionTarget, PointerId};
use arcweft_presentation::interaction::{FocusState, InteractionState, PressedTarget};
use arcweft_presentation::layer::LayerId;
use arcweft_presentation::semantic::SemanticRole;
use arcweft_view::{
    ContainerKind, EventBinding, EventKind, FragmentKind, HandlerId, LayoutBox, LayoutLength,
    LayoutPoint, LayoutResults, LayoutSize, LayoutTree, NodeKey, RichTextSourceId, SemanticSpecId,
    ViewColorValue, ViewElementKind, ViewFragmentBuilder, ViewInteractionSelector, ViewLayerOutput,
    ViewLengthMilli, ViewPropertyKind, ViewScalarMilli, ViewSemanticFragmentBuilder,
    ViewSemanticNode, ViewSpecifiedValue, ViewStyleApplicationTarget, ViewStyleAssignOp,
    ViewStyleCombinator, ViewStyleDeclaration, ViewStylePatch, ViewStylePatchId,
    ViewStylePredicate, ViewStyleProgram, ViewStyleResolver, ViewStyleRevisionSet, ViewStyleRule,
    ViewStyleSelector, ViewStyleSelectorSequence, ViewStyleSheet, ViewStyleSheetId,
    ViewStyleSourceId,
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
    let styles = [ViewStyleApplicationTarget::named(
        ViewStyleSheetId::try_new("style.interaction").unwrap(),
    )];
    let node = fragment
        .push_node(
            NodeKey(1),
            FragmentKind::RichText(RichTextSourceId(1)),
            &styles,
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

fn color(red: u8, green: u8, blue: u8, alpha: u8) -> ViewSpecifiedValue {
    ViewSpecifiedValue::Color {
        value: ViewColorValue::Literal {
            color: PresentationColor::rgba(red, green, blue, alpha),
        },
    }
}

fn style_rule(
    source_order: u32,
    state: Option<ViewInteractionSelector>,
    declarations: Vec<(ViewPropertyKind, ViewSpecifiedValue)>,
) -> ViewStyleRule {
    let selector = ViewStyleSelector::new(vec![
        ViewStyleSelectorSequence::new(
            None,
            Some(ViewElementKind::Button),
            None,
            state
                .map(ViewStylePredicate::Interaction)
                .into_iter()
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap();
    ViewStyleRule::new(
        selector,
        None,
        declarations
            .into_iter()
            .enumerate()
            .map(|(index, (property, value))| {
                ViewStyleDeclaration::new(
                    property,
                    value,
                    ViewStyleAssignOp::Replace,
                    ViewStyleSourceId::new(source_order * 10 + u32::try_from(index).unwrap()),
                )
                .unwrap()
            })
            .collect(),
        source_order,
        ViewStyleSourceId::new(source_order),
    )
    .unwrap()
}

fn interaction_styles() -> ViewStyleProgram {
    let rules = vec![
        style_rule(
            0,
            None,
            vec![(ViewPropertyKind::BackgroundColor, color(20, 30, 40, 255))],
        ),
        style_rule(
            10,
            Some(ViewInteractionSelector::Hovered),
            vec![(ViewPropertyKind::BackgroundColor, color(40, 80, 140, 255))],
        ),
        style_rule(
            20,
            Some(ViewInteractionSelector::Focused),
            vec![
                (ViewPropertyKind::OutlineColor, color(120, 210, 255, 255)),
                (
                    ViewPropertyKind::OutlineWidth,
                    ViewSpecifiedValue::Length {
                        value: ViewLengthMilli::new(3_000),
                    },
                ),
            ],
        ),
        style_rule(
            30,
            Some(ViewInteractionSelector::Pressed),
            vec![
                (ViewPropertyKind::BackgroundColor, color(80, 120, 180, 255)),
                (
                    ViewPropertyKind::Scale,
                    ViewSpecifiedValue::Scalar {
                        value: ViewScalarMilli::new(970),
                    },
                ),
            ],
        ),
        style_rule(
            40,
            Some(ViewInteractionSelector::Disabled),
            vec![(ViewPropertyKind::BackgroundColor, color(30, 30, 30, 180))],
        ),
    ];
    ViewStyleProgram::try_new(
        vec![
            ViewStyleSheet::new(
                ViewStyleSheetId::try_new("style.interaction").unwrap(),
                Vec::new(),
                rules,
            )
            .unwrap(),
        ],
        Vec::new(),
    )
    .unwrap()
}

fn output(enabled: bool) -> (ViewLayerOutput, LayerId, InteractionTarget) {
    let view = layer("view");
    let button = target("button.confirm");
    let (fragment, layouts) = fragment_and_layout();
    let semantics = semantic_fragment(&view, &button, enabled);
    let styles = interaction_styles();

    (
        ViewLayerOutput::from_fragment_with_style_program(&fragment, &layouts, semantics, styles)
            .unwrap(),
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
        .resolve_styles(
            output.semantics(),
            output.style_program(),
            &interaction,
            &PresentationEnvironment::ENGINE_DEFAULT,
            ViewStyleRevisionSet::default(),
            &mut ViewStyleResolver::default(),
        )
        .unwrap();
    let style = resolved.as_slice()[0].style();
    assert_eq!(
        style.value(ViewPropertyKind::BackgroundColor),
        Some(&color(80, 120, 180, 255))
    );
    assert_eq!(
        style.value(ViewPropertyKind::Scale),
        Some(&ViewSpecifiedValue::Scalar {
            value: ViewScalarMilli::new(970),
        })
    );
    assert_eq!(
        style.value(ViewPropertyKind::OutlineColor),
        Some(&color(120, 210, 255, 255))
    );
}

#[test]
fn disabled_rule_has_final_precedence() {
    let (output, _, _) = output(false);
    let resolved = output
        .display()
        .resolve_styles(
            output.semantics(),
            output.style_program(),
            &InteractionState::default(),
            &PresentationEnvironment::ENGINE_DEFAULT,
            ViewStyleRevisionSet::default(),
            &mut ViewStyleResolver::default(),
        )
        .unwrap();
    assert_eq!(
        resolved.as_slice()[0]
            .style()
            .value(ViewPropertyKind::BackgroundColor),
        Some(&color(30, 30, 30, 180))
    );
}

fn ancestry_fragment(
    sheet_id: &ViewStyleSheetId,
    patch_id: ViewStylePatchId,
) -> (arcweft_view::ViewFragment, LayoutResults) {
    let mut fragment = ViewFragmentBuilder::default();
    let child = fragment
        .push_node(
            NodeKey(11),
            FragmentKind::RichText(RichTextSourceId(2)),
            &[],
            &[],
            &[],
            Some(SemanticSpecId(0)),
        )
        .unwrap();
    let root_styles = [
        ViewStyleApplicationTarget::named(sheet_id.clone()),
        ViewStyleApplicationTarget::inline(patch_id),
    ];
    let _root = fragment
        .push_node(
            NodeKey(10),
            FragmentKind::Container(ContainerKind::Block),
            &root_styles,
            &[child],
            &[],
            None,
        )
        .unwrap();
    let fragment = fragment.finish();
    let tree = LayoutTree::from_fragment(&fragment).unwrap();
    let mut layouts = LayoutResults::new(&tree);
    layouts
        .set(
            child,
            LayoutBox::new(
                LayoutPoint::new(LayoutLength::px(0), LayoutLength::px(0)),
                LayoutSize::new(LayoutLength::px(100), LayoutLength::px(20)),
            ),
        )
        .unwrap();
    (fragment, layouts)
}

fn ancestry_program(sheet_id: ViewStyleSheetId, patch_id: ViewStylePatchId) -> ViewStyleProgram {
    let box_selector = ViewStyleSelector::new(vec![
        ViewStyleSelectorSequence::new(None, Some(ViewElementKind::Box), None, Vec::new()).unwrap(),
    ])
    .unwrap();
    let child_selector = ViewStyleSelector::new(vec![
        ViewStyleSelectorSequence::new(None, Some(ViewElementKind::Box), None, Vec::new()).unwrap(),
        ViewStyleSelectorSequence::new(
            Some(ViewStyleCombinator::Child),
            Some(ViewElementKind::Button),
            None,
            Vec::new(),
        )
        .unwrap(),
    ])
    .unwrap();
    let sheet = ViewStyleSheet::new(
        sheet_id,
        Vec::new(),
        vec![
            ViewStyleRule::new(
                box_selector,
                None,
                vec![
                    ViewStyleDeclaration::new(
                        ViewPropertyKind::Color,
                        color(10, 20, 30, 255),
                        ViewStyleAssignOp::Replace,
                        ViewStyleSourceId::new(100),
                    )
                    .unwrap(),
                ],
                0,
                ViewStyleSourceId::new(101),
            )
            .unwrap(),
            ViewStyleRule::new(
                child_selector,
                None,
                vec![
                    ViewStyleDeclaration::new(
                        ViewPropertyKind::BackgroundColor,
                        color(40, 50, 60, 255),
                        ViewStyleAssignOp::Replace,
                        ViewStyleSourceId::new(102),
                    )
                    .unwrap(),
                ],
                1,
                ViewStyleSourceId::new(103),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let patch = ViewStylePatch::new(
        patch_id,
        vec![
            ViewStyleDeclaration::new(
                ViewPropertyKind::Width,
                ViewSpecifiedValue::Length {
                    value: ViewLengthMilli::new(99_000),
                },
                ViewStyleAssignOp::Replace,
                ViewStyleSourceId::new(104),
            )
            .unwrap(),
        ],
    );
    ViewStyleProgram::try_new(vec![sheet], vec![patch]).unwrap()
}

#[test]
fn fragment_resolution_propagates_named_scope_and_inherits_only_parent_values() {
    let sheet_id = ViewStyleSheetId::try_new("style.ancestry").unwrap();
    let patch_id = ViewStylePatchId::new(5);
    let (fragment, layouts) = ancestry_fragment(&sheet_id, patch_id);
    let program = ancestry_program(sheet_id, patch_id);
    let semantics = semantic_fragment(&layer("ancestry"), &target("ancestry.child"), true);
    let output =
        ViewLayerOutput::from_fragment_with_style_program(&fragment, &layouts, semantics, program)
            .unwrap();
    let resolved = output
        .display()
        .resolve_styles(
            output.semantics(),
            output.style_program(),
            &InteractionState::default(),
            &PresentationEnvironment::ENGINE_DEFAULT,
            ViewStyleRevisionSet::default(),
            &mut ViewStyleResolver::default(),
        )
        .unwrap();
    let computed = resolved.as_slice()[0].style();

    assert_eq!(
        computed.value(ViewPropertyKind::Color),
        Some(&color(10, 20, 30, 255))
    );
    assert_eq!(
        computed.value(ViewPropertyKind::BackgroundColor),
        Some(&color(40, 50, 60, 255))
    );
    assert_eq!(computed.value(ViewPropertyKind::Width), None);
}
