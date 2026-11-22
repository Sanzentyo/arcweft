//! Read dependencies stay immutable while one affine edit builds new facts.

use super::transaction::{PreparedEffectDeltaData, PreparedEffectStateId};
use super::*;

#[cfg(test)]
mod tests;

type DefinitionIndex<T> = Arc<BTreeMap<u64, Arc<T>>>;

#[derive(Clone, Debug)]
pub(crate) struct PreparedEffectReadScope {
    issuer: Arc<PreparedEffectIssuer>,
    base: PreparedEffectStateId,
    detached: Option<Arc<ReadScopeData>>,
}

#[derive(Debug)]
pub(super) struct ReadScopeData {
    pub(super) ids: Arc<[u64]>,
    pub(super) layers: BTreeMap<u64, Arc<PreparedEffectDeltaData>>,
    pub(super) functions: DefinitionIndex<Function>,
    pub(super) rows: DefinitionIndex<RowTerm>,
    pub(super) predicates: DefinitionIndex<PredicateTerm>,
    pub(super) choices: Arc<BTreeSet<u64>>,
}

#[derive(Debug)]
pub(super) struct ActiveEffectScope {
    pub(super) input: PreparedEffectReadScope,
    pub(super) functions: BTreeMap<u64, Arc<Function>>,
}

impl PreparedEffectReadScope {
    pub(super) fn detached(&self) -> Option<&ReadScopeData> {
        self.detached.as_deref()
    }

    pub(super) fn is_live_at(&self, base: PreparedEffectStateId) -> bool {
        self.base == base && self.detached.is_none()
    }

    fn layer_count(&self) -> usize {
        self.detached().map_or(0, |data| data.layers.len())
    }

    fn same_selection(&self, other: &Self) -> bool {
        self.base == other.base
            && Arc::ptr_eq(&self.issuer, &other.issuer)
            && match (&self.detached, &other.detached) {
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                (None, None) => true,
                _ => false,
            }
    }

