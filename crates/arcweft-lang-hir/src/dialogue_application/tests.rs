use core::num::{NonZeroU32, NonZeroU64};

use super::*;
use crate::expr::{
    HirCallArgument, HirCallArgumentListTerminator, HirCallCallee, HirCallChildPoison,
    HirCallChildStates, HirCallExpr, HirCallTypeApplication,
};
use crate::identity::{HirDatabaseId, HirIdKind, HirTypedId, PatternId, RawHirId};
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

fn name(value: &str) -> HirName {
    HirName::try_new(value.into()).unwrap()
}

fn suffix(value: &str) -> HirIdSuffix {
    HirIdSuffix::try_new(value.into()).unwrap()
}

fn empty_content(owner: ExprId) -> HirDialogueContent {
    HirDialogueContent::try_new(
        HirDialogueContentId::new(owner),
        Box::new([]),
        Box::new([]),
        Box::new([]),
    )
    .unwrap()
}

fn call_fixture(module: HirModuleId, arguments: Box<[HirCallArgument]>) -> HirCallExpr {
    let argument_states = vec![HirCallChildPoison::Clean; arguments.len()];
    let (call, state) = HirCallExpr::try_new(
        HirCallCallee::value(typed_id(module, 90)),
        HirCallTypeApplication::absent(),
        arguments,
        HirCallArgumentListTerminator::Closed,
        HirCallChildStates::new(HirCallChildPoison::Clean, &argument_states, &[]),
        false,
    )
    .expect("clean call");
    let _ = state;
    call
}

#[derive(Default)]
struct RecordingContext {
    requirements: Vec<HirDialogueTransactionRequirement>,
}

impl HirDialogueTransactionContext for RecordingContext {
    type Error = ();

    fn require(
        &mut self,
        requirement: HirDialogueTransactionRequirement,
    ) -> Result<(), Self::Error> {
        self.requirements.push(requirement);
        Ok(())
    }
}

#[test]
fn immediate_target_coordinates_preserve_duplicates_and_authored_ordinals() {
    let module = module(1, 1);
    let arguments = [
        HirCallArgument::positional(typed_id(module, 1)),
        HirCallArgument::named(name("id"), typed_id(module, 2)),
        HirCallArgument::named(name("other"), typed_id(module, 3)),
        HirCallArgument::named(name("id"), typed_id(module, 4)),
        HirCallArgument::named(name("text_key"), typed_id(module, 5)),
        HirCallArgument::spread(typed_id(module, 6)),
    ];

    let coordinates = HirDialogueCoordinate::from_immediate_arguments(&arguments).unwrap();
    assert_eq!(coordinates.len(), 3);
    assert_eq!(coordinates[0].kind(), HirDialogueCoordinateKind::Id);
    assert_eq!(coordinates[0].argument().get(), 1);
    assert_eq!(coordinates[0].value(), typed_id(module, 2));
    assert_eq!(coordinates[1].kind(), HirDialogueCoordinateKind::Id);
    assert_eq!(coordinates[1].argument().get(), 3);
    assert_eq!(coordinates[1].value(), typed_id(module, 4));
    assert_eq!(coordinates[2].kind(), HirDialogueCoordinateKind::TextKey);
    assert_eq!(coordinates[2].argument().get(), 4);
    assert_eq!(coordinates[2].value(), typed_id(module, 5));
}

