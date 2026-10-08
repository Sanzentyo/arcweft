//! E1 commits actual slot admission without descending back into its body.

use super::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::plan::{RuntimeLocalOwner, RuntimeLocalPlacement, RuntimeLocalStorage};
use crate::runtime_id::RuntimeLocalDeclarationId;
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticMeter};

impl RuntimeBodySemanticContext<'_> {
    pub(crate) fn local_row_digest(
        &self,
        meter: &mut TaskSemanticMeter,
        local: RuntimeLocalDeclarationId,
    ) -> Result<blake3::Hash, RuntimeBodySemanticError> {
        meter.status()?;
        let mut encoder =
            TaskSemanticEncoder::new(b"arcweft.runtime-plan.executable-row.v1\0", meter);
        encoder.tag(1);
        encoder.tag(0);
        self.write_local(&mut encoder, local)?;
        encoder.finish().map_err(Into::into)
    }

    pub(super) fn write_local_placement(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        placement: RuntimeLocalPlacement,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.status()?;
        encoder.tag(match placement.storage() {
            RuntimeLocalStorage::InvocationFrame => 0,
            RuntimeLocalStorage::LineFrame => 1,
            RuntimeLocalStorage::StreamFrame => 2,
        });
        encoder.tag(u8::from(placement.is_mutable()));
        placement
            .initialization()
            .encode_semantic_initialization(encoder);
        encoder.status()?;
        match placement.owner() {
            RuntimeLocalOwner::Function(id) => {
                encoder.tag(0);
                let row = self.plan.function_sites().get(id).ok_or_else(|| {
                    encoder.reject_owner();
                    RuntimeBodySemanticError::MissingRow {
                        table: "local function owner",
                        ordinal: id.get().get() as usize - 1,
                    }
                })?;
                // Accepted definition is a leaf, never the completed F digest.
                encoder.digest(row.definition().as_bytes());
                encoder.tag(row.role().semantic_tag());
                encoder.tag(u8::from(row.function_type().is_some()));
                if let Some(context) = row.function_type() {
                    self.write_type(encoder, context)?;
                }
            }
            RuntimeLocalOwner::PureHelper(id) => {
                encoder.tag(1);
                let row = self.plan.pure_helpers().get(id.0).ok_or_else(|| {
                    encoder.reject_owner();
                    RuntimeBodySemanticError::MissingRow {
                        table: "local helper owner",
                        ordinal: id.0,
                    }
                })?;
                encoder.digest(row.definition.as_bytes());
            }
            RuntimeLocalOwner::TraitMethod(id) => {
                encoder.tag(2);
                let row = self.plan.trait_methods().get(id.0).ok_or_else(|| {
                    encoder.reject_owner();
                    RuntimeBodySemanticError::MissingRow {
                        table: "local method owner",
                        ordinal: id.0,
                    }
                })?;
                encoder.digest(row.definition.as_bytes());
            }
            RuntimeLocalOwner::Line { group, body } => {
                encoder.tag(3);
                let row = self
                    .plan
                    .line_task_groups()
                    .get(group.index())
                    .ok_or_else(|| {
                        encoder.reject_owner();
                        RuntimeBodySemanticError::MissingRow {
                            table: "local Line owner",
                            ordinal: group.index(),
                        }
                    })?;
                encoder.digest(row.definition().as_bytes());
                body.encode_semantic_local_body(encoder);
            }
            RuntimeLocalOwner::Stream { ordinal } => {
                encoder.tag(4);
                let row = self
                    .plan
                    .stream_plans()
                    .get(ordinal as usize)
                    .ok_or_else(|| {
                        encoder.reject_owner();
                        RuntimeBodySemanticError::MissingRow {
                            table: "local Stream owner",
                            ordinal: ordinal as usize,
                        }
                    })?;
                encoder.count(row.id().path().segments().len());
                for segment in row.id().path().segments() {
                    encoder.enter_element();
                    encoder.string(segment.as_str());
                }
            }
        }
        encoder.status().map_err(Into::into)
    }
}
