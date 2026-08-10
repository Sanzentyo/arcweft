//! Immutable accepted catalog, canonical identities, and checked lookup.

use super::{
    CharacterDeclarationNameFallback, CharacterDisplayNameEntry, CharacterDisplayNameInput,
    CharacterDisplayNameKey, CharacterDisplayNameKeyError, CharacterDisplayNameValue,
    CharacterNameLocale, CharacterNameLocalePolicy, CharacterNameLocalePolicyError,
    CharacterNameSourceLocale, CharacterPresentationLimitKind,
    CharacterPresentationLocalePolicyDigest, CharacterPresentationSemanticDigest,
    LocalizedCharacterDisplayName, LocalizedCharacterDisplayNameInput,
    limits::{
        MAX_CATALOG_CHARACTERS, MAX_CATALOG_LOCALIZED_ENTRIES, MAX_CHARACTER_ID_BYTES,
        MAX_LOCALIZED_NAMES_PER_CHARACTER,
    },
    transcript::{locale_policy_digest, semantic_digest},
};
use crate::id::CharacterId;
use core::num::NonZeroU64;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use thiserror::Error;

/// Semantic role of one accepted Character presentation record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CharacterPresentationRole {
    Character,
    Narrator,
}

/// Typed catalog input for one Character before generated keys are attached.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDisplayNameRecordInput {
    character: CharacterId,
    role: CharacterPresentationRole,
    source_locale: Option<CharacterNameSourceLocale>,
    base: Option<CharacterDisplayNameInput>,
    localized: Vec<LocalizedCharacterDisplayNameInput>,
    declaration_fallback: Option<CharacterDisplayNameValue>,
}

/// Canonically ordered accepted display-name metadata for one Character.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDisplayNameRecord {
    character: CharacterId,
    role: CharacterPresentationRole,
    source_locale: Option<CharacterNameSourceLocale>,
    base: Option<CharacterDisplayNameEntry>,
    localized: Box<[LocalizedCharacterDisplayName]>,
    declaration_fallback: Option<CharacterDeclarationNameFallback>,
}

/// Typed input consumed by the sole accepted catalog constructor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterPresentationCatalogInput {
    policy: CharacterNameLocalePolicy,
    records: Vec<CharacterDisplayNameRecordInput>,
}

/// Canonical accepted catalog data independent of process publication order.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterPresentationCatalogData {
    policy: CharacterNameLocalePolicy,
    records: Box<[CharacterDisplayNameRecord]>,
    semantic_digest: CharacterPresentationSemanticDigest,
    locale_policy_digest: CharacterPresentationLocalePolicyDigest,
}

/// Monotonic process-local accepted catalog revision.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterPresentationCatalogRevision(NonZeroU64);

/// Artifact identities and process-local publication order for one generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CharacterPresentationCatalogGeneration {
    revision: CharacterPresentationCatalogRevision,
    semantic_digest: CharacterPresentationSemanticDigest,
    locale_policy_digest: CharacterPresentationLocalePolicyDigest,
}

/// An immutable catalog generation ready for transactional publication.
#[derive(Clone, Debug)]
pub struct AcceptedCharacterPresentationCatalog {
    revision: CharacterPresentationCatalogRevision,
    data: Arc<CharacterPresentationCatalogData>,
}

/// The accepted fallback step that resolved a display name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CharacterDisplayNameResolutionSource {
    ActiveLocale,
    ProjectFallback { ordinal: u8 },
    CharacterSourceLocale,
    Base,
    DeclarationName,
}

/// One deterministic Character display-name lookup result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedCharacterDisplayName {
    value: String,
    key: Option<CharacterDisplayNameKey>,
    source: CharacterDisplayNameResolutionSource,
    hidden: bool,
}

