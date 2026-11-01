//! Bounded selector ABI shared by inert addresses and bank-owned slot removal.

use super::RuntimePresentationError;
use arcweft_interaction_model::presentation::{PresentationArgumentRole, PresentationResourceKind};
use serde::{Deserialize, Deserializer, Serialize, de};
use std::fmt;

/// Closed selector grammar of a slot-address or removal operation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePresentationSlotInput {
    Target,
    Slot,
    Character,
    Scope,
}

impl RuntimePresentationSlotInput {
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Target => 0,
            Self::Slot => 1,
            Self::Character => 2,
            Self::Scope => 3,
        }
    }

    pub const fn from_semantic_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Target),
            1 => Some(Self::Slot),
            2 => Some(Self::Character),
            3 => Some(Self::Scope),
            _ => None,
        }
    }
}

impl TryFrom<PresentationArgumentRole> for RuntimePresentationSlotInput {
    type Error = RuntimePresentationError;

    fn try_from(role: PresentationArgumentRole) -> Result<Self, Self::Error> {
        match role {
            PresentationArgumentRole::TargetEntity => Ok(Self::Target),
            PresentationArgumentRole::Slot => Ok(Self::Slot),
            PresentationArgumentRole::Character => Ok(Self::Character),
            PresentationArgumentRole::Scope => Ok(Self::Scope),
            _ => Err(RuntimePresentationError::SlotReferenceRole { role }),
        }
    }
}

/// The effect selected by one checked slot call. Only removal requires the
/// original execution bank; an address never grants custody of an occupant.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePresentationSlotOperation {
    Reference,
    Remove,
}

impl RuntimePresentationSlotOperation {
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Reference => 0,
            Self::Remove => 1,
        }
    }
    pub const fn from_semantic_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Reference),
            1 => Some(Self::Remove),
            _ => None,
        }
    }
}

/// The role sequence is the exact compact call ABI, independent of the order
/// in which the caller evaluates authored expressions. There are at most four
/// unique roles, retained inline by both live construction and decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimePresentationSlotCall {
    operation: RuntimePresentationSlotOperation,
    resource: PresentationResourceKind,
    inputs: [RuntimePresentationSlotInput; 4],
    length: u8,
}

impl RuntimePresentationSlotCall {
    pub fn try_new(
        operation: RuntimePresentationSlotOperation,
        resource: PresentationResourceKind,
        inputs: impl IntoIterator<Item = RuntimePresentationSlotInput>,
    ) -> Result<Self, RuntimePresentationError> {
        let mut roles = [RuntimePresentationSlotInput::Target; 4];
        let mut length = 0;
        let mut seen = 0_u8;
        for role in inputs {
            let bit = 1_u8 << role.semantic_tag();
            if length == roles.len()
                || seen & bit != 0
                || (role == RuntimePresentationSlotInput::Character
                    && resource != PresentationResourceKind::Character)
                || (role == RuntimePresentationSlotInput::Scope
                    && operation == RuntimePresentationSlotOperation::Reference)
            {
                return Err(RuntimePresentationError::SlotReferenceArguments);
            }
            seen |= bit;
            roles[length] = role;
            length += 1;
        }
        Ok(Self {
            operation,
            resource,
            inputs: roles,
            length: length as u8,
        })
    }

    pub const fn operation(&self) -> RuntimePresentationSlotOperation {
        self.operation
    }
    pub const fn is_reference(&self) -> bool {
        matches!(self.operation, RuntimePresentationSlotOperation::Reference)
    }

    pub(crate) fn validate_execution(&self, count: usize) -> Result<(), RuntimePresentationError> {
        if count != self.inputs().len() {
            return Err(RuntimePresentationError::SlotReferenceArguments);
        }
        if self.inputs().contains(&RuntimePresentationSlotInput::Scope) {
            return Err(RuntimePresentationError::ScopeCapabilityRequired);
        }
        Ok(())
    }

    pub const fn resource(&self) -> PresentationResourceKind {
        self.resource
    }

    pub fn inputs(&self) -> &[RuntimePresentationSlotInput] {
        &self.inputs[..usize::from(self.length)]
    }

