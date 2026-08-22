use super::{
    HirArrayRepeatExpr, HirAssociatedCallSyntax, HirAssociatedReceiver, HirAssociatedSeparator,
    HirAwaitExpr, HirBinaryExpr, HirBinaryOp, HirBlockExpr, HirBorrowExpr, HirBorrowKind,
    HirBracketSequenceExpr, HirCallArgument, HirCallArgumentListTerminator, HirCallArgumentOrdinal,
    HirCallBuildError, HirCallCallee, HirCallChildPoison, HirCallChildStates, HirCallExpr,
    HirCallIssue, HirCallTypeApplication, HirChoiceBody, HirChoiceCompactAction,
    HirChoiceCompactArm, HirChoiceExpr, HirChoiceIf, HirChoiceIfBranch, HirChoiceItem,
    HirChoiceMatch, HirChoiceMatchArm, HirChoicePlan, HirChoicePlanError, HirChoicePlanItem,
    HirClosureExpr, HirClosureParameter, HirComputationBlockExpr, HirComputationBlockKind,
    HirDereferenceExpr, HirExpr, HirExprInvariantError, HirExprKind, HirExpressionChildEdge,
    HirExpressionChildRole, HirExpressionRecoveryIssue, HirGenericExprIssue, HirIfExpr,
    HirIfLetExpr, HirIndexExpr, HirLoopExpr, HirMatchArm, HirMatchExpr, HirNamedBlockExpr,
    HirNamedBlockName, HirNestedExpressionPath, HirNestedExpressionPathSegment, HirPipeExpr,
    HirPlaceholderKind, HirPoisonState, HirRangeExpr, HirRecordExpr, HirRecordField,
    HirRecordFieldIssue, HirRecordLiteralExpr, HirRecoveredName, HirRecoveryIssue,
    HirRecoveryOperandSlot, HirSelectExpr, HirSelectedMember, HirThreadBody,
    HirThreadBodyInvariantError, HirThreadBodyOwner, HirThreadExpr, HirThreadFlowItem,
    HirThreadMode, HirTryExpr, HirTupleExpr, HirUnaryExpr, HirUnaryOp,
};
use crate::dialogue_application::{
    HirBuiltinRichTextTag, HirDialogueContent, HirDialogueContentApplication, HirDialogueContentId,
    HirDialogueCoordinate, HirDialogueNode, HirDialogueNodeId, HirDialogueNodeKind, HirLinePlan,
    HirLinePlanItem, HirPostfixBracket, HirPostfixBracketCandidates, HirPostfixCandidateFailure,
    HirPostfixCandidateFailureKind, HirRichTextArgument, HirRichTextArgumentId,
    HirRichTextDirectStyle, HirRichTextTag, HirRichTextTagId, HirRichTextTagIdentity,
    HirRichTextTagPayload, HirRichTextValue, HirTextFragment,
};
use crate::identity::{
    ExprId, HirDatabaseId, HirLimit, HirModuleId, HirTypedId, ItemId, LocalId, PatternId, RawHirId,
    ScopeId, StmtId, TypeId,
};
use crate::leaf::{
    HirEntityReference, HirIdRef, HirIdRefInvariantError, HirIdRefIssue, HirIdRefRecovery,
    HirIdRefShape, HirIdRefValue, HirLifetimePathRecovery, HirLifetimePathValue,
    HirLifetimeRegistryIssue, HirLifetimeRegistryPath, HirLifetimeRegistryScope, HirLiteral,
    HirLiteralIssue, HirName, HirNameInvariantError, HirNumericSequence,
    HirNumericSequenceRecovery, HirPath, HirPathIssue, HirPathRecovery, HirPathRoot,
    HirPathSegment, HirPathValue, HirShortVariantName, HirStringIssue, HirStringLiteral,
};
use crate::source_index::{
    HirCallArgumentSourcePart, HirDialogueNodeSourcePart, HirExprSourceRole, HirIdRefSourcePart,
    HirMatchArmSourcePart, HirRecordFieldSourcePart, HirRichTextArgumentSourcePart,
    HirRichTextTagSourcePart, HirSourceQueryError,
};
use core::fmt::Debug;
use core::hash::Hash;
use core::num::{NonZeroU32, NonZeroU64};

fn module(database: u64) -> HirModuleId {
    HirModuleId::new(
        HirDatabaseId::from_raw_for_test(NonZeroU64::new(database).unwrap()),
        NonZeroU32::MIN,
    )
}

fn id<I: HirTypedId>(module: HirModuleId, slot: u32) -> I {
    I::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).unwrap(),
        I::KIND,
    ))
}

fn name(value: &str) -> HirName {
    HirName::try_new(value.into()).expect("valid HIR name")
}

fn clean_call(callee: HirCallCallee, arguments: Box<[HirCallArgument]>) -> HirCallExpr {
    let argument_states = vec![HirCallChildPoison::Clean; arguments.len()];
    let (call, state) = HirCallExpr::try_new(
        callee,
        HirCallTypeApplication::absent(),
        arguments,
        HirCallArgumentListTerminator::Closed,
        HirCallChildStates::new(HirCallChildPoison::Clean, &argument_states, &[]),
        false,
    )
    .expect("clean Call payload");
    assert_eq!(state, HirPoisonState::Clean);
    call
}

fn assert_owned_traits<T: Clone + Debug + Eq + Hash + Ord + PartialOrd>() {}
fn assert_expr_traits<T: Clone + Debug + Eq + PartialEq>() {}

#[test]
fn records_expose_the_exact_required_trait_families() {
    assert_expr_traits::<HirExpr>();
    assert_owned_traits::<HirExprKind>();
    assert_owned_traits::<HirCallExpr>();
    assert_owned_traits::<HirClosureParameter>();
    assert_owned_traits::<HirMatchArm>();
    assert_owned_traits::<HirThreadExpr>();
    assert_owned_traits::<HirAssociatedReceiver>();
}

#[test]
fn expression_owner_rejects_a_foreign_child_before_arena_publication() {
    let owner_module = module(1);
    let foreign_module = module(2);
    let owner_scope = id::<ScopeId>(owner_module, 1);
    let local_child = id::<ExprId>(owner_module, 2);
    let foreign_child = id::<ExprId>(foreign_module, 2);

    let expression = HirExpr::try_new(
        owner_scope,
        HirExprKind::Tuple(HirTupleExpr::new(Box::new([local_child]))),
        HirPoisonState::Clean,
    )
    .expect("same-module tuple");
    assert_eq!(expression.scope(), owner_scope);
    let HirExprKind::Tuple(tuple) = expression.kind() else {
        panic!("tuple payload");
    };
    assert_eq!(tuple.elements(), [local_child]);

    assert_eq!(
        HirExpr::try_new(
            owner_scope,
            HirExprKind::Tuple(HirTupleExpr::new(Box::new([foreign_child]))),
            HirPoisonState::Clean,
        ),
        Err(HirExprInvariantError::ForeignChild {
            expected: owner_module,
            actual: foreign_module,
        })
    );
}

#[test]
fn direct_child_inventory_is_source_independent_for_synthetic_chain() {
    let module = module(2);
    let source = id::<ExprId>(module, 1);
    let iterator = id::<ExprId>(module, 2);

    let into_iterator = HirExprKind::ForSynthetic(super::HirForSyntheticExpr::iterator(source));
    let next_value = HirExprKind::ForSynthetic(super::HirForSyntheticExpr::next_value(iterator));

    assert_eq!(into_iterator.direct_expression_children(), [source]);
    assert_eq!(next_value.direct_expression_children(), [iterator]);
}

#[test]
fn child_edges_project_direct_children_and_preserve_recovery_source_gaps() {
    let module = module(22);
    let first = id::<ExprId>(module, 2);
    let second = id::<ExprId>(module, 3);
    let third = id::<ExprId>(module, 4);

    let assert_projection = |kind: HirExprKind, expected: &[ExprId]| {
        let edges = kind.child_edges();
        assert_eq!(
            edges
                .iter()
                .map(HirExpressionChildEdge::child)
                .collect::<Vec<_>>(),
            expected,
            "edge projection drifted for {kind:?}"
        );
        assert_eq!(kind.direct_expression_children(), expected);
    };

    assert_projection(
        HirExprKind::Tuple(HirTupleExpr::new(Box::new([first, second]))),
        &[first, second],
    );
    assert_projection(
        HirExprKind::ArrayRepeat(HirArrayRepeatExpr::new(first, second)),
        &[first, second],
    );
    assert_projection(
        HirExprKind::If(HirIfExpr::new(first, second, third)),
        &[first, second, third],
    );

    let associated = HirExprKind::Call(clean_call(
        HirCallCallee::associated(
            HirAssociatedReceiver::resolved(id::<TypeId>(module, 5)),
            HirAssociatedSeparator::Present(HirAssociatedCallSyntax::ExplicitDoubleColon),
            HirRecoveredName::Valid(name("run")),
        ),
        Box::new([
            HirCallArgument::positional(first),
            HirCallArgument::positional(second),
        ]),
    ));
    assert_projection(associated.clone(), &[first, second]);
    assert_eq!(
        associated.recovery_operand_slot(0),
        None,
        "an associated call keeps its absent callee recovery slot"
    );
    assert_eq!(
        associated.recovery_operand_slot(1),
        Some(HirRecoveryOperandSlot::Retained(first))
    );
    assert_eq!(
        associated.recovery_operand_slot(2),
        Some(HirRecoveryOperandSlot::Retained(second))
    );

    let range_end_only = HirExprKind::Range(HirRangeExpr::new(None, Some(second), false));
    assert_projection(range_end_only.clone(), &[second]);
    assert_eq!(
        range_end_only.recovery_operand_slot(0),
        None,
        "an end-only range keeps its source ordinal 1"
    );
    assert_eq!(
        range_end_only.recovery_operand_slot(1),
        Some(HirRecoveryOperandSlot::Retained(second))
    );

    let if_let_without_guard = HirExprKind::IfLet(HirIfLetExpr::new(
        id::<ScopeId>(module, 6),
        id::<PatternId>(module, 7),
        first,
        None,
        second,
        third,
    ));
    assert_projection(if_let_without_guard.clone(), &[first, second, third]);
    assert_eq!(
        if_let_without_guard.recovery_operand_slot(1),
        None,
        "the omitted guard remains an absent semantic slot"
    );
    assert_eq!(
        if_let_without_guard.recovery_operand_slot(2),
        Some(HirRecoveryOperandSlot::Retained(second))
    );
    assert_eq!(
        if_let_without_guard.recovery_operand_slot(3),
        Some(HirRecoveryOperandSlot::Retained(third))
    );

    let record = HirExprKind::RecordLiteral(HirRecordLiteralExpr::new(Box::new([
        HirRecordField::invalid(HirRecordFieldIssue::MissingValue),
        HirRecordField::explicit(name("value"), second),
    ])));
    assert_projection(record.clone(), &[second]);
    assert_eq!(
        record.recovery_operand_slot(0),
        Some(HirRecoveryOperandSlot::SyntheticOnly)
    );
    assert_eq!(
        record.recovery_operand_slot(1),
        Some(HirRecoveryOperandSlot::Retained(second))
    );
}

