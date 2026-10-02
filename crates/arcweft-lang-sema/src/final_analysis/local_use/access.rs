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

/// One local-rooted place, selected entirely from admitted field schemas.
/// Empty projections denote the whole declaration. Diagnostic field names
/// never participate in overlap or availability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedLocalPlace {
    local: LocalId,
    fields: Box<[crate::final_analysis::CheckedFieldSelection]>,
}

impl CheckedLocalPlace {
    pub(crate) fn from_field<'a>(
        owner: super::ExprId,
        expression: impl Fn(super::ExprId) -> Option<&'a crate::final_analysis::CheckedExpression>,
    ) -> Option<Self> {
        use crate::final_analysis::{
            CheckedExpressionResolution, CheckedFieldReceiver, CheckedSelectResolution,
        };
        let local = expression(owner)?.field_root(|child| {
            expression(child).and_then(crate::final_analysis::CheckedExpression::local_place_source)
        })?;
        let mut current = owner;
        let mut fields = Vec::new();
        while let Some(CheckedExpressionResolution::Select(CheckedSelectResolution::Field(field))) =
            expression(current).map(crate::final_analysis::CheckedExpression::resolution)
        {
            fields.push(field.selection().clone());
            match field.receiver() {
                CheckedFieldReceiver::Binding(_) => break,
                CheckedFieldReceiver::Expression(receiver) => current = receiver,
            }
        }
        fields.reverse();
        Some(Self::new(local, fields.into_boxed_slice()))
    }

    pub(super) fn new(
        local: LocalId,
        fields: Box<[crate::final_analysis::CheckedFieldSelection]>,
    ) -> Self {
        Self { local, fields }
    }

    pub const fn local(&self) -> LocalId {
        self.local
    }
    pub fn fields(&self) -> &[crate::final_analysis::CheckedFieldSelection] {
        &self.fields
    }
    pub(super) fn into_fields(self) -> Box<[crate::final_analysis::CheckedFieldSelection]> {
        self.fields
    }

    pub(super) fn overlaps(&self, other: &Self) -> bool {
        self.local == other.local
            && self
                .fields
                .iter()
                .zip(other.fields.iter())
                .all(|(left, right)| left.field() == right.field())
    }

    pub(super) fn from_mutable(place: &CheckedMutablePlace) -> Self {
        Self::new(
            place.local_id(),
            place
                .nominal_field()
                .map(|field| vec![field.field().clone()].into_boxed_slice())
                .unwrap_or_default(),
        )
    }
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
