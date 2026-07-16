//! Environment field-usage tracking for cascade selection and paint projection.

use crate::style::ComputedViewStyle;
use arcweft_presentation::appearance::{
    PresentationEnvironmentField, PresentationEnvironmentFieldSet,
};

/// Environment fields consulted by cascade selection and paint projection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ViewStyleEnvironmentUsage {
    selection: PresentationEnvironmentFieldSet,
    projection: PresentationEnvironmentFieldSet,
}

impl ViewStyleEnvironmentUsage {
    pub const fn new(
        selection: PresentationEnvironmentFieldSet,
        projection: PresentationEnvironmentFieldSet,
    ) -> Self {
        Self {
            selection,
            projection,
        }
    }

    pub const fn selection(self) -> PresentationEnvironmentFieldSet {
        self.selection
    }

    pub const fn projection(self) -> PresentationEnvironmentFieldSet {
        self.projection
    }

    pub const fn all(self) -> PresentationEnvironmentFieldSet {
        self.selection.union(self.projection)
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            selection: self.selection.union(other.selection),
            projection: self.projection.union(other.projection),
        }
    }
}

pub(super) fn projection_environment_usage(
    computed: &ComputedViewStyle,
) -> PresentationEnvironmentFieldSet {
    if computed
        .properties()
        .any(|(_, property)| property.value().uses_system_color())
    {
        PresentationEnvironmentFieldSet::from_field(PresentationEnvironmentField::ColorScheme)
    } else {
        PresentationEnvironmentFieldSet::NONE
    }
}