#[test]
fn metadata_projection_requires_exact_ordinal_identity_kind_and_uniqueness() {
    let module = module(8, 1);
    let owner = typed_id(module, 1);
    let target = typed_id(module, 2);
    let id = typed_id(module, 3);
    let text_key = typed_id(module, 4);
    let other = typed_id(module, 5);
    let call = call_fixture(
        module,
        Box::new([
            HirCallArgument::named(name("id"), id),
            HirCallArgument::named(name("text_key"), text_key),
        ]),
    );
    let application = |coordinates| {
        HirDialogueContentApplication::try_new(
            owner,
            target,
            empty_content(owner),
            None,
            coordinates,
        )
        .expect("application fixture")
    };
    let exact =
        application(HirDialogueCoordinate::from_immediate_arguments(call.arguments()).unwrap());
    assert_eq!(
        validate_application_metadata_projection(&exact, &call)
            .expect("exact projection")
            .as_ref(),
        exact.coordinates()
    );

    let wrong_ordinal = application(Box::new([HirDialogueCoordinate::new(
        HirDialogueCoordinateKind::Id,
        HirCallArgumentOrdinal::try_new(2).unwrap(),
        id,
    )]));
    assert_eq!(
        validate_application_metadata_projection(&wrong_ordinal, &call),
        Err(HirDialogueApplicationMetadataProjectionError::ArgumentOrdinalMismatch)
    );

    let wrong_identity = application(Box::new([HirDialogueCoordinate::new(
        HirDialogueCoordinateKind::Id,
        HirCallArgumentOrdinal::try_new(0).unwrap(),
        other,
    )]));
    assert_eq!(
        validate_application_metadata_projection(&wrong_identity, &call),
        Err(HirDialogueApplicationMetadataProjectionError::ArgumentIdentityMismatch)
    );

    let wrong_kind = application(Box::new([HirDialogueCoordinate::new(
        HirDialogueCoordinateKind::TextKey,
        HirCallArgumentOrdinal::try_new(0).unwrap(),
        id,
    )]));
    assert_eq!(
        validate_application_metadata_projection(&wrong_kind, &call),
        Err(HirDialogueApplicationMetadataProjectionError::CoordinateKindMismatch)
    );

    let duplicate_call = call_fixture(
        module,
        Box::new([
            HirCallArgument::named(name("id"), id),
            HirCallArgument::named(name("id"), text_key),
        ]),
    );
    let duplicate = application(
        HirDialogueCoordinate::from_immediate_arguments(duplicate_call.arguments()).unwrap(),
    );
    assert_eq!(
        validate_application_metadata_projection(&duplicate, &duplicate_call),
        Err(HirDialogueApplicationMetadataProjectionError::DuplicateCoordinate)
    );
}

#[test]
fn dialogue_content_owns_contiguous_node_tag_and_argument_ids() {
    let module = module(2, 1);
    let owner = typed_id(module, 1);
    let content_id = HirDialogueContentId::new(owner);
    let tag_id = HirRichTextTagId::try_new(content_id, 0).unwrap();
    let argument_id = HirRichTextArgumentId::try_new(tag_id, 0).unwrap();
    let tag = HirRichTextTag::try_new(
        tag_id,
        HirRichTextTagIdentity::Builtin(HirBuiltinRichTextTag::Page),
        vec![HirRichTextArgument::named(
            argument_id,
            name("voice"),
            HirRichTextValue::new("alice".into()),
        )]
        .into_boxed_slice(),
        HirRichTextTagPayload::Arguments,
    )
    .unwrap();
    let nodes = vec![
        HirDialogueNode::new(
            HirDialogueNodeId::try_new(content_id, 0).unwrap(),
            HirDialogueNodeKind::Text(HirTextFragment::new("hello".into())),
        ),
        HirDialogueNode::new(
            HirDialogueNodeId::try_new(content_id, 1).unwrap(),
            HirDialogueNodeKind::AuthoredStartTag(tag_id),
        ),
    ]
    .into_boxed_slice();

    let content =
        HirDialogueContent::try_new(content_id, nodes, Box::new([tag]), Box::new([])).unwrap();
    assert_eq!(content.id(), content_id);
    assert_eq!(content.nodes()[1].id().ordinal(), 1);
    assert_eq!(content.tags()[0].arguments()[0].id().ordinal(), 0);

    let wrong_node = HirDialogueNode::new(
        HirDialogueNodeId::try_new(content_id, 1).unwrap(),
        HirDialogueNodeKind::Text(HirTextFragment::new("gap".into())),
    );
    assert_eq!(
        HirDialogueContent::try_new(
            content_id,
            Box::new([wrong_node]),
            Box::new([]),
            Box::new([]),
        ),
        Err(HirDialogueInvariantError::NonContiguousNodeOrdinal)
    );

    let foreign_content = HirDialogueContentId::new(typed_id(module, 20));
    let foreign_tag = HirRichTextTagId::try_new(foreign_content, 0).unwrap();
    let bad_reference = HirDialogueNode::new(
        HirDialogueNodeId::try_new(content_id, 0).unwrap(),
        HirDialogueNodeKind::AuthoredStartTag(foreign_tag),
    );
    assert_eq!(
        HirDialogueContent::try_new(
            content_id,
            Box::new([bad_reference]),
            Box::new([]),
            Box::new([]),
        ),
        Err(HirDialogueInvariantError::InvalidTagReference)
    );
}

