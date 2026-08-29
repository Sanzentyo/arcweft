use core::num::{NonZeroU32, NonZeroU64};

use super::{
    HirExpressionOwnedBodyRole, HirExpressionOwnedChild, HirExpressionOwnedChildEdge,
    HirLinePlanStatementRole,
};
use crate::body_edges::{HirBodyChild, HirBodyChildRole};
use crate::dialogue_application::{
    HirDialogueContent, HirDialogueContentApplication, HirDialogueContentId, HirLinePlan,
    HirLinePlanItem,
};
use crate::expr::{
    HirAwaitBranch, HirAwaitBranchKind, HirAwaitExpr, HirChoiceBody, HirChoiceExpr, HirChoiceFor,
    HirChoiceItem, HirChoiceMatch, HirChoiceMatchArm, HirChoiceOptionBody, HirChoiceOptionField,
    HirChoiceOptionFor, HirChoicePlan, HirChoicePlanItem, HirExprKind,
    HirNestedExpressionPathSegment, HirThreadBody, HirThreadBodyOwner, HirThreadFlowItem,
};
use crate::identity::{
    ExprId, HirDatabaseId, HirModuleId, HirTypedId, LocalId, PatternId, RawHirId, ScopeId, StmtId,
};
use crate::stmt::{HirContextualStmtBody, HirTrigger};

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

#[test]
fn await_edges_keep_pattern_and_body_source_order() {
    let module = module(153);
    let scope = id::<ScopeId>(module, 1);
    let operand = id::<ExprId>(module, 2);
    let pattern = id::<PatternId>(module, 3);
    let local = id::<LocalId>(module, 4);
    let statement = id::<StmtId>(module, 5);
    let body = HirContextualStmtBody::try_ordinary(scope, Box::new([statement]))
        .expect("ordinary Await branch body");
    let branch = HirAwaitBranch::try_new(
        HirAwaitBranchKind::Pending,
        Some(pattern),
        Box::new([local]),
        body,
    )
    .expect("pending Await branch");
    let kind = HirExprKind::Await(
        HirAwaitExpr::try_new(operand, Box::new([branch])).expect("Await expression"),
    );

    let edges = kind
        .expression_owned_child_edges()
        .expect("checked Await topology");
    assert_eq!(edges.len(), 2);
    assert_eq!(edges[0].child(), HirExpressionOwnedChild::Pattern(pattern));
    assert_eq!(
        edges[0].role(),
        &HirExpressionOwnedBodyRole::AwaitBranchPattern { branch: 0 }
    );
    let HirExpressionOwnedChild::Body(body_edge) = edges[1].child() else {
        panic!("Await body edge");
    };
    assert_eq!(body_edge.child(), HirBodyChild::Statement(statement));
    assert_eq!(body_edge.role(), HirBodyChildRole::Statement { ordinal: 0 });
    assert_eq!(
        edges[1].role(),
        &HirExpressionOwnedBodyRole::AwaitBranchBody { branch: 0 }
    );
}

