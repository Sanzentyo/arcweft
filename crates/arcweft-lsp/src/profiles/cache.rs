//! Atomically published, generation-keyed semantic profile state.

use std::sync::{
    Arc, Mutex, PoisonError, RwLock,
    atomic::{AtomicU8, AtomicU64, Ordering},
};

use arcweft_lang_sema::registration::RegisteredSemanticWorld;
use thiserror::Error;

const ADMISSION_ACTIVE: u8 = 0;
const ADMISSION_CLOSING: u8 = 1;
const ADMISSION_CLOSED: u8 = 2;

/// Monotonic identity of one fully accepted LSP semantic environment.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceptedEnvironmentGeneration(u64);

impl AcceptedEnvironmentGeneration {
    /// Returns the checked generation number.
    pub const fn get(self) -> u64 {
        self.0
    }

    #[cfg(test)]
    pub(crate) const fn for_test(value: u64) -> Self {
        Self(value)
    }
}

/// Failure to publish a complete candidate environment.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum AcceptedEnvironmentReplaceError {
    #[error("profile environment is shutting down")]
    ShuttingDown,
    #[error("accepted environment generation overflowed")]
    GenerationOverflow,
}

/// Rebuild-admission lifecycle for one profile state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProfileEnvironmentLifecycle {
    Active,
    Closing,
    Closed,
}

/// Broad semantic caches owned exclusively by one accepted generation.
#[derive(Debug, Default)]
struct ProfileSemanticCaches {
    entries: Mutex<Vec<(String, String)>>,
    hits: AtomicU64,
}

impl ProfileSemanticCaches {
    fn clear(&self) {
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        self.hits.store(0, Ordering::Release);
    }
}

/// One immutable registered world plus its fresh generation-owned cache namespace.
#[derive(Debug)]
pub struct AcceptedProfileEnvironment {
    generation: AcceptedEnvironmentGeneration,
    world: Arc<RegisteredSemanticWorld>,
    caches: ProfileSemanticCaches,
}

impl AcceptedProfileEnvironment {
    pub const fn generation(&self) -> AcceptedEnvironmentGeneration {
        self.generation
    }

    pub const fn world(&self) -> &Arc<RegisteredSemanticWorld> {
        &self.world
    }

    #[cfg(test)]
    pub(crate) fn insert_cache_for_test(&self, key: &str, value: &str) {
        self.caches
            .entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((key.to_owned(), value.to_owned()));
        self.caches.hits.fetch_add(1, Ordering::AcqRel);
    }

    #[cfg(test)]
    pub(crate) fn cache_snapshot_for_test(&self) -> (Vec<(String, String)>, u64) {
        (
            self.caches
                .entries
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone(),
            self.caches.hits.load(Ordering::Acquire),
        )
    }
}

/// Single-writer publication boundary for accepted LSP semantic environments.
#[derive(Debug)]
pub struct LspProfileState {
    admission: AtomicU8,
    accepted: RwLock<Option<Arc<AcceptedProfileEnvironment>>>,
}

impl LspProfileState {
    pub const fn new() -> Self {
        Self {
            admission: AtomicU8::new(ADMISSION_ACTIVE),
            accepted: RwLock::new(None),
        }
    }

    pub fn lifecycle(&self) -> ProfileEnvironmentLifecycle {
        match self.admission.load(Ordering::Acquire) {
            ADMISSION_ACTIVE => ProfileEnvironmentLifecycle::Active,
            ADMISSION_CLOSING => ProfileEnvironmentLifecycle::Closing,
            ADMISSION_CLOSED => ProfileEnvironmentLifecycle::Closed,
            _ => unreachable!("profile admission has only three states"),
        }
    }

