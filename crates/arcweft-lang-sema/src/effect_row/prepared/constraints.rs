//! The candidate retains Γ and an unevaluated R in the same phase owner.

use super::*;
use crate::effect_row::{
    EffectConstraintEnvironment, EffectConstraintEnvironmentError, EffectConstraintVariable,
};
use crate::types::PreparedEffects;

#[cfg(test)]
mod tests;

impl EffectConstraintEnvironment<PreparedEffects> {
    /// The types owner supplies actual registered applications and the complete
    /// captured rigid Γ. Keep the original environment unchanged until both
    /// the projected R and its Γ admission succeed in the original graph.
    pub(crate) fn project_registered_parameters<C: DecisionControl>(
        &self,
        captured_rigids: &[GenericEffectReference],
        applications: &BTreeMap<GenericEffectReference, PreparedEffectRow>,
        graph: &mut PreparedEffectGraph,
        control: &mut C,
    ) -> Result<Self, PreparedEffectFailure<C::Error>>
    where
        C::Error: From<EffectConstraintEnvironmentError>,
    {
        for _ in &self.parameters {
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
        }
        let parameters = self.parameters.clone();
        let predicate = graph.read().retain_predicate(&self.predicate, control)?;
        let mut result = Self {
            parameters,
            predicate,
        };
        let mut captures = Vec::new();
        for variable in captured_rigids {
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
            if variable.free_parameter().is_none() {
                return Err(PreparedEffectFailure::Control(
                    EffectConstraintEnvironmentError::NonCanonicalScope.into(),
                ));
            }
            captures.push(EffectConstraintVariable::new(
                variable.clone(),
                crate::effect_row::EffectConstraintEligibility::Rigid,
            ));
        }
        result
            .admit_variables(&captures)
            .map_err(|error| PreparedEffectFailure::Control(error.into()))?;
        for variable in applications.keys() {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            if !matches!(variable, GenericEffectReference::Inference(_)) {
                return Err(PreparedEffectInvariant::ParameterContract.into());
            }
            self.validate_reference(variable)
                .map_err(|error| PreparedEffectFailure::Control(error.into()))?;
        }
        let mut edit = graph.edit_terms();
        let predicate =
            edit.map_predicate_parameters(&self.predicate, control, &mut |variable, control| {
                control
                    .charge(DecisionWork::Visit)
                    .map_err(PreparedEffectFailure::Control)?;
                applications
                    .get(variable)
                    .map(|row| {
                        if let PreparedEffectRow::Canonical(row) = row {
                            row.encode(&mut super::super::EffectEqualityControl(control))
                                .map_err(PreparedEffectFailure::Control)?;
                        }
                        Ok(row.clone())
                    })
                    .transpose()
            })?;
        edit.read()
            .visit_predicate_parameters(&predicate, control, &mut |reference, _| {
                result.validate_reference(reference).map_err(Into::into)
            })?;
        control
            .charge(DecisionWork::Visit)
            .map_err(PreparedEffectFailure::Control)?;
        edit.commit()?;
        result.predicate = predicate;
        Ok(result)
    }

    /// Declaration inputs are already canonical formulas, not completed
    /// candidate solutions. Admit them without projection or witness choice.
    pub(crate) fn new_prepared<C: DecisionControl>(
        variables: &[EffectConstraintVariable],
        predicate: &EffectPredicate,
        control: &mut C,
    ) -> Result<Self, C::Error>
    where
        C::Error: From<EffectConstraintEnvironmentError>,
    {
        for _ in variables {
            control.charge(DecisionWork::Node)?;
        }
        let mut environment = Self {
            parameters: BTreeMap::new(),
            predicate: PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
        };
        environment.admit_variables(variables)?;
        for reference in predicate.variables() {
            control.charge(DecisionWork::Visit)?;
            environment.validate_reference(reference)?;
        }
        predicate.encode(&mut super::super::EffectEqualityControl(control))?;
        environment.predicate = PreparedEffectPredicate::Canonical(predicate.clone());
        Ok(environment)
    }

