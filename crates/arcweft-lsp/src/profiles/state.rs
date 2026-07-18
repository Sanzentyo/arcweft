//! Atomically published, generation-keyed semantic profile state.

use std::{
    collections::BTreeMap,
    sync::{
        Arc, PoisonError, RwLock,
        atomic::{AtomicU8, Ordering},
    },
};

use arcweft_lang_hir::symbol::{ProjectSymbolRevision, ProjectSymbolWorldId};
use arcweft_lang_sema::character_definition::{
    CharacterDefinitionQueryResult, CharacterDefinitionRequestBudget,
    CharacterDefinitionResourceError, CharacterDefinitionWorkReceipt, CharacterReferenceInventory,
};
use arcweft_lang_sema::registration::RegisteredSemanticWorld;
use arcweft_launch::ProfileId;
use arcweft_source::{SourceDocumentIdentity, SourceSetRevision};
use lsp_types::Uri;
use thiserror::Error;

use crate::uri_key::LspUriKey;

use super::accepted_project::AcceptedProjectSnapshot;
use super::caches::{
    CharacterDefinitionCacheKey, CharacterReferenceCacheKey, ProfileSemanticCaches,
};

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
        world: Arc<RegisteredSemanticWorld>,
        project: Arc<AcceptedProjectSnapshot>,
        overlays: AcceptedOverlaySet,
    ) -> Result<Self, AcceptedProfileCandidateError> {
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
            Arc::clone(&current.world),
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
    pub(crate) fn insert_cache_for_test(&self, key: &str, value: &str) {
        self.caches.insert_for_test(key, value);
    }

    #[cfg(test)]
    pub(crate) fn cache_snapshot_for_test(&self) -> (Vec<(String, String)>, u64) {
        self.caches.snapshot_for_test()
    }

    #[cfg(test)]
    pub(crate) fn character_cache_entries_for_test(&self) -> (bool, bool) {
        self.caches.character_entries_for_test()
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
        self.replace_accepted_with(None, candidate, |_| {})
    }

    pub(crate) fn replace_accepted_with(
        &self,
        expected: Option<&Arc<AcceptedProfileEnvironment>>,
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
        if let Some(expected) = expected
            && accepted
                .as_ref()
                .is_none_or(|current| !Arc::ptr_eq(current, expected))
        {
            return Err(AcceptedEnvironmentReplaceError::CurrentChanged);
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
            world,
            project,
            overlays,
        } = candidate;
        let candidate = Arc::new(AcceptedProfileEnvironment {
            generation: AcceptedEnvironmentGeneration(generation),
            profile,
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
mod tests {
    use super::*;
    use crate::profiles::accepted_project::{
        AcceptedProjectSnapshot, AcceptedSourceAccess, AcceptedSourceDocumentSeed,
        AcceptedSourceLocator, AcceptedSourceOwnership,
    };
    use arcweft_lang_hir::{
        lower::lower_document_to_hir,
        project::{HirProject, HirProjectModule},
        symbol::{
            CallablePackageId, ExternalDeclarationSeed, ProjectDirectBinding, ProjectSymbolWorldId,
        },
    };
    use arcweft_lang_sema::{
        env::TypeCheckEnv,
        registration::{
            CharacterRegistrar, CharacterRegistrationDiagnosticKind, CharacterRegistrationRequest,
            EnvironmentBindingId, ExternalRegistrationFact, ProjectRegistrationFacts,
            RegisteredExternalOwner,
        },
        types::TypeKind,
    };
    use arcweft_lang_syntax::{
        ast::{
            common::Visibility,
            module_path::{CanonicalModulePath, ModulePathRoot},
            symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
        },
        parser::parse_source,
    };
    use arcweft_launch::ProfileId;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};
    use std::{
        sync::{Barrier, mpsc},
        thread,
        time::{Duration, Instant},
    };

    fn registered_world() -> Arc<RegisteredSemanticWorld> {
        registered_world_with_base(TypeCheckEnv::standard())
    }

    fn registered_world_with_base(base: TypeCheckEnv) -> Arc<RegisteredSemanticWorld> {
        let (document, project) = project_fixture();
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
                project.as_ref(),
                &facts,
                None,
            ))
            .expect("registered semantic world"),
        )
    }

    fn project_fixture() -> (Arc<SourceDocument>, Arc<HirProject>) {
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
        let project = Arc::new(
            HirProject::new(
                "cache-tests",
                [HirProjectModule::try_new(
                    CanonicalModulePath::crate_root(),
                    document.identity().clone(),
                    hir,
                )
                .expect("cache fixture module binding")],
            )
            .expect("HIR project"),
        );
        (document, project)
    }

    fn accepted_candidate(world: Arc<RegisteredSemanticWorld>) -> AcceptedProfileCandidate {
        let (document, project) = project_fixture();
        let source_uri = "file:///workspace/cache-tests/src/main.arcw"
            .parse::<Uri>()
            .expect("source URI");
        let project = Arc::new(
            AcceptedProjectSnapshot::try_new(
                project,
                world.as_ref(),
                vec![AcceptedSourceDocumentSeed::new(
                    document,
                    AcceptedSourceLocator::Uri { uri: source_uri },
                    AcceptedSourceOwnership::Workspace,
                    AcceptedSourceAccess::Writable,
                )],
            )
            .expect("accepted project snapshot"),
        );
        let workspace_uri = "file:///workspace/cache-tests"
            .parse::<Uri>()
            .expect("workspace URI");
        let manifest_uri = "file:///workspace/cache-tests/arcw.toml"
            .parse::<Uri>()
            .expect("manifest URI");
        AcceptedProfileCandidate::try_new(
            AcceptedProfileKey::new(
                &workspace_uri,
                &manifest_uri,
                ProfileId::new("test").expect("valid test profile ID"),
            ),
            world,
            project,
            AcceptedOverlaySet::default(),
        )
        .expect("complete candidate")
    }

    fn external_registration_fact(
        document: &SourceDocument,
        owner: &str,
        binding: ProjectSymbolPath,
    ) -> ExternalRegistrationFact {
        let declaration = document
            .span(SourceRange::new(0, document.text().len()))
            .expect("external declaration span");
        let direct_binding = ProjectDirectBinding::try_new(
            CanonicalModulePath::crate_root(),
            binding,
            Some(Visibility::Public),
            declaration.clone(),
            false,
        )
        .expect("typed direct binding");
        let seed = ExternalDeclarationSeed::try_new(
            SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), owner)
                .expect("opaque canonical path"),
            Some(Visibility::Public),
            declaration.clone(),
            vec![direct_binding],
        )
        .expect("external declaration seed");
        ExternalRegistrationFact::new(
            seed,
            RegisteredExternalOwner::Environment(
                EnvironmentBindingId::try_new(owner).expect("environment owner"),
            ),
            declaration,
        )
    }

    fn colliding_typed_binding_registration()
    -> arcweft_lang_sema::registration::CharacterRegistrationReport {
        let (root, project) = project_fixture();
        let world = ProjectSymbolWorldId::try_new(
            CallablePackageId::try_new("cache-tests").expect("package"),
            root.identity().id().clone(),
            "test",
        )
        .expect("world");
        let first = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-generated://cache-tests/adapter-first")
                    .expect("document id"),
                SourceName::Generated,
                "adapter.first",
            )
            .expect("first adapter document"),
        );
        let second = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-generated://cache-tests/adapter-second")
                    .expect("document id"),
                SourceName::Generated,
                "adapter.second",
            )
            .expect("second adapter document"),
        );
        let shared = || {
            ProjectSymbolPath::new(
                ModulePathRoot::ImplicitCrate,
                [ProjectSymbolSegment::try_new("shared").expect("valid shared segment")],
            )
            .expect("shared typed binding path")
        };
        let first_fact = external_registration_fact(&first, "adapter.first", shared());
        let second_fact = external_registration_fact(&second, "adapter.second", shared());
        let facts = ProjectRegistrationFacts::try_new(
            world,
            vec![root, first, second],
            vec![first_fact, second_fact],
            Vec::new(),
        )
        .expect("colliding facts retain typed evidence");
        let base = TypeCheckEnv::standard()
            .with_symbol("adapter.first", TypeKind::I32)
            .with_symbol("adapter.second", TypeKind::I64);

        CharacterRegistrar::register(CharacterRegistrationRequest::new(
            Arc::new(base),
            project.as_ref(),
            &facts,
            None,
        ))
        .expect_err("typed binding collision rejects the semantic candidate")
    }

    fn insert_cache(environment: &AcceptedProfileEnvironment, key: &str, value: &str) {
        environment.insert_cache_for_test(key, value);
    }

    fn cache_snapshot(environment: &AcceptedProfileEnvironment) -> (Vec<(String, String)>, u64) {
        environment.cache_snapshot_for_test()
    }

    #[test]
    fn successful_identical_rebuild_increments_generation() {
        let state = LspProfileState::new();
        let world = registered_world();
        let first = state
            .replace_accepted(accepted_candidate(Arc::clone(&world)))
            .expect("first accepted environment");
        insert_cache(&first, "analysis", "cached");
        let second = state
            .replace_accepted(accepted_candidate(world))
            .expect("identical complete rebuild is still a new generation");
        assert_eq!(first.generation().get(), 1);
        assert_eq!(second.generation().get(), 2);
        assert_eq!(cache_snapshot(&first).1, 1);
        assert_eq!(cache_snapshot(&second), (Vec::new(), 0));
    }

    #[test]
    fn failed_typed_binding_collision_preserves_accepted_pointer_and_caches() {
        let state = LspProfileState::new();
        let accepted = state
            .replace_accepted(accepted_candidate(registered_world()))
            .expect("baseline accepted environment");
        insert_cache(&accepted, "analysis", "retained");
        let accepted_world = Arc::clone(accepted.world());

        let report = colliding_typed_binding_registration();
        assert!(
            report.diagnostics().iter().any(|diagnostic| matches!(
                diagnostic.kind(),
                CharacterRegistrationDiagnosticKind::CallableCatalog {
                    code:
                        arcweft_lang_sema::callable::CallableDiagnosticCode::CorruptCallableCatalog,
                }
            )),
            "{:?}",
            report.diagnostics()
        );
        let retained = state.current().expect("baseline remains accepted");
        assert!(Arc::ptr_eq(&retained, &accepted));
        assert!(Arc::ptr_eq(retained.world(), &accepted_world));
        assert!(std::ptr::eq(
            retained.world().symbols(),
            accepted.world().symbols()
        ));
        assert!(std::ptr::eq(
            retained.world().environment(),
            accepted.world().environment()
        ));
        assert!(std::ptr::eq(
            retained.world().environment().callable_catalog(),
            accepted.world().environment().callable_catalog()
        ));
        assert!(std::ptr::eq(
            retained.world().character_definition_index(),
            accepted.world().character_definition_index()
        ));
        assert_eq!(retained.generation().get(), 1);
        assert_eq!(
            cache_snapshot(&retained),
            (vec![("analysis".to_owned(), "retained".to_owned())], 1)
        );

        let replacement = state
            .replace_accepted(accepted_candidate(registered_world()))
            .expect("next valid candidate is accepted");
        assert_eq!(replacement.generation().get(), 2);
        assert!(!Arc::ptr_eq(&replacement, &accepted));
        assert_eq!(cache_snapshot(&replacement), (Vec::new(), 0));
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
            .replace_accepted(accepted_candidate(first_world))
            .expect("first accepted environment");
        insert_cache(&first, "analysis", "old base");
        let second = state
            .replace_accepted(accepted_candidate(second_world))
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
        let AcceptedProfileCandidate {
            profile,
            world,
            project,
            overlays,
        } = accepted_candidate(registered_world());
        let previous = Arc::new(AcceptedProfileEnvironment {
            generation: AcceptedEnvironmentGeneration::for_test(u64::MAX),
            profile,
            world,
            project,
            overlays,
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
                .replace_accepted(accepted_candidate(registered_world()))
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
            .replace_accepted(accepted_candidate(registered_world()))
            .expect("accepted environment");

        state.shutdown();

        assert_eq!(state.lifecycle(), ProfileEnvironmentLifecycle::Closed);
        assert!(state.current().is_none());
        assert_eq!(
            state
                .replace_accepted(accepted_candidate(registered_world()))
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
            .replace_accepted(accepted_candidate(registered_world()))
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
            .replace_accepted(accepted_candidate(registered_world()))
            .expect("initial environment");
        let admitted = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let replacement = {
            let state = Arc::clone(&state);
            let admitted = Arc::clone(&admitted);
            let release = Arc::clone(&release);
            thread::spawn(move || {
                state.replace_accepted_with(None, accepted_candidate(registered_world()), |_| {
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
            .replace_accepted(accepted_candidate(registered_world()))
            .expect("initial environment");
        let accepted_guard = state.accepted.write().expect("accepted state lock");
        let (started_tx, started_rx) = mpsc::channel();
        let replacement = {
            let state = Arc::clone(&state);
            thread::spawn(move || {
                started_tx.send(()).expect("replacement start signal");
                state.replace_accepted(accepted_candidate(registered_world()))
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
