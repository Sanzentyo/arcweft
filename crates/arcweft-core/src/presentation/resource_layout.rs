//! One retained-resource layout algebra, independent of bank custody. A staged
//! factory and an unmounted View value retain their distinct producer contracts.

use super::{
    RuntimePresentationFactoryCall, RuntimePresentationFactoryCallError,
    RuntimePresentationViewCall, RuntimePresentationViewCallError,
};
use crate::{pattern::RuntimeSemanticTypeId, program_types::RuntimeProgramTypes};
use arcweft_interaction_model::presentation::PresentationResourceKind;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "producer", content = "call", rename_all = "snake_case")]
pub enum RuntimePresentationResourceLayout {
    Factory(Arc<RuntimePresentationFactoryCall>),
    View(Arc<RuntimePresentationViewCall>),
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "producer", content = "call", rename_all = "snake_case")]
pub(crate) enum RuntimePresentationResourceLayoutRef<'a> {
    Factory(&'a RuntimePresentationFactoryCall),
    View(&'a RuntimePresentationViewCall),
}

impl RuntimePresentationResourceLayout {
    pub(crate) fn borrowed(&self) -> RuntimePresentationResourceLayoutRef<'_> {
        match self {
            Self::Factory(call) => RuntimePresentationResourceLayoutRef::Factory(call),
            Self::View(call) => RuntimePresentationResourceLayoutRef::View(call),
        }
    }

    pub fn factory(&self) -> Option<&RuntimePresentationFactoryCall> {
        match self {
            Self::Factory(call) => Some(call),
            Self::View(_) => None,
        }
    }
    pub fn view(&self) -> Option<&RuntimePresentationViewCall> {
        match self {
            Self::View(call) => Some(call),
            Self::Factory(_) => None,
        }
    }
    pub fn resource(&self) -> PresentationResourceKind {
        self.borrowed().resource()
    }
    pub fn resource_type(&self) -> RuntimeSemanticTypeId {
        self.borrowed().resource_type()
    }
    pub fn body_type(&self) -> RuntimeSemanticTypeId {
        self.borrowed().body_type()
    }
}

impl<'a> RuntimePresentationResourceLayoutRef<'a> {
    pub(crate) fn resource(self) -> PresentationResourceKind {
        match self {
            Self::Factory(call) => call.resource(),
            Self::View(_) => PresentationResourceKind::View,
        }
    }
    pub(crate) fn resource_type(self) -> RuntimeSemanticTypeId {
        match self {
            Self::Factory(call) => call.resource_type(),
            Self::View(call) => call.resource_type(),
        }
    }
    pub(crate) fn body_type(self) -> RuntimeSemanticTypeId {
        match self {
            Self::Factory(call) => call.body_type(),
            Self::View(call) => call.body_type(),
        }
    }
    pub(crate) fn input_count(self) -> usize {
        match self {
            Self::Factory(call) => call.inputs().len(),
            Self::View(call) => call.supplied().len(),
        }
    }
    pub(crate) fn validate_input_types(
        self,
        types: RuntimeProgramTypes<'_>,
        count: usize,
        actual: impl FnMut(usize) -> Option<RuntimeSemanticTypeId>,
    ) -> Result<(), RuntimePresentationResourceLayoutError> {
        match self {
            Self::Factory(call) => call
                .validate_input_types(types, count, actual)
                .map_err(Into::into),
            Self::View(call) => call
                .validate_input_types(types, count, actual)
                .map_err(Into::into),
        }
    }
    pub(crate) fn validate_body_type(
        self,
        types: RuntimeProgramTypes<'_>,
    ) -> Result<(), RuntimePresentationResourceLayoutError> {
        match self {
            Self::Factory(call) => call
                .validate_body_type(types)
                .map_err(RuntimePresentationResourceLayoutError::Factory),
            Self::View(call) => call
                .validate_body_type(types)
                .map_err(RuntimePresentationResourceLayoutError::View),
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RuntimePresentationResourceLayoutError {
    #[error(transparent)]
    Factory(#[from] RuntimePresentationFactoryCallError),
    #[error(transparent)]
    View(#[from] RuntimePresentationViewCallError),
}
