//! Selected-profile Character-name locale policy decoding.

use super::super::{append, reject_scalar_member, required_field};
use super::{ProfileContext, record_optional_profile_table, record_profile_field};
use crate::{
    decode::{index::ManifestIndex, value},
    diagnostic::{ManifestDiagnostic, ManifestDiagnosticCode, ManifestRelatedSpan},
    source_map::{
        CharacterNamesField, LocalizationField, ManifestPath, ManifestPathSegment,
        ManifestRootField, ManifestSourceKey, ProfileField,
    },
};
use arcweft_id::LocaleTag;
use arcweft_manifest_model::{
    CharacterNameLocalePolicySpec, CharacterNameLocalePolicySpecError,
    MAX_PROFILE_CHARACTER_NAME_FALLBACKS, ProfileId, ProfileLocalizationSpec,
};
use arcweft_source::{SourceDocument, SourceSpan};
use std::collections::{BTreeMap, btree_map::Entry};

// Locale validation, source-map publication, duplicate evidence, and accepted
// policy construction intentionally remain in one source-ordered transaction.
#[allow(clippy::too_many_lines)]
pub(super) fn decode_localization(
    document: &SourceDocument,
    index: &ManifestIndex,
    context: ProfileContext<'_>,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> ProfileLocalizationSpec {
    let localization_base = append(context.base, "localization");
    if let Some(field) = index.field_by_path(&localization_base) {
        record_profile_field(
            source_entries,
            context.id,
            ProfileField::Localization,
            field,
        );
    }
    if reject_scalar_member(
        index,
        &localization_base,
        "profile localization policy",
        diagnostics,
    ) {
        return ProfileLocalizationSpec::default();
    }

    let Some(localization_anchor) = record_optional_profile_table(
        index,
        context,
        &localization_base,
        ProfileField::Localization,
        source_entries,
    ) else {
        return ProfileLocalizationSpec::default();
    };

    let character_names_base = append(&localization_base, "character_names");
    if reject_scalar_member(
        index,
        &character_names_base,
        "Character-name localization policy",
        diagnostics,
    ) {
        return ProfileLocalizationSpec::default();
    }
    let Some(character_names_anchor) = record_character_names_table(
        index,
        context.id,
        &character_names_base,
        &localization_anchor,
        source_entries,
    ) else {
        return ProfileLocalizationSpec::default();
    };

    let active_path = append(&character_names_base, "active");
    let Some(active_field) = required_field(
        index,
        &active_path,
        &character_names_anchor,
        "Character-name active locale",
        diagnostics,
    ) else {
        return ProfileLocalizationSpec::default();
    };
    value::record_field(
        source_entries,
        character_names_path(
            context.id,
            [ManifestPathSegment::CharacterNames(
                CharacterNamesField::Active,
            )],
        ),
        active_field,
    );
    let Some(active) = decode_locale(active_field, "Character-name active locale", diagnostics)
    else {
        return ProfileLocalizationSpec::default();
    };

    let fallbacks_path = append(&character_names_base, "fallbacks");
    let Some(fallbacks_field) = index.field_by_path(&fallbacks_path) else {
        return ProfileLocalizationSpec::new(
            CharacterNameLocalePolicySpec::try_new(active, Vec::new()).ok(),
        );
    };
    value::record_field(
        source_entries,
        character_names_path(
            context.id,
            [ManifestPathSegment::CharacterNames(
                CharacterNamesField::Fallbacks,
            )],
        ),
        fallbacks_field,
    );
    let Some(elements) = value::array_elements(
        document,
        fallbacks_field,
        "Character-name fallback locales",
        diagnostics,
    ) else {
        return ProfileLocalizationSpec::default();
    };

    let mut fallbacks =
        Vec::with_capacity(elements.len().min(MAX_PROFILE_CHARACTER_NAME_FALLBACKS));
    let mut first_occurrences = BTreeMap::<LocaleTag, (u16, SourceSpan)>::new();
    let mut policy_failed = false;
    let mut ordinal = 0_u16;
    for (index, (node, span)) in elements.into_iter().enumerate() {
        let Some(source_index) = value::bounded_array_index(index, &span, diagnostics) else {
            policy_failed = true;
            continue;
        };
        value::record_array_element(
            source_entries,
            character_names_path(
                context.id,
                [ManifestPathSegment::CharacterNames(
                    CharacterNamesField::Fallbacks,
                )],
            ),
            source_index,
            span.clone(),
        );
        if index >= MAX_PROFILE_CHARACTER_NAME_FALLBACKS {
            if index == MAX_PROFILE_CHARACTER_NAME_FALLBACKS {
                diagnostics.push(value::diagnostic(
                    ManifestDiagnosticCode::CharacterNameFallbackLimit,
                    format!(
                        "Character-name fallback count exceeds maximum \
                         {MAX_PROFILE_CHARACTER_NAME_FALLBACKS}"
                    ),
                    span,
                    Vec::new(),
                ));
            }
            policy_failed = true;
            continue;
        }

        let Some(raw) = value::node_text(
            &node,
            &span,
            ManifestDiagnosticCode::CharacterNameLocaleInvalid,
            "Character-name fallback locale",
            diagnostics,
        ) else {
            policy_failed = true;
            ordinal = ordinal.saturating_add(1);
            continue;
        };
        let fallback = match LocaleTag::try_new(&raw) {
            Ok(fallback) => fallback,
            Err(error) => {
                diagnostics.push(value::diagnostic(
                    ManifestDiagnosticCode::CharacterNameLocaleInvalid,
                    format!("Character-name fallback locale `{raw}` is invalid: {error}"),
                    span,
                    Vec::new(),
                ));
                policy_failed = true;
                ordinal = ordinal.saturating_add(1);
                continue;
            }
        };

        if fallback == active {
            diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::CharacterNameFallbackDuplicate,
                format!("Character-name fallback `{fallback}` repeats the active locale"),
                span,
                vec![ManifestRelatedSpan::new(
                    "active locale declared here",
                    active_field.value_span.clone(),
                )],
            ));
            policy_failed = true;
            ordinal = ordinal.saturating_add(1);
            continue;
        }

        match first_occurrences.entry(fallback.clone()) {
            Entry::Vacant(entry) => {
                entry.insert((ordinal, span));
                fallbacks.push(fallback);
            }
            Entry::Occupied(first) => {
                diagnostics.push(value::diagnostic(
                    ManifestDiagnosticCode::CharacterNameFallbackDuplicate,
                    format!(
                        "Character-name fallback `{fallback}` at ordinal {ordinal} duplicates ordinal {}",
                        first.get().0
                    ),
                    span,
                    vec![ManifestRelatedSpan::new(
                        "first fallback declared here",
                        first.get().1.clone(),
                    )],
                ));
                policy_failed = true;
            }
        }
        ordinal = ordinal.saturating_add(1);
    }

    if policy_failed {
        return ProfileLocalizationSpec::default();
    }
    match CharacterNameLocalePolicySpec::try_new(active, fallbacks) {
        Ok(policy) => ProfileLocalizationSpec::new(Some(policy)),
        Err(error) => {
            diagnostics.push(policy_invariant_diagnostic(&error, character_names_anchor));
            ProfileLocalizationSpec::default()
        }
    }
}

