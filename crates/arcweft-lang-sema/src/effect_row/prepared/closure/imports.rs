//! One conditional minimization per immutable relation and receiving label basis.

use super::*;
use crate::effect_row::prepared::completion::residual::EffectResidualProgram;
use crate::effect_row::prepared::completion::{ImportedEffectFunction, ImportedEffectRelation};
use arcweft_core::effect_row::EffectDecisionReplacement;
use component::CompiledObligations;
use std::collections::btree_map;

mod clauses;
pub(super) use clauses::InheritedObligations;
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
    source: Arc<ImportedEffectRelation>,
    pub(super) child: Arc<ComponentCompletion>,
    // Absent when all receiving input cells keep the child's coordinates
    // and remain symbolic at this attachment.
    rebound: Option<ReboundImport>,
}

struct ReboundImport {
    principal: Decision,
    substitutes_fixed: bool,
}

#[derive(Default)]
pub(super) struct ImportedRelations {
    pub(super) functions: BTreeMap<u64, ImportedFunctionTable>,
    relations: Vec<Arc<ImportedRelationTable>>,
}

impl ImportedRelations {
    pub(super) fn relation<C: DecisionControl>(
        &self,
        source: &Arc<ImportedEffectRelation>,
        work: &mut Work<'_, C>,
    ) -> Result<&ImportedRelationTable, PreparedEffectFailure<C::Error>> {
        for table in &self.relations {
            work.charge(DecisionWork::Visit)?;
            if Arc::ptr_eq(&table.source, source) {
                return Ok(table);
            }
        }
        Err(PreparedEffectInvariant::UnclosedCell.into())
    }

    pub(super) fn attachments(&self) -> impl Iterator<Item = &ImportedRelationTable> {
        self.relations.iter().map(Arc::as_ref)
    }
}

struct CachedCompletion {
    source: Arc<ScopedEffectCompletion>,
    completion: Arc<ComponentCompletion>,
    obligations: Vec<Arc<Decision>>,
}

struct LoweredImports {
    layout: Layout,
    fixed: FixedCells,
    cache: Vec<CachedCompletion>,
}

fn cached<'a, C: DecisionControl>(
    cache: &'a [CachedCompletion],
    source: &Arc<ScopedEffectCompletion>,
    work: &mut Work<'_, C>,
) -> Result<Option<&'a Arc<ComponentCompletion>>, PreparedEffectFailure<C::Error>> {
    for entry in cache {
        work.charge(DecisionWork::Visit)?;
        if Arc::ptr_eq(&entry.source, source) {
            return Ok(Some(&entry.completion));
        }
    }
    Ok(None)
}

enum ImportSite<'a> {
    Function(u64, &'a ImportedEffectFunction),
    Principal(&'a Arc<ImportedEffectRelation>),
}

impl<'a> ImportSite<'a> {
    fn relation(&self) -> &'a Arc<ImportedEffectRelation> {
        match self {
            Self::Function(_, function) => &function.relation,
            Self::Principal(relation) => relation,
        }
    }
}

/// Resume immediately after the import that needed a child completion. Each
/// graph's declarations and predicates are inspected once in ordinal order.
struct ImportScope<'a> {
    graph: &'a PreparedEffectDefinitions,
    inventory: FunctionInventory<'a>,
    functions: btree_map::Iter<'a, u64, Arc<Function>>,
    predicates: btree_map::Values<'a, u64, Arc<PredicateTerm>>,
    layout: Layout,
    fixed: FixedCells,
}

impl<'a> ImportScope<'a> {
    fn new(
        graph: &'a PreparedEffectDefinitions,
        layout: Layout,
        fixed: FixedCells,
        inventory: FunctionInventory<'a>,
    ) -> Self {
        Self {
            graph,
            inventory,
            functions: graph.functions.iter(),
            predicates: graph.predicates.values(),
            layout,
            fixed,
        }
    }

    fn next<C: DecisionControl>(
        &mut self,
        work: &mut Work<'_, C>,
    ) -> Result<Option<ImportSite<'a>>, PreparedEffectFailure<C::Error>> {
        for (&id, function) in self.functions.by_ref() {
            work.charge(DecisionWork::Visit)?;
            if self.inventory.contains(id)
                && let Function::Imported(imported) = function.as_ref()
            {
                return Ok(Some(ImportSite::Function(id, imported)));
            }
        }
        for predicate in self.predicates.by_ref() {
            work.charge(DecisionWork::Visit)?;
            if let PredicateTerm::ImportedPrincipal(imported) = predicate.as_ref() {
                return Ok(Some(ImportSite::Principal(imported)));
            }
        }
        Ok(None)
    }

    fn install<C: DecisionControl>(
        &mut self,
        site: ImportSite<'a>,
        child: &Arc<ComponentCompletion>,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        let tables = &mut self.layout.imports;
        let relation = tables.attach(self.graph, site.relation(), child, &self.fixed, work)?;
        if let ImportSite::Function(id, imported) = site {
            let table = ImportedFunctionTable::new(imported, relation, &self.fixed, work)?;
            work.charge(DecisionWork::Node)?;
            tables.functions.insert(id, table);
        }
        Ok(())
    }
}

