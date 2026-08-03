use super::*;
use crate::assertion::AssertionMode;
use crate::attachment::{
    AttachedCandidateExpressionChild, AttachedCandidateIfElse, AttachedCandidateIfHead,
    AttachedCandidateKeywordStatement, AttachedCandidateMatchArmBody, AttachedCandidateMatchBody,
    AttachedCandidateNode, AttachedCandidatePathExpression, AttachedCandidateUnsafeAuditId,
    AttachedCandidateUnsafeBody,
};
use crate::expressions::{ExpressionComponentRole, SyntaxClosureParameterPart, SyntaxMatchArmPart};
use crate::grammar::build::GrammarBuildError;
use crate::incremental::SyntaxLimit;
use crate::types::TypeRefNodeStep;

fn with_index_primary(source: &str, inspect: impl FnOnce(AttachedCandidateNode<'_>)) {
    let outer = expression(source, SyntaxKind::PostfixBracketExpression);
    let primary = outer
        .ambiguous_index_candidate()
        .expect("ordinary-index candidate")
        .primary()
        .expect("ordinary-index primary");
    inspect(primary);
}

#[test]
fn candidate_closure_view_preserves_parameter_pattern_type_and_body_relations() {
    with_index_primary("items[|value: Pair| value]", |primary| {
        let closure = primary
            .closure_view()
            .expect("typed candidate Closure view");
        let [parameter] = closure.parameters() else {
            panic!("one candidate Closure parameter");
        };
        assert_eq!(parameter.ordinal(), 0);
        assert!(parameter.ty().is_some());
        assert!(parameter.pattern().children().is_some());
        assert_eq!(
            parameter
                .component(SyntaxClosureParameterPart::Pattern)
                .expect("parameter Pattern source")
                .range(),
            parameter.pattern().whole_source_span().range()
        );
        assert!(matches!(
            closure.body(),
            AttachedCandidateExpressionChild::Authored { ordinal: 0, .. }
        ));
    });
}

#[test]
fn candidate_if_let_view_distinguishes_missing_then_from_authored_else() {
    with_index_primary("items[if let value = source else fallback]", |primary| {
        let if_let = primary.if_let_view().expect("typed candidate IfLet view");
        assert!(matches!(
            if_let.scrutinee(),
            AttachedCandidateExpressionChild::Authored { ordinal: 0, .. }
        ));
        assert!(if_let.guard().is_none());
        assert!(matches!(
            if_let.then_branch(),
            AttachedCandidateExpressionChild::Missing { ordinal: 2, .. }
        ));
        assert!(matches!(
            if_let.else_branch(),
            Some(AttachedCandidateExpressionChild::Authored { ordinal: 3, .. })
        ));
        assert_ne!(
            if_let.else_source_span().range().start(),
            if_let.else_source_span().range().end()
        );
    });
}

#[test]
fn candidate_match_view_keeps_arm_wrappers_as_scope_boundaries() {
    with_index_primary(
        "items[match source { value: I32 when ready => result, _ => fallback }]",
        |primary| {
            let match_expression = primary.match_view().expect("typed candidate Match view");
            let [first, second] = match_expression.arms() else {
                panic!("two candidate Match arms");
            };
            assert_eq!(first.ordinal(), 0);
            assert_eq!(second.ordinal(), 1);
            assert!(first.guard().is_some());
            assert!(second.guard().is_none());
            assert!(matches!(
                first.value(),
                AttachedCandidateExpressionChild::Authored { .. }
            ));
            assert!(first.component(SyntaxMatchArmPart::Pattern).is_some());
            assert!(second.component(SyntaxMatchArmPart::Pattern).is_some());

            let direct = primary.semantic_expression_children().collect::<Vec<_>>();
            assert_eq!(direct.len(), 1, "arm-owned children must not flatten");
            assert_eq!(direct[0].node().role(), SyntaxRole::Scrutinee);
        },
    );
}

#[test]
fn candidate_value_blocks_preserve_statement_order_and_tail_state() {
    with_index_primary("items[{ let value: I32 = 1; value }]", |primary| {
        let block = primary.value_block_view().expect("typed candidate Block");
        let [statement] = block.statements() else {
            panic!("one source-ordered candidate statement");
        };
        assert_eq!(statement.ordinal(), 0);
        assert_eq!(statement.kind(), SyntaxKind::LetStatement);
        assert!(statement.required_pattern(SyntaxRole::Pattern).is_some());
        assert!(matches!(
            block.tail(),
            crate::attachment::AttachedCandidateBlockTail::Expression(_)
        ));
        assert!(block.is_closed());
    });

    for source in [
        "items[result { let value = 1; }]",
        "items[scope retry { marker; }]",
    ] {
        with_index_primary(source, |primary| {
            let block = primary
                .value_block_view()
                .expect("typed candidate value block");
            assert_eq!(block.statements().len(), 1);
            assert!(matches!(
                block.tail(),
                crate::attachment::AttachedCandidateBlockTail::Omitted { .. }
            ));
        });
    }
}

#[test]
fn candidate_statement_relations_do_not_flatten_nested_blocks() {
    with_index_primary(
        "items[{ if condition { marker; } else { fallback; }; }]",
        |primary| {
            let block = primary.value_block_view().expect("typed candidate Block");
            let [statement] = block.statements() else {
                panic!("one outer If statement");
            };
            assert_eq!(statement.kind(), SyntaxKind::IfStatement);
            let conditional = statement.if_view().expect("typed if relation");
            assert!(matches!(
                conditional.head(),
                AttachedCandidateIfHead::Condition(_)
            ));
            let Some(AttachedCandidateIfElse::Block(else_branch)) = conditional.else_branch()
            else {
                panic!("typed else statement block");
            };
            assert_eq!(conditional.then_branch().statements().len(), 1);
            assert_eq!(else_branch.statements().len(), 1);
            assert!(conditional.then_branch().is_closed());
            assert!(else_branch.is_closed());
        },
    );
}

#[test]
fn candidate_assertion_view_retains_mode_delimiters_and_conditions() {
    with_index_primary("items[{ assert.check(true, false); marker }]", |primary| {
        let block = primary.value_block_view().expect("typed candidate Block");
        let [statement] = block.statements() else {
            panic!("one assertion statement");
        };
        let assertion = statement.assertion_view().expect("typed assertion view");
        assert_eq!(assertion.mode(), Some(AssertionMode::Check));
        assert_eq!(assertion.conditions().len(), 2);
        assert!(!assertion.has_recovery());
        assert!(!assertion.open_delimiter().source_span().range().is_empty());
        assert!(!assertion.close_delimiter().source_span().range().is_empty());
    });
}

#[test]
fn candidate_if_and_match_views_preserve_nested_statement_owners() {
    with_index_primary(
        "items[{ if let value = source when ready { value; } else if fallback { marker; }; }]",
        |primary| {
            let block = primary.value_block_view().expect("typed candidate Block");
            let [statement] = block.statements() else {
                panic!("one if-let statement");
            };
            let conditional = statement.if_view().expect("typed candidate if");
            assert!(matches!(
                conditional.head(),
                AttachedCandidateIfHead::Let { guard: Some(_), .. }
            ));
            assert_eq!(conditional.then_branch().statements().len(), 1);
            assert!(matches!(
                conditional.else_branch(),
                Some(AttachedCandidateIfElse::If(_))
            ));
        },
    );

    with_index_primary(
        "items[{ match subject { value when ready => value, _ => { marker; } }; }]",
        |primary| {
            let block = primary.value_block_view().expect("typed candidate Block");
            let [statement] = block.statements() else {
                panic!("one Match statement");
            };
            let matched = statement.match_view().expect("typed candidate Match");
            let AttachedCandidateMatchBody::Block { arms, .. } = matched.body() else {
                panic!("authored Match body");
            };
            let [first, second] = arms.as_ref() else {
                panic!("two Match arms");
            };
            assert!(first.guard().is_some());
            assert!(matches!(
                first.body(),
                AttachedCandidateMatchArmBody::Expression(_)
            ));
            assert!(matches!(
                second.body(),
                AttachedCandidateMatchArmBody::Block(_)
            ));
        },
    );
}

#[test]
fn candidate_unsafe_view_keeps_id_reason_safety_doc_and_missing_body() {
    with_index_primary(
        "items[{ unsafe lifetime @unsafe.audit reason = \"bounded\" { /// SAFETY: owned\n marker; }; }]",
        |primary| {
            let block = primary.value_block_view().expect("typed candidate Block");
            let [statement] = block.statements() else {
                panic!("one unsafe statement");
            };
            let audit = statement
                .unsafe_lifetime_view()
                .expect("typed candidate unsafe lifetime");
            assert!(matches!(
                audit.audit_id(),
                AttachedCandidateUnsafeAuditId::Reference(_)
            ));
            assert!(audit.reason().is_some());
            let AttachedCandidateUnsafeBody::Block(body) = audit.body() else {
                panic!("authored unsafe body");
            };
            assert_eq!(body.safety_documentation().len(), 1);
            assert_eq!(body.statements().len(), 1);
        },
    );

    with_index_primary("items[{ unsafe lifetime @unsafe.audit; }]", |primary| {
        let block = primary.value_block_view().expect("typed candidate Block");
        let [statement] = block.statements() else {
            panic!("one unsafe statement");
        };
        let audit = statement
            .unsafe_lifetime_view()
            .expect("typed missing unsafe body");
        assert!(matches!(
            audit.body(),
            AttachedCandidateUnsafeBody::Missing(_)
        ));
    });
}

#[test]
fn candidate_statement_recovery_views_keep_the_recognized_family() {
    with_index_primary("items[{ assert.assume(); marker }]", |primary| {
        let block = primary.value_block_view().expect("typed candidate Block");
        let [statement] = block.statements() else {
            panic!("one recovered assertion statement");
        };
        let assertion = statement
            .assertion_view()
            .expect("recognized assertion family");
        assert_eq!(assertion.mode(), None);
        assert!(assertion.conditions().is_empty());
        assert!(assertion.has_recovery());
    });

    with_index_primary("items[{ match subject; marker }]", |primary| {
        let block = primary.value_block_view().expect("typed candidate Block");
        let [statement] = block.statements() else {
            panic!("one recovered Match statement");
        };
        let matched = statement.match_view().expect("recognized Match family");
        assert!(matches!(
            matched.body(),
            AttachedCandidateMatchBody::Missing { .. }
        ));
    });

    with_index_primary("items[{ match subject { value => }; marker }]", |primary| {
        let block = primary.value_block_view().expect("typed candidate Block");
        let [statement] = block.statements() else {
            panic!("one recovered Match statement");
        };
        let matched = statement.match_view().expect("recognized Match family");
        let AttachedCandidateMatchBody::Block { arms, .. } = matched.body() else {
            panic!("authored Match body");
        };
        let [arm] = arms.as_ref() else {
            panic!("one recovered Match arm");
        };
        assert!(matches!(
            arm.body(),
            AttachedCandidateMatchArmBody::Expression(
                crate::attachment::AttachedCandidateStatementExpression::Missing(_)
            )
        ));
    });

    with_index_primary(
        "items[{ unsafe lifetime reason { marker; }; marker }]",
        |primary| {
            let block = primary.value_block_view().expect("typed candidate Block");
            let [statement] = block.statements() else {
                panic!("one recovered unsafe statement");
            };
            let audit = statement
                .unsafe_lifetime_view()
                .expect("recognized unsafe-lifetime family");
            assert!(matches!(
                audit.audit_id(),
                AttachedCandidateUnsafeAuditId::Missing(_)
            ));
            assert!(audit.reason().is_some());
            assert!(matches!(
                audit.body(),
                AttachedCandidateUnsafeBody::Block(_)
            ));
        },
    );
}

#[test]
fn candidate_assertion_condition_limit_is_inclusive_and_charged_once() {
    let limit = SyntaxLimit::AssertionConditions;
    let exact = (0..limit.maximum())
        .map(|ordinal| format!("condition_{ordinal}"))
        .collect::<Vec<_>>()
        .join(", ");
    with_index_primary(
        &format!("items[{{ assert.check({exact}); marker }}]"),
        |primary| {
            let block = primary.value_block_view().expect("typed candidate Block");
            let [statement] = block.statements() else {
                panic!("one exact-boundary assertion statement");
            };
            assert_eq!(
                statement
                    .assertion_view()
                    .expect("typed candidate assertion")
                    .conditions()
                    .len(),
                limit.maximum()
            );
        },
    );

    let one_over = format!("{exact}, one_over");
    let source = format!("predicate leaf() = items[{{ assert.check({one_over}); marker }}]\n");
    assert_eq!(
        parse_shadow_document(&document(&source), ParseOptions::default()).unwrap_err(),
        GrammarBuildError::LimitExceeded(limit)
    );
}

#[test]
fn candidate_assignment_views_keep_both_typed_operands_and_missing_slots() {
    with_index_primary(
        "items[{ marker; target = value; registry <- lease; marker }]",
        |primary| {
            let block = primary.value_block_view().expect("typed candidate Block");
            let [prefix, assignment, lifetime_set] = block.statements() else {
                panic!("prefix plus two source-ordered assignment statements");
            };
            assert_eq!(prefix.kind(), SyntaxKind::ExpressionStatement);
            assert_eq!(assignment.kind(), SyntaxKind::AssignmentStatement);
            assert_eq!(lifetime_set.kind(), SyntaxKind::LifetimeSetStatement);
            for statement in [assignment, lifetime_set] {
                let operands = statement
                    .assignment_view()
                    .expect("typed candidate assignment operands");
                assert!(!operands.target().has_recovery());
                assert!(!operands.value().has_recovery());
            }
        },
    );

    for (source, missing_target, missing_value) in [
        ("items[{ marker; = value; marker }]", true, false),
        ("items[{ marker; target =; marker }]", false, true),
        ("items[{ marker; <- lease; marker }]", true, false),
        ("items[{ marker; registry <-; marker }]", false, true),
    ] {
        with_index_primary(source, |primary| {
            let block = primary.value_block_view().expect("typed candidate Block");
            let [_, statement] = block.statements() else {
                panic!("prefix plus one recovered assignment statement");
            };
            let operands = statement
                .assignment_view()
                .expect("recognized recovered assignment family");
            assert_eq!(operands.target().is_missing(), missing_target);
            assert_eq!(operands.value().is_missing(), missing_value);
        });
    }
}

#[test]
fn candidate_required_operand_views_preserve_wait_recovery_and_freeze_flow_blocks() {
    with_index_primary(
        "items[{ marker; return value; yield @entity.value; wait(target); close resource; select choice.member; marker }]",
        |primary| {
            let block = primary.value_block_view().expect("typed candidate Block");
            let [prefix, returned, yielded, waited, closed, selected] = block.statements() else {
                panic!("prefix plus five required-operand statements");
            };
            assert_eq!(prefix.kind(), SyntaxKind::ExpressionStatement);
            for statement in [returned, yielded, waited, closed, selected] {
                let operand = statement
                    .required_operand_view()
                    .expect("typed required operand");
                assert!(!operand.operand().has_recovery());
                assert!(!operand.has_punctuation_recovery());
            }
        },
    );

    with_index_primary("items[{ marker; wait target; marker }]", |primary| {
        let block = primary.value_block_view().expect("typed candidate Block");
        let [_, waited] = block.statements() else {
            panic!("prefix plus recovered Wait statement");
        };
        let waited = waited
            .required_operand_view()
            .expect("typed recovered Wait");
        assert!(!waited.operand().has_recovery());
        assert!(waited.has_punctuation_recovery());
    });

    for source in [
        "items[{ marker; select { marker; }; marker }]",
        "items[{ marker; select result { marker; }; marker }]",
        "items[{ marker; select scope named { marker; }; marker }]",
    ] {
        with_index_primary(source, |primary| {
            let block = primary.value_block_view().expect("typed candidate Block");
            let [_, selected] = block.statements() else {
                panic!("prefix plus block-shaped Select statement");
            };
            assert_eq!(selected.kind(), SyntaxKind::SelectStatement);
            assert!(selected.required_operand_view().is_none());
        });
    }
}

#[test]
fn candidate_keyword_statement_views_preserve_exact_family_relations() {
    with_index_primary(
        "items[{ marker; out 'exit value; goto target; defer cleanup(); signal ready <- true; break 'outer result; continue 'outer; marker }]",
        |primary| {
            let block = primary.value_block_view().expect("typed candidate Block");
            let [prefix, out, goto, defer, signal, broken, continued] = block.statements() else {
                panic!("prefix plus six keyword statements")
            };
            assert_eq!(prefix.kind(), SyntaxKind::ExpressionStatement);

            let AttachedCandidateKeywordStatement::Out { label, value, .. } =
                out.keyword_statement_view().expect("typed candidate Out")
            else {
                panic!("Out payload")
            };
            assert_eq!(label.unwrap().value().unwrap().as_str(), "exit");
            assert!(!value.has_recovery());

            let AttachedCandidateKeywordStatement::Goto { target, .. } =
                goto.keyword_statement_view().expect("typed candidate Goto")
            else {
                panic!("Goto payload")
            };
            assert!(!target.has_recovery());

            let AttachedCandidateKeywordStatement::Defer { expression, .. } = defer
                .keyword_statement_view()
                .expect("typed candidate Defer")
            else {
                panic!("Defer payload")
            };
            assert!(!expression.has_recovery());

            let AttachedCandidateKeywordStatement::Signal {
                target,
                value,
                arrow_recovery,
                ..
            } = signal
                .keyword_statement_view()
                .expect("typed candidate Signal")
            else {
                panic!("Signal payload")
            };
            assert!(!target.has_recovery());
            assert!(!value.has_recovery());
            assert!(arrow_recovery.is_none());

            let AttachedCandidateKeywordStatement::Break { label, value, .. } = broken
                .keyword_statement_view()
                .expect("typed candidate Break")
            else {
                panic!("Break payload")
            };
            assert_eq!(label.unwrap().value().unwrap().as_str(), "outer");
            assert!(value.is_some_and(|value| !value.has_recovery()));

            let AttachedCandidateKeywordStatement::Continue {
                label,
                forbidden_suffix,
                ..
            } = continued
                .keyword_statement_view()
                .expect("typed candidate Continue")
            else {
                panic!("Continue payload")
            };
            assert_eq!(label.unwrap().value().unwrap().as_str(), "outer");
            assert!(forbidden_suffix.is_none());
        },
    );
}

#[test]
fn candidate_keyword_statement_views_keep_missing_and_punctuation_recovery() {
    with_index_primary(
        "items[{ marker; out; goto; defer; signal; break; continue extra; marker }]",
        |primary| {
            let block = primary.value_block_view().expect("typed candidate Block");
            let [_, out, goto, defer, signal, broken, continued] = block.statements() else {
                panic!("prefix plus six recovered keyword statements")
            };

            let AttachedCandidateKeywordStatement::Out { value, .. } =
                out.keyword_statement_view().expect("recovered Out")
            else {
                panic!("Out payload")
            };
            assert!(value.is_missing());

            let AttachedCandidateKeywordStatement::Goto { target, .. } =
                goto.keyword_statement_view().expect("recovered Goto")
            else {
                panic!("Goto payload")
            };
            assert!(target.is_missing());

            let AttachedCandidateKeywordStatement::Defer { expression, .. } =
                defer.keyword_statement_view().expect("recovered Defer")
            else {
                panic!("Defer payload")
            };
            assert!(expression.is_missing());

            let AttachedCandidateKeywordStatement::Signal {
                target,
                value,
                arrow_recovery,
                ..
            } = signal.keyword_statement_view().expect("recovered Signal")
            else {
                panic!("Signal payload")
            };
            assert!(target.is_missing());
            assert!(value.is_missing());
            assert!(arrow_recovery.is_some());

            let AttachedCandidateKeywordStatement::Break { value, .. } =
                broken.keyword_statement_view().expect("empty Break")
            else {
                panic!("Break payload")
            };
            assert!(value.is_none());

            let AttachedCandidateKeywordStatement::Continue {
                forbidden_suffix, ..
            } = continued
                .keyword_statement_view()
                .expect("recovered Continue")
            else {
                panic!("Continue payload")
            };
            assert!(forbidden_suffix.is_some());
        },
    );
}

#[test]
fn candidate_dot_nominal_receiver_keeps_its_callee_owned_type_root() {
    with_index_primary(
        "items[{ defer Vec<Int>.with_capacity(1); marker }]",
        |primary| {
            let block = primary.value_block_view().expect("typed candidate Block");
            let [defer] = block.statements() else {
                panic!("one Defer statement before the tail")
            };
            let AttachedCandidateKeywordStatement::Defer { expression, .. } = defer
                .keyword_statement_view()
                .expect("typed candidate Defer")
            else {
                panic!("Defer payload")
            };
            let call = expression.node();
            let receiver_expression = call
                .semantic_expression_children()
                .find(|child| {
                    child.component_role() == ExpressionComponentRole::CallAssociatedReceiver
                })
                .expect("one dot receiver value-or-nominal expression");
            let AttachedCandidatePathExpression::NominalType(nominal) = receiver_expression
                .node()
                .path_expression_view()
                .expect("one typed nominal interpretation")
            else {
                panic!("dot-associated receiver must not masquerade as a value path")
            };
            assert!(nominal.projection().path().steps().is_empty());
            assert!(nominal.projection().value().nominal_path().is_some());

            let roots = call.direct_semantic_type_roots().collect::<Vec<_>>();
            let [receiver] = roots.as_slice() else {
                panic!("one dot-nominal receiver type root")
            };
            assert_eq!(receiver.role(), SyntaxCallTypeChildRole::DotNominalReceiver);
            assert_eq!(
                nominal.node().source_span().range(),
                receiver.node().source_span().range()
            );
            assert_eq!(receiver.node().kind(), SyntaxKind::GenericApplicationType);
            assert_eq!(
                receiver
                    .node()
                    .direct_semantic_type_children()
                    .map(|child| child.step())
                    .collect::<Vec<_>>(),
                [TypeRefNodeStep::GenericArgument(0)]
            );
        },
    );

    with_index_primary("items[value]", |primary| {
        let AttachedCandidatePathExpression::Value(path) = primary
            .path_expression_view()
            .expect("one typed value-path interpretation")
        else {
            panic!("ordinary value path must not masquerade as a nominal type")
        };
        assert_eq!(path.segments().count(), 1);
        assert!(!path.has_recovery());
    });
}
