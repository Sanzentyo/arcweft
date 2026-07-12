use arcweft_core::plan::{FlowEvent, RuntimeLineId};
use arcweft_render_text::{LineDisplayCatalog, LineDisplaySpec, RichTextDocument, RichTextNode};
use arcweft_runtime_driver::dialogue::{
    DialoguePresentationOperation, DialoguePresentationStore, DialogueViewDefinition,
};
use arcweft_runtime_driver::display::resolve_display_frames;

fn line_id(value: &str) -> RuntimeLineId {
    RuntimeLineId::from_runtime_line_value(value).expect("test line ID is valid")
}

fn display_spec(line: &str, view: &str, text: &str) -> LineDisplaySpec {
    LineDisplaySpec {
        line: line_id(line),
        callee: "narrator".to_owned(),
        speaker_label: None,
        text_key: None,
        view: Some(view.to_owned()),
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

fn resolved_frame(line: &str, view: &str, text: &str) -> arcweft_render_text::LineDisplayFrame {
    let catalog = LineDisplayCatalog::new(vec![display_spec(line, view, text)]);
    let resolution = resolve_display_frames(
        &catalog,
        &[FlowEvent::DialogueLine {
            line: line_id(line),
            bindings: Vec::new(),
        }],
    );
    let DialoguePresentationOperation::Append { frame, .. } = resolution
        .dialogue_operations
        .into_iter()
        .next()
        .expect("one resolved operation")
    else {
        panic!("ordinary dialogue resolves to append")
    };
    frame
}

#[test]
fn same_view_history_mounts_only_its_active_occurrence() {
    let catalog = LineDisplayCatalog::new(vec![
        display_spec("line.first", "view.Dialogue", "first"),
        display_spec("line.second", "view.Dialogue", "second"),
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
    let mut store = DialoguePresentationStore::default();
    store
        .apply_operations(&resolution.dialogue_operations)
        .expect("ordered operations apply");

    let dialogue = store
        .get_by_definition(&DialogueViewDefinition::from("view.Dialogue"))
        .expect("dialogue View presentation exists");
    assert_eq!(
        dialogue
            .entries()
            .iter()
            .map(|entry| entry.frame().text.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
    let inputs = store.view_inputs();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].handle.as_str(), "dialogue.1");
    assert_eq!(inputs[0].view, "view.Dialogue");
    assert!(inputs[0].state.primary_action.target.is_none());

    store
        .synchronize_waiting_line(Some(&line_id("line.first")))
        .expect("runtime waiting line selects its retained entry");
    let dialogue = store
        .get_by_definition(&DialogueViewDefinition::from("view.Dialogue"))
        .expect("dialogue View presentation remains retained");
    assert_eq!(dialogue.active_entry_id(), Some(dialogue.entries()[0].id()));
    assert!(dialogue.entries()[0].is_waiting_for_advance());
    assert!(!dialogue.entries()[1].is_waiting_for_advance());
    let inputs = store.view_inputs();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].handle.as_str(), "dialogue.0");
    assert_eq!(
        inputs[0].state.primary_action.target,
        dialogue.advance_target()
    );
}

#[test]
fn authored_view_definitions_keep_independent_order_and_revisions() {
    let mut store = DialoguePresentationStore::default();
    store
        .apply_operations(&[
            DialoguePresentationOperation::append(
                "view.MainDialogue",
                resolved_frame("line.main.first", "view.MainDialogue", "main-1"),
            ),
            DialoguePresentationOperation::append(
                "view.SideDialogue",
                resolved_frame("line.side.first", "view.SideDialogue", "side-1"),
            ),
            DialoguePresentationOperation::append(
                "view.MainDialogue",
                resolved_frame("line.main.second", "view.MainDialogue", "main-2"),
            ),
        ])
        .expect("initial operations apply");

    let main_target = DialogueViewDefinition::from("view.MainDialogue");
    let side_target = DialogueViewDefinition::from("view.SideDialogue");
    let main = store
        .get_by_definition(&main_target)
        .expect("main dialogue View");
    let side = store
        .get_by_definition(&side_target)
        .expect("side dialogue View");
    let main_id = main.id();
    let side_id = side.id();
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
    let inputs = store.view_inputs();
    assert_eq!(inputs.len(), 2);
    assert_ne!(inputs[0].handle, inputs[1].handle);

    store
        .apply_operations(&[
            DialoguePresentationOperation::replace(
                "view.SideDialogue",
                resolved_frame("line.side.second", "view.SideDialogue", "side-2"),
            ),
            DialoguePresentationOperation::clear("view.MainDialogue"),
            DialoguePresentationOperation::append(
                "view.MainDialogue",
                resolved_frame("line.main.third", "view.MainDialogue", "main-3"),
            ),
        ])
        .expect("replace, clear, and append apply in order");

    let main = store
        .get_by_definition(&main_target)
        .expect("main dialogue View");
    let side = store
        .get_by_definition(&side_target)
        .expect("side dialogue View");
    assert_eq!(main.id(), main_id);
    assert_eq!(main.revision().get(), 4);
    assert_eq!(main.entries()[0].id().get(), 4);
    assert_eq!(main.entries()[0].frame().text, "main-3");
    assert_eq!(side.id(), side_id);
    assert_eq!(side.revision().get(), 2);
    assert_eq!(side.entries()[0].id().get(), 3);
    assert_eq!(side.entries()[0].frame().text, "side-2");
}

#[test]
fn store_round_trip_does_not_reuse_occurrence_identity() {
    let mut store = DialoguePresentationStore::default();
    store
        .apply_operations(&[DialoguePresentationOperation::append(
            "view.MainDialogue",
            resolved_frame("line.before", "view.MainDialogue", "before"),
        )])
        .expect("initial append applies");
    let bytes = serde_json::to_vec(&store).expect("snapshot encodes");
    let mut restored: DialoguePresentationStore =
        serde_json::from_slice(&bytes).expect("snapshot decodes");
    restored.validate().expect("restored store validates");

    restored
        .apply_operations(&[DialoguePresentationOperation::append(
            "view.SideDialogue",
            resolved_frame("line.after", "view.SideDialogue", "after"),
        )])
        .expect("post-restore append applies");

    let inputs = restored.view_inputs();
    assert_eq!(inputs.len(), 2);
    assert_eq!(inputs[0].handle.as_str(), "dialogue.0");
    assert_eq!(inputs[1].handle.as_str(), "dialogue.1");
}

#[test]
fn snapshot_with_retained_entries_but_no_active_entry_is_rejected() {
    let mut store = DialoguePresentationStore::default();
    store
        .apply_operations(&[DialoguePresentationOperation::append(
            "view.Dialogue",
            resolved_frame("line.invalid.active", "view.Dialogue", "retained"),
        )])
        .expect("fixture append applies");
    let mut value = serde_json::to_value(store).expect("store serializes");
    let dialogue = value["presentations"]
        .as_object_mut()
        .and_then(|presentations| presentations.values_mut().next())
        .expect("serialized store contains one dialogue presentation");
    dialogue["active"] = serde_json::Value::Null;
    let tampered: DialoguePresentationStore =
        serde_json::from_value(value).expect("private snapshot shape still decodes");

    assert!(tampered.validate().is_err());
}
