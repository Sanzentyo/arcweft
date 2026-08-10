//! Immutable accepted manifest product built from one exact source document.

use crate::{
    LaunchProfileSelection,
    decode::{self, DecodedManifest},
    diagnostic::{ManifestDiagnostic, ManifestDiagnosticCode, ManifestReport},
    manifest::ArcweftManifestDocument,
    resolve,
    source_map::{
        ContentUnitField, ManifestPath, ManifestPathSegment, ManifestRootField, ManifestSourceKey,
        ManifestSourceMap, ManifestSourceSlot, ManifestTokenPath, ManifestTokenSlot,
        ProfileContentField, ProfileField,
    },
};
use arcweft_manifest_model::{ContentUnitId, EntityIdRef, ProfileId};
use arcweft_source::{SourceDocument, SourceSpan};
use std::sync::Arc;

#[cfg(test)]
thread_local! {
    static MANIFEST_DECODE_PASSES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// One accepted manifest and every typed product derived from its sole parse.
#[derive(Clone, Debug)]
pub struct SourceBackedManifest {
    document: Arc<SourceDocument>,
    manifest: ArcweftManifestDocument,
    source_map: ManifestSourceMap,
}

/// Exact source locations for one manifest content-root occurrence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentRootOccurrenceSource {
    value: SourceSpan,
    selection: SourceSpan,
}

/// Exact source locations for one manifest content-unit declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentUnitManifestSource {
    unit_key: SourceSpan,
    table: SourceSpan,
    visibility: SourceSpan,
    demand: SourceSpan,
}

/// Exact source locations for one selected profile content policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileContentManifestSource {
    unit_key: SourceSpan,
    table: SourceSpan,
    residency: SourceSpan,
    placement: SourceSpan,
    compression: SourceSpan,
}

impl SourceBackedManifest {
    /// Parses and validates one immutable document exactly once.
    pub fn decode(document: Arc<SourceDocument>) -> Result<Self, ManifestReport> {
        #[cfg(test)]
        MANIFEST_DECODE_PASSES.with(|passes| passes.set(passes.get().saturating_add(1)));

        let DecodedManifest {
            manifest,
            source_map,
        } = decode::decode(Arc::clone(&document))?;
        if !Arc::ptr_eq(&document, source_map.document())
            || document.identity() != source_map.document().identity()
        {
            return Err(ManifestReport::single(ManifestDiagnostic::new(
                ManifestDiagnosticCode::TomlSyntax,
                "accepted manifest products do not share the decoded source document",
                document.start_span(),
            )));
        }
        Ok(Self {
            document,
            manifest,
            source_map,
        })
    }

    pub const fn document(&self) -> &Arc<SourceDocument> {
        &self.document
    }

    pub const fn manifest(&self) -> &ArcweftManifestDocument {
        &self.manifest
    }

    pub fn resource_type_manifest_span(&self) -> Option<&SourceSpan> {
        self.source_map.get(&ManifestSourceKey {
            path: ManifestPath::new([ManifestPathSegment::Root(
                ManifestRootField::ResourceTypeManifest,
            )]),
            slot: ManifestSourceSlot::ScalarValue,
        })
    }

    /// Returns the exact authored value span of one profile's selected entry.
    pub fn profile_entry_span(&self, profile: &ProfileId) -> Option<&SourceSpan> {
        self.source_map.get(&ManifestSourceKey {
            path: ManifestPath::new([
                ManifestPathSegment::Root(ManifestRootField::Profiles),
                ManifestPathSegment::Profile(profile.clone()),
                ManifestPathSegment::ProfileField(ProfileField::Entry),
            ]),
            slot: ManifestSourceSlot::ScalarValue,
        })
    }

    /// Iterates every authored profile entry with its exact accepted source span.
    pub fn profile_entries(&self) -> impl Iterator<Item = (&ProfileId, &EntityIdRef, &SourceSpan)> {
        self.manifest
            .profiles
            .iter()
            .filter_map(|(profile_id, profile)| {
                Some((
                    profile_id,
                    profile.entry.as_ref()?,
                    self.profile_entry_span(profile_id)?,
                ))
            })
    }

