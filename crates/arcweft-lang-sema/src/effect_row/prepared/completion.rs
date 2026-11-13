//! Frozen scoped relations and atomic imports of guarded output functions.

use super::*;
use crate::effects::EffectId;

pub(super) mod residual;
use residual::EffectResidualProgram;

/// The immutable original relation, including dependency signatures, lexical
/// quantifiers, declaration domains, alternatives and Future input identities.
/// Freezing does not assert that its satisfiable or principal domain is total.
#[derive(Debug)]
pub(crate) struct ScopedEffectCompletion {
    pub(super) graph: Arc<PreparedEffectGraph>,
    pub(super) support: BTreeSet<EffectId>,
    pub(super) fixed: BTreeMap<u64, EffectRow>,
    pub(super) program: Arc<EffectResidualProgram>,
}

#[derive(Clone, Debug)]
pub(crate) struct GuardedEffectFunction {
    completion: Arc<ScopedEffectCompletion>,
    selected: FunctionRef,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PreparedEffectImportedRef(pub(super) PreparedEffectReference);

#[derive(Debug)]
pub(super) struct ImportedEffectFunction {
    pub(super) completion: Arc<ScopedEffectCompletion>,
    pub(super) selected: FunctionRef,
    /// Each reference belongs to the receiving graph, not the frozen child.
    pub(super) futures: BTreeMap<u64, PreparedEffectUnknownRef>,
}

impl ImportedEffectFunction {
    pub(super) fn definition(&self) -> &Function {
        &self.completion.graph.functions[&self.selected.reference().ordinal]
    }
}

impl PreparedEffectGraph {
    pub(crate) fn complete_scoped<C: DecisionControl>(
        self,
        control: &mut C,
    ) -> Result<Arc<ScopedEffectCompletion>, PreparedEffectFailure<C::Error>> {
        if !self.checkpoints.is_empty() {
            return Err(PreparedEffectInvariant::CheckpointOrder.into());
        }
        let mut work = Work(control);
        for function in self.functions.values() {
            work.charge(DecisionWork::Visit)?;
            if matches!(
                function.as_ref(),
                Function::Scheme(Scheme {
                    state: SchemeState::Declared,
                    ..
                })
            ) {
                return Err(PreparedEffectInvariant::SchemeState.into());
            }
        }
        let support = self.label_support(&mut work)?;
        let program = EffectResidualProgram::new(&self, &mut work)?;
        work.charge(DecisionWork::Node)?;
        work.charge(DecisionWork::Node)?;
        work.charge(DecisionWork::Node)?;
        Ok(Arc::new(ScopedEffectCompletion {
            graph: Arc::new(self),
            support,
            fixed: BTreeMap::new(),
            program: Arc::new(program),
        }))
    }

    /// Bind a child's Future inputs to actual total inference functions in this
    /// graph. Their ordered dependency signatures must agree. The imported
    /// function remains guarded; it is not another minimizable unknown.
    pub(crate) fn import_guarded_function<C: DecisionControl>(
        &mut self,
        function: &GuardedEffectFunction,
        bindings: &[(&PreparedEffectUnknownRef, &PreparedEffectUnknownRef)],
        control: &mut C,
    ) -> Result<PreparedEffectImportedRef, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        let mut futures = BTreeMap::new();
        for (future, actual) in bindings {
            work.charge(DecisionWork::Visit)?;
            let future = FunctionRef::Unknown((*future).clone());
            let declaration = function.completion.graph.function(&future)?;
            if !declaration.is_future() {
                return Err(PreparedEffectInvariant::FutureBinding.into());
            }
            if function
                .completion
                .fixed
                .contains_key(&future.reference().ordinal)
            {
                return Err(PreparedEffectInvariant::DuplicateFutureBinding.into());
            }
            let actual_ref = FunctionRef::Unknown((*actual).clone());
            if declaration.parameters().len() != self.function(&actual_ref)?.parameters().len() {
                return Err(PreparedEffectInvariant::ApplicationArity.into());
            }
            work.charge(DecisionWork::Node)?;
            if futures
                .insert(future.reference().ordinal, (*actual).clone())
                .is_some()
            {
                return Err(PreparedEffectInvariant::DuplicateFutureBinding.into());
            }
        }
        for (&id, declaration) in &function.completion.graph.functions {
            work.charge(DecisionWork::Visit)?;
            if declaration.is_future()
                && !function.completion.fixed.contains_key(&id)
                && !futures.contains_key(&id)
            {
                return Err(PreparedEffectInvariant::UnresolvedFuture.into());
            }
        }
        let id = self.insert_function(
            Function::Imported(ImportedEffectFunction {
                completion: Arc::clone(&function.completion),
                selected: function.selected.clone(),
                futures,
            }),
            &mut work,
        )?;
        Ok(PreparedEffectImportedRef(self.reference(id)))
    }

