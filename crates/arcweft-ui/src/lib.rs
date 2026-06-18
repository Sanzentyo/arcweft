//! Sans I/O UI component, entity, and fragment data for Arcweft presentation.

pub mod component;
pub mod display;
pub mod entity;
pub mod fragment;
pub mod layout;
pub mod reactive;
pub mod semantics;
pub mod style;

use thiserror::Error;

pub use component::{
    ComponentDescriptor, ComponentId, ComponentImplementation, ComponentRegistry,
    ComponentSchemaId, RustComponentId, UiProgramId,
};
pub use display::{DisplayItem, DisplayItemId, DisplayItemKind, DisplayList};
pub use entity::{DirtyFlags, Entity, EntityStore, RawEntity};
pub use fragment::{
    ContainerKind, CustomElementId, EventBinding, EventKind, FragmentKind, FragmentNode, HandlerId,
    ImageId, NodeId, RichTextSourceId, SemanticSpecId, Span32, StyleId, TextSourceId, ViewFragment,
    ViewFragmentBuilder,
};
pub use layout::{
    LayoutBox, LayoutKind, LayoutLength, LayoutNode, LayoutPoint, LayoutResults, LayoutSize,
    LayoutTree,
};
pub use reactive::{EntityInvalidation, ReactiveGraph, ReactiveInvalidation, Revision};
pub use semantics::{UiNodeId, UiSemanticFragment, UiSemanticFragmentBuilder, UiSemanticNode};
pub use style::{
    Invalidation, Milli, PropertyBinding, PropertyBindingTable, PropertyBindingTableBuilder, Rgba8,
    UiPropertyId, UiPropertyKind, UiPropertyValue, ValueSourceId,
};

/// Stable key for one retained UI fragment node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeKey(pub u64);

