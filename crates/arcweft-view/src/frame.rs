//! UI frame output produced after retained fragment layout and semantics.

use crate::{
    DisplayList, LayoutResults, UiError, UiHandlerRouteTable, UiSemanticFragment, UiStyleTable,
    ViewFragment,
};

/// Per-layer UI output ready for host-side frame commit validation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiLayerOutput {
    display: DisplayList,
    semantics: UiSemanticFragment,
    handlers: UiHandlerRouteTable,
    styles: UiStyleTable,
}

impl UiLayerOutput {
    pub fn new(display: DisplayList, semantics: UiSemanticFragment) -> Self {
        Self {
            display,
            semantics,
            handlers: UiHandlerRouteTable::default(),
            styles: UiStyleTable::default(),
        }
    }

    pub fn from_fragment(
        fragment: &ViewFragment,
        layouts: &LayoutResults,
        semantics: UiSemanticFragment,
    ) -> Result<Self, UiError> {
        Self::from_fragment_with_styles(fragment, layouts, semantics, UiStyleTable::default())
    }

    pub fn from_fragment_with_styles(
        fragment: &ViewFragment,
        layouts: &LayoutResults,
        semantics: UiSemanticFragment,
        styles: UiStyleTable,
    ) -> Result<Self, UiError> {
        let display = DisplayList::from_fragment(fragment, layouts)?;
        let handlers = UiHandlerRouteTable::from_fragment(fragment, &semantics)?;
        Ok(Self {
            display,
            semantics,
            handlers,
            styles,
        })
    }

    pub const fn display(&self) -> &DisplayList {
        &self.display
    }

    pub const fn semantics(&self) -> &UiSemanticFragment {
        &self.semantics
    }

    pub const fn handlers(&self) -> &UiHandlerRouteTable {
        &self.handlers
    }

    pub const fn styles(&self) -> &UiStyleTable {
        &self.styles
    }

    pub fn into_parts(self) -> (DisplayList, UiSemanticFragment) {
        (self.display, self.semantics)
    }

    pub fn into_frame_parts(
        self,
    ) -> (
        DisplayList,
        UiSemanticFragment,
        UiHandlerRouteTable,
        UiStyleTable,
    ) {
        (self.display, self.semantics, self.handlers, self.styles)
    }
}
