//! Atomically published, generation-keyed semantic profile state.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, PoisonError, RwLock,
        atomic::{AtomicU8, Ordering},
    },
};

#[cfg(test)]
use std::sync::atomic::AtomicU64;

use arcweft_lang_hir::symbol::{ProjectSymbolRevision, ProjectSymbolWorldId};
use arcweft_lang_sema::{
    character_definition::{CharacterDefinitionQueryResult, CharacterReferenceInventory},
    registration::{
        CharacterDefinitionIndex, CharacterDefinitionLimitKind, CharacterDefinitionLimits,
        RegisteredSemanticWorld,
    },
};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceDocumentIdentity, SourceSetRevision,
    identity::SourceSnapshotId,
};
use lsp_types::Uri;
use thiserror::Error;

use crate::positions::{LineIndex, PositionEncoding};

const ADMISSION_ACTIVE: u8 = 0;
const ADMISSION_CLOSING: u8 = 1;
const ADMISSION_CLOSED: u8 = 2;

/// Stable identity of one workspace manifest and selected launch profile.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceptedProfileKey {
    workspace_uri: String,
    manifest_uri: String,
    profile_id: String,
}

/// Exact editor overlays consumed by one accepted build.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AcceptedOverlaySet {
    entries: Vec<AcceptedOverlayEntry>,
}

/// One URI/version snapshot rebound to its logical project document identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedOverlayEntry {
    uri: String,
    version: Option<i32>,
    logical_identity: SourceDocumentIdentity,
}

/// Immutable source adapter registry owned by one accepted generation.
#[derive(Debug)]
pub struct AcceptedSourceDocuments {
    world: ProjectSymbolWorldId,
    symbol_revision: ProjectSymbolRevision,
    all_source_revision: SourceSetRevision,
    by_identity: BTreeMap<SourceDocumentIdentity, AcceptedSourceDocument>,
    by_uri: BTreeMap<String, SourceDocumentIdentity>,
}

/// One exact source document and its explicit navigation metadata.
#[derive(Debug)]
pub struct AcceptedSourceDocument {
    document: Arc<SourceDocument>,
    locator: AcceptedSourceLocator,
    ownership: AcceptedSourceOwnership,
    access: AcceptedSourceAccess,
    line_index: LineIndex,
}

/// Explicit location authority; no display/source-name inverse parsing is allowed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptedSourceLocator {
    File { path: PathBuf, uri: Uri },
    Uri { uri: Uri },
    Unavailable,
}

/// Ownership category for one accepted source.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AcceptedSourceOwnership {
    Workspace,
    Dependency,
    Generated,
}

/// Mutability category retained for later edit policy, not definition eligibility.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AcceptedSourceAccess {
    Writable,
    ReadOnly,
    Unknown,
}

/// One explicit source record supplied to candidate construction.
#[derive(Clone, Debug)]
pub struct AcceptedSourceDocumentSeed {
    document: Arc<SourceDocument>,
    locator: AcceptedSourceLocator,
    ownership: AcceptedSourceOwnership,
    access: AcceptedSourceAccess,
}

#[derive(Default)]
struct AcceptedSourceRegistryBuilder {
    identities_by_id: BTreeMap<SourceDocumentId, SourceDocumentIdentity>,
    by_identity: BTreeMap<SourceDocumentIdentity, AcceptedSourceDocument>,
    by_uri: BTreeMap<String, SourceDocumentIdentity>,
}

/// Complete validated publication candidate; it cannot be assembled world-only.
#[derive(Debug)]
pub struct AcceptedProfileCandidate {
    profile: AcceptedProfileKey,
    world: Arc<RegisteredSemanticWorld>,
    sources: Arc<AcceptedSourceDocuments>,
    overlays: AcceptedOverlaySet,
}

