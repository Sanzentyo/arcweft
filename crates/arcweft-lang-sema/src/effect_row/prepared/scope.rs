//! Read dependencies stay immutable while one affine edit builds new facts.

use super::transaction::{PreparedEffectDeltaData, PreparedEffectStateId};
use super::*;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub(crate) struct PreparedEffectReadScope {
    pub(super) data: Arc<ReadScopeData>,
}

#[derive(Debug)]
pub(super) struct ReadScopeData {
    issuer: Arc<PreparedEffectIssuer>,
    base: PreparedEffectStateId,
    pub(super) ids: Arc<[u64]>,
    pub(super) layers: BTreeMap<u64, Arc<PreparedEffectDeltaData>>,
    pub(super) functions: BTreeMap<u64, Arc<Function>>,
    pub(super) rows: BTreeMap<u64, Arc<RowTerm>>,
    pub(super) predicates: BTreeMap<u64, Arc<PredicateTerm>>,
    pub(super) choices: BTreeSet<u64>,
}

#[derive(Debug)]
pub(super) struct ActiveEffectScope {
    pub(super) input: PreparedEffectReadScope,
    pub(super) functions: BTreeMap<u64, Arc<Function>>,
}

impl PreparedEffectReadScope {
    pub(super) fn validate<C: DecisionControl>(
        &self,
        graph: &PreparedEffectGraph,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        control
            .charge(DecisionWork::Visit)
            .map_err(PreparedEffectFailure::Control)?;
        if !Arc::ptr_eq(&self.data.issuer, &graph.issuer) {
            return Err(PreparedEffectInvariant::ForeignIssuer.into());
        }
        if self.data.base != graph.state {
            return Err(PreparedEffectInvariant::DeltaBase.into());
        }
        Ok(())
    }

