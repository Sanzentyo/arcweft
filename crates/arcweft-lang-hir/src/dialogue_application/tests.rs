use core::num::{NonZeroU32, NonZeroU64};

use super::*;
use crate::expr::HirCallArgument;
use crate::identity::{HirDatabaseId, HirIdKind, HirTypedId, RawHirId};

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

fn empty_content(owner: ExprId) -> HirDialogueContent {
    HirDialogueContent::try_new(HirDialogueContentId::new(owner), Box::new([]), Box::new([]))
        .unwrap()
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

    let content = HirDialogueContent::try_new(content_id, nodes, Box::new([tag])).unwrap();
    assert_eq!(content.id(), content_id);
    assert_eq!(content.nodes()[1].id().ordinal(), 1);
    assert_eq!(content.tags()[0].arguments()[0].id().ordinal(), 0);

    let wrong_node = HirDialogueNode::new(
        HirDialogueNodeId::try_new(content_id, 1).unwrap(),
        HirDialogueNodeKind::Text(HirTextFragment::new("gap".into())),
    );
    assert_eq!(
        HirDialogueContent::try_new(content_id, Box::new([wrong_node]), Box::new([])),
        Err(HirDialogueInvariantError::NonContiguousNodeOrdinal)
    );

    let foreign_content = HirDialogueContentId::new(typed_id(module, 20));
    let foreign_tag = HirRichTextTagId::try_new(foreign_content, 0).unwrap();
    let bad_reference = HirDialogueNode::new(
        HirDialogueNodeId::try_new(content_id, 0).unwrap(),
        HirDialogueNodeKind::AuthoredStartTag(foreign_tag),
    );
    assert_eq!(
        HirDialogueContent::try_new(content_id, Box::new([bad_reference]), Box::new([])),
        Err(HirDialogueInvariantError::InvalidTagReference)
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
    let content =
        HirDialogueContent::try_new(content_id, nodes, Box::new([fx_tag, dialogue_tag])).unwrap();
    let application =
        HirDialogueContentApplication::try_new(owner, target, content, None, Box::new([])).unwrap();
    let mut context = RecordingContext::default();
    application.validate_transaction(&mut context).unwrap();

    assert!(
        context
            .requirements
            .contains(&HirDialogueTransactionRequirement::Expression {
                id: fx_call,
                expected: HirDialogueExpressionExpectation::Call,
            })
    );
    assert!(
        context
            .requirements
            .contains(&HirDialogueTransactionRequirement::Expression {
                id: dialogue_call,
                expected: HirDialogueExpressionExpectation::Call,
            })
    );
    assert!(
        context
            .requirements
            .contains(&HirDialogueTransactionRequirement::Expression {
                id: interpolation,
                expected: HirDialogueExpressionExpectation::Any,
            })
    );
    assert!(
        context
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
    let pattern = typed_id(module, 3);
    let value = typed_id(module, 4);
    let condition = typed_id(module, 5);
    let plan = HirLinePlan::try_new(
        scope,
        Some(name("reveal")),
        vec![
            HirLinePlanItem::Init(Box::new([statement])),
            HirLinePlanItem::Let { pattern, value },
            HirLinePlanItem::TimelineAssert {
                policy: TimelineAssertPolicy::DebugOnly,
                condition,
            },
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
            .contains(&HirDialogueTransactionRequirement::Pattern(pattern))
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
    let content =
        HirDialogueContent::try_new(content_id, Box::new([error_node]), Box::new([])).unwrap();
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
