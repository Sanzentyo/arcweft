//! Evaluate the immutable prepared term DAG in dependency order.

use super::*;

#[derive(Clone, Copy)]
pub(super) enum Reference<'a> {
    Row(&'a PreparedEffectRowRef),
    Predicate(&'a PreparedEffectPredicateRef, PredicateInputs),
}

#[derive(Clone, Copy)]
enum Term<'a> {
    Row(u64, &'a RowTerm),
    Predicate(u64, &'a PredicateTerm, PredicateInputs),
}

enum Children<'a> {
    Rows(RowChildren<'a>),
    Predicates(
        std::slice::Iter<'a, PreparedEffectPredicate>,
        PredicateInputs,
    ),
    Selected(
        std::array::IntoIter<&'a PreparedEffectPredicate, 2>,
        PredicateInputs,
    ),
    None,
}

enum Child<'a> {
    Term(Reference<'a>),
    Canonical,
}

impl<'a> Iterator for Children<'a> {
    type Item = Child<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let predicate_reference = |predicate: &'a PreparedEffectPredicate, inputs| match predicate {
            PreparedEffectPredicate::Term(reference) => {
                Child::Term(Reference::Predicate(reference, inputs))
            }
            PreparedEffectPredicate::Canonical(_) => Child::Canonical,
        };
        match self {
            Self::Rows(children) => children.next().map(|row| match row {
                PreparedEffectRow::Term(reference) => Child::Term(Reference::Row(reference)),
                PreparedEffectRow::Canonical(_) => Child::Canonical,
            }),
            Self::Predicates(children, inputs) => children
                .next()
                .map(|child| predicate_reference(child, *inputs)),
            Self::Selected(children, inputs) => children
                .next()
                .map(|child| predicate_reference(child, *inputs)),
            Self::None => None,
        }
    }
}

impl<'a> Term<'a> {
    fn children(self) -> Children<'a> {
        match self {
            Self::Row(_, row) => Children::Rows(row.children()),
            Self::Predicate(_, predicate, inputs) => match predicate {
                PredicateTerm::Substitution { arguments, .. } => {
                    Children::Rows(RowChildren::Arguments(arguments.values()))
                }
                PredicateTerm::Relation {
                    actual, permitted, ..
                } => Children::Rows(RowChildren::Pair([actual, permitted].into_iter())),
                PredicateTerm::And(children) | PredicateTerm::Or(children) => {
                    Children::Predicates(children.iter(), inputs.children(predicate))
                }
                PredicateTerm::Not(child) => Children::Predicates(
                    std::slice::from_ref(child).iter(),
                    inputs.children(predicate),
                ),
                PredicateTerm::Select { yes, no, .. } => {
                    Children::Selected([yes, no].into_iter(), inputs.children(predicate))
                }
                PredicateTerm::ImportedPrincipal(_) => Children::None,
            },
        }
    }
}