/// Candidate construction failed before any profile state was mutated.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AcceptedProfileCandidateError {
    #[error("accepted candidate semantic world components disagree")]
    WorldInvariant,
    #[error("accepted source document count exceeds its production limit")]
    Limit {
        kind: CharacterDefinitionLimitKind,
        observed: u64,
        maximum: u64,
    },
    #[error("accepted source document id occurs with conflicting exact identities")]
    ConflictingDocument {
        id: SourceDocumentId,
        first: Box<SourceDocumentIdentity>,
        conflicting: Box<SourceDocumentIdentity>,
    },
    #[error("accepted source URI maps to more than one exact identity")]
    DuplicateUri {
        uri: String,
        first: Box<SourceDocumentIdentity>,
        conflicting: Box<SourceDocumentIdentity>,
    },
    #[error("accepted candidate omits a character declaration document")]
    MissingIndexedDocument { identity: SourceDocumentIdentity },
    #[error("accepted candidate source bytes differ from the indexed document")]
    IndexedDocumentMismatch { identity: SourceDocumentIdentity },
    #[error("accepted candidate source-set revision could not be constructed")]
    SourceSet,
    #[error("accepted candidate source-set revision differs from the registered index")]
    SourceRevision {
        expected: SourceSetRevision,
        actual: SourceSetRevision,
    },
    #[error("accepted overlay URI is not present in the explicit source registry")]
    UnknownOverlayUri { uri: String },
    #[error("accepted overlay logical identity differs from the URI source record")]
    OverlayIdentity {
        uri: String,
        expected: Box<SourceDocumentIdentity>,
        actual: Box<SourceDocumentIdentity>,
    },
}

impl AcceptedProfileKey {
    pub fn new(
        workspace_uri: impl Into<String>,
        manifest_uri: impl Into<String>,
        profile_id: impl Into<String>,
    ) -> Self {
        Self {
            workspace_uri: workspace_uri.into(),
            manifest_uri: manifest_uri.into(),
            profile_id: profile_id.into(),
        }
    }

    pub fn workspace_uri(&self) -> &str {
        &self.workspace_uri
    }

    pub fn manifest_uri(&self) -> &str {
        &self.manifest_uri
    }

    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }
}

impl AcceptedOverlaySet {
    pub fn try_new(
        entries: impl IntoIterator<Item = AcceptedOverlayEntry>,
    ) -> Result<Self, AcceptedProfileCandidateError> {
        let mut entries = entries.into_iter().collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            left.uri
                .cmp(&right.uri)
                .then_with(|| left.version.cmp(&right.version))
                .then_with(|| left.logical_identity.cmp(&right.logical_identity))
        });
        for pair in entries.windows(2) {
            if pair[0].uri == pair[1].uri && pair[0].logical_identity != pair[1].logical_identity {
                return Err(AcceptedProfileCandidateError::OverlayIdentity {
                    uri: pair[1].uri.clone(),
                    expected: Box::new(pair[0].logical_identity.clone()),
                    actual: Box::new(pair[1].logical_identity.clone()),
                });
            }
        }
        entries.dedup();
        Ok(Self { entries })
    }

    pub fn entries(&self) -> impl ExactSizeIterator<Item = &AcceptedOverlayEntry> {
        self.entries.iter()
    }
}

impl AcceptedOverlayEntry {
    pub fn new(
        uri: impl Into<String>,
        version: Option<i32>,
        logical_identity: SourceDocumentIdentity,
    ) -> Self {
        Self {
            uri: uri.into(),
            version,
            logical_identity,
        }
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub const fn version(&self) -> Option<i32> {
        self.version
    }

    pub const fn logical_identity(&self) -> &SourceDocumentIdentity {
        &self.logical_identity
    }
}

impl AcceptedSourceDocumentSeed {
    pub fn new(
        document: Arc<SourceDocument>,
        locator: AcceptedSourceLocator,
        ownership: AcceptedSourceOwnership,
        access: AcceptedSourceAccess,
    ) -> Self {
        Self {
            document,
            locator,
            ownership,
            access,
        }
    }

    pub fn document(&self) -> &Arc<SourceDocument> {
        &self.document
    }

