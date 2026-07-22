//! Atomically published, generation-keyed semantic profile state.

use std::{
    collections::BTreeMap,
    sync::{
        Arc, PoisonError, RwLock,
        atomic::{AtomicU8, Ordering},
    },
};

use arcweft_compiler::project::CompiledProject;
use arcweft_lang_hir::symbol::{ProjectSymbolRevision, ProjectSymbolWorldId};
use arcweft_lang_sema::character_definition::{
    CharacterDefinitionQueryResult, CharacterDefinitionRequestBudget,
    CharacterDefinitionResourceError, CharacterDefinitionWorkReceipt, CharacterReferenceInventory,
};
use arcweft_lang_sema::registration::RegisteredSemanticWorld;
#[cfg(test)]
use arcweft_lang_sema::signature::SignatureQueryOutcome;
use arcweft_launch::ProfileId;
use arcweft_source::{SourceDocumentIdentity, SourceSetRevision};
use lsp_types::Uri;
use thiserror::Error;

use crate::uri_key::LspUriKey;

use super::accepted_project::AcceptedProjectSnapshot;
#[cfg(test)]
use super::caches::SignatureCacheTestSnapshot;
use super::caches::{
    CharacterDefinitionCacheKey, CharacterReferenceCacheKey, ProfileSemanticCaches,
    SignatureCacheGuard,
};
#[cfg(test)]
use super::caches::{SignatureCacheInsertion, SignatureCacheKey};

const ADMISSION_ACTIVE: u8 = 0;
const ADMISSION_CLOSING: u8 = 1;
const ADMISSION_CLOSED: u8 = 2;

/// Stable identity of one workspace manifest and selected launch profile.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceptedProfileKey {
    workspace_uri: LspUriKey,
    manifest_uri: LspUriKey,
    profile_id: ProfileId,
}

/// Exact editor overlays consumed by one accepted build.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AcceptedOverlaySet {
    entries: BTreeMap<LspUriKey, AcceptedOverlayEntry>,
}

/// One URI/version snapshot rebound to its logical project document identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedOverlayEntry {
    version: i32,
    logical_identity: SourceDocumentIdentity,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum AcceptedOverlaySetError {
    #[error("accepted overlay set contains duplicate URI")]
    DuplicateUri { uri: LspUriKey },
}

/// Complete validated publication candidate; it cannot be assembled world-only.
#[derive(Debug)]
pub struct AcceptedProfileCandidate {
    profile: AcceptedProfileKey,
    compiled: Arc<CompiledProject>,
    world: Arc<RegisteredSemanticWorld>,
    project: Arc<AcceptedProjectSnapshot>,
    overlays: AcceptedOverlaySet,
}

/// Candidate construction failed before any profile state was mutated.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum AcceptedProfileCandidateError {
    #[error("candidate world ID differs from the accepted project")]
    WorldMismatch {
        expected: ProjectSymbolWorldId,
        actual: ProjectSymbolWorldId,
    },
    #[error("candidate symbol revision differs from the accepted project")]
    SymbolRevisionMismatch {
        expected: ProjectSymbolRevision,
        actual: ProjectSymbolRevision,
    },
    #[error("candidate character source revision differs from accepted sources")]
    CharacterSourceRevisionMismatch {
        expected: SourceSetRevision,
        actual: SourceSetRevision,
    },
    #[error("candidate compiled HIR differs from the accepted project snapshot")]
    CompiledHirMismatch,
    #[error("candidate overlay URI is absent from accepted sources")]
    UnknownOverlayUri { uri: LspUriKey },
    #[error("candidate overlay identity differs from accepted URI identity")]
    OverlayIdentityMismatch {
        uri: LspUriKey,
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
}

impl AcceptedProfileKey {
    pub fn new(workspace_uri: &Uri, manifest_uri: &Uri, profile_id: ProfileId) -> Self {
        Self {
            workspace_uri: LspUriKey::from_uri(workspace_uri),
            manifest_uri: LspUriKey::from_uri(manifest_uri),
            profile_id,
        }
    }

    pub const fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    pub(crate) const fn workspace_key(&self) -> &LspUriKey {
        &self.workspace_uri
    }

