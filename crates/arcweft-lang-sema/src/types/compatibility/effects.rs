use crate::{
    effect_row::{
        DecisionControl, EffectPredicate, EffectRow,
        prepared::{PreparedEffectPredicate, PreparedEffectRow},
    },
    types::{CanonicalEffects, PreparedEffects, TypeEffectDomain, TypeKind},
};

use super::{
    TypeCompatibilityControl, TypeCompatibilityFailure, TypeCompatibilityPolicy, accepts_node,
};

/// Only function headers depend on the phase's effect representation. The
/// recursive relation, including Choice injection and variance, is shared.
pub(super) trait CompatibilityEffects<D: TypeEffectDomain>:
    TypeCompatibilityControl<D>
{
    fn predicates_equal(
        expected: &D::Predicate,
        actual: &D::Predicate,
        work: &mut impl DecisionControl<Error = Self::Error>,
    ) -> Result<bool, Self::Error>;

    fn rows_compatible(
        expected: &D::Row,
        actual: &D::Row,
        structural: bool,
        work: &mut impl DecisionControl<Error = Self::Error>,
    ) -> Result<bool, Self::Error>;

    fn row_is_unknown(row: &D::Row) -> bool;
}

impl<C: TypeCompatibilityControl> CompatibilityEffects<CanonicalEffects> for C {
    fn predicates_equal(
        expected: &EffectPredicate,
        actual: &EffectPredicate,
        work: &mut impl DecisionControl<Error = Self::Error>,
    ) -> Result<bool, Self::Error> {
        expected.equal_with(actual, work)
    }

    fn rows_compatible(
        expected: &EffectRow,
        actual: &EffectRow,
        structural: bool,
        work: &mut impl DecisionControl<Error = Self::Error>,
    ) -> Result<bool, Self::Error> {
        canonical_rows_compatible(expected, actual, structural, work)
    }

    fn row_is_unknown(row: &EffectRow) -> bool {
        !row.is_known()
    }
}

fn canonical_rows_compatible<C: DecisionControl>(
    expected: &EffectRow,
    actual: &EffectRow,
    structural: bool,
    work: &mut C,
) -> Result<bool, C::Error> {
    if structural {
        return Ok(expected == actual);
    }
    // Strict policies reject unknown annotations during prevalidation.
    if !expected.is_known() || !actual.is_known() {
        return Ok(true);
    }
    actual.is_covered_by(expected, work)
}

/// The result of comparing prepared types without observing their graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreparedTypeCompatibility {
    Accepted,
    Rejected,
    NeedsEffectObservation,
}

enum PreparedComparisonError<E> {
    EffectObservation,
    Control(E),
}

struct PreparedComparison<'a, C>(&'a mut C);

impl<C: TypeCompatibilityControl<PreparedEffects>> TypeCompatibilityControl<PreparedEffects>
    for PreparedComparison<'_, C>
{
    type Error = PreparedComparisonError<C::Error>;

    fn enter(
        &mut self,
        expected: &TypeKind<PreparedEffects>,
        actual: &TypeKind<PreparedEffects>,
    ) -> Result<(), Self::Error> {
        self.0
            .enter(expected, actual)
            .map_err(PreparedComparisonError::Control)
    }
}

impl<C: TypeCompatibilityControl<PreparedEffects>> CompatibilityEffects<PreparedEffects>
    for PreparedComparison<'_, C>
{
    fn predicates_equal(
        expected: &PreparedEffectPredicate,
        actual: &PreparedEffectPredicate,
        work: &mut impl DecisionControl<Error = Self::Error>,
    ) -> Result<bool, Self::Error> {
        match (expected, actual) {
            (
                PreparedEffectPredicate::Canonical(expected),
                PreparedEffectPredicate::Canonical(actual),
            ) => expected.equal_with(actual, work),
            (PreparedEffectPredicate::Term(expected), PreparedEffectPredicate::Term(actual))
                if expected == actual =>
            {
                Ok(true)
            }
            _ => Err(PreparedComparisonError::EffectObservation),
        }
    }

    fn rows_compatible(
        expected: &PreparedEffectRow,
        actual: &PreparedEffectRow,
        structural: bool,
        work: &mut impl DecisionControl<Error = Self::Error>,
    ) -> Result<bool, Self::Error> {
        match (expected, actual) {
            (PreparedEffectRow::Canonical(expected), PreparedEffectRow::Canonical(actual)) => {
                canonical_rows_compatible(expected, actual, structural, work)
            }
            (PreparedEffectRow::Term(expected), PreparedEffectRow::Term(actual))
                if expected == actual =>
            {
                Ok(true)
            }
            _ if !structural
                && (Self::row_is_unknown(expected) || Self::row_is_unknown(actual)) =>
            {
                Ok(true)
            }
            _ => Err(PreparedComparisonError::EffectObservation),
        }
    }

    fn row_is_unknown(row: &PreparedEffectRow) -> bool {
        matches!(row, PreparedEffectRow::Canonical(row) if !row.is_known())
    }
}

