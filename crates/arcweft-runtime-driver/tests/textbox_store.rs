use arcweft_core::plan::{FlowEvent, RuntimeLineId};
use arcweft_render_text::{LineDisplayCatalog, LineDisplaySpec, RichTextDocument, RichTextNode};
use arcweft_runtime_driver::dialogue::{
    TextBoxPresentationOperation, TextBoxPresentationStore, TextBoxTargetId,
};
use arcweft_runtime_driver::display::resolve_display_frames;
use arcweft_view::{ViewMountAllocator, ViewMountId};

fn line_id(value: &str) -> RuntimeLineId {
    RuntimeLineId::from_runtime_line_value(value).expect("test line ID is valid")
}

fn display_spec(line: &str, window: &str, text: &str) -> LineDisplaySpec {
    LineDisplaySpec {
        line: line_id(line),
        callee: "narrator".to_owned(),
        speaker_label: None,
        text_key: None,
        window: Some(window.to_owned()),
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![RichTextNode::Text {
            text: text.to_owned(),
        }]),
    }
}

fn resolved_frame(line: &str, window: &str, text: &str) -> arcweft_render_text::LineDisplayFrame {
    let catalog = LineDisplayCatalog::new(vec![display_spec(line, window, text)]);
    let resolution = resolve_display_frames(
        &catalog,
        &[FlowEvent::DialogueLine {
            line: line_id(line),
            bindings: Vec::new(),
        }],
    );
    let TextBoxPresentationOperation::Append { frame, .. } = resolution
        .textbox_operations
        .into_iter()
        .next()
        .expect("one resolved operation")
    else {
        panic!("ordinary dialogue resolves to append")
    };
    frame
}

