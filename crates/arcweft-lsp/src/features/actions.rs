use crate::diagnostics::DocumentAnalysis;
use crate::documents::DocumentSnapshot;
use arcweft_verify_lsp::source_code_actions;
use lsp_types::{CodeAction, Uri};

/// Computes code actions for one open Arcweft document.
pub fn actions(
    uri: &Uri,
    document: &DocumentSnapshot,
    _analysis: &DocumentAnalysis,
) -> Vec<CodeAction> {
    source_code_actions(uri, document.text())
}
