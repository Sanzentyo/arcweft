use crate::{
    ContainerKind, CustomElementId, DisplayItemKind, DisplayList, EntityStore, FragmentKind,
    ImageId, LayoutBox, LayoutLength, LayoutPoint, LayoutResults, LayoutSize, LayoutTree, NodeKey,
    RichTextSourceId, SemanticSpecId, TextSourceId, ViewError, ViewFragmentBuilder,
    ViewLayerOutput, ViewRegistryId, ViewSemanticFragmentBuilder, ViewSemanticNode,
};
use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::layer::LayerId;
use arcweft_presentation::semantic::SemanticRole;

#[derive(Debug, Eq, PartialEq)]
struct DialogueSkinState {
    hovered_nameplate: bool,
}

fn public_id(value: &str) -> PublicId {
    PublicId::try_new(value).unwrap()
}

fn registry_id(index: usize) -> ViewRegistryId {
    ViewRegistryId::try_from_index(index).unwrap()
}

#[test]
fn display_list_emits_laid_out_paint_nodes_in_fragment_order() {
    let mut entities = EntityStore::default();
    let view = entities
        .insert(
            DialogueSkinState {
                hovered_nameplate: false,
            },
            Some(registry_id(1)),
        )
        .unwrap();
    let mut builder = ViewFragmentBuilder::default();
    let text = builder
        .push_node(
            NodeKey(1),
            FragmentKind::Text(TextSourceId(1)),
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
    let rich_text = builder
        .push_node(
            NodeKey(2),
            FragmentKind::RichText(RichTextSourceId(2)),
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
    let image = builder
        .push_node(
            NodeKey(3),
            FragmentKind::Image(ImageId(3)),
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
    let mounted = builder
        .push_node(
            NodeKey(4),
            FragmentKind::View(view.raw()),
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
    let custom = builder
        .push_node(
            NodeKey(5),
            FragmentKind::Custom(CustomElementId(4)),
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
    let root = builder
        .push_node(
            NodeKey(6),
            FragmentKind::Container(ContainerKind::Stack),
            &[],
            &[text, rich_text, image, mounted, custom],
            &[],
            None,
        )
        .unwrap();
    let fragment = builder.finish();
    let tree = LayoutTree::from_fragment(&fragment).unwrap();
    let mut layouts = LayoutResults::new(&tree);
    for node in [text, rich_text, image, mounted, custom, root] {
        let x = i32::try_from(node.0).unwrap();
        layouts
            .set(
                node,
                LayoutBox::new(
                    LayoutPoint::new(LayoutLength::px(x), LayoutLength::px(0)),
                    LayoutSize::new(LayoutLength::px(10), LayoutLength::px(10)),
                ),
            )
            .unwrap();
    }

    let display = DisplayList::from_fragment(&fragment, &layouts).unwrap();
    let items = display.as_slice();
    assert_eq!(items.len(), 4);
    assert_eq!(items[0].node(), text);
    assert_eq!(items[0].kind(), DisplayItemKind::Text(TextSourceId(1)));
    assert_eq!(items[1].node(), rich_text);
    assert_eq!(
        items[1].kind(),
        DisplayItemKind::RichText(RichTextSourceId(2))
    );
    assert_eq!(items[2].node(), image);
    assert_eq!(items[2].kind(), DisplayItemKind::Image(ImageId(3)));
    assert_eq!(items[3].node(), custom);
    assert_eq!(items[3].kind(), DisplayItemKind::Custom(CustomElementId(4)));
}

#[test]
fn display_list_requires_layout_for_paint_nodes_only() {
    let mut builder = ViewFragmentBuilder::default();
    let text = builder
        .push_node(
            NodeKey(1),
            FragmentKind::Text(TextSourceId(1)),
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
    let root = builder
        .push_node(
            NodeKey(2),
            FragmentKind::Container(ContainerKind::Block),
            &[],
            &[text],
            &[],
            None,
        )
        .unwrap();
    let fragment = builder.finish();
    let tree = LayoutTree::from_fragment(&fragment).unwrap();
    let mut layouts = LayoutResults::new(&tree);
    layouts
        .set(
            root,
            LayoutBox::new(
                LayoutPoint::new(LayoutLength::px(0), LayoutLength::px(0)),
                LayoutSize::new(LayoutLength::px(100), LayoutLength::px(20)),
            ),
        )
        .unwrap();

    assert_eq!(
        DisplayList::from_fragment(&fragment, &layouts),
        Err(ViewError::MissingLayout(text))
    );
}

#[test]
fn view_layer_output_pairs_display_list_and_semantics_for_frame_commit() {
    let view_layer = LayerId::new(public_id("layer.view"));
    let button = InteractionTarget::new(public_id("target.view.confirm"));
    let action = public_id("action.confirm");
    let mut fragment_builder = ViewFragmentBuilder::default();
    let rich_text = fragment_builder
        .push_node(
            NodeKey(1),
            FragmentKind::RichText(RichTextSourceId(1)),
            &[],
            &[],
            &[],
            Some(SemanticSpecId(1)),
        )
        .unwrap();
    let root = fragment_builder
        .push_node(
            NodeKey(2),
            FragmentKind::Container(ContainerKind::Block),
            &[],
            &[rich_text],
            &[],
            None,
        )
        .unwrap();
    let fragment = fragment_builder.finish();
    let tree = LayoutTree::from_fragment(&fragment).unwrap();
    let mut layouts = LayoutResults::new(&tree);
    for node in [rich_text, root] {
        layouts
            .set(
                node,
                LayoutBox::new(
                    LayoutPoint::new(LayoutLength::px(0), LayoutLength::px(0)),
                    LayoutSize::new(LayoutLength::px(120), LayoutLength::px(24)),
                ),
            )
            .unwrap();
    }

    let mut semantic_builder = ViewSemanticFragmentBuilder::default();
    semantic_builder
        .push(
            ViewSemanticNode::new(
                NodeKey(1),
                view_layer,
                button.clone(),
                SemanticRole::Button,
                HitRect::new(0.0, 0.0, 120.0, 24.0),
            )
            .with_label("Confirm")
            .with_action(action),
        )
        .unwrap();

    let output =
        ViewLayerOutput::from_fragment(&fragment, &layouts, semantic_builder.finish()).unwrap();
    assert_eq!(output.display().as_slice().len(), 1);
    assert_eq!(
        output.display().as_slice()[0].kind(),
        DisplayItemKind::RichText(RichTextSourceId(1))
    );
    assert_eq!(output.semantics().as_slice().len(), 1);
    assert_eq!(output.semantics().as_slice()[0].target(), &button);
    assert_eq!(output.semantics().as_slice()[0].label(), Some("Confirm"));
}

#[test]
fn fragment_primitive_style_targets_keep_text_identity_with_interactive_semantics() {
    use crate::{
        ViewLengthMilli, ViewPropertyKind, ViewSpecifiedValue, ViewStyleApplicationTarget,
        ViewStyleAssignOp, ViewStyleDeclaration, ViewStyleProgram, ViewStyleResolver,
        ViewStyleRevisionSet, ViewStyleRule, ViewStyleSelector, ViewStyleSelectorSequence,
        ViewStyleSheet, ViewStyleSheetId, ViewStyleSourceId, ViewStyleTargetKind,
    };
    use arcweft_presentation::{
        appearance::PresentationEnvironment,
        interaction::{FocusState, InteractionState},
    };
    let length = |milli| ViewSpecifiedValue::Length {
        value: ViewLengthMilli::new(milli),
    };
    let rule = |target, order, property, milli| {
        ViewStyleRule::new(
            ViewStyleSelector::new(vec![
                ViewStyleSelectorSequence::new(None, Some(target), None, Vec::new()).unwrap(),
            ])
            .unwrap(),
            None,
            vec![
                ViewStyleDeclaration::new(
                    property,
                    length(milli),
                    ViewStyleAssignOp::Replace,
                    ViewStyleSourceId::new(order),
                )
                .unwrap(),
            ],
            order,
            ViewStyleSourceId::new(order),
        )
        .unwrap()
    };
    let focused_rule = ViewStyleRule::new(
        ViewStyleSelector::new(vec![
            ViewStyleSelectorSequence::new(
                None,
                Some(ViewStyleTargetKind::Text),
                None,
                vec![crate::ViewStylePredicate::Interaction(
                    crate::ViewInteractionSelector::Focused,
                )],
            )
            .unwrap(),
        ])
        .unwrap(),
        None,
        vec![
            ViewStyleDeclaration::new(
                ViewPropertyKind::FontSize,
                length(48_000),
                ViewStyleAssignOp::Replace,
                ViewStyleSourceId::new(3),
            )
            .unwrap(),
        ],
        3,
        ViewStyleSourceId::new(3),
    )
    .unwrap();
    let sheet_id = ViewStyleSheetId::try_new("style.fragment-text").unwrap();
    let program = ViewStyleProgram::try_new(
        vec![
            ViewStyleSheet::new(
                sheet_id.clone(),
                Vec::new(),
                vec![
                    rule(
                        ViewStyleTargetKind::Text,
                        0,
                        ViewPropertyKind::FontSize,
                        42_000,
                    ),
                    rule(
                        ViewStyleTargetKind::RichText,
                        1,
                        ViewPropertyKind::LineHeight,
                        56_700,
                    ),
                    rule(
                        ViewStyleTargetKind::Element(crate::ViewElementKind::Button),
                        2,
                        ViewPropertyKind::FontSize,
                        18_000,
                    ),
                    focused_rule,
                ],
            )
            .unwrap(),
        ],
        Vec::new(),
    )
    .unwrap();
    let applications = [ViewStyleApplicationTarget::named(sheet_id)];
    let mut builder = ViewFragmentBuilder::default();
    builder
        .push_node(
            NodeKey(1),
            FragmentKind::Text(TextSourceId(1)),
            &applications,
            &[],
            &[],
            Some(SemanticSpecId(0)),
        )
        .unwrap();
    builder
        .push_node(
            NodeKey(2),
            FragmentKind::RichText(RichTextSourceId(2)),
            &applications,
            &[],
            &[],
            Some(SemanticSpecId(1)),
        )
        .unwrap();
    let fragment = builder.finish();
    let mut layouts = LayoutResults::new(&LayoutTree::from_fragment(&fragment).unwrap());
    for node in [crate::NodeId(0), crate::NodeId(1)] {
        layouts
            .set(
                node,
                LayoutBox::new(
                    LayoutPoint::new(LayoutLength::px(0), LayoutLength::px(0)),
                    LayoutSize::new(LayoutLength::px(720), LayoutLength::px(84)),
                ),
            )
            .unwrap();
    }
    let mut semantic_builder = ViewSemanticFragmentBuilder::default();
    for (key, target) in [
        (NodeKey(1), "target.style.plain"),
        (NodeKey(2), "target.style.rich"),
    ] {
        semantic_builder
            .push(ViewSemanticNode::new(
                key,
                LayerId::new(public_id("layer.style.text")),
                InteractionTarget::new(public_id(target)),
                SemanticRole::Button,
                HitRect::new(0.0, 0.0, 720.0, 84.0),
            ))
            .unwrap();
    }
    let semantics = semantic_builder.finish();
    let display = DisplayList::from_fragment(&fragment, &layouts).unwrap();
    let resolved = display
        .resolve_styles(
            &semantics,
            &program,
            &InteractionState::default(),
            &PresentationEnvironment::ENGINE_DEFAULT,
            ViewStyleRevisionSet::default(),
            &mut ViewStyleResolver::default(),
        )
        .unwrap();
    assert_eq!(resolved.as_slice().len(), 2);
    assert_eq!(
        resolved.as_slice()[0]
            .style()
            .value(ViewPropertyKind::FontSize),
        Some(&length(42_000))
    );
    assert_eq!(
        resolved.as_slice()[0]
            .style()
            .value(ViewPropertyKind::LineHeight),
        None
    );
    assert_eq!(
        resolved.as_slice()[1]
            .style()
            .value(ViewPropertyKind::FontSize),
        Some(&length(42_000))
    );
    assert_eq!(
        resolved.as_slice()[1]
            .style()
            .value(ViewPropertyKind::LineHeight),
        Some(&length(56_700))
    );
    let mut focused = InteractionState::default();
    focused.set_focus(FocusState::new(
        LayerId::new(public_id("layer.style.text")),
        InteractionTarget::new(public_id("target.style.rich")),
    ));
    let focused = display
        .resolve_styles(
            &semantics,
            &program,
            &focused,
            &PresentationEnvironment::ENGINE_DEFAULT,
            ViewStyleRevisionSet::default(),
            &mut ViewStyleResolver::default(),
        )
        .unwrap();
    assert_eq!(
        focused.as_slice()[0]
            .style()
            .value(ViewPropertyKind::FontSize),
        Some(&length(42_000))
    );
    assert_eq!(
        focused.as_slice()[1]
            .style()
            .value(ViewPropertyKind::FontSize),
        Some(&length(48_000))
    );
    assert_eq!(
        focused.as_slice()[1]
            .style()
            .value(ViewPropertyKind::LineHeight),
        Some(&length(56_700))
    );
}
