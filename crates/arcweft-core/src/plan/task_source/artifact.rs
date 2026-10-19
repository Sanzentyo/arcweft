//! Inert complete original Source carried by executable artifacts.

use crate::entry::RuntimeSchemaLimits;
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::body_semantic::task_image::codec::{
    PreparedRuntimeTaskSource, RuntimeTaskSourceCodecError, RuntimeTaskSourceCodecLimits,
};
use crate::plan::{
    RuntimePlan, RuntimeProgramSourceLimits, RuntimeTaskPlanSealError, RuntimeTaskPlanSealLimits,
    ViewTaskPlanAuthority,
};
use crate::task::semantic::TaskSemanticMeter;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[cfg(test)]
mod tests;

/// The three original allowances of one complete Source admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeTaskSourceArtifactLimits {
    pub codec: RuntimeTaskSourceCodecLimits,
    pub seal: RuntimeTaskPlanSealLimits,
    pub values: RuntimeSchemaLimits,
}

impl Default for RuntimeTaskSourceArtifactLimits {
    fn default() -> Self {
        let seal = RuntimeTaskPlanSealLimits::default();
        let values = RuntimeSchemaLimits::engine_default();
        let bytes = 256 * 1024 * 1024;
        Self {
            codec: RuntimeTaskSourceCodecLimits {
                rows: seal.max_task_plan_rows,
                encoded_bytes: bytes,
                decoded_bytes: bytes,
                request_roles: seal.max_request_roles,
                path_steps: seal.max_children_per_row,
                program: RuntimeProgramSourceLimits {
                    rows: seal.max_executable_rows,
                    nodes: values.max_nodes,
                    elements: values.max_sequence_items,
                    depth: 128,
                    policy_depth: 32,
                    encoded_bytes: bytes,
                    decoded_bytes: bytes,
                    opaque_producer_bytes: crate::value::RuntimeHandleKind::MAX_PRODUCER_BYTES,
                },
            },
            seal,
            values,
        }
    }
}

/// Complete encoded Source, including inert expected-key assertions. Cloning
/// retains the same bytes. Neither deserialization nor possession issues a key.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RuntimeTaskSourceArtifact {
    data: Arc<RuntimeTaskSourceArtifactData>,
}

#[derive(Deserialize, Eq, PartialEq, Serialize)]
struct RuntimeTaskSourceArtifactData {
    expected_executable: [u8; 32],
    bytes: Vec<u8>,
}

impl std::fmt::Debug for RuntimeTaskSourceArtifact {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeTaskSourceArtifact")
            .field("encoded_bytes", &self.data.bytes.len())
            .finish_non_exhaustive()
    }
}

/// Retains the original typed Source failure without exposing construction
/// internals or erasing a child failure into a display string.
#[derive(Clone, Debug, thiserror::Error)]
#[error(transparent)]
pub struct RuntimeTaskSourceArtifactError(Arc<RuntimeTaskSourceCodecError>);

impl From<RuntimeTaskSourceCodecError> for RuntimeTaskSourceArtifactError {
    fn from(error: RuntimeTaskSourceCodecError) -> Self {
        Self(Arc::new(error))
    }
}

impl RuntimeTaskSourceArtifactError {
    /// Returns the common seal's structured failure when admission reached it.
    pub fn seal_error(&self) -> Option<&RuntimeTaskPlanSealError> {
        match self.0.as_ref() {
            RuntimeTaskSourceCodecError::Image(error) => Some(error),
            _ => None,
        }
    }
}

impl RuntimePlan {
    /// Serializes this actual complete program's original Task Source.
    pub fn task_source_artifact(
        &self,
        limits: RuntimeTaskSourceArtifactLimits,
    ) -> Result<RuntimeTaskSourceArtifact, RuntimeTaskSourceArtifactError> {
        let mut meter = TaskSemanticMeter::new(
            limits.seal.max_semantic_work,
            limits.seal.max_transcript_bytes,
        );
        let resources = ValueResourceAdmission::live(limits.values);
        let bytes = self.encode_task_sources(limits.codec, &resources, &mut meter)?;
        Ok(RuntimeTaskSourceArtifact::from_encoded_source(
            *self.executable_semantic_digest().as_bytes(),
            bytes,
        ))
    }
}

impl RuntimeTaskSourceArtifact {
    /// Untrusted artifact assertion, compared with the actual E transcript
    /// before common-seal publication. This is not an issued digest.
    pub fn expected_executable(&self) -> &[u8; 32] {
        &self.data.expected_executable
    }

    pub fn bytes(&self) -> &[u8] {
        self.data.bytes.as_slice()
    }

    pub(crate) fn from_encoded_source(expected_executable: [u8; 32], bytes: Vec<u8>) -> Self {
        Self {
            data: Arc::new(RuntimeTaskSourceArtifactData {
                expected_executable,
                bytes,
            }),
        }
    }

    /// Checks the borrowed wire shape and complete construction footprint.
    /// Expected keys remain assertions until `admit` invokes the common seal.
    pub(crate) fn preflight_encoded(
        bytes: &[u8],
        limits: RuntimeTaskSourceArtifactLimits,
    ) -> Result<(), RuntimeTaskSourceArtifactError> {
        let meter = TaskSemanticMeter::new(
            limits.seal.max_semantic_work,
            limits.seal.max_transcript_bytes,
        );
        let mut resources = ValueResourceAdmission::live(limits.values);
        PreparedRuntimeTaskSource::preflight(
            bytes,
            limits.codec,
            limits.seal,
            meter,
            &mut resources,
        )?;
        Ok(())
    }

    /// Reconstructs the actual complete original program through its one
    /// namespace issuer and common seal. View rows require the real authority.
    pub fn admit(
        &self,
        limits: RuntimeTaskSourceArtifactLimits,
        authority: Option<&dyn ViewTaskPlanAuthority>,
    ) -> Result<RuntimePlan, RuntimeTaskSourceArtifactError> {
        let meter = TaskSemanticMeter::new(
            limits.seal.max_semantic_work,
            limits.seal.max_transcript_bytes,
        );
        let mut resources = ValueResourceAdmission::live(limits.values);
        let image = PreparedRuntimeTaskSource::preflight(
            self.bytes(),
            limits.codec,
            limits.seal,
            meter,
            &mut resources,
        )?
        .materialize()?
        .with_expected_executable_key(*self.expected_executable());
        let sealed = image
            .seal(limits.seal, authority)
            .map_err(RuntimeTaskSourceCodecError::from)?;
        Ok(sealed.into_plan())
    }
}