    pub(crate) fn imported_application<C: DecisionControl>(
        &mut self,
        function: &PreparedEffectImportedRef,
        arguments: Vec<PreparedEffectRow>,
        control: &mut C,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        self.apply_function(FunctionRef::Imported(function.clone()), arguments, control)
    }
}

impl ScopedEffectCompletion {
    /// Substitute only supplied Future inputs. Every other Future stays free,
    /// and the immutable original program is shared with the previous value.
    pub(crate) fn specialize_partial<C: DecisionControl>(
        self: &Arc<Self>,
        bindings: &[(&PreparedEffectUnknownRef, &EffectRow)],
        control: &mut C,
    ) -> Result<Arc<Self>, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        let mut fixed = BTreeMap::new();
        for (&id, row) in &self.fixed {
            let formula = row
                .known()?
                .map_references(&mut work, &mut |reference, _| Ok(reference.clone()))?;
            work.charge(DecisionWork::Node)?;
            fixed.insert(
                id,
                EffectRow {
                    formula: Some(formula),
                },
            );
        }
        let mut support = BTreeSet::new();
        for label in &self.support {
            work.charge(DecisionWork::Node)?;
            support.insert(label.clone());
        }
        for (reference, row) in bindings {
            work.charge(DecisionWork::Visit)?;
            let reference = FunctionRef::Unknown((*reference).clone());
            let definition = self.graph.function(&reference)?;
            if !definition.is_future() {
                return Err(PreparedEffectInvariant::FutureBinding.into());
            }
            let id = reference.reference().ordinal;
            if fixed.contains_key(&id) {
                return Err(PreparedEffectInvariant::DuplicateFutureBinding.into());
            }
            for reference in row.known()?.variables() {
                work.charge(DecisionWork::Visit)?;
                if !definition.parameters().contains(reference) {
                    return Err(
                        PreparedEffectInvariant::UndeclaredParameter(reference.clone()).into(),
                    );
                }
            }
            let formula = row
                .known()?
                .map_references(&mut work, &mut |reference, _| Ok(reference.clone()))?;
            for (label, _) in formula.membership().overrides() {
                work.charge(DecisionWork::Node)?;
                support.insert(label.clone());
            }
            work.charge(DecisionWork::Node)?;
            fixed.insert(
                id,
                EffectRow {
                    formula: Some(formula),
                },
            );
        }
        work.charge(DecisionWork::Node)?;
        Ok(Arc::new(Self {
            graph: Arc::clone(&self.graph),
            support,
            fixed,
            program: Arc::clone(&self.program),
        }))
    }

    pub(crate) fn unknown(
        self: &Arc<Self>,
        reference: &PreparedEffectUnknownRef,
    ) -> Result<GuardedEffectFunction, PreparedEffectInvariant> {
        let selected = FunctionRef::Unknown(reference.clone());
        self.graph.function(&selected)?;
        Ok(GuardedEffectFunction {
            completion: Arc::clone(self),
            selected,
        })
    }

    pub(crate) fn scheme(
        self: &Arc<Self>,
        reference: &PreparedEffectSchemeRef,
    ) -> Result<GuardedEffectFunction, PreparedEffectInvariant> {
        let selected = FunctionRef::Scheme(reference.clone());
        self.graph.function(&selected)?;
        Ok(GuardedEffectFunction {
            completion: Arc::clone(self),
            selected,
        })
    }
}
