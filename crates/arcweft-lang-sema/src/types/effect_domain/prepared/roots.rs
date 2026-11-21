//! Borrow the actual effect roots without copying or interpreting their types.

use super::*;
use crate::types::{TypeInstantiationError, constraints::TypeConstraintShape};

#[cfg(test)]
mod tests;

#[derive(Default)]
pub(crate) struct PreparedTypeEffectRoots<'a> {
    // Only terms lead to prepared graph definitions. Keep the authored
    // port classifications needed for observation and initializer inference.
    rows: Vec<&'a PreparedEffectRow>,
    predicates: Vec<&'a PreparedEffectPredicate>,
    effect_inputs: bool,
    omitted_rows: bool,
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
            result.record_predicate(predicate);
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
                        self.record_row(row);
                    }
                } else {
                    self.record_row(effects);
                }
                control
                    .visit_node(TypeProjectionNodeKind::Effect, depth)
                    .map_err(TypeProjectionError::Control)?;
                self.record_predicate(predicate);
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
        !self.rows.is_empty() || !self.predicates.is_empty()
    }

    pub(crate) const fn has_effect_inputs(&self) -> bool {
        self.effect_inputs
    }

    pub(super) const fn has_omitted_rows(&self) -> bool {
        self.omitted_rows
    }

    fn record_row(&mut self, row: &'a PreparedEffectRow) {
        self.effect_inputs = true;
        match row {
            PreparedEffectRow::Term(_) => self.rows.push(row),
            PreparedEffectRow::Canonical(row) => self.omitted_rows |= !row.is_known(),
        }
    }

    fn record_predicate(&mut self, predicate: &'a PreparedEffectPredicate) {
        self.effect_inputs = true;
        if matches!(predicate, PreparedEffectPredicate::Term(_)) {
            self.predicates.push(predicate);
        }
    }
}
