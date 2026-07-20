//! Generation-owned semantic caches and their exact freshness keys.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use arcweft_lang_hir::symbol::{ProjectSymbolRevision, ProjectSymbolWorldId};
use arcweft_lang_sema::character_definition::{
    CharacterDefinitionQueryResult, CharacterDefinitionRequestBudget,
    CharacterDefinitionResourceError, CharacterDefinitionWorkKind, CharacterDefinitionWorkReceipt,
    CharacterReferenceInventory,
};
use arcweft_lang_sema::{
    callable::PRODUCTION_CALLABLE_LIMITS,
    registration::{CharacterInventoryDigest, CharacterInventoryRevision},
    signature::SignatureQueryOutcome,
};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::{
    MAX_REGISTRATION_SOURCE_BYTES, SourceDocumentIdentity, SourceSetRevision,
    identity::SourceSnapshotId,
};

use super::state::{AcceptedEnvironmentGeneration, AcceptedProfileKey};

pub(crate) const SIGNATURE_CACHE_CAPACITY: usize = 512;

/// Broad semantic caches owned exclusively by one accepted generation.
#[derive(Debug)]
pub(crate) struct ProfileSemanticCaches {
    signature_help: Mutex<DeterministicLruCache<SignatureCacheKey, CacheableSignatureOutcome>>,
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
}

#[derive(Debug)]
struct DeterministicLruCache<K, V> {
    entries: BTreeMap<K, CacheEntry<V>>,
    access_clock: u64,
    capacity: usize,
    #[cfg(test)]
    hits: u64,
    #[cfg(test)]
    misses: u64,
    #[cfg(test)]
    insertions: u64,
    #[cfg(test)]
    replacements: u64,
    #[cfg(test)]
    evictions: u64,
    #[cfg(test)]
    clock_resets: u64,
    #[cfg(test)]
    poison_recoveries: u64,
}

#[derive(Debug)]
struct CacheEntry<V> {
    value: V,
    last_access: u64,
}

pub(crate) struct SignatureCacheGuard<'a> {
    cache: MutexGuard<'a, DeterministicLruCache<SignatureCacheKey, CacheableSignatureOutcome>>,
}

#[derive(Clone, Debug)]
struct CacheableSignatureOutcome {
    outcome: Arc<SignatureQueryOutcome>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CacheInsertOutcome {
    Inserted,
    Replaced,
    InsertedAfterEviction,
    InsertedAfterClockReset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SignatureCacheInsertion {
    Cached,
    NotCachedUnrepresentable,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SignatureCacheTestSnapshot {
    pub(crate) entries: usize,
    pub(crate) access_clock: u64,
    pub(crate) hits: u64,
    pub(crate) misses: u64,
    pub(crate) insertions: u64,
    pub(crate) replacements: u64,
    pub(crate) evictions: u64,
    pub(crate) clock_resets: u64,
    pub(crate) poison_recoveries: u64,
}

/// Exact typed identity of one cacheable native signature query.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SignatureCacheKey {
    generation: AcceptedEnvironmentGeneration,
    world: ProjectSymbolWorldId,
    symbol_revision: ProjectSymbolRevision,
    character_revision: CharacterInventoryRevision,
    character_digest: CharacterInventoryDigest,
    source: SourceDocumentIdentity,
    lsp_version: Option<i32>,
    byte_offset: usize,
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

impl Default for ProfileSemanticCaches {
    fn default() -> Self {
        Self {
            signature_help: Mutex::new(DeterministicLruCache::new(SIGNATURE_CACHE_CAPACITY)),
            character_references: Mutex::default(),
            character_definitions: Mutex::default(),
        }
    }
}

impl ProfileSemanticCaches {
    pub(crate) fn clear(&self) {
        self.signature_cache().cache.clear();
        self.character_references
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        self.character_definitions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
    }

    pub(crate) fn evict_signature_document(&self, document: &SourceDocumentIdentity) -> usize {
        self.signature_cache()
            .cache
            .remove_where(|key| &key.source == document)
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

    #[cfg(test)]
    pub(crate) fn signature_snapshot_for_test(&self) -> SignatureCacheTestSnapshot {
        self.signature_cache().cache.snapshot()
    }

    #[cfg(test)]
    pub(crate) fn set_signature_access_clock_for_test(&self, value: u64) {
        self.signature_cache().cache.access_clock = value;
    }

    #[cfg(test)]
    pub(crate) fn poison_signature_cache_for_test(&self) {
        let poisoned = std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let _guard = self
                        .signature_help
                        .lock()
                        .expect("signature cache initially healthy");
                    panic!("poison signature cache for recovery test");
                })
                .join()
        });
        assert!(poisoned.is_err(), "cache poison test thread must unwind");
    }