    pub fn current(&self) -> Option<Arc<AcceptedProfileEnvironment>> {
        self.accepted
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn replace_accepted(
        &self,
        world: Arc<RegisteredSemanticWorld>,
    ) -> Result<Arc<AcceptedProfileEnvironment>, AcceptedEnvironmentReplaceError> {
        self.replace_accepted_after_admission(world, || {})
    }

    fn replace_accepted_after_admission(
        &self,
        world: Arc<RegisteredSemanticWorld>,
        after_admission: impl FnOnce(),
    ) -> Result<Arc<AcceptedProfileEnvironment>, AcceptedEnvironmentReplaceError> {
        if self.lifecycle() != ProfileEnvironmentLifecycle::Active {
            return Err(AcceptedEnvironmentReplaceError::ShuttingDown);
        }
        let mut accepted = self
            .accepted
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        if self.lifecycle() != ProfileEnvironmentLifecycle::Active {
            return Err(AcceptedEnvironmentReplaceError::ShuttingDown);
        }
        after_admission();
        let generation = accepted.as_ref().map_or(Ok(1), |current| {
            current
                .generation()
                .get()
                .checked_add(1)
                .ok_or(AcceptedEnvironmentReplaceError::GenerationOverflow)
        })?;
        let candidate = Arc::new(AcceptedProfileEnvironment {
            generation: AcceptedEnvironmentGeneration(generation),
            world,
            caches: ProfileSemanticCaches::default(),
        });
        accepted.replace(Arc::clone(&candidate));
        Ok(candidate)
    }

    pub fn shutdown(&self) {
        if self
            .admission
            .compare_exchange(
                ADMISSION_ACTIVE,
                ADMISSION_CLOSING,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            return;
        }
        let mut accepted = self
            .accepted
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(current) = accepted.take() {
            current.caches.clear();
        }
        self.admission.store(ADMISSION_CLOSED, Ordering::Release);
    }
}

impl Default for LspProfileState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_lang_hir::{
        lower::lower_document_to_hir,
        project::{HirProject, HirProjectModule},
        symbol::{CallablePackageId, ProjectSymbolWorldId},
    };
    use arcweft_lang_sema::{
        env::TypeCheckEnv,
        registration::{
            CharacterRegistrar, CharacterRegistrationRequest, ProjectRegistrationFacts,
        },
        types::TypeKind,
    };
    use arcweft_lang_syntax::{ast::module_path::CanonicalModulePath, parser::parse_source};
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
    use std::{
        sync::{Barrier, mpsc},
        thread,
        time::{Duration, Instant},
    };

    fn registered_world() -> Arc<RegisteredSemanticWorld> {
        registered_world_with_base(TypeCheckEnv::standard())
    }

