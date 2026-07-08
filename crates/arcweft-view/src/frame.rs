//! View frame output produced after retained fragment layout and semantics.

use crate::{
    DisplayList, LayoutResults, ViewError, ViewFragment, ViewHandlerRouteTable,
    ViewSemanticFragment, ViewStyleTable,
};

/// Per-layer View output ready for host-side frame commit validation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewLayerOutput {
    display: DisplayList,
    semantics: ViewSemanticFragment,
    handlers: ViewHandlerRouteTable,
    styles: ViewStyleTable,
}

impl ViewLayerOutput {
    pub fn new(display: DisplayList, semantics: ViewSemanticFragment) -> Self {
        Self {
            display,
            semantics,
            handlers: ViewHandlerRouteTable::default(),
            styles: ViewStyleTable::default(),
        }
    }

    pub fn from_fragment(
        fragment: &ViewFragment,
        layouts: &LayoutResults,
        semantics: ViewSemanticFragment,
    ) -> Result<Self, ViewError> {
        Self::from_fragment_with_styles(fragment, layouts, semantics, ViewStyleTable::default())
    }

    pub fn from_fragment_with_styles(
        fragment: &ViewFragment,
        layouts: &LayoutResults,
        semantics: ViewSemanticFragment,
        styles: ViewStyleTable,
    ) -> Result<Self, ViewError> {
        let display = DisplayList::from_fragment(fragment, layouts)?;
        let handlers = ViewHandlerRouteTable::from_fragment(fragment, &semantics)?;
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

    pub const fn semantics(&self) -> &ViewSemanticFragment {
        &self.semantics
    }

    pub const fn handlers(&self) -> &ViewHandlerRouteTable {
        &self.handlers
    }

    pub const fn styles(&self) -> &ViewStyleTable {
        &self.styles
    }

    pub fn into_parts(self) -> (DisplayList, ViewSemanticFragment) {
        (self.display, self.semantics)
    }

    pub fn into_frame_parts(
        self,
    ) -> (
        DisplayList,
        ViewSemanticFragment,
        ViewHandlerRouteTable,
        ViewStyleTable,
    ) {
        (self.display, self.semantics, self.handlers, self.styles)
    }
}