    /// Projects one accepted content unit's exact source locations without reparsing TOML.
    pub fn content_unit_source(&self, unit: &ContentUnitId) -> Option<ContentUnitManifestSource> {
        let base = content_unit_path(unit, []);
        Some(ContentUnitManifestSource {
            unit_key: self
                .source_map
                .get(&ManifestSourceKey {
                    path: base.clone(),
                    slot: ManifestSourceSlot::MapKey,
                })?
                .clone(),
            table: self
                .source_map
                .get(&ManifestSourceKey {
                    path: base,
                    slot: ManifestSourceSlot::TableHeader,
                })?
                .clone(),
            visibility: self.content_unit_field_span(unit, ContentUnitField::Visibility)?,
            demand: self.content_unit_field_span(unit, ContentUnitField::Demand)?,
        })
    }

    /// Projects the value and string-content span of one accepted content root.
    pub fn content_root_source(
        &self,
        unit: &ContentUnitId,
        root_index: usize,
    ) -> Option<ContentRootOccurrenceSource> {
        let root_index = u32::try_from(root_index).ok()?;
        let path = content_unit_path(
            unit,
            [
                ManifestPathSegment::ContentUnitField(ContentUnitField::Roots),
                ManifestPathSegment::Index(root_index),
            ],
        );
        Some(ContentRootOccurrenceSource {
            value: self
                .source_map
                .get(&ManifestSourceKey {
                    path: path.clone(),
                    slot: ManifestSourceSlot::ArrayElement { index: root_index },
                })?
                .clone(),
            selection: self
                .source_map
                .get(&ManifestSourceKey {
                    path,
                    slot: ManifestSourceSlot::StringContent,
                })?
                .clone(),
        })
    }

    /// Projects one selected profile content policy's exact source locations.
    pub fn selected_profile_content_source(
        &self,
        profile: &ProfileId,
        unit: &ContentUnitId,
    ) -> Option<ProfileContentManifestSource> {
        let base = profile_content_path(profile, unit, []);
        Some(ProfileContentManifestSource {
            unit_key: self
                .source_map
                .get(&ManifestSourceKey {
                    path: base.clone(),
                    slot: ManifestSourceSlot::MapKey,
                })?
                .clone(),
            table: self
                .source_map
                .get(&ManifestSourceKey {
                    path: base,
                    slot: ManifestSourceSlot::TableHeader,
                })?
                .clone(),
            residency: self.profile_content_field_span(
                profile,
                unit,
                ProfileContentField::Residency,
            )?,
            placement: self.profile_content_field_span(
                profile,
                unit,
                ProfileContentField::Placement,
            )?,
            compression: self.profile_content_field_span(
                profile,
                unit,
                ProfileContentField::Compression,
            )?,
        })
    }

    /// Returns one exact token span from this accepted manifest revision.
    pub fn manifest_token_span(
        &self,
        path: &ManifestTokenPath,
        slot: ManifestTokenSlot,
    ) -> Option<&SourceSpan> {
        self.source_map.get(&path.source_key(slot)?)
    }

    pub(crate) const fn source_map(&self) -> &ManifestSourceMap {
        &self.source_map
    }

    fn content_unit_field_span(
        &self,
        unit: &ContentUnitId,
        field: ContentUnitField,
    ) -> Option<SourceSpan> {
        self.source_map
            .get(&ManifestSourceKey {
                path: content_unit_path(unit, [ManifestPathSegment::ContentUnitField(field)]),
                slot: ManifestSourceSlot::ScalarValue,
            })
            .cloned()
    }

    fn profile_content_field_span(
        &self,
        profile: &ProfileId,
        unit: &ContentUnitId,
        field: ProfileContentField,
    ) -> Option<SourceSpan> {
        self.source_map
            .get(&ManifestSourceKey {
                path: profile_content_path(
                    profile,
                    unit,
                    [ManifestPathSegment::ProfileContentField(field)],
                ),
                slot: ManifestSourceSlot::ScalarValue,
            })
            .cloned()
    }

    /// Selects and resolves one profile without I/O or source reparsing.
    pub fn resolve_profile(
        &self,
        selection: LaunchProfileSelection<'_>,
    ) -> Result<resolve::ResolvedLaunchProfile, ManifestReport> {
        resolve::resolve_profile(self, selection)
    }
}

impl ContentRootOccurrenceSource {
    pub const fn value(&self) -> &SourceSpan {
        &self.value
    }

    pub const fn selection(&self) -> &SourceSpan {
        &self.selection
    }
}

impl ContentUnitManifestSource {
    pub const fn unit_key(&self) -> &SourceSpan {
        &self.unit_key
    }

    pub const fn table(&self) -> &SourceSpan {
        &self.table
    }

    pub const fn visibility(&self) -> &SourceSpan {
        &self.visibility
    }

    pub const fn demand(&self) -> &SourceSpan {
        &self.demand
    }
}