    pub const fn locator(&self) -> &AcceptedSourceLocator {
        &self.locator
    }
}

impl AcceptedSourceRegistryBuilder {
    fn insert(
        &mut self,
        seed: AcceptedSourceDocumentSeed,
    ) -> Result<(), AcceptedProfileCandidateError> {
        let identity = seed.document.identity().clone();
        if let Some(first) = self.identities_by_id.get(identity.id())
            && first != &identity
        {
            return Err(AcceptedProfileCandidateError::ConflictingDocument {
                id: identity.id().clone(),
                first: Box::new(first.clone()),
                conflicting: Box::new(identity),
            });
        }
        self.identities_by_id
            .insert(identity.id().clone(), identity.clone());

        let maximum = CharacterDefinitionLimits::PRODUCTION.documents();
        let observed = u64::try_from(self.identities_by_id.len()).unwrap_or(u64::MAX);
        if observed > maximum {
            return Err(AcceptedProfileCandidateError::Limit {
                kind: CharacterDefinitionLimitKind::Documents,
                observed,
                maximum,
            });
        }

        if let Some(uri) = seed.locator.uri().map(|uri| uri.as_str().to_owned())
            && let Some(first) = self.by_uri.insert(uri.clone(), identity.clone())
            && first != identity
        {
            return Err(AcceptedProfileCandidateError::DuplicateUri {
                uri,
                first: Box::new(first),
                conflicting: Box::new(identity),
            });
        }
        self.by_identity
            .entry(identity)
            .or_insert_with(|| AcceptedSourceDocument {
                line_index: LineIndex::new(
                    seed.document.text().to_owned(),
                    PositionEncoding::default(),
                ),
                document: seed.document,
                locator: seed.locator,
                ownership: seed.ownership,
                access: seed.access,
            });
        Ok(())
    }

    fn validate_index(
        &self,
        index: &CharacterDefinitionIndex,
    ) -> Result<SourceSetRevision, AcceptedProfileCandidateError> {
        let mut indexed_identities = Vec::with_capacity(index.documents().len());
        for indexed in index.documents() {
            let identity = indexed.identity();
            let accepted = self.by_identity.get(identity).ok_or_else(|| {
                AcceptedProfileCandidateError::MissingIndexedDocument {
                    identity: identity.clone(),
                }
            })?;
            if accepted.document.text() != indexed.text() {
                return Err(AcceptedProfileCandidateError::IndexedDocumentMismatch {
                    identity: identity.clone(),
                });
            }
            indexed_identities.push(identity.clone());
        }
        let actual = SourceSetRevision::try_for_identities(indexed_identities.iter())
            .map_err(|_| AcceptedProfileCandidateError::SourceSet)?;
        if actual != index.source_revision() {
            return Err(AcceptedProfileCandidateError::SourceRevision {
                expected: index.source_revision(),
                actual,
            });
        }
        Ok(actual)
    }

    fn validate_overlays(
        &self,
        overlays: &AcceptedOverlaySet,
    ) -> Result<(), AcceptedProfileCandidateError> {
        for overlay in overlays.entries() {
            let Some(identity) = self.by_uri.get(overlay.uri()) else {
                return Err(AcceptedProfileCandidateError::UnknownOverlayUri {
                    uri: overlay.uri().to_owned(),
                });
            };
            if identity != overlay.logical_identity() {
                return Err(AcceptedProfileCandidateError::OverlayIdentity {
                    uri: overlay.uri().to_owned(),
                    expected: Box::new(identity.clone()),
                    actual: Box::new(overlay.logical_identity().clone()),
                });
            }
        }
        Ok(())
    }

    fn finish(
        self,
        world: ProjectSymbolWorldId,
        symbol_revision: ProjectSymbolRevision,
        all_source_revision: SourceSetRevision,
    ) -> AcceptedSourceDocuments {
        AcceptedSourceDocuments {
            world,
            symbol_revision,
            all_source_revision,
            by_identity: self.by_identity,
            by_uri: self.by_uri,
        }
    }
}

impl AcceptedSourceDocuments {
    pub const fn world(&self) -> &ProjectSymbolWorldId {
        &self.world
    }

    pub const fn symbol_revision(&self) -> &ProjectSymbolRevision {
        &self.symbol_revision
    }

    pub const fn all_source_revision(&self) -> SourceSetRevision {
        self.all_source_revision
    }

    pub fn get(&self, identity: &SourceDocumentIdentity) -> Option<&AcceptedSourceDocument> {
        self.by_identity.get(identity)
    }

