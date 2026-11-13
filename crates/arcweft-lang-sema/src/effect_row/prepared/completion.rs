//! Frozen scoped relations and atomic imports of guarded output functions.

use super::*;
use crate::effects::EffectId;

/// The immutable original relation, including dependency signatures, lexical
/// quantifiers, declaration domains, alternatives and Future input identities.
/// Freezing does not assert that its satisfiable or principal domain is total.
#[derive(Debug)]
pub(crate) struct ScopedEffectCompletion {
    pub(super) graph: PreparedEffectGraph,
    pub(super) support: BTreeSet<EffectId>,
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
        work.charge(DecisionWork::Node)?;
        Ok(Arc::new(ScopedEffectCompletion {
            graph: self,
            support,
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
            if declaration.is_future() && !futures.contains_key(&id) {
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
