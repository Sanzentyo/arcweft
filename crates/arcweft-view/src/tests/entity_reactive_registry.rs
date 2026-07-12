use crate::{
    DirtyFlags, Entity, EntityStore, PropertyBinding, PropertyBindingTableBuilder, ReactiveGraph,
    Revision, RustViewId, ValueSourceId, ViewDescriptor, ViewError, ViewId, ViewImplementation,
    ViewProgramId, ViewPropertyId, ViewPropertyKind, ViewRegistry, ViewSchemaId,
};
use arcweft_id::PublicId;

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

#[test]
fn view_registry_resolves_dense_view_ids() {
    let mut registry = ViewRegistry::default();
    let public_id = public_id("view.dialogue.standard");
    let descriptor = ViewDescriptor::new(
        Some(public_id.clone()),
        ViewSchemaId(7),
        0x1234,
        ViewImplementation::Arcweft(ViewProgramId(3)),
    );

    let id = registry.register(descriptor).unwrap();
    assert_eq!(id, ViewId(0));
    assert_eq!(registry.resolve_public_id(&public_id), Some(id));
    assert_eq!(registry.get(id).unwrap().schema(), ViewSchemaId(7));
}

#[test]
fn property_binding_invalidation_keeps_paint_changes_out_of_layout_and_fragment() {
    let mut builder = PropertyBindingTableBuilder::default();
    builder
        .push(PropertyBinding::new(
            ViewPropertyId(1),
            ViewPropertyKind::Opacity,
            ValueSourceId(10),
        ))
        .unwrap();
    builder
        .push(PropertyBinding::new(
            ViewPropertyId(2),
            ViewPropertyKind::Rotate,
            ValueSourceId(10),
        ))
        .unwrap();
    builder
        .push(PropertyBinding::new(
            ViewPropertyId(3),
            ViewPropertyKind::Width,
            ValueSourceId(11),
        ))
        .unwrap();
    builder
        .push(PropertyBinding::new(
            ViewPropertyId(4),
            ViewPropertyKind::SemanticLabel,
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
            ViewPropertyId(1),
            ViewPropertyKind::Color,
            ValueSourceId(1),
        ))
        .unwrap();

    assert_eq!(
        builder.push(PropertyBinding::new(
            ViewPropertyId(1),
            ViewPropertyKind::BackgroundColor,
            ValueSourceId(2)
        )),
        Err(ViewError::DuplicatePropertyBinding(ViewPropertyId(1)))
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
            Some(ViewId(3)),
        )
        .unwrap();
    let mut bindings = PropertyBindingTableBuilder::default();
    bindings
        .push(PropertyBinding::new(
            ViewPropertyId(1),
            ViewPropertyKind::Opacity,
            ValueSourceId(1),
        ))
        .unwrap();
    bindings
        .push(PropertyBinding::new(
            ViewPropertyId(2),
            ViewPropertyKind::Rotate,
            ValueSourceId(1),
        ))
        .unwrap();
    bindings
        .push(PropertyBinding::new(
            ViewPropertyId(3),
            ViewPropertyKind::Width,
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
fn view_registry_rejects_duplicate_public_ids() {
    let mut registry = ViewRegistry::default();
    let public_id = public_id("view.dialogue.standard");
    let descriptor = || {
        ViewDescriptor::new(
            Some(public_id.clone()),
            ViewSchemaId(1),
            0,
            ViewImplementation::Rust(RustViewId(1)),
        )
    };
    registry.register(descriptor()).unwrap();

    assert_eq!(
        registry.register(descriptor()),
        Err(ViewError::DuplicateViewPublicId(public_id))
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
            Some(ViewId(1)),
        )
        .unwrap();
    assert_eq!(store.view(first), Some(ViewId(1)));
    assert!(store.dirty(first).unwrap().contains(DirtyFlags::FRAGMENT));

    let removed = store.remove(first).unwrap();
    assert_eq!(
        removed,
        DialogueSkinState {
            hovered_nameplate: false
        }
    );

    let second = store
        .insert(InventoryState { selected_slot: 2 }, Some(ViewId(2)))
        .unwrap();
    assert_eq!(second.raw().index(), first.raw().index());
    assert_ne!(second.raw().generation(), first.raw().generation());
    assert!(store.get(first).is_none());
    assert_eq!(
        store.mark_dirty(first, DirtyFlags::LAYOUT),
        Err(ViewError::StaleEntity(first.raw()))
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
        Err(ViewError::EntityTypeMismatch(state.raw()))
    );
    assert_eq!(store.get(state), Some(&InventoryState { selected_slot: 1 }));
}
