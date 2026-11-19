//! Read authority for the original header of an actual effect declaration.

use super::*;

/// Body preparation can replace the body's state, but cannot replace this
/// declaration's formals or domain. The lease borrows that same immutable
/// header rather than keeping another predicate inferred from a call path.
#[derive(Clone, Debug)]
pub(crate) struct PreparedEffectDeclaration {
    reference: PreparedEffectSchemeRef,
    header: Arc<SchemeHeader>,
}

impl PartialEq for PreparedEffectDeclaration {
    fn eq(&self, other: &Self) -> bool {
        self.reference == other.reference && Arc::ptr_eq(&self.header, &other.header)
    }
}

impl Eq for PreparedEffectDeclaration {}

impl std::hash::Hash for PreparedEffectDeclaration {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.reference.hash(state);
    }
}

impl PreparedEffectDeclaration {
    pub(super) fn from_header(
        reference: PreparedEffectSchemeRef,
        header: Arc<SchemeHeader>,
    ) -> Self {
        Self { reference, header }
    }

    pub(crate) fn parameters(&self) -> &[GenericEffectReference] {
        &self.header.parameters
    }

    pub(crate) fn domain(&self) -> &EffectPredicate {
        &self.header.domain
    }

    pub(crate) fn scheme(&self) -> &PreparedEffectSchemeRef {
        &self.reference
    }

    /// An immutable header can outlive a rolled-back declaration. Possessing
    /// its old lease never authorizes using that declaration in another graph
    /// or after its removal from the original graph.
    pub(crate) fn validate(
        &self,
        graph: &PreparedEffectGraph,
    ) -> Result<(), PreparedEffectInvariant> {
        self.validate_read(graph.read())
    }

    pub(crate) fn validate_read(
        &self,
        view: PreparedEffectReadView<'_>,
    ) -> Result<(), PreparedEffectInvariant> {
        let current = view.scheme(&self.reference)?;
        if !Arc::ptr_eq(&current.header, &self.header) {
            return Err(PreparedEffectInvariant::StaleReference);
        }
        Ok(())
    }

    /// A declaration's header can authenticate an application while its body
    /// is still open. The ordered operands belong to this exact application.
    pub(crate) fn application_arguments<'a>(
        &self,
        view: PreparedEffectReadView<'a>,
        row: &PreparedEffectRow,
    ) -> Result<&'a [PreparedEffectRow], PreparedEffectInvariant> {
        self.validate_read(view)?;
        let PreparedEffectRow::Term(row) = row else {
            return Err(PreparedEffectInvariant::StaleReference);
        };
        match view.row(row)? {
            RowTerm::Application {
                function: FunctionRef::Scheme(reference),
                arguments,
            } if reference == self.scheme() => {
                if arguments.len() != self.parameters().len() {
                    return Err(PreparedEffectInvariant::ApplicationArity);
                }
                Ok(arguments)
            }
            _ => Err(PreparedEffectInvariant::StaleReference),
        }
    }
}

impl PreparedEffectGraph {
    pub(crate) fn declaration<C: DecisionControl>(
        &self,
        reference: &PreparedEffectSchemeRef,
        control: &mut C,
    ) -> Result<PreparedEffectDeclaration, PreparedEffectFailure<C::Error>> {
        self.read().declaration(reference, control)
    }
}

impl<'g> PreparedEffectReadView<'g> {
    /// Read the declaration identity of an actual application without asking
    /// for the meaning of its body or of any argument row.
    pub(crate) fn named_application_scheme(
        self,
        row: &PreparedEffectRow,
    ) -> Result<Option<&'g PreparedEffectSchemeRef>, PreparedEffectInvariant> {
        let PreparedEffectRow::Term(reference) = row else {
            return Ok(None);
        };
        let RowTerm::Application {
            function: FunctionRef::Scheme(scheme),
            ..
        } = self.row(reference)?
        else {
            return Ok(None);
        };
        self.scheme(scheme)?;
        Ok(Some(scheme))
    }
}