    pub(super) fn same_origin<C: DecisionControl>(
        &self,
        other: &Self,
        control: &mut C,
    ) -> Result<bool, PreparedEffectFailure<C::Error>> {
        if self.data.layers.len() != other.data.layers.len() {
            return Ok(false);
        }
        for ((left_id, left), (right_id, right)) in self.data.layers.iter().zip(&other.data.layers)
        {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            if left_id != right_id || !Arc::ptr_eq(left, right) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

impl PreparedEffectGraph {
    /// The scope shares definitions and whole original obligations, not the
    /// affine delta token needed to publish them. Flat layers avoid recursive
    /// ownership chains when a deep sequence of source preparations is dropped.
    pub(crate) fn read_scope<'a, C: DecisionControl>(
        &self,
        inputs: impl IntoIterator<Item = &'a PreparedEffectDelta>,
        control: &mut C,
    ) -> Result<PreparedEffectReadScope, PreparedEffectFailure<C::Error>> {
        let mut layers = BTreeMap::<u64, Arc<PreparedEffectDeltaData>>::new();
        for input in inputs {
            self.validate_delta(input, control)?;
            if let Some(dependencies) = &input.dependencies {
                dependencies.validate(self, control)?;
                for (&id, layer) in &dependencies.data.layers {
                    control
                        .charge(DecisionWork::Visit)
                        .map_err(PreparedEffectFailure::Control)?;
                    if let Some(previous) = layers.get(&id) {
                        if !Arc::ptr_eq(previous, layer) {
                            return Err(PreparedEffectInvariant::ReadScopeConflict.into());
                        }
                    } else {
                        control
                            .charge(DecisionWork::Node)
                            .map_err(PreparedEffectFailure::Control)?;
                        layers.insert(id, Arc::clone(layer));
                    }
                }
            }
            let id = input.data.ordinal;
            if let Some(previous) = layers.get(&id) {
                if !Arc::ptr_eq(previous, &input.data) {
                    return Err(PreparedEffectInvariant::ReadScopeConflict.into());
                }
            } else {
                control
                    .charge(DecisionWork::Node)
                    .map_err(PreparedEffectFailure::Control)?;
                layers.insert(id, Arc::clone(&input.data));
            }
        }
        let mut functions = BTreeMap::<u64, Arc<Function>>::new();
        let mut origins = BTreeMap::<u64, u64>::new();
        let mut rows = BTreeMap::new();
        let mut predicates = BTreeMap::new();
        let mut choices = BTreeSet::new();
        let mut ids = Vec::new();
        for (&origin, layer) in &layers {
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
            ids.push(origin);
            for (&id, function) in &layer.functions {
                control
                    .charge(DecisionWork::Visit)
                    .map_err(PreparedEffectFailure::Control)?;
                if let Some(previous) = functions.get(&id) {
                    let precedes = layer
                        .inputs
                        .as_ref()
                        .is_some_and(|inputs| inputs.binary_search(&origins[&id]).is_ok());
                    let update = matches!((previous.as_ref(), function.as_ref()),
                        (Function::Scheme(Scheme { header: before, state: SchemeState::Declared }),
                         Function::Scheme(Scheme { header: after, state: SchemeState::BodyPrepared { .. } })) if Arc::ptr_eq(before, after));
                    if !precedes || !update {
                        return Err(PreparedEffectInvariant::ReadScopeConflict.into());
                    }
                }
                control
                    .charge(DecisionWork::Node)
                    .map_err(PreparedEffectFailure::Control)?;
                control
                    .charge(DecisionWork::Node)
                    .map_err(PreparedEffectFailure::Control)?;
                functions.insert(id, Arc::clone(function));
                origins.insert(id, origin);
            }
            for (&id, value) in &layer.rows {
                control
                    .charge(DecisionWork::Node)
                    .map_err(PreparedEffectFailure::Control)?;
                if rows.insert(id, Arc::clone(value)).is_some() {
                    return Err(PreparedEffectInvariant::ReadScopeConflict.into());
                }
            }
            for (&id, value) in &layer.predicates {
                control
                    .charge(DecisionWork::Node)
                    .map_err(PreparedEffectFailure::Control)?;
                if predicates.insert(id, Arc::clone(value)).is_some() {
                    return Err(PreparedEffectInvariant::ReadScopeConflict.into());
                }
            }
            for &id in &layer.choices {
                control
                    .charge(DecisionWork::Node)
                    .map_err(PreparedEffectFailure::Control)?;
                if !choices.insert(id) {
                    return Err(PreparedEffectInvariant::ReadScopeConflict.into());
                }
            }
        }
        control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        Ok(PreparedEffectReadScope {
            data: Arc::new(ReadScopeData {
                issuer: Arc::clone(&self.issuer),
                base: self.state,
                ids: ids.into(),
                layers,
                functions,
                rows,
                predicates,
                choices,
            }),
        })
    }

    pub(crate) fn edit_scope<C: DecisionControl>(
        &mut self,
        input: &PreparedEffectReadScope,
        control: &mut C,
    ) -> Result<PreparedEffectScopeEdit<'_>, PreparedEffectFailure<C::Error>> {
        if self.active_scope.is_some() {
            return Err(PreparedEffectInvariant::ReadScopeActive.into());
        }
        input.validate(self, control)?;
        let checkpoint = self.checkpoint(control)?;
        self.active_scope = Some(ActiveEffectScope {
            input: input.clone(),
            functions: BTreeMap::new(),
        });
        Ok(PreparedEffectScopeEdit {
            graph: self,
            checkpoint: Some(checkpoint),
        })
    }
}

/// The edit exposes algebra operations, not the mutable graph or its checkpoint
/// stack. Finishing detaches only its own writes; dropping rolls them back.
pub(crate) struct PreparedEffectScopeEdit<'a> {
    graph: &'a mut PreparedEffectGraph,
    checkpoint: Option<PreparedEffectCheckpoint>,
}

impl Drop for PreparedEffectScopeEdit<'_> {
    fn drop(&mut self) {
        if let Some(checkpoint) = &mut self.checkpoint {
            self.graph
                .rollback(checkpoint)
                .expect("scope edit owns the last checkpoint");
        }
        self.graph.active_scope = None;
    }
}

