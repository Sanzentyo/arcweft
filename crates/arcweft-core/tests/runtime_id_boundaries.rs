use arcweft_core::plan::{
    EntryRuntimeId, FlowRuntimeId, RuntimeFlowSeed, RuntimeFlowTargetError, RuntimeLineId,
    RuntimePlan, RuntimePlanBuilder,
};
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
fn checked_flow_identity_is_one_way_and_keeps_public_label_separate() {
    let left = FlowRuntimeId::from_checked_declaration_digest([0x11; 32], "flow.opening")
        .expect("accepted Flow public label");
    let right = FlowRuntimeId::from_checked_declaration_digest([0x22; 32], "flow.opening")
        .expect("accepted Flow public label");

    assert_ne!(left, right);
    assert_eq!(left.public_label().as_str(), "flow.opening");
    assert_eq!(right.public_label().as_str(), "flow.opening");
    assert!(left.canonical_label().starts_with("__checked_flow."));
    assert!(right.canonical_label().starts_with("__checked_flow."));
    assert_ne!(left.canonical_label(), right.canonical_label());
}

#[test]
fn dynamic_flow_target_selects_one_accepted_identity_or_reports_label_ambiguity() {
    let left = FlowRuntimeId::from_checked_declaration_digest([0x11; 32], "flow.opening")
        .expect("accepted Flow public label");
    let right = FlowRuntimeId::from_checked_declaration_digest([0x22; 32], "flow.opening")
        .expect("accepted Flow public label");
    let one = plan_with_flows([left.clone()]);
    assert_eq!(
        one.resolve_flow_target_value("flow.opening"),
        Ok(left.clone())
    );

    let ambiguous = plan_with_flows([left, right]);
    assert_eq!(
        ambiguous.resolve_flow_target_value("flow.opening"),
        Err(RuntimeFlowTargetError::Ambiguous {
            target: "flow.opening".to_owned(),
            matches: 2,
        })
    );
}

fn plan_with_flows(flows: impl IntoIterator<Item = FlowRuntimeId>) -> RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    for flow in flows {
        builder
            .push_flow_seed(RuntimeFlowSeed::new(flow, [], Vec::new()))
            .expect("test Flow admits");
    }
    builder.finish().expect("test runtime plan seals")
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