#[test]
fn dialogue_content_mints_marker_ids_and_joins_tags_to_catalog_rows() {
    let module = module(21, 1);
    let owner = typed_id(module, 1);
    let content_id = HirDialogueContentId::new(owner);
    let first_tag = HirRichTextTagId::try_new(content_id, 0).unwrap();
    let second_tag = HirRichTextTagId::try_new(content_id, 1).unwrap();
    let first = HirRichTextTag::try_new(
        first_tag,
        HirRichTextTagIdentity::Marker,
        Box::new([]),
        HirRichTextTagPayload::None,
    )
    .unwrap();
    let second = HirRichTextTag::try_new(
        second_tag,
        HirRichTextTagIdentity::Marker,
        Box::new([]),
        HirRichTextTagPayload::None,
    )
    .unwrap();

    let content = HirDialogueContent::try_new(
        content_id,
        Box::new([]),
        Box::new([first, second]),
        Box::new([
            (first_tag, HirDialogueMarkName::new(suffix("checkpoint"))),
            (second_tag, HirDialogueMarkName::new(suffix("release"))),
        ]),
    )
    .unwrap();

    assert_eq!(content.marks().len(), 2);
    assert_eq!(content.marks()[0].id().content(), content_id);
    assert_eq!(content.marks()[0].id().ordinal().get(), 0);
    assert_eq!(content.marks()[1].id().ordinal().get(), 1);
    assert_eq!(content.marks()[0].tag(), first_tag);
    assert_eq!(content.marks()[1].tag(), second_tag);
    assert!(matches!(
        content.tags()[0].payload(),
        HirRichTextTagPayload::Marker(mark) if *mark == content.marks()[0].id()
    ));
    assert!(matches!(
        content.tags()[1].payload(),
        HirRichTextTagPayload::Marker(mark) if *mark == content.marks()[1].id()
    ));
}

#[test]
fn dialogue_content_rejects_duplicate_marker_names_transactionally() {
    let module = module(22, 1);
    let owner = typed_id(module, 1);
    let content_id = HirDialogueContentId::new(owner);
    let first_tag = HirRichTextTagId::try_new(content_id, 0).unwrap();
    let second_tag = HirRichTextTagId::try_new(content_id, 1).unwrap();
    let marker = |tag| {
        HirRichTextTag::try_new(
            tag,
            HirRichTextTagIdentity::Marker,
            Box::new([]),
            HirRichTextTagPayload::None,
        )
        .unwrap()
    };

    assert_eq!(
        HirDialogueContent::try_new(
            content_id,
            Box::new([]),
            Box::new([marker(first_tag), marker(second_tag)]),
            Box::new([
                (first_tag, HirDialogueMarkName::new(suffix("same"))),
                (second_tag, HirDialogueMarkName::new(suffix("same"))),
            ]),
        ),
        Err(HirDialogueInvariantError::DuplicateMarkName)
    );
}

#[test]
fn dialogue_content_rejects_marker_inputs_out_of_source_order() {
    let module = module(23, 1);
    let owner = typed_id(module, 1);
    let content_id = HirDialogueContentId::new(owner);
    let first_tag = HirRichTextTagId::try_new(content_id, 0).unwrap();
    let second_tag = HirRichTextTagId::try_new(content_id, 1).unwrap();
    let marker = |tag| {
        HirRichTextTag::try_new(
            tag,
            HirRichTextTagIdentity::Marker,
            Box::new([]),
            HirRichTextTagPayload::None,
        )
        .unwrap()
    };

    assert_eq!(
        HirDialogueContent::try_new(
            content_id,
            Box::new([]),
            Box::new([marker(first_tag), marker(second_tag)]),
            Box::new([
                (second_tag, HirDialogueMarkName::new(suffix("second"))),
                (first_tag, HirDialogueMarkName::new(suffix("first"))),
            ]),
        ),
        Err(HirDialogueInvariantError::NonContiguousMarkOrdinal)
    );
}

