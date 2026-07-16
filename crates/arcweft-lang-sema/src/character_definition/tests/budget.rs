use super::*;

fn assert_exact(kind: CharacterDefinitionWorkKind) {
    let mut budget = CharacterDefinitionRequestBudget::with_maximum_for_test(3);
    for _ in 0..3 {
        budget.charge(kind).expect("inclusive unit");
    }
    assert_eq!(budget.consumed(), 3);
    assert_eq!(budget.sequence.as_slice(), [kind, kind, kind]);
    assert_eq!(budget.terminal, None);
}

fn assert_one_over(kind: CharacterDefinitionWorkKind) {
    let mut budget = CharacterDefinitionRequestBudget::with_maximum_for_test(3);
    for _ in 0..3 {
        budget.charge(kind).expect("inclusive unit");
    }
    let error = budget.charge(kind).expect_err("fourth unit is one-over");
    assert_eq!(
        error,
        CharacterDefinitionResourceError::Limit {
            kind: CharacterDefinitionLimitKind::QueryWork,
            observed: 4,
            maximum: 3,
        }
    );
    assert_eq!(budget.consumed(), 4);
    assert_eq!(budget.sequence.as_slice(), [kind, kind, kind, kind]);
    assert_eq!(budget.charge(kind), Err(error));
    assert_eq!(budget.consumed(), 4);
}

#[test]
fn budget_parser_exact() {
    assert_exact(CharacterDefinitionWorkKind::ParserFact);
}

#[test]
fn budget_parser_one_over() {
    assert_one_over(CharacterDefinitionWorkKind::ParserFact);
}

#[test]
fn budget_project_exact() {
    assert_exact(CharacterDefinitionWorkKind::ProjectSymbolCandidate);
}

#[test]
fn budget_project_one_over() {
    assert_one_over(CharacterDefinitionWorkKind::ProjectSymbolCandidate);
}

#[test]
fn budget_member_exact() {
    assert_exact(CharacterDefinitionWorkKind::TypedMemberCandidate);
}

#[test]
fn budget_member_one_over() {
    assert_one_over(CharacterDefinitionWorkKind::TypedMemberCandidate);
}

#[test]
fn budget_cursor_exact() {
    assert_exact(CharacterDefinitionWorkKind::CursorFact);
}

#[test]
fn budget_cursor_one_over() {
    assert_one_over(CharacterDefinitionWorkKind::CursorFact);
}

#[test]
fn budget_decl_exact() {
    assert_exact(CharacterDefinitionWorkKind::DeclarationCopy);
}

#[test]
fn budget_decl_one_over() {
    assert_one_over(CharacterDefinitionWorkKind::DeclarationCopy);
}

#[test]
fn budget_adapt_exact() {
    assert_exact(CharacterDefinitionWorkKind::SourceAdaptation);
}

#[test]
fn budget_adapt_one_over() {
    assert_one_over(CharacterDefinitionWorkKind::SourceAdaptation);
}

#[test]
fn budget_ident_exact() {
    assert_exact(CharacterDefinitionWorkKind::IdentityCheck);
}

#[test]
fn budget_ident_one_over() {
    assert_one_over(CharacterDefinitionWorkKind::IdentityCheck);
}

#[test]
fn budget_error_exact() {
    assert_exact(CharacterDefinitionWorkKind::AdmittedErrorCandidate);
}

#[test]
fn budget_error_one_over() {
    assert_one_over(CharacterDefinitionWorkKind::AdmittedErrorCandidate);
}

#[test]
fn budget_empty_receipt_is_exact() {
    let budget = CharacterDefinitionRequestBudget::with_maximum_for_test(1);
    let receipt = budget
        .receipt_since(budget.checkpoint())
        .expect("an unchanged nonterminal budget has an empty receipt");
    assert_eq!(receipt.total(), 0);
    assert!(receipt.is_empty());
}

#[test]
fn budget_receipt_preserves_exact_order_and_replay_boundary() {
    let mut source = CharacterDefinitionRequestBudget::with_maximum_for_test(8);
    source
        .charge(CharacterDefinitionWorkKind::ParserFact)
        .expect("prefix");
    let checkpoint = source.checkpoint();
    source
        .charge(CharacterDefinitionWorkKind::TypedMemberCandidate)
        .expect("first receipt unit");
    source
        .charge(CharacterDefinitionWorkKind::IdentityCheck)
        .expect("second receipt unit");
    let receipt = source.receipt_since(checkpoint).expect("valid receipt");
    assert_eq!(receipt.total(), 2);
    assert_eq!(
        receipt.sequence.as_ref(),
        [
            CharacterDefinitionWorkKind::TypedMemberCandidate,
            CharacterDefinitionWorkKind::IdentityCheck,
        ]
    );

    let mut replay = CharacterDefinitionRequestBudget::with_maximum_for_test(1);
    replay
        .charge(CharacterDefinitionWorkKind::ParserFact)
        .expect("consume remaining capacity");
    assert_eq!(
        replay.replay(&receipt),
        Err(CharacterDefinitionResourceError::Limit {
            kind: CharacterDefinitionLimitKind::QueryWork,
            observed: 2,
            maximum: 1,
        })
    );
    assert_eq!(
        replay.sequence,
        [
            CharacterDefinitionWorkKind::ParserFact,
            CharacterDefinitionWorkKind::TypedMemberCandidate,
        ]
    );
}