#[test]
fn choice_invalid_assignment_has_no_expression_recovery_slot() {
    let module = module(23);
    let scope = id::<ScopeId>(module, 1);
    let choice = HirExprKind::Choice(HirChoiceExpr::new(
        None,
        HirChoiceBody::new(scope, Box::new([])),
        Some(HirChoicePlan::new(Box::new([HirChoicePlanItem::Error(
            HirChoicePlanError::InvalidAssignmentKey,
        )]))),
    ));

    assert_eq!(choice.recovery_operand_slot(0), None);
    assert!(choice.child_edges().is_empty());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the Call constructor test exhausts ordering, duplicate, spread, poison, and module-identity rows"
)]
fn call_constructor_enforces_ordering_while_expression_owner_enforces_module_identity() {
    let owner_module = module(3);
    let foreign_module = module(4);
    let callee = id::<ExprId>(owner_module, 1);
    let first = id::<ExprId>(owner_module, 2);
    let second = id::<ExprId>(owner_module, 3);
    let foreign = id::<ExprId>(foreign_module, 2);
    let label = name("limit");

    let clean_states = [HirCallChildPoison::Clean, HirCallChildPoison::Clean];
    let (positional_after_named, positional_state) = HirCallExpr::try_new(
        HirCallCallee::value(callee),
        HirCallTypeApplication::absent(),
        Box::new([
            HirCallArgument::named(label.clone(), first),
            HirCallArgument::positional(second),
        ]),
        HirCallArgumentListTerminator::Closed,
        HirCallChildStates::new(HirCallChildPoison::Clean, &clean_states, &[]),
        false,
    )
    .expect("authored ordering recovery is retained");
    let positional_issue = HirCallIssue::PositionalAfterNamed {
        argument: HirCallArgumentOrdinal::try_new(1).expect("second argument"),
    };
    assert_eq!(
        positional_after_named
            .issues(HirCallChildStates::new(
                HirCallChildPoison::Clean,
                &clean_states,
                &[],
            ))
            .as_ref(),
        std::slice::from_ref(&positional_issue)
    );
    assert_eq!(
        positional_state,
        HirPoisonState::Poisoned(HirRecoveryIssue::InvalidCall(positional_issue))
    );

    let (duplicate, duplicate_state) = HirCallExpr::try_new(
        HirCallCallee::value(callee),
        HirCallTypeApplication::absent(),
        Box::new([
            HirCallArgument::named(label.clone(), first),
            HirCallArgument::named(label.clone(), second),
        ]),
        HirCallArgumentListTerminator::Closed,
        HirCallChildStates::new(HirCallChildPoison::Clean, &clean_states, &[]),
        false,
    )
    .expect("duplicate named arguments remain as authored recovery evidence");
    assert!(duplicate.has_duplicate_named_arguments());
    assert_eq!(duplicate.arguments()[0].resolved_name(), Some(&label));
    assert_eq!(duplicate.arguments()[0].value(), first);
    assert_eq!(duplicate.arguments()[1].resolved_name(), Some(&label));
    assert_eq!(duplicate.arguments()[1].value(), second);
    let duplicate_issue = HirCallIssue::DuplicateNamedArgument {
        first: HirCallArgumentOrdinal::try_new(0).expect("first argument"),
        duplicate: HirCallArgumentOrdinal::try_new(1).expect("second argument"),
    };
    assert_eq!(
        duplicate_state,
        HirPoisonState::Poisoned(HirRecoveryIssue::InvalidCall(duplicate_issue.clone()))
    );
    let scope = id::<ScopeId>(owner_module, 4);
    assert_eq!(
        HirExpr::try_new(
            scope,
            HirExprKind::Call(duplicate.clone()),
            HirPoisonState::Clean,
        ),
        Err(HirExprInvariantError::CleanRecoveryPayload)
    );
    let retained = HirExpr::try_new(
        scope,
        HirExprKind::Call(duplicate),
        HirPoisonState::Poisoned(HirRecoveryIssue::InvalidCall(duplicate_issue)),
    )
    .expect("poisoned call retains duplicate arguments for dialogue coordinates");
    assert!(matches!(retained.kind(), HirExprKind::Call(_)));
    let (spread, spread_state) = HirCallExpr::try_new(
        HirCallCallee::value(callee),
        HirCallTypeApplication::absent(),
        Box::new([
            HirCallArgument::spread(first),
            HirCallArgument::named(label, second),
        ]),
        HirCallArgumentListTerminator::Closed,
        HirCallChildStates::new(HirCallChildPoison::Clean, &clean_states, &[]),
        false,
    )
    .expect("spread ordering recovery is retained");
    let spread_issue = HirCallIssue::SpreadNotLast {
        argument: HirCallArgumentOrdinal::try_new(0).expect("first argument"),
    };
    assert_eq!(
        spread
            .issues(HirCallChildStates::new(
                HirCallChildPoison::Clean,
                &clean_states,
                &[],
            ))
            .as_ref(),
        std::slice::from_ref(&spread_issue)
    );
    assert_eq!(
        spread_state,
        HirPoisonState::Poisoned(HirRecoveryIssue::InvalidCall(spread_issue))
    );
    let (foreign_call, foreign_state) = HirCallExpr::try_new(
        HirCallCallee::value(callee),
        HirCallTypeApplication::absent(),
        Box::new([HirCallArgument::positional(foreign)]),
        HirCallArgumentListTerminator::Closed,
        HirCallChildStates::new(HirCallChildPoison::Clean, &[HirCallChildPoison::Clean], &[]),
        false,
    )
    .expect("Call payload construction remains independent of its arena owner");
    assert_eq!(foreign_state, HirPoisonState::Clean);
    assert_eq!(
        HirExpr::try_new(scope, HirExprKind::Call(foreign_call), foreign_state),
        Err(HirExprInvariantError::ForeignChild {
            expected: owner_module,
            actual: foreign_module,
        })
    );
}

#[test]
fn call_constructor_retains_present_poisoned_arguments_and_rejects_clean_missing_values() {
    let owner_module = module(30);
    let callee = id::<ExprId>(owner_module, 1);
    let value = id::<ExprId>(owner_module, 2);
    let poisoned = [HirCallChildPoison::Poisoned];
    let (call, state) = HirCallExpr::try_new(
        HirCallCallee::value(callee),
        HirCallTypeApplication::absent(),
        Box::new([HirCallArgument::positional(value)]),
        HirCallArgumentListTerminator::Closed,
        HirCallChildStates::new(HirCallChildPoison::Clean, &poisoned, &[]),
        false,
    )
    .expect("an authored poisoned argument remains a present Call child");
    let issue = HirCallIssue::InvalidArgumentValue {
        argument: HirCallArgumentOrdinal::try_new(0).expect("first argument"),
    };
    assert_eq!(
        call.issues(HirCallChildStates::new(
            HirCallChildPoison::Clean,
            &poisoned,
            &[],
        ))
        .as_ref(),
        std::slice::from_ref(&issue)
    );
    assert_eq!(
        state,
        HirPoisonState::Poisoned(HirRecoveryIssue::InvalidCall(issue))
    );

    assert_eq!(
        HirCallExpr::try_new(
            HirCallCallee::value(callee),
            HirCallTypeApplication::absent(),
            Box::new([HirCallArgument::missing_positional(value)]),
            HirCallArgumentListTerminator::Closed,
            HirCallChildStates::new(HirCallChildPoison::Clean, &[HirCallChildPoison::Clean], &[],),
            false,
        ),
        Err(HirCallBuildError::ChildStateShapeMismatch)
    );
}

