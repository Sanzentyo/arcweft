//! Closed language relations whose expected and supplied shapes differ.

use crate::types::{TypeEffectDomain, TypeKind};

pub(super) enum CrossShapeCompatibility<'a, E: TypeEffectDomain> {
    ByteCollection(&'a TypeKind<E>),
    AgentValue,
}

/// The complete existing AgentValue membership algebra, shared by both phases.
pub(super) enum AgentValueShape<'a, E: TypeEffectDomain> {
    Accept,
    Reject,
    Recovered,
    Element(&'a TypeKind<E>),
    Map {
        key: &'a TypeKind<E>,
        value: &'a TypeKind<E>,
    },
    Alternatives(&'a [TypeKind<E>]),
}

impl<E: TypeEffectDomain> TypeKind<E> {
    pub(super) fn agent_value_shape(&self) -> AgentValueShape<'_, E> {
        use AgentValueShape::*;
        if let crate::types::constraints::TypeConstraintShape::Leaf(leaf) = self.constraint_shape()
        {
            return if leaf.is_agent_value() {
                Accept
            } else {
                Reject
            };
        }
        match self {
            Self::Ref(_) => Accept,
            Self::Error(_) => Recovered,
            Self::Vec(inner)
            | Self::Array { item: inner, .. }
            | Self::Slice(inner)
            | Self::Range(inner)
            | Self::Option(inner) => Element(inner),
            Self::Map { key, value, .. } => Map { key, value },
            Self::Choice(alternatives) => Alternatives(alternatives),
            _ => Reject,
        }
    }

    pub(super) fn cross_shape_compatibility<'a>(
        &self,
        actual: &'a Self,
    ) -> Option<CrossShapeCompatibility<'a, E>> {
        match (self, actual) {
            (Self::Bytes, Self::Vec(inner) | Self::Slice(inner) | Self::Seq(inner)) => {
                Some(CrossShapeCompatibility::ByteCollection(inner))
            }
            (Self::AgentValue, _) => Some(CrossShapeCompatibility::AgentValue),
            _ => None,
        }
    }
}
