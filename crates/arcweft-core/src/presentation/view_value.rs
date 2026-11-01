//! Source-selected retained View construction. Omitted parameters remain absent
//! so the retained View owner can apply its original reactive defaults at mount.

use crate::{
    entry::RuntimeSchemaLimits,
    pattern::RuntimeSemanticTypeId,
    plan::{PlanValueAuthority, RuntimePlanTypeProjection, RuntimeTypeScope},
    program_types::{RuntimeProgramTypes, RuntimeProgramValueType},
    value::AwbcRuntimeValueSnapshotError,
};
use arcweft_id::view::ViewId;
use arcweft_interaction_model::presentation::PresentationResourceKind;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

/// Checked declaration meaning; this is neither a Core function identity nor
/// an executable frame or a grant to mount the View.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "[u8; 32]", into = "[u8; 32]")]
pub struct RuntimePresentationViewIdentity([u8; 32]);

impl RuntimePresentationViewIdentity {
    pub fn from_checked_declaration(
        digest: [u8; 32],
    ) -> Result<Self, RuntimePresentationViewCallError> {
        Self::try_from(digest)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl TryFrom<[u8; 32]> for RuntimePresentationViewIdentity {
    type Error = RuntimePresentationViewCallError;
    fn try_from(value: [u8; 32]) -> Result<Self, Self::Error> {
        if value == [0; 32] {
            return Err(RuntimePresentationViewCallError::ZeroDefinition);
        }
        Ok(Self(value))
    }
}

impl From<RuntimePresentationViewIdentity> for [u8; 32] {
    fn from(value: RuntimePresentationViewIdentity) -> Self {
        value.0
    }
}

/// Exact supplied-parameter destinations in compact ABI order. The tuple owns
/// supplied values only; a missing destination denotes the declaration's own
/// default, rather than a manufactured runtime value.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(try_from = "ViewCallWire")]
pub struct RuntimePresentationViewCall {
    view: ViewId,
    definition: RuntimePresentationViewIdentity,
    parameters: u32,
    supplied: Box<[u32]>,
    resource_type: RuntimeSemanticTypeId,
    body_type: RuntimeSemanticTypeId,
}

impl RuntimePresentationViewCall {
    pub fn try_new(
        view: ViewId,
        definition: RuntimePresentationViewIdentity,
        parameters: u32,
        supplied: Box<[u32]>,
        resource_type: RuntimeSemanticTypeId,
        body_type: RuntimeSemanticTypeId,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimePresentationViewCallError> {
        Self::preflight(
            view.as_str(),
            definition,
            parameters,
            supplied.len(),
            supplied.iter().copied(),
            resource_type,
            body_type,
            limits,
        )?;
        Ok(Self {
            view,
            definition,
            parameters,
            supplied,
            resource_type,
            body_type,
        })
    }

    /// Admit borrowed metadata before allocating a duplicate index or copying
    /// a decoded View identity and parameter row.
    pub(crate) fn preflight(
        view: &str,
        definition: RuntimePresentationViewIdentity,
        parameters: u32,
        supplied_count: usize,
        supplied: impl Iterator<Item = u32> + Clone,
        resource_type: RuntimeSemanticTypeId,
        body_type: RuntimeSemanticTypeId,
        limits: RuntimeSchemaLimits,
    ) -> Result<(), RuntimePresentationViewCallError> {
        if parameters > limits.max_sequence_items || supplied_count > parameters as usize {
            return Err(RuntimePresentationViewCallError::ParameterCount {
                parameters,
                supplied: supplied_count,
                limit: limits.max_sequence_items,
            });
        }
        crate::value::RuntimeRetainedValueAdmission::standalone(limits)
            .validate_metadata(&ViewCallRef {
                view,
                definition,
                parameters,
                supplied: ViewInputs(supplied.clone(), supplied_count),
                resource_type,
                body_type,
            })
            .map_err(|error| RuntimePresentationViewCallError::Resource(Box::new(error)))?;
        let mut seen = BTreeSet::new();
        let mut count = 0;
        for parameter in supplied {
            count += 1;
            if parameter >= parameters || !seen.insert(parameter) {
                return Err(RuntimePresentationViewCallError::Parameter {
                    parameter,
                    parameters,
                });
            }
        }
        if count != supplied_count {
            return Err(RuntimePresentationViewCallError::Arguments {
                expected: supplied_count,
                actual: count,
            });
        }
        Ok(())
    }

    pub fn view(&self) -> &ViewId {
        &self.view
    }
    pub const fn definition(&self) -> RuntimePresentationViewIdentity {
        self.definition
    }
    pub const fn parameters(&self) -> u32 {
        self.parameters
    }
    pub fn supplied(&self) -> &[u32] {
        &self.supplied
    }
    pub const fn resource_type(&self) -> RuntimeSemanticTypeId {
        self.resource_type
    }
    pub const fn body_type(&self) -> RuntimeSemanticTypeId {
        self.body_type
    }

    pub fn retained_body_identity(
        view: &ViewId,
        definition: RuntimePresentationViewIdentity,
        parameters: u32,
        supplied: &[u32],
        resource_type: RuntimeSemanticTypeId,
        scope: &RuntimeTypeScope,
        fields: usize,
        mut field: impl FnMut(usize) -> Option<RuntimeSemanticTypeId>,
    ) -> Result<RuntimeSemanticTypeId, RuntimePresentationViewCallError> {
        if fields != supplied.len() {
            return Err(RuntimePresentationViewCallError::BodyLayout {
                expected: supplied.len(),
                actual: Some(fields),
            });
        }
        let mut hash = blake3::Hasher::new();
        hash.update(b"arcweft.presentation.view-value-body.v1\0");
        hash.update(&(view.as_str().len() as u64).to_le_bytes());
        hash.update(view.as_str().as_bytes());
        hash.update(definition.as_bytes());
        hash.update(&parameters.to_le_bytes());
        hash.update(&(supplied.len() as u64).to_le_bytes());
        for parameter in supplied {
            hash.update(&parameter.to_le_bytes());
        }
        hash.update(resource_type.as_bytes());
        hash.update(&(scope.binders().len() as u64).to_le_bytes());
        for binder in scope.binders() {
            hash.update(&binder.types().to_le_bytes());
            hash.update(&binder.const_lengths().to_le_bytes());
            hash.update(&binder.effects().to_le_bytes());
        }
        for ordinal in 0..fields {
            hash.update(
                field(ordinal)
                    .ok_or(RuntimePresentationViewCallError::BodyField { ordinal })?
                    .as_bytes(),
            );
        }
        Ok(RuntimeSemanticTypeId::from_bytes(
            *hash.finalize().as_bytes(),
        ))
    }

    fn check_layout(
        &self,
        scope: &RuntimeTypeScope,
        count: usize,
        field: impl FnMut(usize) -> Option<RuntimeSemanticTypeId>,
    ) -> Result<(), RuntimePresentationViewCallError> {
        let actual = Self::retained_body_identity(
            &self.view,
            self.definition,
            self.parameters,
            &self.supplied,
            self.resource_type,
            scope,
            count,
            field,
        )?;
        if actual != self.body_type {
            return Err(RuntimePresentationViewCallError::BodyIdentity {
                expected: self.body_type,
                actual,
            });
        }
        Ok(())
    }

    fn plan_body<'a>(
        &self,
        types: PlanValueAuthority<'a>,
    ) -> Result<&'a [crate::runtime_id::RuntimePlanTypeId], RuntimePresentationViewCallError> {
        let resource = types
            .type_for_semantic(self.resource_type)
            .map_err(|error| RuntimePresentationViewCallError::Type(Box::new(error)))?;
        if !matches!(
            resource.projection(),
            RuntimePlanTypeProjection::PresentationResource(PresentationResourceKind::View)
        ) {
            return Err(RuntimePresentationViewCallError::ResourceType);
        }
        let body = types
            .type_for_semantic(self.body_type)
            .map_err(|error| RuntimePresentationViewCallError::Type(Box::new(error)))?;
        let RuntimePlanTypeProjection::Tuple(fields) = body.projection() else {
            return Err(RuntimePresentationViewCallError::BodyLayout {
                expected: self.supplied.len(),
                actual: None,
            });
        };
        self.check_layout(body.scope(), fields.len(), |index| {
            types.get(fields[index]).map(|row| row.semantic_identity())
        })?;
        Ok(fields)
    }

    pub(crate) fn validate_argument_count(
        &self,
        count: usize,
    ) -> Result<(), RuntimePresentationViewCallError> {
        if count != self.supplied.len() {
            return Err(RuntimePresentationViewCallError::Arguments {
                expected: self.supplied.len(),
                actual: count,
            });
        }
        Ok(())
    }

    pub(crate) fn validate_result(
        &self,
        types: RuntimeProgramTypes<'_>,
        result: RuntimeSemanticTypeId,
    ) -> Result<(), RuntimePresentationViewCallError> {
        if result != self.resource_type {
            return Err(RuntimePresentationViewCallError::ResourceType);
        }
        self.validate_body_type(types)
    }

    fn check_input_type(
        &self,
        index: usize,
        expected: RuntimeSemanticTypeId,
        actual: Option<RuntimeSemanticTypeId>,
    ) -> Result<(), RuntimePresentationViewCallError> {
        if actual != Some(expected) {
            return Err(RuntimePresentationViewCallError::InputType {
                index,
                expected,
                actual,
            });
        }
        Ok(())
    }

    pub(crate) fn validate_plan_call(
        &self,
        types: PlanValueAuthority<'_>,
        physical_count: usize,
        mut input: impl FnMut(
            usize,
        ) -> Option<(
            u32,
            crate::value::RuntimeCallArgumentMode,
            RuntimeSemanticTypeId,
        )>,
        result: RuntimeSemanticTypeId,
    ) -> Result<usize, RuntimePresentationViewCallError> {
        use crate::{plan::RuntimePlanTypeProjection as P, value::RuntimeCallArgumentMode as M};
        let fields = self.plan_body(types)?;
        if result != self.resource_type {
            return Err(RuntimePresentationViewCallError::ResourceType);
        }
        let invalid =
            |index, reason| RuntimePresentationViewCallError::InputLayout { index, reason };
        if physical_count > fields.len().saturating_add(1) {
            return Err(invalid(physical_count, "too many physical operands"));
        }
        let mut logical = vec![None; fields.len()];
        let mut written = 0;
        for index in 0..physical_count {
            let (start, mode, semantic) =
                input(index).ok_or_else(|| invalid(index, "missing original input type"))?;
            let row = types
                .type_for_semantic(semantic)
                .map_err(|cause| RuntimePresentationViewCallError::Type(Box::new(cause)))?;
            let width = match mode {
                M::Value => 1,
                M::Spread => match row.projection() {
                    P::Tuple(items) => items.len(),
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
                .ok_or_else(|| invalid(index, "operand range is outside the View capture"))?;
            for (offset, slot) in logical[start..end].iter_mut().enumerate() {
                let semantic = match mode {
                    M::Value => semantic,
                    M::Spread => {
                        let child = match row.projection() {
                            P::Tuple(items) => items[offset],
                            P::Array { item, .. } => *item,
                            _ => unreachable!("spread checked"),
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
        self.validate_argument_count(written)?;
        for (index, field) in fields.iter().enumerate() {
            let expected = types
                .get(*field)
                .ok_or(RuntimePresentationViewCallError::BodyField { ordinal: index })?
                .semantic_identity();
            self.check_input_type(index, expected, logical[index])?;
        }
        Ok(written)
    }

    pub(crate) fn validate_input_types(
        &self,
        types: RuntimeProgramTypes<'_>,
        count: usize,
        mut actual: impl FnMut(usize) -> Option<RuntimeSemanticTypeId>,
    ) -> Result<(), RuntimePresentationViewCallError> {
        self.validate_argument_count(count)?;
        self.validate_body_type(types)?;
        match types {
            RuntimeProgramTypes::Plan(program) => {
                let types = PlanValueAuthority::Inventory(program);
                for (index, field) in self.plan_body(types)?.iter().enumerate() {
                    let expected = types
                        .get(*field)
                        .ok_or(RuntimePresentationViewCallError::BodyField { ordinal: index })?
                        .semantic_identity();
                    self.check_input_type(index, expected, actual(index))?;
                }
            }
            RuntimeProgramTypes::Awbc(program) => {
                let RuntimeProgramValueType::Awbc { ty, .. } = types
                    .value_type(self.body_type)
                    .map_err(|cause| RuntimePresentationViewCallError::Type(Box::new(cause)))?
                else {
                    unreachable!("AWBC authority")
                };
                let crate::awbc::schema::AwbcRuntimeTypeShape::Tuple(fields) =
                    program.runtime_types[ty.index()].shape()
                else {
                    unreachable!("body checked")
                };
                for (index, field) in fields.iter().enumerate() {
                    let expected = program
                        .runtime_types
                        .get(field.index())
                        .ok_or(RuntimePresentationViewCallError::BodyField { ordinal: index })?
                        .semantic_identity();
                    self.check_input_type(index, expected, actual(index))?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn validate_body_type(
        &self,
        types: RuntimeProgramTypes<'_>,
    ) -> Result<(), RuntimePresentationViewCallError> {
        match types {
            RuntimeProgramTypes::Plan(program) => self
                .plan_body(PlanValueAuthority::Inventory(program))
                .map(|_| ()),
            RuntimeProgramTypes::Awbc(program) => {
                let lookup = |semantic| -> Result<_, RuntimePresentationViewCallError> {
                    let RuntimeProgramValueType::Awbc { ty, .. } = types
                        .value_type(semantic)
                        .map_err(|error| RuntimePresentationViewCallError::Type(Box::new(error)))?
                    else {
                        unreachable!("AWBC authority")
                    };
                    Ok(ty)
                };
                if !matches!(
                    program.runtime_types[lookup(self.resource_type)?.index()].shape(),
                    crate::awbc::schema::AwbcRuntimeTypeShape::PresentationResource(
                        PresentationResourceKind::View
                    )
                ) {
                    return Err(RuntimePresentationViewCallError::ResourceType);
                }
                let body = &program.runtime_types[lookup(self.body_type)?.index()];
                let crate::awbc::schema::AwbcRuntimeTypeShape::Tuple(fields) = body.shape() else {
                    return Err(RuntimePresentationViewCallError::BodyLayout {
                        expected: self.supplied.len(),
                        actual: None,
                    });
                };
                self.check_layout(body.scope(), fields.len(), |index| {
                    program
                        .runtime_types
                        .get(fields[index].index())
                        .map(|row| row.semantic_identity())
                })
            }
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RuntimePresentationViewCallError {
    #[error("View value requires {expected} supplied arguments, found {actual}")]
    Arguments { expected: usize, actual: usize },
    #[error("View argument {index} has invalid layout: {reason}")]
    InputLayout { index: usize, reason: &'static str },
    #[error("View argument {index} has type {actual:?}, expected {expected:?}")]
    InputType {
        index: usize,
        expected: RuntimeSemanticTypeId,
        actual: Option<RuntimeSemanticTypeId>,
    },
    #[error("retained View definition identity must not be zero")]
    ZeroDefinition,
    #[error(
        "View parameter row has {supplied} supplied values for {parameters} parameters (limit {limit})"
    )]
    ParameterCount {
        parameters: u32,
        supplied: usize,
        limit: u32,
    },
    #[error(
        "View parameter {parameter} is repeated or outside its {parameters}-parameter declaration"
    )]
    Parameter { parameter: u32, parameters: u32 },
    #[error("retained View metadata exceeds its resource allowance: {0}")]
    Resource(Box<AwbcRuntimeValueSnapshotError>),
    #[error("retained View resource type is not its original ViewValue type")]
    ResourceType,
    #[error("retained View body requires {expected} fields, found {actual:?}")]
    BodyLayout {
        expected: usize,
        actual: Option<usize>,
    },
    #[error("retained View body has no original field type at {ordinal}")]
    BodyField { ordinal: usize },
    #[error("retained View body identity {actual:?} differs from {expected:?}")]
    BodyIdentity {
        expected: RuntimeSemanticTypeId,
        actual: RuntimeSemanticTypeId,
    },
    #[error("retained View type is not owned by its original program: {0}")]
    Type(Box<crate::program_types::RuntimeProgramTypeError>),
}

#[derive(Serialize)]
struct ViewCallRef<'a, I> {
    view: &'a str,
    definition: RuntimePresentationViewIdentity,
    parameters: u32,
    supplied: I,
    resource_type: RuntimeSemanticTypeId,
    body_type: RuntimeSemanticTypeId,
}

struct ViewInputs<I>(I, usize);

impl<I: Iterator<Item = u32> + Clone> Serialize for ViewInputs<I> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut sequence = serializer.serialize_seq(Some(self.1))?;
        for parameter in self.0.clone() {
            sequence.serialize_element(&parameter)?;
        }
        sequence.end()
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ViewCallWire {
    view: ViewId,
    definition: RuntimePresentationViewIdentity,
    parameters: u32,
    supplied: Box<[u32]>,
    resource_type: RuntimeSemanticTypeId,
    body_type: RuntimeSemanticTypeId,
}

impl TryFrom<ViewCallWire> for RuntimePresentationViewCall {
    type Error = RuntimePresentationViewCallError;
    fn try_from(wire: ViewCallWire) -> Result<Self, Self::Error> {
        Self::try_new(
            wire.view,
            wire.definition,
            wire.parameters,
            wire.supplied,
            wire.resource_type,
            wire.body_type,
            RuntimeSchemaLimits::engine_default(),
        )
    }
}

impl Serialize for RuntimePresentationViewCall {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ViewCallRef {
            view: self.view.as_str(),
            definition: self.definition,
            parameters: self.parameters,
            supplied: &self.supplied,
            resource_type: self.resource_type,
            body_type: self.body_type,
        }
        .serialize(serializer)
    }
}