#[test]
fn same_step_dialogue_events_append_in_order_to_the_same_target() {
    let catalog = LineDisplayCatalog::new(vec![
        display_spec("line.first", "textbox.main", "first"),
        display_spec("line.second", "textbox.main", "second"),
    ]);
    let resolution = resolve_display_frames(
        &catalog,
        &[
            FlowEvent::DialogueLine {
                line: line_id("line.first"),
                bindings: Vec::new(),
            },
            FlowEvent::DialogueLine {
                line: line_id("line.second"),
                bindings: Vec::new(),
            },
        ],
    );
    assert!(
        resolution
            .textbox_operations
            .iter()
            .all(|operation| matches!(operation, TextBoxPresentationOperation::Append { .. }))
    );

    let mut allocator = ViewMountAllocator::default();
    let authored_mount = allocator.allocate().expect("authored mount allocates");
    let mut store = TextBoxPresentationStore::default();
    store
        .apply_operations(&resolution.textbox_operations, &mut allocator)
        .expect("ordered operations apply");

    let main = store
        .get_by_target(&TextBoxTargetId::from("textbox.main"))
        .expect("main TextBox exists");
    assert_eq!(
        main.entries()
            .iter()
            .map(|entry| entry.frame().text.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
    assert_eq!(main.active_entry_id(), Some(main.entries()[1].id()));
    assert_ne!(main.mount().view_mount_id(), authored_mount);
    assert_eq!(main.mount().view_mount_id(), ViewMountId::from_raw(1));

    store
        .synchronize_waiting_line(Some(&line_id("line.first")))
        .expect("runtime waiting line selects its retained entry");
    let main = store
        .get_by_target(&TextBoxTargetId::from("textbox.main"))
        .expect("main TextBox remains mounted");
    assert_eq!(main.active_entry_id(), Some(main.entries()[0].id()));
    assert!(main.entries()[0].is_waiting_for_advance());
    assert!(!main.entries()[1].is_waiting_for_advance());
    assert_eq!(
        main.advance_target()
            .expect("first entry is actionable")
            .entry,
        main.entries()[0].id()
    );
}

#[test]
fn targets_keep_independent_order_ids_mounts_and_revisions_across_replace_and_clear() {
    let mut allocator = ViewMountAllocator::default();
    let mut store = TextBoxPresentationStore::default();
    store
        .apply_operations(
            &[
                TextBoxPresentationOperation::append(
                    "textbox.main",
                    resolved_frame("line.main.first", "textbox.main", "main-1"),
                ),
                TextBoxPresentationOperation::append(
                    "textbox.side",
                    resolved_frame("line.side.first", "textbox.side", "side-1"),
                ),
                TextBoxPresentationOperation::append(
                    "textbox.main",
                    resolved_frame("line.main.second", "textbox.main", "main-2"),
                ),
            ],
            &mut allocator,
        )
        .expect("initial operations apply");

    let main_target = TextBoxTargetId::from("textbox.main");
    let side_target = TextBoxTargetId::from("textbox.side");
    let main = store.get_by_target(&main_target).expect("main TextBox");
    let side = store.get_by_target(&side_target).expect("side TextBox");
    let main_id = main.id();
    let main_mount = main.mount();
    let side_id = side.id();
    let side_mount = side.mount();
    assert_eq!(main.revision().get(), 2);
    assert_eq!(side.revision().get(), 1);
    assert_eq!(
        main.entries()
            .iter()
            .map(|entry| entry.id().get())
            .collect::<Vec<_>>(),
        vec![0, 2]
    );
    assert_eq!(side.entries()[0].id().get(), 1);

    store
        .apply_operations(
            &[
                TextBoxPresentationOperation::replace(
                    "textbox.side",
                    resolved_frame("line.side.second", "textbox.side", "side-2"),
                ),
                TextBoxPresentationOperation::clear("textbox.main"),
                TextBoxPresentationOperation::append(
                    "textbox.main",
                    resolved_frame("line.main.third", "textbox.main", "main-3"),
                ),
            ],
            &mut allocator,
        )
        .expect("replace, clear, and append apply in order");

    let main = store.get_by_target(&main_target).expect("main TextBox");
    let side = store.get_by_target(&side_target).expect("side TextBox");
    assert_eq!(main.id(), main_id);
    assert_eq!(main.mount(), main_mount);
    assert_eq!(main.revision().get(), 4);
    assert_eq!(main.entries().len(), 1);
    assert_eq!(main.entries()[0].id().get(), 4);
    assert_eq!(main.entries()[0].frame().text, "main-3");
    assert_eq!(side.id(), side_id);
    assert_eq!(side.mount(), side_mount);
    assert_eq!(side.revision().get(), 2);
    assert_eq!(side.entries().len(), 1);
    assert_eq!(side.entries()[0].id().get(), 3);
    assert_eq!(side.entries()[0].frame().text, "side-2");
}

#[test]
fn store_and_shared_mount_cursor_round_trip_without_reusing_identity() {
    let mut allocator = ViewMountAllocator::default();
    let mut store = TextBoxPresentationStore::default();
    store
        .apply_operations(
            &[TextBoxPresentationOperation::append(
                "textbox.main",
                resolved_frame("line.before", "textbox.main", "before"),
            )],
            &mut allocator,
        )
        .expect("initial append applies");
    let bytes = serde_json::to_vec(&(store, allocator)).expect("snapshot encodes");
    let (mut restored, mut restored_allocator): (TextBoxPresentationStore, ViewMountAllocator) =
        serde_json::from_slice(&bytes).expect("snapshot decodes");
    restored.validate().expect("restored store validates");

    restored
        .apply_operations(
            &[TextBoxPresentationOperation::append(
                "textbox.side",
                resolved_frame("line.after", "textbox.side", "after"),
            )],
            &mut restored_allocator,
        )
        .expect("post-restore append applies");

    let main = restored
        .get_by_target(&TextBoxTargetId::from("textbox.main"))
        .expect("main TextBox");
    let side = restored
        .get_by_target(&TextBoxTargetId::from("textbox.side"))
        .expect("side TextBox");
    assert_eq!(main.id().get(), 0);
    assert_eq!(main.entries()[0].id().get(), 0);
    assert_eq!(main.mount().get(), 0);
    assert_eq!(side.id().get(), 1);
    assert_eq!(side.entries()[0].id().get(), 1);
    assert_eq!(side.mount().get(), 1);
    assert_eq!(restored_allocator.next(), 2);
}

#[test]
fn snapshot_with_retained_entries_but_no_active_entry_is_rejected() {
    let mut allocator = ViewMountAllocator::default();
    let mut store = TextBoxPresentationStore::default();
    store
        .apply_operations(
            &[TextBoxPresentationOperation::append(
                "textbox.main",
                resolved_frame("line.invalid.active", "textbox.main", "retained"),
            )],
            &mut allocator,
        )
        .expect("fixture append applies");
    let mut value = serde_json::to_value(store).expect("store serializes");
    let textbox = value["textboxes"]
        .as_object_mut()
        .and_then(|textboxes| textboxes.values_mut().next())
        .expect("serialized store contains one TextBox");
    textbox["active"] = serde_json::Value::Null;
    let tampered: TextBoxPresentationStore =
        serde_json::from_value(value).expect("private snapshot shape still decodes");

    assert!(tampered.validate().is_err());
}
