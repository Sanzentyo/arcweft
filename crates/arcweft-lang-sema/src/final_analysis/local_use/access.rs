use super::{CheckedLocalValueTransfer, LocalId};
use crate::final_analysis::CheckedPlace;

/// A writable operation does not produce a value carrier. Whole-place
/// assignment initializes the declaration after evaluating its new value;
/// mutation requires the existing value to remain available.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedLocalPlaceMode {
    Assign,
    Mutate,
    /// Synchronous field inspection produces a proven Copy value without transferring its owner.
    Inspect,
}

/// Initialization at the write boundary, after the RHS has executed.
/// Descendant facts refine a record container independently of its root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedPlaceInitialization {
    Initialized,
    Uninitialized,
    Conditional,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedDisplacedField {
    fields: Box<[crate::final_analysis::CheckedFieldSelection]>,
    initialization: CheckedPlaceInitialization,
}

impl CheckedDisplacedField {
    pub(super) fn new(
        fields: Box<[crate::final_analysis::CheckedFieldSelection]>,
        initialization: CheckedPlaceInitialization,
    ) -> Self {
        Self {
            fields,
            initialization,
        }
    }
    pub const fn fields(&self) -> &[crate::final_analysis::CheckedFieldSelection] {
        &self.fields
    }
    pub const fn initialization(&self) -> CheckedPlaceInitialization {
        self.initialization
    }
}

/// Static cleanup contour of the replaced place. Conditional states require
/// drop flags; they never defer source availability or borrow legality.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedPlaceDisplacement {
    Unreachable,
    Reachable {
        initialization: CheckedPlaceInitialization,
        fields: Box<[CheckedDisplacedField]>,
    },
}

/// Exact source place admitted by the owning availability checker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedLocalPlaceAccess {
    place: CheckedPlace,
    mode: CheckedLocalPlaceMode,
    displacement: Option<CheckedPlaceDisplacement>,
}

impl CheckedLocalPlaceAccess {
    pub(super) const fn new(place: CheckedPlace, mode: CheckedLocalPlaceMode) -> Self {
        Self {
            place,
            mode,
            displacement: None,
        }
    }

    pub const fn place(&self) -> &CheckedPlace {
        &self.place
    }

    pub const fn mode(&self) -> CheckedLocalPlaceMode {
        self.mode
    }

    pub const fn displacement(&self) -> Option<&CheckedPlaceDisplacement> {
        self.displacement.as_ref()
    }

    pub(super) fn seal_displacement(
        &mut self,
        displacement: CheckedPlaceDisplacement,
    ) -> Result<(), super::CheckedLocalUseError> {
        if self.mode != CheckedLocalPlaceMode::Assign || self.displacement.is_some() {
            return Err(super::CheckedLocalUseError::InvalidTopology);
        }
        self.displacement = Some(displacement);
        Ok(())
    }
}

/// One accepted local access. Both operations share the same site/generation
/// authority; a place operation cannot be projected as Copy/Move/Borrow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedLocalAccess {
    ValueTransfer(CheckedLocalValueTransfer),
    PlaceAccess(Box<CheckedLocalPlaceAccess>),
}

impl CheckedLocalAccess {
    pub fn local(&self) -> LocalId {
        match self {
            Self::ValueTransfer(transfer) => transfer.local(),
            Self::PlaceAccess(access) => access.place().local_id(),
        }
    }

    pub fn value_transfer(&self) -> Option<CheckedLocalValueTransfer> {
        match self {
            Self::ValueTransfer(transfer) => Some(transfer.clone()),
            Self::PlaceAccess(_) => None,
        }
    }

    pub fn place_access(&self) -> Option<&CheckedLocalPlaceAccess> {
        match self {
            Self::ValueTransfer(_) => None,
            Self::PlaceAccess(access) => Some(access),
        }
    }
}

impl From<CheckedLocalValueTransfer> for CheckedLocalAccess {
    fn from(transfer: CheckedLocalValueTransfer) -> Self {
        Self::ValueTransfer(transfer)
    }
}