    fn registered_world_with_base(base: TypeCheckEnv) -> Arc<RegisteredSemanticWorld> {
        let source = "flow @flow.main main { return \"ok\" }\n";
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-project://cache-tests/src/main.arcw")
                    .expect("document id"),
                SourceName::path("src/main.arcw"),
                source,
            )
            .expect("source document"),
        );
        let parsed = parse_source(source);
        assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
        let hir = lower_document_to_hir(&document, parsed.typed_tree()).expect("lowered HIR");
        let project = HirProject::new(
            "cache-tests",
            [HirProjectModule::new(
                CanonicalModulePath::crate_root(),
                document.identity().clone(),
                hir,
            )],
        )
        .expect("HIR project");
        let world = ProjectSymbolWorldId::try_new(
            CallablePackageId::try_new("cache-tests").expect("package"),
            document.identity().id().clone(),
            "test",
        )
        .expect("world");
        let facts = ProjectRegistrationFacts::try_new(
            world,
            vec![Arc::clone(&document)],
            Vec::new(),
            Vec::new(),
        )
        .expect("registration facts");
        Arc::new(
            CharacterRegistrar::register(CharacterRegistrationRequest::new(
                Arc::new(base),
                &project,
                &facts,
                None,
            ))
            .expect("registered semantic world"),
        )
    }

    fn insert_cache(environment: &AcceptedProfileEnvironment, key: &str, value: &str) {
        environment
            .caches
            .entries
            .lock()
            .expect("cache lock")
            .push((key.to_owned(), value.to_owned()));
        environment.caches.hits.fetch_add(1, Ordering::AcqRel);
    }

    fn cache_snapshot(environment: &AcceptedProfileEnvironment) -> (Vec<(String, String)>, u64) {
        (
            environment
                .caches
                .entries
                .lock()
                .expect("cache lock")
                .clone(),
            environment.caches.hits.load(Ordering::Acquire),
        )
    }

    #[test]
    fn successful_identical_rebuild_increments_generation() {
        let state = LspProfileState::new();
        let world = registered_world();
        let first = state
            .replace_accepted(Arc::clone(&world))
            .expect("first accepted environment");
        insert_cache(&first, "analysis", "cached");
        let second = state
            .replace_accepted(world)
            .expect("identical complete rebuild is still a new generation");
        assert_eq!(first.generation().get(), 1);
        assert_eq!(second.generation().get(), 2);
        assert_eq!(cache_snapshot(&first).1, 1);
        assert_eq!(cache_snapshot(&second), (Vec::new(), 0));
    }

    #[test]
    fn base_change_same_character_invalidates_broad_cache() {
        let state = LspProfileState::new();
        let first_world = registered_world_with_base(
            TypeCheckEnv::standard().with_symbol("adapter.mode", TypeKind::String),
        );
        let second_world = registered_world_with_base(
            TypeCheckEnv::standard().with_symbol("adapter.mode", TypeKind::Bool),
        );
        assert_eq!(
            first_world.environment().character_digest(),
            second_world.environment().character_digest(),
            "the narrow character key deliberately cannot observe base facts"
        );

        let first = state
            .replace_accepted(first_world)
            .expect("first accepted environment");
        insert_cache(&first, "analysis", "old base");
        let second = state
            .replace_accepted(second_world)
            .expect("changed base is a complete accepted rebuild");

        assert_eq!(second.generation().get(), 2);
        assert_eq!(cache_snapshot(&second), (Vec::new(), 0));
        assert!(Arc::ptr_eq(
            &state.current().expect("current environment"),
            &second
        ));
        assert_eq!(cache_snapshot(&first).1, 1);
    }

    #[test]
    fn generation_overflow_preserves_state() {
        let state = LspProfileState::new();
        let previous = Arc::new(AcceptedProfileEnvironment {
            generation: AcceptedEnvironmentGeneration::for_test(u64::MAX),
            world: registered_world(),
            caches: ProfileSemanticCaches::default(),
        });
        insert_cache(&previous, "analysis", "cached");
        state
            .accepted
            .write()
            .expect("accepted state lock")
            .replace(Arc::clone(&previous));

        assert_eq!(
            state
                .replace_accepted(registered_world())
                .expect_err("generation overflow rejects replacement"),
            AcceptedEnvironmentReplaceError::GenerationOverflow
        );
        let retained = state.current().expect("old environment remains accepted");
        assert!(Arc::ptr_eq(&retained, &previous));
        assert_eq!(cache_snapshot(&retained).1, 1);
    }

    #[test]
    fn shutdown_rejects_new_rebuilds() {
        let state = LspProfileState::new();
        state
            .replace_accepted(registered_world())
            .expect("accepted environment");

        state.shutdown();

        assert_eq!(state.lifecycle(), ProfileEnvironmentLifecycle::Closed);
        assert!(state.current().is_none());
        assert_eq!(
            state
                .replace_accepted(registered_world())
                .expect_err("shutdown rejects replacement"),
            AcceptedEnvironmentReplaceError::ShuttingDown
        );
        state.shutdown();
        assert_eq!(state.lifecycle(), ProfileEnvironmentLifecycle::Closed);
    }

    #[test]
    fn shutdown_clears_cache_before_world_drop() {
        let state = LspProfileState::new();
        let reader = state
            .replace_accepted(registered_world())
            .expect("accepted environment");
        insert_cache(&reader, "analysis", "cached");
        assert_eq!(Arc::strong_count(&reader), 2);

        state.shutdown();

        assert_eq!(cache_snapshot(&reader), (Vec::new(), 0));
        assert_eq!(Arc::strong_count(&reader), 1);
        assert!(state.current().is_none());
    }

    #[test]
    fn shutdown_closes_admission_before_waiting_for_replacement() {
        let state = Arc::new(LspProfileState::new());
        state
            .replace_accepted(registered_world())
            .expect("initial environment");
        let admitted = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let replacement = {
            let state = Arc::clone(&state);
            let admitted = Arc::clone(&admitted);
            let release = Arc::clone(&release);
            thread::spawn(move || {
                state.replace_accepted_after_admission(registered_world(), || {
                    admitted.wait();
                    release.wait();
                })
            })
        };
        admitted.wait();
        let shutdown = {
            let state = Arc::clone(&state);
            thread::spawn(move || state.shutdown())
        };
        wait_for_lifecycle(&state, ProfileEnvironmentLifecycle::Closing);
        release.wait();
        let replacement = replacement
            .join()
            .expect("replacement thread")
            .expect("replacement passed the second admission check");
        shutdown.join().expect("shutdown thread");
        assert_eq!(replacement.generation().get(), 2);
        assert_eq!(state.lifecycle(), ProfileEnvironmentLifecycle::Closed);
        assert!(state.current().is_none());
        assert_eq!(cache_snapshot(&replacement), (Vec::new(), 0));

        let state = Arc::new(LspProfileState::new());
        state
            .replace_accepted(registered_world())
            .expect("initial environment");
        let accepted_guard = state.accepted.write().expect("accepted state lock");
        let (started_tx, started_rx) = mpsc::channel();
        let replacement = {
            let state = Arc::clone(&state);
            thread::spawn(move || {
                started_tx.send(()).expect("replacement start signal");
                state.replace_accepted(registered_world())
            })
        };
        started_rx.recv().expect("replacement started");
        let shutdown = {
            let state = Arc::clone(&state);
            thread::spawn(move || state.shutdown())
        };
        wait_for_lifecycle(&state, ProfileEnvironmentLifecycle::Closing);
        drop(accepted_guard);
        assert_eq!(
            replacement
                .join()
                .expect("replacement thread")
                .expect_err("candidate did not pass the second admission check"),
            AcceptedEnvironmentReplaceError::ShuttingDown
        );
        shutdown.join().expect("shutdown thread");
        assert_eq!(state.lifecycle(), ProfileEnvironmentLifecycle::Closed);
        assert!(state.current().is_none());
    }

    fn wait_for_lifecycle(state: &LspProfileState, expected: ProfileEnvironmentLifecycle) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while state.lifecycle() != expected {
            assert!(
                Instant::now() < deadline,
                "profile lifecycle did not reach {expected:?}"
            );
            thread::yield_now();
        }
    }
}