/// Error while building or updating UI state.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum UiError {
    #[error("duplicate UI node key {0:?}")]
    DuplicateNodeKey(NodeKey),
    #[error("duplicate component public id {0}")]
    DuplicateComponentPublicId(arcweft_id::PublicId),
    #[error("stale UI entity {0:?}")]
    StaleEntity(RawEntity),
    #[error("UI entity has a different state type: {0:?}")]
    EntityTypeMismatch(RawEntity),
    #[error("invalid UI fragment node {0:?}")]
    InvalidFragmentNode(NodeId),
    #[error("missing layout for UI fragment node {0:?}")]
    MissingLayout(NodeId),
    #[error("duplicate UI property binding {0:?}")]
    DuplicatePropertyBinding(UiPropertyId),
    #[error("too many UI items")]
    CapacityExceeded,
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_id::PublicId;
    use arcweft_presentation::hit::HitRect;
    use arcweft_presentation::input::{ActionTarget, InputEpoch, InteractionTarget};
    use arcweft_presentation::interaction::InteractionState;
    use arcweft_presentation::layer::{
        LayerId, LayerInputPolicy, LayerKind, LayerNode, LayerOrder, LayerTree, RenderPhase,
    };
    use arcweft_presentation::semantic::SemanticRole;

    #[derive(Debug, Eq, PartialEq)]
    struct DialogueSkinState {
        hovered_nameplate: bool,
    }

    #[derive(Debug, Eq, PartialEq)]
    struct InventoryState {
        selected_slot: u8,
    }

    fn public_id(value: &str) -> PublicId {
        PublicId::try_new(value).unwrap()
    }

    fn layer_id(name: &str) -> LayerId {
        LayerId::new(public_id(&format!("layer.{name}")))
    }

    fn target(name: &str) -> InteractionTarget {
        InteractionTarget::new(public_id(&format!("target.{name}")))
    }

    fn order(phase: RenderPhase, z: i32) -> LayerOrder {
        LayerOrder {
            phase,
            z,
            stable_index: 0,
        }
    }

    #[test]
    fn view_fragment_keeps_text_media_component_and_custom_nodes_flat() {
        let mut entities = EntityStore::default();
        let component_state = entities
            .insert(
                DialogueSkinState {
                    hovered_nameplate: false,
                },
                Some(ComponentId(4)),
            )
            .unwrap();

        let mut builder = ViewFragmentBuilder::default();
        let rich_text = builder
            .push_node(
                NodeKey(10),
                FragmentKind::RichText(RichTextSourceId(1)),
                StyleId(1),
                &[],
                &[EventBinding::new(EventKind::Activate, HandlerId(9))],
                Some(SemanticSpecId(1)),
            )
            .unwrap();
        let image = builder
            .push_node(
                NodeKey(11),
                FragmentKind::Image(ImageId(2)),
                StyleId(2),
                &[],
                &[],
                None,
            )
            .unwrap();
        let nested_component = builder
            .push_node(
                NodeKey(12),
                FragmentKind::Component(component_state.raw()),
                StyleId(3),
                &[],
                &[],
                None,
            )
            .unwrap();
        let custom = builder
            .push_node(
                NodeKey(13),
                FragmentKind::Custom(CustomElementId(7)),
                StyleId(4),
                &[],
                &[],
                None,
            )
            .unwrap();
        let root = builder
            .push_node(
                NodeKey(14),
                FragmentKind::Container(ContainerKind::Stack),
                StyleId(5),
                &[rich_text, image, nested_component, custom],
                &[],
                None,
            )
            .unwrap();

        let fragment = builder.finish();
        assert_eq!(fragment.nodes().len(), 5);
        assert_eq!(
            fragment.node_children(root),
            Some([rich_text, image, nested_component, custom].as_slice())
        );
        assert_eq!(
            fragment.node_events(rich_text),
            Some([EventBinding::new(EventKind::Activate, HandlerId(9))].as_slice())
        );
        assert_eq!(fragment.nodes()[rich_text.0 as usize].style(), StyleId(1));
    }

    #[test]
    fn view_fragment_rejects_duplicate_keys_and_missing_children() {
        let mut builder = ViewFragmentBuilder::default();
        builder
            .push_node(
                NodeKey(1),
                FragmentKind::Text(TextSourceId(1)),
                StyleId(1),
                &[],
                &[],
                None,
            )
            .unwrap();

        assert_eq!(
            builder.push_node(
                NodeKey(1),
                FragmentKind::Text(TextSourceId(2)),
                StyleId(1),
                &[],
                &[],
                None
            ),
            Err(UiError::DuplicateNodeKey(NodeKey(1)))
        );
        assert_eq!(
            builder.push_node(
                NodeKey(2),
                FragmentKind::Container(ContainerKind::Block),
                StyleId(1),
                &[NodeId(99)],
                &[],
                None
            ),
            Err(UiError::InvalidFragmentNode(NodeId(99)))
        );
    }

    #[test]
    fn layout_tree_preserves_fragment_node_order_and_child_counts() {
        let mut builder = ViewFragmentBuilder::default();
        let text = builder
            .push_node(
                NodeKey(1),
                FragmentKind::Text(TextSourceId(1)),
                StyleId(1),
                &[],
                &[],
                None,
            )
            .unwrap();
        let image = builder
            .push_node(
                NodeKey(2),
                FragmentKind::Image(ImageId(1)),
                StyleId(1),
                &[],
                &[],
                None,
            )
            .unwrap();
        let root = builder
            .push_node(
                NodeKey(3),
                FragmentKind::Container(ContainerKind::Block),
                StyleId(1),
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
                StyleId(1),
                &[],
                &[],
                None,
            )
            .unwrap();
        let tree = LayoutTree::from_fragment(&builder.finish()).unwrap();
        let mut results = LayoutResults::new(&tree);

        assert_eq!(results.require(text), Err(UiError::MissingLayout(text)));
        let layout = LayoutBox::new(
            LayoutPoint::new(LayoutLength::px(4), LayoutLength::px(8)),
            LayoutSize::new(LayoutLength::px(120), LayoutLength::px(24)),
        );
        results.set(text, layout).unwrap();
        assert_eq!(results.require(text), Ok(layout));
        assert_eq!(
            results.set(NodeId(99), layout),
            Err(UiError::InvalidFragmentNode(NodeId(99)))
        );
    }

    #[test]
    fn display_list_emits_laid_out_paint_nodes_in_fragment_order() {
        let mut entities = EntityStore::default();
        let component = entities
            .insert(
                DialogueSkinState {
                    hovered_nameplate: false,
                },
                Some(ComponentId(1)),
            )
            .unwrap();
        let mut builder = ViewFragmentBuilder::default();
        let text = builder
            .push_node(
                NodeKey(1),
                FragmentKind::Text(TextSourceId(1)),
                StyleId(1),
                &[],
                &[],
                None,
            )
            .unwrap();
        let rich_text = builder
            .push_node(
                NodeKey(2),
                FragmentKind::RichText(RichTextSourceId(2)),
                StyleId(1),
                &[],
                &[],
                None,
            )
            .unwrap();
        let image = builder
            .push_node(
                NodeKey(3),
                FragmentKind::Image(ImageId(3)),
                StyleId(1),
                &[],
                &[],
                None,
            )
            .unwrap();
        let mounted = builder
            .push_node(
                NodeKey(4),
                FragmentKind::Component(component.raw()),
                StyleId(1),
                &[],
                &[],
                None,
            )
            .unwrap();
        let custom = builder
            .push_node(
                NodeKey(5),
                FragmentKind::Custom(CustomElementId(4)),
                StyleId(1),
                &[],
                &[],
                None,
            )
            .unwrap();
        let root = builder
            .push_node(
                NodeKey(6),
                FragmentKind::Container(ContainerKind::Stack),
                StyleId(1),
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
                StyleId(1),
                &[],
                &[],
                None,
            )
            .unwrap();
        let root = builder
            .push_node(
                NodeKey(2),
                FragmentKind::Container(ContainerKind::Block),
                StyleId(1),
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
            Err(UiError::MissingLayout(text))
        );
    }

    #[test]
    fn component_registry_resolves_dense_component_ids() {
        let mut registry = ComponentRegistry::default();
        let public_id = public_id("ui.dialogue.standard");
        let descriptor = ComponentDescriptor::new(
            Some(public_id.clone()),
            ComponentSchemaId(7),
            0x1234,
            ComponentImplementation::Arcweft(UiProgramId(3)),
        );

        let id = registry.register(descriptor).unwrap();
        assert_eq!(id, ComponentId(0));
        assert_eq!(registry.resolve_public_id(&public_id), Some(id));
        assert_eq!(registry.get(id).unwrap().schema(), ComponentSchemaId(7));
    }

    #[test]
    fn property_binding_invalidation_keeps_paint_changes_out_of_layout_and_fragment() {
        let mut builder = PropertyBindingTableBuilder::default();
        builder
            .push(PropertyBinding::new(
                UiPropertyId(1),
                UiPropertyKind::Opacity,
                ValueSourceId(10),
            ))
            .unwrap();
        builder
            .push(PropertyBinding::new(
                UiPropertyId(2),
                UiPropertyKind::Rotate,
                ValueSourceId(10),
            ))
            .unwrap();
        builder
            .push(PropertyBinding::new(
                UiPropertyId(3),
                UiPropertyKind::Width,
                ValueSourceId(11),
            ))
            .unwrap();
        builder
            .push(PropertyBinding::new(
                UiPropertyId(4),
                UiPropertyKind::SemanticLabel,
                ValueSourceId(12),
            ))
            .unwrap();

        let table = builder.finish();
        let paint_flags = table.dirty_flags_for_source(ValueSourceId(10));
        assert!(paint_flags.contains(DirtyFlags::PAINT));
        assert!(!paint_flags.contains(DirtyFlags::LAYOUT));
        assert!(!paint_flags.contains(DirtyFlags::FRAGMENT));

        let layout_flags = table.dirty_flags_for_source(ValueSourceId(11));
        assert!(layout_flags.contains(DirtyFlags::LAYOUT));
        assert!(!layout_flags.contains(DirtyFlags::FRAGMENT));

        let semantics_flags = table.dirty_flags_for_source(ValueSourceId(12));
        assert!(semantics_flags.contains(DirtyFlags::SEMANTICS));
        assert!(!semantics_flags.contains(DirtyFlags::LAYOUT));
    }

    #[test]
    fn property_binding_rejects_duplicate_property_slots() {
        let mut builder = PropertyBindingTableBuilder::default();
        builder
            .push(PropertyBinding::new(
                UiPropertyId(1),
                UiPropertyKind::Color,
                ValueSourceId(1),
            ))
            .unwrap();

        assert_eq!(
            builder.push(PropertyBinding::new(
                UiPropertyId(1),
                UiPropertyKind::BackgroundColor,
                ValueSourceId(2)
            )),
            Err(UiError::DuplicatePropertyBinding(UiPropertyId(1)))
        );
    }

    #[test]
    fn reactive_graph_coalesces_property_bindings_by_source_and_entity() {
        let mut store = EntityStore::default();
        let entity = store
            .insert(
                DialogueSkinState {
                    hovered_nameplate: false,
                },
                Some(ComponentId(3)),
            )
            .unwrap();
        let mut bindings = PropertyBindingTableBuilder::default();
        bindings
            .push(PropertyBinding::new(
                UiPropertyId(1),
                UiPropertyKind::Opacity,
                ValueSourceId(1),
            ))
            .unwrap();
        bindings
            .push(PropertyBinding::new(
                UiPropertyId(2),
                UiPropertyKind::Rotate,
                ValueSourceId(1),
            ))
            .unwrap();
        bindings
            .push(PropertyBinding::new(
                UiPropertyId(3),
                UiPropertyKind::Width,
                ValueSourceId(2),
            ))
            .unwrap();

        let mut graph = ReactiveGraph::default();
        graph.watch_property_table(entity.raw(), &bindings.finish());

        let paint = graph.invalidate(ValueSourceId(1)).unwrap();
        assert_eq!(paint.source(), ValueSourceId(1));
        assert_eq!(paint.revision(), Revision(1));
        assert_eq!(paint.entities().len(), 1);
        assert_eq!(paint.entities()[0].entity(), entity.raw());
        assert!(paint.entities()[0].dirty().contains(DirtyFlags::PAINT));
        assert!(!paint.entities()[0].dirty().contains(DirtyFlags::LAYOUT));
        assert!(!paint.entities()[0].dirty().contains(DirtyFlags::FRAGMENT));

        let layout = graph.invalidate(ValueSourceId(2)).unwrap();
        assert_eq!(layout.revision(), Revision(1));
        assert!(layout.entities()[0].dirty().contains(DirtyFlags::LAYOUT));
        assert!(!layout.entities()[0].dirty().contains(DirtyFlags::FRAGMENT));

        assert_eq!(
            graph.invalidate(ValueSourceId(1)).unwrap().revision(),
            Revision(2)
        );
    }

    #[test]
    fn reactive_graph_returns_empty_invalidation_for_unwatched_sources() {
        let mut graph = ReactiveGraph::default();
        let invalidation = graph.invalidate(ValueSourceId(99)).unwrap();

        assert_eq!(invalidation.source(), ValueSourceId(99));
        assert_eq!(invalidation.revision(), Revision(1));
        assert!(invalidation.entities().is_empty());
    }

    #[test]
    fn component_registry_rejects_duplicate_public_ids() {
        let mut registry = ComponentRegistry::default();
        let public_id = public_id("ui.dialogue.standard");
        let descriptor = || {
            ComponentDescriptor::new(
                Some(public_id.clone()),
                ComponentSchemaId(1),
                0,
                ComponentImplementation::Rust(RustComponentId(1)),
            )
        };
        registry.register(descriptor()).unwrap();

        assert_eq!(
            registry.register(descriptor()),
            Err(UiError::DuplicateComponentPublicId(public_id))
        );
    }

    #[test]
    fn entity_store_rejects_stale_reused_handles() {
        let mut store = EntityStore::default();
        let first = store
            .insert(
                DialogueSkinState {
                    hovered_nameplate: false,
                },
                Some(ComponentId(1)),
            )
            .unwrap();
        assert_eq!(store.component(first), Some(ComponentId(1)));
        assert!(store.dirty(first).unwrap().contains(DirtyFlags::FRAGMENT));

        let removed = store.remove(first).unwrap();
        assert_eq!(
            removed,
            DialogueSkinState {
                hovered_nameplate: false
            }
        );

        let second = store
            .insert(InventoryState { selected_slot: 2 }, Some(ComponentId(2)))
            .unwrap();
        assert_eq!(second.raw().index(), first.raw().index());
        assert_ne!(second.raw().generation(), first.raw().generation());
        assert!(store.get(first).is_none());
        assert_eq!(
            store.mark_dirty(first, DirtyFlags::LAYOUT),
            Err(UiError::StaleEntity(first.raw()))
        );
        assert_eq!(
            store.get(second),
            Some(&InventoryState { selected_slot: 2 })
        );
    }

    #[test]
    fn entity_store_detects_wrong_state_type_on_remove() {
        let mut store = EntityStore::default();
        let state = store
            .insert(InventoryState { selected_slot: 1 }, None)
            .unwrap();
        let wrong_type = Entity::<DialogueSkinState>::from_raw(state.raw());

        assert_eq!(
            store.remove(wrong_type),
            Err(UiError::EntityTypeMismatch(state.raw()))
        );
        assert_eq!(store.get(state), Some(&InventoryState { selected_slot: 1 }));
    }

    #[test]
    fn ui_semantic_fragment_lowers_to_presentation_semantic_tree() {
        let ui_layer = layer_id("ui");
        let button_target = target("ui.confirm");
        let action = public_id("action.confirm");
        let mut builder = UiSemanticFragmentBuilder::default();
        let id = builder
            .push(
                UiSemanticNode::new(
                    NodeKey(10),
                    ui_layer,
                    button_target.clone(),
                    SemanticRole::Button,
                    HitRect::new(0.0, 0.0, 80.0, 24.0),
                )
                .with_label("Confirm")
                .with_action(action.clone()),
            )
            .unwrap();
        assert_eq!(id, UiNodeId(0));

        let tree = builder.finish().to_semantic_tree();
        let lowered = tree
            .lower_action(&button_target, &action)
            .expect("UI action lowers through presentation semantics");
        assert_eq!(lowered.target(), &ActionTarget::Entity(button_target));
        assert_eq!(lowered.kind(), &action);
    }

    #[test]
    fn ui_semantic_fragment_rejects_duplicate_node_keys() {
        let ui_layer = layer_id("ui");
        let mut builder = UiSemanticFragmentBuilder::default();
        builder
            .push(UiSemanticNode::new(
                NodeKey(1),
                ui_layer.clone(),
                target("ui.first"),
                SemanticRole::Button,
                HitRect::new(0.0, 0.0, 10.0, 10.0),
            ))
            .unwrap();

        assert_eq!(
            builder.push(UiSemanticNode::new(
                NodeKey(1),
                ui_layer,
                target("ui.second"),
                SemanticRole::Button,
                HitRect::new(10.0, 0.0, 10.0, 10.0),
            )),
            Err(UiError::DuplicateNodeKey(NodeKey(1)))
        );
    }

    #[test]
    fn ui_semantic_tree_routes_agent_invoke_through_layer_policy() {
        let root = layer_id("root");
        let ui = layer_id("ui");
        let button = target("ui.confirm");
        let action = public_id("action.confirm");
        let mut layers = LayerTree::new(LayerNode::new(
            root.clone(),
            LayerKind::Root,
            order(RenderPhase::Background, 0),
        ));
        layers
            .insert(
                LayerNode::new(ui.clone(), LayerKind::GameUi, order(RenderPhase::GameUi, 0))
                    .with_parent(root)
                    .with_input_policy(LayerInputPolicy::HitTest),
            )
            .unwrap();

        let mut builder = UiSemanticFragmentBuilder::default();
        builder
            .push(
                UiSemanticNode::new(
                    NodeKey(2),
                    ui,
                    button.clone(),
                    SemanticRole::Button,
                    HitRect::new(0.0, 0.0, 80.0, 24.0),
                )
                .with_action(action.clone()),
            )
            .unwrap();

        let tree = builder.finish().to_semantic_tree();
        let lowered = tree
            .route_and_lower_action(
                InputEpoch(1),
                &button,
                &action,
                &layers,
                &InteractionState::default(),
            )
            .unwrap();
        assert_eq!(lowered.target(), &ActionTarget::Entity(button));
    }
}
