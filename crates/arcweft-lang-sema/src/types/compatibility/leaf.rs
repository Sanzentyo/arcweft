//! Effect-independent scalar compatibility shared by preparation and checking.

use crate::types::{
    StageActorHandleType, TypeEffectDomain, TypeKind,
    constraints::{TypeConstraintShape, TypeLeaf},
};

impl<'a> TypeLeaf<'a> {
    /// Compatibility may inspect a recovered scalar name after the caller's
    /// strict-policy validation. This does not admit an unresolved source.
    pub(super) fn from_type<E: TypeEffectDomain>(ty: &'a TypeKind<E>) -> Option<Self> {
        match ty {
            TypeKind::Named(name) => Some(Self::Named(name)),
            _ => match ty.constraint_shape() {
                TypeConstraintShape::Leaf(leaf) => Some(leaf),
                _ => None,
            },
        }
    }

    /// Leaves contain neither recursive type children nor effect evidence.
    /// No prepared term is solved or converted to make this decision.
    pub(in crate::types) fn accepts(self, actual: Self, structural: bool) -> bool {
        match (self, actual) {
            (
                Self::StageActorHandle(StageActorHandleType::Any),
                Self::StageActorHandle(StageActorHandleType::Exact(_)),
            ) if !structural => true,
            (Self::CharacterDialogue(expected), Self::CharacterDialogue(actual)) => {
                if structural {
                    expected.character() == actual.character()
                } else {
                    expected.accepts(actual)
                }
            }
            (Self::CharacterNominal(expected), Self::CharacterNominal(actual)) => {
                expected.family() == actual.family()
                    && expected.character() == actual.character()
                    && match (expected.part(), actual.part()) {
                        (Some(expected), Some(actual)) => expected == actual,
                        _ => true,
                    }
            }
            (Self::CompileTimeEnum(expected), Self::CompileTimeEnum(actual)) => {
                expected.accepts(*actual)
            }
            (Self::CompileTimeFx(expected), Self::CompileTimeFx(actual)) => {
                expected.accepts(actual)
            }
            (Self::ActionName, Self::String | Self::Named(_)) if !structural => true,
            (Self::AgentValue, actual) if !structural => actual.is_agent_value(),
            _ => self == actual,
        }
    }

    /// Scalar members of the existing AgentValue algebra. Container recursion
    /// remains with the shared type relation, rather than a second type model.
    pub(super) fn is_agent_value(self) -> bool {
        matches!(
            self,
            Self::Bool
                | Self::I8
                | Self::I16
                | Self::I32
                | Self::I64
                | Self::I128
                | Self::ISize
                | Self::U8
                | Self::U16
                | Self::U32
                | Self::U64
                | Self::U128
                | Self::USize
                | Self::F32
                | Self::F64
                | Self::String
                | Self::Char
                | Self::Bytes
                | Self::Duration
                | Self::Progress
                | Self::DisplayContext
                | Self::DisplayError
                | Self::ActionName
                | Self::AgentValue
                | Self::ObservedObject
                | Self::AgentBBox
                | Self::CaptureRef
                | Self::AgentResource
                | Self::AgentResourceBody
                | Self::AgentBuiltin(_)
        )
    }
}
