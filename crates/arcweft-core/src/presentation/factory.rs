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

    /// Resolves the already-admitted source dispositions before executing any
    /// capture. The temporary type slots are bounded by this factory's complete
    /// retained product; source order remains in the original operand vector.
    pub(crate) fn validate_plan_call(
        &self,
        types: crate::plan::PlanValueAuthority<'_>,
        physical_count: usize,
        mut input: impl FnMut(
            usize,
        ) -> Option<(
            u32,
            crate::value::RuntimeCallArgumentMode,
            RuntimeSemanticTypeId,
        )>,
        result: RuntimeSemanticTypeId,
    ) -> Result<usize, RuntimePresentationFactoryCallError> {
        use crate::{plan::RuntimePlanTypeProjection as P, value::RuntimeCallArgumentMode as M};
        let invalid =
            |index, reason| RuntimePresentationFactoryCallError::InputLayout { index, reason };
        self.plan_body(types)?;
        self.plan_result_owner(types, result)?;
        // An admitted disposition has at most one final spread, whose empty
        // tuple may add one physical operand without adding a logical field.
        if physical_count > self.inputs.len().saturating_add(1) {
            return Err(invalid(physical_count, "too many physical operands"));
        }
        let mut logical = vec![None; self.inputs.len()];
        let mut written = 0;
        for index in 0..physical_count {
            let (start, mode, semantic) =
                input(index).ok_or_else(|| invalid(index, "missing original input type"))?;
            let row = types
                .type_for_semantic(semantic)
                .map_err(|cause| RuntimePresentationFactoryCallError::Type(Box::new(cause)))?;
            let width = match mode {
                M::Value => 1,
                M::Spread => match row.projection() {
                    P::Tuple(fields) => fields.len(),
                    P::Array { length, .. } => length
                        .constant()
                        .and_then(|n| usize::try_from(n).ok())
                        .ok_or_else(|| invalid(index, "spread array length is not closed"))?,
                    _ => return Err(invalid(index, "spread requires an admitted tuple or array")),
                },
            };
            let start = usize::try_from(start)
                .map_err(|_| invalid(index, "position exceeds the host index domain"))?;
            let end = start
                .checked_add(width)
                .filter(|end| *end <= logical.len())
                .ok_or_else(|| invalid(index, "operand range is outside the factory product"))?;
            for (offset, slot) in logical[start..end].iter_mut().enumerate() {
                let semantic = match mode {
                    M::Value => semantic,
                    M::Spread => {
                        let child = match row.projection() {
                            P::Tuple(fields) => fields[offset],
                            P::Array { item, .. } => *item,
                            _ => unreachable!("spread shape checked"),
                        };
                        types
                            .get(child)
                            .ok_or_else(|| invalid(index, "spread child has no original type"))?
                            .semantic_identity()
                    }
                };
                if slot.replace(semantic).is_some() {
                    return Err(invalid(index, "operand ranges overlap"));
                }
                written += 1;
            }
        }
        if written != logical.len() {
            return Err(RuntimePresentationFactoryCallError::Arguments {
                expected: logical.len(),
                actual: written,
            });
        }
        self.validate_plan_input_types(types, logical.len(), |index| logical[index])?;
        Ok(logical.len())
    }

    fn validate_plan_input_types(
        &self,
        types: crate::plan::PlanValueAuthority<'_>,
        count: usize,
        mut actual: impl FnMut(usize) -> Option<RuntimeSemanticTypeId>,
    ) -> Result<(), RuntimePresentationFactoryCallError> {
        self.validate_input_count(count)?;
        let fields = self.plan_body(types)?;
        for (ordinal, field) in fields.iter().enumerate() {
            let expected = types
                .get(*field)
                .ok_or(RuntimePresentationFactoryCallError::BodyField { ordinal })?
                .semantic_identity();
            let index = self
                .body_input_index(ordinal)
                .expect("admitted body ordinal");
            self.check_input_type(index, expected, actual(index))?;
        }
        Ok(())
    }

    fn validate_input_count(
        &self,
        count: usize,
    ) -> Result<(), RuntimePresentationFactoryCallError> {
        if count != self.inputs.len() {
            return Err(RuntimePresentationFactoryCallError::Arguments {
                expected: self.inputs.len(),
                actual: count,
            });
        }
        Ok(())
    }

    fn check_input_type(
        &self,
        index: usize,
        expected: RuntimeSemanticTypeId,
        actual: Option<RuntimeSemanticTypeId>,
    ) -> Result<(), RuntimePresentationFactoryCallError> {
        if actual != Some(expected) {
            return Err(RuntimePresentationFactoryCallError::InputType {
                index,
                expected,
                actual,
            });
        }
        Ok(())
    }

    /// Exact source-level type identities remain distinct when their scalar
    /// representations happen to be equal.
    pub(crate) fn validate_input_types(
        &self,
        types: crate::program_types::RuntimeProgramTypes<'_>,
        count: usize,
        mut actual: impl FnMut(usize) -> Option<RuntimeSemanticTypeId>,
    ) -> Result<(), RuntimePresentationFactoryCallError> {
        use crate::{
            awbc::schema::AwbcRuntimeTypeShape,
            program_types::{RuntimeProgramTypes, RuntimeProgramValueType},
        };
        match types {
            RuntimeProgramTypes::Plan(program) => self.validate_plan_input_types(
                crate::plan::PlanValueAuthority::Inventory(program),
                count,
                actual,
            ),
            RuntimeProgramTypes::Awbc(program) => {
                self.validate_input_count(count)?;
                self.validate_body_type(types)?;
                let RuntimeProgramValueType::Awbc { ty, .. } = types
                    .value_type(self.body_type)
                    .map_err(|cause| RuntimePresentationFactoryCallError::Type(Box::new(cause)))?
                else {
                    unreachable!("AWBC authority returns AWBC rows")
                };
                let AwbcRuntimeTypeShape::Tuple(fields) = program.runtime_types[ty.index()].shape()
                else {
                    unreachable!("body layout checked")
                };
                for (ordinal, field) in fields.iter().enumerate() {
                    let expected = program
                        .runtime_types
                        .get(field.index())
                        .ok_or(RuntimePresentationFactoryCallError::BodyField { ordinal })?
                        .semantic_identity();
                    let index = self
                        .body_input_index(ordinal)
                        .expect("admitted body ordinal");
                    self.check_input_type(index, expected, actual(index))?;
                }
                Ok(())
            }
        }
    }

    fn plan_result_owner(
        &self,
        types: crate::plan::PlanValueAuthority<'_>,
        result: RuntimeSemanticTypeId,
    ) -> Result<crate::pattern::RuntimeOpaqueTypeOwner, RuntimePresentationFactoryCallError> {
        use crate::{pattern::RuntimeOpaqueTypeOwner, plan::RuntimePlanTypeProjection as P};
        let expected = RuntimeOpaqueTypeOwner::presentation_handle(self.resource(), result);
        let row = types
            .type_for_semantic(result)
            .map_err(|cause| RuntimePresentationFactoryCallError::Type(Box::new(cause)))?;
        if !matches!(row.projection(), P::Opaque { producer, admission, value_class, persistence, arguments }
            if producer == expected.producer() && *admission == expected.admission() && *value_class == expected.value_class() && *persistence == expected.persistence() && arguments.is_empty())
        {
            return Err(RuntimePresentationFactoryCallError::ResultType { result });
        }
        Ok(expected)
    }

    pub(crate) fn result_owner(
        &self,
        types: crate::program_types::RuntimeProgramTypes<'_>,
        result: RuntimeSemanticTypeId,
    ) -> Result<crate::pattern::RuntimeOpaqueTypeOwner, RuntimePresentationFactoryCallError> {
        use crate::{
            awbc::schema::AwbcRuntimeTypeShape as P,
            pattern::RuntimeOpaqueTypeOwner,
            program_types::{RuntimeProgramTypes, RuntimeProgramValueType},
        };
        match types {
            RuntimeProgramTypes::Plan(program) => {
                self.plan_result_owner(crate::plan::PlanValueAuthority::Inventory(program), result)
            }
            RuntimeProgramTypes::Awbc(program) => {
                let expected = RuntimeOpaqueTypeOwner::presentation_handle(self.resource(), result);
                let RuntimeProgramValueType::Awbc { ty, .. } = types
                    .value_type(result)
                    .map_err(|cause| RuntimePresentationFactoryCallError::Type(Box::new(cause)))?
                else {
                    unreachable!("AWBC authority returns AWBC rows")
                };
                if !matches!(program.runtime_types[ty.index()].shape(), P::Opaque { producer, admission, value_class, persistence, arguments }
                    if program.strings.get(producer.index()).is_some_and(|producer| producer == expected.producer().as_str()) && *admission == expected.admission() && *value_class == expected.value_class() && *persistence == expected.persistence() && arguments.is_empty())
                {
                    return Err(RuntimePresentationFactoryCallError::ResultType { result });
                }
                Ok(expected)
            }
        }
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

    fn check_resource_kind(
        &self,
        actual: Option<PresentationResourceKind>,
    ) -> Result<(), RuntimePresentationFactoryCallError> {
        if actual != Some(self.resource()) {
            return Err(RuntimePresentationFactoryCallError::ResourceType {
                expected: self.resource(),
                actual,
            });
        }
        Ok(())
    }

    fn check_body_layout(
        &self,
        scope: &crate::plan::RuntimeTypeScope,
        count: usize,
        field: impl FnMut(usize) -> Option<RuntimeSemanticTypeId>,
    ) -> Result<(), RuntimePresentationFactoryCallError> {
        if count != self.body_len() {
            return Err(RuntimePresentationFactoryCallError::BodyLayout {
                expected: self.body_len(),
                actual: Some(count),
            });
        }
        let actual = Self::retained_body_identity(
            self.callable,
            &self.inputs,
            self.resource_type,
            scope,
            count,
            field,
        )?;
        if actual != self.body_type {
            return Err(RuntimePresentationFactoryCallError::BodyIdentity {
                expected: self.body_type,
                actual,
            });
        }
        Ok(())
    }

    fn plan_body<'a>(
        &self,
        types: crate::plan::PlanValueAuthority<'a>,
    ) -> Result<&'a [crate::runtime_id::RuntimePlanTypeId], RuntimePresentationFactoryCallError>
    {
        use crate::plan::RuntimePlanTypeProjection as P;
        let resource = types
            .type_for_semantic(self.resource_type)
            .map_err(|cause| RuntimePresentationFactoryCallError::Type(Box::new(cause)))?;
        self.check_resource_kind(match resource.projection() {
            P::PresentationResource(kind) => Some(*kind),
            _ => None,
        })?;
        let body = types
            .type_for_semantic(self.body_type)
            .map_err(|cause| RuntimePresentationFactoryCallError::Type(Box::new(cause)))?;
        let P::Tuple(fields) = body.projection() else {
            return Err(RuntimePresentationFactoryCallError::BodyLayout {
                expected: self.body_len(),
                actual: None,
            });
        };
        self.check_body_layout(body.scope(), fields.len(), |ordinal| {
            types
                .get(fields[ordinal])
                .map(|row| row.semantic_identity())
        })?;
        Ok(fields)
    }

    pub(crate) fn validate_body_type(
        &self,
        types: crate::program_types::RuntimeProgramTypes<'_>,
    ) -> Result<(), RuntimePresentationFactoryCallError> {
        use crate::{
            awbc::schema::AwbcRuntimeTypeShape as P,
            program_types::{RuntimeProgramTypes, RuntimeProgramValueType},
        };
        match types {
            RuntimeProgramTypes::Plan(program) => self
                .plan_body(crate::plan::PlanValueAuthority::Inventory(program))
                .map(|_| ()),
            RuntimeProgramTypes::Awbc(program) => {
                let lookup = |semantic| -> Result<_, RuntimePresentationFactoryCallError> {
                    let RuntimeProgramValueType::Awbc { ty, .. } =
                        types.value_type(semantic).map_err(|cause| {
                            RuntimePresentationFactoryCallError::Type(Box::new(cause))
                        })?
                    else {
                        unreachable!("AWBC authority returns AWBC rows")
                    };
                    Ok(ty)
                };
                let resource = &program.runtime_types[lookup(self.resource_type)?.index()];
                self.check_resource_kind(match resource.shape() {
                    P::PresentationResource(kind) => Some(*kind),
                    _ => None,
                })?;
                let body = &program.runtime_types[lookup(self.body_type)?.index()];
                let P::Tuple(fields) = body.shape() else {
                    return Err(RuntimePresentationFactoryCallError::BodyLayout {
                        expected: self.body_len(),
                        actual: None,
                    });
                };
                self.check_body_layout(body.scope(), fields.len(), |ordinal| {
                    program
                        .runtime_types
                        .get(fields[ordinal].index())
                        .map(|row| row.semantic_identity())
                })
            }
        }
    }
}
#[derive(Clone, Debug, Error, PartialEq)]
pub enum RuntimePresentationFactoryCallError {
    #[error("presentation argument {index} has an invalid executable ABI: {reason}")]
    InputLayout { index: usize, reason: &'static str },
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
