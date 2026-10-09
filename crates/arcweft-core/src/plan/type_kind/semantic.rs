//! Exhaustive private executable type-row fields on the owning type algebra.
//! Child IDs are canonical table-reference roles, never public semantic IDs.

use super::RuntimePlanTypeProjection;
use crate::runtime_id::RuntimePlanTypeId;
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticEncodingError};

impl<R> RuntimePlanTypeProjection<R> {
    pub(crate) fn try_visit_semantic_metadata_child_counts<E>(
        &self,
        visitor: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Function {
                contract,
                parameters,
                ..
            } => {
                contract.try_visit_semantic_child_counts(visitor)?;
                visitor(parameters.len())?;
            }
            Self::BuiltinVariant { cases, .. } => visitor(cases.len())?,
            Self::BoundType(..)
            | Self::Never
            | Self::Unit
            | Self::Bool
            | Self::Signed(..)
            | Self::Unsigned(..)
            | Self::F32
            | Self::F64
            | Self::String
            | Self::Color
            | Self::Char
            | Self::Bytes
            | Self::Duration
            | Self::Progress
            | Self::EntityReference
            | Self::AgentValue
            | Self::Range(..)
            | Self::Iterator(..)
            | Self::Sequence { .. }
            | Self::Array { .. }
            | Self::Map { .. }
            | Self::Need(..)
            | Self::Stream { .. }
            | Self::Result { .. }
            | Self::Option { .. }
            | Self::ThreadHandle(..)
            | Self::Shared(..)
            | Self::Reference(..)
            | Self::Nominal { .. }
            | Self::Tuple(..)
            | Self::Record(..)
            | Self::Choice(..)
            | Self::Opaque { .. }
            | Self::Agent(..) => {}
        }
        Ok(())
    }

    pub(crate) const fn executable_semantic_kind(&self) -> u8 {
        match self {
            Self::BoundType(..) => 0,
            Self::Never => 1,
            Self::Unit => 2,
            Self::Bool => 3,
            Self::Signed(..) => 4,
            Self::Unsigned(..) => 5,
            Self::F32 => 6,
            Self::F64 => 7,
            Self::String => 8,
            Self::Color => 9,
            Self::Char => 10,
            Self::Bytes => 11,
            Self::Duration => 12,
            Self::Progress => 13,
            Self::EntityReference => 14,
            Self::AgentValue => 15,
            Self::Range(..) => 16,
            Self::Iterator(..) => 17,
            Self::Sequence { .. } => 18,
            Self::Array { .. } => 19,
            Self::Map { .. } => 20,
            Self::Need(..) => 21,
            Self::Stream { .. } => 22,
            Self::Result { .. } => 23,
            Self::Option { .. } => 24,
            Self::BuiltinVariant { .. } => 25,
            Self::ThreadHandle(..) => 26,
            Self::Shared(..) => 27,
            Self::Reference(..) => 28,
            Self::Function { .. } => 29,
            Self::Nominal { .. } => 30,
            Self::Tuple(..) => 31,
            Self::Record(..) => 32,
            Self::Choice(..) => 33,
            Self::Opaque { .. } => 34,
            Self::Agent(..) => 35,
        }
    }
}

impl RuntimePlanTypeProjection<RuntimePlanTypeId> {
    pub(crate) fn encode_executable_shape(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
    ) -> Result<(), TaskSemanticEncodingError> {
        encoder.status()?;
        match self {
            Self::BoundType(reference) => {
                encoder.ordinal(reference.depth());
                encoder.ordinal(u32::from(reference.slot()));
            }
            Self::Signed(width) => encoder.tag(match width {
                crate::value::RuntimeSignedIntWidth::I8 => 0,
                crate::value::RuntimeSignedIntWidth::I16 => 1,
                crate::value::RuntimeSignedIntWidth::I32 => 2,
                crate::value::RuntimeSignedIntWidth::I64 => 3,
                crate::value::RuntimeSignedIntWidth::I128 => 4,
                crate::value::RuntimeSignedIntWidth::ISize => 5,
            }),
            Self::Unsigned(width) => encoder.tag(match width {
                crate::value::RuntimeUnsignedIntWidth::U8 => 0,
                crate::value::RuntimeUnsignedIntWidth::U16 => 1,
                crate::value::RuntimeUnsignedIntWidth::U32 => 2,
                crate::value::RuntimeUnsignedIntWidth::U64 => 3,
                crate::value::RuntimeUnsignedIntWidth::U128 => 4,
                crate::value::RuntimeUnsignedIntWidth::USize => 5,
            }),
            Self::Sequence { kind, .. } => encoder.tag(kind.semantic_tag()),
            Self::Array { length, .. } => length.encode_semantic_length(encoder),
            Self::Map { kind, .. } => encoder.tag(kind.semantic_tag()),
            Self::BuiltinVariant { owner, cases } => {
                encoder.tag(owner.semantic_tag());
                encoder.count(cases.len());
                for payload in cases {
                    encoder.enter_element();
                    encoder.status()?;
                    encoder.tag(u8::from(payload.is_some()));
                }
            }
            Self::Function {
                contract,
                parameters,
                ..
            } => {
                contract.encode_semantic_contract(encoder)?;
                encoder.count(parameters.len());
            }
            Self::Nominal {
                nominal, layout, ..
            } => {
                // This is the accepted public nominal identity, not source type spelling.
                encoder.string(nominal.as_str());
                encoder.digest(layout.as_bytes());
            }
            Self::Opaque {
                producer,
                admission,
                value_class,
                persistence,
                ..
            } => {
                encoder.string(producer.as_str());
                encoder.tag(admission.encoded());
                encoder.tag(value_class.semantic_tag());
                encoder.tag(persistence.semantic_tag());
            }
            Self::Agent(agent) => encoder.tag(agent.operational_type().semantic_tag()),
            Self::Never
            | Self::Unit
            | Self::Bool
            | Self::F32
            | Self::F64
            | Self::String
            | Self::Color
            | Self::Char
            | Self::Bytes
            | Self::Duration
            | Self::Progress
            | Self::EntityReference
            | Self::AgentValue
            | Self::Range(_)
            | Self::Iterator(_)
            | Self::Need(_)
            | Self::Stream { .. }
            | Self::Result { .. }
            | Self::Option { .. }
            | Self::ThreadHandle(_)
            | Self::Shared(_)
            | Self::Reference(_)
            | Self::Tuple(_)
            | Self::Record(_)
            | Self::Choice(_) => {}
        }
        encoder.status()?;
        let children = self.children();
        encoder.count(children.len());
        for (ordinal, child) in children.iter().enumerate() {
            encoder.enter_element();
            encoder.status()?;
            encoder.count(ordinal);
            encoder.ordinal(child.get().get() - 1);
        }
        encoder.status()
    }
}
