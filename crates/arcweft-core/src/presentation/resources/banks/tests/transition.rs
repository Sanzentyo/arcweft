use super::*;

fn register(
    banks: &mut PresentationBanks<PresentationBody<RuntimePayload>>,
    program: &RuntimeProgramOwner,
    bank: RuntimePresentationBankId,
    env: &mut RuntimeEnv,
    expected: RuntimeSemanticTypeId,
    value: bool,
) -> RuntimePresentationHandleToken {
    let owner = bind(banks, bank, program, env);
    banks
        .ledger_mut(bank)
        .unwrap()
        .register(
            program,
            owner,
            PresentationRegistrationSite::Independent(PresentationResourceKind::Image),
            body(expected, value),
        )
        .unwrap()
        .0
}

fn exit(
    banks: &mut PresentationBanks<PresentationBody<RuntimePayload>>,
    program: &RuntimeProgramOwner,
    source: &mut RuntimeEnv,
    destination: Option<&mut RuntimeEnv>,
    escaping: &[RuntimePresentationHandleToken],
) -> Result<(), RuntimePresentationError> {
    banks.transition_scope_contour(
        program,
        vec![PresentationScopeExit {
            execution: execution(),
            fiber: fiber(),
            scope: source.presentation_scope_target().unwrap(),
        }],
        destination.map(|env| PresentationScopeExit {
            execution: execution(),
            fiber: fiber(),
            scope: env.presentation_scope_target().unwrap(),
        }),
        escaping,
        &[],
    )
}

#[test]
fn escaping_values_keep_original_banks_and_join_the_actual_receiving_scope() {
    let (program, artifact, expected, dialogue) = fixture();
    let root = RuntimePresentationBankId::Root;
    let mut banks = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits {
            handles: 8,
            scopes: 2,
        },
    );
    let mut source = RuntimeEnv::default();
    let mut receiver = RuntimeEnv::default();
    let sentinel = register(&mut banks, &program, root, &mut receiver, expected, false);
    let mut escaping = Vec::new();
    let mut discarded = Vec::new();
    for bank in [root, dialogue] {
        escaping.push(register(
            &mut banks,
            &program,
            bank,
            &mut source,
            expected,
            true,
        ));
        discarded.push(register(
            &mut banks,
            &program,
            bank,
            &mut source,
            expected,
            false,
        ));
    }
    let receiver_root = receiver.presentation_scope_binding().unwrap().grants[&root].owner;
    exit(
        &mut banks,
        &program,
        &mut source,
        Some(&mut receiver),
        &escaping,
    )
    .unwrap();
    assert!(source.presentation_scope_binding().is_none());
    let binding = receiver.presentation_scope_binding().unwrap();
    assert_eq!(binding.banks().collect::<Vec<_>>(), [root, dialogue]);
    assert_eq!(binding.grants[&root].owner, receiver_root);
    assert_eq!(
        banks
            .retirements()
            .iter()
            .map(|item| item.token.clone())
            .collect::<Vec<_>>(),
        discarded
    );
    for token in &escaping {
        let owner = binding.grants[&token.bank()].owner;
        let ledger = &banks.ledgers[&token.bank()];
        assert_eq!(
            ledger
                .validate_handle(&program, token, Some(owner))
                .unwrap(),
            token.issuance()
        );
        assert_eq!(
            ledger.state.leases[&token.issuance()]
                .body
                .as_ref()
                .unwrap()
                .value
                .value(),
            &RuntimeValue::Bool(true)
        );
    }
    let mut admission = RuntimeRetainedValueAdmission::for_program(
        program.clone(),
        RuntimeSchemaLimits::engine_default(),
    );
    banks
        .admit_retained(&program, artifact, &mut admission)
        .unwrap();
    receiver.admit_retained(&mut admission).unwrap();
    admission.finish_presentation_custody().unwrap();
    banks
        .retire_bound_scope(
            &program,
            execution(),
            fiber(),
            receiver.presentation_scope_target().unwrap(),
        )
        .unwrap();
    assert_eq!(
        banks.retirements()[2..]
            .iter()
            .map(|item| item.token.clone())
            .collect::<Vec<_>>(),
        [sentinel, escaping[0].clone(), escaping[1].clone()]
    );
}

