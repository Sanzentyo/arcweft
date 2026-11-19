//! Completed lexical rows and declaration applications retained by a continuation.

use super::*;
use crate::effect_row::prepared::{
    PreparedEffectDeclaration, PreparedEffectFailure, PreparedEffectGraph, PreparedEffectPredicate,
    PreparedEffectRow, PreparedEffectTermEdit,
};
use crate::types::projection_control::{
    EffectProjectionControl, visit_effect_predicate, visit_effect_row,
};

/// A retained application owns its declaration and its lexically resolved
/// arguments. It survives an effect read scope without keeping newly issued
/// graph term IDs alive or claiming that the declaration's body was observed.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum RetainedEffectRow {
    Canonical(EffectRow),
    Named {
        declaration: PreparedEffectDeclaration,
        arguments: Box<[EffectRow]>,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct RetainedEffects;

impl sealed::Sealed for RetainedEffects {}

impl TypeEffectDomain for RetainedEffects {
    type Row = RetainedEffectRow;
    type Predicate = EffectPredicate;
}

/// Both completed domains use the same reference mapping. A Named row maps
/// each operand under its function header's binder and keeps its declaration.
pub(crate) trait RebindableEffects: TypeEffectDomain<Predicate = EffectPredicate> {
    fn rows(row: &Self::Row) -> &[EffectRow];
    fn same_header(left: &Self::Row, right: &Self::Row) -> bool;

    fn map_row<E>(
        row: &Self::Row,
        map: impl FnMut(&EffectRow) -> Result<EffectRow, E>,
    ) -> Result<Self::Row, E>;
}

impl RebindableEffects for CanonicalEffects {
    fn rows(row: &EffectRow) -> &[EffectRow] {
        std::slice::from_ref(row)
    }
    fn same_header(_: &EffectRow, _: &EffectRow) -> bool {
        true
    }

    fn map_row<E>(
        row: &EffectRow,
        mut map: impl FnMut(&EffectRow) -> Result<EffectRow, E>,
    ) -> Result<EffectRow, E> {
        map(row)
    }
}

impl RebindableEffects for RetainedEffects {
    fn rows(row: &RetainedEffectRow) -> &[EffectRow] {
        match row {
            RetainedEffectRow::Canonical(row) => std::slice::from_ref(row),
            RetainedEffectRow::Named { arguments, .. } => arguments,
        }
    }

    fn same_header(left: &RetainedEffectRow, right: &RetainedEffectRow) -> bool {
        match (left, right) {
            (RetainedEffectRow::Canonical(_), RetainedEffectRow::Canonical(_)) => true,
            (
                RetainedEffectRow::Named {
                    declaration: left,
                    arguments: left_arguments,
                },
                RetainedEffectRow::Named {
                    declaration: right,
                    arguments: right_arguments,
                },
            ) => left == right && left_arguments.len() == right_arguments.len(),
            _ => false,
        }
    }

    fn map_row<E>(
        row: &RetainedEffectRow,
        mut map: impl FnMut(&EffectRow) -> Result<EffectRow, E>,
    ) -> Result<RetainedEffectRow, E> {
        Ok(match row {
            RetainedEffectRow::Canonical(row) => RetainedEffectRow::Canonical(map(row)?),
            RetainedEffectRow::Named {
                declaration,
                arguments,
            } => RetainedEffectRow::Named {
                declaration: declaration.clone(),
                arguments: arguments.iter().map(map).collect::<Result<_, _>>()?,
            },
        })
    }
}

impl TypeKind<RetainedEffects> {
    pub(crate) fn install_effects<C: TypeProjectionControl>(
        &self,
        graph: &mut PreparedEffectGraph,
        control: &mut C,
    ) -> Result<
        TypeKind<PreparedEffects>,
        TypeProjectionError<PreparedTypeEffectProjectionFailure<C::Error>>,
    > {
        let mut projection = InstallEffects {
            control,
            edit: graph.edit_terms(),
            depth: 1,
        };
        let value = self.project_effect_domain(&GenericScope::default(), &mut projection)?;
        projection.edit.commit().map_err(|error| {
            PreparedTypeEffectProjectionFailure::from_effect(PreparedEffectFailure::Invariant(
                error,
            ))
        })?;
        Ok(value)
    }
}

struct InstallEffects<'a, 'g, C> {
    control: &'a mut C,
    edit: PreparedEffectTermEdit<'g>,
    depth: u64,
}

impl<C: TypeProjectionControl> TypeProjectionControl for InstallEffects<'_, '_, C> {
    type Error = PreparedTypeEffectProjectionFailure<C::Error>;

    fn check(&mut self) -> Result<(), Self::Error> {
        self.control
            .check()
            .map_err(PreparedTypeEffectProjectionFailure::Control)
    }

    fn visit_node(&mut self, kind: TypeProjectionNodeKind, depth: u64) -> Result<(), Self::Error> {
        self.control
            .visit_node(kind, depth)
            .map_err(PreparedTypeEffectProjectionFailure::Control)?;
        self.depth = depth;
        Ok(())
    }

    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        self.control
            .visit_binding()
            .map_err(PreparedTypeEffectProjectionFailure::Control)
    }
}

impl<C: TypeProjectionControl> TypeEffectProjection<RetainedEffects, PreparedEffects>
    for InstallEffects<'_, '_, C>
{
    fn project_row(
        &mut self,
        row: &RetainedEffectRow,
        _: &GenericScope,
    ) -> Result<PreparedEffectRow, TypeProjectionError<Self::Error>> {
        let mut prepare = |row: &EffectRow| {
            visit_effect_row(self.control, row, self.depth)
                .map_err(PreparedTypeEffectProjectionFailure::from_projection)?;
            Ok(PreparedEffectRow::Canonical(row.clone()))
        };
        match row {
            RetainedEffectRow::Canonical(row) => prepare(row),
            RetainedEffectRow::Named {
                declaration,
                arguments,
            } => {
                let arguments = arguments.iter().map(prepare).collect::<Result<_, _>>()?;
                self.edit
                    .named_application(
                        declaration,
                        arguments,
                        &mut EffectProjectionControl {
                            control: self.control,
                            depth: self.depth,
                        },
                    )
                    .map_err(PreparedTypeEffectProjectionFailure::from_effect)
            }
        }
    }

    fn project_predicate(
        &mut self,
        predicate: &EffectPredicate,
        _: &GenericScope,
    ) -> Result<PreparedEffectPredicate, TypeProjectionError<Self::Error>> {
        visit_effect_predicate(self.control, predicate, self.depth)
            .map_err(PreparedTypeEffectProjectionFailure::from_projection)?;
        Ok(PreparedEffectPredicate::Canonical(predicate.clone()))
    }
}
