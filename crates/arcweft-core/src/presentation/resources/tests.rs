use super::*;
use crate::awbc::schema::AwbcProgram;
use std::sync::Arc;

fn new_program() -> RuntimeProgramOwner {
    RuntimeProgramOwner::Awbc(Arc::new(AwbcProgram::default()))
}

fn scope(fiber: u64, scope: u64) -> PresentationScopeOwner {
    PresentationScopeOwner {
        fiber: RuntimePersistentFiberId::from_allocated(fiber),
        scope: RuntimeScopeInstanceId::from_allocated(NonZeroU64::new(scope).unwrap()),
    }
}

fn address(name: &str) -> PresentationSlotAddress {
    PresentationSlotAddress::try_new(
        PublicId::try_new("target.scene").unwrap(),
        PublicId::try_new(format!("slot.background.{name}")).unwrap(),
        PresentationResourceKind::Background,
    )
    .unwrap()
}

fn new_ledger<V>(program: RuntimeProgramOwner, capacity: usize) -> PresentationLedger<V> {
    PresentationLedger::new(
        program,
        RuntimeArtifactFingerprint::try_from_bytes([7; 32]).unwrap(),
        ExecutionInstanceId::from_allocated(NonZeroU64::MIN),
        capacity,
    )
}

#[test]
fn replacement_and_clear_move_non_clone_bodies_without_old_handle_clearing_new_slot() {
    #[derive(Debug, PartialEq)]
    struct CapturedBody(Box<i32>);
    let program = new_program();
    let mut ledger = new_ledger(program.clone(), 4);
    let outer = scope(0, 1);
    let inner = scope(0, 2);
    let (first, previous) = ledger
        .register(
            &program,
            outer,
            PresentationRegistrationSite::Slot(address("main")),
            CapturedBody(Box::new(11)),
        )
        .unwrap();
    assert!(previous.is_none());
    let (second, previous) = ledger
        .register(
            &program,
            inner,
            PresentationRegistrationSite::Slot(address("main")),
            CapturedBody(Box::new(22)),
        )
        .unwrap();
    assert_eq!(previous, Some(CapturedBody(Box::new(11))));
    assert_eq!(ledger.release(&program, &first, Some(outer)).unwrap(), None);
    assert_eq!(
        ledger.clear(&program, &address("main")).unwrap(),
        Some(CapturedBody(Box::new(22)))
    );
    let (third, previous) = ledger
        .register(
            &program,
            outer,
            PresentationRegistrationSite::Slot(address("main")),
            CapturedBody(Box::new(33)),
        )
        .unwrap();
    assert!(previous.is_none());
    assert_eq!(ledger.exit_scope(&program, inner).unwrap(), []);
    assert_eq!(
        ledger.release(&program, &second, Some(inner)),
        Err(PresentationLedgerError::UnknownHandle)
    );
    assert_eq!(
        ledger.release(&program, &third, Some(outer)).unwrap(),
        Some(CapturedBody(Box::new(33)))
    );
    assert!(ledger.leases.is_empty());
}

#[test]
fn original_lease_exact_token_owner_and_replay_checks_are_failure_atomic() {
    let program = new_program();
    let mut ledger = new_ledger(program.clone(), 3);
    let owner = scope(0, 1);
    let (token, _) = ledger
        .register(
            &program,
            owner,
            PresentationRegistrationSite::Slot(address("a")),
            10,
        )
        .unwrap();
    let foreign = RuntimePresentationHandleToken::from_issued(
        *token.artifact(),
        PresentationResourceKind::Image,
        token.execution(),
        token.fiber(),
        token.issuance(),
    );
    let checks = [
        (
            new_program(),
            token.clone(),
            Some(owner),
            PresentationLedgerError::Program,
        ),
        (
            program.clone(),
            foreign,
            Some(owner),
            PresentationLedgerError::UnknownHandle,
        ),
        (
            program.clone(),
            token.clone(),
            Some(scope(1, 1)),
            PresentationLedgerError::Owner,
        ),
    ];
    for (selected, token, selected_owner, expected) in checks {
        assert_eq!(
            ledger.release(&selected, &token, selected_owner),
            Err(expected)
        );
        assert_eq!(ledger.leases.len(), 1);
    }
    let decoded =
        RuntimePresentationHandleToken::try_decode_payload(&token.encode_payload()).unwrap();
    let mut other_execution = new_ledger::<i32>(program.clone(), 3);
    other_execution.execution = ExecutionInstanceId::from_allocated(NonZeroU64::new(2).unwrap());
    assert_eq!(
        other_execution.release(&program, &decoded, Some(owner)),
        Err(PresentationLedgerError::Execution)
    );
    assert_eq!(
        ledger.release(&program, &decoded, Some(owner)).unwrap(),
        Some(10)
    );
    assert_eq!(
        ledger.release(&program, &decoded, Some(owner)),
        Err(PresentationLedgerError::UnknownHandle)
    );
}