    pub(crate) fn signature_cache(&self) -> SignatureCacheGuard<'_> {
        let cache = match self.signature_help.lock() {
            Ok(cache) => cache,
            Err(poisoned) => {
                let mut cache = poisoned.into_inner();
                cache.clear();
                #[cfg(test)]
                {
                    cache.poison_recoveries = cache.poison_recoveries.saturating_add(1);
                }
                self.signature_help.clear_poison();
                cache
            }
        };
        SignatureCacheGuard { cache }
    }
}

impl SignatureCacheGuard<'_> {
    pub(crate) fn cached(&mut self, key: &SignatureCacheKey) -> Option<Arc<SignatureQueryOutcome>> {
        self.cache.get(key).map(|entry| entry.outcome)
    }

    pub(crate) fn insert(
        &mut self,
        key: SignatureCacheKey,
        outcome: Arc<SignatureQueryOutcome>,
        accepted_source_bytes: u64,
    ) -> SignatureCacheInsertion {
        let Some(entry) = CacheableSignatureOutcome::try_new(&key, outcome, accepted_source_bytes)
        else {
            return SignatureCacheInsertion::NotCachedUnrepresentable;
        };
        let _ = self.cache.insert(key, entry);
        SignatureCacheInsertion::Cached
    }
}

impl CacheableSignatureOutcome {
    fn try_new(
        key: &SignatureCacheKey,
        outcome: Arc<SignatureQueryOutcome>,
        accepted_source_bytes: u64,
    ) -> Option<Self> {
        complete_signature_entry_size(key, outcome.as_ref(), accepted_source_bytes)?;
        Some(Self { outcome })
    }
}

/// Computes a conservative bound for every allocation retained by one entry.
///
/// Result-owned strings and collections originate from accepted project input,
/// the bounded static registry, or charged query work. Charging the combined
/// input bound once per maximum query operation dominates the retained result
/// while keeping every size conversion and addition checked.
fn complete_signature_entry_size(
    key: &SignatureCacheKey,
    outcome: &SignatureQueryOutcome,
    accepted_source_bytes: u64,
) -> Option<usize> {
    let accepted_source_bytes = usize::try_from(accepted_source_bytes).ok()?;
    let static_source_bytes = usize::try_from(MAX_REGISTRATION_SOURCE_BYTES).ok()?;
    let input_bytes = accepted_source_bytes.checked_add(static_source_bytes)?;
    let query_work = usize::try_from(PRODUCTION_CALLABLE_LIMITS.max_query_work()).ok()?;
    let retained_result_bound = input_bytes.checked_mul(query_work)?;
    std::mem::size_of::<SignatureCacheKey>()
        .checked_add(key.checked_dynamic_size()?)?
        .checked_add(std::mem::size_of::<CacheEntry<CacheableSignatureOutcome>>())?
        .checked_add(std::mem::size_of_val(outcome))?
        .checked_add(std::mem::size_of::<usize>().checked_mul(2)?)?
        .checked_add(retained_result_bound)
}

