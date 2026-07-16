use arcweft_id::PublicId;
use arcweft_presentation::appearance::{PresentationColor, PresentationEnvironment};
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
    LayoutResults, LayoutSize, LayoutTree, NodeKey, RichTextSourceId, SemanticSpecId,
    ViewColorValue, ViewElementKind, ViewFragmentBuilder, ViewInteractionSelector, ViewLayerOutput,
    ViewPropertyKind, ViewSemanticFragmentBuilder, ViewSemanticNode, ViewSpecifiedValue,
    ViewStyleApplicationTarget, ViewStyleAssignOp, ViewStyleDeclaration, ViewStylePredicate,
    ViewStyleProgram, ViewStyleResolver, ViewStyleRevisionSet, ViewStyleRule, ViewStyleSelector,
    ViewStyleSelectorSequence, ViewStyleSheet, ViewStyleSheetId, ViewStyleSourceId,
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

fn color(red: u8, green: u8, blue: u8) -> ViewSpecifiedValue {
    ViewSpecifiedValue::Color {
        value: ViewColorValue::Literal {
            color: PresentationColor::rgb(red, green, blue),
        },
    }
}

fn style_rule(
    source_order: u32,
    state: Option<ViewInteractionSelector>,
    value: ViewSpecifiedValue,
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
        vec![
            ViewStyleDeclaration::new(
                ViewPropertyKind::BackgroundColor,
                value,
                ViewStyleAssignOp::Replace,
                ViewStyleSourceId::new(source_order),
            )
            .unwrap(),
        ],
        source_order,
        ViewStyleSourceId::new(source_order + 10),
    )
    .unwrap()
}

fn interaction_style_program(sheet_id: ViewStyleSheetId) -> ViewStyleProgram {
    ViewStyleProgram::try_new(
        vec![
            ViewStyleSheet::new(
                sheet_id,
                Vec::new(),
                vec![
                    style_rule(0, None, color(10, 20, 30)),
                    style_rule(
                        1,
                        Some(ViewInteractionSelector::Hovered),
                        color(40, 80, 120),
                    ),
                ],
            )
            .unwrap(),
        ],
        Vec::new(),
    )
    .unwrap()
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
    let sheet_id = ViewStyleSheetId::try_new("style.runtime_host").unwrap();
    let styles = [ViewStyleApplicationTarget::named(sheet_id.clone())];
    let node = fragment
        .push_node(
            NodeKey(1),
            FragmentKind::RichText(RichTextSourceId(1)),
            &styles,
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
    let output = ViewLayerOutput::from_fragment_with_style_program(
        &fragment,
        &layouts,
        semantics.finish(),
        interaction_style_program(sheet_id),
    )
    .unwrap();

    let mut builder = ViewFrameCommitBuilder::new(&layers);
    builder.push_layer(view.clone(), output).unwrap();
    let commit = builder.finish();
    let input = InputEvent::activate(InputEpoch(2), button.clone());
    assert_eq!(commit.dispatch_input(&input)[0].handler(), HandlerId(5));

    let mut interaction = InteractionState::default();
    let _ = interaction.set_hover_path(HoverPath::new(PointerId(0), vec![button]));
    let resolved = commit
        .resolve_styles(
            &interaction,
            &PresentationEnvironment::ENGINE_DEFAULT,
            ViewStyleRevisionSet::default(),
            &mut ViewStyleResolver::default(),
        )
        .unwrap();
    assert_eq!(resolved[0].layer(), &view);
    assert_eq!(
        resolved[0].display().as_slice()[0]
            .style()
            .value(ViewPropertyKind::BackgroundColor),
        Some(&color(40, 80, 120))
    );
}