    pub(crate) fn accepts_plan_result<T>(
        &self,
        shape: &crate::plan::RuntimePlanTypeProjection<T>,
        item_resource: impl FnOnce(&T) -> Option<PresentationResourceKind>,
    ) -> bool {
        if self.operation == RuntimePresentationSlotOperation::Remove {
            return matches!(shape, crate::plan::RuntimePlanTypeProjection::Option { item, .. }
                if item_resource(item) == Some(self.resource));
        }
        use crate::{
            pattern::RuntimeOpaqueTypeAdmission,
            value::{RuntimeOpaquePersistence, RuntimeOpaqueValueClass},
        };
        matches!(shape, crate::plan::RuntimePlanTypeProjection::Opaque { producer, admission, value_class, persistence, arguments }
            if producer.as_str() == crate::pattern::RuntimeOpaqueTypeOwner::presentation_slot_ref_producer(self.resource)
                && *admission == RuntimeOpaqueTypeAdmission::ExactIdentity
                && *value_class == RuntimeOpaqueValueClass::Plain
                && *persistence == RuntimeOpaquePersistence::ConstantAndSnapshot
                && arguments.is_empty())
    }

    pub(crate) fn accepts_awbc_result(
        &self,
        program: &crate::awbc::schema::AwbcProgram,
        ty: crate::awbc::schema::AwbcTypeId,
    ) -> bool {
        if self.operation == RuntimePresentationSlotOperation::Remove {
            return program.builtin_variant_payload_item(ty, crate::pattern::RuntimeBuiltinVariantCaseIdentity::OptionSome)
                .and_then(|item| program.runtime_types.get(item.index()))
                .is_some_and(|row| matches!(row.shape(), crate::awbc::schema::AwbcRuntimeTypeShape::PresentationResource(kind) if *kind == self.resource));
        }
        use crate::{
            pattern::RuntimeOpaqueTypeAdmission,
            value::{RuntimeOpaquePersistence, RuntimeOpaqueValueClass},
        };
        matches!(program.runtime_types.get(ty.index()).map(|ty| ty.shape()),
            Some(crate::awbc::schema::AwbcRuntimeTypeShape::Opaque { producer, admission, value_class, persistence, arguments })
                if program.strings.get(producer.index()).is_some_and(|producer| producer == crate::pattern::RuntimeOpaqueTypeOwner::presentation_slot_ref_producer(self.resource))
                    && *admission == RuntimeOpaqueTypeAdmission::ExactIdentity
                    && *value_class == RuntimeOpaqueValueClass::Plain
                    && *persistence == RuntimeOpaquePersistence::ConstantAndSnapshot
                    && arguments.is_empty())
    }

    /// Constructs the value under the caller's original program/type quota.
    /// A parsed public identity alone never creates an occupant or a lease.
    pub(crate) fn construct(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        owner: &crate::pattern::RuntimeOpaqueTypeOwner,
        values: &[crate::value::RuntimeValue],
    ) -> Result<crate::value::RuntimeValue, crate::value::AwbcRuntimeValueSnapshotError> {
        let result = (|| {
            admission.status()?;
            if !self.is_reference() {
                return Err(RuntimePresentationError::InvalidState.into());
            }
            if values.len() != self.inputs().len() {
                return Err(RuntimePresentationError::SlotReferenceArguments.into());
            }
            super::RuntimePresentationSlotSelectors::from_values(
                self.inputs().iter().copied().zip(values),
            )?
            .with_parts(self.resource, admission, |admission, target, slot| {
                admission.construct_presentation_slot_ref_parts(owner, self.resource, target, slot)
            })
        })();
        result.inspect_err(|error| admission.retain_failure(error.clone()))
    }