#[test]
fn escape_is_excluded_from_retirement_capacity_and_full_scope_grant_is_replaced_atomically() {
    let (program, artifact, expected, _) = fixture();
    let root = RuntimePresentationBankId::Root;
    let mut banks = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits {
            handles: 2,
            scopes: 1,
        },
    );
    let mut prior = RuntimeEnv::default();
    register(&mut banks, &program, root, &mut prior, expected, false);
    banks
        .retire_bound_scope(
            &program,
            execution(),
            fiber(),
            prior.presentation_scope_target().unwrap(),
        )
        .unwrap();
    let mut source = RuntimeEnv::default();
    let mut receiver = RuntimeEnv::default();
    let escaped = register(&mut banks, &program, root, &mut source, expected, true);
    let discarded = register(&mut banks, &program, root, &mut source, expected, false);
    let original = source.presentation_scope_binding().unwrap().grants[&root].owner;
    exit(
        &mut banks,
        &program,
        &mut source,
        Some(&mut receiver),
        std::slice::from_ref(&escaped),
    )
    .unwrap();
    let replacement = receiver.presentation_scope_binding().unwrap().grants[&root].owner;
    assert!(replacement.scope > original.scope);
    assert_eq!(banks.ledgers[&root].state.scopes.len(), 1);
    assert_eq!(banks.retirements().len(), 2);
    assert_eq!(banks.retirements()[1].token, discarded);
    assert_eq!(
        banks.ledgers[&root].state.leases[&escaped.issuance()].owner,
        Some(replacement)
    );
}

#[test]
fn complete_capacity_refusal_preserves_both_contours_and_retries_under_the_same_policy() {
    let (program, artifact, expected, dialogue) = fixture();
    let root = RuntimePresentationBankId::Root;
    let mut banks = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits {
            handles: 3,
            scopes: 4,
        },
    );
    let mut prior = RuntimeEnv::default();
    for _ in 0..2 {
        register(&mut banks, &program, root, &mut prior, expected, false);
    }
    banks
        .retire_bound_scope(
            &program,
            execution(),
            fiber(),
            prior.presentation_scope_target().unwrap(),
        )
        .unwrap();
    let mut source = RuntimeEnv::default();
    let mut receiver = RuntimeEnv::default();
    let escaped = register(&mut banks, &program, root, &mut source, expected, true);
    let discarded = [
        register(&mut banks, &program, root, &mut source, expected, false),
        register(&mut banks, &program, dialogue, &mut source, expected, false),
    ];
    let before = banks.snapshot_admitted().unwrap();
    let scope = source
        .presentation_scope_binding()
        .unwrap()
        .snapshot_admitted(&program)
        .unwrap();
    for _ in 0..2 {
        assert_eq!(
            exit(
                &mut banks,
                &program,
                &mut source,
                Some(&mut receiver),
                std::slice::from_ref(&escaped)
            ),
            Err(RuntimePresentationError::RetirementCapacity)
        );
        assert_eq!(banks.snapshot_admitted().unwrap(), before);
        assert_eq!(
            source
                .presentation_scope_binding()
                .unwrap()
                .snapshot_admitted(&program)
                .unwrap(),
            scope
        );
        assert!(receiver.presentation_scope_binding().is_none());
    }
    banks.retirements_mut().clear();
    exit(
        &mut banks,
        &program,
        &mut source,
        Some(&mut receiver),
        std::slice::from_ref(&escaped),
    )
    .unwrap();
    assert_eq!(
        banks
            .retirements()
            .iter()
            .map(|item| item.token.clone())
            .collect::<Vec<_>>(),
        discarded
    );
    assert!(source.presentation_scope_binding().is_none());
}

#[test]
fn later_invalid_token_and_destination_cursor_exhaustion_leave_first_escape_owned() {
    let (program, artifact, expected, _) = fixture();
    let root = RuntimePresentationBankId::Root;
    let mut banks = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits {
            handles: 4,
            scopes: 1,
        },
    );
    let mut source = RuntimeEnv::default();
    let mut receiver = RuntimeEnv::default();
    let escaped = register(&mut banks, &program, root, &mut source, expected, true);
    let invalid = RuntimePresentationHandleToken::from_issued(
        artifact,
        PresentationResourceKind::Image,
        execution(),
        fiber(),
        NonZeroU64::new(99).unwrap(),
        root,
    );
    let before = banks.snapshot_admitted().unwrap();
    for values in [
        vec![escaped.clone(), invalid],
        vec![escaped.clone(), escaped.clone()],
    ] {
        assert!(
            exit(
                &mut banks,
                &program,
                &mut source,
                Some(&mut receiver),
                &values
            )
            .is_err()
        );
        assert_eq!(banks.snapshot_admitted().unwrap(), before);
        assert!(receiver.presentation_scope_binding().is_none());
    }
    banks.ledgers.get_mut(&root).unwrap().state.next_scope = RuntimeIdCursor::Exhausted;
    let before = banks.snapshot_admitted().unwrap();
    assert!(matches!(
        exit(
            &mut banks,
            &program,
            &mut source,
            Some(&mut receiver),
            std::slice::from_ref(&escaped)
        ),
        Err(RuntimePresentationError::Identity(_))
    ));
    assert_eq!(banks.snapshot_admitted().unwrap(), before);
    assert!(source.presentation_scope_binding().is_some());
    assert!(receiver.presentation_scope_binding().is_none());
}