impl TypeKind<PreparedEffects> {
    /// Compare the retained values at their source scope. Repeated references
    /// need no observation. Distinct terms cannot decide a branch of the
    /// relation before observation, including a candidate Choice injection.
    pub(crate) fn check_prepared_compatibility<C: TypeCompatibilityControl<PreparedEffects>>(
        &self,
        actual: &Self,
        work: &mut C,
    ) -> Result<PreparedTypeCompatibility, C::Error> {
        match accepts_node(
            self,
            actual,
            TypeCompatibilityPolicy::Recovery,
            &mut PreparedComparison(work),
            true,
            false,
        ) {
            Ok(true) => Ok(PreparedTypeCompatibility::Accepted),
            Ok(false) | Err(TypeCompatibilityFailure::Forbidden { .. }) => {
                Ok(PreparedTypeCompatibility::Rejected)
            }
            Err(TypeCompatibilityFailure::Control(PreparedComparisonError::EffectObservation)) => {
                Ok(PreparedTypeCompatibility::NeedsEffectObservation)
            }
            Err(TypeCompatibilityFailure::Control(PreparedComparisonError::Control(error))) => {
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use super::*;
    use crate::{
        effect_row::{DecisionWork, prepared::PreparedEffectGraph},
        types::GenericBinder,
    };

    struct Work;

    impl DecisionControl for Work {
        type Error = Infallible;

        fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    impl TypeCompatibilityControl<PreparedEffects> for Work {
        type Error = Infallible;

        fn enter(
            &mut self,
            _: &TypeKind<PreparedEffects>,
            _: &TypeKind<PreparedEffects>,
        ) -> Result<(), Infallible> {
            Ok(())
        }
    }

    fn arrow(
        effects: PreparedEffectRow,
        result: TypeKind<PreparedEffects>,
    ) -> TypeKind<PreparedEffects> {
        TypeKind::Function {
            binder: GenericBinder::EMPTY,
            predicate: PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            params: vec![TypeKind::String],
            return_type: Box::new(result),
            effects,
        }
    }

    fn latent_row(graph: &mut PreparedEffectGraph) -> PreparedEffectRow {
        let declaration = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut Work)
            .unwrap();
        graph.application(&declaration, vec![], &mut Work).unwrap()
    }

    #[test]
    fn copies_of_an_unprepared_arrow_keep_their_effect_reference() {
        let mut graph = PreparedEffectGraph::default();
        let effects = latent_row(&mut graph);
        let actual = TypeKind::Tuple(vec![arrow(effects.clone(), TypeKind::String)]);
        let incompatible_return = TypeKind::Tuple(vec![arrow(effects, TypeKind::I64)]);
        assert_eq!(
            actual.check_prepared_compatibility(&actual.clone(), &mut Work),
            Ok(PreparedTypeCompatibility::Accepted)
        );
        assert_eq!(
            incompatible_return.check_prepared_compatibility(&actual, &mut Work),
            Ok(PreparedTypeCompatibility::Rejected)
        );
    }

    #[test]
    fn a_distinct_unprepared_row_cannot_decide_a_choice_injection() {
        let mut graph = PreparedEffectGraph::default();
        let actual = arrow(latent_row(&mut graph), TypeKind::String);
        let other = arrow(latent_row(&mut graph), TypeKind::String);
        let expected = TypeKind::Choice(vec![actual.clone(), other]);
        assert_eq!(
            expected.check_prepared_compatibility(&actual, &mut Work),
            Ok(PreparedTypeCompatibility::NeedsEffectObservation)
        );
    }
}