#[test]
fn dialogue_content_rejects_forged_marker_payloads_and_catalog_overflow() {
    let module = module(24, 1);
    let owner = typed_id(module, 1);
    let content_id = HirDialogueContentId::new(owner);
    let first_tag = HirRichTextTagId::try_new(content_id, 0).unwrap();
    let second_tag = HirRichTextTagId::try_new(content_id, 1).unwrap();
    let foreign_content_id = HirDialogueContentId::new(typed_id(module, 2));
    let foreign_tag = HirRichTextTagId::try_new(foreign_content_id, 0).unwrap();
    let foreign_marker = HirDialogueContent::try_new(
        foreign_content_id,
        Box::new([]),
        Box::new([HirRichTextTag::try_new(
            foreign_tag,
            HirRichTextTagIdentity::Marker,
            Box::new([]),
            HirRichTextTagPayload::None,
        )
        .unwrap()]),
        Box::new([(foreign_tag, HirDialogueMarkName::new(suffix("foreign")))]),
    )
    .unwrap()
    .marks()[0]
        .id();

    assert_eq!(
        HirRichTextTag::try_new(
            first_tag,
            HirRichTextTagIdentity::Marker,
            Box::new([]),
            HirRichTextTagPayload::Marker(foreign_marker),
        ),
        Err(HirDialogueInvariantError::InvalidMarkReference)
    );
    let argument_id = HirRichTextArgumentId::try_new(first_tag, 0).unwrap();
    assert_eq!(
        HirRichTextTag::try_new(
            first_tag,
            HirRichTextTagIdentity::Marker,
            Box::new([HirRichTextArgument::positional(
                argument_id,
                HirRichTextValue::new("extra".into()),
            )]),
            HirRichTextTagPayload::None,
        ),
        Err(HirDialogueInvariantError::InvalidMarkReference)
    );

    let marker = || {
        HirRichTextTag::try_new(
            first_tag,
            HirRichTextTagIdentity::Marker,
            Box::new([]),
            HirRichTextTagPayload::None,
        )
        .unwrap()
    };
    assert_eq!(
        HirDialogueContent::try_new(
            content_id,
            Box::new([]),
            Box::new([marker()]),
            Box::new([
                (first_tag, HirDialogueMarkName::new(suffix("first"))),
                (second_tag, HirDialogueMarkName::new(suffix("second"))),
            ]),
        ),
        Err(HirDialogueInvariantError::InvalidMarkReference),
        "an N+1 catalog row must fail before any content value is published"
    );
    assert_eq!(
        HirDialogueContent::try_new(
            content_id,
            Box::new([]),
            Box::new([marker()]),
            Box::new([
                (first_tag, HirDialogueMarkName::new(suffix("first"))),
                (first_tag, HirDialogueMarkName::new(suffix("duplicate"))),
            ]),
        ),
        Err(HirDialogueInvariantError::InvalidMarkReference)
    );
}

#[test]
fn dialogue_mark_catalog_limit_accepts_exact_n_and_rejects_n_plus_one() {
    fn inputs(
        content: HirDialogueContentId,
        count: usize,
    ) -> (
        Box<[HirRichTextTag]>,
        Box<[(HirRichTextTagId, HirDialogueMarkName)]>,
    ) {
        let mut tags = Vec::new();
        let mut marks = Vec::new();
        for ordinal in 0..count {
            let tag = HirRichTextTagId::try_new(content, ordinal).expect("mark tag ordinal");
            tags.push(
                HirRichTextTag::try_new(
                    tag,
                    HirRichTextTagIdentity::Marker,
                    Box::new([]),
                    HirRichTextTagPayload::None,
                )
                .expect("unminted marker tag"),
            );
            marks.push((
                tag,
                HirDialogueMarkName::new(suffix(&format!("mark_{ordinal}"))),
            ));
        }
        (tags.into_boxed_slice(), marks.into_boxed_slice())
    }

    let content = HirDialogueContentId::new(typed_id(module(25, 1), 1));
    let (tags, marks) = inputs(content, 2);
    let exact =
        HirDialogueContent::try_new_with_mark_limit_for_test(content, Box::new([]), tags, marks, 2)
            .expect("exact mark bound publishes complete content");
    assert_eq!(exact.marks().len(), 2);
    assert!(exact.tags().iter().all(|tag| matches!(
        tag.payload(),
        HirRichTextTagPayload::Marker(mark) if mark.content() == content
    )));

    let (tags, marks) = inputs(content, 3);
    assert_eq!(
        HirDialogueContent::try_new_with_mark_limit_for_test(content, Box::new([]), tags, marks, 2,),
        Err(HirDialogueInvariantError::MarkCatalogLimitExceeded {
            observed: 3,
            maximum: 2,
        }),
        "N+1 must return no partially minted content"
    );
}