#[test]
fn terminal_escape_keeps_exact_lease_and_requires_an_actual_retained_value_owner() {
    let (program, artifact, expected, _) = fixture();
    let root = RuntimePresentationBankId::Root;
    let mut banks = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits {
            handles: 2,
            scopes: 2,
        },
    );
    let mut source = RuntimeEnv::default();
    let escaped = register(&mut banks, &program, root, &mut source, expected, true);
    let discarded = register(&mut banks, &program, root, &mut source, expected, false);
    exit(
        &mut banks,
        &program,
        &mut source,
        None,
        std::slice::from_ref(&escaped),
    )
    .unwrap();
    assert!(source.presentation_scope_binding().is_none());
    assert_eq!(banks.retirements()[0].token, discarded);
    assert_eq!(
        banks.ledgers[&root].state.leases[&escaped.issuance()].owner,
        None
    );
    let mut admission = RuntimeRetainedValueAdmission::for_program(
        program.clone(),
        RuntimeSchemaLimits::engine_default(),
    );
    banks
        .admit_retained(&program, artifact, &mut admission)
        .unwrap();
    assert!(admission.finish_presentation_custody().is_err());
    assert_eq!(
        banks
            .ledgers
            .get_mut(&root)
            .unwrap()
            .release(&program, &escaped, None)
            .unwrap()
            .unwrap()
            .value
            .into_value(),
        RuntimeValue::Bool(true)
    );
}

#[test]
fn receipt_from_an_independent_scope_moves_to_the_actual_receiver_before_issuer_exit() {
    let (program, artifact, expected, dialogue) = fixture();
    let mut banks = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits {
            handles: 4,
            scopes: 4,
        },
    );
    let mut issuer = RuntimeEnv::default();
    let mut source = RuntimeEnv::default();
    let mut receiver = RuntimeEnv::default();
    let escaped = register(&mut banks, &program, dialogue, &mut issuer, expected, true);
    let discarded = register(&mut banks, &program, dialogue, &mut issuer, expected, false);
    bind(
        &mut banks,
        RuntimePresentationBankId::Root,
        &program,
        &mut source,
    );
    exit(
        &mut banks,
        &program,
        &mut source,
        Some(&mut receiver),
        std::slice::from_ref(&escaped),
    )
    .unwrap();
    let receiving_owner = receiver.presentation_scope_binding().unwrap().grants[&dialogue].owner;
    assert_eq!(
        banks.ledgers[&dialogue].state.leases[&escaped.issuance()].owner,
        Some(receiving_owner)
    );
    banks
        .retire_bound_scope(
            &program,
            execution(),
            fiber(),
            issuer.presentation_scope_target().unwrap(),
        )
        .unwrap();
    assert_eq!(
        banks
            .retirements()
            .iter()
            .map(|item| item.token.clone())
            .collect::<Vec<_>>(),
        [discarded]
    );
    assert!(
        banks.ledgers[&dialogue]
            .validate_handle(&program, &escaped, Some(receiving_owner))
            .is_ok()
    );
    banks
        .retire_bound_scope(
            &program,
            execution(),
            fiber(),
            receiver.presentation_scope_target().unwrap(),
        )
        .unwrap();
    assert_eq!(banks.retirements()[1].token, escaped);
}

#[test]
fn terminal_receipt_detaches_from_an_independent_issuer_before_that_scope_ends() {
    let (program, artifact, expected, dialogue) = fixture();
    let mut banks = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits {
            handles: 4,
            scopes: 4,
        },
    );
    let mut issuer = RuntimeEnv::default();
    let mut source = RuntimeEnv::default();
    let escaped = register(&mut banks, &program, dialogue, &mut issuer, expected, true);
    bind(
        &mut banks,
        RuntimePresentationBankId::Root,
        &program,
        &mut source,
    );
    exit(
        &mut banks,
        &program,
        &mut source,
        None,
        std::slice::from_ref(&escaped),
    )
    .unwrap();
    banks
        .retire_bound_scope(
            &program,
            execution(),
            fiber(),
            issuer.presentation_scope_target().unwrap(),
        )
        .unwrap();
    assert!(banks.retirements().is_empty());
    assert!(
        banks.ledgers[&dialogue]
            .validate_handle(&program, &escaped, None)
            .is_ok()
    );
    assert!(
        banks
            .ledgers
            .get_mut(&dialogue)
            .unwrap()
            .release(&program, &escaped, None)
            .unwrap()
            .is_some()
    );
}