    /// The lexical owner separately validates bound references against its
    /// actual frames. This check authenticates the branch's free inventory.
    pub(crate) fn validate_row<C: DecisionControl>(
        &self,
        row: &PreparedEffectRow,
        graph: PreparedEffectReadView<'_>,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>>
    where
        C::Error: From<EffectConstraintEnvironmentError>,
    {
        graph.visit_row_parameters(row, control, &mut |reference, _| {
            self.validate_reference(reference).map_err(Into::into)
        })
    }

    fn conjoin<C: DecisionControl>(
        &self,
        edit: &mut PreparedEffectTermEdit<'_>,
        added: PreparedEffectPredicate,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        let previous = edit.read().retain_predicate(&self.predicate, control)?;
        for _ in 0..2 {
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
        }
        edit.and(vec![previous, added], control)
    }

    /// Retain the exact relation in the original graph. Admission, bounded
    /// copies and all term allocations precede the atomic publication of R.
    pub(crate) fn constrain_relation<C: DecisionControl>(
        &mut self,
        graph: &mut PreparedEffectGraph,
        relation: PreparedEffectRelation,
        actual: &PreparedEffectRow,
        permitted: &PreparedEffectRow,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>>
    where
        C::Error: From<EffectConstraintEnvironmentError>,
    {
        self.validate_row(actual, graph.read(), control)?;
        self.validate_row(permitted, graph.read(), control)?;
        let mut edit = graph.edit_terms();
        let actual = edit.read().retain_row(actual, control)?;
        let permitted = edit.read().retain_row(permitted, control)?;
        let added = edit.relate(relation, actual, permitted, control)?;
        let predicate = self.conjoin(&mut edit, added, control)?;
        control
            .charge(DecisionWork::Visit)
            .map_err(PreparedEffectFailure::Control)?;
        edit.commit()?;
        self.predicate = predicate;
        Ok(())
    }

    pub(crate) fn restore_predicate<C: DecisionControl>(
        &mut self,
        predicate: &PreparedEffectPredicate,
        graph: &mut PreparedEffectGraph,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>>
    where
        C::Error: From<EffectConstraintEnvironmentError>,
    {
        graph
            .read()
            .visit_predicate_parameters(predicate, control, &mut |reference, _| {
                self.validate_reference(reference).map_err(Into::into)
            })?;
        let mut edit = graph.edit_terms();
        let added = edit.read().retain_predicate(predicate, control)?;
        let predicate = self.conjoin(&mut edit, added, control)?;
        control
            .charge(DecisionWork::Visit)
            .map_err(PreparedEffectFailure::Control)?;
        edit.commit()?;
        self.predicate = predicate;
        Ok(())
    }

    pub(crate) fn restore_completed_inherited<C: DecisionControl>(
        &mut self,
        variable: GenericEffectReference,
        row: &PreparedEffectRow,
        graph: &mut PreparedEffectGraph,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>>
    where
        C::Error: From<EffectConstraintEnvironmentError>,
    {
        control
            .charge(DecisionWork::Visit)
            .map_err(PreparedEffectFailure::Control)?;
        let parameter = self.parameters.get(&variable).ok_or_else(|| {
            PreparedEffectFailure::Control(C::Error::from(
                EffectConstraintEnvironmentError::ForeignVariable {
                    variable: variable.clone(),
                },
            ))
        })?;
        if parameter.restored {
            return Err(PreparedEffectFailure::Control(
                EffectConstraintEnvironmentError::NonCanonicalScope.into(),
            ));
        }
        control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        let actual = PreparedEffectRow::Canonical(EffectRow::open(
            crate::effects::EffectSet::new(),
            variable.clone(),
        ));
        self.constrain_relation(graph, PreparedEffectRelation::Equal, &actual, row, control)?;
        self.parameters
            .get_mut(&variable)
            .expect("validated inherited variable")
            .restored = true;
        Ok(())
    }
}
