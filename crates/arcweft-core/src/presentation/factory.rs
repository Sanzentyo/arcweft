//! Typed input and retained-body layout of one presentation factory.
//!
//! Evaluation order belongs to the checked expression operands. This row is
//! their compact ABI and contains no source-name resolver or execution grant.

use crate::{entry::RuntimeSchemaLimits, pattern::RuntimeSemanticTypeId};
use arcweft_interaction_model::presentation::{
    PresentationArgumentKey, PresentationArgumentRole, PresentationCallableId,
    PresentationCallableResult, PresentationResourceKind,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

/// Factory identity, exact input destinations and the original program's
/// retained tuple type. The tuple excludes the borrowed scope selector: scope
/// controls custody and is not a field owned by the displayed resource.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(try_from = "FactoryCallWire")]
pub struct RuntimePresentationFactoryCall {
    callable: PresentationCallableId,
    inputs: Box<[PresentationArgumentKey]>,
    resource_type: RuntimeSemanticTypeId,
    body_type: RuntimeSemanticTypeId,
    scope_input: Option<usize>,
}

impl RuntimePresentationFactoryCall {
    /// Admits the complete borrowed metadata under the caller-selected policy
    /// before building the duplicate-key index or publishing the owned row.
    pub fn try_new(
        callable: PresentationCallableId,
        inputs: Box<[PresentationArgumentKey]>,
        resource_type: RuntimeSemanticTypeId,
        body_type: RuntimeSemanticTypeId,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimePresentationFactoryCallError> {
        if !matches!(callable.result(), PresentationCallableResult::Handle(_)) {
            return Err(RuntimePresentationFactoryCallError::NotFactory { callable });
        }
        let wire = FactoryCallRef {
            callable,
            inputs: &inputs,
            resource_type,
            body_type,
        };
        crate::value::RuntimeRetainedValueAdmission::standalone(limits)
            .validate_metadata(&wire)
            .map_err(|cause| RuntimePresentationFactoryCallError::Resource(Box::new(cause)))?;
        let mut seen = BTreeSet::new();
        let mut scope_input = None;
        for (index, key) in inputs.iter().enumerate() {
            if !callable.accepts_argument_role(key.role()) {
                return Err(RuntimePresentationFactoryCallError::Role {
                    callable,
                    role: key.role(),
                });
            }
            if !seen.insert(key) {
                return Err(RuntimePresentationFactoryCallError::Duplicate { index });
            }
            if key.role() == PresentationArgumentRole::Scope {
                scope_input = Some(index);
            }
        }
        Ok(Self {
            callable,
            inputs,
            resource_type,
            body_type,
            scope_input,
        })
    }

    pub const fn callable(&self) -> PresentationCallableId {
        self.callable
    }

    pub fn resource(&self) -> PresentationResourceKind {
        let PresentationCallableResult::Handle(resource) = self.callable.result() else {
            unreachable!("factory construction checks the result family")
        };
        resource
    }

    pub fn inputs(&self) -> &[PresentationArgumentKey] {
        &self.inputs
    }

    pub const fn body_type(&self) -> RuntimeSemanticTypeId {
        self.body_type
    }

    pub const fn resource_type(&self) -> RuntimeSemanticTypeId {
        self.resource_type
    }

    /// Tuple fields are selected from the same input inventory; no copied
    /// index or independently editable body-role table can disagree with it.
    pub fn body_inputs(&self) -> impl Iterator<Item = (usize, &PresentationArgumentKey)> {
        self.inputs
            .iter()
            .enumerate()
            .filter(|(_, key)| key.role() != PresentationArgumentRole::Scope)
    }

    pub fn scope_input(&self) -> Option<usize> {
        self.scope_input
    }

    pub fn body_len(&self) -> usize {
        self.inputs.len() - usize::from(self.scope_input().is_some())
    }

    pub fn body_input_index(&self, ordinal: usize) -> Option<usize> {
        if ordinal >= self.body_len() {
            return None;
        }
        Some(ordinal + usize::from(self.scope_input().is_some_and(|scope| scope <= ordinal)))
    }

    pub(crate) fn validate_body_type(
        &self,
        types: crate::program_types::RuntimeProgramTypes<'_>,
    ) -> Result<(), RuntimePresentationFactoryCallError> {
        use crate::{
            awbc::schema::AwbcRuntimeTypeShape, plan::RuntimePlanTypeProjection,
            program_types::RuntimeProgramValueType,
        };
        let kind = match types
            .value_type(self.resource_type)
            .map_err(|cause| RuntimePresentationFactoryCallError::Type(Box::new(cause)))?
        {
            RuntimeProgramValueType::Plan { program, ty } => {
                match program.type_table().get(ty).map(|row| row.projection()) {
                    Some(RuntimePlanTypeProjection::PresentationResource(kind)) => Some(*kind),
                    _ => None,
                }
            }
            RuntimeProgramValueType::Awbc { program, ty } => {
                match program.runtime_types.get(ty.index()).map(|row| row.shape()) {
                    Some(AwbcRuntimeTypeShape::PresentationResource(kind)) => Some(*kind),
                    _ => None,
                }
            }
        };
        if kind != Some(self.resource()) {
            return Err(RuntimePresentationFactoryCallError::ResourceType {
                expected: self.resource(),
                actual: kind,
            });
        }
        let length = match types
            .value_type(self.body_type)
            .map_err(|cause| RuntimePresentationFactoryCallError::Type(Box::new(cause)))?
        {
            RuntimeProgramValueType::Plan { program, ty } => {
                match program.type_table().get(ty).map(|ty| ty.projection()) {
                    Some(RuntimePlanTypeProjection::Tuple(items)) => Some(items.len()),
                    _ => None,
                }
            }
            RuntimeProgramValueType::Awbc { program, ty } => {
                match program.runtime_types.get(ty.index()).map(|ty| ty.shape()) {
                    Some(AwbcRuntimeTypeShape::Tuple(items)) => Some(items.len()),
                    _ => None,
                }
            }
        };
        if length != Some(self.body_len()) {
            return Err(RuntimePresentationFactoryCallError::BodyLayout {
                expected: self.body_len(),
                actual: length,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum RuntimePresentationFactoryCallError {
    #[error("presentation factory requires resource type {expected:?}, found {actual:?}")]
    ResourceType {
        expected: PresentationResourceKind,
        actual: Option<PresentationResourceKind>,
    },
    #[error("presentation callable {callable:?} does not construct a handle")]
    NotFactory { callable: PresentationCallableId },
    #[error("presentation callable {callable:?} does not admit input role {role:?}")]
    Role {
        callable: PresentationCallableId,
        role: PresentationArgumentRole,
    },
    #[error("duplicate presentation argument at ABI position {index}")]
    Duplicate { index: usize },
    #[error("presentation factory metadata exceeds its resource allowance: {0}")]
    Resource(Box<crate::value::AwbcRuntimeValueSnapshotError>),
    #[error("presentation body requires a tuple of {expected} fields, found {actual:?}")]
    BodyLayout {
        expected: usize,
        actual: Option<usize>,
    },
    #[error("presentation body type does not belong to its original program: {0}")]
    Type(Box<crate::program_types::RuntimeProgramTypeError>),
}

#[derive(Serialize)]
struct FactoryCallRef<'a> {
    callable: PresentationCallableId,
    inputs: &'a [PresentationArgumentKey],
    resource_type: RuntimeSemanticTypeId,
    body_type: RuntimeSemanticTypeId,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FactoryCallWire {
    callable: PresentationCallableId,
    inputs: Box<[PresentationArgumentKey]>,
    resource_type: RuntimeSemanticTypeId,
    body_type: RuntimeSemanticTypeId,
}

impl TryFrom<FactoryCallWire> for RuntimePresentationFactoryCall {
    type Error = RuntimePresentationFactoryCallError;

    fn try_from(wire: FactoryCallWire) -> Result<Self, Self::Error> {
        Self::try_new(
            wire.callable,
            wire.inputs,
            wire.resource_type,
            wire.body_type,
            RuntimeSchemaLimits::engine_default(),
        )
    }
}

impl Serialize for RuntimePresentationFactoryCall {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        FactoryCallRef {
            callable: self.callable,
            inputs: &self.inputs,
            resource_type: self.resource_type,
            body_type: self.body_type,
        }
        .serialize(serializer)
    }
}