/// Actual input support is admitted by the root completion boundary. Every
/// nested relation keeps its own quantifiers, uniform choices and late inputs.
fn lower_selected_imports<'a, C: DecisionControl>(
    graph: &'a PreparedEffectDefinitions,
    layout: Layout,
    fixed: FixedCells,
    inventory: FunctionInventory<'a>,
    work: &mut Work<'_, C>,
) -> Result<LoweredImports, PreparedEffectFailure<C::Error>> {
    let mut cache = Vec::new();
    let mut pending = Vec::new();
    let labels = Arc::clone(&layout.labels);
    let mut scope = ImportScope::new(graph, layout, fixed, inventory);
    loop {
        if let Some(site) = scope.next(work)? {
            let source = site.relation().completion();
            if let Some(child) = cached(&cache, source, work)? {
                scope.install(site, child, work)?;
            } else {
                // Every child uses the receiving label basis. Admit its
                // shape before evaluating its own fixed input cells.
                let layout = Layout::with_shared_labels(&source.graph, Arc::clone(&labels), work)?;
                let fixed = FixedCells::new(&source.graph, &source.fixed, layout.view(), work)?
                    .admit_finiteness(&source.graph, &source.program, work)?;
                work.charge(DecisionWork::Node)?;
                pending.push((scope, site));
                scope = ImportScope::new(&source.graph, layout, fixed, FunctionInventory::All);
            }
            continue;
        }
        let Some((mut parent, site)) = pending.pop() else {
            return Ok(LoweredImports {
                layout: scope.layout,
                fixed: scope.fixed,
                cache,
            });
        };
        let source = site.relation().completion();
        let inherited =
            clauses::inherited_from(&cache, &source.graph, &scope.layout, &scope.fixed, work)?;
        let mut obligations = Vec::new();
        let component = CompiledComponent::compile(
            &source.graph,
            &source.program,
            scope.fixed,
            scope.layout,
            CompiledObligations {
                inherited,
                retain: Some(&mut obligations),
            },
            Some(&source.support),
            work,
        )?;
        let completed = component.complete(work)?;
        work.charge(DecisionWork::Node)?;
        let completed = Arc::new(completed);
        cache.push(CachedCompletion {
            source: Arc::clone(source),
            completion: Arc::clone(&completed),
            obligations,
        });
        parent.install(site, &completed, work)?;
        scope = parent;
    }
}

pub(super) fn compile_with_imports<C: DecisionControl>(
    graph: &PreparedEffectDefinitions,
    layout: Layout,
    fixed: FixedCells,
    inventory: FunctionInventory<'_>,
    work: &mut Work<'_, C>,
) -> Result<CompiledComponent, PreparedEffectFailure<C::Error>> {
    let compiled;
    let program = match inventory {
        FunctionInventory::Frozen(program) => program,
        _ => {
            compiled = EffectResidualProgram::with_inventory(graph, inventory, work)?;
            &compiled
        }
    };
    let fixed = fixed.admit_finiteness(graph, program, work)?;
    let LoweredImports {
        layout,
        fixed,
        cache,
    } = lower_selected_imports(graph, layout, fixed, inventory, work)?;
    let inherited = clauses::inherited_from(&cache, graph, &layout, &fixed, work)?;
    CompiledComponent::compile(
        graph,
        program,
        fixed,
        layout,
        CompiledObligations {
            inherited,
            retain: None,
        },
        None,
        work,
    )
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
        fixed: Option<&FixedCells>,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        if let Some(fixed) = fixed {
            return decision
                .substitute_references(work, |cell, _| Ok(fixed.operand(self.bind_cell(cell)?)));
        }
        decision.map_references(work, &mut |cell, _| {
            self.bind_cell(cell).map_err(Into::into)
        })
    }
}

