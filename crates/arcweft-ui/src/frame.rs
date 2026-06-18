//! UI frame output produced after retained fragment layout and semantics.

use crate::{DisplayList, LayoutResults, UiError, UiSemanticFragment, ViewFragment};

/// Per-layer UI output ready for host-side frame commit validation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiLayerOutput {
    display: DisplayList,
    semantics: UiSemanticFragment,
}

impl UiLayerOutput {
    pub fn new(display: DisplayList, semantics: UiSemanticFragment) -> Self {
        Self { display, semantics }
    }

    pub fn from_fragment(
        fragment: &ViewFragment,
        layouts: &LayoutResults,
        semantics: UiSemanticFragment,
    ) -> Result<Self, UiError> {
        DisplayList::from_fragment(fragment, layouts).map(|display| Self { display, semantics })
    }

    pub const fn display(&self) -> &DisplayList {
        &self.display
    }

    pub const fn semantics(&self) -> &UiSemanticFragment {
        &self.semantics
    }

    pub fn into_parts(self) -> (DisplayList, UiSemanticFragment) {
        (self.display, self.semantics)
    }
}
