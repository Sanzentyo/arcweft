use super::*;
use crate::awbc::schema::AwbcProgram;
use std::sync::Arc;

fn new_program() -> RuntimeProgramOwner {
    RuntimeProgramOwner::Awbc(Arc::new(AwbcProgram::default()))
}

fn execution() -> ExecutionInstanceId {
    ExecutionInstanceId::from_allocated(NonZeroU64::MIN)
}

fn bodies<V>(retired: Vec<PresentationRetirement<V>>) -> Vec<V> {
    retired
        .into_iter()
        .map(PresentationRetirement::into_body)
        .collect()
}

fn scope(fiber: u64, scope: u64) -> PresentationScopeOwner {
    PresentationScopeOwner {
        execution: execution(),
        fiber: RuntimePersistentFiberId::from_allocated(fiber),
        scope: RuntimeScopeInstanceId::from_allocated(NonZeroU64::new(scope).unwrap()),
    }
}

fn new_scope<V>(
    ledger: &mut PresentationLedger<V>,
    program: &RuntimeProgramOwner,
    fiber: u64,
) -> PresentationScopeOwner {
    ledger
        .bind_scope(
            program,
            execution(),
            RuntimePersistentFiberId::from_allocated(fiber),
            crate::value::RuntimeEnv::default()
                .presentation_scope_target()
                .unwrap(),
        )
        .unwrap()
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
        PresentationLedgerLimits {
            handles: capacity,
            scopes: capacity,
        },
    )
}

#[test]
fn replacement_and_clear_move_non_clone_bodies_without_old_handle_clearing_new_slot() {
    #[derive(Debug, PartialEq)]
    struct CapturedBody(Box<i32>);
    let program = new_program();
    let mut ledger = new_ledger(program.clone(), 4);
    let outer = new_scope(&mut ledger, &program, 0);
    let inner = new_scope(&mut ledger, &program, 0);
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
    assert_eq!(bodies(ledger.exit_scope(&program, inner).unwrap()), []);
    assert_eq!(
        ledger.release(&program, &second, Some(inner)),
        Err(RuntimePresentationError::UnknownHandle)
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
    let owner = new_scope(&mut ledger, &program, 0);
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
            RuntimePresentationError::Program,
        ),
        (
            program.clone(),
            foreign,
            Some(owner),
            RuntimePresentationError::UnknownHandle,
        ),
        (
            program.clone(),
            token.clone(),
            Some(scope(1, 1)),
            RuntimePresentationError::Owner,
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
    let foreign_execution = RuntimePresentationHandleToken::from_issued(
        *decoded.artifact(),
        decoded.resource(),
        ExecutionInstanceId::from_allocated(NonZeroU64::new(2).unwrap()),
        decoded.fiber(),
        decoded.issuance(),
    );
    assert_eq!(
        ledger.release(&program, &foreign_execution, Some(owner)),
        Err(RuntimePresentationError::Execution)
    );
    assert_eq!(
        ledger.release(&program, &decoded, Some(owner)).unwrap(),
        Some(10)
    );
    assert_eq!(
        ledger.release(&program, &decoded, Some(owner)),
        Err(RuntimePresentationError::UnknownHandle)
    );
}

#[test]
fn moved_and_detached_leases_survive_only_their_former_scope_cleanup() {
    let program = new_program();
    let mut ledger = new_ledger(program.clone(), 8);
    let original = new_scope(&mut ledger, &program, 0);
    let destination = new_scope(&mut ledger, &program, 1);
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
        Err(RuntimePresentationError::Owner)
    );
    assert_eq!(
        bodies(ledger.exit_scope(&program, original).unwrap()),
        Vec::<i32>::new()
    );
    assert_eq!(
        bodies(ledger.exit_scope(&program, destination).unwrap()),
        [1]
    );
    assert_eq!(ledger.release(&program, &detached, None).unwrap(), Some(2));
}