/// Accepted-catalog construction failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CharacterPresentationCatalogError {
    #[error("{kind} count {observed} exceeds maximum {maximum}")]
    Limit {
        kind: CharacterPresentationLimitKind,
        observed: u64,
        maximum: u64,
    },
    #[error("arithmetic overflow while computing {operation}")]
    ArithmeticOverflow { operation: &'static str },
    #[error("Character `{character}` at ordinal {duplicate} duplicates accepted ordinal {first}")]
    DuplicateCharacter {
        character: CharacterId,
        first: u32,
        duplicate: u32,
    },
    #[error(
        "locale `{locale}` for Character `{character}` at ordinal {duplicate} duplicates ordinal {first}"
    )]
    DuplicateLocale {
        character: CharacterId,
        locale: CharacterNameLocale,
        first: u32,
        duplicate: u32,
    },
    #[error("generated Character display-name key `{key}` is duplicated")]
    DuplicateGeneratedKey { key: CharacterDisplayNameKey },
    #[error("Character `{character}` source locale `{locale}` has no exact localized entry")]
    SourceLocaleWithoutEntry {
        character: CharacterId,
        locale: CharacterNameLocale,
    },
    #[error("Character `{character}` has no accepted display-name result")]
    MissingAnyAcceptedName { character: CharacterId },
    #[error("narrator `{character}` requires explicit base display-name metadata")]
    NarratorRequiresBase { character: CharacterId },
    #[error("narrator `{character}` may not have a declaration-name fallback")]
    NarratorForbidsDeclarationFallback { character: CharacterId },
    #[error(transparent)]
    InvalidFallbackPolicy(#[from] CharacterNameLocalePolicyError),
    #[error(transparent)]
    InvalidGeneratedKey(#[from] CharacterDisplayNameKeyError),
}

/// Deterministic lookup failure against an accepted catalog.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CharacterDisplayNameLookupError {
    #[error("Character `{character}` is not present in the accepted presentation catalog")]
    UnknownCharacter { character: CharacterId },
    #[error("Character `{character}` has no display name accepted by the active locale chain")]
    MissingAcceptedName {
        character: CharacterId,
        active: CharacterNameLocale,
        attempted_locales: Box<[CharacterNameLocale]>,
        has_base: bool,
        has_declaration: bool,
    },
}

/// Failure to advance a process-local catalog revision.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CharacterPresentationCatalogRevisionError {
    #[error("Character presentation catalog revision is exhausted")]
    RevisionExhausted,
}

/// Failure to prepare an immutable catalog publication candidate.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CharacterPresentationCatalogPublicationError {
    #[error("Character presentation catalog revision is exhausted")]
    RevisionExhausted,
}

impl CharacterDisplayNameRecordInput {
    pub fn try_new(
        character: CharacterId,
        role: CharacterPresentationRole,
        source_locale: Option<CharacterNameSourceLocale>,
        base: Option<CharacterDisplayNameInput>,
        localized: Vec<LocalizedCharacterDisplayNameInput>,
        declaration_fallback: Option<CharacterDisplayNameValue>,
    ) -> Result<Self, CharacterPresentationCatalogError> {
        validate_character_id_limit(&character)?;
        validate_localized_count(&character, localized.len())?;

        let mut locale_ordinals = BTreeMap::new();
        for (ordinal, localized_name) in localized.iter().enumerate() {
            let ordinal = checked_u32(ordinal, "localized display-name ordinal")?;
            if let Some(first) = locale_ordinals.insert(localized_name.locale().clone(), ordinal) {
                return Err(CharacterPresentationCatalogError::DuplicateLocale {
                    character,
                    locale: localized_name.locale().clone(),
                    first,
                    duplicate: ordinal,
                });
            }
        }

        if let Some(source_locale) = &source_locale
            && !locale_ordinals.contains_key(source_locale.locale())
        {
            return Err(
                CharacterPresentationCatalogError::SourceLocaleWithoutEntry {
                    character,
                    locale: source_locale.locale().clone(),
                },
            );
        }

        match role {
            CharacterPresentationRole::Character => {
                if base.is_none() && localized.is_empty() && declaration_fallback.is_none() {
                    return Err(CharacterPresentationCatalogError::MissingAnyAcceptedName {
                        character,
                    });
                }
            }
            CharacterPresentationRole::Narrator => {
                if base.is_none() {
                    return Err(CharacterPresentationCatalogError::NarratorRequiresBase {
                        character,
                    });
                }
                if declaration_fallback.is_some() {
                    return Err(
                        CharacterPresentationCatalogError::NarratorForbidsDeclarationFallback {
                            character,
                        },
                    );
                }
            }
        }

        Ok(Self {
            character,
            role,
            source_locale,
            base,
            localized,
            declaration_fallback,
        })
    }