#[test]
fn closure_and_if_let_patterns_are_expression_owned_edges() {
    let module = module(156);
    let scope = id::<ScopeId>(module, 1);
    let pattern = id::<PatternId>(module, 2);
    let body = id::<ExprId>(module, 3);
    let closure = HirExprKind::Closure(crate::expr::HirClosureExpr::new(
        scope,
        Box::new([
            crate::expr::HirClosureParameter::try_new(pattern, None, scope)
                .expect("closure parameter"),
        ]),
        None,
        body,
        Box::new([]),
    ));
    let closure_edges = closure
        .expression_owned_child_edges()
        .expect("closure owned topology");
    assert!(matches!(
        closure_edges.first(),
        Some(edge)
            if edge.child() == HirExpressionOwnedChild::Pattern(pattern)
                && matches!(
                    edge.role(),
                    HirExpressionOwnedBodyRole::ClosureParameterPattern { parameter: 0 }
                )
    ));

    let scrutinee = id::<ExprId>(module, 4);
    let then_branch = id::<ExprId>(module, 5);
    let else_branch = id::<ExprId>(module, 6);
    let if_let = HirExprKind::IfLet(crate::expr::HirIfLetExpr::new(
        scope,
        pattern,
        scrutinee,
        None,
        then_branch,
        else_branch,
    ));
    let if_let_edges = if_let
        .expression_owned_child_edges()
        .expect("if-let owned topology");
    assert!(matches!(
        if_let_edges.first(),
        Some(edge)
            if edge.child() == HirExpressionOwnedChild::Pattern(pattern)
                && matches!(edge.role(), HirExpressionOwnedBodyRole::IfLetPattern)
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one golden Choice inventory keeps all ten logical families and their nested source order auditable together"
)]
fn choice_edges_cover_ten_logical_roles_with_typed_nested_paths() {
    let module = module(154);
    let scope = id::<ScopeId>(module, 1);
    let source = id::<ExprId>(module, 2);
    let statements = (10..16)
        .map(|slot| id::<StmtId>(module, slot))
        .collect::<Vec<_>>();
    let patterns = (20..25)
        .map(|slot| id::<PatternId>(module, slot))
        .collect::<Vec<_>>();
    let empty_body = || HirChoiceBody::new(scope, Box::new([]));
    let thread_body = |statement| {
        HirThreadBody::try_new(
            HirThreadBodyOwner::NestedScope(scope),
            scope,
            Box::new([HirThreadFlowItem::Statement(statement)]),
        )
        .expect("nested Choice thread body")
    };
    let choice = HirExprKind::Choice(HirChoiceExpr::new(
        None,
        HirChoiceBody::new(
            scope,
            Box::new([
                HirChoiceItem::Let(statements[0]),
                HirChoiceItem::For(HirChoiceFor::new(
                    patterns[0],
                    source,
                    empty_body(),
                    Box::new([]),
                )),
                HirChoiceItem::Match(HirChoiceMatch::new(
                    source,
                    Box::new([HirChoiceMatchArm::new(
                        patterns[1],
                        None,
                        empty_body(),
                        Box::new([]),
                    )]),
                )),
                HirChoiceItem::OptionFor(HirChoiceOptionFor::new(
                    patterns[2],
                    source,
                    HirChoiceOptionBody::new(
                        scope,
                        Box::new([
                            HirChoiceOptionField::Select(thread_body(statements[1])),
                            HirChoiceOptionField::Let(statements[2]),
                        ]),
                    ),
                    Box::new([]),
                )),
            ]),
        ),
        Some(HirChoicePlan::new(Box::new([
            HirChoicePlanItem::Timeout {
                duration: source,
                body: thread_body(statements[3]),
            },
            HirChoicePlanItem::Cancel {
                trigger: HirTrigger::Input(patterns[3]),
                body: thread_body(statements[4]),
            },
            HirChoicePlanItem::OnSelect {
                pattern: patterns[4],
                locals: Box::new([]),
                body: thread_body(statements[5]),
            },
        ]))),
    ));

    let edges = choice
        .expression_owned_child_edges()
        .expect("checked Choice topology");
    assert_eq!(edges.len(), 11);
    assert!(matches!(
        edges[0].role(),
        HirExpressionOwnedBodyRole::ChoiceLetStatement { path }
            if path.segments() == [HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 0 }]
    ));
    assert!(matches!(
        edges[1].role(),
        HirExpressionOwnedBodyRole::ChoiceForPattern { .. }
    ));
    assert!(matches!(
        edges[2].role(),
        HirExpressionOwnedBodyRole::ChoiceMatchArmPattern { arm: 0, .. }
    ));
    assert!(matches!(
        edges[3].role(),
        HirExpressionOwnedBodyRole::ChoiceOptionForPattern { .. }
    ));
    assert!(matches!(
        edges[4].role(),
        HirExpressionOwnedBodyRole::ChoiceOptionSelectBody { path, field: 0 }
            if path.segments() == [
                HirNestedExpressionPathSegment::ChoiceBodyItem { ordinal: 3 },
                HirNestedExpressionPathSegment::ChoiceOptionBody,
                HirNestedExpressionPathSegment::ChoiceOptionField { ordinal: 0 },
            ]
    ));
    assert!(matches!(
        edges[5].role(),
        HirExpressionOwnedBodyRole::ChoiceOptionLetStatement { field: 1, .. }
    ));
    assert!(matches!(
        edges[6].role(),
        HirExpressionOwnedBodyRole::ChoicePlanTimeoutBody { .. }
    ));
    assert!(matches!(
        edges[7].role(),
        HirExpressionOwnedBodyRole::ChoicePlanCancelTrigger { .. }
    ));
    assert_eq!(
        edges[7].child(),
        HirExpressionOwnedChild::Pattern(patterns[3])
    );
    assert!(matches!(
        edges[8].role(),
        HirExpressionOwnedBodyRole::ChoicePlanCancelBody { .. }
    ));
    assert!(matches!(
        edges[9].role(),
        HirExpressionOwnedBodyRole::ChoicePlanOnSelectPattern { .. }
    ));
    assert!(matches!(
        edges[10].role(),
        HirExpressionOwnedBodyRole::ChoicePlanOnSelectBody { .. }
    ));
    let golden = edges
        .iter()
        .map(|edge| match edge.role() {
            HirExpressionOwnedBodyRole::ChoiceLetStatement { .. } => 0,
            HirExpressionOwnedBodyRole::ChoiceForPattern { .. } => 1,
            HirExpressionOwnedBodyRole::ChoiceMatchArmPattern { .. } => 2,
            HirExpressionOwnedBodyRole::ChoiceOptionForPattern { .. } => 3,
            HirExpressionOwnedBodyRole::ChoiceOptionSelectBody { .. } => 4,
            HirExpressionOwnedBodyRole::ChoiceOptionLetStatement { .. } => 5,
            HirExpressionOwnedBodyRole::ChoicePlanTimeoutBody { .. } => 6,
            HirExpressionOwnedBodyRole::ChoicePlanCancelTrigger { .. } => 7,
            HirExpressionOwnedBodyRole::ChoicePlanCancelBody { .. } => 8,
            HirExpressionOwnedBodyRole::ChoicePlanOnSelectPattern { .. } => 9,
            HirExpressionOwnedBodyRole::ChoicePlanOnSelectBody { .. } => 10,
            _ => u8::MAX,
        })
        .collect::<Vec<_>>();
    assert_eq!(golden, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
}

#[test]
fn dialogue_edges_keep_six_statement_roles_and_group_kinds() {
    let module = module(155);
    let scope = id::<ScopeId>(module, 1);
    let owner = id::<ExprId>(module, 2);
    let target = id::<ExprId>(module, 3);
    let statements = (10..17)
        .map(|slot| id::<StmtId>(module, slot))
        .collect::<Vec<_>>();
    let plan = HirLinePlan::try_new(
        scope,
        None,
        Box::new([
            HirLinePlanItem::Init(Box::new([statements[0], statements[1]])),
            HirLinePlanItem::StartGroup(Box::new([
                HirLinePlanItem::Thread(statements[2]),
                HirLinePlanItem::TogetherGroup(Box::new([
                    HirLinePlanItem::On(statements[3]),
                    HirLinePlanItem::Statement(statements[4]),
                    HirLinePlanItem::CancelRule(statements[5]),
                    HirLinePlanItem::Error(statements[6]),
                ])),
            ])),
        ]),
    )
    .expect("line plan");
    let content = HirDialogueContent::try_new(
        HirDialogueContentId::new(owner),
        Box::new([]),
        Box::new([]),
        Box::new([]),
    )
    .expect("empty dialogue content");
    let dialogue = HirExprKind::DialogueContentApplication(
        HirDialogueContentApplication::try_new(owner, target, content, Some(plan), Box::new([]))
            .expect("dialogue application"),
    );

    let edges = dialogue
        .expression_owned_child_edges()
        .expect("checked dialogue topology");
    assert_dialogue_edges(&edges);
}

fn assert_dialogue_edges(edges: &[HirExpressionOwnedChildEdge]) {
    assert_eq!(edges.len(), 7);
    assert!(matches!(
        edges[0].role(),
        HirExpressionOwnedBodyRole::DialogueLinePlanStatement {
            role: HirLinePlanStatementRole::Init { statement: 0 },
            ..
        }
    ));
    assert!(matches!(
        edges[1].role(),
        HirExpressionOwnedBodyRole::DialogueLinePlanStatement {
            role: HirLinePlanStatementRole::Init { statement: 1 },
            ..
        }
    ));
    assert!(matches!(
        edges[2].role(),
        HirExpressionOwnedBodyRole::DialogueLinePlanStatement {
            path,
            role: HirLinePlanStatementRole::Thread,
        } if path.segments() == [
            HirNestedExpressionPathSegment::LinePlanItem { ordinal: 1 },
            HirNestedExpressionPathSegment::LinePlanStartGroupItem { ordinal: 0 },
        ]
    ));
    assert!(matches!(
        edges[3].role(),
        HirExpressionOwnedBodyRole::DialogueLinePlanStatement {
            path,
            role: HirLinePlanStatementRole::On,
        } if path.segments() == [
            HirNestedExpressionPathSegment::LinePlanItem { ordinal: 1 },
            HirNestedExpressionPathSegment::LinePlanStartGroupItem { ordinal: 1 },
            HirNestedExpressionPathSegment::LinePlanTogetherGroupItem { ordinal: 0 },
        ]
    ));
    assert!(matches!(
        edges[4].role(),
        HirExpressionOwnedBodyRole::DialogueLinePlanStatement {
            role: HirLinePlanStatementRole::Statement,
            ..
        }
    ));
    assert!(matches!(
        edges[5].role(),
        HirExpressionOwnedBodyRole::DialogueLinePlanStatement {
            role: HirLinePlanStatementRole::CancelRule,
            ..
        }
    ));
    assert!(matches!(
        edges[6].role(),
        HirExpressionOwnedBodyRole::DialogueLinePlanStatement {
            role: HirLinePlanStatementRole::Error,
            ..
        }
    ));
    let golden = edges
        .iter()
        .map(|edge| match edge.role() {
            HirExpressionOwnedBodyRole::DialogueLinePlanStatement {
                role: HirLinePlanStatementRole::Init { .. },
                ..
            } => 0,
            HirExpressionOwnedBodyRole::DialogueLinePlanStatement {
                role: HirLinePlanStatementRole::Thread,
                ..
            } => 1,
            HirExpressionOwnedBodyRole::DialogueLinePlanStatement {
                role: HirLinePlanStatementRole::On,
                ..
            } => 2,
            HirExpressionOwnedBodyRole::DialogueLinePlanStatement {
                role: HirLinePlanStatementRole::Statement,
                ..
            } => 3,
            HirExpressionOwnedBodyRole::DialogueLinePlanStatement {
                role: HirLinePlanStatementRole::CancelRule,
                ..
            } => 4,
            HirExpressionOwnedBodyRole::DialogueLinePlanStatement {
                role: HirLinePlanStatementRole::Error,
                ..
            } => 5,
            _ => u8::MAX,
        })
        .collect::<Vec<_>>();
    assert_eq!(golden, [0, 0, 1, 2, 3, 4, 5]);
}