#[test]
fn budget_terminal_receipt_returns_the_original_error() {
    let mut budget = CharacterDefinitionRequestBudget::with_maximum_for_test(0);
    let checkpoint = budget.checkpoint();
    let terminal = budget
        .charge(CharacterDefinitionWorkKind::CursorFact)
        .expect_err("first unit is one-over");
    assert_eq!(budget.receipt_since(checkpoint), Err(terminal));
}

#[test]
fn budget_checkpoint_rejects_impossible_positions() {
    let mut budget = CharacterDefinitionRequestBudget::with_maximum_for_test(4);
    budget
        .charge(CharacterDefinitionWorkKind::CursorFact)
        .expect("unit");
    let error = CharacterDefinitionRequestBudget::arithmetic_overflow();
    assert_eq!(
        budget.receipt_since(CharacterDefinitionBudgetCheckpoint {
            consumed: 2,
            sequence_len: 0,
        }),
        Err(error.clone())
    );
    assert_eq!(
        budget.receipt_since(CharacterDefinitionBudgetCheckpoint {
            consumed: 1,
            sequence_len: 2,
        }),
        Err(error)
    );
}

#[test]
fn budget_receipt_mismatch_is_terminal() {
    let mut budget = CharacterDefinitionRequestBudget::with_maximum_for_test(4);
    budget
        .charge(CharacterDefinitionWorkKind::CursorFact)
        .expect("unit");
    let receipt = CharacterDefinitionWorkReceipt {
        total: 2,
        sequence: Box::new([CharacterDefinitionWorkKind::IdentityCheck]),
    };
    let error = CharacterDefinitionRequestBudget::arithmetic_overflow();
    assert_eq!(budget.replay(&receipt), Err(error.clone()));
    assert_eq!(budget.consumed(), 1);
    assert_eq!(
        budget.charge(CharacterDefinitionWorkKind::ParserFact),
        Err(error)
    );
}

#[test]
fn budget_addition_overflow_is_terminal_without_saturation() {
    let mut budget = CharacterDefinitionRequestBudget {
        maximum: u64::MAX,
        consumed: u64::MAX,
        sequence: Vec::new(),
        terminal: None,
    };
    let error = CharacterDefinitionRequestBudget::arithmetic_overflow();
    assert_eq!(
        budget.charge(CharacterDefinitionWorkKind::ParserFact),
        Err(error.clone())
    );
    assert!(budget.sequence.is_empty());
    assert_eq!(budget.consumed(), u64::MAX);
    assert_eq!(
        budget.charge(CharacterDefinitionWorkKind::CursorFact),
        Err(error)
    );
}

#[test]
fn budget_sequence_count_conversion_and_addition_are_checked() {
    let error = CharacterDefinitionRequestBudget::arithmetic_overflow();
    assert_eq!(
        CharacterDefinitionRequestBudget::checked_next_sequence_count(u128::from(u64::MAX) + 1,),
        Err(error.clone())
    );
    assert_eq!(
        CharacterDefinitionRequestBudget::checked_next_sequence_count(u128::from(u64::MAX)),
        Err(error)
    );
}

#[test]
fn budget_production_exact_4096() {
    let mut budget = CharacterDefinitionRequestBudget::for_request();
    for _ in 0..CharacterDefinitionLimits::PRODUCTION.query_work() {
        budget
            .charge(CharacterDefinitionWorkKind::ParserFact)
            .expect("the inclusive production boundary succeeds");
    }
    assert_eq!(budget.maximum(), 4_096);
    assert_eq!(budget.consumed(), 4_096);
    assert_eq!(budget.terminal, None);
}

#[test]
fn budget_production_one_over_4097() {
    let mut budget = CharacterDefinitionRequestBudget::for_request();
    for _ in 0..CharacterDefinitionLimits::PRODUCTION.query_work() {
        budget
            .charge(CharacterDefinitionWorkKind::IdentityCheck)
            .expect("the inclusive production boundary succeeds");
    }
    assert_eq!(
        budget.charge(CharacterDefinitionWorkKind::SourceAdaptation),
        Err(CharacterDefinitionResourceError::Limit {
            kind: CharacterDefinitionLimitKind::QueryWork,
            observed: 4_097,
            maximum: 4_096,
        })
    );
    assert_eq!(budget.consumed(), 4_097);
    assert_eq!(
        budget.sequence.last(),
        Some(&CharacterDefinitionWorkKind::SourceAdaptation)
    );
}

#[test]
fn budget_concurrent_requests_are_independent() {
    let first = std::thread::spawn(|| {
        let mut budget = CharacterDefinitionRequestBudget::with_maximum_for_test(2);
        budget
            .charge(CharacterDefinitionWorkKind::ParserFact)
            .expect("first request");
        budget.consumed()
    });
    let second = std::thread::spawn(|| {
        let mut budget = CharacterDefinitionRequestBudget::with_maximum_for_test(3);
        budget
            .charge(CharacterDefinitionWorkKind::IdentityCheck)
            .expect("second request one");
        budget
            .charge(CharacterDefinitionWorkKind::IdentityCheck)
            .expect("second request two");
        budget.consumed()
    });
    assert_eq!(first.join().expect("first request thread"), 1);
    assert_eq!(second.join().expect("second request thread"), 2);
}