impl ImportedRelations {
    fn attach<C: DecisionControl>(
        &mut self,
        graph: &PreparedEffectDefinitions,
        imported: &Arc<ImportedEffectRelation>,
        child: &Arc<ComponentCompletion>,
        fixed: &FixedCells,
        work: &mut Work<'_, C>,
    ) -> Result<Arc<ImportedRelationTable>, PreparedEffectFailure<C::Error>> {
        for table in &self.relations {
            work.charge(DecisionWork::Visit)?;
            if Arc::ptr_eq(&table.source, imported) {
                return Ok(Arc::clone(table));
            }
        }
        let mut same_coordinates = true;
        let mut substitutes_fixed = false;
        for (&original, actual) in imported.inputs() {
            work.charge(DecisionWork::Visit)?;
            graph.function(actual)?;
            same_coordinates &= actual.reference().ordinal == original;
            substitutes_fixed |= fixed.binds_coefficient(actual.reference().ordinal);
        }
        // The completed child can reference only unresolved inputs. Its binding
        // covers those inputs, so matching ordinals preserve all child cells.
        let rebound = if !same_coordinates || substitutes_fixed {
            let principal = imported.bind_decision(
                &child.principal,
                substitutes_fixed.then_some(fixed),
                work,
            )?;
            Some(ReboundImport {
                principal,
                substitutes_fixed,
            })
        } else {
            None
        };
        work.charge(DecisionWork::Node)?;
        let table = Arc::new(ImportedRelationTable {
            source: Arc::clone(imported),
            child: Arc::clone(child),
            rebound,
        });
        work.charge(DecisionWork::Node)?;
        self.relations.push(Arc::clone(&table));
        Ok(table)
    }
}

impl ImportedFunctionTable {
    fn new<C: DecisionControl>(
        imported: &ImportedEffectFunction,
        relation: Arc<ImportedRelationTable>,
        fixed: &FixedCells,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        let selected = imported.selected.reference().ordinal;
        let is_input = imported.definition().is_input();
        let masks = *relation
            .child
            .layout
            .tables
            .get(&selected)
            .ok_or(PreparedEffectInvariant::UnclosedCell)?;
        let membership = if relation.rebound.is_none() && !is_input {
            // The selected reference names a declaration in the full
            // child layout. Every cell of a local declaration belongs to
            // the completed tuple, including implicit false coefficients.
            ImportedMembership::Child(selected)
        } else {
            let substituted = relation
                .rebound
                .as_ref()
                .filter(|binding| binding.substitutes_fixed)
                .map(|_| fixed);
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
                        relation.source.bind_decision(value, substituted, work)?
                    } else if is_input {
                        let actual = relation.source.bind_cell(&cell)?;
                        if let Some(value) = fixed.value(&actual) {
                            Decision::constant(value)
                        } else {
                            Decision::variable(actual, work)?
                        }
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
        Ok(Self {
            relation,
            membership,
        })
    }
}

impl ImportedRelationTable {
    /// Instantiate the already-computed conditional result, then independently
    /// replay its original program. This operation never minimizes again.
    pub(super) fn specialize<C: DecisionControl>(
        &self,
        parent: &PrincipalEffectClosure,
        work: &mut Work<'_, C>,
    ) -> Result<Option<PrincipalEffectClosure>, PreparedEffectFailure<C::Error>> {
        if !parent.coefficients.evaluate(self.principal(), work)? {
            return Ok(None);
        }
        let completion = self.source.completion();
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
        let child_layout = self.child.layout.view();
        let parent_layout = parent.layout.view();
        for (&future, actual) in self.source.inputs() {
            let actual_definition = parent.graph.function(actual)?;
            let actual_id = actual.reference().ordinal;
            let actual_table = parent_layout.function_table(actual_id, actual_definition)?;
            let future_definition = &graph.functions[&future];
            let future_table = child_layout.function_table(future, future_definition)?;
            if actual_table.entries != future_table.entries {
                return Err(PreparedEffectInvariant::ApplicationArity.into());
            }
            let parameters = future_definition.parameters();
            let row = parent.functions[&actual_id].known()?;
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
            // The parent already confirmed its row against these cells.
            // Transfer positional coefficients and retain the transported
            // formula for the child's independent replay.
            for class in 0..child_layout.classes() {
                let parent_class = child_layout
                    .label_classes()
                    .class_in(class, parent_layout.label_classes());
                for mask in 0..future_table.entries {
                    work.charge(DecisionWork::Node)?;
                    let value = actual_table.closed_value(
                        parent_class,
                        mask,
                        &parent.coefficients,
                        work,
                    )?;
                    coefficients.insert(future_table.cell(class, mask), value);
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
        self.rebound
            .as_ref()
            .map_or(&self.child.principal, |rebound| &rebound.principal)
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
