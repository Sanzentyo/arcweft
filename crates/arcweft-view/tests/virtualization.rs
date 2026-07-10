use arcweft_id::PublicId;
use arcweft_view::program::{ViewStableKey, ViewVirtualAxis};
use arcweft_view::virtualization::{
    ViewVirtualAnchor, ViewVirtualItem, ViewVirtualScrollTarget, ViewVirtualizationError,
    ViewVirtualizationRuntime,
};

fn item(key: u64, extent_milli: u32) -> ViewVirtualItem {
    ViewVirtualItem::new(ViewStableKey(key), extent_milli)
}

fn target(value: &str) -> ViewVirtualScrollTarget {
    ViewVirtualScrollTarget::from(PublicId::try_new(value).unwrap())
}

#[test]
fn range_table_includes_materialized_and_retained_only_items() {
    let mut runtime = ViewVirtualizationRuntime::default();
    let mount = runtime
        .mount(
            target("scroll.inventory"),
            ViewVirtualAxis::Vertical,
            100,
            vec![item(10, 60), item(11, 60), item(12, 60), item(13, 60)],
        )
        .unwrap();
    let list = runtime.get_mut(mount).unwrap();
    list.scroll_to_milli(70);

    let table = list.range_table();
    assert_eq!(table.scroll_target.as_str(), "scroll.inventory");
    assert_eq!(table.materialized.start, 1);
    assert_eq!(table.materialized.end, 3);
    assert_eq!(
        table
            .items
            .iter()
            .map(|range| (range.key.0, range.start_milli, range.materialized))
            .collect::<Vec<_>>(),
        vec![
            (10, 0, false),
            (11, 60, true),
            (12, 120, true),
            (13, 180, false),
        ]
    );
}

#[test]
fn source_replacement_preserves_key_relative_anchor() {
    let mut runtime = ViewVirtualizationRuntime::default();
    let mount = runtime
        .mount(
            target("scroll.inventory"),
            ViewVirtualAxis::Vertical,
            50,
            vec![item(1, 40), item(2, 40), item(3, 40)],
        )
        .unwrap();
    let list = runtime.get_mut(mount).unwrap();
    list.scroll_to_milli(55);
    assert_eq!(list.anchor().unwrap().key, ViewStableKey(2));

    list.replace_items(vec![item(9, 20), item(1, 40), item(2, 40), item(3, 40)])
        .unwrap();

    assert_eq!(list.offset_milli(), 75);
    assert_eq!(list.anchor().unwrap().key, ViewStableKey(2));
    assert_eq!(list.anchor().unwrap().offset_within_item_milli, 15);

    let before = list.clone();
    assert!(matches!(
        list.replace_items(vec![item(1, 10), item(1, 20)]),
        Err(ViewVirtualizationError::DuplicateItemKey { .. })
    ));
    assert_eq!(&*list, &before);
}

#[test]
fn snapshot_restores_independent_mounts_and_monotonic_allocator() {
    let mut runtime = ViewVirtualizationRuntime::default();
    let first = runtime
        .mount(
            target("scroll.left"),
            ViewVirtualAxis::Horizontal,
            50,
            vec![item(1, 50), item(2, 50), item(3, 50)],
        )
        .unwrap();
    let second = runtime
        .mount(
            target("scroll.right"),
            ViewVirtualAxis::Horizontal,
            50,
            vec![item(1, 50), item(2, 50), item(3, 50)],
        )
        .unwrap();
    runtime.get_mut(first).unwrap().scroll_to_milli(25);
    runtime.get_mut(second).unwrap().scroll_to_milli(75);
    let snapshot = runtime.snapshot();

    let mut restored = ViewVirtualizationRuntime::from_snapshot(&snapshot).unwrap();
    assert_eq!(restored.get(first).unwrap().offset_milli(), 25);
    assert_eq!(restored.get(second).unwrap().offset_milli(), 75);
    restored.unmount(first);
    let third = restored
        .mount(
            target("scroll.third"),
            ViewVirtualAxis::Vertical,
            10,
            vec![item(1, 10)],
        )
        .unwrap();
    assert!(third.get() > second.get());
}

#[test]
fn invalid_mount_input_does_not_consume_an_occurrence_id() {
    let mut runtime = ViewVirtualizationRuntime::default();
    assert_eq!(
        runtime
            .mount(
                target("scroll.inventory"),
                ViewVirtualAxis::Vertical,
                10,
                vec![item(1, 10), item(1, 20)],
            )
            .unwrap_err(),
        ViewVirtualizationError::DuplicateItemKey {
            key: ViewStableKey(1)
        }
    );
    assert_eq!(
        runtime
            .mount(
                target("scroll.inventory"),
                ViewVirtualAxis::Vertical,
                10,
                vec![item(1, 0)],
            )
            .unwrap_err(),
        ViewVirtualizationError::ZeroItemExtent { index: 0 }
    );
    let first = runtime
        .mount(
            target("scroll.inventory"),
            ViewVirtualAxis::Vertical,
            10,
            vec![item(1, 10)],
        )
        .unwrap();
    assert_eq!(first.get(), 0);
}

