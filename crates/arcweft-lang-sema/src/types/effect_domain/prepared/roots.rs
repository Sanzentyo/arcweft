//! Borrow the actual effect roots without copying or interpreting their types.

use super::*;
use crate::types::{TypeInstantiationError, constraints::TypeConstraintShape};

#[cfg(test)]
mod tests;

#[derive(Default)]
pub(crate) struct PreparedTypeEffectRoots<'a> {
    rows: Vec<&'a PreparedEffectRow>,
    predicates: Vec<&'a PreparedEffectPredicate>,
}

impl<'a> PreparedTypeEffectRoots<'a> {
    /// The shared type-shape grammar owns every child. This read only retains
    /// the header ports needed by the frozen relation; it issues no type or Γ.
    /// An exhausted collection returns no partial root inventory.
    pub(crate) fn collect<C: TypeProjectionControl>(
        types: impl IntoIterator<Item = &'a TypeKind<PreparedEffects>>,
        predicates: impl IntoIterator<Item = &'a PreparedEffectPredicate>,
        control: &mut C,
    ) -> Result<Self, TypeProjectionError<C::Error>> {
        let mut result = Self::default();
        for predicate in predicates {
            control
                .visit_node(TypeProjectionNodeKind::Effect, 1)
                .map_err(TypeProjectionError::Control)?;
            result.predicates.push(predicate);
        }
        result.extend(types, control)?;
        Ok(result)
    }

    pub(crate) fn extend<C: TypeProjectionControl>(
        &mut self,
        types: impl IntoIterator<Item = &'a TypeKind<PreparedEffects>>,
        control: &mut C,
    ) -> Result<(), TypeProjectionError<C::Error>> {
        for ty in types {
            self.extend_type(ty, None, control)?;
        }
        Ok(())
    }

    pub(crate) fn extend_type<C: TypeProjectionControl>(
        &mut self,
        ty: &'a TypeKind<PreparedEffects>,
        named: Option<PreparedNamedEffectUse<'a, 'a>>,
        control: &mut C,
    ) -> Result<(), TypeProjectionError<C::Error>> {
        let mut pending = Vec::new();
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
                if let Some(named) = &named
                    && named.matches_row(effects)
                {
                    for row in named.arguments() {
                        control
                            .visit_node(TypeProjectionNodeKind::Effect, depth)
                            .map_err(TypeProjectionError::Control)?;
                        self.rows.push(row);
                    }
                } else {
                    self.rows.push(effects);
                }
                control
                    .visit_node(TypeProjectionNodeKind::Effect, depth)
                    .map_err(TypeProjectionError::Control)?;
                self.predicates.push(predicate);
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
        Ok(())
    }

    pub(crate) fn rows(&self) -> &[&'a PreparedEffectRow] {
        &self.rows
    }
    pub(crate) fn predicates(&self) -> &[&'a PreparedEffectPredicate] {
        &self.predicates
    }

    pub(in crate::types) fn has_terms(&self) -> bool {
        self.rows
            .iter()
            .any(|row| matches!(row, PreparedEffectRow::Term(_)))
            || self
                .predicates
                .iter()
                .any(|predicate| matches!(predicate, PreparedEffectPredicate::Term(_)))
    }
}
