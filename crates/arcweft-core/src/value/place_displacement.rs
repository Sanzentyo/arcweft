//! Static initialization contours used by assignment and cleanup boundaries.

use serde::{Deserialize, Serialize};

use super::{RuntimeMutablePlace, RuntimeRecordFieldId};

/// A definite or path-dependent initialization fact. Conditional initialization
/// requires a drop flag; source access legality is established before execution.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimePlaceInitialization {
    Initialized,
    Uninitialized,
    Conditional,
}

impl RuntimePlaceInitialization {
    pub(crate) fn join(self, incoming: Self) -> Self {
        if self == incoming {
            self
        } else {
            Self::Conditional
        }
    }

    pub(crate) fn is_initialized(self) -> bool {
        self == Self::Initialized
    }
    pub(crate) fn is_uninitialized(self) -> bool {
        self == Self::Uninitialized
    }
}

/// A relative, schema-ordered child path and its post-RHS initialization.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeDisplacedField<F> {
    pub fields: Box<[F]>,
    pub initialization: RuntimePlaceInitialization,
}

/// Exact static contour of the replaced place. This is carried by the owning
/// assignment, not a copied access index. Field paths refine record containers.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum RuntimePlaceDisplacement<F> {
    Unreachable,
    Reachable {
        initialization: RuntimePlaceInitialization,
        fields: Box<[RuntimeDisplacedField<F>]>,
    },
}

impl<F> RuntimePlaceDisplacement<F> {
    /// A host/runtime-owned write with path-dependent current occupancy.
    /// Source-lowered writes use their checked post-RHS contour instead.
    pub fn conditional() -> Self {
        Self::Reachable {
            initialization: RuntimePlaceInitialization::Conditional,
            fields: Box::new([]),
        }
    }
}

/// One typed write, including the old-value cleanup obligation after its RHS.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAssignment {
    place: RuntimeMutablePlace,
    displacement: RuntimePlaceDisplacement<RuntimeRecordFieldId>,
}

impl RuntimeAssignment {
    pub fn new(
        place: RuntimeMutablePlace,
        displacement: RuntimePlaceDisplacement<RuntimeRecordFieldId>,
    ) -> Self {
        Self {
            place,
            displacement,
        }
    }
    pub const fn place(&self) -> &RuntimeMutablePlace {
        &self.place
    }
    pub const fn displacement(&self) -> &RuntimePlaceDisplacement<RuntimeRecordFieldId> {
        &self.displacement
    }
    pub const fn local(&self) -> crate::runtime_id::RuntimeLocalDeclarationId {
        self.place.local()
    }
}