    pub(crate) const fn manifest_key(&self) -> &LspUriKey {
        &self.manifest_uri
    }
}

impl AcceptedOverlaySet {
    pub(crate) fn try_new(
        entries: impl IntoIterator<Item = (LspUriKey, AcceptedOverlayEntry)>,
    ) -> Result<Self, AcceptedOverlaySetError> {
        let mut accepted = BTreeMap::new();
        for (uri, entry) in entries {
            if accepted.insert(uri.clone(), entry).is_some() {
                return Err(AcceptedOverlaySetError::DuplicateUri { uri });
            }
        }
        Ok(Self { entries: accepted })
    }

    pub(crate) fn get(&self, uri: &LspUriKey) -> Option<&AcceptedOverlayEntry> {
        self.entries.get(uri)
    }

    pub(crate) fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (&LspUriKey, &AcceptedOverlayEntry)> {
        self.entries.iter()
    }
}

impl AcceptedOverlayEntry {
    pub(crate) fn new(version: i32, logical_identity: SourceDocumentIdentity) -> Self {
        Self {
            version,
            logical_identity,
        }
    }

    pub(crate) const fn version(&self) -> i32 {
        self.version
    }

    pub const fn logical_identity(&self) -> &SourceDocumentIdentity {
        &self.logical_identity
    }
}

impl AcceptedProfileCandidate {
    #[allow(
        clippy::result_large_err,
        reason = "candidate admission preserves exact world, revision, URI, and source identity evidence"
    )]
    pub(crate) fn try_new(
        profile: AcceptedProfileKey,
        compiled: Arc<CompiledProject>,
        project: Arc<AcceptedProjectSnapshot>,
        overlays: AcceptedOverlaySet,
    ) -> Result<Self, AcceptedProfileCandidateError> {
        if compiled.hir_project() != project.hir_project().as_ref() {
            return Err(AcceptedProfileCandidateError::CompiledHirMismatch);
        }
        let world = compiled.registered_world_arc();
        let symbols = world.symbols();
        let index = world.character_definition_index();
        let sources = project.sources();
        if sources.world() != symbols.world() {
            return Err(AcceptedProfileCandidateError::WorldMismatch {
                expected: sources.world().clone(),
                actual: symbols.world().clone(),
            });
        }
        if sources.symbol_revision() != symbols.revision() {
            return Err(AcceptedProfileCandidateError::SymbolRevisionMismatch {
                expected: *sources.symbol_revision(),
                actual: *symbols.revision(),
            });
        }
        if sources.all_source_revision() != index.source_revision() {
            return Err(
                AcceptedProfileCandidateError::CharacterSourceRevisionMismatch {
                    expected: sources.all_source_revision(),
                    actual: index.source_revision(),
                },
            );
        }
        for (uri, overlay) in overlays.iter() {
            let Some(identity) = project.source_identity_by_uri(uri) else {
                return Err(AcceptedProfileCandidateError::UnknownOverlayUri { uri: uri.clone() });
            };
            if identity != overlay.logical_identity() {
                return Err(AcceptedProfileCandidateError::OverlayIdentityMismatch {
                    uri: uri.clone(),
                    expected: identity.clone(),
                    actual: overlay.logical_identity().clone(),
                });
            }
        }
        Ok(Self {
            profile,
            compiled,
            world,
            project,
            overlays,
        })
    }

    #[allow(
        clippy::result_large_err,
        reason = "unchanged-project admission preserves the same exact candidate rejection evidence"
    )]
    pub(crate) fn try_from_unchanged_project(
        current: &Arc<AcceptedProfileEnvironment>,
        overlays: AcceptedOverlaySet,
    ) -> Result<Self, AcceptedProfileCandidateError> {
        Self::try_new(
            current.profile.clone(),
            Arc::clone(&current.compiled),
            Arc::clone(&current.project),
            overlays,
        )
    }

    pub const fn profile(&self) -> &AcceptedProfileKey {
        &self.profile
    }

    pub const fn world(&self) -> &Arc<RegisteredSemanticWorld> {
        &self.world
    }

    pub(crate) const fn project(&self) -> &Arc<AcceptedProjectSnapshot> {
        &self.project
    }

    pub const fn overlays(&self) -> &AcceptedOverlaySet {
        &self.overlays
    }
}

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
    #[error("expected accepted environment is no longer current")]
    CurrentChanged,
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