#[test]
fn one_receipt_keeps_its_original_bank_and_discards_both_scoped_and_external_values_once() {
    let (program, artifact, expected, dialogue) = fixture();
    let root = RuntimePresentationBankId::Root;
    let mut banks = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits {
            handles: 4,
            scopes: 4,
        },
    );
    let mut source = RuntimeEnv::default();
    let mut issuer = RuntimeEnv::default();
    let mut receiver = RuntimeEnv::default();
    let scoped = register(&mut banks, &program, root, &mut source, expected, false);
    let kept = register(&mut banks, &program, dialogue, &mut issuer, expected, true);
    let dropped = register(&mut banks, &program, dialogue, &mut issuer, expected, false);
    let sentinel = register(&mut banks, &program, root, &mut receiver, expected, true);
    banks
        .transition_scope_contour(
            &program,
            vec![PresentationScopeExit {
                execution: execution(),
                fiber: fiber(),
                scope: source.presentation_scope_target().unwrap(),
            }],
            Some(PresentationScopeExit {
                execution: execution(),
                fiber: fiber(),
                scope: receiver.presentation_scope_target().unwrap(),
            }),
            std::slice::from_ref(&kept),
            &[dropped.clone(), scoped.clone()],
        )
        .unwrap();
    assert!(source.presentation_scope_binding().is_none());
    assert_eq!(
        banks
            .retirements()
            .iter()
            .map(|item| item.token.clone())
            .collect::<Vec<_>>(),
        [scoped, dropped]
    );
    let owner = receiver.presentation_scope_binding().unwrap().grants[&dialogue].owner;
    assert!(
        banks.ledgers[&dialogue]
            .validate_handle(&program, &kept, Some(owner))
            .is_ok()
    );
    banks
        .retire_bound_scope(
            &program,
            execution(),
            fiber(),
            issuer.presentation_scope_target().unwrap(),
        )
        .unwrap();
    assert_eq!(banks.retirements().len(), 2);
    banks
        .retire_bound_scope(
            &program,
            execution(),
            fiber(),
            receiver.presentation_scope_target().unwrap(),
        )
        .unwrap();
    assert_eq!(
        banks.retirements()[2..]
            .iter()
            .map(|item| item.token.clone())
            .collect::<Vec<_>>(),
        [sentinel, kept]
    );
}

#[test]
fn receipt_capacity_includes_unwind_and_external_discard_before_any_input_moves() {
    let (program, artifact, expected, dialogue) = fixture();
    let root = RuntimePresentationBankId::Root;
    let mut banks = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits {
            handles: 3,
            scopes: 4,
        },
    );
    let mut prior = RuntimeEnv::default();
    register(&mut banks, &program, root, &mut prior, expected, false);
    banks
        .retire_bound_scope(
            &program,
            execution(),
            fiber(),
            prior.presentation_scope_target().unwrap(),
        )
        .unwrap();
    let mut source = RuntimeEnv::default();
    let mut issuer = RuntimeEnv::default();
    let mut receiver = RuntimeEnv::default();
    let scoped = register(&mut banks, &program, root, &mut source, expected, false);
    let other = register(&mut banks, &program, root, &mut source, expected, false);
    let kept = register(&mut banks, &program, dialogue, &mut issuer, expected, true);
    let dropped = register(&mut banks, &program, dialogue, &mut issuer, expected, false);
    let before = banks.snapshot_admitted().unwrap();
    let original_scope = source
        .presentation_scope_binding()
        .unwrap()
        .snapshot_admitted(&program)
        .unwrap();
    for _ in 0..2 {
        assert_eq!(
            banks.transition_scope_contour(
                &program,
                vec![PresentationScopeExit {
                    execution: execution(),
                    fiber: fiber(),
                    scope: source.presentation_scope_target().unwrap(),
                }],
                Some(PresentationScopeExit {
                    execution: execution(),
                    fiber: fiber(),
                    scope: receiver.presentation_scope_target().unwrap(),
                }),
                std::slice::from_ref(&kept),
                &[dropped.clone(), scoped.clone()]
            ),
            Err(RuntimePresentationError::RetirementCapacity)
        );
        assert_eq!(banks.snapshot_admitted().unwrap(), before);
        assert_eq!(
            source
                .presentation_scope_binding()
                .unwrap()
                .snapshot_admitted(&program)
                .unwrap(),
            original_scope
        );
        assert!(receiver.presentation_scope_binding().is_none());
    }
    banks.retirements_mut().clear();
    banks
        .transition_scope_contour(
            &program,
            vec![PresentationScopeExit {
                execution: execution(),
                fiber: fiber(),
                scope: source.presentation_scope_target().unwrap(),
            }],
            Some(PresentationScopeExit {
                execution: execution(),
                fiber: fiber(),
                scope: receiver.presentation_scope_target().unwrap(),
            }),
            std::slice::from_ref(&kept),
            &[dropped.clone(), scoped.clone()],
        )
        .unwrap();
    assert_eq!(
        banks
            .retirements()
            .iter()
            .map(|item| item.token.clone())
            .collect::<Vec<_>>(),
        [scoped, other, dropped]
    );
    assert!(receiver.presentation_scope_binding().is_some());
}