impl PreparedEffectScopeEdit<'_> {
    pub(crate) fn read(&self) -> PreparedEffectReadView<'_> {
        self.graph.read()
    }

    pub(crate) fn finish<C: DecisionControl>(
        mut self,
        control: &mut C,
    ) -> Result<PreparedEffectDelta, PreparedEffectFailure<C::Error>> {
        let delta = self
            .graph
            .extract_delta(self.checkpoint.as_mut().expect("open scope edit"), control)?;
        self.checkpoint = None;
        Ok(delta)
    }

    pub(crate) fn declare<C: DecisionControl>(
        &mut self,
        parameters: Vec<GenericEffectReference>,
        domain: EffectPredicate,
        control: &mut C,
    ) -> Result<PreparedEffectSchemeRef, PreparedEffectFailure<C::Error>> {
        self.graph.declare(parameters, domain, control)
    }
    pub(crate) fn declare_unknown<C: DecisionControl>(
        &mut self,
        parameters: Vec<GenericEffectReference>,
        eligibility: PreparedEffectUnknownEligibility,
        control: &mut C,
    ) -> Result<PreparedEffectUnknownRef, PreparedEffectFailure<C::Error>> {
        self.graph.declare_unknown(parameters, eligibility, control)
    }
    pub(crate) fn application<C: DecisionControl>(
        &mut self,
        function: &PreparedEffectSchemeRef,
        arguments: Vec<PreparedEffectRow>,
        control: &mut C,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        self.graph.application(function, arguments, control)
    }
    pub(crate) fn unknown_application<C: DecisionControl>(
        &mut self,
        function: &PreparedEffectUnknownRef,
        arguments: Vec<PreparedEffectRow>,
        control: &mut C,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        self.graph.unknown_application(function, arguments, control)
    }
    pub(crate) fn union<C: DecisionControl>(
        &mut self,
        rows: Vec<PreparedEffectRow>,
        control: &mut C,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        self.graph.union(rows, control)
    }
    pub(crate) fn prepare_body<C: DecisionControl>(
        &mut self,
        function: &PreparedEffectSchemeRef,
        body: PreparedEffectRow,
        obligations: Vec<PreparedEffectPredicate>,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        self.graph
            .prepare_body(function, body, obligations, control)
    }
    pub(crate) fn relate<C: DecisionControl>(
        &mut self,
        relation: PreparedEffectRelation,
        actual: PreparedEffectRow,
        permitted: PreparedEffectRow,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        self.graph.relate(relation, actual, permitted, control)
    }
    pub(crate) fn and<C: DecisionControl>(
        &mut self,
        values: Vec<PreparedEffectPredicate>,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        self.graph.and(values, control)
    }
    pub(crate) fn or<C: DecisionControl>(
        &mut self,
        values: Vec<PreparedEffectPredicate>,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        self.graph.or(values, control)
    }
    pub(crate) fn not<C: DecisionControl>(
        &mut self,
        value: PreparedEffectPredicate,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        self.graph.not(value, control)
    }
    pub(crate) fn add_scoped_obligation<C: DecisionControl>(
        &mut self,
        parameters: Vec<GenericEffectReference>,
        domain: PreparedEffectPredicate,
        predicate: PreparedEffectPredicate,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        self.graph
            .add_scoped_obligation(parameters, domain, predicate, control)
    }
    pub(crate) fn choice<C: DecisionControl>(
        &mut self,
        control: &mut C,
    ) -> Result<PreparedEffectChoice, PreparedEffectFailure<C::Error>> {
        self.graph.choice(control)
    }
    pub(crate) fn select_row<C: DecisionControl>(
        &mut self,
        choice: &PreparedEffectChoice,
        yes: PreparedEffectRow,
        no: PreparedEffectRow,
        control: &mut C,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        self.graph.select_row(choice, yes, no, control)
    }
    pub(crate) fn select_predicate<C: DecisionControl>(
        &mut self,
        choice: &PreparedEffectChoice,
        yes: PreparedEffectPredicate,
        no: PreparedEffectPredicate,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        self.graph.select_predicate(choice, yes, no, control)
    }
    pub(crate) fn substitute<C: DecisionControl>(
        &mut self,
        row: &PreparedEffectRow,
        arguments: &BTreeMap<GenericEffectReference, PreparedEffectRow>,
        control: &mut C,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        self.graph.substitute(row, arguments, control)
    }
    pub(crate) fn substitute_predicate<C: DecisionControl>(
        &mut self,
        value: &PreparedEffectPredicate,
        arguments: &BTreeMap<GenericEffectReference, PreparedEffectRow>,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        self.graph.substitute_predicate(value, arguments, control)
    }
    pub(crate) fn edit_terms(&mut self) -> PreparedEffectTermEdit<'_> {
        self.graph.edit_terms()
    }
}
