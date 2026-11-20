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
        self.validate_issuer(graph, &mut Work(control))?;
        if self.data.base != graph.state {
            return Err(PreparedEffectInvariant::DeltaBase.into());
        }
        Ok(())
    }

    fn validate_issuer<C: DecisionControl>(
        &self,
        graph: &PreparedEffectGraph,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        work.charge(DecisionWork::Visit)?;
        if !Arc::ptr_eq(&self.data.issuer, &graph.relation.issuer) {
            return Err(PreparedEffectInvariant::ForeignIssuer.into());
        }
        Ok(())
    }

    pub(super) fn validate_ancestry<C: DecisionControl>(
        &self,
        graph: &PreparedEffectGraph,
        witnessed: impl FnOnce(PreparedEffectStateId) -> bool,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        self.validate_issuer(graph, &mut Work(control))?;
        if !graph.retains_state(self.data.base) && !witnessed(self.data.base) {
            return Err(PreparedEffectInvariant::DeltaBase.into());
        }
        Ok(())
    }

    pub(super) fn require_enclosing<C: DecisionControl>(
        &self,
        enclosing: Option<&ActiveEffectScope>,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        if let Some(enclosing) = enclosing {
            for (&id, layer) in &enclosing.input.data.layers {
                control
                    .charge(DecisionWork::Visit)
                    .map_err(PreparedEffectFailure::Control)?;
                if !self
                    .data
                    .layers
                    .get(&id)
                    .is_some_and(|selected| Arc::ptr_eq(selected, layer))
                {
                    return Err(PreparedEffectInvariant::ReadScopeConflict.into());
                }
            }
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
        self.delta_scope_data(inputs, control, |input, control| {
            // The assembled view meters the definitions it actually inspects.
            self.validate_delta_base(input, control)
        })
        .map(|data| PreparedEffectReadScope {
            data: Arc::new(data),
        })
    }

    pub(super) fn delta_scope_data<'a, C: DecisionControl>(
        &self,
        inputs: impl IntoIterator<Item = &'a PreparedEffectDelta>,
        control: &mut C,
        mut validate: impl FnMut(
            &'a PreparedEffectDelta,
            &mut C,
        ) -> Result<(), PreparedEffectFailure<C::Error>>,
    ) -> Result<ReadScopeData, PreparedEffectFailure<C::Error>> {
        let mut layers = BTreeMap::new();
        let mut work = Work(control);
        if let Some(active) = &self.active_scope {
            for layer in active.input.data.layers.values() {
                retain_layer(&mut layers, layer, &mut work)?;
            }
        }
        for input in inputs {
            validate(input, work.0)?;
            if let Some(dependencies) = &input.dependencies {
                for layer in dependencies.data.layers.values() {
                    retain_layer(&mut layers, layer, &mut work)?;
                }
            }
            retain_layer(&mut layers, &input.data, &mut work)?;
        }
        let order = ordered_layers(&layers, &mut work)?;
        self.assemble_read_scope(layers, order, &mut work)
    }

    /// Read only through selected layers whose bases still have an ancestry
    /// witness. An enclosing extracted layer can witness the bases of nested
    /// probes. Joining reads never reissues the permits to publish the layers.
    pub(crate) fn compose_read_scopes<'a, C: DecisionControl>(
        &self,
        scopes: impl IntoIterator<Item = &'a PreparedEffectReadScope>,
        control: &mut C,
    ) -> Result<PreparedEffectReadScope, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        let mut scopes = scopes.into_iter().peekable();
        let first = scopes.next();
        // The retained view already authenticated these layers at this state.
        // An identical enclosing view introduces no new dependencies.
        if let Some(scope) = first
            && scopes.peek().is_none()
            && self.can_reuse_read_scope(scope)
        {
            scope.validate_issuer(self, &mut work)?;
            return Ok(scope.clone());
        }
        let mut layers = BTreeMap::new();
        let mut bases = BTreeMap::new();
        let mut reusable: Option<PreparedEffectReadScope> = None;
        let mut retain_scope = |scope: &PreparedEffectReadScope| {
            scope.validate_issuer(self, &mut work)?;
            self.require_detached_base(scope.data.base, &mut bases, &mut work)?;
            for layer in scope.data.layers.values() {
                retain_layer(&mut layers, layer, &mut work)?;
            }
            if self.can_reuse_read_scope(scope)
                && reusable
                    .as_ref()
                    .is_none_or(|current| current.data.layers.len() < scope.data.layers.len())
            {
                reusable = Some(scope.clone());
            }
            Ok::<(), PreparedEffectFailure<C::Error>>(())
        };
        for scope in first.into_iter().chain(scopes) {
            retain_scope(scope)?;
        }
        if let Some(active) = &self.active_scope {
            retain_scope(&active.input)?;
        }
        let order = ordered_layers(&layers, &mut work)?;
        self.confirm_scope_ancestry(&layers, &order, bases, &mut work)?;
        // Layer admission has checked that duplicate ordinals have identical
        // definitions. If the union has the same size, this scope contains
        // every selected layer and has already assembled the current overlay.
        if let Some(scope) = reusable
            && scope.data.layers.len() == layers.len()
        {
            return Ok(scope);
        }
        self.assemble_read_scope(layers, order, &mut work)
            .map(|data| PreparedEffectReadScope {
                data: Arc::new(data),
            })
    }

    fn can_reuse_read_scope(&self, scope: &PreparedEffectReadScope) -> bool {
        scope.data.base == self.state
            && self
                .active_scope
                .as_ref()
                .is_none_or(|active| Arc::ptr_eq(&active.input.data, &scope.data))
    }

    fn require_detached_base<C: DecisionControl>(
        &self,
        base: PreparedEffectStateId,
        required: &mut BTreeMap<PreparedEffectStateId, bool>,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        work.charge(DecisionWork::Visit)?;
        if !self.retains_state(base)
            && let std::collections::btree_map::Entry::Vacant(entry) = required.entry(base)
        {
            work.charge(DecisionWork::Node)?;
            entry.insert(false);
        }
        Ok(())
    }

    fn confirm_scope_ancestry<C: DecisionControl>(
        &self,
        layers: &BTreeMap<u64, Arc<PreparedEffectDeltaData>>,
        order: &[(PreparedEffectStateId, u64)],
        mut required: BTreeMap<PreparedEffectStateId, bool>,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        for &(base, _) in order {
            self.require_detached_base(base, &mut required, work)?;
        }
        if required.is_empty() {
            return Ok(());
        }
        for &(base, ordinal) in order {
            work.charge(DecisionWork::Visit)?;
            if required.get(&base) == Some(&false) {
                return Err(PreparedEffectInvariant::DeltaBase.into());
            }
            let layer = &layers[&ordinal];
            // The ancestry is sorted by state id. Look up only outstanding
            // bases in this layer's range, without copying its revision history.
            let first = layer.ancestors.first().copied().unwrap_or(layer.result);
            let last = layer.ancestors.last().copied().unwrap_or(layer.result);
            for (&state, witnessed) in
                required.range_mut(first.min(layer.result)..=last.max(layer.result))
            {
                work.charge(DecisionWork::Visit)?;
                if !*witnessed {
                    *witnessed =
                        state == layer.result || layer.ancestors.binary_search(&state).is_ok();
                }
            }
        }
        for witnessed in required.into_values() {
            work.charge(DecisionWork::Visit)?;
            if !witnessed {
                return Err(PreparedEffectInvariant::DeltaBase.into());
            }
        }
        Ok(())
    }

    fn assemble_read_scope<C: DecisionControl>(
        &self,
        layers: BTreeMap<u64, Arc<PreparedEffectDeltaData>>,
        order: Vec<(PreparedEffectStateId, u64)>,
        work: &mut Work<'_, C>,
    ) -> Result<ReadScopeData, PreparedEffectFailure<C::Error>> {
        let mut functions = BTreeMap::<u64, Arc<Function>>::new();
        let mut origins = BTreeMap::<u64, u64>::new();
        let mut rows = BTreeMap::new();
        let mut predicates = BTreeMap::new();
        let mut choices = BTreeSet::new();
        let mut ids = Vec::new();
        for &origin in layers.keys() {
            work.charge(DecisionWork::Node)?;
            ids.push(origin);
        }
        for (_, origin) in order {
            let layer = &layers[&origin];
            for (&id, function) in &layer.functions {
                work.charge(DecisionWork::Visit)?;
                if let Some(previous) = functions.get(&id) {
                    if Arc::ptr_eq(previous, function) {
                        continue;
                    }
                    let previous_layer = &layers[&origins[&id]];
                    let precedes = layer.inputs.as_ref().is_some_and(|inputs| {
                        inputs.binary_search(&previous_layer.ordinal).is_ok()
                    }) || (previous_layer.base < layer.base
                        && (previous_layer.result == layer.base
                            || previous_layer.ancestors.binary_search(&layer.base).is_ok()));
                    if !precedes || !prepared_body_of(previous, function) {
                        return Err(PreparedEffectInvariant::ReadScopeConflict.into());
                    }
                }
                work.charge(DecisionWork::Node)?;
                work.charge(DecisionWork::Node)?;
                functions.insert(id, Arc::clone(function));
                origins.insert(id, origin);
            }
            for (&id, value) in &layer.rows {
                retain_definition(&mut rows, id, value, work)?;
            }
            for (&id, value) in &layer.predicates {
                retain_definition(&mut predicates, id, value, work)?;
            }
            for &id in &layer.choices {
                work.charge(DecisionWork::Node)?;
                choices.insert(id);
            }
        }
        for (&id, detached) in &mut functions {
            work.charge(DecisionWork::Visit)?;
            let live = self
                .active_scope
                .as_ref()
                .and_then(|scope| scope.functions.get(&id))
                .or_else(|| self.relation.functions.get(&id));
            if let Some(live) = live {
                if prepared_body_of(detached, live) {
                    *detached = Arc::clone(live);
                } else if !Arc::ptr_eq(live, detached) && !prepared_body_of(live, detached) {
                    return Err(PreparedEffectInvariant::ReadScopeConflict.into());
                }
            }
        }
        work.charge(DecisionWork::Node)?;
        work.charge(DecisionWork::Node)?;
        Ok(ReadScopeData {
            issuer: Arc::clone(&self.relation.issuer),
            base: self.state,
            ids: ids.into(),
            layers,
            functions,
            rows,
            predicates,
            choices,
        })
    }

    pub(crate) fn edit_scope<C: DecisionControl>(
        &mut self,
        input: &PreparedEffectReadScope,
        control: &mut C,
    ) -> Result<PreparedEffectScopeEdit<'_>, PreparedEffectFailure<C::Error>> {
        let checkpoint = self.checkpoint_in_read_scope(input, control)?;
        Ok(PreparedEffectScopeEdit {
            graph: self,
            checkpoint: Some(checkpoint),
        })
    }

    /// Check a branch using transient derived terms. The result uses only
    /// closed values and owned evidence, so the edit rolls back on either exit.
    pub(crate) fn validate_in_read_scope<C, T, E>(
        &mut self,
        input: &PreparedEffectReadScope,
        control: &mut C,
        validate: impl FnOnce(&mut Self, &mut C) -> Result<T, E>,
    ) -> Result<T, E>
    where
        C: DecisionControl,
        E: From<PreparedEffectFailure<C::Error>>,
    {
        let edit = self.edit_scope(input, control)?;
        validate(&mut *edit.graph, control)
    }

    /// Run branch-local algebra against its selected reads. The result gets
    /// a read scope for its derived terms. The temporary edit's permit ends here.
    pub(crate) fn compute_in_read_scope<C, T, E>(
        &mut self,
        input: &PreparedEffectReadScope,
        control: &mut C,
        compute: impl FnOnce(&mut Self, &mut C) -> Result<T, E>,
    ) -> Result<(T, PreparedEffectReadScope), E>
    where
        C: DecisionControl,
        E: From<PreparedEffectFailure<C::Error>>,
    {
        let edit = self.edit_scope(input, control)?;
        let output = compute(&mut *edit.graph, control)?;
        let delta = edit.finish(control)?;
        let reads = if delta.data.base == delta.data.result {
            input.clone()
        } else {
            self.read_scope([&delta], control)?
        };
        Ok((output, reads))
    }
}