#[test]
fn application_requires_exact_content_owner_module_and_coordinate_order() {
    let expected_module = module(3, 1);
    let owner = typed_id(expected_module, 1);
    let target = typed_id(expected_module, 2);
    let later = HirDialogueCoordinate::new(
        HirDialogueCoordinateKind::Id,
        HirCallArgumentOrdinal::try_new(2).unwrap(),
        typed_id(expected_module, 3),
    );
    let earlier = HirDialogueCoordinate::new(
        HirDialogueCoordinateKind::TextKey,
        HirCallArgumentOrdinal::try_new(1).unwrap(),
        typed_id(expected_module, 4),
    );
    assert_eq!(
        HirDialogueContentApplication::try_new(
            owner,
            target,
            empty_content(owner),
            None,
            Box::new([later, earlier]),
        ),
        Err(HirDialogueInvariantError::UnorderedCoordinates)
    );

    let other_owner = typed_id(expected_module, 9);
    assert_eq!(
        HirDialogueContentApplication::try_new(
            owner,
            target,
            empty_content(other_owner),
            None,
            Box::new([]),
        ),
        Err(HirDialogueInvariantError::InvalidContentOwner)
    );

    let foreign = module(4, 1);
    assert!(matches!(
        HirDialogueContentApplication::try_new(
            owner,
            typed_id(foreign, 1),
            empty_content(owner),
            None,
            Box::new([]),
        ),
        Err(HirDialogueInvariantError::ForeignChild { expected, actual })
            if expected == expected_module && actual == foreign
    ));
}

#[test]
fn rich_text_calls_share_expr_ids_and_report_call_kind_requirements() {
    let module = module(5, 1);
    let owner = typed_id(module, 1);
    let target = typed_id(module, 2);
    let fx_call = typed_id(module, 3);
    let dialogue_call = typed_id(module, 4);
    let interpolation = typed_id(module, 5);
    let content_id = HirDialogueContentId::new(owner);

    let fx_tag_id = HirRichTextTagId::try_new(content_id, 0).unwrap();
    let dialogue_tag_id = HirRichTextTagId::try_new(content_id, 1).unwrap();
    let fx_tag = HirRichTextTag::try_new(
        fx_tag_id,
        HirRichTextTagIdentity::Builtin(HirBuiltinRichTextTag::Fx(HirBuiltinRichTextFx::Wave)),
        Box::new([]),
        HirRichTextTagPayload::FxCall(fx_call),
    )
    .unwrap();
    let dialogue_tag = HirRichTextTag::try_new(
        dialogue_tag_id,
        HirRichTextTagIdentity::Builtin(HirBuiltinRichTextTag::HostEvent(
            HirRichTextHostEvent::Call,
        )),
        Box::new([]),
        HirRichTextTagPayload::DialogueCall(dialogue_call),
    )
    .unwrap();
    let nodes = Box::new([
        HirDialogueNode::new(
            HirDialogueNodeId::try_new(content_id, 0).unwrap(),
            HirDialogueNodeKind::AuthoredStartTag(fx_tag_id),
        ),
        HirDialogueNode::new(
            HirDialogueNodeId::try_new(content_id, 1).unwrap(),
            HirDialogueNodeKind::AuthoredStartTag(dialogue_tag_id),
        ),
        HirDialogueNode::new(
            HirDialogueNodeId::try_new(content_id, 2).unwrap(),
            HirDialogueNodeKind::Interpolation(interpolation),
        ),
    ]);
    let content = HirDialogueContent::try_new(
        content_id,
        nodes,
        Box::new([fx_tag, dialogue_tag]),
        Box::new([]),
    )
    .unwrap();
    let application =
        HirDialogueContentApplication::try_new(owner, target, content, None, Box::new([])).unwrap();
    let mut validation = RecordingContext::default();
    application.validate_transaction(&mut validation).unwrap();

    assert!(
        validation
            .requirements
            .contains(&HirDialogueTransactionRequirement::Expression {
                id: fx_call,
                expected: HirDialogueExpressionExpectation::Call,
            })
    );
    assert!(
        validation
            .requirements
            .contains(&HirDialogueTransactionRequirement::Expression {
                id: dialogue_call,
                expected: HirDialogueExpressionExpectation::Call,
            })
    );
    assert!(
        validation
            .requirements
            .contains(&HirDialogueTransactionRequirement::Expression {
                id: interpolation,
                expected: HirDialogueExpressionExpectation::Unrestricted,
            })
    );
    assert!(
        validation
            .requirements
            .contains(&HirDialogueTransactionRequirement::RichTextCharge(
                HirRichTextCharge::ContentTags { observed: 2 },
            ))
    );
}

