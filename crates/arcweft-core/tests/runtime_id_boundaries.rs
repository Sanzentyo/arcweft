use arcweft_core::plan::{EntryRuntimeId, FlowRuntimeId, RuntimeLineId};
use arcweft_core::runtime_id::{
    RuntimeIdError, RuntimeIdFamily, RuntimeIdPath, RuntimeIdReference, RuntimeIdReferenceAnchor,
    RuntimePublicLabel,
};
use arcweft_core::stream::StreamRuntimeId;

#[test]
fn source_flow_entity_lowers_to_canonical_runtime_id_without_family_payload() {
    let flow = FlowRuntimeId::from_source_entity_body("flow.main").expect("flow source ID lowers");

    assert_eq!(flow.canonical_label(), "main");
    assert_eq!(flow.public_label().as_str(), "flow.main");
    assert_eq!(flow.path().segments().len(), 1);
}

#[test]
fn fragment_alias_lowers_to_same_flow_runtime_domain() {
    let fragment =
        FlowRuntimeId::from_source_entity_body("frag.intro").expect("fragment source ID lowers");

    assert_eq!(fragment.canonical_label(), "intro");
    assert_eq!(fragment.public_label().as_str(), "flow.intro");
}

#[test]
fn public_label_dot_is_not_a_runtime_namespace_selector() {
    let label = RuntimePublicLabel::new("flow.chapter.one.main");

    assert_eq!(label.as_str(), "flow.chapter.one.main");
    assert_eq!(label.into_string(), "flow.chapter.one.main");
}

#[test]
fn canonical_runtime_id_rejects_source_family_segment() {
    let err = FlowRuntimeId::canonical("flow.main").expect_err("family segment is not canonical");

    assert_eq!(
        err,
        RuntimeIdError::ReservedFamilySegment {
            family: RuntimeIdFamily::Flow,
            segment: "flow".to_owned(),
        }
    );
}

#[test]
fn wrong_source_family_is_a_structured_diagnostic() {
    let err = FlowRuntimeId::from_source_entity_body("view.main")
        .expect_err("view IDs are not flow runtime targets");

    assert_eq!(
        err,
        RuntimeIdError::WrongSourceFamily {
            expected: RuntimeIdFamily::Flow,
            found: "view".to_owned(),
            value: "view.main".to_owned(),
        }
    );
}

#[test]
fn entry_runtime_ids_have_their_own_path_boundary() {
    let entry =
        EntryRuntimeId::from_source_entity_body("entry.main").expect("entry source ID lowers");

    assert_eq!(entry.canonical_label(), "main");
    assert_eq!(entry.public_label().as_str(), "entry.main");
}

#[test]
fn line_runtime_id_does_not_store_say_prefixed_string() {
    let line = RuntimeLineId::from_source_entity_body("say.main.alice.001")
        .expect("line source ID lowers");

    assert_eq!(line.canonical_label(), "main.alice.001");
    assert_eq!(line.public_label().as_str(), "say.main.alice.001");
    assert_eq!(line.path().segments().len(), 3);
}

#[test]
fn stream_runtime_id_does_not_store_stream_prefixed_string() {
    let stream = StreamRuntimeId::from_source_entity_body("stream.audio.rms")
        .expect("stream source ID lowers");

    assert_eq!(stream.canonical_label(), "audio.rms");
    assert_eq!(stream.public_label().as_str(), "stream.audio.rms");
    assert_eq!(stream.path().segments().len(), 2);
}

#[test]
fn canonical_stream_runtime_id_rejects_stream_family_segment() {
    let err =
        StreamRuntimeId::canonical("stream.rms").expect_err("family segment is not canonical");

    assert_eq!(
        err,
        RuntimeIdError::ReservedFamilySegment {
            family: RuntimeIdFamily::Stream,
            segment: "stream".to_owned(),
        }
    );
}

#[test]
fn relative_references_are_not_runtime_lookup_ids() {
    let path = RuntimeIdPath::from_canonical_str(RuntimeIdFamily::Flow, "next")
        .expect("relative target suffix is a valid path");
    let reference = RuntimeIdReference::new(RuntimeIdReferenceAnchor::Parent(1), path);

    assert_eq!(reference.anchor(), RuntimeIdReferenceAnchor::Parent(1));
    assert_eq!(reference.path().label(), "next");
}
