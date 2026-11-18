//! One conditional minimization per immutable relation and receiving label basis.

use super::*;
use crate::effect_row::prepared::completion::ImportedEffectRelation;

#[cfg(test)]
mod tests;

pub(super) struct ImportedFunctionTable {
    pub(super) relation: Arc<ImportedRelationTable>,
    membership: Box<[Box<[Decision]>]>,
}

pub(super) struct ImportedRelationTable {
    pub(super) child: Arc<ComponentCompletion>,
    bindings: BTreeMap<Atom, Decision>,
    principal: Decision,
}

#[derive(Default)]
pub(super) struct ImportedRelations {
    pub(super) functions: BTreeMap<u64, ImportedFunctionTable>,
    relations: Vec<(Arc<ImportedEffectRelation>, Arc<ImportedRelationTable>)>,
}

impl ImportedRelations {
    pub(super) fn relation<C: DecisionControl>(
        &self,
        source: &Arc<ImportedEffectRelation>,
        work: &mut Work<'_, C>,
    ) -> Result<&ImportedRelationTable, PreparedEffectFailure<C::Error>> {
        for (original, table) in &self.relations {
            work.charge(DecisionWork::Visit)?;
            if Arc::ptr_eq(original, source) {
                return Ok(table);
            }
        }
        Err(PreparedEffectInvariant::UnclosedCell.into())
    }
}

type Cache = Vec<(Arc<ScopedEffectCompletion>, Arc<ComponentCompletion>)>;