#[test]
fn postfix_candidates_report_distinct_interpretation_requirements() {
    let module = module(6, 1);
    let target = typed_id(module, 1);
    let index = typed_id(module, 2);
    let dialogue = typed_id(module, 3);
    let postfix = HirPostfixBracket::try_new(
        target,
        HirPostfixBracketCandidates::Ambiguous { index, dialogue },
    )
    .unwrap();
    let mut context = RecordingContext::default();
    let owner = typed_id(module, 4);
    postfix.validate_transaction(owner, &mut context).unwrap();

    assert!(
        context
            .requirements
            .contains(&HirDialogueTransactionRequirement::Expression {
                id: index,
                expected: HirDialogueExpressionExpectation::PostfixIndexCandidate {
                    owner,
                    role: SyntheticRole::PostfixIndexCandidateExpression,
                    target,
                },
            })
    );
    assert!(
        context
            .requirements
            .contains(&HirDialogueTransactionRequirement::Expression {
                id: dialogue,
                expected: HirDialogueExpressionExpectation::DialogueContentCandidate {
                    owner,
                    role: SyntheticRole::DialogueContentCandidateExpression,
                    target,
                },
            })
    );
    assert!(!postfix.has_recovery());

    let inherited_role = SyntheticRole::PostfixIndexCandidateExpression;
    let mut nested_context = RecordingContext::default();
    postfix
        .validate_candidate_transaction(owner, inherited_role, &mut nested_context)
        .unwrap();
    assert!(
        nested_context
            .requirements
            .contains(&HirDialogueTransactionRequirement::Expression {
                id: index,
                expected: HirDialogueExpressionExpectation::PostfixIndexCandidate {
                    owner,
                    role: inherited_role,
                    target,
                },
            })
    );
    assert!(
        nested_context
            .requirements
            .contains(&HirDialogueTransactionRequirement::Expression {
                id: dialogue,
                expected: HirDialogueExpressionExpectation::DialogueContentCandidate {
                    owner,
                    role: inherited_role,
                    target,
                },
            })
    );

    assert_eq!(
        HirPostfixBracket::try_new(
            target,
            HirPostfixBracketCandidates::Ambiguous {
                index: target,
                dialogue,
            },
        ),
        Err(HirDialogueInvariantError::InvalidPostfixCandidate)
    );
}

#[test]
fn line_plan_uses_hir_owned_policy_and_existing_child_arenas() {
    let module = module(7, 1);
    let scope = typed_id(module, 1);
    let statement = typed_id(module, 2);
    let statement_two = typed_id(module, 3);
    let plan = HirLinePlan::try_new(
        scope,
        Some(name("reveal")),
        vec![
            HirLinePlanItem::Init(Box::new([statement])),
            HirLinePlanItem::Statement(statement_two),
        ]
        .into_boxed_slice(),
    )
    .unwrap();
    let mut context = RecordingContext::default();
    plan.validate_transaction(&mut context).unwrap();

    assert_eq!(plan.root_scope(), scope);
    assert_eq!(plan.label().map(HirName::as_str), Some("reveal"));
    assert!(
        context
            .requirements
            .contains(&HirDialogueTransactionRequirement::Scope(scope))
    );
    assert!(
        context
            .requirements
            .contains(&HirDialogueTransactionRequirement::Statement(statement))
    );
    assert!(
        context
            .requirements
            .contains(&HirDialogueTransactionRequirement::Statement(statement))
    );
}

