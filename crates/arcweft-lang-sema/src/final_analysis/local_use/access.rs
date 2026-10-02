use super::{CheckedLocalValueTransfer, LocalId};
use crate::final_analysis::CheckedMutablePlace;

/// A writable operation does not produce a value carrier. Whole-place
/// assignment initializes the declaration after evaluating its new value;
/// mutation requires the existing value to remain available.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedLocalPlaceMode {
    Assign,
    Mutate,
}

/// Exact source place admitted by the owning availability checker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedLocalPlaceAccess {
    place: CheckedMutablePlace,
    mode: CheckedLocalPlaceMode,
}

impl CheckedLocalPlaceAccess {
    pub(super) const fn new(place: CheckedMutablePlace, mode: CheckedLocalPlaceMode) -> Self {
        Self { place, mode }
    }

    pub const fn place(&self) -> &CheckedMutablePlace {
        &self.place
    }

    pub const fn mode(&self) -> CheckedLocalPlaceMode {
        self.mode
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

    pub const fn value_transfer(&self) -> Option<CheckedLocalValueTransfer> {
        match self {
            Self::ValueTransfer(transfer) => Some(*transfer),
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