impl<'a> Compilation<'a> {
    fn pending_term<C: DecisionControl>(
        &self,
        reference: Reference<'_>,
        parent: Option<Term<'a>>,
        work: &mut Work<'_, C>,
    ) -> Result<Option<Term<'a>>, PreparedEffectFailure<C::Error>> {
        work.charge(DecisionWork::Visit)?;
        match reference {
            Reference::Row(reference) => {
                let term = self.graph.row(reference)?;
                let id = reference.0.ordinal;
                if matches!(parent, Some(Term::Row(parent, _)) if id >= parent) {
                    return Err(PreparedEffectInvariant::StaleReference.into());
                }
                Ok((!self.rows.contains_key(&id)).then_some(Term::Row(id, term)))
            }
            Reference::Predicate(reference, inputs) => {
                let term = self.graph.predicate(reference)?;
                let id = reference.0.ordinal;
                if matches!(parent, Some(Term::Predicate(parent, ..)) if id >= parent) {
                    return Err(PreparedEffectInvariant::StaleReference.into());
                }
                Ok((!self.predicates.contains_key(&(id, inputs)))
                    .then_some(Term::Predicate(id, term, inputs)))
            }
        }
    }

    pub(super) fn prepare<C: DecisionControl>(
        &mut self,
        root: Reference<'_>,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        let Some(mut term) = self.pending_term(root, None, work)? else {
            return Ok(());
        };
        let mut children = term.children();
        let mut suspended = Vec::new();
        loop {
            if let Some(child) = children.next() {
                match child {
                    Child::Canonical => work.charge(DecisionWork::Visit)?,
                    Child::Term(reference) => {
                        if let Some(child) = self.pending_term(reference, Some(term), work)? {
                            work.charge(DecisionWork::Node)?;
                            suspended.push((term, children));
                            term = child;
                            children = child.children();
                        }
                    }
                }
                continue;
            }
            match term {
                Term::Row(id, row) => self.compile_row(id, row, work)?,
                Term::Predicate(id, predicate, inputs) => {
                    self.compile_predicate(id, predicate, inputs, work)?
                }
            }
            let Some((parent, remaining)) = suspended.pop() else {
                return Ok(());
            };
            term = parent;
            children = remaining;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect_row::prepared::test_support::{Budget, budget, literal, open, parameter};
    use crate::types::{GenericBinder, GenericScope};

    #[test]
    fn unrestricted_membership_preserves_uniform_choices_and_boolean_predicates() {
        let mut graph = PreparedEffectGraph::default();
        let mut control = budget();
        let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
        let parameter = parameter(&scope, 0);
        let input = open(&parameter);
        let unknown = graph
            .declare_unknown(
                vec![parameter.clone()],
                PreparedEffectUnknownEligibility::Bindable,
                &mut control,
            )
            .unwrap();
        let output = graph
            .unknown_application(&unknown, vec![input.clone()], &mut control)
            .unwrap();
        let included = graph
            .relate(
                PreparedEffectRelation::AdmittedSubset,
                input,
                output,
                &mut control,
            )
            .unwrap();
        let denied = graph.not(included.clone(), &mut control).unwrap();
        let disjunction = graph
            .or(vec![included.clone(), denied], &mut control)
            .unwrap();
        let choice = graph.choice(&mut control).unwrap();
        let selected = graph
            .select_predicate(&choice, included.clone(), disjunction, &mut control)
            .unwrap();
        let conjunction = graph
            .and(vec![selected.clone(), included.clone()], &mut control)
            .unwrap();
        let work = &mut Work(&mut control);
        let layout = Layout::with_shared_labels(&graph.relation, Arc::from([]), work).unwrap();
        let parameters = [parameter];
        let mut compiler = Compilation::new(
            &graph.relation,
            layout.view(),
            FormalOwner::Observation,
            &parameters,
            ApplicationCells::Variables,
        );
        let formals = compiler.formals(work).unwrap();
        for predicate in [&selected, &conjunction] {
            let reference = compiler
                .predicate(predicate, work)
                .unwrap()
                .forall(&formals, work)
                .unwrap();
            let prequantified = compiler
                .predicate_in_unrestricted_scope(predicate, work)
                .unwrap();
            assert_eq!(prequantified.forall(&formals, work).unwrap(), reference);
            assert!(!reference.is_constant(true));
            assert!(!reference.is_constant(false));
            assert!(
                prequantified
                    .variables()
                    .all(|variable| !formals.contains(variable))
            );
        }
        let selected = compiler
            .predicate_in_unrestricted_scope(&selected, work)
            .unwrap();
        assert!(
            selected
                .variables()
                .any(|variable| matches!(variable, Atom::Choice(_)))
        );
        let ordinary = compiler.predicate(&included, work).unwrap();
        assert!(
            ordinary
                .variables()
                .any(|variable| formals.contains(variable))
        );
    }

    #[test]
    fn mixed_dependencies_share_completed_terms_and_validate_a_cached_root() {
        fn terms(graph: &mut PreparedEffectGraph) -> (PreparedEffectRow, PreparedEffectPredicate) {
            let mut work = budget();
            let function = graph
                .declare_unknown(
                    vec![],
                    PreparedEffectUnknownEligibility::Bindable,
                    &mut work,
                )
                .unwrap();
            let row = graph
                .unknown_application(&function, vec![], &mut work)
                .unwrap();
            let predicate = graph
                .relate(
                    PreparedEffectRelation::Equal,
                    row.clone(),
                    literal(&[]),
                    &mut work,
                )
                .unwrap();
            (row, predicate)
        }

        let mut graph = PreparedEffectGraph::default();
        let (row, predicate) = terms(&mut graph);
        let (foreign_row, foreign_predicate) = terms(&mut PreparedEffectGraph::default());
        let mut work = budget();
        let layout =
            Layout::with_shared_labels(&graph.relation, Arc::from([]), &mut Work(&mut work))
                .unwrap();
        let mut compilation = Compilation::new(
            &graph.relation,
            layout.view(),
            FormalOwner::Observation,
            &[],
            ApplicationCells::Variables,
        );

        let original = compilation
            .predicate(&predicate, &mut Work(&mut work))
            .unwrap();
        assert!(original.node_count() > 0);
        let mut retained = Budget(1);
        let same = compilation
            .predicate(&predicate, &mut Work(&mut retained))
            .unwrap();
        assert!(Arc::ptr_eq(&original, &same));
        assert_eq!(retained.0, 0);
        let mut dependency = Budget(1);
        let value = compilation.row(&row, &mut Work(&mut dependency)).unwrap();
        assert!(value.membership[0].node_count() > 0);
        assert_eq!(dependency.0, 0);
        assert!(matches!(
            compilation.row(&foreign_row, &mut Work(&mut Budget(1))),
            Err(PreparedEffectFailure::Invariant(
                PreparedEffectInvariant::ForeignIssuer
            ))
        ));
        assert!(matches!(
            compilation.predicate(&foreign_predicate, &mut Work(&mut Budget(1))),
            Err(PreparedEffectFailure::Invariant(
                PreparedEffectInvariant::ForeignIssuer
            ))
        ));
    }
}