fn decode_locale(
    field: &crate::decode::index::IndexedField,
    expectation: &str,
    diagnostics: &mut Vec<ManifestDiagnostic>,
) -> Option<LocaleTag> {
    let raw = value::text(
        field,
        ManifestDiagnosticCode::CharacterNameLocaleInvalid,
        expectation,
        diagnostics,
    )?;
    LocaleTag::try_new(&raw).map_or_else(
        |error| {
            diagnostics.push(value::diagnostic(
                ManifestDiagnosticCode::CharacterNameLocaleInvalid,
                format!("{expectation} `{raw}` is invalid: {error}"),
                field.value_span.clone(),
                Vec::new(),
            ));
            None
        },
        Some,
    )
}

fn record_character_names_table(
    index: &ManifestIndex,
    profile: &ProfileId,
    base: &[String],
    fallback_anchor: &SourceSpan,
    source_entries: &mut BTreeMap<ManifestSourceKey, SourceSpan>,
) -> Option<SourceSpan> {
    let path = character_names_path(profile, []);
    if let Some(table) = index.table_by_path(base) {
        value::record_table(source_entries, path, table);
        return Some(table.header_span.clone());
    }
    index
        .fields
        .keys()
        .any(|candidate| candidate.starts_with(base) && candidate.len() > base.len())
        .then(|| fallback_anchor.clone())
}

fn character_names_path(
    profile: &ProfileId,
    tail: impl IntoIterator<Item = ManifestPathSegment>,
) -> ManifestPath {
    let mut segments = vec![
        ManifestPathSegment::Root(ManifestRootField::Profiles),
        ManifestPathSegment::Profile(profile.clone()),
        ManifestPathSegment::ProfileField(ProfileField::Localization),
        ManifestPathSegment::Localization(LocalizationField::CharacterNames),
    ];
    segments.extend(tail);
    ManifestPath::new(segments)
}

fn policy_invariant_diagnostic(
    error: &CharacterNameLocalePolicySpecError,
    anchor: SourceSpan,
) -> ManifestDiagnostic {
    let code = match error {
        CharacterNameLocalePolicySpecError::TooManyFallbacks { .. } => {
            ManifestDiagnosticCode::CharacterNameFallbackLimit
        }
        CharacterNameLocalePolicySpecError::DuplicateFallback { .. }
        | CharacterNameLocalePolicySpecError::ActiveRepeated { .. } => {
            ManifestDiagnosticCode::CharacterNameFallbackDuplicate
        }
    };
    value::diagnostic(code, error.to_string(), anchor, Vec::new())
}
