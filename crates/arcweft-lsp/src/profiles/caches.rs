//! Generation-owned semantic caches and their exact freshness keys.

use std::sync::{Arc, Mutex, PoisonError};

#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};

use arcweft_lang_hir::symbol::{ProjectSymbolRevision, ProjectSymbolWorldId};
use arcweft_lang_sema::character_definition::{
    CharacterDefinitionQueryResult, CharacterDefinitionRequestBudget,
    CharacterDefinitionResourceError, CharacterDefinitionWorkKind, CharacterDefinitionWorkReceipt,
    CharacterReferenceInventory,
};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::{SourceDocumentIdentity, SourceSetRevision, identity::SourceSnapshotId};

use super::state::{AcceptedEnvironmentGeneration, AcceptedProfileKey};

/// Broad semantic caches owned exclusively by one accepted generation.
#[derive(Debug, Default)]
pub(crate) struct ProfileSemanticCaches {
    character_references: Mutex<
        Option<(
            CharacterReferenceCacheKey,
            Arc<CharacterReferenceCacheEntry>,
        )>,
    >,
    character_definitions: Mutex<
        Option<(
            CharacterDefinitionCacheKey,
            Arc<CharacterDefinitionCacheEntry>,
        )>,
    >,
    #[cfg(test)]
    entries: Mutex<Vec<(String, String)>>,
    #[cfg(test)]
    hits: AtomicU64,
}

#[derive(Debug)]
struct CharacterReferenceCacheEntry {
    inventory: Arc<CharacterReferenceInventory>,
    work: CharacterDefinitionWorkReceipt,
}

#[derive(Debug)]
struct CharacterDefinitionCacheEntry {
    result: Arc<CharacterDefinitionQueryResult>,
    work: CharacterDefinitionWorkReceipt,
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
    lsp_version: i32,
}

/// Exact identity of one Sans-I/O character-definition query result.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct CharacterDefinitionCacheKey {
    references: CharacterReferenceCacheKey,
    index_source_revision: SourceSetRevision,
    cursor: usize,
}

impl ProfileSemanticCaches {
    pub(crate) fn clear(&self) {
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

    pub(crate) fn cached_character_references(
        &self,
        key: &CharacterReferenceCacheKey,
        budget: &mut CharacterDefinitionRequestBudget,
    ) -> Result<Option<Arc<CharacterReferenceInventory>>, CharacterDefinitionResourceError> {
        let entry = {
            let cache = self
                .character_references
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            budget.charge(CharacterDefinitionWorkKind::IdentityCheck)?;
            cache
                .as_ref()
                .filter(|(candidate, _)| candidate == key)
                .map(|(_, entry)| Arc::clone(entry))
        };
        let Some(entry) = entry else {
            return Ok(None);
        };
        budget.replay(&entry.work)?;
        Ok(Some(Arc::clone(&entry.inventory)))
    }

    pub(crate) fn cache_character_references(
        &self,
        key: CharacterReferenceCacheKey,
        inventory: Arc<CharacterReferenceInventory>,
        work: CharacterDefinitionWorkReceipt,
    ) {
        self.character_references
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .replace((
                key,
                Arc::new(CharacterReferenceCacheEntry { inventory, work }),
            ));
    }

    pub(crate) fn cached_character_definition(
        &self,
        key: &CharacterDefinitionCacheKey,
        budget: &mut CharacterDefinitionRequestBudget,
    ) -> Result<Option<Arc<CharacterDefinitionQueryResult>>, CharacterDefinitionResourceError> {
        let entry = {
            let cache = self
                .character_definitions
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            budget.charge(CharacterDefinitionWorkKind::IdentityCheck)?;
            cache
                .as_ref()
                .filter(|(candidate, _)| candidate == key)
                .map(|(_, entry)| Arc::clone(entry))
        };
        let Some(entry) = entry else {
            return Ok(None);
        };
        budget.replay(&entry.work)?;
        Ok(Some(Arc::clone(&entry.result)))
    }

    pub(crate) fn cache_character_definition(
        &self,
        key: CharacterDefinitionCacheKey,
        result: Arc<CharacterDefinitionQueryResult>,
        work: CharacterDefinitionWorkReceipt,
    ) {
        self.character_definitions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .replace((
                key,
                Arc::new(CharacterDefinitionCacheEntry { result, work }),
            ));
    }

    #[cfg(test)]
    pub(crate) fn insert_for_test(&self, key: &str, value: &str) {
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((key.to_owned(), value.to_owned()));
        self.hits.fetch_add(1, Ordering::AcqRel);
    }

    #[cfg(test)]
    pub(crate) fn snapshot_for_test(&self) -> (Vec<(String, String)>, u64) {
        (
            self.entries
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone(),
            self.hits.load(Ordering::Acquire),
        )
    }

    #[cfg(test)]
    pub(crate) fn character_entries_for_test(&self) -> (bool, bool) {
        (
            self.character_references
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .is_some(),
            self.character_definitions
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .is_some(),
        )
    }
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
        lsp_version: i32,
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
