//! Bounded, typed ABI for constructing an inert slot address. These inputs
//! issue no presentation lease and never select an external callable.

use super::RuntimePresentationError;
use arcweft_interaction_model::presentation::{PresentationArgumentRole, PresentationResourceKind};
use serde::{Deserialize, Deserializer, Serialize, de};
use std::fmt;

/// Closed argument grammar of a slot-address constructor.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePresentationSlotRefInput {
    Target,
    Slot,
    Character,
}

impl RuntimePresentationSlotRefInput {
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Target => 0,
            Self::Slot => 1,
            Self::Character => 2,
        }
    }

    pub const fn from_semantic_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Target),
            1 => Some(Self::Slot),
            2 => Some(Self::Character),
            _ => None,
        }
    }
}

impl TryFrom<PresentationArgumentRole> for RuntimePresentationSlotRefInput {
    type Error = RuntimePresentationError;

    fn try_from(role: PresentationArgumentRole) -> Result<Self, Self::Error> {
        match role {
            PresentationArgumentRole::TargetEntity => Ok(Self::Target),
            PresentationArgumentRole::Slot => Ok(Self::Slot),
            PresentationArgumentRole::Character => Ok(Self::Character),
            _ => Err(RuntimePresentationError::SlotReferenceRole { role }),
        }
    }
}

/// The role sequence is the exact compact call ABI, independent of the order
/// in which the caller evaluates authored expressions. There are at most three
/// unique roles, retained inline by both live construction and decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimePresentationSlotRefCall {
    resource: PresentationResourceKind,
    inputs: [RuntimePresentationSlotRefInput; 3],
    length: u8,
}

impl RuntimePresentationSlotRefCall {
    pub fn try_new(
        resource: PresentationResourceKind,
        inputs: impl IntoIterator<Item = RuntimePresentationSlotRefInput>,
    ) -> Result<Self, RuntimePresentationError> {
        let mut roles = [RuntimePresentationSlotRefInput::Target; 3];
        let mut length = 0;
        let mut seen = 0_u8;
        for role in inputs {
            let bit = 1_u8 << role.semantic_tag();
            if length == roles.len()
                || seen & bit != 0
                || (role == RuntimePresentationSlotRefInput::Character
                    && resource != PresentationResourceKind::Character)
            {
                return Err(RuntimePresentationError::SlotReferenceArguments);
            }
            seen |= bit;
            roles[length] = role;
            length += 1;
        }
        Ok(Self {
            resource,
            inputs: roles,
            length: length as u8,
        })
    }

    pub const fn resource(&self) -> PresentationResourceKind {
        self.resource
    }

    pub fn inputs(&self) -> &[RuntimePresentationSlotRefInput] {
        &self.inputs[..usize::from(self.length)]
    }

    pub(crate) fn accepts_plan_result<T>(
        &self,
        shape: &crate::plan::RuntimePlanTypeProjection<T>,
    ) -> bool {
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
        use crate::value::{RuntimeEntityReference, RuntimeValue};
        use arcweft_id::{DeclarationIdentityFamily, ProjectEntityReferenceFamily};

        let result = (|| {
            admission.status()?;
            if values.len() != self.inputs().len() {
                return Err(RuntimePresentationError::SlotReferenceArguments.into());
            }
            let mut target = None;
            let mut slot = None;
            let mut character = None;
            for (&role, value) in self.inputs().iter().zip(values) {
                let invalid = || RuntimePresentationError::SlotReferenceInput { role };
                let RuntimeValue::EntityRef(reference) = value else {
                    return Err(invalid().into());
                };
                let (family, public_id) = match reference {
                    RuntimeEntityReference::PresentationAddress(identity) => {
                        (identity.kind().family(), identity.public_id())
                    }
                    RuntimeEntityReference::ImportedProject(reference) => {
                        (reference.family(), reference.public_id())
                    }
                    RuntimeEntityReference::Project {
                        family: DeclarationIdentityFamily::Character,
                        public_id,
                    } => (ProjectEntityReferenceFamily::Character, public_id),
                    _ => return Err(invalid().into()),
                };
                match role {
                    RuntimePresentationSlotRefInput::Target
                        if family == ProjectEntityReferenceFamily::Target =>
                    {
                        target = Some(public_id.as_str());
                    }
                    RuntimePresentationSlotRefInput::Slot
                        if family == ProjectEntityReferenceFamily::Slot =>
                    {
                        slot = Some(public_id.as_str());
                    }
                    RuntimePresentationSlotRefInput::Character
                        if family == ProjectEntityReferenceFamily::Character =>
                    {
                        character = Some(public_id.as_str());
                    }
                    _ => return Err(invalid().into()),
                }
            }
            let target = target.unwrap_or("target.scene");
            let default;
            let slot = match slot {
                Some(slot) => slot,
                None => {
                    default = if self.resource == PresentationResourceKind::Character {
                        let character = character
                            .and_then(|id| id.strip_prefix("character."))
                            .filter(|name| !name.is_empty())
                            .ok_or(RuntimePresentationError::SlotReferenceInput {
                                role: RuntimePresentationSlotRefInput::Character,
                            })?;
                        admission.construct_metadata_string(&[
                            "slot.character.",
                            character,
                            ".default",
                        ])?
                    } else {
                        admission.construct_metadata_string(&[
                            "slot.",
                            self.resource.as_str(),
                            ".default",
                        ])?
                    };
                    &default
                }
            };
            admission.construct_presentation_slot_ref_parts(owner, self.resource, target, slot)
        })();
        result.inspect_err(|error| admission.retain_failure(error.clone()))
    }

