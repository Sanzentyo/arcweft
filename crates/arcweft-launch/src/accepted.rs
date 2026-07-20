//! Immutable accepted manifest product built from one exact source document.

use crate::{
    LaunchProfileSelection,
    decode::{self, DecodedManifest},
    diagnostic::{ManifestDiagnostic, ManifestDiagnosticCode, ManifestReport},
    manifest::ArcweftManifestDocument,
    resolve,
    source_map::{
        ManifestPath, ManifestPathSegment, ManifestRootField, ManifestSourceKey, ManifestSourceMap,
        ManifestSourceSlot, ProfileField,
    },
};
use arcweft_manifest_model::{EntityIdRef, ProfileId};
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

    pub(crate) const fn source_map(&self) -> &ManifestSourceMap {
        &self.source_map
    }

    /// Selects and resolves one profile without I/O or source reparsing.
    pub fn resolve_profile(
        &self,
        selection: LaunchProfileSelection<'_>,
    ) -> Result<resolve::ResolvedLaunchProfile, ManifestReport> {
        resolve::resolve_profile(self, selection)
    }
}

#[cfg(test)]
mod tests {
    use super::{MANIFEST_DECODE_PASSES, SourceBackedManifest};
    use crate::LaunchProfileSelection;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
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
}
