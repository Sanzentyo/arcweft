//! Resource bodies retain their original Source, layout and invocation.
//!
//! The ledger's custody envelope may move this value between callers; it does
//! not reinterpret captures in the replacing caller's function context.

use super::{RuntimePresentationResourceLayout, RuntimePresentationResourceLayoutRef};
use crate::{
    pattern::RuntimeSemanticTypeId,
    plan::task_source::RuntimeTaskSourceProgram,
    program_types::RuntimeFunctionEffectInstantiation,
    value::{AwbcRuntimeValueSnapshot, RuntimePayload, RuntimeValue},
};
use arcweft_interaction_model::presentation::PresentationResourceKind;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[cfg(test)]
mod tests;

/// An owned resource body, independently of a handle's scope or slot custody.
/// Only the admitted executable evaluator can publish a live instance.
#[derive(Clone)]
pub struct RuntimePresentationResourceValue {
    pub(crate) source: RuntimeTaskSourceProgram,
    pub(crate) layout: RuntimePresentationResourceLayout,
    pub(crate) binding: Option<Arc<RuntimeFunctionEffectInstantiation>>,
    pub(crate) body: RuntimePayload,
}

impl RuntimePresentationResourceValue {
    pub fn resource(&self) -> PresentationResourceKind {
        self.layout.resource()
    }

    pub fn layout(&self) -> &RuntimePresentationResourceLayout {
        &self.layout
    }

    pub fn body_type(&self) -> RuntimeSemanticTypeId {
        self.layout.body_type()
    }

    pub fn body(&self) -> &RuntimeValue {
        &self.body.0
    }

    /// The retained executable remains the authority for this body's types.
    pub fn program_owner(&self) -> crate::task::RuntimeProgramOwner {
        self.source.program_owner()
    }

    /// The construction invocation, independently of a later receiving scope.
    pub fn type_instantiation(&self) -> Option<&RuntimeFunctionEffectInstantiation> {
        self.binding.as_deref()
    }

    /// Copies a presentation consumer's immutable inputs only after the whole
    /// body passes its original Source, invocation, ownership and resource
    /// allowance. Affine descendants are rejected before any child is copied.
    pub fn copy_unrestricted_body(
        &self,
        limits: crate::entry::RuntimeSchemaLimits,
    ) -> Result<RuntimeValue, crate::value::AwbcRuntimeValueSnapshotError> {
        crate::value::RuntimeRetainedValueAdmission::standalone(limits)
            .validate_presentation_body(self)?;
        Ok(self.body().clone())
    }

    pub(crate) fn original_executable(&self) -> [u8; 32] {
        *self
            .source
            .original()
            .executable_semantic_digest()
            .as_bytes()
    }
}

impl std::fmt::Debug for RuntimePresentationResourceValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimePresentationResourceValue")
            .field("original_executable", &self.original_executable())
            .field("layout", &self.layout)
            .field("binding", &self.binding)
            .field("body", &self.body)
            .finish()
    }
}

impl PartialEq for RuntimePresentationResourceValue {
    fn eq(&self, other: &Self) -> bool {
        self.source
            .program_owner()
            .same_program(&other.source.program_owner())
            && self.layout == other.layout
            && self.binding == other.binding
            && self.body == other.body
    }
}

impl Serialize for RuntimePresentationResourceValue {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom(
            "presentation resource values require an original-program snapshot",
        ))
    }
}

impl<'de> Deserialize<'de> for RuntimePresentationResourceValue {
    fn deserialize<D: serde::Deserializer<'de>>(_: D) -> Result<Self, D::Error> {
        Err(serde::de::Error::custom(
            "presentation resource values require original-program admission",
        ))
    }
}

/// Inert data only. Original executable assertions, layout, binding and every
/// captured child require admission together before a live value is issued.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePresentationResourceSnapshot {
    pub(crate) original_executable: [u8; 32],
    pub(crate) layout: RuntimePresentationResourceLayout,
    pub(crate) binding: Option<Arc<RuntimeFunctionEffectInstantiation>>,
    pub(crate) body: Box<AwbcRuntimeValueSnapshot>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimePresentationResourceView<'a> {
    NativeConstruction {
        program: &'a Arc<crate::plan::RuntimePlan>,
        layout: RuntimePresentationResourceLayoutRef<'a>,
        binding: Option<&'a RuntimeFunctionEffectInstantiation>,
        body: &'a RuntimeValue,
    },
    Construction(&'a RuntimePresentationResourceValue),
    Live(&'a RuntimePresentationResourceValue),
    Snapshot(&'a RuntimePresentationResourceSnapshot),
}

impl<'a> RuntimePresentationResourceView<'a> {
    pub(crate) fn live_value(self) -> Option<&'a RuntimePresentationResourceValue> {
        match self {
            Self::NativeConstruction { .. } => None,
            Self::Construction(value) | Self::Live(value) => Some(value),
            Self::Snapshot(_) => None,
        }
    }
    pub(crate) fn layout(self) -> RuntimePresentationResourceLayoutRef<'a> {
        match self {
            Self::NativeConstruction { layout, .. } => layout,
            Self::Construction(value) | Self::Live(value) => value.layout().borrowed(),
            Self::Snapshot(value) => value.layout.borrowed(),
        }
    }

    pub(crate) fn original_executable(self) -> [u8; 32] {
        match self {
            Self::NativeConstruction { program, .. } => {
                *program.executable_semantic_digest().as_bytes()
            }
            Self::Construction(value) | Self::Live(value) => value.original_executable(),
            Self::Snapshot(value) => value.original_executable,
        }
    }

    pub(crate) fn source(self) -> Option<&'a RuntimeTaskSourceProgram> {
        match self {
            Self::NativeConstruction { .. } => None,
            Self::Construction(value) | Self::Live(value) => Some(&value.source),
            Self::Snapshot(_) => None,
        }
    }

    pub(crate) fn binding(self) -> Option<&'a RuntimeFunctionEffectInstantiation> {
        match self {
            Self::NativeConstruction { binding, .. } => binding,
            Self::Construction(value) | Self::Live(value) => value.binding.as_deref(),
            Self::Snapshot(value) => value.binding.as_deref(),
        }
    }

    pub(crate) fn body(self) -> crate::value::RuntimeValueView<'a> {
        match self {
            Self::NativeConstruction { body, .. } => body.view(),
            Self::Construction(value) | Self::Live(value) => value.body.0.view(),
            Self::Snapshot(value) => value.body.view(),
        }
    }

    pub(crate) fn native_construction_program(self) -> Option<&'a Arc<crate::plan::RuntimePlan>> {
        match self {
            Self::NativeConstruction { program, .. } => Some(program),
            _ => None,
        }
    }
}