impl ProfileContentManifestSource {
    pub const fn unit_key(&self) -> &SourceSpan {
        &self.unit_key
    }

    pub const fn table(&self) -> &SourceSpan {
        &self.table
    }

    pub const fn residency(&self) -> &SourceSpan {
        &self.residency
    }

    pub const fn placement(&self) -> &SourceSpan {
        &self.placement
    }

    pub const fn compression(&self) -> &SourceSpan {
        &self.compression
    }
}

fn content_unit_path(
    unit: &ContentUnitId,
    tail: impl IntoIterator<Item = ManifestPathSegment>,
) -> ManifestPath {
    ManifestPath::new(
        [
            ManifestPathSegment::Root(ManifestRootField::ContentUnits),
            ManifestPathSegment::ContentUnit(unit.clone()),
        ]
        .into_iter()
        .chain(tail)
        .collect::<Vec<_>>(),
    )
}

fn profile_content_path(
    profile: &ProfileId,
    unit: &ContentUnitId,
    tail: impl IntoIterator<Item = ManifestPathSegment>,
) -> ManifestPath {
    ManifestPath::new(
        [
            ManifestPathSegment::Root(ManifestRootField::Profiles),
            ManifestPathSegment::Profile(profile.clone()),
            ManifestPathSegment::ProfileField(ProfileField::Content),
            ManifestPathSegment::ProfileContent(unit.clone()),
        ]
        .into_iter()
        .chain(tail)
        .collect::<Vec<_>>(),
    )
}

