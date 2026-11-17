//! Borrow the actual effect roots without copying or interpreting their types.

use super::*;
use crate::types::{TypeInstantiationError, constraints::TypeConstraintShape};

#[cfg(test)]
mod tests;

pub(in crate::types) struct PreparedTypeEffectRoots<'a> {
    rows: Vec<&'a PreparedEffectRow>,
    predicates: Vec<&'a PreparedEffectPredicate>,
}

impl<'a> PreparedTypeEffectRoots<'a> {
    /// The shared type-shape grammar owns every child. This read only retains
    /// the header ports needed by the frozen relation; it issues no type or Γ.
    /// An exhausted collection returns no partial root inventory.
    pub(in crate::types) fn collect<C: TypeProjectionControl>(
        types: impl IntoIterator<Item = &'a TypeKind<PreparedEffects>>,
        predicates: impl IntoIterator<Item = &'a PreparedEffectPredicate>,
        control: &mut C,
    ) -> Result<Self, TypeProjectionError<C::Error>> {
        let mut result = Self {
            rows: Vec::new(),
            predicates: Vec::new(),
        };
        for predicate in predicates {
            control
                .visit_node(TypeProjectionNodeKind::Effect, 1)
                .map_err(TypeProjectionError::Control)?;
            result.predicates.push(predicate);
        }
        let mut pending = Vec::new();
        for ty in types {
            control
                .visit_node(TypeProjectionNodeKind::Type, 1)
                .map_err(TypeProjectionError::Control)?;
            pending.push((ty, 1u64));
            while let Some((ty, depth)) = pending.pop() {
                let shape = ty.constraint_shape();
                if let TypeConstraintShape::Function {
                    effects, predicate, ..
                } = shape
                {
                    control
                        .visit_node(TypeProjectionNodeKind::Effect, depth)
                        .map_err(TypeProjectionError::Control)?;
                    result.rows.push(effects);
                    control
                        .visit_node(TypeProjectionNodeKind::Effect, depth)
                        .map_err(TypeProjectionError::Control)?;
                    result.predicates.push(predicate);
                }
                let next_depth = depth
                    .checked_add(1)
                    .ok_or(TypeInstantiationError::DepthOverflow)?;
                for child in shape.children() {
                    control
                        .visit_node(TypeProjectionNodeKind::Type, next_depth)
                        .map_err(TypeProjectionError::Control)?;
                    pending.push((child, next_depth));
                }
            }
        }
        Ok(result)
    }

    pub(in crate::types) fn rows(&self) -> &[&'a PreparedEffectRow] {
        &self.rows
    }
    pub(in crate::types) fn predicates(&self) -> &[&'a PreparedEffectPredicate] {
        &self.predicates
    }
}