fn cached<'a, C: DecisionControl>(
    cache: &'a Cache,
    source: &Arc<ScopedEffectCompletion>,
    work: &mut Work<'_, C>,
) -> Result<Option<&'a Arc<ComponentCompletion>>, PreparedEffectFailure<C::Error>> {
    for (owner, value) in cache {
        work.charge(DecisionWork::Visit)?;
        if Arc::ptr_eq(owner, source) {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

fn enqueue<C: DecisionControl>(
    graph: &PreparedEffectDefinitions,
    inventory: FunctionInventory<'_>,
    pending: &mut Vec<(Arc<ScopedEffectCompletion>, bool)>,
    work: &mut Work<'_, C>,
) -> Result<(), PreparedEffectFailure<C::Error>> {
    for (id, function) in &graph.functions {
        work.charge(DecisionWork::Visit)?;
        if inventory.contains(*id) {
            if let Function::Imported(imported) = function.as_ref() {
                work.charge(DecisionWork::Node)?;
                pending.push((Arc::clone(&imported.relation.completion), false));
            }
        }
    }
    for predicate in graph.predicates.values() {
        work.charge(DecisionWork::Visit)?;
        if let PredicateTerm::ImportedPrincipal(imported) = predicate.as_ref() {
            work.charge(DecisionWork::Node)?;
            pending.push((Arc::clone(&imported.completion), false));
        }
    }
    Ok(())
}

/// Actual input support is admitted by the root completion boundary. Every
/// nested relation keeps its own quantifiers, uniform choices and late inputs.
pub(super) fn lower_selected_imports<C: DecisionControl>(
    graph: &PreparedEffectDefinitions,
    labels: &[EffectId],
    inventory: FunctionInventory<'_>,
    work: &mut Work<'_, C>,
) -> Result<ImportedRelations, PreparedEffectFailure<C::Error>> {
    let mut cache = Vec::new();
    let mut pending = Vec::new();
    enqueue(graph, inventory, &mut pending, work)?;
    while let Some((source, ready)) = pending.pop() {
        work.charge(DecisionWork::Visit)?;
        if cached(&cache, &source, work)?.is_some() {
            continue;
        }
        if !ready {
            work.charge(DecisionWork::Node)?;
            pending.push((Arc::clone(&source), true));
            enqueue(&source.graph, FunctionInventory::All, &mut pending, work)?;
            continue;
        }
        let mut basis = BTreeSet::new();
        for label in labels {
            work.charge(DecisionWork::Node)?;
            basis.insert(label.clone());
        }
        let mut layout = Layout::with_labels(&source.graph, basis, work)?;
        layout.imports = assemble_selected(&source.graph, &cache, FunctionInventory::All, work)?;
        let mut component = CompiledComponent::compile(
            &source.graph,
            &source.program,
            &source.fixed,
            layout,
            work,
        )?;
        component.restrict_support(&source.graph, &source.support, work)?;
        let completed = component.complete(work)?;
        work.charge(DecisionWork::Node)?;
        cache.push((source, Arc::new(completed)));
    }
    assemble_selected(graph, &cache, inventory, work)
}

fn attach<C: DecisionControl>(
    graph: &PreparedEffectDefinitions,
    tables: &mut ImportedRelations,
    imported: &Arc<ImportedEffectRelation>,
    cache: &Cache,
    work: &mut Work<'_, C>,
) -> Result<Arc<ImportedRelationTable>, PreparedEffectFailure<C::Error>> {
    for (source, table) in &tables.relations {
        work.charge(DecisionWork::Visit)?;
        if Arc::ptr_eq(source, imported) {
            work.charge(DecisionWork::Node)?;
            return Ok(Arc::clone(table));
        }
    }
    let child =
        cached(cache, &imported.completion, work)?.ok_or(PreparedEffectInvariant::UnclosedCell)?;
    let mut bindings = BTreeMap::new();
    for (&future, actual) in &imported.inputs {
        work.charge(DecisionWork::Visit)?;
        graph.function(actual)?;
        for class in 0..child.layout.view().classes() {
            for mask in 0..child.layout.tables[&future] {
                work.charge(DecisionWork::Node)?;
                bindings.insert(
                    Atom::Cell {
                        scheme: future,
                        class,
                        mask,
                    },
                    Decision::variable(
                        Atom::Cell {
                            scheme: actual.reference().ordinal,
                            class,
                            mask,
                        },
                        work,
                    )?,
                );
            }
        }
    }
    let principal = child.completion.principal().substitute(&bindings, work)?;
    work.charge(DecisionWork::Node)?;
    let table = Arc::new(ImportedRelationTable {
        child: Arc::clone(child),
        bindings,
        principal,
    });
    work.charge(DecisionWork::Node)?;
    tables
        .relations
        .push((Arc::clone(imported), Arc::clone(&table)));
    Ok(table)
}

fn assemble_selected<C: DecisionControl>(
    graph: &PreparedEffectDefinitions,
    cache: &Cache,
    inventory: FunctionInventory<'_>,
    work: &mut Work<'_, C>,
) -> Result<ImportedRelations, PreparedEffectFailure<C::Error>> {
    let mut result = ImportedRelations::default();
    for (&id, function) in &graph.functions {
        work.charge(DecisionWork::Visit)?;
        if !inventory.contains(id) {
            continue;
        }
        if let Function::Imported(imported) = function.as_ref() {
            let relation = attach(graph, &mut result, &imported.relation, cache, work)?;
            let selected = imported.selected.reference().ordinal;
            let mut membership = Vec::new();
            for class in 0..relation.child.layout.view().classes() {
                let mut cells = Vec::new();
                for mask in 0..relation.child.layout.tables[&selected] {
                    let cell = Atom::Cell {
                        scheme: selected,
                        class,
                        mask,
                    };
                    let value = if let Some(value) = relation.child.coefficients.get(&cell) {
                        work.copy_decision(value)?
                    } else if imported.definition().is_input() {
                        Decision::variable(cell, work)?
                    } else {
                        return Err(PreparedEffectInvariant::UnclosedCell.into());
                    };
                    work.charge(DecisionWork::Node)?;
                    cells.push(value.substitute(&relation.bindings, work)?);
                }
                membership.push(cells.into_boxed_slice());
            }
            work.charge(DecisionWork::Node)?;
            result.functions.insert(
                id,
                ImportedFunctionTable {
                    relation,
                    membership: membership.into_boxed_slice(),
                },
            );
        }
    }
    for predicate in graph.predicates.values() {
        work.charge(DecisionWork::Visit)?;
        if let PredicateTerm::ImportedPrincipal(imported) = predicate.as_ref() {
            attach(graph, &mut result, imported, cache, work)?;
        }
    }
    Ok(result)
}

impl ImportedRelationTable {
    /// Instantiate the already-computed conditional result, then independently
    /// replay its original program. This operation never minimizes again.
    pub(super) fn specialize<C: DecisionControl>(
        &self,
        imported: &ImportedEffectRelation,
        parent: &PrincipalEffectClosure,
        work: &mut Work<'_, C>,
    ) -> Result<Option<PrincipalEffectClosure>, PreparedEffectFailure<C::Error>> {
        let guard = self
            .principal(parent.layout.view(), work)?
            .substitute(&parent.coefficients, work)?;
        if guard.is_constant(false) {
            return Ok(None);
        }
        if !guard.is_constant(true) {
            return Err(PreparedEffectInvariant::UnclosedCell.into());
        }
        let graph = &imported.completion.graph;
        let mut fixed = BTreeMap::new();
        for (&future, row) in &imported.completion.fixed {
            let formula = work.copy_formula(row.known()?)?;
            work.charge(DecisionWork::Node)?;
            fixed.insert(
                future,
                EffectRow {
                    formula: Some(formula),
                },
            );
        }
        let mut coefficients = BTreeMap::new();
        for (&future, actual) in &imported.inputs {
            let actual = actual.clone();
            let actual_definition = parent.graph.function(&actual)?;
            let parameters = graph.functions[&future].parameters();
            let row = parent.functions[&actual.reference().ordinal].known()?;
            let formula = row.map_references(work, &mut |reference, _| {
                let index = actual_definition
                    .parameters()
                    .iter()
                    .position(|parameter| parameter == reference)
                    .ok_or_else(|| {
                        PreparedEffectInvariant::UndeclaredParameter(reference.clone())
                    })?;
                Ok(parameters[index].clone())
            })?;
            for class in 0..self.child.layout.view().classes() {
                for mask in 0..self.child.layout.tables[&future] {
                    let value = self.child.layout.view().at(formula.membership(), class).evaluate_with_control(|reference| {
                        let index = parameters.iter().position(|parameter| parameter == reference)
                            .expect("specialized Future references come from its declared parameters");
                        mask & (1usize << index) != 0
                    }, work)?;
                    work.charge(DecisionWork::Node)?;
                    coefficients.insert(
                        Atom::Cell {
                            scheme: future,
                            class,
                            mask,
                        },
                        Decision::constant(value),
                    );
                }
            }
            work.charge(DecisionWork::Node)?;
            fixed.insert(
                future,
                EffectRow {
                    formula: Some(formula),
                },
            );
        }
        let mut completed = BTreeMap::new();
        for (cell, value) in &self.child.coefficients {
            let value = value.substitute(&coefficients, work)?;
            if !value.is_constant(true) && !value.is_constant(false) {
                return Err(PreparedEffectInvariant::UnclosedCell.into());
            }
            work.charge(DecisionWork::Node)?;
            completed.insert(cell.clone(), value);
        }
        let witnesses = self.child.witnesses.substitute(&coefficients, work)?;
        for (cell, value) in coefficients {
            work.charge(DecisionWork::Node)?;
            completed.insert(cell, value);
        }
        let mut choices = BTreeSet::new();
        for choice in &self.child.choices {
            work.charge(DecisionWork::Node)?;
            choices.insert(choice.clone());
        }
        Ok(Some(PrincipalEffectClosure::reify(
            Arc::clone(graph),
            Arc::clone(&self.child.layout),
            completed,
            choices,
            witnesses,
            fixed,
            work,
        )?))
    }

    fn transport<C: DecisionControl>(
        &self,
        decision: &Decision,
        receiving: ClassLayout<'_>,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        decision.map_references(work, &mut |atom, _| match atom {
            Atom::Cell {
                scheme,
                class,
                mask,
            } => {
                let class = if *class == 0 {
                    0
                } else {
                    receiving
                        .labels
                        .binary_search(&self.child.layout.labels[*class - 1])
                        .map(|index| index + 1)
                        .map_err(|_| PreparedEffectInvariant::UnclosedCell)?
                };
                Ok(Atom::Cell {
                    scheme: *scheme,
                    class,
                    mask: *mask,
                })
            }
            _ => Err(PreparedEffectInvariant::UnclosedCell.into()),
        })
    }

    pub(super) fn principal<C: DecisionControl>(
        &self,
        receiving: ClassLayout<'_>,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        self.transport(&self.principal, receiving, work)
    }
}

impl ImportedFunctionTable {
    pub(super) fn coefficient<C: DecisionControl>(
        &self,
        receiving: ClassLayout<'_>,
        class: usize,
        mask: usize,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        let source = if class == 0 {
            0
        } else {
            self.relation
                .child
                .layout
                .labels
                .binary_search(&receiving.labels[class - 1])
                .map_or(0, |index| index + 1)
        };
        self.relation
            .transport(&self.membership[source][mask], receiving, work)
    }

    pub(super) fn principal<C: DecisionControl>(
        &self,
        receiving: ClassLayout<'_>,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        self.relation.principal(receiving, work)
    }
}