#[test]
fn capacity_and_cursor_exhaustion_return_body_and_preserve_prior_slot_and_cursor() {
    let program = new_program();
    let mut ledger = new_ledger(program.clone(), 1);
    let owner = new_scope(&mut ledger, &program, 0);
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
            cause: RuntimePresentationError::Capacity,
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
    assert!(matches!(error.cause, RuntimePresentationError::Identity(_)));
    assert_eq!(error.body, 3);
    assert_eq!(ledger.next, RuntimeIdCursor::Exhausted);
    assert!(ledger.leases.is_empty());
}

#[test]
fn scope_cleanup_is_issuance_ordered_and_address_families_are_checked() {
    let program = new_program();
    let mut ledger = new_ledger(program.clone(), 3);
    let owner = new_scope(&mut ledger, &program, 0);
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
    assert_eq!(
        bodies(ledger.exit_scope(&program, owner).unwrap()),
        [3, 2, 1]
    );
    assert_eq!(
        ledger.exit_scope(&program, owner),
        Err(RuntimePresentationError::Owner)
    );
    assert_eq!(
        PresentationSlotAddress::try_new(
            PublicId::try_new("asset.invalid").unwrap(),
            PublicId::try_new("slot.background.default").unwrap(),
            PresentationResourceKind::Background,
        ),
        Err(RuntimePresentationError::InvalidAddress)
    );
}

#[test]
fn independent_resources_of_every_family_never_replace_each_other() {
    let program = new_program();
    let mut ledger = new_ledger(program.clone(), 16);
    let owner = new_scope(&mut ledger, &program, 0);
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
        bodies(ledger.exit_scope(&program, owner).unwrap()),
        [10, 20].repeat(8)
    );
}