    pub fn by_uri(&self, uri: &Uri) -> Option<&AcceptedSourceDocument> {
        self.by_uri
            .get(uri.as_str())
            .and_then(|identity| self.by_identity.get(identity))
    }

    pub fn documents(&self) -> impl ExactSizeIterator<Item = &AcceptedSourceDocument> {
        self.by_identity.values()
    }
}

impl AcceptedSourceDocument {
    pub const fn document(&self) -> &Arc<SourceDocument> {
        &self.document
    }

    pub const fn locator(&self) -> &AcceptedSourceLocator {
        &self.locator
    }

    pub const fn ownership(&self) -> AcceptedSourceOwnership {
        self.ownership
    }

    pub const fn access(&self) -> AcceptedSourceAccess {
        self.access
    }

    pub const fn line_index(&self) -> &LineIndex {
        &self.line_index
    }
}

impl AcceptedSourceLocator {
    pub fn uri(&self) -> Option<&Uri> {
        match self {
            Self::File { uri, .. } | Self::Uri { uri } => Some(uri),
            Self::Unavailable => None,
        }
    }

    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::File { path, .. } => Some(path),
            Self::Uri { .. } | Self::Unavailable => None,
        }
    }
}

impl AcceptedProfileCandidate {
    pub fn try_new(
        profile: AcceptedProfileKey,
        world: Arc<RegisteredSemanticWorld>,
        source_seeds: Vec<AcceptedSourceDocumentSeed>,
        overlays: AcceptedOverlaySet,
    ) -> Result<Self, AcceptedProfileCandidateError> {
        let symbols = world.symbols();
        let environment = world.environment();
        let index = world.character_definition_index();
        if environment.world() != symbols.world()
            || index.world() != symbols.world()
            || environment.symbol_revision() != symbols.revision()
            || index.symbol_revision() != symbols.revision()
        {
            return Err(AcceptedProfileCandidateError::WorldInvariant);
        }
        let source_world = symbols.world().clone();
        let source_symbol_revision = *symbols.revision();
        let mut sources = AcceptedSourceRegistryBuilder::default();
        for seed in source_seeds {
            sources.insert(seed)?;
        }
        let actual = sources.validate_index(index)?;
        sources.validate_overlays(&overlays)?;
        Ok(Self {
            profile,
            world,
            sources: Arc::new(sources.finish(source_world, source_symbol_revision, actual)),
            overlays,
        })
    }

    pub const fn profile(&self) -> &AcceptedProfileKey {
        &self.profile
    }

    pub const fn world(&self) -> &Arc<RegisteredSemanticWorld> {
        &self.world
    }

    pub const fn sources(&self) -> &Arc<AcceptedSourceDocuments> {
        &self.sources
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
    character_references:
        Mutex<Option<(CharacterReferenceCacheKey, Arc<CharacterReferenceInventory>)>>,
    character_definitions:
        Mutex<Option<(CharacterDefinitionCacheKey, CharacterDefinitionQueryResult)>>,
    #[cfg(test)]
    entries: Mutex<Vec<(String, String)>>,
    #[cfg(test)]
    hits: AtomicU64,
}

impl ProfileSemanticCaches {
    fn clear(&self) {
        self.character_references
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        self.character_definitions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        #[cfg(test)]
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        #[cfg(test)]
        self.hits.store(0, Ordering::Release);
    }
}

/// Exact identity of one request-scoped character-reference inventory.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct CharacterReferenceCacheKey {
    profile: AcceptedProfileKey,
    generation: AcceptedEnvironmentGeneration,
    world: ProjectSymbolWorldId,
    symbol_revision: ProjectSymbolRevision,
    document: SourceDocumentIdentity,
    module: CanonicalModulePath,
    syntax_snapshot: Option<SourceSnapshotId>,
    lsp_version: Option<i32>,
}