    pub(crate) fn evaluate_in_context(
        &self,
        context: &crate::pure::RuntimeExternalCallContext,
        values: &[crate::value::RuntimeValue],
    ) -> Result<crate::value::RuntimeValue, crate::value::RuntimeEvalError> {
        if !self.is_reference() {
            return Err(crate::value::RuntimeEvalError::UnsupportedPure {
                name: "core.presentation.remove".into(),
                reason: "slot removal requires its owning execution bank".into(),
            });
        }
        use crate::{
            pattern::RuntimeCheckedType,
            value::{
                AwbcRuntimeValueSnapshotError, RuntimeEvalError, RuntimeRetainedValueAdmission,
            },
        };
        let evaluate = || -> Result<_, AwbcRuntimeValueSnapshotError> {
            let result_type = context
                .result_type()
                .ok_or(RuntimePresentationError::Program)?;
            let execution = context
                .execution_context()
                .ok_or(RuntimePresentationError::Program)?;
            let owner = context
                .program_owner()
                .ok_or(RuntimePresentationError::Program)?;
            let limits = context.limits().ok_or(RuntimePresentationError::Program)?;
            let valid_result = match execution.types().value_type(result_type)? {
                crate::program_types::RuntimeProgramValueType::Plan { program, ty } => program
                    .type_table()
                    .get(ty)
                    .is_some_and(|ty| self.accepts_plan_result(ty.projection(), |item| {
                        program.type_table().get(*item).and_then(|row| match row.projection() {
                            crate::plan::RuntimePlanTypeProjection::PresentationResource(kind) => Some(*kind), _ => None,
                        })
                    })),
                crate::program_types::RuntimeProgramValueType::Awbc { program, ty } => {
                    self.accepts_awbc_result(program, ty)
                }
            };
            if !valid_result {
                return Err(RuntimePresentationError::SlotReferenceArguments.into());
            }
            let mut admission = RuntimeRetainedValueAdmission::for_program(owner, limits);
            // The result has one closed, bounded producer identity. Charge it
            // before projecting that owner from the original program tables.
            admission.validate_metadata_string(
                crate::pattern::RuntimeOpaqueTypeOwner::presentation_slot_ref_producer(
                    self.resource,
                ),
            )?;
            let RuntimeCheckedType::Opaque {
                owner: result_owner,
            } = execution.types().checked_type(result_type)?
            else {
                return Err(RuntimePresentationError::SlotReferenceArguments.into());
            };
            let types = context
                .argument_types()
                .ok_or(RuntimePresentationError::Program)?;
            if types.len() != values.len() {
                return Err(RuntimePresentationError::SlotReferenceArguments.into());
            }
            for (value, ty) in values.iter().zip(types) {
                admission.validate_typed_borrowed(value.into(), *ty)?;
            }
            self.construct(&mut admission, &result_owner, values)
        };
        evaluate().map_err(|source| RuntimeEvalError::NativeWorkAdmission {
            source: Box::new(source),
        })
    }
}

