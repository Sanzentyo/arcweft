//! Borrowed display admission over the original custody banks.

use super::{PresentationBanks, PresentationBody, PresentationRegistrationSite};
use crate::{
    entry::RuntimeSchemaLimits,
    presentation::RuntimePresentationResourceValue,
    runtime_id::RuntimePresentationHandleToken,
    value::{RuntimePayload, RuntimeValue},
};
use arcweft_interaction_model::presentation::PresentationSlotAddress;
use thiserror::Error;

#[cfg(test)]
mod tests;

/// A read-only view of every currently registered resource, across all banks.
/// Admission precedes publishing any resource reference. The borrow prevents
/// execution, restore or replacement from mutating custody during projection.
/// It owns no body, grants no handle, and has no wire constructor.
///
/// Execution cannot invalidate an outstanding resource borrow:
///
/// ```compile_fail
/// use arcweft_core::{engine::Engine, entry::RuntimeSchemaLimits};
/// fn cannot_step_while_projecting(engine: &mut Engine) {
///     let frame = engine.presentation_frame(RuntimeSchemaLimits::engine_default()).unwrap();
///     engine.step(Default::default(), Default::default());
///     assert!(frame.is_empty());
/// }
/// ```
///
/// A saved display description cannot manufacture execution custody:
///
/// ```compile_fail
/// use arcweft_core::presentation::RuntimePresentationFrame;
/// let frame: RuntimePresentationFrame<'_> = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Clone, Copy, Debug)]
pub struct RuntimePresentationFrame<'a> {
    banks: Option<&'a PresentationBanks<PresentationBody<RuntimePayload>>>,
    resources: usize,
}

/// One original resource and its authoritative registration metadata.
#[derive(Clone, Copy, Debug)]
pub struct RuntimePresentationResourceRef<'a> {
    token: &'a RuntimePresentationHandleToken,
    slot: Option<&'a PresentationSlotAddress>,
    visibility_override: Option<bool>,
    value: &'a RuntimePresentationResourceValue,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimePresentationFrameError {
    #[error("presentation frame resource count overflow")]
    Arithmetic,
    #[error("presentation frame contains {actual} resources, exceeding maximum {maximum}")]
    Resources { actual: usize, maximum: u32 },
    #[error("presentation resource {ordinal} disagrees with its original custody record")]
    Resource { ordinal: usize },
}

impl<'a> RuntimePresentationFrame<'a> {
    pub(crate) fn try_new(
        banks: Option<&'a PresentationBanks<PresentationBody<RuntimePayload>>>,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimePresentationFrameError> {
        let mut resources = 0_usize;
        if let Some(banks) = banks {
            for lease in banks.active_leases() {
                resources = resources
                    .checked_add(1)
                    .ok_or(RuntimePresentationFrameError::Arithmetic)?;
                if !limits.permits_sequence_items(resources) {
                    return Err(RuntimePresentationFrameError::Resources {
                        actual: resources,
                        maximum: limits.max_sequence_items,
                    });
                }
                let Some(body) = lease.body.as_ref() else {
                    unreachable!("active_leases borrows occupied original records");
                };
                let RuntimeValue::PresentationResource(value) = body.value.value() else {
                    return Err(RuntimePresentationFrameError::Resource {
                        ordinal: resources - 1,
                    });
                };
                if value.resource() != lease.token.resource()
                    || value.resource() != lease.site.resource()
                    || value.layout().resource_type() != body.expected
                {
                    return Err(RuntimePresentationFrameError::Resource {
                        ordinal: resources - 1,
                    });
                }
            }
        }
        Ok(Self { banks, resources })
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.resources
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.resources == 0
    }

    /// Visits bank identity then issuance order. Retired and displaced bodies
    /// are absent; the old handle cannot project a replacement's body.
    pub fn resources(self) -> impl Iterator<Item = RuntimePresentationResourceRef<'a>> + 'a {
        self.banks.into_iter().flat_map(|banks| {
            banks.active_leases().map(|lease| {
                let Some(body) = lease.body.as_ref() else {
                    unreachable!("the admitted immutable frame retains occupied records");
                };
                let RuntimeValue::PresentationResource(value) = body.value.value() else {
                    unreachable!("frame admission checked every immutable resource carrier");
                };
                RuntimePresentationResourceRef {
                    token: &lease.token,
                    slot: match &lease.site {
                        PresentationRegistrationSite::Slot(address) => Some(address),
                        PresentationRegistrationSite::Independent(_) => None,
                    },
                    visibility_override: lease.visibility_override,
                    value,
                }
            })
        })
    }
}

impl<'a> RuntimePresentationResourceRef<'a> {
    #[must_use]
    pub const fn token(self) -> &'a RuntimePresentationHandleToken {
        self.token
    }

    #[must_use]
    pub const fn slot(self) -> Option<&'a PresentationSlotAddress> {
        self.slot
    }

    /// An explicit handle command overrides the original authored visibility.
    #[must_use]
    pub const fn visibility_override(self) -> Option<bool> {
        self.visibility_override
    }

    #[must_use]
    pub const fn value(self) -> &'a RuntimePresentationResourceValue {
        self.value
    }
}