impl CharacterReferenceCacheKey {
    #[allow(
        clippy::too_many_arguments,
        reason = "the cache key deliberately carries every independent freshness identity"
    )]
    pub(crate) fn new(
        profile: AcceptedProfileKey,
        generation: AcceptedEnvironmentGeneration,
        world: ProjectSymbolWorldId,
        symbol_revision: ProjectSymbolRevision,
        document: SourceDocumentIdentity,
        module: CanonicalModulePath,
        syntax_snapshot: Option<SourceSnapshotId>,
        lsp_version: Option<i32>,
    ) -> Self {
        Self {
            profile,
            generation,
            world,
            symbol_revision,
            document,
            module,
            syntax_snapshot,
            lsp_version,
        }
    }
}

/// Exact identity of one Sans-I/O character-definition query result.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct CharacterDefinitionCacheKey {
    references: CharacterReferenceCacheKey,
    index_source_revision: SourceSetRevision,
    cursor: usize,
}

impl CharacterDefinitionCacheKey {
    pub(crate) const fn new(
        references: CharacterReferenceCacheKey,
        index_source_revision: SourceSetRevision,
        cursor: usize,
    ) -> Self {
        Self {
            references,
            index_source_revision,
            cursor,
        }
    }
}

/// One immutable registered world plus its fresh generation-owned cache namespace.
#[derive(Debug)]
pub struct AcceptedProfileEnvironment {
    generation: AcceptedEnvironmentGeneration,
    profile: AcceptedProfileKey,
    world: Arc<RegisteredSemanticWorld>,
    sources: Arc<AcceptedSourceDocuments>,
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

    pub const fn sources(&self) -> &Arc<AcceptedSourceDocuments> {
        &self.sources
    }

    pub const fn overlays(&self) -> &AcceptedOverlaySet {
        &self.overlays
    }

    pub(crate) fn cached_character_references(
        &self,
        key: &CharacterReferenceCacheKey,
    ) -> Option<Arc<CharacterReferenceInventory>> {
        self.caches
            .character_references
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .filter(|(candidate, _)| candidate == key)
            .map(|(_, inventory)| Arc::clone(inventory))
    }

    pub(crate) fn cache_character_references(
        &self,
        key: CharacterReferenceCacheKey,
        inventory: Arc<CharacterReferenceInventory>,
    ) {
        self.caches
            .character_references
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .replace((key, inventory));
    }

    pub(crate) fn cached_character_definition(
        &self,
        key: &CharacterDefinitionCacheKey,
    ) -> Option<CharacterDefinitionQueryResult> {
        self.caches
            .character_definitions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .filter(|(candidate, _)| candidate == key)
            .map(|(_, result)| result.clone())
    }

    pub(crate) fn cache_character_definition(
        &self,
        key: CharacterDefinitionCacheKey,
        result: CharacterDefinitionQueryResult,
    ) {
        self.caches
            .character_definitions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .replace((key, result));
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
        candidate: AcceptedProfileCandidate,
    ) -> Result<Arc<AcceptedProfileEnvironment>, AcceptedEnvironmentReplaceError> {
        self.replace_accepted_after_admission(candidate, || {})
    }

    fn replace_accepted_after_admission(
        &self,
        candidate: AcceptedProfileCandidate,
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
        let AcceptedProfileCandidate {
            profile,
            world,
            sources,
            overlays,
        } = candidate;
        let candidate = Arc::new(AcceptedProfileEnvironment {
            generation: AcceptedEnvironmentGeneration(generation),
            profile,
            world,
            sources,
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

    fn accepted_candidate(world: Arc<RegisteredSemanticWorld>) -> AcceptedProfileCandidate {
        AcceptedProfileCandidate::try_new(
            AcceptedProfileKey::new(
                "file:///workspace/cache-tests",
                "file:///workspace/cache-tests/arcw.toml",
                "test",
            ),
            world,
            Vec::new(),
            AcceptedOverlaySet::default(),
        )
        .expect("complete candidate")
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
            sources,
            overlays,
        } = accepted_candidate(registered_world());
        let previous = Arc::new(AcceptedProfileEnvironment {
            generation: AcceptedEnvironmentGeneration::for_test(u64::MAX),
            profile,
            world,
            sources,
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
                state.replace_accepted_after_admission(
                    accepted_candidate(registered_world()),
                    || {
                        admitted.wait();
                        release.wait();
                    },
                )
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
