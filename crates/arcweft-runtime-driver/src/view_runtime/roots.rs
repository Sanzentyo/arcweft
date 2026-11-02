//! Root inputs preserve the distinction between inert host descriptions and
//! resources borrowed from the original execution custody.

use super::catalog::ViewProgramCatalog;
use crate::{
    presentation_handles::PresentationHandleRecord,
    presentation_projection::{PresentationProjection, PresentationProjectionError},
};
use arcweft_core::{
    entry::RuntimeSchemaLimits,
    presentation::{RuntimePresentationFrame, RuntimePresentationResourceValue},
    value::{RuntimeRetainedValueAdmission, RuntimeValue},
};
use arcweft_id::presentation::PresentationHandleId;
use arcweft_interaction_model::presentation::PresentationArgumentRole;
use std::{borrow::Cow, collections::BTreeMap};

/// Render roots supplied by a host, or borrowed from the actual executable.
/// Descriptions grant no Core access. Session execution always selects its
/// complete borrowed frame, including original supplied View inputs.
#[derive(Clone, Copy, Debug)]
pub enum BundleViewRootInput<'a> {
    Descriptions(&'a [PresentationHandleRecord]),
    Execution(RuntimePresentationFrame<'a>),
}

impl<'a> BundleViewRootInput<'a> {
    pub(super) fn handles(
        self,
    ) -> Result<Cow<'a, [PresentationHandleRecord]>, PresentationProjectionError> {
        match self {
            Self::Descriptions(handles) => Ok(Cow::Borrowed(handles)),
            Self::Execution(frame) => {
                PresentationProjection::try_new(frame, RuntimeSchemaLimits::engine_default())
                    .map(|projection| Cow::Owned(projection.handles))
            }
        }
    }

    /// A shared allowance covers every Source root before the first copied
    /// input or mount mutation. Original Source admission traverses real
    /// function captures and rejects affine descendants.
    pub(super) fn admit(
        self,
        admission: &mut RuntimeRetainedValueAdmission,
        catalog: Option<&ViewProgramCatalog>,
    ) -> Result<(), PresentationProjectionError> {
        let Self::Execution(frame) = self else {
            return Ok(());
        };
        for resource in frame.resources() {
            let Some(value) = Self::view_value(resource.value()) else {
                continue;
            };
            admission.validate_presentation_body(value)?;
            let call = value.layout().view().expect("View producer selected");
            let definition = catalog
                .and_then(|catalog| {
                    catalog
                        .definition_index(call.view())
                        .map(|index| catalog.execution_definition(index))
                })
                .ok_or_else(|| Self::error(resource.token().presentation_id()))?;
            if definition.construction_identity != Some(call.definition())
                || definition.parameters.len() != call.parameters() as usize
            {
                return Err(Self::error(resource.token().presentation_id()));
            }
        }
        Ok(())
    }

    pub(super) fn arguments(
        self,
        handle: &PresentationHandleId,
    ) -> Result<Option<BTreeMap<u16, RuntimeValue>>, PresentationProjectionError> {
        let Self::Execution(frame) = self else {
            return Ok(None);
        };
        let Some(resource) = frame
            .resources()
            .find(|resource| resource.token().presentation_id() == *handle)
        else {
            // Dialogue roots are owned by the separate dialogue occurrence.
            return Ok(None);
        };
        let Some(value) = Self::view_value(resource.value()) else {
            return Ok(None);
        };
        let call = value.layout().view().expect("View producer selected");
        let RuntimeValue::Tuple(values) =
            value.copy_unrestricted_body(RuntimeSchemaLimits::engine_default())?
        else {
            return Err(Self::error(handle.clone()));
        };
        if values.len() != call.supplied().len() {
            return Err(Self::error(handle.clone()));
        }
        call.supplied()
            .iter()
            .zip(values)
            .map(|(ordinal, value)| {
                u16::try_from(*ordinal)
                    .map(|ordinal| (ordinal, value))
                    .map_err(|_| Self::error(handle.clone()))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(Some)
    }

    fn view_value(
        value: &RuntimePresentationResourceValue,
    ) -> Option<&RuntimePresentationResourceValue> {
        if value.layout().view().is_some() {
            return Some(value);
        }
        let call = value.layout().factory()?;
        let RuntimeValue::Tuple(values) = value.body() else {
            return None;
        };
        call.body_inputs()
            .zip(values)
            .find_map(|((_, key), value)| {
                if key.role() != PresentationArgumentRole::ViewValue {
                    return None;
                }
                match value {
                    RuntimeValue::PresentationResource(value)
                        if value.layout().view().is_some() =>
                    {
                        Some(value.as_ref())
                    }
                    _ => None,
                }
            })
    }

    fn error(handle: PresentationHandleId) -> PresentationProjectionError {
        PresentationProjectionError::Argument {
            handle,
            role: PresentationArgumentRole::ViewValue,
            expected: "the original checked View construction ABI in the accepted product",
        }
    }
}