    pub(crate) fn evaluate_in_context(
        &self,
        context: &crate::pure::RuntimeExternalCallContext,
        values: &[crate::value::RuntimeValue],
    ) -> Result<crate::value::RuntimeValue, crate::value::RuntimeEvalError> {
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
                    .is_some_and(|ty| self.accepts_plan_result(ty.projection())),
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

impl Serialize for RuntimePresentationSlotRefCall {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Wire<'a> {
            resource: PresentationResourceKind,
            inputs: &'a [RuntimePresentationSlotRefInput],
        }
        Wire {
            resource: self.resource,
            inputs: self.inputs(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RuntimePresentationSlotRefCall {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            resource: PresentationResourceKind,
            #[serde(deserialize_with = "bounded_inputs")]
            inputs: ([RuntimePresentationSlotRefInput; 3], usize),
        }

        fn bounded_inputs<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<([RuntimePresentationSlotRefInput; 3], usize), D::Error> {
            struct Inputs;
            impl<'de> de::Visitor<'de> for Inputs {
                type Value = ([RuntimePresentationSlotRefInput; 3], usize);

                fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str("at most three slot reference argument roles")
                }

                fn visit_seq<A: de::SeqAccess<'de>>(
                    self,
                    mut sequence: A,
                ) -> Result<Self::Value, A::Error> {
                    let mut roles = [RuntimePresentationSlotRefInput::Target; 3];
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
        let call = RuntimePresentationSlotRefCall::try_new(
            PresentationResourceKind::Background,
            [RuntimePresentationSlotRefInput::Target],
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
        let empty =
            RuntimePresentationSlotRefCall::try_new(PresentationResourceKind::Background, [])
                .unwrap();
        assert_eq!(empty.construct(&mut admission, &owner, &[]), Err(error));
    }

    #[test]
    fn slot_reference_call_abi_rejects_duplicate_foreign_and_overlong_roles() {
        use RuntimePresentationSlotRefInput::{Character, Slot, Target};
        for resource in PresentationResourceKind::ALL {
            let call = RuntimePresentationSlotRefCall::try_new(resource, [Slot, Target]).unwrap();
            assert_eq!(call.inputs(), &[Slot, Target]);
            assert_eq!(call.resource(), resource);
            assert_eq!(
                serde_json::from_str::<RuntimePresentationSlotRefCall>(
                    &serde_json::to_string(&call).unwrap()
                )
                .unwrap(),
                call
            );
            assert!(RuntimePresentationSlotRefCall::try_new(resource, [Slot, Slot]).is_err());
            assert_eq!(
                RuntimePresentationSlotRefCall::try_new(resource, [Character]).is_ok(),
                resource == PresentationResourceKind::Character
            );
        }
        for json in [
            r#"{"resource":"background","inputs":["target","target"]}"#,
            r#"{"resource":"background","inputs":["character"]}"#,
            r#"{"resource":"character","inputs":["target","slot","character","target"]}"#,
            r#"{"resource":"background","inputs":["scope"]}"#,
            r#"{"resource":"background","inputs":[],"other":true}"#,
        ] {
            assert!(serde_json::from_str::<RuntimePresentationSlotRefCall>(json).is_err());
        }
        assert!(
            RuntimePresentationSlotRefInput::try_from(PresentationArgumentRole::Scope).is_err()
        );
    }
}
