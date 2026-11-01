use super::*;
use arcweft_interaction_model::presentation::PresentationHandleMethodId as Method;

#[test]
fn handle_visibility_keeps_original_body_and_roundtrips_both_banks() {
    let (program, artifact, expected, dialogue) = fixture();
    for bank in [RuntimePresentationBankId::Root, dialogue] {
        let mut banks = PresentationBanks::new(
            program.clone(),
            artifact,
            PresentationLedgerLimits {
                handles: 4,
                scopes: 4,
            },
        );
        let mut env = RuntimeEnv::default();
        let scope = bind(&mut banks, bank, &program, &mut env);
        let token = banks
            .register_and_retire(&program, scope, site(), body(expected, true))
            .unwrap();
        for (method, visible) in [(Method::Hide, false), (Method::Show, true)] {
            banks
                .apply_handle_method(&program, &token, Some(scope), method)
                .unwrap();
            let lease = &banks.ledgers[&bank].state.leases[&token.issuance()];
            assert_eq!(lease.token, token);
            assert_eq!(lease.owner, Some(scope));
            assert_eq!(lease.visibility_override, Some(visible));
            assert_eq!(
                lease.body.as_ref().unwrap().value.value(),
                &RuntimeValue::Bool(true)
            );
            assert!(banks.retirements.is_empty());
            let image = banks.snapshot_admitted().unwrap();
            banks = image.clone().into_admitted_live(&program).unwrap();
            assert_eq!(banks.snapshot_admitted().unwrap(), image);
        }
    }
}

#[test]
fn terminal_handle_methods_retire_exact_original_body_once() {
    let (program, artifact, expected, dialogue) = fixture();
    for bank in [RuntimePresentationBankId::Root, dialogue] {
        for method in [
            Method::Unmount,
            Method::Release,
            Method::Destroy,
            Method::OverlayPop,
        ] {
            let mut banks = PresentationBanks::new(
                program.clone(),
                artifact,
                PresentationLedgerLimits {
                    handles: 4,
                    scopes: 4,
                },
            );
            let mut env = RuntimeEnv::default();
            let scope = bind(&mut banks, bank, &program, &mut env);
            let resource = if method == Method::OverlayPop {
                PresentationResourceKind::Overlay
            } else {
                PresentationResourceKind::Image
            };
            let token = banks
                .register_and_retire(
                    &program,
                    scope,
                    PresentationRegistrationSite::Independent(resource),
                    body(expected, true),
                )
                .unwrap();
            banks
                .apply_handle_method(&program, &token, Some(scope), method)
                .unwrap();
            assert!(banks.ledgers[&bank].state.leases.is_empty());
            assert_eq!(banks.retirements.len(), 1);
            assert_eq!(banks.retirements[0].token, token);
            assert_eq!(
                banks.retirements[0].body.value.value(),
                &RuntimeValue::Bool(true)
            );
            let before = banks.snapshot_admitted().unwrap();
            assert!(
                banks
                    .apply_handle_method(&program, &token, Some(scope), method)
                    .is_err()
            );
            assert_eq!(banks.snapshot_admitted().unwrap(), before);
        }
    }
}

#[test]
fn displaced_handle_cannot_change_or_retire_its_replacement() {
    let (program, artifact, expected, _) = fixture();
    let mut banks = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits {
            handles: 4,
            scopes: 4,
        },
    );
    let mut env = RuntimeEnv::default();
    let scope = bind(
        &mut banks,
        RuntimePresentationBankId::Root,
        &program,
        &mut env,
    );
    let first = banks
        .register_and_retire(&program, scope, site(), body(expected, true))
        .unwrap();
    let second = banks
        .register_and_retire(&program, scope, site(), body(expected, false))
        .unwrap();
    let before = banks.snapshot_admitted().unwrap();
    for method in [Method::Show, Method::Hide, Method::OverlayPop] {
        assert!(
            banks
                .apply_handle_method(&program, &first, Some(scope), method)
                .is_err()
        );
        assert_eq!(banks.snapshot_admitted().unwrap(), before);
    }
    banks
        .apply_handle_method(&program, &first, Some(scope), Method::Release)
        .unwrap();
    assert_eq!(banks.retirements.len(), 1);
    let current = &banks.ledgers[&RuntimePresentationBankId::Root].state.leases[&second.issuance()];
    assert_eq!(current.token, second);
    assert_eq!(current.visibility_override, None);
    assert_eq!(
        current.body.as_ref().unwrap().value.value(),
        &RuntimeValue::Bool(false)
    );
}

#[test]
fn terminal_handle_rejection_preserves_banks_body_queue_and_grants() {
    let (program, artifact, expected, _) = fixture();
    let bank = RuntimePresentationBankId::Root;
    let mut banks = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits {
            handles: 3,
            scopes: 3,
        },
    );
    let mut env = RuntimeEnv::default();
    let scope = bind(&mut banks, bank, &program, &mut env);
    let site = PresentationRegistrationSite::Independent(PresentationResourceKind::Overlay);
    for _ in 0..3 {
        let token = banks
            .register_and_retire(&program, scope, site.clone(), body(expected, false))
            .unwrap();
        banks
            .apply_handle_method(&program, &token, Some(scope), Method::Release)
            .unwrap();
    }
    let token = banks
        .register_and_retire(&program, scope, site, body(expected, true))
        .unwrap();
    let before = banks.snapshot_admitted().unwrap();
    let grant = env.presentation_scope_binding().cloned();
    for method in [
        Method::Unmount,
        Method::Release,
        Method::Destroy,
        Method::OverlayPop,
    ] {
        assert_eq!(
            banks.apply_handle_method(&program, &token, Some(scope), method),
            Err(RuntimePresentationError::RetirementCapacity)
        );
        assert_eq!(banks.snapshot_admitted().unwrap(), before);
        assert_eq!(env.presentation_scope_binding(), grant.as_ref());
    }
}
