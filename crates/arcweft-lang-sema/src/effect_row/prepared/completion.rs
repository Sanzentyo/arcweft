//! Frozen scoped relations and atomic imports of guarded output functions.

use super::*;
use crate::effects::EffectId;

pub(super) mod residual;
use residual::EffectResidualProgram;

mod binding;
pub(crate) use binding::{BoundScopedEffectCompletion, PreparedEffectScopedBinding};
mod closing;

/// The immutable original relation, including dependency signatures, lexical
/// quantifiers, declaration domains, alternatives and Future input identities.
/// Freezing does not assert that its satisfiable or principal domain is total.
#[derive(Debug)]
pub(crate) struct ScopedEffectCompletion {
    pub(super) graph: Arc<PreparedEffectDefinitions>,
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

/// One exact attachment of an original scoped relation to this graph's actual
/// inputs. Functions and the scalar principal condition share this owner.
#[derive(Clone, Debug)]
pub(crate) struct PreparedEffectImportedRelation {
    pub(super) owner: PreparedEffectGraphOwner,
    pub(super) relation: Arc<ImportedEffectRelation>,
}

#[derive(Debug)]
pub(super) struct ImportedEffectRelation {
    pub(super) completion: Arc<ScopedEffectCompletion>,
    /// Each reference belongs to the receiving graph, not the frozen child.
    pub(super) inputs: BTreeMap<u64, FunctionRef>,
}

#[derive(Debug)]
pub(super) struct ImportedEffectFunction {
    pub(super) relation: Arc<ImportedEffectRelation>,
    pub(super) selected: FunctionRef,
}

impl ImportedEffectFunction {
    pub(super) fn definition(&self) -> &Function {
        &self.relation.completion.graph.functions[&self.selected.reference().ordinal]
    }
}

impl PreparedEffectGraph {
    pub(crate) fn complete_scoped<C: DecisionControl>(
        self,
        control: &mut C,
    ) -> Result<Arc<ScopedEffectCompletion>, PreparedEffectFailure<C::Error>> {
        if self.active_scope.is_some() {
            return Err(PreparedEffectInvariant::ReadScopeActive.into());
        }
        if !self.checkpoints.is_empty() {
            return Err(PreparedEffectInvariant::CheckpointOrder.into());
        }
        self.relation.complete_scoped(control)
    }
}

impl PreparedEffectDefinitions {
    pub(in crate::effect_row::prepared) fn complete_scoped<C: DecisionControl>(
        self,
        control: &mut C,
    ) -> Result<Arc<ScopedEffectCompletion>, PreparedEffectFailure<C::Error>> {
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
}

impl PreparedEffectGraph {
    /// Bind a child's Future inputs to actual total inference functions in this
    /// graph. Their ordered dependency signatures must agree. The imported
    /// Named inputs reattach only to their original issuer and immutable header.
    pub(crate) fn import_guarded_function<C: DecisionControl>(
        &mut self,
        function: &GuardedEffectFunction,
        bindings: &[(&PreparedEffectUnknownRef, &PreparedEffectUnknownRef)],
        control: &mut C,
    ) -> Result<PreparedEffectImportedRef, PreparedEffectFailure<C::Error>> {
        let relation = self.import_scoped_relation(&function.completion, bindings, control)?;
        self.import_relation_function(&relation, function, control)
    }

    pub(crate) fn import_scoped_relation<C: DecisionControl>(
        &mut self,
        completion: &Arc<ScopedEffectCompletion>,
        bindings: &[(&PreparedEffectUnknownRef, &PreparedEffectUnknownRef)],
        control: &mut C,
    ) -> Result<PreparedEffectImportedRelation, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        let mut inputs = BTreeMap::new();
        for (future, actual) in bindings {
            work.charge(DecisionWork::Visit)?;
            let future = FunctionRef::Unknown((*future).clone());
            let declaration = completion.graph.function(&future)?;
            if !declaration.is_future() {
                return Err(PreparedEffectInvariant::FutureBinding.into());
            }
            if completion.fixed.contains_key(&future.reference().ordinal) {
                return Err(PreparedEffectInvariant::DuplicateFutureBinding.into());
            }
            let actual_ref = FunctionRef::Unknown((*actual).clone());
            if declaration.parameters().len() != self.function(&actual_ref)?.parameters().len() {
                return Err(PreparedEffectInvariant::ApplicationArity.into());
            }
            work.charge(DecisionWork::Node)?;
            if inputs
                .insert(future.reference().ordinal, actual_ref)
                .is_some()
            {
                return Err(PreparedEffectInvariant::DuplicateFutureBinding.into());
            }
        }
        for (&id, declaration) in &completion.graph.functions {
            work.charge(DecisionWork::Visit)?;
            if let Function::SchemeInput(declaration) = declaration.as_ref() {
                declaration.validate(self)?;
                work.charge(DecisionWork::Node)?;
                inputs.insert(id, FunctionRef::Scheme(declaration.scheme().clone()));
            }
            if declaration.is_future()
                && !completion.fixed.contains_key(&id)
                && !inputs.contains_key(&id)
            {
                return Err(PreparedEffectInvariant::UnresolvedFuture.into());
            }
        }
        work.charge(DecisionWork::Node)?;
        Ok(PreparedEffectImportedRelation {
            owner: PreparedEffectGraphOwner(Arc::clone(&self.relation.issuer)),
            relation: Arc::new(ImportedEffectRelation {
                completion: Arc::clone(completion),
                inputs,
            }),
        })
    }

    pub(crate) fn import_relation_function<C: DecisionControl>(
        &mut self,
        relation: &PreparedEffectImportedRelation,
        function: &GuardedEffectFunction,
        control: &mut C,
    ) -> Result<PreparedEffectImportedRef, PreparedEffectFailure<C::Error>> {
        self.read().validate_owner(&relation.owner)?;
        relation.relation.validate(self.read(), control)?;
        if !Arc::ptr_eq(&relation.relation.completion, &function.completion) {
            return Err(PreparedEffectInvariant::ImportedRelationOrigin.into());
        }
        function.completion.graph.function(&function.selected)?;
        let id = self.insert_function(
            Function::Imported(ImportedEffectFunction {
                relation: Arc::clone(&relation.relation),
                selected: function.selected.clone(),
            }),
            &mut Work(control),
        )?;
        Ok(PreparedEffectImportedRef(self.reference(id)))
    }

    /// Keep the principal region itself, including a relation with no output
    /// functions. Its inputs are whole functions, never forged row parameters.
    pub(crate) fn imported_principal<C: DecisionControl>(
        &mut self,
        relation: &PreparedEffectImportedRelation,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        self.read().validate_owner(&relation.owner)?;
        relation.relation.validate(self.read(), control)?;
        self.insert_predicate(
            PredicateTerm::ImportedPrincipal(Arc::clone(&relation.relation)),
            control,
        )
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

impl ImportedEffectRelation {
    pub(super) fn validate<C: DecisionControl>(
        &self,
        receiving: PreparedEffectReadView<'_>,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        for actual in self.inputs.values() {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            receiving.function(actual)?;
        }
        Ok(())
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
            let formula = work.copy_formula(row.known()?)?;
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
            let formula = work.copy_formula(row.known()?)?;
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
