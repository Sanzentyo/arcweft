//! Typed input and retained-body layout of one presentation factory.
//!
//! Evaluation order belongs to the checked expression operands. This row is
//! their compact ABI and contains no source-name resolver or execution grant.

use crate::{entry::RuntimeSchemaLimits, pattern::RuntimeSemanticTypeId};
use arcweft_interaction_model::presentation::{
    PresentationArgumentKey, PresentationArgumentKeyRef, PresentationArgumentRole,
    PresentationCallableId, PresentationCallableResult, PresentationResourceKind,
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
    /// Apply the same metadata grammar as the owned row to a bounded borrowed
    /// input stream. Decoders call this before copying keys or allocating rows.
    pub(crate) fn preflight_inputs<'a>(
        callable: PresentationCallableId,
        inputs: impl Iterator<Item = PresentationArgumentKeyRef<'a>> + Clone,
        resource_type: RuntimeSemanticTypeId,
        body_type: RuntimeSemanticTypeId,
        limits: RuntimeSchemaLimits,
    ) -> Result<Option<usize>, RuntimePresentationFactoryCallError> {
        if !matches!(callable.result(), PresentationCallableResult::Handle(_)) {
            return Err(RuntimePresentationFactoryCallError::NotFactory { callable });
        }
        crate::value::RuntimeRetainedValueAdmission::standalone(limits)
            .validate_metadata(&FactoryCallRef {
                callable,
                inputs: FactoryInputs(inputs.clone()),
                resource_type,
                body_type,
            })
            .map_err(|cause| RuntimePresentationFactoryCallError::Resource(Box::new(cause)))?;
        let mut seen = BTreeSet::new();
        let mut scope_input = None;
        for (index, key) in inputs.enumerate() {
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
        Ok(scope_input)
    }

    /// Canonical identity of the retained product, including its destinations.
    /// Equal physical tuple shapes cannot exchange metadata roles or keys.
    /// Child lookup borrows the selected type graph and allocates no side table.
    pub fn retained_body_identity(
        callable: PresentationCallableId,
        inputs: &[PresentationArgumentKey],
        resource_type: RuntimeSemanticTypeId,
        scope: &crate::plan::RuntimeTypeScope,
        fields: usize,
        mut field: impl FnMut(usize) -> Option<RuntimeSemanticTypeId>,
    ) -> Result<RuntimeSemanticTypeId, RuntimePresentationFactoryCallError> {
        let mut hash = blake3::Hasher::new();
        hash.update(b"arcweft.presentation.retained-product.v1\0");
        hash.update(&[callable.semantic_tag()]);
        hash.update(resource_type.as_bytes());
        hash.update(&(inputs.len() as u64).to_le_bytes());
        for key in inputs {
            hash.update(&[
                key.role().semantic_tag(),
                u8::from(key.parameter().is_some()),
            ]);
            if let Some(parameter) = key.parameter() {
                hash.update(&(parameter.len() as u64).to_le_bytes());
                hash.update(parameter.as_bytes());
            }
        }
        hash.update(&(scope.binders().len() as u64).to_le_bytes());
        for binder in scope.binders() {
            hash.update(&binder.types().to_le_bytes());
            hash.update(&binder.const_lengths().to_le_bytes());
            hash.update(&binder.effects().to_le_bytes());
        }
        hash.update(&(fields as u64).to_le_bytes());
        for ordinal in 0..fields {
            let identity =
                field(ordinal).ok_or(RuntimePresentationFactoryCallError::BodyField { ordinal })?;
            hash.update(identity.as_bytes());
        }
        Ok(RuntimeSemanticTypeId::from_bytes(
            *hash.finalize().as_bytes(),
        ))
    }

    /// Admits the complete borrowed metadata under the caller-selected policy
    /// before building the duplicate-key index or publishing the owned row.
    pub fn try_new(
        callable: PresentationCallableId,
        inputs: Box<[PresentationArgumentKey]>,
        resource_type: RuntimeSemanticTypeId,
        body_type: RuntimeSemanticTypeId,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimePresentationFactoryCallError> {
        let scope_input = Self::preflight_inputs(
            callable,
            inputs.iter().map(PresentationArgumentKey::as_borrowed),
            resource_type,
            body_type,
            limits,
        )?;
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

    pub(crate) fn validate_argument_count(
        &self,
        count: usize,
    ) -> Result<(), RuntimePresentationFactoryCallError> {
        if count != self.inputs.len() {
            return Err(RuntimePresentationFactoryCallError::Arguments {
                expected: self.inputs.len(),
                actual: count,
            });
        }
        // A public scope name is not a dynamic lexical grant. Until the Source
        // capability producer is connected, reject rather than minting custody.
        if self.scope_input.is_some() {
            return Err(RuntimePresentationFactoryCallError::ExplicitScope);
        }
        Ok(())
    }

    /// Exact source-level type identities are part of this ABI even when two
    /// rows admit the same physical scalar representation.
    pub(crate) fn validate_input_types(
        &self,
        types: crate::program_types::RuntimeProgramTypes<'_>,
        count: usize,
        mut actual: impl FnMut(usize) -> Option<RuntimeSemanticTypeId>,
    ) -> Result<(), RuntimePresentationFactoryCallError> {
        use crate::{
            awbc::schema::AwbcRuntimeTypeShape, plan::RuntimePlanTypeProjection,
            program_types::RuntimeProgramValueType,
        };
        if count != self.inputs.len() {
            return Err(RuntimePresentationFactoryCallError::Arguments {
                expected: self.inputs.len(),
                actual: count,
            });
        }
        self.validate_body_type(types)?;
        let body = types
            .value_type(self.body_type)
            .map_err(|cause| RuntimePresentationFactoryCallError::Type(Box::new(cause)))?;
        for ordinal in 0..self.body_len() {
            let expected = match body {
                RuntimeProgramValueType::Plan { program, ty } => {
                    match program.type_table().get(ty).map(|row| row.projection()) {
                        Some(RuntimePlanTypeProjection::Tuple(fields)) => fields
                            .get(ordinal)
                            .and_then(|ty| program.type_table().get(*ty))
                            .map(|row| row.semantic_identity()),
                        _ => None,
                    }
                }
                RuntimeProgramValueType::Awbc { program, ty } => {
                    match program.runtime_types.get(ty.index()).map(|row| row.shape()) {
                        Some(AwbcRuntimeTypeShape::Tuple(fields)) => fields
                            .get(ordinal)
                            .and_then(|ty| program.runtime_types.get(ty.index()))
                            .map(|row| row.semantic_identity()),
                        _ => None,
                    }
                }
            }
            .ok_or(RuntimePresentationFactoryCallError::BodyField { ordinal })?;
            let index = self
                .body_input_index(ordinal)
                .expect("admitted body ordinal");
            let actual = actual(index);
            if actual != Some(expected) {
                return Err(RuntimePresentationFactoryCallError::InputType {
                    index,
                    expected,
                    actual,
                });
            }
        }
        Ok(())
    }

    pub(crate) fn result_owner(
        &self,
        types: crate::program_types::RuntimeProgramTypes<'_>,
        result: RuntimeSemanticTypeId,
    ) -> Result<crate::pattern::RuntimeOpaqueTypeOwner, RuntimePresentationFactoryCallError> {
        use crate::{
            awbc::schema::AwbcRuntimeTypeShape, pattern::RuntimeOpaqueTypeOwner,
            plan::RuntimePlanTypeProjection, program_types::RuntimeProgramValueType,
        };
        let expected = RuntimeOpaqueTypeOwner::presentation_handle(self.resource(), result);
        let valid = match types
            .value_type(result)
            .map_err(|cause| RuntimePresentationFactoryCallError::Type(Box::new(cause)))?
        {
            RuntimeProgramValueType::Plan { program, ty } => matches!(
                program.type_table().get(ty).map(|row| row.projection()),
                Some(RuntimePlanTypeProjection::Opaque { producer, admission, value_class, persistence, arguments })
                    if producer == expected.producer() && *admission == expected.admission()
                        && *value_class == expected.value_class() && *persistence == expected.persistence()
                        && arguments.is_empty()
            ),
            RuntimeProgramValueType::Awbc { program, ty } => matches!(
                program.runtime_types.get(ty.index()).map(|row| row.shape()),
                Some(AwbcRuntimeTypeShape::Opaque { producer, admission, value_class, persistence, arguments })
                    if program.strings.get(producer.index()).is_some_and(|producer| producer == expected.producer().as_str())
                        && *admission == expected.admission() && *value_class == expected.value_class()
                        && *persistence == expected.persistence() && arguments.is_empty()
            ),
        };
        if !valid {
            return Err(RuntimePresentationFactoryCallError::ResultType { result });
        }
        Ok(expected)
    }

    pub(crate) fn registration_site(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        body: &crate::value::RuntimeValue,
    ) -> Result<
        super::resources::PresentationRegistrationSite,
        crate::value::AwbcRuntimeValueSnapshotError,
    > {
        use super::{
            RuntimePresentationError, RuntimePresentationSlotRefInput,
            RuntimePresentationSlotSelectors,
        };
        use arcweft_interaction_model::presentation::{
            PresentationCallableOperation, PresentationFactoryPlacement, PresentationSlotAddress,
        };
        let PresentationCallableOperation::Construct {
            resource,
            placement,
        } = self.callable.operation()
        else {
            unreachable!("factory result was admitted")
        };
        if placement == PresentationFactoryPlacement::Independent {
            return Ok(super::resources::PresentationRegistrationSite::Independent(
                resource,
            ));
        }
        let crate::value::RuntimeValue::Tuple(values) = body else {
            return Err(RuntimePresentationError::InvalidState.into());
        };
        if values.len() != self.body_len() {
            return Err(RuntimePresentationError::InvalidState.into());
        }
        let selectors = self
            .body_inputs()
            .enumerate()
            .filter_map(|(ordinal, (_, key))| {
                RuntimePresentationSlotRefInput::try_from(key.role())
                    .ok()
                    .map(|role| (role, &values[ordinal]))
            });
        RuntimePresentationSlotSelectors::from_values(selectors)?.with_parts(
            resource,
            admission,
            |admission, target, slot| {
                PresentationSlotAddress::validate_text(target, slot, resource)
                    .map_err(RuntimePresentationError::from)?;
                admission.validate_metadata_string(target)?;
                admission.validate_metadata_string(slot)?;
                let address = PresentationSlotAddress::try_new(
                    arcweft_id::PublicId::try_new(target.to_owned())
                        .expect("borrowed target validated"),
                    arcweft_id::PublicId::try_new(slot.to_owned())
                        .expect("borrowed slot validated"),
                    resource,
                )
                .expect("borrowed slot address validated");
                Ok(super::resources::PresentationRegistrationSite::Slot(
                    address,
                ))
            },
        )
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
        let (length, identity) = match types
            .value_type(self.body_type)
            .map_err(|cause| RuntimePresentationFactoryCallError::Type(Box::new(cause)))?
        {
            RuntimeProgramValueType::Plan { program, ty } => {
                match program.type_table().get(ty).map(|ty| ty.projection()) {
                    Some(RuntimePlanTypeProjection::Tuple(items)) => {
                        if items.len() != self.body_len() {
                            return Err(RuntimePresentationFactoryCallError::BodyLayout {
                                expected: self.body_len(),
                                actual: Some(items.len()),
                            });
                        }
                        let row = program.type_table().get(ty).expect("selected body row");
                        let identity = Self::retained_body_identity(
                            self.callable,
                            &self.inputs,
                            self.resource_type,
                            row.scope(),
                            items.len(),
                            |ordinal| {
                                program
                                    .type_table()
                                    .get(items[ordinal])
                                    .map(|row| row.semantic_identity())
                            },
                        )?;
                        (Some(items.len()), Some(identity))
                    }
                    _ => (None, None),
                }
            }
            RuntimeProgramValueType::Awbc { program, ty } => {
                match program.runtime_types.get(ty.index()).map(|ty| ty.shape()) {
                    Some(AwbcRuntimeTypeShape::Tuple(items)) => {
                        if items.len() != self.body_len() {
                            return Err(RuntimePresentationFactoryCallError::BodyLayout {
                                expected: self.body_len(),
                                actual: Some(items.len()),
                            });
                        }
                        let row = &program.runtime_types[ty.index()];
                        let identity = Self::retained_body_identity(
                            self.callable,
                            &self.inputs,
                            self.resource_type,
                            row.scope(),
                            items.len(),
                            |ordinal| {
                                program
                                    .runtime_types
                                    .get(items[ordinal].index())
                                    .map(|row| row.semantic_identity())
                            },
                        )?;
                        (Some(items.len()), Some(identity))
                    }
                    _ => (None, None),
                }
            }
        };
        if length != Some(self.body_len()) {
            return Err(RuntimePresentationFactoryCallError::BodyLayout {
                expected: self.body_len(),
                actual: length,
            });
        }
        if identity != Some(self.body_type) {
            return Err(RuntimePresentationFactoryCallError::BodyIdentity {
                expected: self.body_type,
                actual: identity.expect("tuple arity checked"),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum RuntimePresentationFactoryCallError {
    #[error("presentation input {index} requires type {expected:?}, found {actual:?}")]
    InputType {
        index: usize,
        expected: RuntimeSemanticTypeId,
        actual: Option<RuntimeSemanticTypeId>,
    },
    #[error("presentation factory expected {expected} inputs, found {actual}")]
    Arguments { expected: usize, actual: usize },
    #[error(
        "presentation factory requires an actual lexical scope capability for an explicit scope"
    )]
    ExplicitScope,
    #[error("presentation factory has an invalid Handle result type {result:?}")]
    ResultType { result: RuntimeSemanticTypeId },
    #[error("presentation retained product has no field type at ordinal {ordinal}")]
    BodyField { ordinal: usize },
    #[error(
        "presentation retained product identity {expected:?} disagrees with its layout {actual:?}"
    )]
    BodyIdentity {
        expected: RuntimeSemanticTypeId,
        actual: RuntimeSemanticTypeId,
    },
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
struct FactoryCallRef<I> {
    callable: PresentationCallableId,
    inputs: I,
    resource_type: RuntimeSemanticTypeId,
    body_type: RuntimeSemanticTypeId,
}

struct FactoryInputs<I>(I);

impl<'a, I> Serialize for FactoryInputs<I>
where
    I: Iterator<Item = PresentationArgumentKeyRef<'a>> + Clone,
{
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut sequence = serializer.serialize_seq(self.0.size_hint().1)?;
        for input in self.0.clone() {
            sequence.serialize_element(&input)?;
        }
        sequence.end()
    }
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