    #[must_use]
    pub const fn character(&self) -> &CharacterId {
        &self.character
    }

    #[must_use]
    pub const fn role(&self) -> CharacterPresentationRole {
        self.role
    }

    #[must_use]
    pub const fn source_locale(&self) -> Option<&CharacterNameSourceLocale> {
        self.source_locale.as_ref()
    }

    #[must_use]
    pub const fn base(&self) -> Option<&CharacterDisplayNameInput> {
        self.base.as_ref()
    }

    #[must_use]
    pub fn localized(&self) -> &[LocalizedCharacterDisplayNameInput] {
        &self.localized
    }

    #[must_use]
    pub const fn declaration_fallback(&self) -> Option<&CharacterDisplayNameValue> {
        self.declaration_fallback.as_ref()
    }
}

impl CharacterDisplayNameRecord {
    #[must_use]
    pub const fn character(&self) -> &CharacterId {
        &self.character
    }

    #[must_use]
    pub const fn role(&self) -> CharacterPresentationRole {
        self.role
    }

    #[must_use]
    pub const fn source_locale(&self) -> Option<&CharacterNameSourceLocale> {
        self.source_locale.as_ref()
    }

    #[must_use]
    pub const fn base(&self) -> Option<&CharacterDisplayNameEntry> {
        self.base.as_ref()
    }

    #[must_use]
    pub fn localized(&self) -> &[LocalizedCharacterDisplayName] {
        &self.localized
    }

    #[must_use]
    pub const fn declaration_fallback(&self) -> Option<&CharacterDeclarationNameFallback> {
        self.declaration_fallback.as_ref()
    }

    fn localized_entry(&self, locale: &CharacterNameLocale) -> Option<&CharacterDisplayNameEntry> {
        self.localized
            .binary_search_by(|candidate| candidate.locale().cmp(locale))
            .ok()
            .map(|index| self.localized[index].entry())
    }
}

impl CharacterPresentationCatalogInput {
    pub fn try_new(
        policy: CharacterNameLocalePolicy,
        records: Vec<CharacterDisplayNameRecordInput>,
    ) -> Result<Self, CharacterPresentationCatalogError> {
        validate_catalog_counts(&records)?;

        let mut character_ordinals = BTreeMap::new();
        for (ordinal, record) in records.iter().enumerate() {
            let ordinal = checked_u32(ordinal, "Character record ordinal")?;
            if let Some(first) = character_ordinals.insert(record.character().clone(), ordinal) {
                return Err(CharacterPresentationCatalogError::DuplicateCharacter {
                    character: record.character().clone(),
                    first,
                    duplicate: ordinal,
                });
            }
        }

        Ok(Self { policy, records })
    }

    #[must_use]
    pub const fn policy(&self) -> &CharacterNameLocalePolicy {
        &self.policy
    }

    #[must_use]
    pub fn records(&self) -> &[CharacterDisplayNameRecordInput] {
        &self.records
    }
}