#[test]
fn call_constructor_accepts_exact_and_rejects_one_over_context_limits() {
    let owner_module = module(12);
    let callee = id::<ExprId>(owner_module, 1);

    for (limit, rich_text_context, limit_kind) in [
        (32_usize, true, HirLimit::RichTextCallArguments),
        (128, false, HirLimit::CallArguments),
    ] {
        let exact = (0..limit)
            .map(|ordinal| {
                HirCallArgument::positional(id::<ExprId>(
                    owner_module,
                    u32::try_from(ordinal).unwrap() + 2,
                ))
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let exact_states = vec![HirCallChildPoison::Clean; exact.len()];
        let (exact_call, exact_state) = HirCallExpr::try_new(
            HirCallCallee::value(callee),
            HirCallTypeApplication::absent(),
            exact,
            HirCallArgumentListTerminator::Closed,
            HirCallChildStates::new(HirCallChildPoison::Clean, &exact_states, &[]),
            rich_text_context,
        )
        .expect("exact contextual call limit");
        assert_eq!(exact_call.arguments().len(), limit);
        assert_eq!(exact_state, HirPoisonState::Clean);

        let one_over = (0..=limit)
            .map(|ordinal| {
                HirCallArgument::positional(id::<ExprId>(
                    owner_module,
                    u32::try_from(ordinal).unwrap() + 2,
                ))
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let one_over_states = vec![HirCallChildPoison::Clean; one_over.len()];
        assert_eq!(
            HirCallExpr::try_new(
                HirCallCallee::value(callee),
                HirCallTypeApplication::absent(),
                one_over,
                HirCallArgumentListTerminator::Closed,
                HirCallChildStates::new(HirCallChildPoison::Clean, &one_over_states, &[],),
                rich_text_context,
            ),
            Err(HirCallBuildError::LimitExceeded {
                observed: limit + 1,
                limit: limit_kind,
            })
        );
    }
}

#[test]
fn associated_call_retains_the_typed_receiver_and_authored_separator_family() {
    let owner_module = module(5);
    let root = id::<TypeId>(owner_module, 1);
    let argument = id::<ExprId>(owner_module, 2);
    let call = clean_call(
        HirCallCallee::associated(
            HirAssociatedReceiver::resolved(root),
            HirAssociatedSeparator::Present(HirAssociatedCallSyntax::DotFallback),
            HirRecoveredName::Valid(name("with_capacity")),
        ),
        Box::new([HirCallArgument::positional(argument)]),
    );

    let HirCallCallee::Associated {
        receiver,
        member,
        separator,
    } = call.callee()
    else {
        panic!("associated type callee");
    };
    assert_eq!(receiver.type_id(), Some(root));
    assert_eq!(
        member.resolved().map(HirName::as_str),
        Some("with_capacity")
    );
    assert_eq!(
        *separator,
        HirAssociatedSeparator::Present(HirAssociatedCallSyntax::DotFallback)
    );
    assert_eq!(call.arguments()[0].value(), argument);
}

#[test]
fn closure_parameters_and_match_arms_reject_cross_module_children() {
    let owner_module = module(6);
    let foreign_module = module(7);
    let scope = id::<ScopeId>(owner_module, 1);
    let pattern = id::<PatternId>(owner_module, 2);
    let foreign_pattern = id::<PatternId>(foreign_module, 2);
    let value = id::<ExprId>(owner_module, 3);
    let local = id::<LocalId>(owner_module, 4);

    assert!(HirClosureParameter::try_new(pattern, None, scope).is_ok());
    assert_eq!(
        HirClosureParameter::try_new(foreign_pattern, None, scope),
        Err(HirExprInvariantError::ForeignChild {
            expected: owner_module,
            actual: foreign_module,
        })
    );

    let arm = HirMatchArm::try_new(scope, pattern, None, value, Box::new([local]))
        .expect("same-module arm");
    assert_eq!(arm.scope(), scope);
    assert_eq!(arm.pattern(), pattern);
    assert_eq!(arm.guard(), None);
    assert_eq!(arm.value(), value);
    assert_eq!(arm.locals(), [local]);

    let duplicate = HirMatchArm::try_new(scope, pattern, None, value, Box::new([local]))
        .expect("same-module duplicate arm");
    assert_eq!(
        HirMatchExpr::try_new(value, Box::new([arm, duplicate])),
        Err(HirExprInvariantError::DuplicateMatchArmScope { scope })
    );

    let foreign_scope = id::<ScopeId>(foreign_module, 1);
    let foreign_value = id::<ExprId>(foreign_module, 3);
    let foreign_local = id::<LocalId>(foreign_module, 4);
    let foreign_arm = HirMatchArm::try_new(
        foreign_scope,
        foreign_pattern,
        None,
        foreign_value,
        Box::new([foreign_local]),
    )
    .expect("internally consistent foreign arm");
    assert_eq!(
        HirMatchExpr::try_new(value, Box::new([foreign_arm])),
        Err(HirExprInvariantError::ForeignChild {
            expected: owner_module,
            actual: foreign_module,
        })
    );
}

#[test]
fn thread_body_preserves_source_order_and_rejects_foreign_flow_items() {
    let owner_module = module(8);
    let foreign_module = module(9);
    let scope = id::<ScopeId>(owner_module, 1);
    let statement = id::<StmtId>(owner_module, 2);
    let dialogue = id::<ExprId>(owner_module, 3);
    let owner = id::<ExprId>(owner_module, 4);
    let foreign = id::<StmtId>(foreign_module, 2);
    let body = HirThreadBody::try_new(
        HirThreadBodyOwner::ThreadExpression(owner),
        scope,
        Box::new([
            HirThreadFlowItem::Statement(statement),
            HirThreadFlowItem::DialogueApplication(dialogue),
        ]),
    )
    .expect("same-module body");
    let thread = HirThreadExpr::new(None, HirThreadMode::Attached, body);
    assert_eq!(thread.name(), None);
    assert_eq!(thread.mode(), HirThreadMode::Attached);
    assert_eq!(thread.scope(), scope);
    assert!(matches!(
        thread.body().items(),
        [HirThreadFlowItem::Statement(first), HirThreadFlowItem::DialogueApplication(second)]
            if *first == statement && *second == dialogue
    ));

    assert_eq!(
        HirThreadBody::try_new(
            HirThreadBodyOwner::ThreadExpression(id::<ExprId>(owner_module, 3)),
            scope,
            Box::new([HirThreadFlowItem::Error(foreign)]),
        ),
        Err(HirThreadBodyInvariantError::ForeignReference {
            expected: owner_module,
            actual: foreign_module,
        })
    );
}

#[test]
fn thread_body_item_limit_is_inclusive_for_every_owner() {
    let owner_module = module(80);
    let scope = id::<ScopeId>(owner_module, 1);
    let maximum = HirLimit::ThreadFlowItems.maximum();
    let items = (1..=maximum)
        .map(|slot| {
            HirThreadFlowItem::Statement(id::<StmtId>(
                owner_module,
                u32::try_from(slot).expect("Thread-body limit fits u32"),
            ))
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();

    for owner in [
        HirThreadBodyOwner::Flow(id::<ItemId>(owner_module, 2)),
        HirThreadBodyOwner::ThreadExpression(id::<ExprId>(owner_module, 3)),
        HirThreadBodyOwner::NestedScope(scope),
    ] {
        let exact = HirThreadBody::try_new(owner, scope, items.clone())
            .expect("the inclusive ThreadFlowItems limit commits for every body owner");
        assert_eq!(exact.items().len(), maximum);
    }

    let mut one_over = items.into_vec();
    one_over.push(HirThreadFlowItem::DialogueApplication(id::<ExprId>(
        owner_module,
        u32::try_from(maximum + 1).expect("one-over Thread-body limit fits u32"),
    )));
    for owner in [
        HirThreadBodyOwner::Flow(id::<ItemId>(owner_module, 2)),
        HirThreadBodyOwner::ThreadExpression(id::<ExprId>(owner_module, 3)),
        HirThreadBodyOwner::NestedScope(scope),
    ] {
        assert_eq!(
            HirThreadBody::try_new(owner, scope, one_over.clone().into_boxed_slice()),
            Err(HirThreadBodyInvariantError::ItemLimit {
                observed: maximum + 1,
                maximum,
            })
        );
    }
}

#[test]
fn record_children_are_checked_by_the_expression_owner() {
    let owner_module = module(10);
    let foreign_module = module(11);
    let scope = id::<ScopeId>(owner_module, 1);
    let foreign_local = id::<LocalId>(foreign_module, 1);
    let kind = HirExprKind::RecordLiteral(HirRecordLiteralExpr::new(Box::new([
        HirRecordField::shorthand(name("value"), foreign_local),
    ])));

    assert_eq!(
        HirExpr::try_new(scope, kind, HirPoisonState::Clean),
        Err(HirExprInvariantError::ForeignChild {
            expected: owner_module,
            actual: foreign_module,
        })
    );
}

#[test]
fn expression_owner_requires_poison_for_recovery_payload() {
    let owner_module = module(13);
    let scope = id::<ScopeId>(owner_module, 1);
    let issue = HirStringIssue::Unterminated;
    let kind = HirExprKind::Literal(HirLiteral::String(HirStringLiteral::Invalid(issue)));
    let expected = HirRecoveryIssue::MalformedLiteral(HirLiteralIssue::String(issue));

    assert_eq!(
        HirExpr::try_new(scope, kind.clone(), HirPoisonState::Clean),
        Err(HirExprInvariantError::CleanRecoveryPayload)
    );
    assert_eq!(
        HirExpr::try_new(
            scope,
            kind.clone(),
            HirPoisonState::Poisoned(HirRecoveryIssue::StaleSource),
        ),
        Err(HirExprInvariantError::LeafRecoveryIssueMismatch {
            expected: expected.clone(),
            actual: HirRecoveryIssue::StaleSource,
        })
    );

    let poisoned = HirExpr::try_new(scope, kind, HirPoisonState::Poisoned(expected.clone()))
        .expect("typed poison admits the retained invalid literal");
    assert_eq!(poisoned.state(), &HirPoisonState::Poisoned(expected));

    for kind in [
        HirExprKind::Literal(HirLiteral::Boolean(true)),
        HirExprKind::Unit,
        HirExprKind::Placeholder(HirPlaceholderKind::PartialApplication),
    ] {
        HirExpr::try_new(scope, kind.clone(), HirPoisonState::Clean)
            .expect("resolved leaf is clean-only");
        assert_eq!(
            HirExpr::try_new(
                scope,
                kind,
                HirPoisonState::Poisoned(HirRecoveryIssue::StaleSource),
            ),
            Err(HirExprInvariantError::UnexpectedLeafPoison {
                actual: HirRecoveryIssue::StaleSource,
            })
        );
    }

    assert_eq!(
        HirExpr::try_new(
            scope,
            HirExprKind::Error(super::HirExprError::new(
                HirGenericExprIssue::UnclassifiedSyntax,
            )),
            HirPoisonState::Clean,
        ),
        Err(HirExprInvariantError::CleanRecoveryPayload)
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the leaf recovery test exhausts every retained leaf payload and exact shared issue"
)]
fn expression_leaf_recovery_requires_the_exact_shared_issue() {
    let owner_module = module(131);
    let scope = id::<ScopeId>(owner_module, 1);
    let recovered = [
        (
            HirExprKind::EntityReference(HirIdRefValue::Recovered(HirIdRefRecovery::new(
                HirIdRefShape::Missing,
                HirIdRefIssue::Missing,
            ))),
            HirRecoveryIssue::InvalidId(HirIdRefIssue::Missing),
        ),
        (
            HirExprKind::LifetimePath(HirLifetimePathValue::Recovered(
                HirLifetimePathRecovery::new(
                    false,
                    0,
                    false,
                    HirLifetimeRegistryIssue::MissingScope,
                ),
            )),
            HirRecoveryIssue::InvalidLifetimeRegistry(HirLifetimeRegistryIssue::MissingScope),
        ),
        (
            HirExprKind::Path(HirPathValue::Recovered(HirPathRecovery::new(
                HirPathRoot::Crate,
                0,
                HirPathIssue::Empty,
            ))),
            HirRecoveryIssue::InvalidPath(HirPathIssue::Empty),
        ),
        (
            HirExprKind::ShortVariant(HirShortVariantName::Recovered(
                HirNameInvariantError::InvalidIdentifier,
            )),
            HirRecoveryIssue::InvalidName(HirNameInvariantError::InvalidIdentifier),
        ),
    ];

    for (kind, expected) in recovered {
        assert_eq!(
            HirExpr::try_new(scope, kind.clone(), HirPoisonState::Clean),
            Err(HirExprInvariantError::CleanRecoveryPayload)
        );
        assert_eq!(
            HirExpr::try_new(
                scope,
                kind.clone(),
                HirPoisonState::Poisoned(HirRecoveryIssue::StaleSource),
            ),
            Err(HirExprInvariantError::LeafRecoveryIssueMismatch {
                expected: expected.clone(),
                actual: HirRecoveryIssue::StaleSource,
            })
        );
        assert_eq!(
            HirExpr::try_new(
                scope,
                kind.clone(),
                HirPoisonState::Poisoned(expected.clone())
            )
            .expect("matching leaf recovery issue"),
            HirExpr {
                scope,
                kind,
                state: HirPoisonState::Poisoned(expected),
            }
        );
    }

    let generic_error = HirExprKind::Error(super::HirExprError::new(
        HirGenericExprIssue::UnclassifiedSyntax,
    ));
    let generic_issue = HirRecoveryIssue::InvalidExpression(HirExpressionRecoveryIssue::Generic(
        HirGenericExprIssue::UnclassifiedSyntax,
    ));
    assert_eq!(
        HirExpr::try_new(scope, generic_error.clone(), HirPoisonState::Clean),
        Err(HirExprInvariantError::CleanRecoveryPayload)
    );
    assert_eq!(
        HirExpr::try_new(
            scope,
            generic_error.clone(),
            HirPoisonState::Poisoned(HirRecoveryIssue::InvalidExpression(
                HirExpressionRecoveryIssue::Generic(HirGenericExprIssue::TransactionalChildFailure,),
            )),
        ),
        Err(HirExprInvariantError::LeafRecoveryIssueMismatch {
            expected: generic_issue.clone(),
            actual: HirRecoveryIssue::InvalidExpression(HirExpressionRecoveryIssue::Generic(
                HirGenericExprIssue::TransactionalChildFailure,
            ),),
        })
    );
    assert_eq!(
        HirExpr::try_new(
            scope,
            generic_error.clone(),
            HirPoisonState::Poisoned(generic_issue.clone()),
        )
        .expect("matching generic expression recovery issue"),
        HirExpr {
            scope,
            kind: generic_error,
            state: HirPoisonState::Poisoned(generic_issue),
        }
    );
}

#[test]
fn expression_resolved_leaf_rejects_leaf_poison() {
    let owner_module = module(132);
    let scope = id::<ScopeId>(owner_module, 1);
    let entity = HirIdRefValue::Resolved(HirIdRef::absolute(
        HirEntityReference::try_new("scene.entry".into()).expect("valid entity reference"),
    ));
    let lifetime = HirLifetimePathValue::Resolved(HirLifetimeRegistryPath::try_new(
        HirLifetimeRegistryScope::Frame,
        Box::new([name("value")]),
        false,
    ));
    let path = HirPathValue::Resolved(
        HirPath::try_new(
            HirPathRoot::ImplicitCrate,
            Box::new([HirPathSegment::Identifier(name("Value"))]),
        )
        .expect("valid path"),
    );
    let variant = HirShortVariantName::Resolved(name("Ready"));

    for (kind, issue) in [
        (
            HirExprKind::EntityReference(entity),
            HirRecoveryIssue::InvalidId(HirIdRefIssue::Missing),
        ),
        (
            HirExprKind::LifetimePath(lifetime),
            HirRecoveryIssue::InvalidLifetimeRegistry(HirLifetimeRegistryIssue::MissingScope),
        ),
        (
            HirExprKind::Path(path),
            HirRecoveryIssue::InvalidPath(HirPathIssue::Empty),
        ),
        (
            HirExprKind::ShortVariant(variant),
            HirRecoveryIssue::InvalidName(HirNameInvariantError::InvalidIdentifier),
        ),
    ] {
        assert_eq!(
            HirExpr::try_new(scope, kind, HirPoisonState::Poisoned(issue.clone())),
            Err(HirExprInvariantError::UnexpectedLeafPoison { actual: issue })
        );
    }
}

#[test]
fn recovered_expression_leaf_roles_follow_retained_shapes() {
    let owner = id::<ExprId>(module(133), 1);
    let entity = HirExprKind::EntityReference(HirIdRefValue::Recovered(HirIdRefRecovery::new(
        HirIdRefShape::FamilyRelative {
            parent_depth: 2,
            suffix_segment_count: 3,
        },
        HirIdRefIssue::Invalid(HirIdRefInvariantError::InvalidSuffix),
    )));
    for role in [
        HirExprSourceRole::EntityReference(HirIdRefSourcePart::Whole),
        HirExprSourceRole::EntityReference(HirIdRefSourcePart::Family),
        HirExprSourceRole::EntityReference(HirIdRefSourcePart::FamilySeparator),
        HirExprSourceRole::EntityReference(HirIdRefSourcePart::ParentMarker { ordinal: 1 }),
        HirExprSourceRole::EntityReference(HirIdRefSourcePart::SuffixSegment { ordinal: 2 }),
    ] {
        assert_eq!(entity.validate_source_role(owner, role), Ok(()));
    }
    assert!(matches!(
        entity.validate_source_role(
            owner,
            HirExprSourceRole::EntityReference(HirIdRefSourcePart::SuffixSegment { ordinal: 3 })
        ),
        Err(HirSourceQueryError::ExprOrdinalOutOfBounds { length: 3, .. })
    ));

    let lifetime = HirExprKind::LifetimePath(HirLifetimePathValue::Recovered(
        HirLifetimePathRecovery::new(false, 2, false, HirLifetimeRegistryIssue::MissingScope),
    ));
    assert_eq!(
        lifetime.validate_source_role(owner, HirExprSourceRole::RegistryScope),
        Ok(())
    );
    assert_eq!(
        lifetime.validate_source_role(owner, HirExprSourceRole::OptionalMarker),
        Ok(())
    );
    assert_eq!(
        lifetime.validate_source_role(owner, HirExprSourceRole::RegistryKeySegment { ordinal: 1 }),
        Ok(())
    );
    assert!(matches!(
        lifetime.validate_source_role(owner, HirExprSourceRole::RegistryKeySegment { ordinal: 2 }),
        Err(HirSourceQueryError::ExprOrdinalOutOfBounds { length: 2, .. })
    ));

    let path = HirExprKind::Path(HirPathValue::Recovered(HirPathRecovery::new(
        HirPathRoot::Crate,
        2,
        HirPathIssue::InvalidSegment { ordinal: 1 },
    )));
    assert_eq!(
        path.validate_source_role(owner, HirExprSourceRole::PathRoot),
        Ok(())
    );
    assert_eq!(
        path.validate_source_role(owner, HirExprSourceRole::PathSegment { ordinal: 1 }),
        Ok(())
    );
    assert!(matches!(
        path.validate_source_role(owner, HirExprSourceRole::PathSegment { ordinal: 2 }),
        Err(HirSourceQueryError::ExprOrdinalOutOfBounds { length: 2, .. })
    ));

    let short = HirExprKind::ShortVariant(HirShortVariantName::Recovered(
        HirNameInvariantError::InvalidIdentifier,
    ));
    assert_eq!(
        short.validate_source_role(owner, HirExprSourceRole::ShortVariantName),
        Ok(())
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the closed 35-family table is clearest as one exhaustive matrix"
)]
fn expression_source_roles_cover_the_closed_thirty_six_family_matrix() {
    let module = module(14);
    let owner = id::<ExprId>(module, 1);
    let first = id::<ExprId>(module, 2);
    let second = id::<ExprId>(module, 3);
    let scope = id::<ScopeId>(module, 4);
    let statement = id::<StmtId>(module, 5);
    let pattern = id::<PatternId>(module, 6);
    let local = id::<LocalId>(module, 7);
    let path = HirPath::try_new(
        HirPathRoot::ImplicitCrate,
        Box::new([HirPathSegment::Identifier(name("Value"))]),
    )
    .expect("test path");
    let entity = HirIdRef::absolute(
        HirEntityReference::try_new("scene.entry".into()).expect("test entity reference"),
    );
    let lifetime = HirLifetimeRegistryPath::try_new(
        HirLifetimeRegistryScope::Frame,
        Box::new([name("value")]),
        false,
    );
    let call = clean_call(
        HirCallCallee::value(first),
        Box::new([HirCallArgument::named(name("value"), second)]),
    );
    let closure_parameter =
        HirClosureParameter::try_new(pattern, None, scope).expect("test closure parameter");
    let match_arm = HirMatchArm::try_new(scope, pattern, None, second, Box::new([local]))
        .expect("test match arm");
    let thread = HirThreadExpr::new(
        None,
        HirThreadMode::Attached,
        HirThreadBody::try_new(
            HirThreadBodyOwner::ThreadExpression(owner),
            scope,
            Box::new([HirThreadFlowItem::Statement(statement)]),
        )
        .expect("test thread body"),
    );
    let content =
        HirDialogueContent::try_new(HirDialogueContentId::new(owner), Box::new([]), Box::new([]))
            .expect("empty dialogue content");
    let dialogue =
        HirDialogueContentApplication::try_new(owner, first, content, None, Box::new([]))
            .expect("test dialogue application");
    let postfix = HirPostfixBracket::try_new(
        first,
        HirPostfixBracketCandidates::Invalid {
            index: HirPostfixCandidateFailure::new(HirPostfixCandidateFailureKind::EmptyPayload),
            dialogue: HirPostfixCandidateFailure::new(
                HirPostfixCandidateFailureKind::InvalidDialogueAtom,
            ),
        },
    )
    .expect("test postfix bracket");

    let families = vec![
        (HirExprKind::Unit, HirExprSourceRole::Whole),
        (
            HirExprKind::Literal(HirLiteral::Boolean(true)),
            HirExprSourceRole::LiteralBody,
        ),
        (
            HirExprKind::EntityReference(HirIdRefValue::Resolved(entity)),
            HirExprSourceRole::EntityReference(HirIdRefSourcePart::SuffixSegment { ordinal: 1 }),
        ),
        (
            HirExprKind::LifetimePath(HirLifetimePathValue::Resolved(lifetime)),
            HirExprSourceRole::RegistryKeySegment { ordinal: 0 },
        ),
        (
            HirExprKind::Path(HirPathValue::Resolved(path.clone())),
            HirExprSourceRole::PathSegment { ordinal: 0 },
        ),
        (
            HirExprKind::ShortVariant(HirShortVariantName::Resolved(name("Ready"))),
            HirExprSourceRole::ShortVariantName,
        ),
        (
            HirExprKind::Placeholder(HirPlaceholderKind::PipeLeft),
            HirExprSourceRole::PlaceholderMarker,
        ),
        (
            HirExprKind::Tuple(HirTupleExpr::new(Box::new([first]))),
            HirExprSourceRole::Element { ordinal: 0 },
        ),
        (
            HirExprKind::BracketSequence(HirBracketSequenceExpr::new(Box::new([first]))),
            HirExprSourceRole::Element { ordinal: 0 },
        ),
        (
            HirExprKind::NumericBracketSequence(
                HirNumericSequence::try_new(
                    Box::new([]),
                    None,
                    HirNumericSequenceRecovery::MissingFinalElement { ordinal: 0 },
                )
                .expect("valid missing-final recovery"),
            ),
            HirExprSourceRole::NumericCommonSuffix,
        ),
        (
            HirExprKind::ArrayRepeat(HirArrayRepeatExpr::new(first, second)),
            HirExprSourceRole::RepeatLength,
        ),
        (HirExprKind::Call(call), HirExprSourceRole::CallCallee),
        (
            HirExprKind::Select(HirSelectExpr::new(
                first,
                HirSelectedMember::Name(name("member")),
            )),
            HirExprSourceRole::SelectedMember,
        ),
        (
            HirExprKind::Index(HirIndexExpr::new(first, second)),
            HirExprSourceRole::Index,
        ),
        (
            HirExprKind::Pipe(HirPipeExpr::new(first, second)),
            HirExprSourceRole::LeftOperand,
        ),
        (
            HirExprKind::Try(HirTryExpr::new(first)),
            HirExprSourceRole::Operator,
        ),
        (
            HirExprKind::Await(
                HirAwaitExpr::try_new(first, Box::new([])).expect("empty await branches"),
            ),
            HirExprSourceRole::Operand,
        ),
        (
            HirExprKind::Thread(thread),
            HirExprSourceRole::ThreadModifier,
        ),
        (
            HirExprKind::Range(HirRangeExpr::new(None, None, false)),
            HirExprSourceRole::RangeStart,
        ),
        (
            HirExprKind::Record(HirRecordExpr::new(
                path,
                Box::new([HirRecordField::explicit(name("field"), first)]),
            )),
            HirExprSourceRole::RecordPath,
        ),
        (
            HirExprKind::RecordLiteral(HirRecordLiteralExpr::new(Box::new([
                HirRecordField::shorthand(name("field"), local),
            ]))),
            HirExprSourceRole::RecordField {
                field: 0,
                part: HirRecordFieldSourcePart::Name,
            },
        ),
        (
            HirExprKind::Binary(HirBinaryExpr::new(first, HirBinaryOp::Add, second)),
            HirExprSourceRole::Operator,
        ),
        (
            HirExprKind::Borrow(HirBorrowExpr::new(HirBorrowKind::Shared, first)),
            HirExprSourceRole::Operand,
        ),
        (
            HirExprKind::Dereference(HirDereferenceExpr::new(first)),
            HirExprSourceRole::Operator,
        ),
        (
            HirExprKind::Closure(HirClosureExpr::new(
                scope,
                Box::new([closure_parameter]),
                None,
                first,
                Box::new([]),
            )),
            HirExprSourceRole::ReturnType,
        ),
        (
            HirExprKind::Unary(HirUnaryExpr::new(HirUnaryOp::Not, first)),
            HirExprSourceRole::Operand,
        ),
        (
            HirExprKind::Block(HirBlockExpr::new(scope, Box::new([statement]), first)),
            HirExprSourceRole::Statement { ordinal: 0 },
        ),
        (
            HirExprKind::ComputationBlock(HirComputationBlockExpr::new(
                HirComputationBlockKind::Option,
                scope,
                Box::new([]),
                first,
            )),
            HirExprSourceRole::Tail,
        ),
        (
            HirExprKind::NamedBlock(HirNamedBlockExpr::new(
                HirNamedBlockName::Resolved(name("retry")),
                scope,
                Box::new([]),
                first,
            )),
            HirExprSourceRole::Name,
        ),
        (
            HirExprKind::If(HirIfExpr::new(first, second, first)),
            HirExprSourceRole::ElseBranch,
        ),
        (
            HirExprKind::IfLet(HirIfLetExpr::new(
                scope, pattern, first, None, second, first,
            )),
            HirExprSourceRole::Guard,
        ),
        (
            HirExprKind::Match(
                HirMatchExpr::try_new(first, Box::new([match_arm]))
                    .expect("same-module match expression"),
            ),
            HirExprSourceRole::MatchArm {
                arm: 0,
                part: HirMatchArmSourcePart::Guard,
            },
        ),
        (
            HirExprKind::DialogueContentApplication(dialogue),
            HirExprSourceRole::ContentBody,
        ),
        (
            HirExprKind::PostfixBracket(postfix),
            HirExprSourceRole::Content,
        ),
        (
            HirExprKind::Error(super::HirExprError::new(
                HirGenericExprIssue::UnclassifiedSyntax,
            )),
            HirExprSourceRole::Recovery,
        ),
    ];

    assert_eq!(families.len(), 35);
    for (kind, role) in families {
        assert_eq!(kind.validate_source_role(owner, role), Ok(()), "{kind:?}");
    }

    let choice = HirExprKind::Choice(HirChoiceExpr::new(
        None,
        HirChoiceBody::new(scope, vec![HirChoiceItem::Error].into_boxed_slice()),
        None,
    ));
    assert!(matches!(
        choice.validate_source_role(owner, HirExprSourceRole::Recovery),
        Err(HirSourceQueryError::ExprRoleNotApplicable { .. })
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the negative matrix keeps exact one-over and wrong-part evidence together"
)]
fn expression_source_roles_reject_wrong_parts_and_exact_one_over_ordinals() {
    let module = module(15);
    let owner = id::<ExprId>(module, 1);
    let first = id::<ExprId>(module, 2);
    let second = id::<ExprId>(module, 3);

    let tuple = HirExprKind::Tuple(HirTupleExpr::new(Box::new([first])));
    let one_over = HirExprSourceRole::Element { ordinal: 1 };
    assert_eq!(
        tuple.validate_source_role(owner, one_over),
        Err(HirSourceQueryError::ExprOrdinalOutOfBounds {
            owner,
            role: one_over,
            length: 1,
        })
    );

    let incomplete_numeric = HirExprKind::NumericBracketSequence(
        HirNumericSequence::try_new(
            Box::new([]),
            None,
            HirNumericSequenceRecovery::MissingFinalElement { ordinal: 0 },
        )
        .expect("valid missing-final recovery"),
    );
    let missing_numeric = HirExprSourceRole::NumericElement { ordinal: 0 };
    assert_eq!(
        incomplete_numeric.validate_source_role(owner, missing_numeric),
        Ok(())
    );
    let one_over_incomplete_numeric = HirExprSourceRole::NumericElement { ordinal: 1 };
    assert_eq!(
        incomplete_numeric.validate_source_role(owner, one_over_incomplete_numeric),
        Err(HirSourceQueryError::ExprOrdinalOutOfBounds {
            owner,
            role: one_over_incomplete_numeric,
            length: 1,
        })
    );

    let arguments = vec![
        HirCallArgument::positional(first),
        HirCallArgument::named(name("id"), second),
    ]
    .into_boxed_slice();
    let coordinates = HirDialogueCoordinate::from_immediate_arguments(&arguments)
        .expect("bounded dialogue coordinates");
    let content =
        HirDialogueContent::try_new(HirDialogueContentId::new(owner), Box::new([]), Box::new([]))
            .expect("empty dialogue content");
    let dialogue = HirExprKind::DialogueContentApplication(
        HirDialogueContentApplication::try_new(owner, first, content, None, coordinates)
            .expect("dialogue coordinates"),
    );
    assert_eq!(dialogue.direct_expression_children(), [first, second]);
    let coordinate = HirCallArgumentOrdinal::try_new(1).expect("coordinate ordinal");
    assert_eq!(
        dialogue.validate_source_role(
            owner,
            HirExprSourceRole::ConfigurationArgument {
                argument: coordinate,
                part: HirCallArgumentSourcePart::Name,
            },
        ),
        Ok(())
    );
    let non_coordinate = HirCallArgumentOrdinal::try_new(0).expect("non-coordinate ordinal");
    let non_coordinate_role = HirExprSourceRole::ConfigurationArgument {
        argument: non_coordinate,
        part: HirCallArgumentSourcePart::Value,
    };
    assert_eq!(
        dialogue.validate_source_role(owner, non_coordinate_role),
        Err(HirSourceQueryError::ExprRoleNotApplicable {
            owner,
            role: non_coordinate_role,
        })
    );
    assert_eq!(
        dialogue.validate_source_role_with_context(owner, non_coordinate_role, Some(2)),
        Err(HirSourceQueryError::ExprRoleNotApplicable {
            owner,
            role: non_coordinate_role,
        })
    );
    let one_over_configuration = HirExprSourceRole::ConfigurationArgument {
        argument: HirCallArgumentOrdinal::try_new(2).expect("one-over configuration ordinal"),
        part: HirCallArgumentSourcePart::Value,
    };
    assert_eq!(
        dialogue.validate_source_role_with_context(owner, one_over_configuration, Some(2)),
        Err(HirSourceQueryError::ExprOrdinalOutOfBounds {
            owner,
            role: one_over_configuration,
            length: 2,
        })
    );

    let call = HirExprKind::Call(clean_call(
        HirCallCallee::value(first),
        Box::new([HirCallArgument::positional(second)]),
    ));
    assert_eq!(call.direct_expression_children(), [first, second]);
    assert_eq!(
        call.validate_source_role(owner, HirExprSourceRole::CallAssociatedReceiver),
        Err(HirSourceQueryError::ExprRoleNotApplicable {
            owner,
            role: HirExprSourceRole::CallAssociatedReceiver,
        })
    );
    let positional_name = HirExprSourceRole::CallArgument {
        argument: HirCallArgumentOrdinal::try_new(0).expect("argument ordinal"),
        part: HirCallArgumentSourcePart::Name,
    };
    assert_eq!(
        call.validate_source_role(owner, positional_name),
        Err(HirSourceQueryError::ExprRoleNotApplicable {
            owner,
            role: positional_name,
        })
    );

    let associated_call = HirExprKind::Call(clean_call(
        HirCallCallee::associated(
            HirAssociatedReceiver::resolved(id::<TypeId>(module, 4)),
            HirAssociatedSeparator::Present(HirAssociatedCallSyntax::DotFallback),
            HirRecoveredName::Valid(name("with_capacity")),
        ),
        Box::new([]),
    ));
    assert_eq!(
        associated_call.validate_source_role(owner, HirExprSourceRole::CallAssociatedReceiver),
        Ok(())
    );
    assert_eq!(
        associated_call.validate_source_role(owner, HirExprSourceRole::CallCallee),
        Err(HirSourceQueryError::ExprRoleNotApplicable {
            owner,
            role: HirExprSourceRole::CallCallee,
        })
    );
}

#[test]
fn postfix_bracket_source_roles_cover_the_closed_component_matrix() {
    let module = module(15);
    let owner = id::<ExprId>(module, 1);
    let target = id::<ExprId>(module, 2);
    let postfix = HirExprKind::PostfixBracket(
        HirPostfixBracket::try_new(
            target,
            HirPostfixBracketCandidates::Invalid {
                index: HirPostfixCandidateFailure::new(
                    HirPostfixCandidateFailureKind::EmptyPayload,
                ),
                dialogue: HirPostfixCandidateFailure::new(
                    HirPostfixCandidateFailureKind::EmptyPayload,
                ),
            },
        )
        .expect("postfix bracket"),
    );

    for role in [
        HirExprSourceRole::Target,
        HirExprSourceRole::OpenBracket,
        HirExprSourceRole::CloseBracket,
        HirExprSourceRole::Content,
    ] {
        assert_eq!(postfix.validate_source_role(owner, role), Ok(()));
    }

    assert_eq!(
        postfix.validate_source_role(owner, HirExprSourceRole::Colon),
        Err(HirSourceQueryError::ExprRoleNotApplicable {
            owner,
            role: HirExprSourceRole::Colon,
        })
    );
    assert_eq!(
        postfix.validate_source_role(owner, HirExprSourceRole::ContentBody),
        Err(HirSourceQueryError::ExprRoleNotApplicable {
            owner,
            role: HirExprSourceRole::ContentBody,
        })
    );
}

#[test]
fn dialogue_and_rich_text_source_roles_validate_nested_typed_ordinals() {
    let module = module(16);
    let owner = id::<ExprId>(module, 1);
    let target = id::<ExprId>(module, 2);
    let content_id = HirDialogueContentId::new(owner);
    let tag_id = HirRichTextTagId::try_new(content_id, 0).expect("tag id");
    let argument_id = HirRichTextArgumentId::try_new(tag_id, 0).expect("argument id");
    let argument = HirRichTextArgument::named(
        argument_id,
        name("tone"),
        HirRichTextValue::new("calm".into()),
    );
    let tag = HirRichTextTag::try_new(
        tag_id,
        HirRichTextTagIdentity::Builtin(HirBuiltinRichTextTag::DirectStyle(
            HirRichTextDirectStyle::Color,
        )),
        Box::new([argument]),
        HirRichTextTagPayload::Arguments,
    )
    .expect("rich text tag");
    let start = HirDialogueNode::new(
        HirDialogueNodeId::try_new(content_id, 0).expect("start node id"),
        HirDialogueNodeKind::AuthoredStartTag(tag_id),
    );
    let text = HirDialogueNode::new(
        HirDialogueNodeId::try_new(content_id, 1).expect("text node id"),
        HirDialogueNodeKind::Text(HirTextFragment::new("hello".into())),
    );
    let content = HirDialogueContent::try_new(content_id, Box::new([start, text]), Box::new([tag]))
        .expect("dialogue content");
    let kind = HirExprKind::DialogueContentApplication(
        HirDialogueContentApplication::try_new(owner, target, content, None, Box::new([]))
            .expect("dialogue application"),
    );

    for role in [
        HirExprSourceRole::DialogueNode {
            ordinal: 0,
            part: HirDialogueNodeSourcePart::Whole,
        },
        HirExprSourceRole::DialogueNode {
            ordinal: 1,
            part: HirDialogueNodeSourcePart::Text,
        },
        HirExprSourceRole::RichTextTag {
            tag: 0,
            part: HirRichTextTagSourcePart::Name,
        },
        HirExprSourceRole::RichTextArgument {
            tag: 0,
            argument: 0,
            part: HirRichTextArgumentSourcePart::Equals,
        },
    ] {
        assert_eq!(kind.validate_source_role(owner, role), Ok(()));
    }

    let node_one_over = HirExprSourceRole::DialogueNode {
        ordinal: 2,
        part: HirDialogueNodeSourcePart::Error,
    };
    assert_eq!(
        kind.validate_source_role(owner, node_one_over),
        Err(HirSourceQueryError::ExprOrdinalOutOfBounds {
            owner,
            role: node_one_over,
            length: 2,
        })
    );
    let argument_one_over = HirExprSourceRole::RichTextArgument {
        tag: 0,
        argument: 1,
        part: HirRichTextArgumentSourcePart::Value,
    };
    assert_eq!(
        kind.validate_source_role(owner, argument_one_over),
        Err(HirSourceQueryError::ExprOrdinalOutOfBounds {
            owner,
            role: argument_one_over,
            length: 1,
        })
    );
}

fn edge_role_tag(role: &HirExpressionChildRole) -> u8 {
    match role {
        HirExpressionChildRole::Element { .. } => 0,
        HirExpressionChildRole::RepeatedValue => 1,
        HirExpressionChildRole::RepeatLength => 2,
        HirExpressionChildRole::Callee => 3,
        HirExpressionChildRole::Argument { .. } => 4,
        HirExpressionChildRole::Target => 5,
        HirExpressionChildRole::Index => 6,
        HirExpressionChildRole::PipeLeft => 7,
        HirExpressionChildRole::PipeRight => 8,
        HirExpressionChildRole::Operand => 9,
        HirExpressionChildRole::RangeStart => 10,
        HirExpressionChildRole::RangeEnd => 11,
        HirExpressionChildRole::RecordField { .. } => 12,
        HirExpressionChildRole::BinaryLeft => 13,
        HirExpressionChildRole::BinaryRight => 14,
        HirExpressionChildRole::ClosureBody => 15,
        HirExpressionChildRole::BlockTail => 16,
        HirExpressionChildRole::LoopTail => 17,
        HirExpressionChildRole::Condition => 18,
        HirExpressionChildRole::ThenBranch => 19,
        HirExpressionChildRole::ElseBranch => 20,
        HirExpressionChildRole::Scrutinee => 21,
        HirExpressionChildRole::Guard { .. } => 22,
        HirExpressionChildRole::ArmValue { .. } => 23,
        HirExpressionChildRole::IfLetGuard => 24,
        HirExpressionChildRole::DialogueTarget => 25,
        HirExpressionChildRole::DialogueCoordinate { .. } => 26,
        HirExpressionChildRole::DialogueInterpolation { .. } => 27,
        HirExpressionChildRole::DialogueTagPayload { .. } => 28,
        HirExpressionChildRole::LinePlanOptionValue { .. } => 29,
        HirExpressionChildRole::LinePlanLetValue { .. } => 30,
        HirExpressionChildRole::LinePlanOut { .. } => 31,
        HirExpressionChildRole::LinePlanTimelineAssert { .. } => 32,
        HirExpressionChildRole::LinePlanExpression { .. } => 33,
        HirExpressionChildRole::LinePlanTimedCueAnchor { .. } => 34,
        HirExpressionChildRole::LinePlanTimedCueBody { .. } => 35,
        HirExpressionChildRole::PostfixIndexCandidate => 36,
        HirExpressionChildRole::PostfixDialogueCandidate => 37,
        HirExpressionChildRole::ForInput => 38,
        HirExpressionChildRole::ChoiceIfCondition { .. } => 39,
        HirExpressionChildRole::ChoiceForSource { .. } => 40,
        HirExpressionChildRole::ChoiceMatchScrutinee { .. } => 41,
        HirExpressionChildRole::ChoiceMatchGuard { .. } => 42,
        HirExpressionChildRole::ChoiceOptionId { .. } => 43,
        HirExpressionChildRole::ChoiceOptionForSource { .. } => 44,
        HirExpressionChildRole::ChoiceCompactLabel { .. } => 45,
        HirExpressionChildRole::ChoiceCompactCondition { .. } => 46,
        HirExpressionChildRole::ChoiceCompactOut { .. } => 47,
        HirExpressionChildRole::ChoiceOptionLabel { .. } => 48,
        HirExpressionChildRole::ChoiceOptionFieldId { .. } => 49,
        HirExpressionChildRole::ChoiceOptionValue { .. } => 50,
        HirExpressionChildRole::ChoiceOptionVisible { .. } => 51,
        HirExpressionChildRole::ChoiceOptionEnabled { .. } => 52,
        HirExpressionChildRole::ChoiceOptionOrder { .. } => 53,
        HirExpressionChildRole::ChoiceOptionHotkey { .. } => 54,
        HirExpressionChildRole::ChoiceOptionViewKey { .. } => 55,
        HirExpressionChildRole::ChoiceOptionViewValue { .. } => 56,
        HirExpressionChildRole::ChoicePlanAssignment { .. } => 57,
        HirExpressionChildRole::ChoicePlanTimeout { .. } => 58,
        HirExpressionChildRole::ChoicePlanCancelSignal { .. } => 59,
        HirExpressionChildRole::ChoicePlanCancelTimeout { .. } => 60,
        HirExpressionChildRole::ChoicePlanCancelExpr { .. } => 61,
    }
}

fn assert_edge_contract(kind: &HirExprKind, expected_children: &[ExprId], expected_roles: &[u8]) {
    let edges = kind.child_edges();
    assert_eq!(
        edges
            .iter()
            .map(HirExpressionChildEdge::child)
            .collect::<Vec<_>>(),
        expected_children,
        "child edge IDs drifted for {kind:?}"
    );
    assert_eq!(
        edges
            .iter()
            .map(|edge| edge_role_tag(edge.role()))
            .collect::<Vec<_>>(),
        expected_roles,
        "child edge roles drifted for {kind:?}"
    );
}

fn nested_path(segments: Vec<HirNestedExpressionPathSegment>) -> HirNestedExpressionPath {
    HirNestedExpressionPath::try_from_segments(segments.into_boxed_slice())
        .expect("nested edge paths are nonempty")
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the independent expected vectors are the differential contract for all 38 HIR families"
)]
fn child_edges_have_independent_expected_children_and_role_families_for_all_38_variants() {
    let module = module(150);
    let scope = id::<ScopeId>(module, 1);
    let scope_two = id::<ScopeId>(module, 2);
    let pattern = id::<PatternId>(module, 3);
    let statement = id::<StmtId>(module, 4);
    let first = id::<ExprId>(module, 10);
    let second = id::<ExprId>(module, 11);
    let third = id::<ExprId>(module, 12);
    let fourth = id::<ExprId>(module, 13);
    let callee = id::<ExprId>(module, 14);
    let entity = HirIdRefValue::Resolved(HirIdRef::absolute(
        HirEntityReference::try_new("scene.entry".into()).expect("test entity reference"),
    ));
    let lifetime = HirLifetimePathValue::Resolved(HirLifetimeRegistryPath::try_new(
        HirLifetimeRegistryScope::Frame,
        Box::new([name("value")]),
        false,
    ));
    let path = HirPathValue::Resolved(
        HirPath::try_new(
            HirPathRoot::ImplicitCrate,
            Box::new([HirPathSegment::Identifier(name("Value"))]),
        )
        .expect("test path"),
    );
    let thread = HirThreadExpr::new(
        None,
        HirThreadMode::Attached,
        HirThreadBody::try_new(
            HirThreadBodyOwner::ThreadExpression(first),
            scope,
            Box::new([HirThreadFlowItem::Statement(statement)]),
        )
        .expect("test thread body"),
    );
    let call = HirExprKind::Call(clean_call(
        HirCallCallee::value(callee),
        Box::new([
            HirCallArgument::positional(first),
            HirCallArgument::positional(second),
        ]),
    ));
    let match_arm_zero = HirMatchArm::try_new(scope, pattern, Some(second), third, Box::new([]))
        .expect("test match arm");
    let match_arm_one = HirMatchArm::try_new(scope_two, pattern, None, fourth, Box::new([]))
        .expect("test second match arm");
    let content_owner = id::<ExprId>(module, 20);
    let content = HirDialogueContent::try_new(
        HirDialogueContentId::new(content_owner),
        Box::new([]),
        Box::new([]),
    )
    .expect("empty dialogue content");
    let dialogue =
        HirDialogueContentApplication::try_new(content_owner, first, content, None, Box::new([]))
            .expect("dialogue application");
    let postfix = HirPostfixBracket::try_new(
        first,
        HirPostfixBracketCandidates::Ambiguous {
            index: second,
            dialogue: third,
        },
    )
    .expect("ambiguous postfix bracket");
    let numeric =
        HirNumericSequence::try_new(Box::new([]), None, HirNumericSequenceRecovery::Complete)
            .expect("empty numeric sequence");
    let record_path = HirPath::try_new(
        HirPathRoot::ImplicitCrate,
        Box::new([HirPathSegment::Identifier(name("Record"))]),
    )
    .expect("record path");

    let cases = vec![
        (HirExprKind::Unit, vec![], vec![]),
        (
            HirExprKind::Literal(HirLiteral::Boolean(true)),
            vec![],
            vec![],
        ),
        (HirExprKind::EntityReference(entity), vec![], vec![]),
        (HirExprKind::LifetimePath(lifetime), vec![], vec![]),
        (HirExprKind::Path(path), vec![], vec![]),
        (
            HirExprKind::ShortVariant(HirShortVariantName::Resolved(name("Ready"))),
            vec![],
            vec![],
        ),
        (
            HirExprKind::Placeholder(HirPlaceholderKind::PartialApplication),
            vec![],
            vec![],
        ),
        (
            HirExprKind::Tuple(HirTupleExpr::new(Box::new([first, second]))),
            vec![first, second],
            vec![0, 0],
        ),
        (
            HirExprKind::BracketSequence(HirBracketSequenceExpr::new(Box::new([first, second]))),
            vec![first, second],
            vec![0, 0],
        ),
        (HirExprKind::NumericBracketSequence(numeric), vec![], vec![]),
        (
            HirExprKind::ArrayRepeat(HirArrayRepeatExpr::new(first, second)),
            vec![first, second],
            vec![1, 2],
        ),
        (call, vec![callee, first, second], vec![3, 4, 4]),
        (
            HirExprKind::Select(HirSelectExpr::new(first, HirSelectedMember::Missing)),
            vec![first],
            vec![5],
        ),
        (
            HirExprKind::Index(HirIndexExpr::new(first, second)),
            vec![first, second],
            vec![5, 6],
        ),
        (
            HirExprKind::Pipe(HirPipeExpr::new(first, second)),
            vec![first, second],
            vec![7, 8],
        ),
        (
            HirExprKind::Try(HirTryExpr::new(first)),
            vec![first],
            vec![9],
        ),
        (
            HirExprKind::Await(
                HirAwaitExpr::try_new(first, Box::new([])).expect("empty await branches"),
            ),
            vec![first],
            vec![9],
        ),
        (HirExprKind::Thread(thread), vec![], vec![]),
        (
            HirExprKind::Choice(HirChoiceExpr::new(
                None,
                HirChoiceBody::new(scope, Box::new([])),
                None,
            )),
            vec![],
            vec![],
        ),
        (
            HirExprKind::Range(HirRangeExpr::new(Some(first), Some(second), false)),
            vec![first, second],
            vec![10, 11],
        ),
        (
            HirExprKind::Record(HirRecordExpr::new(
                record_path,
                Box::new([HirRecordField::explicit(name("field"), first)]),
            )),
            vec![first],
            vec![12],
        ),
        (
            HirExprKind::RecordLiteral(HirRecordLiteralExpr::new(Box::new([
                HirRecordField::explicit(name("field"), second),
            ]))),
            vec![second],
            vec![12],
        ),
        (
            HirExprKind::Binary(HirBinaryExpr::new(first, HirBinaryOp::Add, second)),
            vec![first, second],
            vec![13, 14],
        ),
        (
            HirExprKind::Borrow(HirBorrowExpr::new(HirBorrowKind::Shared, first)),
            vec![first],
            vec![9],
        ),
        (
            HirExprKind::Dereference(HirDereferenceExpr::new(first)),
            vec![first],
            vec![9],
        ),
        (
            HirExprKind::Closure(HirClosureExpr::new(
                scope,
                Box::new([]),
                None,
                first,
                Box::new([]),
            )),
            vec![first],
            vec![15],
        ),
        (
            HirExprKind::Unary(HirUnaryExpr::new(HirUnaryOp::Not, first)),
            vec![first],
            vec![9],
        ),
        (
            HirExprKind::Block(HirBlockExpr::new(scope, Box::new([]), first)),
            vec![first],
            vec![16],
        ),
        (
            HirExprKind::ComputationBlock(HirComputationBlockExpr::new(
                HirComputationBlockKind::Option,
                scope,
                Box::new([]),
                first,
            )),
            vec![first],
            vec![16],
        ),
        (
            HirExprKind::NamedBlock(HirNamedBlockExpr::new(
                HirNamedBlockName::Resolved(name("retry")),
                scope,
                Box::new([]),
                first,
            )),
            vec![first],
            vec![16],
        ),
        (
            HirExprKind::Loop(HirLoopExpr::new(scope, Box::new([]), first)),
            vec![first],
            vec![17],
        ),
        (
            HirExprKind::If(HirIfExpr::new(first, second, third)),
            vec![first, second, third],
            vec![18, 19, 20],
        ),
        (
            HirExprKind::IfLet(HirIfLetExpr::new(
                scope,
                pattern,
                first,
                Some(second),
                third,
                fourth,
            )),
            vec![first, second, third, fourth],
            vec![21, 24, 19, 20],
        ),
        (
            HirExprKind::Match(
                HirMatchExpr::try_new(first, Box::new([match_arm_zero, match_arm_one]))
                    .expect("same-module match expression"),
            ),
            vec![first, second, third, fourth],
            vec![21, 22, 23, 23],
        ),
        (
            HirExprKind::DialogueContentApplication(dialogue),
            vec![first],
            vec![25],
        ),
        (
            HirExprKind::PostfixBracket(postfix),
            vec![first, second, third],
            vec![5, 36, 37],
        ),
        (
            HirExprKind::Error(super::HirExprError::new(
                HirGenericExprIssue::UnclassifiedSyntax,
            )),
            vec![],
            vec![],
        ),
        (
            HirExprKind::ForSynthetic(super::HirForSyntheticExpr::iterator(first)),
            vec![first],
            vec![38],
        ),
    ];

    assert_eq!(cases.len(), 38);
    for (kind, expected_children, expected_roles) in cases {
        assert_edge_contract(&kind, &expected_children, &expected_roles);
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the exhaustive nested-choice differential keeps every expected order and path visible"
)]
#[test]
fn choice_child_edges_keep_multi_branch_else_and_match_arm_lifo_paths() {
    let module = module(151);
    let scope = id::<ScopeId>(module, 1);
    let pattern = id::<PatternId>(module, 2);
    let if_condition_zero = id::<ExprId>(module, 10);
    let if_condition_one = id::<ExprId>(module, 11);
    let match_scrutinee = id::<ExprId>(module, 12);
    let match_guard = id::<ExprId>(module, 13);
    let branch_zero_label = id::<ExprId>(module, 20);
    let branch_zero_out = id::<ExprId>(module, 21);
    let branch_one_label = id::<ExprId>(module, 22);
    let branch_one_out = id::<ExprId>(module, 23);
    let else_label = id::<ExprId>(module, 24);
    let else_out = id::<ExprId>(module, 25);
    let arm_zero_label = id::<ExprId>(module, 26);
    let arm_zero_out = id::<ExprId>(module, 27);
    let arm_one_label = id::<ExprId>(module, 28);
    let arm_one_out = id::<ExprId>(module, 29);
    let identity = HirIdRefValue::Resolved(HirIdRef::absolute(
        HirEntityReference::try_new("choice.option".into()).expect("choice identity"),
    ));
    let compact = |label, output| {
        HirChoiceItem::CompactArm(HirChoiceCompactArm::new(
            identity.clone(),
            label,
            None,
            HirChoiceCompactAction::Out(output),
        ))
    };
    let branch_zero = HirChoiceBody::new(
        scope,
        Box::new([compact(branch_zero_label, branch_zero_out)]),
    );
    let branch_one =
        HirChoiceBody::new(scope, Box::new([compact(branch_one_label, branch_one_out)]));
    let else_body = HirChoiceBody::new(scope, Box::new([compact(else_label, else_out)]));
    let arm_zero = HirChoiceBody::new(scope, Box::new([compact(arm_zero_label, arm_zero_out)]));
    let arm_one = HirChoiceBody::new(scope, Box::new([compact(arm_one_label, arm_one_out)]));
    let choice = HirExprKind::Choice(HirChoiceExpr::new(
        None,
        HirChoiceBody::new(
            scope,
            Box::new([
                HirChoiceItem::If(HirChoiceIf::new(
                    Box::new([
                        HirChoiceIfBranch::new(if_condition_zero, branch_zero),
                        HirChoiceIfBranch::new(if_condition_one, branch_one),
                    ]),
                    Some(else_body),
                )),
                HirChoiceItem::Match(HirChoiceMatch::new(
                    match_scrutinee,
                    Box::new([
                        HirChoiceMatchArm::new(pattern, Some(match_guard), arm_zero, Box::new([])),
                        HirChoiceMatchArm::new(pattern, None, arm_one, Box::new([])),
                    ]),
                )),
            ]),
        ),
        None,
    ));

    let edges = choice.child_edges();
    assert_eq!(
        edges
            .iter()
            .map(HirExpressionChildEdge::child)
            .collect::<Vec<_>>(),
        [
            if_condition_zero,
            if_condition_one,
            match_scrutinee,
            match_guard,
            arm_one_label,
            arm_one_out,
            arm_zero_label,
            arm_zero_out,
            else_label,
            else_out,
            branch_one_label,
            branch_one_out,
            branch_zero_label,
            branch_zero_out,
        ]
    );
    let p = |segments| nested_path(segments);
    let expected_roles = vec![
        HirExpressionChildRole::ChoiceIfCondition {
            path: p(vec![
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 0 },
                HirNestedExpressionPathSegment::ChoiceIfBranch { ordinal: 0 },
            ]),
            branch: 0,
        },
        HirExpressionChildRole::ChoiceIfCondition {
            path: p(vec![
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 0 },
                HirNestedExpressionPathSegment::ChoiceIfBranch { ordinal: 1 },
            ]),
            branch: 1,
        },
        HirExpressionChildRole::ChoiceMatchScrutinee {
            path: p(vec![HirNestedExpressionPathSegment::ChoiceBodyItem {
                ordinal: 1,
            }]),
        },
        HirExpressionChildRole::ChoiceMatchGuard {
            path: p(vec![
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 1 },
                HirNestedExpressionPathSegment::ChoiceMatchArm { ordinal: 0 },
            ]),
            arm: 0,
        },
        HirExpressionChildRole::ChoiceCompactLabel {
            path: p(vec![
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 1 },
                HirNestedExpressionPathSegment::ChoiceMatchArm { ordinal: 1 },
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 0 },
            ]),
        },
        HirExpressionChildRole::ChoiceCompactOut {
            path: p(vec![
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 1 },
                HirNestedExpressionPathSegment::ChoiceMatchArm { ordinal: 1 },
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 0 },
            ]),
        },
        HirExpressionChildRole::ChoiceCompactLabel {
            path: p(vec![
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 1 },
                HirNestedExpressionPathSegment::ChoiceMatchArm { ordinal: 0 },
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 0 },
            ]),
        },
        HirExpressionChildRole::ChoiceCompactOut {
            path: p(vec![
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 1 },
                HirNestedExpressionPathSegment::ChoiceMatchArm { ordinal: 0 },
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 0 },
            ]),
        },
        HirExpressionChildRole::ChoiceCompactLabel {
            path: p(vec![
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 0 },
                HirNestedExpressionPathSegment::ChoiceIfElse,
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 0 },
            ]),
        },
        HirExpressionChildRole::ChoiceCompactOut {
            path: p(vec![
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 0 },
                HirNestedExpressionPathSegment::ChoiceIfElse,
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 0 },
            ]),
        },
        HirExpressionChildRole::ChoiceCompactLabel {
            path: p(vec![
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 0 },
                HirNestedExpressionPathSegment::ChoiceIfBranch { ordinal: 1 },
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 0 },
            ]),
        },
        HirExpressionChildRole::ChoiceCompactOut {
            path: p(vec![
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 0 },
                HirNestedExpressionPathSegment::ChoiceIfBranch { ordinal: 1 },
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 0 },
            ]),
        },
        HirExpressionChildRole::ChoiceCompactLabel {
            path: p(vec![
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 0 },
                HirNestedExpressionPathSegment::ChoiceIfBranch { ordinal: 0 },
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 0 },
            ]),
        },
        HirExpressionChildRole::ChoiceCompactOut {
            path: p(vec![
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 0 },
                HirNestedExpressionPathSegment::ChoiceIfBranch { ordinal: 0 },
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 0 },
            ]),
        },
    ];
    assert_eq!(
        edges
            .iter()
            .map(|edge| edge.role().clone())
            .collect::<Vec<_>>(),
        expected_roles
    );
}

