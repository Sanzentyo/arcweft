//! The shared scope fold admits a retained type under its effect-term issuer.

use super::*;
use crate::effect_row::prepared::{PreparedEffectInvariant, PreparedEffectReadView};

#[cfg(test)]
mod tests;

#[derive(Debug, thiserror::Error)]
pub(crate) enum PreparedTypeRetentionFailure<E: std::error::Error + 'static> {
    #[error("prepared type retention aborted")]
    Control(#[source] E),
    #[error(transparent)]
    Effect(#[from] PreparedEffectInvariant),
}

impl<E: std::error::Error + 'static> PreparedTypeRetentionFailure<E> {
    fn projection(
        error: TypeProjectionError<E>,
    ) -> TypeProjectionError<PreparedTypeRetentionFailure<E>> {
        match error {
            TypeProjectionError::Instantiation(error) => TypeProjectionError::Instantiation(error),
            TypeProjectionError::Control(error) => {
                TypeProjectionError::Control(Self::Control(error))
            }
        }
    }
}

struct Retention<'a, C> {
    effects: PreparedEffectReadView<'a>,
    control: &'a mut C,
    depth: u64,
}

impl<C: TypeProjectionControl> TypeProjectionControl for Retention<'_, C> {
    type Error = PreparedTypeRetentionFailure<C::Error>;

    fn check(&mut self) -> Result<(), Self::Error> {
        self.control
            .check()
            .map_err(PreparedTypeRetentionFailure::Control)
    }

    fn visit_node(&mut self, kind: TypeProjectionNodeKind, depth: u64) -> Result<(), Self::Error> {
        self.control
            .visit_node(kind, depth)
            .map_err(PreparedTypeRetentionFailure::Control)?;
        self.depth = depth;
        Ok(())
    }

    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        self.control
            .visit_binding()
            .map_err(PreparedTypeRetentionFailure::Control)
    }
}

impl<C: TypeProjectionControl> TypeEffectProjection<PreparedEffects, PreparedEffects>
    for Retention<'_, C>
{
    fn project_row(
        &mut self,
        row: &PreparedEffectRow,
        _: &GenericScope,
    ) -> Result<PreparedEffectRow, TypeProjectionError<Self::Error>> {
        self.effects
            .validate_retained_row(row)
            .map_err(|error| TypeProjectionError::Control(error.into()))?;
        if let PreparedEffectRow::Canonical(row) = row {
            visit_effect_row(self.control, row, self.depth)
                .map_err(PreparedTypeRetentionFailure::projection)?;
        }
        Ok(row.clone())
    }

    fn project_predicate(
        &mut self,
        predicate: &PreparedEffectPredicate,
        _: &GenericScope,
    ) -> Result<PreparedEffectPredicate, TypeProjectionError<Self::Error>> {
        self.effects
            .validate_retained_predicate(predicate)
            .map_err(|error| TypeProjectionError::Control(error.into()))?;
        if let PreparedEffectPredicate::Canonical(predicate) = predicate {
            visit_effect_predicate(self.control, predicate, self.depth)
                .map_err(PreparedTypeRetentionFailure::projection)?;
        }
        Ok(predicate.clone())
    }
}

impl TypeKind<PreparedEffects> {
    pub(crate) fn retain_as_tuple<C: TypeProjectionControl>(
        items: &[Self],
        scope: &GenericScope,
        effects: PreparedEffectReadView<'_>,
        control: &mut C,
    ) -> Result<Self, TypeProjectionError<PreparedTypeRetentionFailure<C::Error>>> {
        let mut retention = Retention {
            effects,
            control,
            depth: 1,
        };
        retention.check().map_err(TypeProjectionError::Control)?;
        retention
            .visit_node(TypeProjectionNodeKind::Type, 1)
            .map_err(TypeProjectionError::Control)?;
        let items = items
            .iter()
            .map(|item| item.project_effect_domain_at(scope, 2, &mut retention))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::Tuple(items))
    }

    /// Retain the existing root below a new metatype node. Child traversal
    /// starts at depth 2 and charges each copy there. The outer box is allocated
    /// only after that traversal succeeds.
    pub(crate) fn retain_as_meta_type<C: TypeProjectionControl>(
        &self,
        scope: &GenericScope,
        effects: PreparedEffectReadView<'_>,
        control: &mut C,
    ) -> Result<Self, TypeProjectionError<PreparedTypeRetentionFailure<C::Error>>> {
        control.check().map_err(|error| {
            TypeProjectionError::Control(PreparedTypeRetentionFailure::Control(error))
        })?;
        control
            .visit_node(TypeProjectionNodeKind::Type, 1)
            .map_err(|error| {
                TypeProjectionError::Control(PreparedTypeRetentionFailure::Control(error))
            })?;
        let retained = self.project_effect_domain_at(
            scope,
            2,
            &mut Retention {
                effects,
                control,
                depth: 2,
            },
        )?;
        Ok(Self::MetaType(Box::new(retained)))
    }

    pub(crate) fn retain_effects<C: TypeProjectionControl>(
        &self,
        scope: &GenericScope,
        effects: PreparedEffectReadView<'_>,
        control: &mut C,
    ) -> Result<Self, TypeProjectionError<PreparedTypeRetentionFailure<C::Error>>> {
        self.project_effect_domain(
            scope,
            &mut Retention {
                effects,
                control,
                depth: 1,
            },
        )
    }
}