fn ordered_layers<C: DecisionControl>(
    layers: &BTreeMap<u64, Arc<PreparedEffectDeltaData>>,
    work: &mut Work<'_, C>,
) -> Result<Vec<(PreparedEffectStateId, u64)>, PreparedEffectFailure<C::Error>> {
    let mut ordered = Vec::new();
    for (&id, layer) in layers {
        work.charge(DecisionWork::Node)?;
        ordered.push((layer.base, id));
    }
    ordered.sort_unstable();
    Ok(ordered)
}

fn retain_layer<C: DecisionControl>(
    layers: &mut BTreeMap<u64, Arc<PreparedEffectDeltaData>>,
    layer: &Arc<PreparedEffectDeltaData>,
    work: &mut Work<'_, C>,
) -> Result<(), PreparedEffectFailure<C::Error>> {
    if layer.has_transition() {
        retain_definition(layers, layer.ordinal, layer, work)
    } else {
        work.charge(DecisionWork::Visit)
    }
}

fn retain_definition<T, C: DecisionControl>(
    definitions: &mut BTreeMap<u64, Arc<T>>,
    id: u64,
    value: &Arc<T>,
    work: &mut Work<'_, C>,
) -> Result<(), PreparedEffectFailure<C::Error>> {
    work.charge(DecisionWork::Visit)?;
    if let Some(previous) = definitions.get(&id) {
        if !Arc::ptr_eq(previous, value) {
            return Err(PreparedEffectInvariant::ReadScopeConflict.into());
        }
    } else {
        work.charge(DecisionWork::Node)?;
        definitions.insert(id, Arc::clone(value));
    }
    Ok(())
}

fn prepared_body_of(declaration: &Function, prepared: &Function) -> bool {
    matches!((declaration, prepared),
        (Function::Scheme(Scheme { header: before, state: SchemeState::Declared }),
         Function::Scheme(Scheme { header: after, state: SchemeState::BodyPrepared { .. } })) if Arc::ptr_eq(before, after))
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