#[test]
fn dialogue_line_plan_edges_keep_sibling_group_deep_lifo_order_and_paths() {
    let module = module(152);
    let scope = id::<ScopeId>(module, 1);
    let owner = id::<ExprId>(module, 2);
    let target = id::<ExprId>(module, 3);
    let first = id::<ExprId>(module, 10);
    let second = id::<ExprId>(module, 11);
    let third = id::<ExprId>(module, 12);
    let fourth = id::<ExprId>(module, 13);
    let fifth = id::<ExprId>(module, 14);
    let sixth = id::<ExprId>(module, 15);
    let plan = HirLinePlan::try_new(
        scope,
        None,
        Box::new([
            HirLinePlanItem::StartGroup(Box::new([
                HirLinePlanItem::Expression(first),
                HirLinePlanItem::TogetherGroup(Box::new([
                    HirLinePlanItem::StartGroup(Box::new([HirLinePlanItem::Expression(second)])),
                    HirLinePlanItem::Expression(third),
                ])),
            ])),
            HirLinePlanItem::TogetherGroup(Box::new([
                HirLinePlanItem::Expression(fourth),
                HirLinePlanItem::StartGroup(Box::new([HirLinePlanItem::Expression(fifth)])),
            ])),
            HirLinePlanItem::Expression(sixth),
        ]),
    )
    .expect("line plan");
    let content =
        HirDialogueContent::try_new(HirDialogueContentId::new(owner), Box::new([]), Box::new([]))
            .expect("empty dialogue content");
    let kind = HirExprKind::DialogueContentApplication(
        HirDialogueContentApplication::try_new(owner, target, content, Some(plan), Box::new([]))
            .expect("dialogue line plan application"),
    );

    let edges = kind.child_edges();
    assert_eq!(
        edges
            .iter()
            .map(HirExpressionChildEdge::child)
            .collect::<Vec<_>>(),
        [target, sixth, fourth, fifth, first, third, second]
    );
    let p = |segments| nested_path(segments);
    let expected_roles = vec![
        HirExpressionChildRole::DialogueTarget,
        HirExpressionChildRole::LinePlanExpression {
            path: p(vec![HirNestedExpressionPathSegment::LinePlanItem {
                ordinal: 2,
            }]),
        },
        HirExpressionChildRole::LinePlanExpression {
            path: p(vec![
                HirNestedExpressionPathSegment::LinePlanItem { ordinal: 1 },
                HirNestedExpressionPathSegment::LinePlanTogetherGroupItem { ordinal: 0 },
            ]),
        },
        HirExpressionChildRole::LinePlanExpression {
            path: p(vec![
                HirNestedExpressionPathSegment::LinePlanItem { ordinal: 1 },
                HirNestedExpressionPathSegment::LinePlanTogetherGroupItem { ordinal: 1 },
                HirNestedExpressionPathSegment::LinePlanStartGroupItem { ordinal: 0 },
            ]),
        },
        HirExpressionChildRole::LinePlanExpression {
            path: p(vec![
                HirNestedExpressionPathSegment::LinePlanItem { ordinal: 0 },
                HirNestedExpressionPathSegment::LinePlanStartGroupItem { ordinal: 0 },
            ]),
        },
        HirExpressionChildRole::LinePlanExpression {
            path: p(vec![
                HirNestedExpressionPathSegment::LinePlanItem { ordinal: 0 },
                HirNestedExpressionPathSegment::LinePlanStartGroupItem { ordinal: 1 },
                HirNestedExpressionPathSegment::LinePlanTogetherGroupItem { ordinal: 1 },
            ]),
        },
        HirExpressionChildRole::LinePlanExpression {
            path: p(vec![
                HirNestedExpressionPathSegment::LinePlanItem { ordinal: 0 },
                HirNestedExpressionPathSegment::LinePlanStartGroupItem { ordinal: 1 },
                HirNestedExpressionPathSegment::LinePlanTogetherGroupItem { ordinal: 0 },
                HirNestedExpressionPathSegment::LinePlanStartGroupItem { ordinal: 0 },
            ]),
        },
    ];
    assert_eq!(
        edges
            .iter()
            .map(|edge| edge.role().clone())
            .collect::<Vec<_>>(),
        expected_roles
    );
}

