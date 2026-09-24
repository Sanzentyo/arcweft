use super::{ProgramGeneration, SwapCompatibility, SwapError, SwapPhase, SwapSession};
use arcweft_bundle::container::BundleDigest;
use arcweft_core::task::GenerationId;
use std::sync::Arc;

fn generation(id: u64) -> Arc<ProgramGeneration> {
    Arc::new(ProgramGeneration::empty(
        GenerationId::new(id),
        BundleDigest::of(&id.to_le_bytes()),
        BundleDigest::ZERO,
    ))
}

fn commit(session: &mut SwapSession, next: u64) {
    assert_eq!(
        session.prepare(generation(next)).unwrap(),
        SwapCompatibility::ContentOnly
    );
    session.begin_quiescence().unwrap();
    session.commit().unwrap();
}

#[test]
fn retired_generation_pins_survive_subsequent_committed_generations() {
    let mut session = SwapSession::new(generation(0));
    let old = session.pin_active_generation();
    commit(&mut session, 1);
    session.retire_unused();
    assert_eq!(session.phase(), SwapPhase::Retiring);
    commit(&mut session, 2);
    assert_eq!(session.active_generation_id(), GenerationId::new(2));
    assert_eq!(
        session
            .retired()
            .iter()
            .map(|owner| owner.id)
            .collect::<Vec<_>>(),
        [GenerationId::new(0), GenerationId::new(1)]
    );
    session.retire_unused();
    assert_eq!(session.phase(), SwapPhase::Retiring);
    assert_eq!(session.retired().len(), 1);
    assert!(Arc::ptr_eq(&session.retired()[0], &old));
    drop(old);
    session.retire_unused();
    assert_eq!(session.phase(), SwapPhase::Idle);
    assert!(session.retired().is_empty());
}

#[test]
fn retiring_cannot_prepare_during_a_runtime_step() {
    let mut session = SwapSession::new(generation(0));
    let _old = session.pin_active_generation();
    commit(&mut session, 1);
    session.retire_unused();
    session.enter_runtime_step();
    assert_eq!(
        session.prepare(generation(2)),
        Err(SwapError::RuntimeNotQuiescent)
    );
    assert_eq!(session.phase(), SwapPhase::Retiring);
    assert!(session.prepared.is_none());
    assert_eq!(session.retired().len(), 1);
    assert_eq!(session.active_generation_id(), GenerationId::new(1));
    session.finish_runtime_step();
    assert_eq!(
        session.prepare(generation(2)),
        Ok(SwapCompatibility::ContentOnly)
    );
}

#[test]
fn retirement_collection_preserves_a_prepared_transaction() {
    let mut session = SwapSession::new(generation(0));
    let old = session.pin_active_generation();
    commit(&mut session, 1);
    session.retire_unused();
    session.prepare(generation(2)).unwrap();
    drop(old);
    session.retire_unused();
    assert!(session.retired().is_empty());
    assert_eq!(session.phase(), SwapPhase::Prepared);
    session.begin_quiescence().unwrap();
    session.retire_unused();
    assert_eq!(session.phase(), SwapPhase::Quiescing);
    session.commit().unwrap();
    assert_eq!(session.active_generation_id(), GenerationId::new(2));
}

#[test]
fn pending_and_unretired_transaction_phases_still_reject_another_prepare() {
    let mut session = SwapSession::new(generation(0));
    session.prepare(generation(1)).unwrap();
    assert_eq!(
        session.prepare(generation(2)),
        Err(SwapError::WrongPhase {
            expected: SwapPhase::Idle,
            actual: SwapPhase::Prepared
        })
    );
    session.begin_quiescence().unwrap();
    assert_eq!(
        session.prepare(generation(2)),
        Err(SwapError::WrongPhase {
            expected: SwapPhase::Idle,
            actual: SwapPhase::Quiescing
        })
    );
    session.commit().unwrap();
    assert_eq!(
        session.prepare(generation(2)),
        Err(SwapError::WrongPhase {
            expected: SwapPhase::Idle,
            actual: SwapPhase::Committed
        })
    );
}