#[cfg(test)]
mod tests {
    use super::{MANIFEST_DECODE_PASSES, SourceBackedManifest};
    use crate::{LaunchProfileSelection, ManifestTokenPath, ManifestTokenSlot};
    use arcweft_manifest_model::{ContentUnitId, ProfileId};
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};
    use std::sync::Arc;

    #[test]
    fn accepted_products_retain_the_exact_supplied_document_arc() {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("accepted-manifest").expect("document id"),
                SourceName::Memory,
                "schema = 1\n[package]\nid = \"org.arcweft.test\"\nversion = \"1.0.0\"\n",
            )
            .expect("source document"),
        );
        let accepted = SourceBackedManifest::decode(Arc::clone(&document)).expect("manifest");

        assert!(Arc::ptr_eq(&document, accepted.document()));
        assert!(Arc::ptr_eq(
            accepted.document(),
            accepted.source_map().document()
        ));
        assert_eq!(
            accepted.document().identity(),
            accepted.source_map().document().identity()
        );
        assert_eq!(accepted.manifest().package.id.as_str(), "org.arcweft.test");
    }

    #[test]
    fn accepted_consumers_reuse_one_manifest_decode() {
        MANIFEST_DECODE_PASSES.with(|passes| passes.set(0));
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("single-decode-manifest").expect("document id"),
                SourceName::Memory,
                r#"schema = 1
[package]
id = "org.arcweft.test"
version = "1.0.0"
[profiles.dev]
kind = "game"
source = "src/main.arcw"
entry = "@entry.game"
[profiles.release]
kind = "game"
source = "src/main.arcw"
entry = "@entry.game"
"#,
            )
            .expect("source document"),
        );

        let accepted = SourceBackedManifest::decode(document).expect("accepted manifest");
        accepted
            .resolve_profile(LaunchProfileSelection::Explicit("dev"))
            .expect("development profile");
        accepted
            .resolve_profile(LaunchProfileSelection::Explicit("release"))
            .expect("release profile");
        assert_eq!(accepted.profile_entries().count(), 2);
        assert_eq!(
            MANIFEST_DECODE_PASSES.with(std::cell::Cell::get),
            1,
            "accepted profile and source-map consumers must not reparse the manifest"
        );
    }

    #[test]
    fn content_accessors_project_exact_existing_source_map_spans() {
        let source = r#"schema = 1
[package]
id = "org.arcweft.test"
version = "1.0.0"
[content-units.cast]
roots = ["@character.alice", '@view.dialogue']
visibility = "private"
demand = "optional"
[profiles.dev]
kind = "game"
source = "src/main.arcw"
[profiles.dev.content.cast]
residency = "startup"
placement = "embedded"
compression = "none"
"#;
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("content-source-manifest").unwrap(),
                SourceName::Memory,
                source,
            )
            .unwrap(),
        );
        let accepted = SourceBackedManifest::decode(document).unwrap();
        let unit = ContentUnitId::new("cast").unwrap();
        let profile = ProfileId::new("dev").unwrap();

        let unit_source = accepted.content_unit_source(&unit).unwrap();
        assert_eq!(span_text(source, unit_source.unit_key()), "cast");
        assert_eq!(
            span_text(source, unit_source.table()),
            "[content-units.cast]"
        );
        assert_eq!(span_text(source, unit_source.visibility()), "\"private\"");
        assert_eq!(span_text(source, unit_source.demand()), "\"optional\"");

        let first = accepted.content_root_source(&unit, 0).unwrap();
        assert_eq!(span_text(source, first.value()), "\"@character.alice\"");
        assert_eq!(span_text(source, first.selection()), "@character.alice");
        let second = accepted.content_root_source(&unit, 1).unwrap();
        assert_eq!(span_text(source, second.value()), "'@view.dialogue'");
        assert_eq!(span_text(source, second.selection()), "@view.dialogue");
        assert!(accepted.content_root_source(&unit, 2).is_none());

        let policy = accepted
            .selected_profile_content_source(&profile, &unit)
            .unwrap();
        assert_eq!(span_text(source, policy.unit_key()), "cast");
        assert_eq!(
            span_text(source, policy.table()),
            "[profiles.dev.content.cast]"
        );
        assert_eq!(span_text(source, policy.residency()), "\"startup\"");
        assert_eq!(span_text(source, policy.placement()), "\"embedded\"");
        assert_eq!(span_text(source, policy.compression()), "\"none\"");
    }

    fn span_text<'a>(source: &'a str, span: &arcweft_source::SourceSpan) -> &'a str {
        &source[span.range().start()..span.range().end()]
    }

    #[test]
    fn character_name_token_paths_are_bound_to_the_accepted_document_revision() {
        let source = r#"schema = 1
[package]
id = "org.arcweft.test"
version = "1.0.0"
[profiles.dev]
kind = "game"
source = "src/main.arcw"
[profiles.dev.localization.character_names]
active = "ja-JP"
fallbacks = ["en", "fr"]
"#;
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("localization-manifest").expect("document id"),
                SourceName::Memory,
                source,
            )
            .expect("source document"),
        );
        let accepted =
            SourceBackedManifest::decode(Arc::clone(&document)).expect("accepted manifest");
        let profile = ProfileId::new("dev").expect("profile id");

        for (path, slot, expected) in [
            (
                ManifestTokenPath::ProfileCharacterNamesTable {
                    profile: profile.clone(),
                },
                ManifestTokenSlot::TableHeader,
                "[profiles.dev.localization.character_names]",
            ),
            (
                ManifestTokenPath::ProfileCharacterNamesActive {
                    profile: profile.clone(),
                },
                ManifestTokenSlot::Value,
                "\"ja-JP\"",
            ),
            (
                ManifestTokenPath::ProfileCharacterNamesFallback {
                    profile,
                    ordinal: 1,
                },
                ManifestTokenSlot::Value,
                "\"fr\"",
            ),
        ] {
            let span = accepted
                .manifest_token_span(&path, slot)
                .expect("manifest token span");
            let start = source.find(expected).expect("fixture substring");
            assert_eq!(
                span.range(),
                arcweft_source::SourceRange::new(start, start + expected.len())
            );
            assert_eq!(span.source(), document.identity());
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn dialogue_token_paths_publish_the_exact_accepted_document_spans() {
        let layout = std::iter::repeat_n("0", 32).collect::<Vec<_>>().join(", ");
        let style_element = format!("{{ layout = [{layout}], value = {{ Record = [] }} }}");
        let styles_value = format!("[{style_element}]");
        let source = format!(
            r#"schema = 1
[package]
id = "org.arcweft.test"
version = "1.0.0"
[profiles.dev]
kind = "game"
source = "src/main.arcw"
[profiles.dev.dialogue]
view = "view.dialogue"
style = "style.dialogue"
[profiles.dev.dialogue.inline-failure]
kind = "fallback"
[profiles.dev.dialogue.inline-failure.fallback]
kind = "text"
text = "[missing]"
[profiles.dev.dialogue.inline-failure.fallback.style]
kind = "apply"
styles = {styles_value}
"#
        );
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("dialogue-manifest").expect("document id"),
                SourceName::Memory,
                source.clone(),
            )
            .expect("source document"),
        );
        let accepted =
            SourceBackedManifest::decode(Arc::clone(&document)).expect("accepted manifest");
        let profile = ProfileId::new("dev").expect("profile id");
        let range = |needle: &str, occurrence: usize| {
            let start = source
                .match_indices(needle)
                .nth(occurrence)
                .map(|(start, _)| start)
                .expect("fixture token");
            SourceRange::new(start, start + needle.len())
        };

        let cases = vec![
            (
                ManifestTokenPath::ProfileTable {
                    profile: profile.clone(),
                },
                ManifestTokenSlot::TableHeader,
                range("[profiles.dev]", 0),
            ),
            (
                ManifestTokenPath::ProfileDialogueTable {
                    profile: profile.clone(),
                },
                ManifestTokenSlot::TableHeader,
                range("[profiles.dev.dialogue]", 0),
            ),
            (
                ManifestTokenPath::ProfileDialogueView {
                    profile: profile.clone(),
                },
                ManifestTokenSlot::FieldKey,
                range("view", 0),
            ),
            (
                ManifestTokenPath::ProfileDialogueView {
                    profile: profile.clone(),
                },
                ManifestTokenSlot::Value,
                range("\"view.dialogue\"", 0),
            ),
            (
                ManifestTokenPath::ProfileDialogueStyle {
                    profile: profile.clone(),
                },
                ManifestTokenSlot::FieldKey,
                range("style", 0),
            ),
            (
                ManifestTokenPath::ProfileDialogueStyle {
                    profile: profile.clone(),
                },
                ManifestTokenSlot::Value,
                range("\"style.dialogue\"", 0),
            ),
            (
                ManifestTokenPath::ProfileDialogueInlineFailureTable {
                    profile: profile.clone(),
                },
                ManifestTokenSlot::TableHeader,
                range("[profiles.dev.dialogue.inline-failure]", 0),
            ),
            (
                ManifestTokenPath::ProfileDialogueInlineFailureKind {
                    profile: profile.clone(),
                },
                ManifestTokenSlot::Value,
                range("\"fallback\"", 0),
            ),
            (
                ManifestTokenPath::ProfileDialogueInlineFallbackTable {
                    profile: profile.clone(),
                },
                ManifestTokenSlot::TableHeader,
                range("[profiles.dev.dialogue.inline-failure.fallback]", 0),
            ),
            (
                ManifestTokenPath::ProfileDialogueInlineFallbackKind {
                    profile: profile.clone(),
                },
                ManifestTokenSlot::Value,
                range("\"text\"", 0),
            ),
            (
                ManifestTokenPath::ProfileDialogueInlineFallbackText {
                    profile: profile.clone(),
                },
                ManifestTokenSlot::Value,
                range("\"[missing]\"", 0),
            ),
            (
                ManifestTokenPath::ProfileDialogueInlineFallbackStyleTable {
                    profile: profile.clone(),
                },
                ManifestTokenSlot::TableHeader,
                range("[profiles.dev.dialogue.inline-failure.fallback.style]", 0),
            ),
            (
                ManifestTokenPath::ProfileDialogueInlineFallbackStyleKind {
                    profile: profile.clone(),
                },
                ManifestTokenSlot::Value,
                range("\"apply\"", 0),
            ),
            (
                ManifestTokenPath::ProfileDialogueInlineFallbackStyles {
                    profile: profile.clone(),
                },
                ManifestTokenSlot::Value,
                range(&styles_value, 0),
            ),
            (
                ManifestTokenPath::ProfileDialogueInlineFallbackStyleElement {
                    profile,
                    ordinal: 0,
                },
                ManifestTokenSlot::Value,
                range(&style_element, 0),
            ),
        ];

        for (path, slot, expected_range) in cases {
            let span = accepted
                .manifest_token_span(&path, slot)
                .unwrap_or_else(|| panic!("dialogue manifest token span for {path:?} / {slot:?}"));
            assert_eq!(span.range(), expected_range);
            assert_eq!(span.source(), document.identity());
        }

        assert!(
            accepted
                .manifest_token_span(
                    &ManifestTokenPath::ProfileTable {
                        profile: ProfileId::new("dev").expect("profile id"),
                    },
                    ManifestTokenSlot::Value,
                )
                .is_none()
        );
        assert!(
            accepted
                .manifest_token_span(
                    &ManifestTokenPath::ProfileDialogueView {
                        profile: ProfileId::new("dev").expect("profile id"),
                    },
                    ManifestTokenSlot::TableHeader,
                )
                .is_none()
        );
        assert!(
            accepted
                .manifest_token_span(
                    &ManifestTokenPath::ProfileDialogueInlineFallbackStyleElement {
                        profile: ProfileId::new("dev").expect("profile id"),
                        ordinal: 0,
                    },
                    ManifestTokenSlot::FieldKey,
                )
                .is_none()
        );
    }
}
