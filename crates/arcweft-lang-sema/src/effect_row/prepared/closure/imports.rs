//! One conditional minimization per immutable relation and receiving label basis.

use super::*;
use crate::effect_row::prepared::completion::ImportedEffectRelation;

#[cfg(test)]
mod tests;

pub(super) struct ImportedFunctionTable {
    pub(super) relation: Arc<ImportedRelationTable>,
    membership: ImportedMembership,
}

enum ImportedMembership {
    Child(u64),
    Rebound(Box<[Box<[Decision]>]>),
}

pub(super) struct ImportedRelationTable {
    pub(super) child: Arc<ComponentCompletion>,
    // Absent when all receiving input cells keep the child's coordinates.
    rebound_principal: Option<Decision>,
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
                pending.push((Arc::clone(imported.relation.completion()), false));
            }
        }
    }
    for predicate in graph.predicates.values() {
        work.charge(DecisionWork::Visit)?;
        if let PredicateTerm::ImportedPrincipal(imported) = predicate.as_ref() {
            work.charge(DecisionWork::Node)?;
            pending.push((Arc::clone(imported.completion()), false));
        }
    }
    Ok(())
}

/// Actual input support is admitted by the root completion boundary. Every
/// nested relation keeps its own quantifiers, uniform choices and late inputs.
pub(super) fn lower_selected_imports<C: DecisionControl>(
    graph: &PreparedEffectDefinitions,
    labels: &Arc<[EffectId]>,
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
        // Every table reached by this lowering shares the root basis.
        // Observation labels are projected into this basis by the caller.
        let mut layout = Layout::with_shared_labels(&source.graph, Arc::clone(labels), work)?;
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

impl ImportedEffectRelation {
    fn bind_cell(&self, cell: &Atom) -> Result<Atom, PreparedEffectInvariant> {
        let &Atom::Cell {
            scheme,
            class,
            mask,
        } = cell
        else {
            return Err(PreparedEffectInvariant::UnclosedCell);
        };
        let input = self
            .inputs()
            .get(&scheme)
            .ok_or(PreparedEffectInvariant::UnclosedCell)?;
        Ok(Atom::Cell {
            scheme: input.reference().ordinal,
            class,
            mask,
        })
    }

    fn bind_decision<C: DecisionControl>(
        &self,
        decision: &Decision,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        decision.map_references(work, &mut |cell, _| {
            self.bind_cell(cell).map_err(Into::into)
        })
    }
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
        cached(cache, imported.completion(), work)?.ok_or(PreparedEffectInvariant::UnclosedCell)?;
    let mut same_coordinates = true;
    for (&original, actual) in imported.inputs() {
        work.charge(DecisionWork::Visit)?;
        graph.function(actual)?;
        same_coordinates &= actual.reference().ordinal == original;
    }
    // The completed child can reference only unresolved inputs. Its binding
    // covers those inputs, so matching ordinals preserve all child cells.
    let rebound_principal = (!same_coordinates)
        .then(|| imported.bind_decision(&child.principal, work))
        .transpose()?;
    work.charge(DecisionWork::Node)?;
    let table = Arc::new(ImportedRelationTable {
        child: Arc::clone(child),
        rebound_principal,
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
            let is_input = imported.definition().is_input();
            let masks = *relation
                .child
                .layout
                .tables
                .get(&selected)
                .ok_or(PreparedEffectInvariant::UnclosedCell)?;
            let membership = if relation.rebound_principal.is_none() && !is_input {
                // The selected reference names a declaration in the full
                // child layout. Every cell of a local declaration belongs to
                // the completed tuple, including implicit false coefficients.
                ImportedMembership::Child(selected)
            } else {
                let mut membership = Vec::new();
                for class in 0..relation.child.layout.view().classes() {
                    let mut cells = Vec::new();
                    for mask in 0..masks {
                        let cell = Atom::Cell {
                            scheme: selected,
                            class,
                            mask,
                        };
                        let value = if let Some(value) = relation.child.coefficients.get(&cell) {
                            imported.relation.bind_decision(value, work)?
                        } else if is_input {
                            Decision::variable(imported.relation.bind_cell(&cell)?, work)?
                        } else {
                            return Err(PreparedEffectInvariant::UnclosedCell.into());
                        };
                        work.charge(DecisionWork::Node)?;
                        cells.push(value);
                    }
                    membership.push(cells.into_boxed_slice());
                }
                ImportedMembership::Rebound(membership.into_boxed_slice())
            };
            work.charge(DecisionWork::Node)?;
            result.functions.insert(
                id,
                ImportedFunctionTable {
                    relation,
                    membership,
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
        if !parent.coefficients.evaluate(self.principal(), work)? {
            return Ok(None);
        }
        let completion = imported.completion();
        let graph = &completion.graph;
        let mut fixed = BTreeMap::new();
        for (&future, row) in &completion.fixed {
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
        for (&future, actual) in imported.inputs() {
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
                        value,
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
        let witnesses = if let Some(witnesses) = &self.child.choice_witnesses {
            witnesses.specialize(&coefficients, work)?.close(work)?
        } else {
            // The child uses its own coefficient basis. Confirm the guard
            // against the actual rows transported into that basis.
            if !self.child.principal.try_evaluate_with_control(
                |cell| {
                    coefficients
                        .get(cell)
                        .copied()
                        .ok_or_else(|| PreparedEffectInvariant::UnclosedCell.into())
                },
                work,
            )? {
                return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
            }
            ClosedEffectWitnesses::unique()
        };
        let completed = self.child.coefficients.specialize(coefficients, work)?;
        Ok(Some(PrincipalEffectClosure::reify(
            Arc::clone(graph),
            Arc::clone(&self.child.layout),
            completed,
            witnesses,
            fixed,
            work,
        )?))
    }

    pub(super) fn principal(&self) -> &Decision {
        self.rebound_principal
            .as_ref()
            .unwrap_or(&self.child.principal)
    }
}

impl ImportedFunctionTable {
    pub(super) fn coefficient(
        &self,
        class: usize,
        mask: usize,
    ) -> Result<&Decision, PreparedEffectInvariant> {
        match &self.membership {
            ImportedMembership::Child(scheme) => {
                self.relation.child.coefficients.get(&Atom::Cell {
                    scheme: *scheme,
                    class,
                    mask,
                })
            }
            ImportedMembership::Rebound(membership) => membership
                .get(class)
                .and_then(|coefficients| coefficients.get(mask)),
        }
        .ok_or(PreparedEffectInvariant::UnclosedCell)
    }
}
