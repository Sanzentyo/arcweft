use super::{
    HirArrayRepeatExpr, HirAssociatedCallSyntax, HirAssociatedReceiver, HirAwaitExpr,
    HirAwaitPropagation, HirBinaryExpr, HirBinaryOp, HirBlockExpr, HirBorrowExpr, HirBorrowKind,
    HirBracketSequenceExpr, HirCallArgument, HirCallArgumentListTerminator, HirCallArgumentOrdinal,
    HirCallBuildError, HirCallCallee, HirCallChildPoison, HirCallChildStates, HirCallExpr,
    HirCallIssue, HirCallTypeApplication, HirChoiceBody, HirChoiceExpr, HirChoiceItem,
    HirClosureExpr, HirClosureParameter, HirComputationBlockExpr, HirComputationBlockKind,
    HirDereferenceExpr, HirExpr, HirExprInvariantError, HirExprKind, HirExpressionRecoveryIssue,
    HirGenericExprIssue, HirIfExpr, HirIfLetExpr, HirIndexExpr, HirMatchArm, HirMatchExpr,
    HirNamedBlockExpr, HirNamedBlockName, HirPipeExpr, HirPlaceholderKind, HirPoisonState,
    HirRangeExpr, HirRecordExpr, HirRecordField, HirRecordLiteralExpr, HirRecoveredName,
    HirRecoveryIssue, HirSelectExpr, HirSelectedMember, HirThreadBody, HirThreadBodyInvariantError,
    HirThreadBodyOwner, HirThreadExpr, HirThreadFlowItem, HirThreadMode, HirTryExpr, HirTryForm,
    HirTupleExpr, HirUnaryExpr, HirUnaryOp,
};
use crate::dialogue_application::{
    HirDialogueContent, HirDialogueContentApplication, HirDialogueContentId, HirDialogueCoordinate,
    HirDialogueNode, HirDialogueNodeId, HirDialogueNodeKind, HirPostfixBracket,
    HirPostfixBracketCandidates, HirPostfixCandidateFailure, HirPostfixCandidateFailureKind,
    HirRichTextArgument, HirRichTextArgumentId, HirRichTextTag, HirRichTextTagId,
    HirRichTextTagIdentity, HirRichTextTagPayload, HirRichTextValue, HirTextFragment,
};
use crate::identity::{
    ExprId, HirDatabaseId, HirLimit, HirModuleId, HirTypedId, LocalId, PatternId, RawHirId,
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
fn call_constructor_enforces_limit_ordering_and_module_identity() {
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
        &[positional_issue.clone()]
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
        &[spread_issue.clone()]
    );
    assert_eq!(
        spread_state,
        HirPoisonState::Poisoned(HirRecoveryIssue::InvalidCall(spread_issue))
    );
    assert_eq!(
        HirCallExpr::try_new(
            HirCallCallee::value(callee),
            HirCallTypeApplication::absent(),
            Box::new([HirCallArgument::positional(foreign)]),
            HirCallArgumentListTerminator::Closed,
            HirCallChildStates::new(HirCallChildPoison::Clean, &[HirCallChildPoison::Clean], &[],),
            false,
        ),
        Err(HirCallBuildError::ChildIdentityMismatch)
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
        &[issue.clone()]
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
            HirRecoveredName::Valid(name("with_capacity")),
            HirAssociatedCallSyntax::DotFallback,
        ),
        Box::new([HirCallArgument::positional(argument)]),
    );

    let HirCallCallee::Associated {
        receiver,
        member,
        syntax,
    } = call.callee()
    else {
        panic!("associated type callee");
    };
    assert_eq!(receiver.type_id(), root);
    assert_eq!(
        member.resolved().map(HirName::as_str),
        Some("with_capacity")
    );
    assert_eq!(*syntax, HirAssociatedCallSyntax::DotFallback);
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
    let thread =
        HirThreadExpr::try_new(None, HirThreadMode::Attached, body).expect("same-module thread");
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
            HirThreadBodyOwner::ThreadExpression(owner),
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
    let thread = HirThreadExpr::try_new(
        None,
        HirThreadMode::Attached,
        HirThreadBody::try_new(
            HirThreadBodyOwner::ThreadExpression(owner),
            scope,
            Box::new([HirThreadFlowItem::Statement(statement)]),
        )
        .expect("test thread body"),
    )
    .expect("test thread");
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
            HirExprKind::Try(HirTryExpr::new(first, HirTryForm::PrefixTry)),
            HirExprSourceRole::Operator,
        ),
        (
            HirExprKind::Await(HirAwaitExpr::new(
                first,
                HirAwaitPropagation::PreserveResult,
            )),
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
                HirComputationBlockKind::Task,
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
            HirRecoveredName::Valid(name("with_capacity")),
            HirAssociatedCallSyntax::DotFallback,
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
        HirRichTextTagIdentity::Marker(name("voice")),
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

// Keeping this match exhaustive provides typed evidence for the closed
// 37-variant inventory without inspecting implementation source text.
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
        HirExprKind::If(_) => 30,
        HirExprKind::IfLet(_) => 31,
        HirExprKind::Match(_) => 32,
        HirExprKind::DialogueContentApplication(_) => 33,
        HirExprKind::PostfixBracket(_) => 34,
        HirExprKind::Error(_) => 35,
        HirExprKind::ForSynthetic(_) => 36,
    }
}

#[test]
fn expression_inventory_is_the_closed_37_variant_contract() {
    assert_eq!(final_variant_ordinal(&HirExprKind::Unit), 0);
    assert_eq!(
        final_variant_ordinal(&HirExprKind::Error(super::HirExprError::new(
            HirGenericExprIssue::UnclassifiedSyntax,
        ))),
        35
    );
    assert_eq!(
        final_variant_ordinal(&HirExprKind::ForSynthetic(
            super::HirForSyntheticExpr::iterator(id(module(1), 1)),
        )),
        36
    );
}
