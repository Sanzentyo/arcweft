//! Typed normalization for a correlated accepted-world test fixture.

use std::sync::Arc;

use arcweft_lang_hir::symbol::ProjectSymbolRevision;
use arcweft_source::SourceDocument;

use super::PreparedSignatureRequest;

impl PreparedSignatureRequest {
    /// Aligns the correlated source-set revision so a test can isolate the
    /// subsequent character-digest authority in validation order.
    pub(crate) fn align_symbol_revision_for_stamp_test(&mut self, revision: ProjectSymbolRevision) {
        self.stamp.symbol_revision = revision;
    }

    /// Aligns the correlated protocol version so a test can isolate a changed
    /// document identity without fabricating a second syntax lineage.
    pub(crate) fn align_lsp_version_for_stamp_test(&mut self, version: i32) {
        self.stamp.lsp_version = version;
    }

    /// Replaces only the request-side document allocation so the freshness
    /// matrix can exercise exact lease identity without constructing an
    /// impossible accepted project whose HIR and parsed source disagree.
    pub(crate) fn replace_accepted_document_for_stamp_test(
        &mut self,
        document: Arc<SourceDocument>,
    ) {
        self.stamp.accepted_document = document;
    }
}