impl CharacterPresentationCatalogData {
    pub fn try_from_inputs(
        input: CharacterPresentationCatalogInput,
    ) -> Result<Self, CharacterPresentationCatalogError> {
        let CharacterPresentationCatalogInput {
            policy,
            mut records,
        } = input;
        records.sort_by(|left, right| left.character.cmp(&right.character));

        let mut generated_keys = BTreeSet::new();
        let mut accepted_records = Vec::with_capacity(records.len());
        for record in records {
            let character = record.character;
            let base = record
                .base
                .map(|entry| accept_entry(&character, None, entry, &mut generated_keys))
                .transpose()?;

            let mut localized_inputs = record.localized;
            localized_inputs.sort_by(|left, right| left.locale().cmp(right.locale()));
            let mut localized = Vec::with_capacity(localized_inputs.len());
            for input in localized_inputs {
                let (locale, entry) = (input.locale().clone(), input.entry().clone());
                let entry = accept_entry(&character, Some(&locale), entry, &mut generated_keys)?;
                localized.push(LocalizedCharacterDisplayName::new(locale, entry));
            }

            let declaration_fallback = record
                .declaration_fallback
                .map(|value| {
                    let key = CharacterDisplayNameKey::for_declaration(&character)?;
                    insert_generated_key(&mut generated_keys, &key)?;
                    Ok::<_, CharacterPresentationCatalogError>(
                        CharacterDeclarationNameFallback::new(key, value),
                    )
                })
                .transpose()?;

            accepted_records.push(CharacterDisplayNameRecord {
                character,
                role: record.role,
                source_locale: record.source_locale,
                base,
                localized: localized.into_boxed_slice(),
                declaration_fallback,
            });
        }

        let semantic_digest = semantic_digest(&accepted_records)?;
        let locale_policy_digest = locale_policy_digest(&policy)?;
        Ok(Self {
            policy,
            records: accepted_records.into_boxed_slice(),
            semantic_digest,
            locale_policy_digest,
        })
    }

    #[must_use]
    pub const fn policy(&self) -> &CharacterNameLocalePolicy {
        &self.policy
    }

    #[must_use]
    pub fn records(&self) -> &[CharacterDisplayNameRecord] {
        &self.records
    }

    #[must_use]
    pub const fn semantic_digest(&self) -> CharacterPresentationSemanticDigest {
        self.semantic_digest
    }

    #[must_use]
    pub const fn locale_policy_digest(&self) -> CharacterPresentationLocalePolicyDigest {
        self.locale_policy_digest
    }

    pub fn record(
        &self,
        id: &CharacterId,
    ) -> Result<&CharacterDisplayNameRecord, CharacterDisplayNameLookupError> {
        self.records
            .binary_search_by(|record| record.character().cmp(id))
            .ok()
            .map(|index| &self.records[index])
            .ok_or_else(|| CharacterDisplayNameLookupError::UnknownCharacter {
                character: id.clone(),
            })
    }

    pub fn resolve(
        &self,
        id: &CharacterId,
        active: &CharacterNameLocale,
    ) -> Result<ResolvedCharacterDisplayName, CharacterDisplayNameLookupError> {
        let record = self.record(id)?;
        let mut seen = BTreeSet::new();
        let mut attempted = Vec::new();

        if let Some(resolved) = probe_locale(
            record,
            active,
            CharacterDisplayNameResolutionSource::ActiveLocale,
            &mut seen,
            &mut attempted,
        ) {
            return Ok(resolved);
        }

        let mut ordinal = 0_u8;
        for fallback in self.policy.fallbacks() {
            if let Some(resolved) = probe_locale(
                record,
                fallback.locale(),
                CharacterDisplayNameResolutionSource::ProjectFallback { ordinal },
                &mut seen,
                &mut attempted,
            ) {
                return Ok(resolved);
            }
            ordinal = ordinal.saturating_add(1);
        }

        if let Some(source_locale) = record.source_locale()
            && let Some(resolved) = probe_locale(
                record,
                source_locale.locale(),
                CharacterDisplayNameResolutionSource::CharacterSourceLocale,
                &mut seen,
                &mut attempted,
            )
        {
            return Ok(resolved);
        }

        if let Some(base) = record.base() {
            return Ok(resolved_entry(
                base,
                CharacterDisplayNameResolutionSource::Base,
            ));
        }
        if let Some(declaration) = record.declaration_fallback() {
            return Ok(ResolvedCharacterDisplayName {
                value: declaration.value().as_str().to_owned(),
                key: Some(declaration.key().clone()),
                source: CharacterDisplayNameResolutionSource::DeclarationName,
                hidden: false,
            });
        }

        Err(CharacterDisplayNameLookupError::MissingAcceptedName {
            character: id.clone(),
            active: active.clone(),
            attempted_locales: attempted.into_boxed_slice(),
            has_base: record.base().is_some(),
            has_declaration: record.declaration_fallback().is_some(),
        })
    }
}