#[test]
fn last_identity_is_issued_once_and_foreign_registration_does_not_consume_it() {
    let program = new_program();
    let mut ledger = new_ledger(program.clone(), 3);
    ledger.next = RuntimeIdCursor::Next(NonZeroU64::MAX);
    let owner = new_scope(&mut ledger, &program, 0);
    assert_eq!(
        ledger.register(
            &new_program(),
            owner,
            PresentationRegistrationSite::Slot(address("a")),
            12,
        ),
        Err(PresentationRegisterError {
            cause: RuntimePresentationError::Program,
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
        RuntimePresentationError::Identity(_)
    ));
    assert_eq!(rejected.body, 16);
    assert_eq!(
        ledger.release(&program, &token, Some(owner)).unwrap(),
        Some(14)
    );
}

#[test]
fn reached_scope_binding_is_stable_and_exhaustion_never_changes_the_frame_or_grants() {
    let program = new_program();
    let mut ledger = new_ledger::<i32>(program.clone(), 2);
    let fiber = RuntimePersistentFiberId::from_allocated(3);
    let mut env = crate::value::RuntimeEnv::default();
    let owner = ledger
        .bind_scope(
            &program,
            execution(),
            fiber,
            env.presentation_scope_target().unwrap(),
        )
        .unwrap();
    ledger.next_scope = RuntimeIdCursor::Exhausted;
    assert_eq!(
        ledger
            .bind_scope(
                &program,
                execution(),
                fiber,
                env.presentation_scope_target().unwrap()
            )
            .unwrap(),
        owner
    );
    env.push_scope();
    assert!(matches!(
        ledger.bind_scope(
            &program,
            execution(),
            fiber,
            env.presentation_scope_target().unwrap()
        ),
        Err(RuntimePresentationError::Identity(_))
    ));
    assert!(env.presentation_scope_binding().is_none());
    assert_eq!(ledger.scopes.len(), 1);
    assert_eq!(ledger.next_scope, RuntimeIdCursor::Exhausted);
    env.pop_scope();
    assert_eq!(
        ledger
            .bind_scope(
                &program,
                execution(),
                fiber,
                env.presentation_scope_target().unwrap()
            )
            .unwrap(),
        owner
    );
    ledger
        .exit_bound_scope(
            &program,
            execution(),
            fiber,
            env.presentation_scope_target().unwrap(),
        )
        .unwrap();
    assert!(env.presentation_scope_binding().is_none());
    assert!(matches!(
        ledger.bind_scope(
            &program,
            execution(),
            fiber,
            env.presentation_scope_target().unwrap()
        ),
        Err(RuntimePresentationError::Identity(_))
    ));
}

#[test]
fn foreign_context_cannot_adopt_a_reached_scope_grant() {
    let program = new_program();
    let mut ledger = new_ledger::<i32>(program.clone(), 2);
    let fiber = RuntimePersistentFiberId::from_allocated(3);
    let mut env = crate::value::RuntimeEnv::default();
    let owner = ledger
        .bind_scope(
            &program,
            execution(),
            fiber,
            env.presentation_scope_target().unwrap(),
        )
        .unwrap();
    assert_eq!(
        ledger.bind_scope(
            &new_program(),
            execution(),
            fiber,
            env.presentation_scope_target().unwrap()
        ),
        Err(RuntimePresentationError::Program)
    );
    assert_eq!(
        ledger.bind_scope(
            &program,
            execution(),
            RuntimePersistentFiberId::from_allocated(4),
            env.presentation_scope_target().unwrap()
        ),
        Err(RuntimePresentationError::Owner)
    );
    let execution = ExecutionInstanceId::from_allocated(NonZeroU64::new(2).unwrap());
    assert_eq!(
        ledger.bind_scope(
            &program,
            execution,
            fiber,
            env.presentation_scope_target().unwrap()
        ),
        Err(RuntimePresentationError::Owner)
    );
    assert_eq!(env.presentation_scope_binding().unwrap().owner, owner);
    assert_eq!(ledger.scopes.len(), 1);
}

#[test]
fn scope_grant_limit_is_checked_before_issuing_or_binding_the_reached_frame() {
    let program = new_program();
    let mut ledger = new_ledger::<i32>(program.clone(), 2);
    ledger.limits.scopes = 1;
    let fiber = RuntimePersistentFiberId::from_allocated(3);
    let mut env = crate::value::RuntimeEnv::default();
    let owner = ledger
        .bind_scope(
            &program,
            execution(),
            fiber,
            env.presentation_scope_target().unwrap(),
        )
        .unwrap();
    let before = ledger.next_scope;
    env.push_scope();
    assert_eq!(
        ledger.bind_scope(
            &program,
            execution(),
            fiber,
            env.presentation_scope_target().unwrap()
        ),
        Err(RuntimePresentationError::ScopeCapacity)
    );
    assert!(env.presentation_scope_binding().is_none());
    assert_eq!(ledger.scopes.len(), 1);
    assert_eq!(ledger.next_scope, before);
    env.pop_scope();
    assert_eq!(
        ledger
            .bind_scope(
                &program,
                execution(),
                fiber,
                env.presentation_scope_target().unwrap()
            )
            .unwrap(),
        owner
    );
}

#[test]
fn one_program_slot_replaces_across_real_execution_owners() {
    let program = new_program();
    let mut ledger = new_ledger(program.clone(), 4);
    let mut first_env = crate::value::RuntimeEnv::default();
    let mut second_env = crate::value::RuntimeEnv::default();
    let first = ledger
        .bind_scope(
            &program,
            execution(),
            RuntimePersistentFiberId::from_allocated(1),
            first_env.presentation_scope_target().unwrap(),
        )
        .unwrap();
    let second_execution = ExecutionInstanceId::from_allocated(NonZeroU64::new(2).unwrap());
    let second = ledger
        .bind_scope(
            &program,
            second_execution,
            RuntimePersistentFiberId::from_allocated(2),
            second_env.presentation_scope_target().unwrap(),
        )
        .unwrap();
    let (old, _) = ledger
        .register(
            &program,
            first,
            PresentationRegistrationSite::Slot(address("shared")),
            10,
        )
        .unwrap();
    let (new, previous) = ledger
        .register(
            &program,
            second,
            PresentationRegistrationSite::Slot(address("shared")),
            20,
        )
        .unwrap();
    assert_eq!(previous, Some(10));
    assert_eq!(old.execution(), execution());
    assert_eq!(new.execution(), second_execution);
    assert_eq!(ledger.release(&program, &old, Some(first)).unwrap(), None);
    assert_eq!(
        ledger.release(&program, &new, Some(second)).unwrap(),
        Some(20)
    );
}