#[test]
fn recovery_is_explicit_without_defaulting_invalid_values() {
    let module = module(8, 1);
    let owner = typed_id(module, 1);
    let content_id = HirDialogueContentId::new(owner);
    let error_node = HirDialogueNode::new(
        HirDialogueNodeId::try_new(content_id, 0).unwrap(),
        HirDialogueNodeKind::Error(HirDialogueContentError::InvalidEscape),
    );
    let content = HirDialogueContent::try_new(
        content_id,
        Box::new([error_node]),
        Box::new([]),
        Box::new([]),
    )
    .unwrap();
    let application = HirDialogueContentApplication::try_new(
        owner,
        typed_id(module, 2),
        content,
        None,
        Box::new([]),
    )
    .unwrap();
    assert!(application.has_recovery());

    let invalid = HirRichTextArgument::invalid(
        HirRichTextArgumentId::try_new(HirRichTextTagId::try_new(content_id, 0).unwrap(), 0)
            .unwrap(),
        HirRichTextArgumentIssue::MissingValue,
    );
    assert_eq!(invalid.value(), None);
    assert_eq!(
        invalid.issue(),
        Some(HirRichTextArgumentIssue::MissingValue)
    );

    let invalid_postfix = HirPostfixBracket::try_new(
        typed_id(module, 3),
        HirPostfixBracketCandidates::Invalid {
            index: HirPostfixCandidateFailure::new(HirPostfixCandidateFailureKind::MissingOperand),
            dialogue: HirPostfixCandidateFailure::new(
                HirPostfixCandidateFailureKind::InvalidDialogueAtom,
            ),
        },
    )
    .unwrap();
    assert!(invalid_postfix.has_recovery());
}

#[test]
fn rich_text_id_ordinals_enforce_their_closed_domains() {
    let module = module(9, 1);
    let content = HirDialogueContentId::new(typed_id(module, 1));
    let tag = HirRichTextTagId::try_new(content, 0).unwrap();
    assert_eq!(
        HirRichTextArgumentId::try_new(tag, 32),
        Err(HirDialogueOrdinalError::Argument { ordinal: 32 })
    );
    if usize::BITS > u32::BITS {
        let too_large = u32::MAX as usize + 1;
        assert_eq!(
            HirDialogueNodeId::try_new(content, too_large),
            Err(HirDialogueOrdinalError::Node { ordinal: too_large })
        );
        assert_eq!(
            HirRichTextTagId::try_new(content, too_large),
            Err(HirDialogueOrdinalError::Tag { ordinal: too_large })
        );
    }
}

#[test]
fn closed_rich_text_inventories_have_contract_cardinalities() {
    let direct = [
        HirRichTextDirectStyle::Emphasis,
        HirRichTextDirectStyle::Strong,
        HirRichTextDirectStyle::Italic,
        HirRichTextDirectStyle::Oblique,
        HirRichTextDirectStyle::Color,
        HirRichTextDirectStyle::Font,
        HirRichTextDirectStyle::Size,
        HirRichTextDirectStyle::Ruby,
    ];
    let style = [
        HirRichTextStyleSelector::Italic,
        HirRichTextStyleSelector::Oblique,
        HirRichTextStyleSelector::Opacity,
        HirRichTextStyleSelector::Layer,
        HirRichTextStyleSelector::ZIndex,
    ];
    let layout = [
        HirRichTextLayoutSelector::HorizontalTb,
        HirRichTextLayoutSelector::VerticalRl,
        HirRichTextLayoutSelector::VerticalLr,
        HirRichTextLayoutSelector::Direction,
        HirRichTextLayoutSelector::RubyOver,
        HirRichTextLayoutSelector::RubyUnder,
        HirRichTextLayoutSelector::RubyInterCharacter,
    ];
    let transform = [
        HirRichTextTransformSelector::Offset,
        HirRichTextTransformSelector::Rotate,
        HirRichTextTransformSelector::Scale,
        HirRichTextTransformSelector::Skew,
    ];
    let fx = [
        HirBuiltinRichTextFx::Wave,
        HirBuiltinRichTextFx::Shake,
        HirBuiltinRichTextFx::Jitter,
        HirBuiltinRichTextFx::Arc,
        HirBuiltinRichTextFx::Spin,
        HirBuiltinRichTextFx::Pulse,
        HirBuiltinRichTextFx::Motion,
        HirBuiltinRichTextFx::Typewriter,
        HirBuiltinRichTextFx::Sparkle,
        HirBuiltinRichTextFx::Shader,
    ];
    assert_eq!(
        (
            direct.len(),
            style.len(),
            layout.len(),
            transform.len(),
            fx.len()
        ),
        (8, 5, 7, 4, 10)
    );
}

#[test]
fn typed_id_helpers_use_the_expected_arena_kinds() {
    let module = module(10, 1);
    assert_eq!(typed_id::<ExprId>(module, 1).kind(), HirIdKind::Expr);
    assert_eq!(typed_id::<StmtId>(module, 2).kind(), HirIdKind::Stmt);
    assert_eq!(typed_id::<PatternId>(module, 3).kind(), HirIdKind::Pattern);
    assert_eq!(typed_id::<ScopeId>(module, 4).kind(), HirIdKind::Scope);
}