/// One immutable registered world plus its fresh generation-owned cache namespace.
#[derive(Debug)]
pub struct AcceptedProfileEnvironment {
    generation: AcceptedEnvironmentGeneration,
    profile: AcceptedProfileKey,
    compiled: Arc<CompiledProject>,
    world: Arc<RegisteredSemanticWorld>,
    project: Arc<AcceptedProjectSnapshot>,
    overlays: AcceptedOverlaySet,
    caches: ProfileSemanticCaches,
}

impl AcceptedProfileEnvironment {
    pub const fn generation(&self) -> AcceptedEnvironmentGeneration {
        self.generation
    }

    pub const fn world(&self) -> &Arc<RegisteredSemanticWorld> {
        &self.world
    }

    pub(crate) const fn compiled(&self) -> &Arc<CompiledProject> {
        &self.compiled
    }

    pub const fn profile(&self) -> &AcceptedProfileKey {
        &self.profile
    }

    pub(crate) const fn project(&self) -> &Arc<AcceptedProjectSnapshot> {
        &self.project
    }

    pub const fn overlays(&self) -> &AcceptedOverlaySet {
        &self.overlays
    }

    pub(crate) fn clear_caches(&self) {
        self.caches.clear();
    }

    pub(crate) fn signature_cache(&self) -> SignatureCacheGuard<'_> {
        self.caches.signature_cache()
    }

    pub(crate) fn evict_signature_document(&self, document: &SourceDocumentIdentity) -> usize {
        self.caches.evict_signature_document(document)
    }

    pub(crate) fn cached_character_references(
        &self,
        key: &CharacterReferenceCacheKey,
        budget: &mut CharacterDefinitionRequestBudget,
    ) -> Result<Option<Arc<CharacterReferenceInventory>>, CharacterDefinitionResourceError> {
        self.caches.cached_character_references(key, budget)
    }

    pub(crate) fn cache_character_references(
        &self,
        key: CharacterReferenceCacheKey,
        inventory: Arc<CharacterReferenceInventory>,
        work: CharacterDefinitionWorkReceipt,
    ) {
        self.caches.cache_character_references(key, inventory, work);
    }

    pub(crate) fn cached_character_definition(
        &self,
        key: &CharacterDefinitionCacheKey,
        budget: &mut CharacterDefinitionRequestBudget,
    ) -> Result<Option<Arc<CharacterDefinitionQueryResult>>, CharacterDefinitionResourceError> {
        self.caches.cached_character_definition(key, budget)
    }

    pub(crate) fn cache_character_definition(
        &self,
        key: CharacterDefinitionCacheKey,
        result: Arc<CharacterDefinitionQueryResult>,
        work: CharacterDefinitionWorkReceipt,
    ) {
        self.caches.cache_character_definition(key, result, work);
    }

    #[cfg(test)]
    pub(crate) fn character_cache_entries_for_test(&self) -> (bool, bool) {
        self.caches.character_entries_for_test()
    }

    #[cfg(test)]
    fn signature_cache_key_for_test(&self, byte_offset: usize) -> SignatureCacheKey {
        let source = self
            .project
            .sources()
            .documents()
            .find_map(|accepted| {
                let source = accepted.document().identity().clone();
                self.project.module_key(&source).is_some().then_some(source)
            })
            .expect("accepted test environment has a module-backed source document");
        let symbols = self.world.symbols();
        let environment = self.world.environment();
        SignatureCacheKey::new(
            self.generation,
            symbols.world().clone(),
            *symbols.revision(),
            environment.character_revision(),
            environment.character_digest(),
            environment.environment_digest(),
            source,
            Some(1),
            byte_offset,
        )
    }

    #[cfg(test)]
    pub(crate) fn seed_signature_cache_for_test(&self, byte_offset: usize) {
        let _ = self.signature_cache().insert(
            self.signature_cache_key_for_test(byte_offset),
            Arc::new(SignatureQueryOutcome::NotApplicable(
                arcweft_lang_sema::signature::SignatureNotApplicable::CursorOutsideArgumentList,
            )),
            self.project.footprint().source_bytes(),
        );
    }

    #[cfg(test)]
    pub(crate) fn signature_cache_snapshot_for_test(&self) -> SignatureCacheTestSnapshot {
        self.caches.signature_snapshot_for_test()
    }

    #[cfg(test)]
    pub(crate) fn set_signature_access_clock_for_test(&self, value: u64) {
        self.caches.set_signature_access_clock_for_test(value);
    }

    #[cfg(test)]
    pub(crate) fn poison_signature_cache_for_test(&self) {
        self.caches.poison_signature_cache_for_test();
    }
}