    pub(super) fn validate<C: DecisionControl>(
        &self,
        graph: &PreparedEffectGraph,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        self.validate_issuer(graph, &mut Work(control))?;
        if self.base != graph.state {
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
        if !Arc::ptr_eq(&self.issuer, &graph.relation.issuer) {
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
        if !graph.retains_state(self.base) && !witnessed(self.base) {
            return Err(PreparedEffectInvariant::DeltaBase.into());
        }
        Ok(())
    }

    pub(super) fn require_enclosing<C: DecisionControl>(
        &self,
        enclosing: Option<&ActiveEffectScope>,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        if let Some(enclosing) = enclosing
            && !self.contains_layers(&enclosing.input, &mut Work(control))?
        {
            return Err(PreparedEffectInvariant::ReadScopeConflict.into());
        }
        Ok(())
    }

    fn contains_layers<C: DecisionControl>(
        &self,
        other: &Self,
        work: &mut Work<'_, C>,
    ) -> Result<bool, PreparedEffectFailure<C::Error>> {
        let Some(required) = other.detached() else {
            return Ok(true);
        };
        let Some(selected) = self.detached() else {
            return Ok(false);
        };
        if std::ptr::eq(selected, required) {
            return Ok(true);
        }
        if selected.layers.len() < required.layers.len() {
            return Ok(false);
        }
        for (&id, layer) in &required.layers {
            work.charge(DecisionWork::Visit)?;
            if !selected
                .layers
                .get(&id)
                .is_some_and(|selected| Arc::ptr_eq(selected, layer))
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn same_origin<C: DecisionControl>(
        &self,
        other: &Self,
        control: &mut C,
    ) -> Result<bool, PreparedEffectFailure<C::Error>> {
        let (Some(left), Some(right)) = (self.detached(), other.detached()) else {
            return Ok(self.detached.is_none() && other.detached.is_none());
        };
        if left.layers.len() != right.layers.len() {
            return Ok(false);
        }
        for ((left_id, left), (right_id, right)) in left.layers.iter().zip(&right.layers) {
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
    fn bind_read_scope(&self, detached: Option<ReadScopeData>) -> PreparedEffectReadScope {
        PreparedEffectReadScope {
            issuer: Arc::clone(&self.relation.issuer),
            base: self.state,
            detached: detached.map(Arc::new),
        }
    }

    /// The scope shares definitions and whole original obligations, not the
    /// affine delta token needed to publish them. Flat layers avoid recursive
    /// ownership chains when a deep sequence of source preparations is dropped.
    pub(crate) fn read_scope<'a, C: DecisionControl>(
        &self,
        inputs: impl IntoIterator<Item = &'a PreparedEffectDelta>,
        control: &mut C,
    ) -> Result<PreparedEffectReadScope, PreparedEffectFailure<C::Error>> {
        let mut inputs = inputs.into_iter().peekable();
        let first = inputs.next();
        if let Some(delta) = first
            && inputs.peek().is_none()
            && let Some(dependencies) = &delta.dependencies
            && self.can_reuse_read_scope(dependencies)
        {
            match (delta.layer(), dependencies.detached()) {
                (None, _) => {
                    self.validate_delta_base(delta, control)?;
                    return Ok(dependencies.clone());
                }
                (Some(layer), Some(data)) if layer.functions.is_empty() => {
                    self.validate_delta_base(delta, control)?;
                    return self.extend_read_scope(data, layer, control);
                }
                _ => {}
            }
        }
        self.delta_scope_data(
            first.into_iter().chain(inputs),
            control,
            |input, control| {
                // The assembled view meters the definitions it actually inspects.
                self.validate_delta_base(input, control)
            },
        )
        .map(|data| self.bind_read_scope(data))
    }

    pub(super) fn delta_scope_data<'a, C: DecisionControl>(
        &self,
        inputs: impl IntoIterator<Item = &'a PreparedEffectDelta>,
        control: &mut C,
        mut validate: impl FnMut(
            &'a PreparedEffectDelta,
            &mut C,
        ) -> Result<(), PreparedEffectFailure<C::Error>>,
    ) -> Result<Option<ReadScopeData>, PreparedEffectFailure<C::Error>> {
        let mut inputs = inputs.into_iter().peekable();
        let mut layers = BTreeMap::new();
        let mut work = Work(control);
        if let Some(active) = self
            .active_scope
            .as_ref()
            .and_then(|scope| scope.input.detached())
        {
            for layer in active.layers.values() {
                retain_definition(&mut layers, layer.ordinal, layer, &mut work)?;
            }
        } else if inputs.peek().is_none() {
            // Issuing a base-only read still observes the caller's control.
            work.charge(DecisionWork::Visit)?;
        }
        for input in inputs {
            validate(input, work.0)?;
            if let Some(dependencies) = input
                .dependencies
                .as_ref()
                .and_then(PreparedEffectReadScope::detached)
            {
                for layer in dependencies.layers.values() {
                    retain_definition(&mut layers, layer.ordinal, layer, &mut work)?;
                }
            }
            if let Some(layer) = input.layer() {
                retain_definition(&mut layers, layer.ordinal, layer, &mut work)?;
            } else {
                work.charge(DecisionWork::Visit)?;
            }
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
        let mut scopes = scopes.into_iter();
        let first = scopes.next();
        if first.is_none() && self.active_scope.is_none() {
            work.charge(DecisionWork::Visit)?;
            return Ok(self.bind_read_scope(None));
        }
        let mut additional = None;
        if let Some(scope) = first.filter(|scope| self.can_reuse_read_scope(scope)) {
            // This view already authenticated its layers at the current state.
            // A live base with identical selected layers adds no new dependency.
            scope.validate_issuer(self, &mut work)?;
            for next in scopes.by_ref() {
                next.validate_issuer(self, &mut work)?;
                work.charge(DecisionWork::Visit)?;
                if !self.retains_state(next.base) || !scope.contains_layers(next, &mut work)? {
                    additional = Some(next);
                    break;
                }
            }
            if additional.is_none() {
                return Ok(scope.clone());
            }
        }
        let mut layers = BTreeMap::new();
        let mut bases = BTreeMap::new();
        let mut reusable: Option<PreparedEffectReadScope> = None;
        let mut retain_scope = |scope: &PreparedEffectReadScope| {
            scope.validate_issuer(self, &mut work)?;
            self.require_detached_base(scope.base, &mut bases, &mut work)?;
            if let Some(data) = scope.detached() {
                for layer in data.layers.values() {
                    retain_definition(&mut layers, layer.ordinal, layer, &mut work)?;
                }
            }
            if self.can_reuse_read_scope(scope)
                && reusable
                    .as_ref()
                    .is_none_or(|current| current.layer_count() < scope.layer_count())
            {
                reusable = Some(scope.clone());
            }
            Ok::<(), PreparedEffectFailure<C::Error>>(())
        };
        for scope in first.into_iter().chain(additional).chain(scopes) {
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
            && scope.layer_count() == layers.len()
        {
            return Ok(scope);
        }
        self.assemble_read_scope(layers, order, &mut work)
            .map(|data| self.bind_read_scope(data))
    }

    fn can_reuse_read_scope(&self, scope: &PreparedEffectReadScope) -> bool {
        scope.base == self.state
            && self
                .active_scope
                .as_ref()
                .is_none_or(|active| active.input.same_selection(scope))
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
    ) -> Result<Option<ReadScopeData>, PreparedEffectFailure<C::Error>> {
        if layers.is_empty() {
            return Ok(None);
        }
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
        Ok(Some(ReadScopeData {
            ids: ids.into(),
            layers,
            functions: Arc::new(functions),
            rows: Arc::new(rows),
            predicates: Arc::new(predicates),
            choices: Arc::new(choices),
        }))
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
        let checkpoint = edit.checkpoint.as_ref().expect("open scope edit");
        let reads = if edit
            .graph
            .checkpoint_has_transition(checkpoint)
            .map_err(PreparedEffectFailure::Invariant)?
        {
            let delta = edit.finish(control)?;
            self.read_scope([&delta], control)?
        } else {
            Work(control).charge(DecisionWork::Visit)?;
            drop(edit);
            input.clone()
        };
        Ok((output, reads))
    }

    /// Terms keep their original definitions for every read scope. An edit
    /// that adds terms to its exact input can share the other indexes, since
    /// there is no function replacement to reconcile against the live graph.
    fn extend_read_scope<C: DecisionControl>(
        &self,
        old: &ReadScopeData,
        added: &Arc<PreparedEffectDeltaData>,
        control: &mut C,
    ) -> Result<PreparedEffectReadScope, PreparedEffectFailure<C::Error>> {
        if old.ids.last().is_some_and(|id| *id >= added.ordinal) {
            return Err(PreparedEffectInvariant::ReadScopeConflict.into());
        }
        let mut work = Work(control);
        let mut layers = BTreeMap::new();
        let mut ids = Vec::new();
        for (&id, layer) in &old.layers {
            work.charge(DecisionWork::Node)?;
            layers.insert(id, Arc::clone(layer));
            work.charge(DecisionWork::Node)?;
            ids.push(id);
        }
        retain_definition(&mut layers, added.ordinal, added, &mut work)?;
        work.charge(DecisionWork::Node)?;
        ids.push(added.ordinal);
        let rows = extend_definitions(&old.rows, &added.rows, &mut work)?;
        let predicates = extend_definitions(&old.predicates, &added.predicates, &mut work)?;
        let choices = if added.choices.is_empty() {
            Arc::clone(&old.choices)
        } else {
            let mut choices = BTreeSet::new();
            for &id in old.choices.iter().chain(&added.choices) {
                work.charge(DecisionWork::Node)?;
                choices.insert(id);
            }
            Arc::new(choices)
        };
        work.charge(DecisionWork::Node)?;
        work.charge(DecisionWork::Node)?;
        Ok(self.bind_read_scope(Some(ReadScopeData {
            ids: ids.into(),
            layers,
            functions: Arc::clone(&old.functions),
            rows,
            predicates,
            choices,
        })))
    }
}

fn extend_definitions<T, C: DecisionControl>(
    base: &DefinitionIndex<T>,
    added: &BTreeMap<u64, Arc<T>>,
    work: &mut Work<'_, C>,
) -> Result<DefinitionIndex<T>, PreparedEffectFailure<C::Error>> {
    if added.is_empty() {
        return Ok(Arc::clone(base));
    }
    let mut definitions = BTreeMap::new();
    for (&id, value) in base.iter() {
        work.charge(DecisionWork::Node)?;
        definitions.insert(id, Arc::clone(value));
    }
    for (&id, value) in added {
        retain_definition(&mut definitions, id, value, work)?;
    }
    Ok(Arc::new(definitions))
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