#[test]
fn invalid_or_overlapping_discard_cannot_follow_an_earlier_escape_move() {
    let (program, artifact, expected, _) = fixture();
    let root = RuntimePresentationBankId::Root;
    let mut banks = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits {
            handles: 4,
            scopes: 4,
        },
    );
    let mut source = RuntimeEnv::default();
    let mut receiver = RuntimeEnv::default();
    let kept = register(&mut banks, &program, root, &mut source, expected, true);
    let dropped = register(&mut banks, &program, root, &mut source, expected, false);
    let invalid = RuntimePresentationHandleToken::from_issued(
        artifact,
        PresentationResourceKind::Image,
        execution(),
        fiber(),
        NonZeroU64::new(99).unwrap(),
        root,
    );
    let before = banks.snapshot_admitted().unwrap();
    for discarded in [
        vec![kept.clone()],
        vec![dropped.clone(), dropped.clone()],
        vec![dropped, invalid],
    ] {
        assert!(
            banks
                .transition_scope_contour(
                    &program,
                    vec![PresentationScopeExit {
                        execution: execution(),
                        fiber: fiber(),
                        scope: source.presentation_scope_target().unwrap(),
                    }],
                    Some(PresentationScopeExit {
                        execution: execution(),
                        fiber: fiber(),
                        scope: receiver.presentation_scope_target().unwrap(),
                    }),
                    std::slice::from_ref(&kept),
                    &discarded
                )
                .is_err()
        );
        assert_eq!(banks.snapshot_admitted().unwrap(), before);
        assert!(source.presentation_scope_binding().is_some());
        assert!(receiver.presentation_scope_binding().is_none());
    }
}

#[test]
fn partial_unwind_moves_only_exiting_owners_without_a_spare_scope_grant() {
    let (program, artifact, expected, _) = fixture();
    let root = RuntimePresentationBankId::Root;
    let mut banks = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits {
            handles: 2,
            scopes: 1,
        },
    );
    let mut source = RuntimeEnv::default();
    let mut unrelated = RuntimeEnv::default();
    let mut receiver = RuntimeEnv::default();
    let kept = register(&mut banks, &program, root, &mut source, expected, true);
    let before = banks.snapshot_admitted().unwrap();
    banks
        .transition_exiting_arguments(
            &program,
            vec![PresentationScopeExit {
                execution: execution(),
                fiber: fiber(),
                scope: unrelated.presentation_scope_target().unwrap(),
            }],
            Some(PresentationScopeExit {
                execution: execution(),
                fiber: fiber(),
                scope: receiver.presentation_scope_target().unwrap(),
            }),
            std::slice::from_ref(&kept),
        )
        .unwrap();
    assert_eq!(banks.snapshot_admitted().unwrap(), before);
    assert!(receiver.presentation_scope_binding().is_none());
    assert!(source.presentation_scope_binding().is_some());
    banks
        .transition_exiting_arguments(
            &program,
            vec![PresentationScopeExit {
                execution: execution(),
                fiber: fiber(),
                scope: source.presentation_scope_target().unwrap(),
            }],
            Some(PresentationScopeExit {
                execution: execution(),
                fiber: fiber(),
                scope: receiver.presentation_scope_target().unwrap(),
            }),
            std::slice::from_ref(&kept),
        )
        .unwrap();
    assert!(source.presentation_scope_binding().is_none());
    assert!(receiver.presentation_scope_binding().is_some());
    assert!(banks.retirements().is_empty());
    exit(&mut banks, &program, &mut receiver, None, &[]).unwrap();
    assert_eq!(
        banks
            .retirements()
            .iter()
            .map(|item| &item.token)
            .collect::<Vec<_>>(),
        [&kept]
    );
}
