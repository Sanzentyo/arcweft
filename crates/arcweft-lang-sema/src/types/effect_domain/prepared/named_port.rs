//! A declaration application on the return spine of a prepared function type.

use super::*;
use crate::effect_row::{
    DecisionControl, DecisionWork,
    prepared::{PreparedEffectDeclaration, PreparedEffectInvariant, PreparedEffectReadView},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedNamedEffectPort {
    declaration: PreparedEffectDeclaration,
    return_edges: usize,
}

/// The exact field selected in one borrowed type and its live operands.
pub(crate) struct PreparedNamedEffectUse<'ty, 'g> {
    row: &'ty PreparedEffectRow,
    declaration: &'g PreparedEffectDeclaration,
    arguments: &'g [PreparedEffectRow],
}

impl<'g> PreparedNamedEffectUse<'_, 'g> {
    pub(crate) fn matches_row(&self, row: &PreparedEffectRow) -> bool {
        std::ptr::eq(self.row, row)
    }

    pub(crate) fn declaration(&self) -> &'g PreparedEffectDeclaration {
        self.declaration
    }

    pub(crate) fn arguments(&self) -> &'g [PreparedEffectRow] {
        self.arguments
    }
}

impl PreparedNamedEffectPort {
    pub(crate) fn for_type<C: DecisionControl>(
        declaration: &PreparedEffectDeclaration,
        return_edges: usize,
        ty: &TypeKind<PreparedEffects>,
        graph: PreparedEffectReadView<'_>,
        control: &mut C,
    ) -> Result<Option<Self>, PreparedEffectFailure<C::Error>> {
        let port = Self {
            declaration: declaration.clone(),
            return_edges,
        };
        Ok(port
            .application(ty, graph, control)?
            .is_some()
            .then_some(port))
    }

    pub(crate) fn declaration(&self) -> &PreparedEffectDeclaration {
        &self.declaration
    }

    /// Re-open the occurrence on a scoped copy of the result. Scope transport
    /// can change the term and its arguments, so the lease owns the declaration
    /// and the structural position rather than a graph-local row identity.
    pub(crate) fn application<'ty, 'g, C: DecisionControl>(
        &'g self,
        ty: &'ty TypeKind<PreparedEffects>,
        graph: PreparedEffectReadView<'g>,
        control: &mut C,
    ) -> Result<Option<PreparedNamedEffectUse<'ty, 'g>>, PreparedEffectFailure<C::Error>> {
        let row = Self::return_spine_row(self.return_edges, ty, control)?;
        match row {
            PreparedEffectRow::Canonical(_) => Ok(None),
            PreparedEffectRow::Term(_) => Ok(Some(PreparedNamedEffectUse {
                row,
                declaration: &self.declaration,
                arguments: self.declaration.application_arguments(graph, row)?,
            })),
        }
    }

    fn return_spine_row<'a, C: DecisionControl>(
        return_edges: usize,
        mut ty: &'a TypeKind<PreparedEffects>,
        control: &mut C,
    ) -> Result<&'a PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        for edge in 0..=return_edges {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            let TypeKind::Function {
                effects,
                return_type,
                ..
            } = ty
            else {
                return Err(PreparedEffectInvariant::ApplicationArity.into());
            };
            if edge == return_edges {
                return Ok(effects);
            }
            ty = return_type;
        }
        unreachable!("the final return edge is included")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        effect_row::prepared::PreparedEffectGraph, effects::EffectSet, types::GenericBinder,
    };

    struct Control;

    impl DecisionControl for Control {
        type Error = std::convert::Infallible;

        fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    fn literal(labels: &[&str]) -> PreparedEffectRow {
        PreparedEffectRow::Canonical(EffectRow::closed(
            EffectSet::from_labels(labels.iter().copied()).unwrap(),
        ))
    }

    fn arrow(
        parameter: TypeKind<PreparedEffects>,
        result: TypeKind<PreparedEffects>,
        row: PreparedEffectRow,
    ) -> TypeKind<PreparedEffects> {
        TypeKind::function_with_contract(
            GenericBinder::EMPTY,
            PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            [parameter],
            result,
            row,
        )
    }

    #[test]
    fn scoped_result_port_reads_the_ordered_application_on_the_return_spine() {
        let mut graph = PreparedEffectGraph::default();
        let mut control = Control;
        let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
        let formals = vec![
            scope.bound_effect(0, 0).unwrap(),
            scope.bound_effect(0, 1).unwrap(),
        ];
        let scheme = graph
            .declare(
                formals.clone(),
                EffectPredicate::unconstrained(),
                &mut control,
            )
            .unwrap();
        let other = graph
            .declare(formals, EffectPredicate::unconstrained(), &mut control)
            .unwrap();
        let header = graph.declaration(&scheme, &mut control).unwrap();
        let before = [literal(&["log.write"]), literal(&["fs.read"])];
        let after = [before[1].clone(), before[0].clone()];
        let before_term = graph
            .application(&scheme, before.to_vec(), &mut control)
            .unwrap();
        let replay = graph
            .application(&scheme, before.to_vec(), &mut control)
            .unwrap();
        let after_term = graph
            .application(&scheme, after.to_vec(), &mut control)
            .unwrap();
        let wrong_term = graph
            .application(&other, after.to_vec(), &mut control)
            .unwrap();
        assert_ne!(before_term, replay);
        assert!(before_term.same_continuation_row(&replay));
        assert!(!before_term.same_continuation_row(&after_term));
        assert!(!after_term.same_continuation_row(&wrong_term));
        let result = |terminal| {
            arrow(
                arrow(TypeKind::I64, TypeKind::I64, before_term.clone()),
                arrow(TypeKind::I64, TypeKind::Bool, terminal),
                literal(&[]),
            )
        };
        let initial = result(before_term.clone());
        let port =
            PreparedNamedEffectPort::for_type(&header, 1, &initial, graph.read(), &mut control)
                .unwrap()
                .unwrap();
        assert_eq!(
            port.application(&initial, graph.read(), &mut control)
                .unwrap()
                .map(|application| application.arguments),
            Some(before.as_slice())
        );
        assert_eq!(
            port.application(&result(after_term), graph.read(), &mut control)
                .unwrap()
                .map(|application| application.arguments),
            Some(after.as_slice())
        );
        assert!(matches!(
            port.application(&result(wrong_term), graph.read(), &mut control),
            Err(PreparedEffectFailure::Invariant(
                PreparedEffectInvariant::StaleReference
            )),
        ));
        assert!(!graph.read().scheme_body_is_prepared(&scheme).unwrap());
    }
}
