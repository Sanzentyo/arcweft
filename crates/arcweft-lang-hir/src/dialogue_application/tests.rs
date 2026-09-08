use core::num::{NonZeroU32, NonZeroU64};

use super::{
    HirDialogueContent, HirDialogueContentId, HirDialogueControl, HirDialogueMarkName,
    HirDialogueNode, HirDialogueNodeId, HirDialogueNodeKind, HirDialoguePointAction,
    HirDialoguePointActionArgument, HirDialoguePointActionArgumentId,
    HirDialoguePointActionIdentity, HirDialoguePointActionPayload, HirRawLiteralBody,
    HirRichTextHostEvent, HirRichTextValue,
};
use crate::identity::{ExprId, HirDatabaseId, HirModuleId, HirTypedId, RawHirId};
use crate::leaf::HirIdSuffix;

fn module(database: u64, slot: u32) -> HirModuleId {
    HirModuleId::new(
        HirDatabaseId::from_raw_for_test(NonZeroU64::new(database).unwrap()),
        NonZeroU32::new(slot).unwrap(),
    )
}

fn typed_id<I: HirTypedId>(module: HirModuleId, slot: u32) -> I {
    I::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).unwrap(),
        I::KIND,
    ))
}

fn mark_name(value: &str) -> HirDialogueMarkName {
    HirDialogueMarkName::new(HirIdSuffix::try_new(value.into()).unwrap())
}

#[test]
fn point_action_owns_typed_payload_and_arguments() {
    let module = module(1, 1);
    let owner = typed_id::<ExprId>(module, 1);
    let content = HirDialogueContentId::new(owner);
    let node = HirDialogueNodeId::try_new(content, 0).unwrap();
    let argument = HirDialoguePointActionArgument::named(
        HirDialoguePointActionArgumentId::try_new(node, 0).unwrap(),
        "tone".into(),
        HirRichTextValue::new("calm".into()),
    );
    let action = HirDialoguePointAction::try_new(
        node,
        HirDialoguePointActionIdentity::Control(HirDialogueControl::Page),
        Box::new([argument]),
        HirDialoguePointActionPayload::None,
    )
    .unwrap();

    assert!(matches!(
        action.identity(),
        HirDialoguePointActionIdentity::Control(HirDialogueControl::Page)
    ));
    assert_eq!(action.arguments()[0].name(), Some("tone"));
    assert_eq!(action.arguments()[0].value().unwrap().as_str(), "calm");
    assert_eq!(action.payload(), HirDialoguePointActionPayload::None);
}

#[test]
fn point_action_payloads_keep_call_and_timed_cue_expression_owners() {
    let module = module(4, 1);
    let call_owner = typed_id::<ExprId>(module, 10);
    let call_expression = typed_id::<ExprId>(module, 11);
    let call_content = HirDialogueContentId::new(call_owner);
    let call_node = HirDialogueNodeId::try_new(call_content, 0).unwrap();
    let call_action = HirDialoguePointAction::try_new(
        call_node,
        HirDialoguePointActionIdentity::Host(HirRichTextHostEvent::Call),
        Box::new([]),
        HirDialoguePointActionPayload::Call(call_expression),
    )
    .unwrap();
    assert_eq!(
        call_action.payload(),
        HirDialoguePointActionPayload::Call(call_expression)
    );
    assert_eq!(call_action.payload().expression(), Some(call_expression));

    let timed_owner = typed_id::<ExprId>(module, 20);
    let timed_expression = typed_id::<ExprId>(module, 21);
    let timed_content = HirDialogueContentId::new(timed_owner);
    let timed_node = HirDialogueNodeId::try_new(timed_content, 0).unwrap();
    let duration = HirDialoguePointActionArgument::positional(
        HirDialoguePointActionArgumentId::try_new(timed_node, 0).unwrap(),
        HirRichTextValue::new("120ms".into()),
    );
    let timed_action = HirDialoguePointAction::try_new(
        timed_node,
        HirDialoguePointActionIdentity::Host(HirRichTextHostEvent::TimedCue),
        Box::new([duration]),
        HirDialoguePointActionPayload::TimedCue(timed_expression),
    )
    .unwrap();
    assert_eq!(
        timed_action.arguments()[0].value().unwrap().as_str(),
        "120ms"
    );
    assert_eq!(
        timed_action.payload(),
        HirDialoguePointActionPayload::TimedCue(timed_expression)
    );
    assert_eq!(timed_action.payload().expression(), Some(timed_expression));
}

#[test]
fn marker_catalog_is_minted_from_point_actions() {
    let module = module(2, 1);
    let owner = typed_id::<ExprId>(module, 1);
    let content_id = HirDialogueContentId::new(owner);
    let node_id = HirDialogueNodeId::try_new(content_id, 0).unwrap();
    let name = mark_name("checkpoint");
    let action = HirDialoguePointAction::try_new(
        node_id,
        HirDialoguePointActionIdentity::Mark(name.clone()),
        Box::new([]),
        HirDialoguePointActionPayload::None,
    )
    .unwrap();
    let content = HirDialogueContent::try_new(
        content_id,
        Box::new([HirDialogueNode::new(
            node_id,
            HirDialogueNodeKind::PointAction(action),
        )]),
        Box::new([(node_id, name.clone())]),
    )
    .unwrap();

    assert_eq!(content.marks().len(), 1);
    assert_eq!(content.marks()[0].name(), &name);
    assert_eq!(content.marks()[0].action(), node_id);
}

#[test]
fn raw_literal_content_is_opaque_and_node_free() {
    let module = module(3, 1);
    let owner = typed_id::<ExprId>(module, 1);
    let content = HirDialogueContent::try_new_raw_literal(
        HirDialogueContentId::new(owner),
        HirRawLiteralBody::new("a[b][p]".into()),
    )
    .unwrap();

    assert!(content.nodes().is_empty());
    assert_eq!(content.raw_literal().unwrap().as_str(), "a[b][p]");
    assert!(content.marks().is_empty());
}
