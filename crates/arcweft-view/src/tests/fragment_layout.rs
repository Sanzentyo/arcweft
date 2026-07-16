use crate::{
    ContainerKind, CustomElementId, EntityStore, EventBinding, EventKind, FragmentKind, HandlerId,
    ImageId, LayoutBox, LayoutKind, LayoutLength, LayoutPoint, LayoutResults, LayoutSize,
    LayoutTree, NodeId, NodeKey, RichTextSourceId, SemanticSpecId, TextSourceId, ViewError,
    ViewFragmentBuilder, ViewRegistryId, ViewStyleApplicationTarget, ViewStylePatchId,
    ViewStyleSheetId,
};

#[derive(Debug, Eq, PartialEq)]
struct DialogueSkinState {
    hovered_nameplate: bool,
}

fn registry_id(index: usize) -> ViewRegistryId {
    ViewRegistryId::try_from_index(index).unwrap()
}

#[test]
fn view_fragment_keeps_text_media_view_and_custom_nodes_flat() {
    let mut entities = EntityStore::default();
    let view_state = entities
        .insert(
            DialogueSkinState {
                hovered_nameplate: false,
            },
            Some(registry_id(4)),
        )
        .unwrap();

    let mut builder = ViewFragmentBuilder::default();
    let styles = [
        ViewStyleApplicationTarget::named(ViewStyleSheetId::try_new("style.dialogue").unwrap()),
        ViewStyleApplicationTarget::inline(ViewStylePatchId::new(3)),
    ];
    let rich_text = builder
        .push_node(
            NodeKey(10),
            FragmentKind::RichText(RichTextSourceId(1)),
            &styles,
            &[],
            &[EventBinding::new(EventKind::Activate, HandlerId(9))],
            Some(SemanticSpecId(1)),
        )
        .unwrap();
    let image = builder
        .push_node(
            NodeKey(11),
            FragmentKind::Image(ImageId(2)),
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
    let nested_view = builder
        .push_node(
            NodeKey(12),
            FragmentKind::View(view_state.raw()),
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
    let custom = builder
        .push_node(
            NodeKey(13),
            FragmentKind::Custom(CustomElementId(7)),
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
    let root = builder
        .push_node(
            NodeKey(14),
            FragmentKind::Container(ContainerKind::Stack),
            &[],
            &[rich_text, image, nested_view, custom],
            &[],
            None,
        )
        .unwrap();

    let fragment = builder.finish();
    assert_eq!(fragment.nodes().len(), 5);
    assert_eq!(
        fragment.node_children(root),
        Some([rich_text, image, nested_view, custom].as_slice())
    );
    assert_eq!(
        fragment.node_events(rich_text),
        Some([EventBinding::new(EventKind::Activate, HandlerId(9))].as_slice())
    );
    assert_eq!(
        fragment.node_style_applications(rich_text),
        Some(styles.as_slice())
    );
}

#[test]
fn view_fragment_rejects_duplicate_keys_and_missing_children() {
    let mut builder = ViewFragmentBuilder::default();
    builder
        .push_node(
            NodeKey(1),
            FragmentKind::Text(TextSourceId(1)),
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();

    assert_eq!(
        builder.push_node(
            NodeKey(1),
            FragmentKind::Text(TextSourceId(2)),
            &[],
            &[],
            &[],
            None
        ),
        Err(ViewError::DuplicateNodeKey(NodeKey(1)))
    );
    assert_eq!(
        builder.push_node(
            NodeKey(2),
            FragmentKind::Container(ContainerKind::Block),
            &[],
            &[NodeId(99)],
            &[],
            None
        ),
        Err(ViewError::InvalidFragmentNode(NodeId(99)))
    );
}

#[test]
fn layout_tree_preserves_fragment_node_order_and_child_counts() {
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
    let image = builder
        .push_node(
            NodeKey(2),
            FragmentKind::Image(ImageId(1)),
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
    let root = builder
        .push_node(
            NodeKey(3),
            FragmentKind::Container(ContainerKind::Block),
            &[],
            &[text, image],
            &[],
            None,
        )
        .unwrap();
    let fragment = builder.finish();

    let tree = LayoutTree::from_fragment(&fragment).unwrap();
    assert_eq!(tree.len(), 3);
    assert_eq!(tree.nodes()[text.0 as usize].node(), text);
    assert_eq!(tree.nodes()[text.0 as usize].kind(), LayoutKind::Text);
    assert_eq!(tree.nodes()[text.0 as usize].child_count(), 0);
    assert_eq!(tree.nodes()[image.0 as usize].kind(), LayoutKind::Image);
    assert_eq!(tree.nodes()[root.0 as usize].kind(), LayoutKind::Container);
    assert_eq!(tree.nodes()[root.0 as usize].child_count(), 2);
}

#[test]
fn layout_results_report_missing_and_invalid_nodes() {
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
    let tree = LayoutTree::from_fragment(&builder.finish()).unwrap();
    let mut results = LayoutResults::new(&tree);

    assert_eq!(results.require(text), Err(ViewError::MissingLayout(text)));
    let layout = LayoutBox::new(
        LayoutPoint::new(LayoutLength::px(4), LayoutLength::px(8)),
        LayoutSize::new(LayoutLength::px(120), LayoutLength::px(24)),
    );
    results.set(text, layout).unwrap();
    assert_eq!(results.require(text), Ok(layout));
    assert_eq!(
        results.set(NodeId(99), layout),
        Err(ViewError::InvalidFragmentNode(NodeId(99)))
    );
}
