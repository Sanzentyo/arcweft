use crate::documents::DocumentSnapshot;
use arcweft_verify_lsp::inferred_id_inlay_hints_with_mapper;
use lsp_types::InlayHint;

/// Computes Arcweft inferred-ID inlay hints for one source snapshot.
pub fn hints(document: &DocumentSnapshot) -> Vec<InlayHint> {
    inferred_id_inlay_hints_with_mapper(document.text(), document.line_index())
}