impl<K, V> DeterministicLruCache<K, V>
where
    K: Clone + Ord,
{
    fn new(capacity: usize) -> Self {
        debug_assert!(capacity > 0, "cache capacity must be positive");
        Self {
            entries: BTreeMap::new(),
            access_clock: 0,
            capacity,
            #[cfg(test)]
            hits: 0,
            #[cfg(test)]
            misses: 0,
            #[cfg(test)]
            insertions: 0,
            #[cfg(test)]
            replacements: 0,
            #[cfg(test)]
            evictions: 0,
            #[cfg(test)]
            clock_resets: 0,
            #[cfg(test)]
            poison_recoveries: 0,
        }
    }

    fn get(&mut self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        if !self.entries.contains_key(key) {
            #[cfg(test)]
            {
                self.misses = self.misses.saturating_add(1);
            }
            return None;
        }
        let Some(access) = self.access_clock.checked_add(1) else {
            self.clear_for_clock_overflow();
            #[cfg(test)]
            {
                self.misses = self.misses.saturating_add(1);
            }
            return None;
        };
        self.access_clock = access;
        let entry = self
            .entries
            .get_mut(key)
            .expect("the existing cache key remains present");
        entry.last_access = access;
        let value = entry.value.clone();
        #[cfg(test)]
        {
            self.hits = self.hits.saturating_add(1);
        }
        Some(value)
    }

    fn insert(&mut self, key: K, value: V) -> CacheInsertOutcome {
        let (access, reset) = if let Some(access) = self.access_clock.checked_add(1) {
            (access, false)
        } else {
            self.clear_for_clock_overflow();
            (1, true)
        };
        self.access_clock = access;

        if let Some(entry) = self.entries.get_mut(&key) {
            entry.value = value;
            entry.last_access = access;
            #[cfg(test)]
            {
                self.replacements = self.replacements.saturating_add(1);
            }
            return CacheInsertOutcome::Replaced;
        }

        let evicted = if self.entries.len() == self.capacity {
            let key = self
                .entries
                .iter()
                .min_by(|(left_key, left), (right_key, right)| {
                    left.last_access
                        .cmp(&right.last_access)
                        .then_with(|| left_key.cmp(right_key))
                })
                .map(|(key, _)| key.clone())
                .expect("a full positive-capacity cache has an eviction candidate");
            self.entries.remove(&key);
            #[cfg(test)]
            {
                self.evictions = self.evictions.saturating_add(1);
            }
            true
        } else {
            false
        };
        self.entries.insert(
            key,
            CacheEntry {
                value,
                last_access: access,
            },
        );
        #[cfg(test)]
        {
            self.insertions = self.insertions.saturating_add(1);
        }
        if reset {
            CacheInsertOutcome::InsertedAfterClockReset
        } else if evicted {
            CacheInsertOutcome::InsertedAfterEviction
        } else {
            CacheInsertOutcome::Inserted
        }
    }

    fn remove_where(&mut self, predicate: impl Fn(&K) -> bool) -> usize {
        let keys = self
            .entries
            .keys()
            .filter(|key| predicate(key))
            .cloned()
            .collect::<Vec<_>>();
        let removed = keys.len();
        for key in keys {
            self.entries.remove(&key);
        }
        removed
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.access_clock = 0;
    }

    fn clear_for_clock_overflow(&mut self) {
        self.clear();
        #[cfg(test)]
        {
            self.clock_resets = self.clock_resets.saturating_add(1);
        }
    }

    #[cfg(test)]
    fn snapshot(&self) -> SignatureCacheTestSnapshot {
        SignatureCacheTestSnapshot {
            entries: self.entries.len(),
            access_clock: self.access_clock,
            hits: self.hits,
            misses: self.misses,
            insertions: self.insertions,
            replacements: self.replacements,
            evictions: self.evictions,
            clock_resets: self.clock_resets,
            poison_recoveries: self.poison_recoveries,
        }
    }
}

impl SignatureCacheKey {
    #[allow(
        clippy::too_many_arguments,
        reason = "the key is the exact typed projection of every result-relevant request stamp field"
    )]
    pub(crate) fn new(
        generation: AcceptedEnvironmentGeneration,
        world: ProjectSymbolWorldId,
        symbol_revision: ProjectSymbolRevision,
        character_revision: CharacterInventoryRevision,
        character_digest: CharacterInventoryDigest,
        source: SourceDocumentIdentity,
        lsp_version: Option<i32>,
        byte_offset: usize,
    ) -> Self {
        Self {
            generation,
            world,
            symbol_revision,
            character_revision,
            character_digest,
            source,
            lsp_version,
            byte_offset,
        }
    }

    pub(crate) const fn byte_offset(&self) -> usize {
        self.byte_offset
    }

    fn checked_dynamic_size(&self) -> Option<usize> {
        [
            self.world.package().as_str().len(),
            self.world.root_document().as_str().len(),
            self.world.profile().len(),
            self.source.id().as_str().len(),
        ]
        .into_iter()
        .try_fold(0usize, usize::checked_add)
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

#[cfg(test)]
mod tests;
