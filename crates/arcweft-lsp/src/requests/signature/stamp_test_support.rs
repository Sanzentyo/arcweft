//! Typed normalization for a correlated accepted-world test fixture.

use arcweft_lang_hir::symbol::ProjectSymbolRevision;

use super::PreparedSignatureRequest;

impl PreparedSignatureRequest {
    /// Aligns the correlated source-set revision so a test can isolate the
    /// subsequent character-digest authority in validation order.
    pub(crate) fn align_symbol_revision_for_stamp_test(&mut self, revision: ProjectSymbolRevision) {
        self.stamp.symbol_revision = revision;
    }
}
