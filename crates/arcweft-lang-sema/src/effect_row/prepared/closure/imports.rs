//! One conditional minimization per immutable child and receiving label basis.

use super::*;
use crate::effect_row::prepared::completion::ImportedEffectFunction;

#[cfg(test)]
mod tests;

pub(super) struct ImportedFunctionTable {
    pub(super) child: Arc<ComponentCompletion>,
    membership: Box<[Box<[Decision]>]>,
    principal: Decision,
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

/// The root's public completion boundary has already admitted its full support
/// (including every actual Future row). Child programs retain unbound Futures;
/// only their lowering into this proven finite basis uses coefficient tables.
pub(super) fn lower_imports<C: DecisionControl>(
    graph: &PreparedEffectGraph,
    labels: &[EffectId],
    work: &mut Work<'_, C>,
) -> Result<BTreeMap<u64, ImportedFunctionTable>, PreparedEffectFailure<C::Error>> {
    lower_selected_imports(graph, labels, FunctionInventory::All, work)
}

pub(super) fn lower_selected_imports<C: DecisionControl>(
    graph: &PreparedEffectGraph,
    labels: &[EffectId],
    inventory: FunctionInventory<'_>,
    work: &mut Work<'_, C>,
) -> Result<BTreeMap<u64, ImportedFunctionTable>, PreparedEffectFailure<C::Error>> {
    let mut cache = Vec::new();
    let mut todo = Vec::new();
    for (_, function) in graph
        .functions
        .iter()
        .filter(|(id, _)| inventory.contains(**id))
    {
        if let Function::Imported(imported) = function.as_ref() {
            work.charge(DecisionWork::Node)?;
            todo.push((Arc::clone(&imported.completion), false));
        }
    }
    while let Some((source, ready)) = todo.pop() {
        work.charge(DecisionWork::Visit)?;
        if cached(&cache, &source, work)?.is_some() {
            continue;
        }
        if !ready {
            work.charge(DecisionWork::Node)?;
            todo.push((Arc::clone(&source), true));
            for function in source.graph.functions.values() {
                if let Function::Imported(imported) = function.as_ref() {
                    work.charge(DecisionWork::Node)?;
                    todo.push((Arc::clone(&imported.completion), false));
                }
            }
            continue;
        }
        let mut basis = BTreeSet::new();
        for label in labels {
            work.charge(DecisionWork::Node)?;
            basis.insert(label.clone());
        }
        let mut layout = Layout::with_labels(&source.graph, basis, work)?;
        layout.imports = assemble(&source.graph, &cache, work)?;
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

fn assemble<C: DecisionControl>(
    graph: &PreparedEffectGraph,
    cache: &Cache,
    work: &mut Work<'_, C>,
) -> Result<BTreeMap<u64, ImportedFunctionTable>, PreparedEffectFailure<C::Error>> {
    assemble_selected(graph, cache, FunctionInventory::All, work)
}

fn assemble_selected<C: DecisionControl>(
    graph: &PreparedEffectGraph,
    cache: &Cache,
    inventory: FunctionInventory<'_>,
    work: &mut Work<'_, C>,
) -> Result<BTreeMap<u64, ImportedFunctionTable>, PreparedEffectFailure<C::Error>> {
    let mut result = BTreeMap::new();
    for (&id, function) in &graph.functions {
        if !inventory.contains(id) {
            continue;
        }
        if let Function::Imported(imported) = function.as_ref() {
            let child = cached(cache, &imported.completion, work)?
                .ok_or(PreparedEffectInvariant::UnclosedCell)?;
            let mut bindings = BTreeMap::new();
            for (&future, actual) in &imported.futures {
                let actual = FunctionRef::Unknown(actual.clone());
                graph.function(&actual)?;
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
            let selected = imported.selected.reference().ordinal;
            let mut membership = Vec::new();
            for class in 0..child.layout.view().classes() {
                let mut cells = Vec::new();
                for mask in 0..child.layout.tables[&selected] {
                    let cell = Atom::Cell {
                        scheme: selected,
                        class,
                        mask,
                    };
                    let value = if let Some(value) = child.coefficients.get(&cell) {
                        compilation::copy(value, work)?
                    } else if imported.definition().is_future() {
                        Decision::variable(cell, work)?
                    } else {
                        return Err(PreparedEffectInvariant::UnclosedCell.into());
                    };
                    work.charge(DecisionWork::Node)?;
                    cells.push(value.substitute(&bindings, work)?);
                }
                membership.push(cells.into_boxed_slice());
            }
            let principal = child.completion.principal().substitute(&bindings, work)?;
            work.charge(DecisionWork::Node)?;
            result.insert(
                id,
                ImportedFunctionTable {
                    child: Arc::clone(child),
                    membership: membership.into_boxed_slice(),
                    principal,
                },
            );
        }
    }
    Ok(result)
}

impl ImportedFunctionTable {
    /// Instantiate the already-computed conditional result, then independently
    /// replay its original program. This operation never minimizes again.
    pub(super) fn specialize<'g, C: DecisionControl>(
        &self,
        imported: &'g ImportedEffectFunction,
        parent: &PrincipalEffectClosure<'_>,
        work: &mut Work<'_, C>,
    ) -> Result<Option<PrincipalEffectClosure<'g>>, PreparedEffectFailure<C::Error>> {
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
            let formula = row
                .known()?
                .map_references(work, &mut |reference, _| Ok(reference.clone()))?;
            work.charge(DecisionWork::Node)?;
            fixed.insert(
                future,
                EffectRow {
                    formula: Some(formula),
                },
            );
        }
        let mut coefficients = BTreeMap::new();
        for (&future, actual) in &imported.futures {
            let actual = FunctionRef::Unknown(actual.clone());
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
            graph,
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
            self.child
                .layout
                .labels
                .binary_search(&receiving.labels[class - 1])
                .map_or(0, |index| index + 1)
        };
        self.transport(&self.membership[source][mask], receiving, work)
    }

    pub(super) fn principal<C: DecisionControl>(
        &self,
        receiving: ClassLayout<'_>,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        self.transport(&self.principal, receiving, work)
    }
}