impl CharacterPresentationCatalogRevision {
    pub const INITIAL: Self = Self(NonZeroU64::MIN);

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }

    pub fn checked_next(self) -> Result<Self, CharacterPresentationCatalogRevisionError> {
        self.get()
            .checked_add(1)
            .and_then(NonZeroU64::new)
            .map(Self)
            .ok_or(CharacterPresentationCatalogRevisionError::RevisionExhausted)
    }
}

impl CharacterPresentationCatalogGeneration {
    #[must_use]
    pub const fn new(
        revision: CharacterPresentationCatalogRevision,
        semantic_digest: CharacterPresentationSemanticDigest,
        locale_policy_digest: CharacterPresentationLocalePolicyDigest,
    ) -> Self {
        Self {
            revision,
            semantic_digest,
            locale_policy_digest,
        }
    }

    #[must_use]
    pub const fn revision(self) -> CharacterPresentationCatalogRevision {
        self.revision
    }

    #[must_use]
    pub const fn semantic_digest(self) -> CharacterPresentationSemanticDigest {
        self.semantic_digest
    }

    #[must_use]
    pub const fn locale_policy_digest(self) -> CharacterPresentationLocalePolicyDigest {
        self.locale_policy_digest
    }
}

impl AcceptedCharacterPresentationCatalog {
    pub fn publish_initial(
        data: CharacterPresentationCatalogData,
    ) -> Result<Self, CharacterPresentationCatalogPublicationError> {
        Ok(Self {
            revision: CharacterPresentationCatalogRevision::INITIAL,
            data: Arc::new(data),
        })
    }

    #[must_use]
    pub const fn revision(&self) -> CharacterPresentationCatalogRevision {
        self.revision
    }

    #[must_use]
    pub fn data(&self) -> &CharacterPresentationCatalogData {
        self.data.as_ref()
    }

    #[must_use]
    pub fn generation(&self) -> CharacterPresentationCatalogGeneration {
        CharacterPresentationCatalogGeneration::new(
            self.revision,
            self.data.semantic_digest(),
            self.data.locale_policy_digest(),
        )
    }

    pub fn candidate_replacement(
        &self,
        data: CharacterPresentationCatalogData,
    ) -> Result<Self, CharacterPresentationCatalogPublicationError> {
        let revision = self
            .revision
            .checked_next()
            .map_err(|_| CharacterPresentationCatalogPublicationError::RevisionExhausted)?;
        Ok(Self {
            revision,
            data: Arc::new(data),
        })
    }
}

impl ResolvedCharacterDisplayName {
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    #[must_use]
    pub const fn key(&self) -> Option<&CharacterDisplayNameKey> {
        self.key.as_ref()
    }

    #[must_use]
    pub const fn source(&self) -> CharacterDisplayNameResolutionSource {
        self.source
    }

    #[must_use]
    pub const fn is_hidden(&self) -> bool {
        self.hidden
    }
}

fn validate_character_id_limit(
    character: &CharacterId,
) -> Result<(), CharacterPresentationCatalogError> {
    if character.as_str().len() > MAX_CHARACTER_ID_BYTES {
        return Err(CharacterPresentationCatalogError::Limit {
            kind: CharacterPresentationLimitKind::CharacterIdBytes,
            observed: u64::try_from(character.as_str().len()).unwrap_or(u64::MAX),
            maximum: MAX_CHARACTER_ID_BYTES as u64,
        });
    }
    Ok(())
}

fn validate_localized_count(
    character: &CharacterId,
    count: usize,
) -> Result<(), CharacterPresentationCatalogError> {
    if count > MAX_LOCALIZED_NAMES_PER_CHARACTER {
        return Err(CharacterPresentationCatalogError::Limit {
            kind: CharacterPresentationLimitKind::LocalizedEntriesPerCharacter,
            observed: u64::try_from(count).unwrap_or(u64::MAX),
            maximum: MAX_LOCALIZED_NAMES_PER_CHARACTER as u64,
        });
    }
    validate_character_id_limit(character)
}

