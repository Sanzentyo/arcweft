//! Reuse quantified original clauses within one receiving coefficient basis.

use super::{
    Arc, BTreeMap, CachedCompletion, Decision, DecisionControl, DecisionWork, FixedCells, Function,
    Layout, PreparedEffectDefinitions, PreparedEffectFailure, Work,
};

#[derive(Default)]
pub(in crate::effect_row::prepared::closure) struct InheritedObligations<'a>(&'a [Arc<Decision>]);

impl InheritedObligations<'_> {
    pub(in crate::effect_row::prepared::closure) fn get(
        &self,
        id: usize,
    ) -> Option<&Arc<Decision>> {
        self.0.get(id)
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }
}

pub(super) fn inherited_from<'a, C: DecisionControl>(
    cache: &'a [CachedCompletion],
    graph: &PreparedEffectDefinitions,
    layout: &Layout,
    fixed: &FixedCells,
    work: &mut Work<'_, C>,
) -> Result<InheritedObligations<'a>, PreparedEffectFailure<C::Error>> {
    let mut inherited = &[][..];
    if graph.obligations.is_empty() {
        return Ok(InheritedObligations::default());
    }
    for entry in cache {
        work.charge(DecisionWork::Visit)?;
        if entry.obligations.len() > inherited.len()
            && entry.matches_prefix(graph, layout, fixed, work)?
        {
            inherited = &entry.obligations;
        }
    }
    Ok(InheritedObligations(inherited))
}

impl CachedCompletion {
    fn matches_prefix<C: DecisionControl>(
        &self,
        graph: &PreparedEffectDefinitions,
        layout: &Layout,
        fixed: &FixedCells,
        work: &mut Work<'_, C>,
    ) -> Result<bool, PreparedEffectFailure<C::Error>> {
        let original = &self.source.graph;
        let original_layout = &self.completion.layout;
        // The clause already quantified its own formal inputs. Its remaining
        // cell and choice coordinates still belong to the original issuer.
        if !Arc::ptr_eq(&original.issuer, &graph.issuer)
            || !Arc::ptr_eq(&original_layout.labels, &layout.labels)
            || original.obligations.len() > graph.obligations.len()
        {
            return Ok(false);
        }
        for (before, after) in original.obligations.iter().zip(&graph.obligations) {
            work.charge(DecisionWork::Visit)?;
            if !Arc::ptr_eq(before, after) {
                return Ok(false);
            }
        }
        for (&id, before) in &original.functions {
            work.charge(DecisionWork::Visit)?;
            if original_layout.tables.get(&id) != layout.tables.get(&id) {
                return Ok(false);
            }
            let Some(after) = graph.functions.get(&id) else {
                return Ok(false);
            };
            if Arc::ptr_eq(before, after) {
                continue;
            }
            // A Named body can enter the receiving relation without changing
            // how any original application reads the header and its cells.
            let same_application = match (before.as_ref(), after.as_ref()) {
                (Function::SchemeInput(input), Function::Scheme(named))
                | (Function::Scheme(named), Function::SchemeInput(input)) => {
                    input.has_header(&named.header)
                }
                (Function::Scheme(left), Function::Scheme(right)) => {
                    Arc::ptr_eq(&left.header, &right.header)
                }
                (Function::SchemeInput(left), Function::SchemeInput(right)) => left == right,
                _ => false,
            };
            if !same_application {
                return Ok(false);
            }
        }
        if !same_definitions(&original.rows, &graph.rows, work)?
            || !same_definitions(&original.predicates, &graph.predicates, work)?
            || !self
                .completion
                .coefficients
                .fixed()
                .agrees_on(fixed, original, work)?
        {
            return Ok(false);
        }
        for choice in &original.choices {
            work.charge(DecisionWork::Visit)?;
            if !graph.choices.contains(choice) {
                return Ok(false);
            }
        }
        // The same nested attachment and completion, with the same receiving
        // fixed inputs, use identical bindings for the guard and coefficients.
        for attachment in original_layout.imports.attachments() {
            let mut found = false;
            for receiving in layout.imports.attachments() {
                work.charge(DecisionWork::Visit)?;
                if Arc::ptr_eq(&attachment.source, &receiving.source)
                    && Arc::ptr_eq(&attachment.child, &receiving.child)
                {
                    found = true;
                    break;
                }
            }
            if !found {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn same_definitions<T, C: DecisionControl>(
    original: &BTreeMap<u64, Arc<T>>,
    receiving: &BTreeMap<u64, Arc<T>>,
    work: &mut Work<'_, C>,
) -> Result<bool, PreparedEffectFailure<C::Error>> {
    for (id, term) in original {
        work.charge(DecisionWork::Visit)?;
        if !receiving
            .get(id)
            .is_some_and(|value| Arc::ptr_eq(value, term))
        {
            return Ok(false);
        }
    }
    Ok(true)
}