#[test]
fn invalid_snapshot_restore_is_atomic_and_rejects_normalization() {
    let mut runtime = ViewVirtualizationRuntime::default();
    let mount = runtime
        .mount(
            target("scroll.inventory"),
            ViewVirtualAxis::Vertical,
            10,
            vec![item(1, 10), item(2, 10)],
        )
        .unwrap();
    runtime.get_mut(mount).unwrap().scroll_to_milli(5);
    let before = runtime.clone();

    let mut invalid_list = runtime.get(mount).unwrap().snapshot();
    invalid_list.viewport_extent_milli = 7;
    invalid_list.items[0].extent_milli = 0;
    let before_list = runtime.get(mount).unwrap().clone();
    assert!(matches!(
        runtime.get_mut(mount).unwrap().restore(&invalid_list),
        Err(ViewVirtualizationError::ZeroItemExtent { .. })
    ));
    assert_eq!(runtime.get(mount).unwrap(), &before_list);

    let mut invalid_item = runtime.snapshot();
    invalid_item.mounts[0].items[0].extent_milli = 0;
    assert!(matches!(
        runtime.restore(&invalid_item),
        Err(ViewVirtualizationError::ZeroItemExtent { .. })
    ));
    assert_eq!(runtime, before);

    let mut invalid_anchor = runtime.snapshot();
    invalid_anchor.mounts[0].anchor = Some(ViewVirtualAnchor {
        key: ViewStableKey(2),
        offset_within_item_milli: 0,
    });
    assert_eq!(
        runtime.restore(&invalid_anchor).unwrap_err(),
        ViewVirtualizationError::SnapshotAnchorMismatch { mount }
    );
    assert_eq!(runtime, before);

    let mut invalid_offset = runtime.snapshot();
    invalid_offset.mounts[0].absolute_offset_milli = 11;
    assert_eq!(
        runtime.restore(&invalid_offset).unwrap_err(),
        ViewVirtualizationError::SnapshotOffsetOutOfRange {
            mount,
            saved: 11,
            maximum: 10,
        }
    );
    assert_eq!(runtime, before);

    let mut stale_allocator = runtime.snapshot();
    stale_allocator.next_mount_id = mount.get();
    assert_eq!(
        runtime.restore(&stale_allocator).unwrap_err(),
        ViewVirtualizationError::SnapshotMountAllocatorNotFresh {
            next_mount_id: 0,
            greatest_mount_id: 0,
        }
    );
    assert_eq!(runtime, before);
}

#[test]
fn half_open_window_handles_exact_and_one_milli_boundaries() {
    let mut runtime = ViewVirtualizationRuntime::default();
    let mount = runtime
        .mount(
            target("scroll.inventory"),
            ViewVirtualAxis::Vertical,
            10,
            vec![item(1, 10), item(2, 10), item(3, 10)],
        )
        .unwrap();
    let list = runtime.get_mut(mount).unwrap();
    assert_eq!(
        (
            list.materialized_window().start,
            list.materialized_window().end
        ),
        (0, 1)
    );
    list.scroll_to_milli(9);
    assert_eq!(
        (
            list.materialized_window().start,
            list.materialized_window().end
        ),
        (0, 2)
    );
    list.scroll_to_milli(10);
    assert_eq!(
        (
            list.materialized_window().start,
            list.materialized_window().end
        ),
        (1, 2)
    );

    list.set_viewport_extent_milli(0);
    list.scroll_to_milli(30);
    assert!(list.materialized_window().is_empty());
    assert_eq!(list.anchor().unwrap().key, ViewStableKey(3));
    assert_eq!(list.anchor().unwrap().offset_within_item_milli, 10);
    list.replace_items(vec![item(9, 10), item(1, 10), item(2, 10), item(3, 10)])
        .unwrap();
    assert_eq!(list.offset_milli(), 40);

    list.set_viewport_extent_milli(100);
    assert_eq!(
        (
            list.materialized_window().start,
            list.materialized_window().end
        ),
        (0, 4)
    );

    let empty = runtime
        .mount(
            target("scroll.empty"),
            ViewVirtualAxis::Vertical,
            100,
            Vec::new(),
        )
        .unwrap();
    assert!(runtime.get(empty).unwrap().materialized_window().is_empty());
    assert_eq!(runtime.get(empty).unwrap().anchor(), None);
}

#[test]
fn range_pages_are_bounded_but_keep_global_indices() {
    let mut runtime = ViewVirtualizationRuntime::default();
    let mount = runtime
        .mount(
            target("scroll.inventory"),
            ViewVirtualAxis::Vertical,
            10,
            vec![item(1, 10), item(2, 10), item(3, 10), item(4, 10)],
        )
        .unwrap();
    let page = runtime.get(mount).unwrap().range_page(1, 2);
    assert_eq!((page.total_items, page.start, page.end), (4, 1, 3));
    assert_eq!(
        page.items.iter().map(|item| item.index).collect::<Vec<_>>(),
        vec![1, 2]
    );
}

#[test]
fn serialized_snapshot_round_trips_and_rejects_invalid_scroll_target() {
    let mut runtime = ViewVirtualizationRuntime::default();
    runtime
        .mount(
            target("scroll.inventory"),
            ViewVirtualAxis::Vertical,
            10,
            vec![item(1, 10)],
        )
        .unwrap();
    let snapshot = runtime.snapshot();
    let encoded = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(
        serde_json::from_value::<arcweft_view::virtualization::ViewVirtualizationSnapshot>(
            encoded.clone(),
        )
        .unwrap(),
        snapshot
    );

    let mut invalid = encoded;
    invalid["mounts"][0]["scroll_target"] = serde_json::json!("");
    assert!(
        serde_json::from_value::<arcweft_view::virtualization::ViewVirtualizationSnapshot>(invalid)
            .is_err()
    );
}