fn validate_catalog_counts(
    records: &[CharacterDisplayNameRecordInput],
) -> Result<(), CharacterPresentationCatalogError> {
    if records.len() > MAX_CATALOG_CHARACTERS {
        return Err(CharacterPresentationCatalogError::Limit {
            kind: CharacterPresentationLimitKind::Characters,
            observed: u64::try_from(records.len()).unwrap_or(u64::MAX),
            maximum: MAX_CATALOG_CHARACTERS as u64,
        });
    }

    let mut localized_total = 0_usize;
    for record in records {
        validate_localized_count(record.character(), record.localized().len())?;
        localized_total = localized_total
            .checked_add(record.localized().len())
            .ok_or(CharacterPresentationCatalogError::ArithmeticOverflow {
                operation: "total localized entry count",
            })?;
        if localized_total > MAX_CATALOG_LOCALIZED_ENTRIES {
            return Err(CharacterPresentationCatalogError::Limit {
                kind: CharacterPresentationLimitKind::TotalLocalizedEntries,
                observed: u64::try_from(localized_total).unwrap_or(u64::MAX),
                maximum: MAX_CATALOG_LOCALIZED_ENTRIES as u64,
            });
        }
    }
    Ok(())
}

fn accept_entry(
    character: &CharacterId,
    locale: Option<&CharacterNameLocale>,
    input: CharacterDisplayNameInput,
    generated_keys: &mut BTreeSet<CharacterDisplayNameKey>,
) -> Result<CharacterDisplayNameEntry, CharacterPresentationCatalogError> {
    match input {
        CharacterDisplayNameInput::Visible(value) => {
            let key = match locale {
                Some(locale) => CharacterDisplayNameKey::for_locale(character, locale)?,
                None => CharacterDisplayNameKey::for_base(character)?,
            };
            insert_generated_key(generated_keys, &key)?;
            Ok(CharacterDisplayNameEntry::Visible { key, value })
        }
        CharacterDisplayNameInput::Hidden => Ok(CharacterDisplayNameEntry::Hidden),
    }
}

fn insert_generated_key(
    generated_keys: &mut BTreeSet<CharacterDisplayNameKey>,
    key: &CharacterDisplayNameKey,
) -> Result<(), CharacterPresentationCatalogError> {
    if generated_keys.insert(key.clone()) {
        Ok(())
    } else {
        Err(CharacterPresentationCatalogError::DuplicateGeneratedKey { key: key.clone() })
    }
}

fn probe_locale(
    record: &CharacterDisplayNameRecord,
    locale: &CharacterNameLocale,
    source: CharacterDisplayNameResolutionSource,
    seen: &mut BTreeSet<CharacterNameLocale>,
    attempted: &mut Vec<CharacterNameLocale>,
) -> Option<ResolvedCharacterDisplayName> {
    if !seen.insert(locale.clone()) {
        return None;
    }
    attempted.push(locale.clone());
    record
        .localized_entry(locale)
        .map(|entry| resolved_entry(entry, source))
}

fn resolved_entry(
    entry: &CharacterDisplayNameEntry,
    source: CharacterDisplayNameResolutionSource,
) -> ResolvedCharacterDisplayName {
    match entry {
        CharacterDisplayNameEntry::Visible { key, value } => ResolvedCharacterDisplayName {
            value: value.as_str().to_owned(),
            key: Some(key.clone()),
            source,
            hidden: false,
        },
        CharacterDisplayNameEntry::Hidden => ResolvedCharacterDisplayName {
            value: String::new(),
            key: None,
            source,
            hidden: true,
        },
    }
}

fn checked_u32(
    value: usize,
    operation: &'static str,
) -> Result<u32, CharacterPresentationCatalogError> {
    u32::try_from(value)
        .map_err(|_| CharacterPresentationCatalogError::ArithmeticOverflow { operation })
}

#[cfg(test)]
mod revision_tests {
    use super::{CharacterPresentationCatalogRevision, CharacterPresentationCatalogRevisionError};
    use core::num::NonZeroU64;

    #[test]
    fn revision_overflow_is_typed_and_never_wraps() {
        let exhausted = CharacterPresentationCatalogRevision(
            NonZeroU64::new(u64::MAX).expect("u64::MAX is nonzero"),
        );
        assert_eq!(
            exhausted.checked_next(),
            Err(CharacterPresentationCatalogRevisionError::RevisionExhausted)
        );
    }
}
