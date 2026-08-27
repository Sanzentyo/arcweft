use arcweft_character::id::CharacterId;
use arcweft_core::{
    entry::RuntimeValueDigest,
    plan::RuntimeLineId,
    runtime_id::{DialogueActivationId, RuntimeDialogueContentPlanId, RuntimePersistentFiberId},
};
use arcweft_dialogue::InlineFailurePolicy;
use arcweft_id::TextKey;
use arcweft_render_text::{RuntimeLineContext, resolve_frame};
use arcweft_runtime_driver::dialogue::{
    DialoguePresentationOperation, DialoguePresentationStore, DialogueViewDefinition,
};
use arcweft_source::{ProductSourceRef, SourceDocument, SourceDocumentId, SourceName};
use arcweft_text_model::{
    CharacterDialoguePresentationConfig, DialogueContentSpec, DialoguePresentationCharacter,
    LineDisplayFrame, RichTextDocument, RichTextNode,
};
use std::collections::BTreeMap;

mod support;

fn line_id(value: &str) -> RuntimeLineId {
    RuntimeLineId::from_runtime_line_value(value).expect("test line ID is valid")
}

fn activation(occurrence: u64) -> DialogueActivationId {
    DialogueActivationId::new(
        arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes([0x47; 32])
            .expect("fixture artifact"),
        RuntimePersistentFiberId::from_allocated(1),
        serde_json::from_value::<RuntimeDialogueContentPlanId>(serde_json::json!(1))
            .expect("fixture content identity"),
        occurrence,
    )
}

fn source_ref() -> ProductSourceRef {
    let manifest = SourceDocument::try_new(
        SourceDocumentId::try_new("runtime-driver-dialogue-view-store-test").expect("document ID"),
        SourceName::Memory,
        "test manifest",
    )
    .expect("test document");
    ProductSourceRef::try_for_identity(manifest.identity()).expect("product source identity")
}

fn view_definition(value: &str) -> DialogueViewDefinition {
    DialogueViewDefinition::new(
        arcweft_view::ViewId::try_new(value).expect("test View ID is valid"),
    )
}

fn runtime_context(view: &str) -> RuntimeLineContext {
    RuntimeLineContext::new(
        Vec::new(),
        DialoguePresentationCharacter {
            id: CharacterId::try_new("character.narrator").expect("character identity"),
            display_name: "Narrator".to_owned(),
        },
        CharacterDialoguePresentationConfig {
            view: arcweft_view::ViewId::try_new(view).expect("test View ID is valid"),
            voice: None,
            look: None,
            stage: None,
            portrait: None,
            focus: None,
            cleanup: None,
            source_locale: None,
            hooks: Vec::new(),
            inline_failure: InlineFailurePolicy::FailLine,
            custom: BTreeMap::new(),
            config_digest: RuntimeValueDigest::ZERO,
        },
        Vec::new(),
        Vec::new(),
    )
}

fn content_spec(line: &str, text: &str) -> DialogueContentSpec {
    DialogueContentSpec::new(
        line_id(line),
        TextKey::try_new(line.replace("line.", "text.")).expect("text key"),
        RichTextDocument::new(vec![RichTextNode::Text {
            text: text.to_owned(),
        }]),
        support::character_plan("character.narrator"),
        arcweft_text_model::DialoguePresentationSnapshot::new(
            support::dialogue_profile(),
            support::dialogue_profile_revision(),
        ),
        Vec::new(),
        source_ref(),
    )
}

fn resolved_frame(line: &str, view: &str, text: &str) -> LineDisplayFrame {
    resolve_frame(&content_spec(line, text), &runtime_context(view))
        .expect("final dialogue content resolves with explicit runtime context")
}

#[test]
fn same_view_history_mounts_only_its_active_occurrence() {
    let mut store = DialoguePresentationStore::default();
    store
        .apply_operations(&[
            DialoguePresentationOperation::append(
                activation(0),
                view_definition("view.Dialogue"),
                resolved_frame("line.first", "view.Dialogue", "first"),
            ),
            DialoguePresentationOperation::append(
                activation(1),
                view_definition("view.Dialogue"),
                resolved_frame("line.second", "view.Dialogue", "second"),
            ),
        ])
        .expect("ordered operations apply");

    let dialogue = store
        .get_by_definition(&view_definition("view.Dialogue"))
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
    assert_eq!(inputs[0].view.as_str(), "view.Dialogue");
    assert!(inputs[0].state.primary_action.target.is_none());

    store
        .synchronize_waiting_activation(Some(&activation(0)))
        .expect("runtime waiting activation selects its retained entry");
    let dialogue = store
        .get_by_definition(&view_definition("view.Dialogue"))
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
fn unknown_waiting_activation_is_rejected_without_mutating_the_store() {
    let mut store = DialoguePresentationStore::default();
    store
        .apply_operations(&[DialoguePresentationOperation::append(
            activation(10),
            view_definition("view.Dialogue"),
            resolved_frame("line.only", "view.Dialogue", "only"),
        )])
        .expect("dialogue append applies");
    let retained = activation(10);
    store
        .synchronize_waiting_activation(Some(&retained))
        .expect("retained activation selects its entry");
    let before = store.clone();
    let unknown = activation(11);

    let error = store
        .synchronize_waiting_activation(Some(&unknown))
        .expect_err("unknown activation must fail closed");

    assert_eq!(
        error,
        arcweft_runtime_driver::dialogue::DialoguePresentationStoreError::UnknownActivation {
            activation: unknown,
        }
    );
    assert_eq!(store, before);
}

#[test]
fn authored_view_definitions_keep_independent_order_and_revisions() {
    let mut store = DialoguePresentationStore::default();
    store
        .apply_operations(&[
            DialoguePresentationOperation::append(
                activation(2),
                view_definition("view.MainDialogue"),
                resolved_frame("line.main.first", "view.MainDialogue", "main-1"),
            ),
            DialoguePresentationOperation::append(
                activation(3),
                view_definition("view.SideDialogue"),
                resolved_frame("line.side.first", "view.SideDialogue", "side-1"),
            ),
            DialoguePresentationOperation::append(
                activation(4),
                view_definition("view.MainDialogue"),
                resolved_frame("line.main.second", "view.MainDialogue", "main-2"),
            ),
        ])
        .expect("initial operations apply");

    let main_target = view_definition("view.MainDialogue");
    let side_target = view_definition("view.SideDialogue");
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
                activation(5),
                view_definition("view.SideDialogue"),
                resolved_frame("line.side.second", "view.SideDialogue", "side-2"),
            ),
            DialoguePresentationOperation::clear(view_definition("view.MainDialogue")),
            DialoguePresentationOperation::append(
                activation(6),
                view_definition("view.MainDialogue"),
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
            activation(7),
            view_definition("view.MainDialogue"),
            resolved_frame("line.before", "view.MainDialogue", "before"),
        )])
        .expect("initial append applies");
    let bytes = serde_json::to_vec(&store).expect("snapshot encodes");
    let mut restored: DialoguePresentationStore =
        serde_json::from_slice(&bytes).expect("snapshot decodes");
    restored.validate().expect("restored store validates");

    restored
        .apply_operations(&[DialoguePresentationOperation::append(
            activation(8),
            view_definition("view.SideDialogue"),
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
            activation(9),
            view_definition("view.Dialogue"),
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