#[test]
fn moved_and_detached_leases_survive_only_their_former_scope_cleanup() {
    let program = new_program();
    let mut ledger = new_ledger(program.clone(), 8);
    let original = scope(0, 7);
    let destination = scope(1, 7);
    let (moved, _) = ledger
        .register(
            &program,
            original,
            PresentationRegistrationSite::Slot(address("moved")),
            1,
        )
        .unwrap();
    let (detached, _) = ledger
        .register(
            &program,
            original,
            PresentationRegistrationSite::Slot(address("detached")),
            2,
        )
        .unwrap();
    ledger
        .transfer(&program, &moved, original, Some(destination))
        .unwrap();
    ledger
        .transfer(&program, &detached, original, None)
        .unwrap();
    assert_eq!(
        ledger.transfer(&program, &moved, original, None),
        Err(PresentationLedgerError::Owner)
    );
    assert_eq!(
        ledger.exit_scope(&program, original).unwrap(),
        Vec::<i32>::new()
    );
    assert_eq!(ledger.exit_scope(&program, destination).unwrap(), [1]);
    assert_eq!(ledger.release(&program, &detached, None).unwrap(), Some(2));
}

#[test]
fn capacity_and_cursor_exhaustion_return_body_and_preserve_prior_slot_and_cursor() {
    let program = new_program();
    let mut ledger = new_ledger(program.clone(), 1);
    let owner = scope(0, 1);
    let (token, _) = ledger
        .register(
            &program,
            owner,
            PresentationRegistrationSite::Slot(address("a")),
            1,
        )
        .unwrap();
    let cursor = ledger.next;
    assert_eq!(
        ledger.register(
            &program,
            owner,
            PresentationRegistrationSite::Slot(address("a")),
            2
        ),
        Err(PresentationRegisterError {
            cause: PresentationLedgerError::Capacity,
            body: 2
        })
    );
    assert_eq!(ledger.next, cursor);
    assert_eq!(
        ledger.release(&program, &token, Some(owner)).unwrap(),
        Some(1)
    );
    ledger.next = RuntimeIdCursor::Exhausted;
    let error = ledger
        .register(
            &program,
            owner,
            PresentationRegistrationSite::Slot(address("a")),
            3,
        )
        .unwrap_err();
    assert!(matches!(error.cause, PresentationLedgerError::Identity(_)));
    assert_eq!(error.body, 3);
    assert_eq!(ledger.next, RuntimeIdCursor::Exhausted);
    assert!(ledger.leases.is_empty());
}

#[test]
fn scope_cleanup_is_issuance_ordered_and_address_families_are_checked() {
    let program = new_program();
    let mut ledger = new_ledger(program.clone(), 3);
    let owner = scope(0, 1);
    for (name, value) in [("z", 3), ("a", 2), ("m", 1)] {
        ledger
            .register(
                &program,
                owner,
                PresentationRegistrationSite::Slot(address(name)),
                value,
            )
            .unwrap();
    }
    assert_eq!(ledger.exit_scope(&program, owner).unwrap(), [3, 2, 1]);
    assert_eq!(
        ledger.exit_scope(&program, owner).unwrap(),
        Vec::<i32>::new()
    );
    assert_eq!(
        PresentationSlotAddress::try_new(
            PublicId::try_new("asset.invalid").unwrap(),
            PublicId::try_new("slot.background.default").unwrap(),
            PresentationResourceKind::Background,
        ),
        Err(PresentationLedgerError::InvalidAddress)
    );
}

#[test]
fn independent_resources_of_every_family_never_replace_each_other() {
    let program = new_program();
    let mut ledger = new_ledger(program.clone(), 16);
    let owner = scope(0, 1);
    for kind in PresentationResourceKind::ALL {
        for value in [10, 20] {
            let (token, previous) = ledger
                .register(
                    &program,
                    owner,
                    PresentationRegistrationSite::Independent(kind),
                    value,
                )
                .unwrap();
            assert_eq!(token.resource(), kind);
            assert!(previous.is_none());
        }
    }
    assert_eq!(
        ledger.exit_scope(&program, owner).unwrap(),
        [10, 20].repeat(8)
    );
}

#[test]
fn last_identity_is_issued_once_and_foreign_registration_does_not_consume_it() {
    let program = new_program();
    let mut ledger = new_ledger(program.clone(), 3);
    ledger.next = RuntimeIdCursor::Next(NonZeroU64::MAX);
    let owner = scope(0, 1);
    assert_eq!(
        ledger.register(
            &new_program(),
            owner,
            PresentationRegistrationSite::Slot(address("a")),
            12,
        ),
        Err(PresentationRegisterError {
            cause: PresentationLedgerError::Program,
            body: 12
        })
    );
    assert_eq!(ledger.next, RuntimeIdCursor::Next(NonZeroU64::MAX));
    let (token, _) = ledger
        .register(
            &program,
            owner,
            PresentationRegistrationSite::Slot(address("a")),
            14,
        )
        .unwrap();
    assert_eq!(token.issuance(), NonZeroU64::MAX);
    let rejected = ledger
        .register(
            &program,
            owner,
            PresentationRegistrationSite::Slot(address("a")),
            16,
        )
        .unwrap_err();
    assert!(matches!(
        rejected.cause,
        PresentationLedgerError::Identity(_)
    ));
    assert_eq!(rejected.body, 16);
    assert_eq!(
        ledger.release(&program, &token, Some(owner)).unwrap(),
        Some(14)
    );
}
