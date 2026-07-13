use crate::{
    ContainerKind, CustomElementId, DisplayItemKind, DisplayList, EntityStore, FragmentKind,
    ImageId, LayoutBox, LayoutLength, LayoutPoint, LayoutResults, LayoutSize, LayoutTree, NodeKey,
    RichTextSourceId, SemanticSpecId, TextSourceId, ViewError, ViewFragmentBuilder, ViewId,
    ViewLayerOutput, ViewSemanticFragmentBuilder, ViewSemanticNode,
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

#[test]
fn display_list_emits_laid_out_paint_nodes_in_fragment_order() {
    let mut entities = EntityStore::default();
    let view = entities
        .insert(
            DialogueSkinState {
                hovered_nameplate: false,
            },
            Some(ViewId(1)),
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
