use super::*;

#[test]
fn one_physical_scope_admits_restores_and_retires_both_issuing_banks_once() {
    let (program, artifact, expected, dialogue) = fixture();
    let mut banks = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits::for_schema(RuntimeSchemaLimits::engine_default()),
    );
    let mut env = RuntimeEnv::default();
    let mut tokens = Vec::new();
    for (bank, value) in [(RuntimePresentationBankId::Root, true), (dialogue, false)] {
        let owner = bind(&mut banks, bank, &program, &mut env);
        tokens.push(
            banks
                .ledger_mut(bank)
                .unwrap()
                .register(&program, owner, site(), body(expected, value))
                .unwrap()
                .0,
        );
    }
    let binding = env.presentation_scope_binding().unwrap();
    assert_eq!(
        binding.banks().collect::<Vec<_>>(),
        [RuntimePresentationBankId::Root, dialogue]
    );
    assert_eq!(tokens[0].issuance(), tokens[1].issuance());
    assert_ne!(tokens[0], tokens[1]);
    env.validate_presentation_context(&program, execution(), fiber())
        .unwrap();
    let mut admission = RuntimeRetainedValueAdmission::for_program(
        program.clone(),
        RuntimeSchemaLimits::engine_default(),
    );
    banks
        .admit_retained(&program, artifact, &mut admission)
        .unwrap();
    env.admit_retained(&mut admission).unwrap();
    admission.finish_presentation_custody().unwrap();
    let wire = binding.snapshot_admitted(&program).unwrap();
    let decoded: RuntimePresentationScopeSnapshot =
        serde_json::from_slice(&serde_json::to_vec(&wire).unwrap()).unwrap();
    assert_eq!(decoded, wire);
    let image = banks.snapshot_admitted().unwrap();
    let mut banks = image.clone().into_admitted_live(&program).unwrap();
    assert_eq!(banks.snapshot_admitted().unwrap(), image);
    let mut restored_binding = Some(decoded.into_admitted_live(&program));
    banks
        .retire_bound_scope(
            &program,
            execution(),
            fiber(),
            PresentationScopeTarget::native(&mut restored_binding),
        )
        .unwrap();
    assert!(restored_binding.is_none());
    assert_eq!(
        banks
            .retirements()
            .iter()
            .map(|item| item.token.clone())
            .collect::<Vec<_>>(),
        tokens
    );
    banks
        .retire_bound_scope(
            &program,
            execution(),
            fiber(),
            PresentationScopeTarget::native(&mut restored_binding),
        )
        .unwrap();
    assert_eq!(banks.retirements().len(), 2);
    let mut admission = RuntimeRetainedValueAdmission::for_program(
        program.clone(),
        RuntimeSchemaLimits::engine_default(),
    );
    banks
        .admit_retained(&program, artifact, &mut admission)
        .unwrap();
    admission.finish_presentation_custody().unwrap();
}

#[test]
fn scope_capacity_refusal_preflights_every_bank_and_keeps_both_grants_for_retry() {
    let (program, artifact, expected, dialogue) = fixture();
    let mut banks = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits {
            handles: 2,
            scopes: 4,
        },
    );
    let mut prior = RuntimeEnv::default();
    let owner = bind(
        &mut banks,
        RuntimePresentationBankId::Root,
        &program,
        &mut prior,
    );
    banks
        .ledger_mut(RuntimePresentationBankId::Root)
        .unwrap()
        .register(&program, owner, site(), body(expected, false))
        .unwrap();
    banks
        .retire_bound_scope(
            &program,
            execution(),
            fiber(),
            prior.presentation_scope_target().unwrap(),
        )
        .unwrap();
    let mut env = RuntimeEnv::default();
    let mut tokens = Vec::new();
    for (bank, value) in [(RuntimePresentationBankId::Root, true), (dialogue, false)] {
        let owner = bind(&mut banks, bank, &program, &mut env);
        tokens.push(
            banks
                .ledger_mut(bank)
                .unwrap()
                .register(&program, owner, site(), body(expected, value))
                .unwrap()
                .0,
        );
    }
    let before = banks.snapshot_admitted().unwrap();
    let grant = env
        .presentation_scope_binding()
        .unwrap()
        .snapshot_admitted(&program)
        .unwrap();
    for _ in 0..2 {
        assert_eq!(
            banks.retire_bound_scope(
                &program,
                execution(),
                fiber(),
                env.presentation_scope_target().unwrap()
            ),
            Err(RuntimePresentationError::RetirementCapacity)
        );
        assert_eq!(banks.snapshot_admitted().unwrap(), before);
        assert_eq!(
            env.presentation_scope_binding()
                .unwrap()
                .snapshot_admitted(&program)
                .unwrap(),
            grant
        );
    }
    banks.retirements_mut().clear();
    banks
        .retire_bound_scope(
            &program,
            execution(),
            fiber(),
            env.presentation_scope_target().unwrap(),
        )
        .unwrap();
    assert!(env.presentation_scope_binding().is_none());
    assert_eq!(
        banks
            .retirements()
            .iter()
            .map(|item| item.token.clone())
            .collect::<Vec<_>>(),
        tokens
    );
}

#[test]
fn scope_grant_wire_rejects_duplicate_bank_before_its_second_grant_and_rejects_empty_admission() {
    let (program, artifact, _, dialogue) = fixture();
    let mut banks: PresentationBanks<PresentationBody<RuntimePayload>> = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits::for_schema(RuntimeSchemaLimits::engine_default()),
    );
    let mut env = RuntimeEnv::default();
    for bank in [RuntimePresentationBankId::Root, dialogue] {
        bind(&mut banks, bank, &program, &mut env);
    }
    let wire = env
        .presentation_scope_binding()
        .unwrap()
        .snapshot_admitted(&program)
        .unwrap();
    let mut value = serde_json::to_value(&wire).unwrap();
    let rows = value["grants"].as_array_mut().unwrap();
    let bank = rows[0][0].clone();
    rows.push(serde_json::json!([bank, {"unread_invalid_grant": true}]));
    let error = serde_json::from_value::<RuntimePresentationScopeSnapshot>(value).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("duplicate presentation bank identity")
    );
    let empty: RuntimePresentationScopeSnapshot =
        serde_json::from_value(serde_json::json!({"grants": []})).unwrap();
    let mut admission = RuntimeRetainedValueAdmission::for_program(
        program.clone(),
        RuntimeSchemaLimits::engine_default(),
    );
    banks
        .admit_retained(&program, artifact, &mut admission)
        .unwrap();
    let error = empty.admit_retained(&mut admission).unwrap_err();
    assert_eq!(admission.finish_presentation_custody().unwrap_err(), error);
}
