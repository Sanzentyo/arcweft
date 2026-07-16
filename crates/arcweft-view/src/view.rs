//! Typed View identities, registry data, and mounted-occurrence allocation.

use serde::{Deserialize, Serialize};
use thiserror::Error;

mod identity;
mod registry;

pub use identity::{AcceptedViewProgramRevision, ViewId, ViewIdentityError, ViewProgramId};
pub use registry::{
    ViewDescriptor, ViewImplementation, ViewRegistry, ViewRegistryError, ViewRegistryId,
};

/// Stable identifier for a view call/property schema.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ViewSchemaId(pub u64);

/// Runtime-allocated occurrence of one mounted View.
///
/// A program may be mounted more than once, so program identity is not an
/// instance identity. Only [`ViewMountAllocator`] creates new IDs.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ViewMountId(u64);

/// Monotonic allocator shared by retained View mount owners.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ViewMountAllocator {
    next: u64,
}

/// Failure to allocate or restore a View mount identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewMountAllocationError {
    #[error("the View mount-id allocator is exhausted")]
    Exhausted,
    #[error(
        "View mount allocator cursor {next} is not newer than restored live mount {greatest_live}"
    )]
    CursorNotFresh { next: u64, greatest_live: u64 },
}

/// Stable identifier for a host-registered Rust view implementation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RustViewId(pub u32);

impl ViewMountId {
    /// Reconstructs a persisted mount identity at a typed boundary.
    ///
    /// Live issuance still belongs to [`ViewMountAllocator`]; callers that
    /// restore several IDs must validate the allocator cursor separately.
    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub(crate) const fn from_allocated(value: u64) -> Self {
        Self::from_raw(value)
    }
}

impl ViewMountAllocator {
    /// Allocates an ID that will never be reused by this allocator.
    pub fn allocate(&mut self) -> Result<ViewMountId, ViewMountAllocationError> {
        let allocated = self.next;
        self.next = self
            .next
            .checked_add(1)
            .ok_or(ViewMountAllocationError::Exhausted)?;
        Ok(ViewMountId::from_allocated(allocated))
    }

    pub const fn next(self) -> u64 {
        self.next
    }

    /// Restores a cursor after validating it against all live mount IDs.
    pub fn restore_cursor(
        &mut self,
        next: u64,
        greatest_live: Option<ViewMountId>,
    ) -> Result<(), ViewMountAllocationError> {
        if let Some(greatest_live) = greatest_live
            && next <= greatest_live.get()
        {
            return Err(ViewMountAllocationError::CursorNotFresh {
                next,
                greatest_live: greatest_live.get(),
            });
        }
        self.next = next;
        Ok(())
    }
}