impl PreparedEffectReadView<'_> {
    pub(crate) fn declaration<C: DecisionControl>(
        self,
        reference: &PreparedEffectSchemeRef,
        control: &mut C,
    ) -> Result<PreparedEffectDeclaration, PreparedEffectFailure<C::Error>> {
        control
            .charge(DecisionWork::Visit)
            .map_err(PreparedEffectFailure::Control)?;
        let scheme = self.scheme(reference)?;
        Ok(PreparedEffectDeclaration {
            reference: reference.clone(),
            header: Arc::clone(&scheme.header),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect_row::prepared::test_support::{Budget, Exhausted, budget, literal};
    use crate::types::{GenericBinder, GenericScope};

    #[test]
    fn declaration_authority_retains_the_original_domain_across_body_preparation() {
        let mut work = budget();
        let mut graph = PreparedEffectGraph::default();
        let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
        let formal = scope.bound_effect(0, 0).unwrap();
        let domain = EffectFormula::literal(crate::effects::EffectSet::new(), Some(formal.clone()))
            .subset(
                &EffectFormula::literal(
                    crate::effects::EffectSet::from_labels(["log.write"]).unwrap(),
                    None,
                ),
                &mut work,
            )
            .unwrap();
        let scheme = graph
            .declare(vec![formal.clone()], domain.clone(), &mut work)
            .unwrap();
        let declaration = graph.declaration(&scheme, &mut work).unwrap();
        assert_eq!(declaration.parameters(), &[formal]);
        assert_eq!(declaration.domain(), &domain);
        assert_eq!(declaration.scheme(), &scheme);
        graph
            .prepare_body(&scheme, literal(&[]), vec![], &mut work)
            .unwrap();
        declaration.validate(&graph).unwrap();
        assert_eq!(declaration, graph.declaration(&scheme, &mut work).unwrap());
        assert_eq!(declaration.domain(), &domain);
    }

    #[test]
    fn declaration_authority_rejects_foreign_and_rolled_back_bodies() {
        let mut work = budget();
        let mut graph = PreparedEffectGraph::default();
        let persistent = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut work)
            .unwrap();
        let valid = graph.declaration(&persistent, &mut work).unwrap();
        let mut checkpoint = graph.checkpoint(&mut work).unwrap();
        let temporary = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut work)
            .unwrap();
        let stale = graph.declaration(&temporary, &mut work).unwrap();
        assert_eq!(
            stale.validate(&PreparedEffectGraph::default()),
            Err(PreparedEffectInvariant::ForeignIssuer)
        );
        graph.rollback(&mut checkpoint).unwrap();
        assert_eq!(
            stale.validate(&graph),
            Err(PreparedEffectInvariant::StaleReference)
        );
        assert!(matches!(
            graph.declaration(&temporary, &mut work),
            Err(PreparedEffectFailure::Invariant(
                PreparedEffectInvariant::StaleReference
            ))
        ));
        valid.validate(&graph).unwrap();
        let replacement = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut work)
            .unwrap();
        assert_ne!(temporary, replacement);
        assert_eq!(
            stale.validate(&graph),
            Err(PreparedEffectInvariant::StaleReference)
        );
    }

    #[test]
    fn declaration_authority_reserves_work_before_sharing_the_header() {
        let mut work = budget();
        let mut graph = PreparedEffectGraph::default();
        let scheme = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut work)
            .unwrap();
        assert!(matches!(
            graph.declaration(&scheme, &mut Budget(0)),
            Err(PreparedEffectFailure::Control(Exhausted))
        ));
        graph
            .declaration(&scheme, &mut Budget(1))
            .unwrap()
            .validate(&graph)
            .unwrap();
    }

    #[test]
    fn open_declaration_authenticates_only_its_own_ordered_application() {
        let mut work = budget();
        let mut graph = PreparedEffectGraph::default();
        let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
        let formals = vec![
            scope.bound_effect(0, 0).unwrap(),
            scope.bound_effect(0, 1).unwrap(),
        ];
        let scheme = graph
            .declare(formals.clone(), EffectPredicate::unconstrained(), &mut work)
            .unwrap();
        let other = graph
            .declare(formals, EffectPredicate::unconstrained(), &mut work)
            .unwrap();
        let declaration = graph.declaration(&scheme, &mut work).unwrap();
        let arguments = [literal(&["fs.read"]), literal(&["log.write"])];
        let named = graph
            .application(&scheme, arguments.to_vec(), &mut work)
            .unwrap();
        let unrelated = graph
            .application(&other, arguments.to_vec(), &mut work)
            .unwrap();
        assert!(!graph.read().scheme_body_is_prepared(&scheme).unwrap());
        assert_eq!(
            declaration
                .application_arguments(graph.read(), &named)
                .unwrap(),
            &arguments
        );
        assert_eq!(
            declaration.application_arguments(graph.read(), &unrelated),
            Err(PreparedEffectInvariant::StaleReference)
        );
        let mut checkpoint = graph.checkpoint(&mut work).unwrap();
        let temporary = graph
            .application(&scheme, arguments.to_vec(), &mut work)
            .unwrap();
        graph.rollback(&mut checkpoint).unwrap();
        assert_eq!(
            declaration.application_arguments(graph.read(), &temporary),
            Err(PreparedEffectInvariant::StaleReference)
        );
    }
}
