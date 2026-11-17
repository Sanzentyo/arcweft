//! Closed language relations whose expected and supplied shapes differ.

use crate::types::{TypeEffectDomain, TypeKind};

pub(super) enum CrossShapeCompatibility<'a, E: TypeEffectDomain> {
    ByteCollection(&'a TypeKind<E>),
    AgentValue,
}

impl<E: TypeEffectDomain> TypeKind<E> {
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