fn final_variant_ordinal(kind: &HirExprKind) -> u8 {
    match kind {
        HirExprKind::Unit => 0,
        HirExprKind::Literal(_) => 1,
        HirExprKind::EntityReference(_) => 2,
        HirExprKind::LifetimePath(_) => 3,
        HirExprKind::Path(_) => 4,
        HirExprKind::ShortVariant(_) => 5,
        HirExprKind::Placeholder(_) => 6,
        HirExprKind::Tuple(_) => 7,
        HirExprKind::BracketSequence(_) => 8,
        HirExprKind::NumericBracketSequence(_) => 9,
        HirExprKind::ArrayRepeat(_) => 10,
        HirExprKind::Call(_) => 11,
        HirExprKind::Select(_) => 12,
        HirExprKind::Index(_) => 13,
        HirExprKind::Pipe(_) => 14,
        HirExprKind::Try(_) => 15,
        HirExprKind::Await(_) => 16,
        HirExprKind::Thread(_) => 17,
        HirExprKind::Choice(_) => 18,
        HirExprKind::Range(_) => 19,
        HirExprKind::Record(_) => 20,
        HirExprKind::RecordLiteral(_) => 21,
        HirExprKind::Binary(_) => 22,
        HirExprKind::Borrow(_) => 23,
        HirExprKind::Dereference(_) => 24,
        HirExprKind::Closure(_) => 25,
        HirExprKind::Unary(_) => 26,
        HirExprKind::Block(_) => 27,
        HirExprKind::ComputationBlock(_) => 28,
        HirExprKind::NamedBlock(_) => 29,
        HirExprKind::Loop(_) => 30,
        HirExprKind::If(_) => 31,
        HirExprKind::IfLet(_) => 32,
        HirExprKind::Match(_) => 33,
        HirExprKind::DialogueContentApplication(_) => 34,
        HirExprKind::PostfixBracket(_) => 35,
        HirExprKind::Error(_) => 36,
        HirExprKind::ForSynthetic(_) => 37,
    }
}

#[test]
fn expression_inventory_is_the_closed_38_variant_contract() {
    assert_eq!(final_variant_ordinal(&HirExprKind::Unit), 0);
    assert_eq!(
        final_variant_ordinal(&HirExprKind::Error(super::HirExprError::new(
            HirGenericExprIssue::UnclassifiedSyntax,
        ))),
        36
    );
    assert_eq!(
        final_variant_ordinal(&HirExprKind::ForSynthetic(
            super::HirForSyntheticExpr::iterator(id(module(1), 1)),
        )),
        37
    );
}
