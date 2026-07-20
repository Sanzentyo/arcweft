//! Canonical digest transcripts for accepted presentation catalog data.

use super::{
    CharacterDisplayNameEntry, CharacterDisplayNameRecord, CharacterNameLocalePolicy,
    CharacterPresentationCatalogError, CharacterPresentationLocalePolicyDigest,
    CharacterPresentationRole, CharacterPresentationSemanticDigest,
};

const SEMANTIC_DIGEST_DOMAIN: &[u8] = b"arcweft.character-presentation.semantic.v1\0";
const LOCALE_POLICY_DIGEST_DOMAIN: &[u8] = b"arcweft.character-presentation.locale-policy.v1\0";

pub(super) fn semantic_digest(
    records: &[CharacterDisplayNameRecord],
) -> Result<CharacterPresentationSemanticDigest, CharacterPresentationCatalogError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(SEMANTIC_DIGEST_DOMAIN);
    put_u32(&mut hasher, records.len(), "semantic Character count")?;

    for record in records {
        put_bytes(
            &mut hasher,
            record.character().as_str().as_bytes(),
            "semantic Character ID bytes",
        )?;
        hasher.update(&[match record.role() {
            CharacterPresentationRole::Character => 1,
            CharacterPresentationRole::Narrator => 2,
        }]);

        if let Some(source_locale) = record.source_locale() {
            hasher.update(&[1]);
            put_bytes(
                &mut hasher,
                source_locale.locale().locale_tag().as_str().as_bytes(),
                "semantic source locale bytes",
            )?;
        } else {
            hasher.update(&[0]);
        }

        put_entry(&mut hasher, record.base(), "semantic base entry")?;
        put_u32(
            &mut hasher,
            record.localized().len(),
            "semantic localized count",
        )?;
        for localized in record.localized() {
            put_bytes(
                &mut hasher,
                localized.locale().locale_tag().as_str().as_bytes(),
                "semantic localized locale bytes",
            )?;
            put_required_entry(&mut hasher, localized.entry(), "semantic localized entry")?;
        }

        if let Some(declaration) = record.declaration_fallback() {
            hasher.update(&[1]);
            put_bytes(
                &mut hasher,
                declaration.key().as_str().as_bytes(),
                "semantic declaration key bytes",
            )?;
            put_bytes(
                &mut hasher,
                declaration.value().as_str().as_bytes(),
                "semantic declaration value bytes",
            )?;
        } else {
            hasher.update(&[0]);
        }
    }

    Ok(CharacterPresentationSemanticDigest::from_bytes(
        *hasher.finalize().as_bytes(),
    ))
}

pub(super) fn locale_policy_digest(
    policy: &CharacterNameLocalePolicy,
) -> Result<CharacterPresentationLocalePolicyDigest, CharacterPresentationCatalogError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(LOCALE_POLICY_DIGEST_DOMAIN);
    put_bytes(
        &mut hasher,
        policy.default_active().locale_tag().as_str().as_bytes(),
        "locale-policy default active bytes",
    )?;
    put_u32(
        &mut hasher,
        policy.fallbacks().len(),
        "locale-policy fallback count",
    )?;
    for fallback in policy.fallbacks() {
        put_bytes(
            &mut hasher,
            fallback.locale().locale_tag().as_str().as_bytes(),
            "locale-policy fallback bytes",
        )?;
    }
    Ok(CharacterPresentationLocalePolicyDigest::from_bytes(
        *hasher.finalize().as_bytes(),
    ))
}

fn put_entry(
    hasher: &mut blake3::Hasher,
    entry: Option<&CharacterDisplayNameEntry>,
    operation: &'static str,
) -> Result<(), CharacterPresentationCatalogError> {
    match entry {
        None => {
            hasher.update(&[0]);
            Ok(())
        }
        Some(entry) => put_required_entry(hasher, entry, operation),
    }
}

fn put_required_entry(
    hasher: &mut blake3::Hasher,
    entry: &CharacterDisplayNameEntry,
    operation: &'static str,
) -> Result<(), CharacterPresentationCatalogError> {
    match entry {
        CharacterDisplayNameEntry::Visible { key, value } => {
            hasher.update(&[1]);
            put_bytes(hasher, key.as_str().as_bytes(), operation)?;
            put_bytes(hasher, value.as_str().as_bytes(), operation)
        }
        CharacterDisplayNameEntry::Hidden => {
            hasher.update(&[2]);
            Ok(())
        }
    }
}

fn put_u32(
    hasher: &mut blake3::Hasher,
    value: usize,
    operation: &'static str,
) -> Result<(), CharacterPresentationCatalogError> {
    let value = u32::try_from(value)
        .map_err(|_| CharacterPresentationCatalogError::ArithmeticOverflow { operation })?;
    hasher.update(&value.to_le_bytes());
    Ok(())
}

fn put_bytes(
    hasher: &mut blake3::Hasher,
    bytes: &[u8],
    operation: &'static str,
) -> Result<(), CharacterPresentationCatalogError> {
    put_u32(hasher, bytes.len(), operation)?;
    hasher.update(bytes);
    Ok(())
}
