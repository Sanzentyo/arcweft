use arcweft_core::plan::{EntryRuntimeId, FlowRuntimeId, RuntimeLineId};
use arcweft_core::runtime_id::{RuntimeIdError, RuntimeIdFamily, RuntimePublicLabel};

#[test]
fn source_flow_entity_lowers_to_canonical_runtime_id() {
    let flow = FlowRuntimeId::from_source_entity_body("flow.main").expect("flow source ID lowers");

    assert_eq!(flow.as_str(), "main");
    assert_eq!(flow.public_label().as_str(), "flow.main");
}

#[test]
fn fragment_alias_lowers_to_same_flow_runtime_domain() {
    let fragment =
        FlowRuntimeId::from_source_entity_body("frag.intro").expect("fragment source ID lowers");

    assert_eq!(fragment.as_str(), "intro");
    assert_eq!(fragment.public_label().as_str(), "flow.intro");
}

#[test]
fn public_label_dot_is_not_a_runtime_namespace_selector() {
    let label = RuntimePublicLabel::for_family(RuntimeIdFamily::Flow, "chapter.one.main");

    assert_eq!(label.as_str(), "flow.chapter.one.main");
    assert_eq!(label.into_string(), "flow.chapter.one.main");
}

#[test]
fn canonical_runtime_id_rejects_source_family_prefix() {
    let err = FlowRuntimeId::canonical("flow.main").expect_err("family prefix is not canonical");

    assert_eq!(
        err,
        RuntimeIdError::CanonicalContainsFamilyPrefix {
            family: RuntimeIdFamily::Flow,
            value: "flow.main".to_owned(),
            prefix: "flow.",
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
fn entry_runtime_ids_have_their_own_boundary() {
    let entry =
        EntryRuntimeId::from_source_entity_body("entry.main").expect("entry source ID lowers");

    assert_eq!(entry.as_str(), "main");
    assert_eq!(entry.public_label().as_str(), "entry.main");
}

#[test]
fn line_public_label_preserves_existing_content_id() {
    let line = RuntimeLineId("say.main.alice.001".to_owned());

    assert_eq!(line.as_str(), "say.main.alice.001");
    assert_eq!(line.public_label().as_str(), "say.main.alice.001");
}
