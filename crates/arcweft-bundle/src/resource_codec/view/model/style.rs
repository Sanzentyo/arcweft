//! Thin bundle wrapper around canonical native Style product data.

use crate::resource_codec::types::{CrossSectionRef, ProductSourceRef, SourceRangeRef};
pub use arcweft_view::style::{
    ViewStyleApplicationTarget, ViewStyleAssignOp, ViewStyleDeclaration, ViewStylePatch,
    ViewStylePatchId, ViewStyleProgram, ViewStyleRule, ViewStyleSheet, ViewStyleSheetId,
    ViewStyleSourceId, ViewStyleToken, ViewStyleTokenId,
};
use serde::{Deserialize, Serialize};

/// Product Style section decoded from `ViewStyle`.
///
/// The canonical program owns sheet, token, rule, and patch identities. This
/// wrapper adds only the product identity and cross-section metadata required
/// by an AWFB section.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewStyleResource {
    pub style_program_id: String,
    pub program: ViewStyleProgram,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<ProductSourceRef>,
    pub source_map_refs: Vec<SourceRangeRef>,
    pub adapter_requirements: Vec<CrossSectionRef>,
}

impl ViewStyleResource {
    pub fn sheet(&self, id: &ViewStyleSheetId) -> Option<&ViewStyleSheet> {
        self.program.sheet(id)
    }

    pub fn inline_patch(&self, id: ViewStylePatchId) -> Option<&ViewStylePatch> {
        self.program.patch(id)
    }
}