/// Single-writer publication boundary for accepted LSP semantic environments.
#[derive(Debug)]
pub struct LspProfileState {
    admission: AtomicU8,
    accepted: RwLock<Option<Arc<AcceptedProfileEnvironment>>>,
}

#[derive(Clone, Copy)]
enum AcceptedEnvironmentExpectation<'a> {
    Any,
    Exact(Option<&'a Arc<AcceptedProfileEnvironment>>),
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
        self.accepted_read().clone()
    }

    pub(crate) fn accepted_read(
        &self,
    ) -> std::sync::RwLockReadGuard<'_, Option<Arc<AcceptedProfileEnvironment>>> {
        self.accepted.read().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn accepted_write(
        &self,
    ) -> std::sync::RwLockWriteGuard<'_, Option<Arc<AcceptedProfileEnvironment>>> {
        self.accepted
            .write()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub fn replace_accepted(
        &self,
        candidate: AcceptedProfileCandidate,
    ) -> Result<Arc<AcceptedProfileEnvironment>, AcceptedEnvironmentReplaceError> {
        self.replace_accepted_inner(AcceptedEnvironmentExpectation::Any, candidate, |_| {})
    }

    pub(crate) fn replace_accepted_with(
        &self,
        expected: Option<&Arc<AcceptedProfileEnvironment>>,
        candidate: AcceptedProfileCandidate,
        before_swap: impl FnOnce(Option<&Arc<AcceptedProfileEnvironment>>),
    ) -> Result<Arc<AcceptedProfileEnvironment>, AcceptedEnvironmentReplaceError> {
        self.replace_accepted_inner(
            AcceptedEnvironmentExpectation::Exact(expected),
            candidate,
            before_swap,
        )
    }

    fn replace_accepted_inner(
        &self,
        expected: AcceptedEnvironmentExpectation<'_>,
        candidate: AcceptedProfileCandidate,
        before_swap: impl FnOnce(Option<&Arc<AcceptedProfileEnvironment>>),
    ) -> Result<Arc<AcceptedProfileEnvironment>, AcceptedEnvironmentReplaceError> {
        if self.lifecycle() != ProfileEnvironmentLifecycle::Active {
            return Err(AcceptedEnvironmentReplaceError::ShuttingDown);
        }
        let mut accepted = self.accepted_write();
        if self.lifecycle() != ProfileEnvironmentLifecycle::Active {
            return Err(AcceptedEnvironmentReplaceError::ShuttingDown);
        }
        if let AcceptedEnvironmentExpectation::Exact(expected) = expected {
            let current_matches = match (expected, accepted.as_ref()) {
                (None, None) => true,
                (Some(expected), Some(current)) => Arc::ptr_eq(current, expected),
                (None | Some(_), _) => false,
            };
            if !current_matches {
                return Err(AcceptedEnvironmentReplaceError::CurrentChanged);
            }
        }
        let generation = accepted.as_ref().map_or(Ok(1), |current| {
            current
                .generation()
                .get()
                .checked_add(1)
                .ok_or(AcceptedEnvironmentReplaceError::GenerationOverflow)
        })?;
        before_swap(accepted.as_ref());
        let AcceptedProfileCandidate {
            profile,
            compiled,
            world,
            project,
            overlays,
        } = candidate;
        let candidate = Arc::new(AcceptedProfileEnvironment {
            generation: AcceptedEnvironmentGeneration(generation),
            profile,
            compiled,
            world,
            project,
            overlays,
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
        let mut accepted = self.accepted_write();
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
mod tests;