impl Serialize for RuntimePresentationSlotCall {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Wire<'a> {
            operation: RuntimePresentationSlotOperation,
            resource: PresentationResourceKind,
            inputs: &'a [RuntimePresentationSlotInput],
        }
        Wire {
            operation: self.operation,
            resource: self.resource,
            inputs: self.inputs(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RuntimePresentationSlotCall {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            operation: RuntimePresentationSlotOperation,
            resource: PresentationResourceKind,
            #[serde(deserialize_with = "bounded_inputs")]
            inputs: ([RuntimePresentationSlotInput; 4], usize),
        }

        fn bounded_inputs<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<([RuntimePresentationSlotInput; 4], usize), D::Error> {
            struct Inputs;
            impl<'de> de::Visitor<'de> for Inputs {
                type Value = ([RuntimePresentationSlotInput; 4], usize);

                fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str("at most four slot call argument roles")
                }

                fn visit_seq<A: de::SeqAccess<'de>>(
                    self,
                    mut sequence: A,
                ) -> Result<Self::Value, A::Error> {
                    let mut roles = [RuntimePresentationSlotInput::Target; 4];
                    let mut length = 0;
                    while let Some(role) = sequence.next_element()? {
                        if length == roles.len() {
                            return Err(de::Error::custom("too many slot reference inputs"));
                        }
                        roles[length] = role;
                        length += 1;
                    }
                    Ok((roles, length))
                }
            }
            deserializer.deserialize_seq(Inputs)
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::try_new(
            wire.operation,
            wire.resource,
            wire.inputs.0[..wire.inputs.1].iter().copied(),
        )
        .map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_reference_construction_rejects_foreign_inputs_and_retains_the_failure() {
        use crate::{
            entry::RuntimeSchemaLimits,
            pattern::{RuntimeOpaqueTypeOwner, RuntimeSemanticTypeId},
            value::{RuntimeRetainedValueAdmission, RuntimeValue},
        };
        let owner = RuntimeOpaqueTypeOwner::presentation_slot_ref(
            PresentationResourceKind::Background,
            RuntimeSemanticTypeId::from_bytes([73; 32]),
        );
        let mut admission =
            RuntimeRetainedValueAdmission::standalone(RuntimeSchemaLimits::engine_default());
        let call = RuntimePresentationSlotCall::try_new(
            crate::presentation::RuntimePresentationSlotOperation::Reference,
            PresentationResourceKind::Background,
            [RuntimePresentationSlotInput::Target],
        )
        .unwrap();
        let error = call
            .construct(
                &mut admission,
                &owner,
                &[RuntimeValue::String("target.scene".to_owned())],
            )
            .unwrap_err();
        assert_eq!(admission.status(), Err(error.clone()));
        let empty = RuntimePresentationSlotCall::try_new(
            crate::presentation::RuntimePresentationSlotOperation::Reference,
            PresentationResourceKind::Background,
            [],
        )
        .unwrap();
        assert_eq!(empty.construct(&mut admission, &owner, &[]), Err(error));
    }

    #[test]
    fn slot_reference_call_abi_rejects_duplicate_foreign_and_overlong_roles() {
        use RuntimePresentationSlotInput::{Character, Slot, Target};
        for resource in PresentationResourceKind::ALL {
            let call = RuntimePresentationSlotCall::try_new(
                crate::presentation::RuntimePresentationSlotOperation::Reference,
                resource,
                [Slot, Target],
            )
            .unwrap();
            assert_eq!(call.inputs(), &[Slot, Target]);
            assert_eq!(call.resource(), resource);
            assert_eq!(
                serde_json::from_str::<RuntimePresentationSlotCall>(
                    &serde_json::to_string(&call).unwrap()
                )
                .unwrap(),
                call
            );
            assert!(
                RuntimePresentationSlotCall::try_new(
                    crate::presentation::RuntimePresentationSlotOperation::Reference,
                    resource,
                    [Slot, Slot]
                )
                .is_err()
            );
            assert_eq!(
                RuntimePresentationSlotCall::try_new(
                    crate::presentation::RuntimePresentationSlotOperation::Reference,
                    resource,
                    [Character]
                )
                .is_ok(),
                resource == PresentationResourceKind::Character
            );
        }
        for json in [
            r#"{"operation":"reference","resource":"background","inputs":["target","target"]}"#,
            r#"{"operation":"reference","resource":"background","inputs":["character"]}"#,
            r#"{"operation":"remove","resource":"character","inputs":["target","slot","character","scope","target"]}"#,
            r#"{"operation":"reference","resource":"background","inputs":["scope"]}"#,
            r#"{"operation":"remove","resource":"background","inputs":[],"other":true}"#,
            r#"{"resource":"background","inputs":[]}"#,
        ] {
            assert!(serde_json::from_str::<RuntimePresentationSlotCall>(json).is_err());
        }
        assert_eq!(
            RuntimePresentationSlotInput::try_from(PresentationArgumentRole::Scope),
            Ok(RuntimePresentationSlotInput::Scope)
        );
        let removal = RuntimePresentationSlotCall::try_new(
            RuntimePresentationSlotOperation::Remove,
            PresentationResourceKind::Character,
            [Target, Slot, Character, RuntimePresentationSlotInput::Scope],
        )
        .unwrap();
        assert_eq!(
            removal.validate_execution(4),
            Err(RuntimePresentationError::ScopeCapabilityRequired)
        );
        assert_eq!(
            serde_json::from_str::<RuntimePresentationSlotCall>(
                &serde_json::to_string(&removal).unwrap()
            )
            .unwrap(),
            removal
        );
    }
}
