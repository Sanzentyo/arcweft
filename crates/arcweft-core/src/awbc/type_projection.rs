//! Projection from verified AWBC type rows to native runtime owners.

use super::schema::{AwbcProgram, AwbcRuntimeType, AwbcTypeId};
use crate::entry::RuntimeIdentityError;
use crate::pattern::{
    RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId,
    RuntimeSemanticTypeId,
};
use thiserror::Error;

/// Failure to reify an AWBC runtime type as its native checked owner.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AwbcTypeProjectionError {
    #[error("AWBC runtime type index {index} is out of bounds")]
    RuntimeTypeOutOfBounds { index: u32 },
    #[error("AWBC opaque producer string index {index} is out of bounds")]
    ProducerStringOutOfBounds { index: u32 },
    #[error("AWBC opaque producer at string index {index} is invalid: {source}")]
    InvalidOpaqueProducer {
        index: u32,
        source: RuntimeIdentityError,
    },
}

impl AwbcRuntimeType {
    /// Reifies an opaque row through the same native owner used by checked
    /// execution. Non-opaque rows return `None`.
    pub fn try_opaque_owner(
        &self,
        strings: &[String],
    ) -> Result<Option<RuntimeOpaqueTypeOwner>, AwbcTypeProjectionError> {
        let Self::Opaque {
            producer,
            semantic_identity,
            admission,
        } = self
        else {
            return Ok(None);
        };
        let spelling = strings
            .get(producer.index())
            .ok_or(AwbcTypeProjectionError::ProducerStringOutOfBounds { index: producer.0 })?;
        let producer =
            RuntimeOpaqueTypeProducerId::try_new(spelling.clone()).map_err(|source| {
                AwbcTypeProjectionError::InvalidOpaqueProducer {
                    index: producer.0,
                    source,
                }
            })?;
        let semantic_identity = RuntimeSemanticTypeId::from_bytes(*semantic_identity);
        Ok(Some(match admission {
            RuntimeOpaqueTypeAdmission::ExactIdentity => {
                RuntimeOpaqueTypeOwner::exact(producer, semantic_identity)
            }
            RuntimeOpaqueTypeAdmission::ProducerWide => {
                RuntimeOpaqueTypeOwner::producer_wide(producer, semantic_identity)
            }
        }))
    }
}

impl AwbcProgram {
    /// Reifies one indexed opaque row through its canonical string table.
    pub fn opaque_owner(
        &self,
        ty: AwbcTypeId,
    ) -> Result<Option<RuntimeOpaqueTypeOwner>, AwbcTypeProjectionError> {
        self.runtime_types
            .get(ty.index())
            .ok_or(AwbcTypeProjectionError::RuntimeTypeOutOfBounds { index: ty.0 })?
            .try_opaque_owner(&self.strings)
    }
}
