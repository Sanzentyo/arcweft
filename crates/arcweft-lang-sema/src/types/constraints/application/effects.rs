//! A registered parameter applied to its actual introduction Γ.

use super::*;
use crate::{
    effect_row::{
        EffectRow,
        prepared::{
            PreparedEffectGraph, PreparedEffectInvariant, PreparedEffectReadView,
            PreparedEffectRow, PreparedEffectTermEdit,
        },
    },
    types::constraints::{
        OriginalProducerContext, TypeConstraintError,
        context::{TypeConstraintAccounting, TypeConstraintContext},
    },
};

use crate::{effect_row::EffectConstraintEnvironment, types::PreparedEffects};

/// The function's identity is issued by the graph. Its application is derived
/// only from the actual introduction Γ, before any candidate solution exists.
#[cfg_attr(test, derive(Clone))]
pub(in crate::types::constraints) struct ConstraintEffectFunction {
    function: PreparedEffectUnknownRef,
    application: PreparedEffectRow,
}

#[derive(Clone, Copy)]
pub(in crate::types::constraints) struct ConstraintEffectFunctionApplication<'a> {
    binding: &'a ConstraintEffectFunction,
    scope: &'a ConstraintLexicalScope,
}

impl ConstraintEffectFunction {
    pub(in crate::types::constraints) fn prepare<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    >(
        function: PreparedEffectUnknownRef,
        introduction: &ConstraintLexicalScope,
        original: &OriginalProducerContext,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        let mut dependencies = Vec::new();
        for dependency in graph.read().unknown_dependencies(&function)? {
            context.enter_node()?;
            dependencies.push(dependency.clone());
        }
        let mut edit = graph.edit_terms();
        let mut arguments = Vec::new();
        for dependency in dependencies {
            context.enter_node()?;
            let argument = match &dependency {
                GenericEffectReference::Inference(_) => {
                    let source = original.rigid_scope();
                    let captured = source
                        .imported
                        .and_then(|lease| lease.effect_application(&dependency))
                        .ok_or(PreparedEffectInvariant::ParameterContract)?;
                    captured.transport(introduction, &mut edit, context)?
                }
                GenericEffectReference::Bound(bound) => {
                    introduction
                        .shape()
                        .bound_effect(bound.depth(), bound.slot())?;
                    PreparedEffectRow::Canonical(EffectRow::open(
                        crate::effects::EffectSet::new(),
                        dependency,
                    ))
                }
                GenericEffectReference::Free(_) => PreparedEffectRow::Canonical(EffectRow::open(
                    crate::effects::EffectSet::new(),
                    dependency,
                )),
            };
            arguments.push(argument);
        }
        let application = edit.introduction_application(&function, arguments, context)?;
        edit.commit()?;
        Ok(Self {
            function,
            application,
        })
    }

    pub(super) fn function(&self) -> &PreparedEffectUnknownRef {
        &self.function
    }

    pub(super) fn in_scope<'a>(
        &'a self,
        scope: &'a ConstraintLexicalScope,
    ) -> ConstraintEffectFunctionApplication<'a> {
        ConstraintEffectFunctionApplication {
            binding: self,
            scope,
        }
    }

    pub(super) fn validate(
        &self,
        graph: PreparedEffectReadView<'_>,
    ) -> Result<(), PreparedEffectInvariant> {
        graph.validate_unknown(&self.function)?;
        graph.validate_retained_row(&self.application)
    }
}

impl ConstraintEffectFunctionApplication<'_> {
    pub(in crate::types::constraints) fn validate(
        self,
        graph: PreparedEffectReadView<'_>,
    ) -> Result<(), PreparedEffectInvariant> {
        self.binding.validate(graph)
    }

    /// Captured applications already contain their parents' real applications.
    /// Transport therefore rewrites ambient references, never resolves names or
    /// substitutes a candidate's selected solution into the original relation.
    pub(in crate::types::constraints) fn transport<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    >(
        self,
        target: &ConstraintLexicalScope,
        edit: &mut PreparedEffectTermEdit<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<PreparedEffectRow, TypeConstraintError> {
        self.transport_into(target, target.shape(), edit, context)
    }

    /// A type-owned local binder extends the actual ambient Γ. Captured
    /// applications keep their original Γ dependencies beneath that binder.
    pub(in crate::types::constraints) fn transport_into<
        A: TypeConstraintAccounting,
        D: ConstraintDomain,
    >(
        self,
        target: &ConstraintLexicalScope,
        occurrence: &crate::types::GenericScope,
        edit: &mut PreparedEffectTermEdit<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<PreparedEffectRow, TypeConstraintError> {
        let result = (|| {
            self.binding.validate(edit.read())?;
            for _ in target.shape().binders() {
                context.enter_node()?;
            }
            let inner = usize::try_from(occurrence.extension_depth(target.shape())?)
                .map_err(|_| super::super::TypeConstraintAbort::ArithmeticOverflow)?;
            if self.scope == target && inner == 0 {
                return Ok(edit.read().retain_row(&self.binding.application, context)?);
            }
            edit.map_row_parameters(
                &self.binding.application,
                context,
                &mut |reference, context| {
                    let mapped = match reference {
                        GenericEffectReference::Bound(bound) => {
                            self.scope
                                .shape()
                                .bound_effect(bound.depth(), bound.slot())
                                .map_err(TypeConstraintError::from)
                                .map_err(
                                    crate::effect_row::prepared::PreparedEffectFailure::Control,
                                )?;
                            let depth = target
                                .ambient_depth(self.scope, bound.depth(), 0, inner, context)
                                .map_err(|error| {
                                    crate::effect_row::prepared::PreparedEffectFailure::Control(
                                        TypeConstraintContext::<A, D>::binding_transport_error(
                                            error,
                                            reference.clone().into(),
                                        ),
                                    )
                                })?;
                            occurrence
                                .bound_effect(depth, bound.slot())
                                .map_err(TypeConstraintError::from)
                                .map_err(
                                    crate::effect_row::prepared::PreparedEffectFailure::Control,
                                )?
                        }
                        GenericEffectReference::Free(_) => reference.clone(),
                        GenericEffectReference::Inference(_) => {
                            return Err(PreparedEffectInvariant::ParameterContract.into());
                        }
                    };
                    Ok((&mapped != reference).then(|| {
                        PreparedEffectRow::Canonical(EffectRow::open(
                            crate::effects::EffectSet::new(),
                            mapped,
                        ))
                    }))
                },
            )
            .map_err(TypeConstraintError::from)
        })();
        if result.is_err() {
            edit.invalidate();
        }
        result
    }
}

impl<D: ConstraintDomain> ConstraintApplicationInventory<D> {
    pub(in crate::types::constraints) fn admit_captured_rigids<A: TypeConstraintAccounting>(
        &self,
        effects: &mut EffectConstraintEnvironment<PreparedEffects>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        let mut variables = Vec::new();
        for reference in self.captured_rigid_effects(context)? {
            context.enter_node()?;
            variables.push(crate::effect_row::EffectConstraintVariable::new(
                reference,
                EffectConstraintEligibility::Rigid,
            ));
        }
        effects.admit_variables(&variables).map_err(Into::into)
    }

    /// Prepare the original R with the application owners admitted by this
    /// branch. Its existing effect environment owns the complete rigid Γ.
    pub(in crate::types::constraints) fn prepare_effect_predicate<A: TypeConstraintAccounting>(
        &self,
        effects: &EffectConstraintEnvironment<PreparedEffects>,
        predicate: &crate::effect_row::prepared::PreparedEffectPredicate,
        scope: &ConstraintLexicalScope,
        graph: &mut PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<crate::effect_row::prepared::PreparedEffectPredicate, TypeConstraintError> {
        if predicate.constant_truth().is_some() {
            return Ok(graph.read().retain_predicate(predicate, context)?);
        }
        context.with_effect_transaction(graph, |context, graph| {
            let mut variables = BTreeSet::new();
            graph.read().visit_predicate_parameters(
                predicate,
                context,
                &mut |reference, context| {
                    context.enter_node()?;
                    match reference {
                        GenericEffectReference::Inference(_) => {
                            variables.insert(reference.clone());
                        }
                        GenericEffectReference::Bound(bound) => {
                            scope.shape().bound_effect(bound.depth(), bound.slot())?;
                        }
                        GenericEffectReference::Free(_) => {}
                    }
                    Ok(())
                },
            )?;
            let mut applications = BTreeMap::new();
            let mut edit = graph.edit_terms();
            for variable in variables {
                context.enter_node()?;
                let application = self
                    .effect_application(&variable)
                    .ok_or(PreparedEffectInvariant::ParameterContract)?;
                let row = application.transport(scope, &mut edit, context)?;
                applications.insert(variable, row);
            }
            edit.commit()?;
            effects
                .project_registered_predicate(predicate, &applications, graph, context)
                .map_err(Into::into)
        })
    }

    /// Γ comes from the actual source and imported owners, including unused
    /// rigid inputs. A row's observed support never defines this inventory.
    fn captured_rigid_effects<A: TypeConstraintAccounting>(
        &self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<BTreeSet<GenericEffectReference>, TypeConstraintError> {
        let mut roots = self
            .imported
            .iter()
            .rev()
            .map(|lease| lease.owner.as_ref())
            .chain(
                self.applications
                    .values()
                    .rev()
                    .map(|owner| owner.as_ref() as &dyn ImportedApplicationScopeAuthority),
            )
            .peekable();
        let mut pending = Vec::new();
        let mut visited: BTreeMap<ConstraintApplicationId, &dyn ImportedApplicationScopeAuthority> =
            BTreeMap::new();
        let mut captures = BTreeSet::new();
        while let Some(owner) = pending.pop().or_else(|| roots.next()) {
            context.enter_node()?;
            if let Some(previous) = visited.get(&owner.id()) {
                if !previous.same_owner(owner) {
                    return Err(PreparedEffectInvariant::ParameterContract.into());
                }
                continue;
            }
            let source = owner.original_context().rigid_scope();
            // A terminal owner cannot be encountered again. In particular,
            // a root without imported owners needs no visited table entry.
            if !pending.is_empty()
                || roots.peek().is_some()
                || source
                    .imported
                    .is_some_and(|imported| !imported.applications.is_empty())
            {
                visited.insert(owner.id(), owner);
            }
            for variable in source.effects {
                context.enter_node()?;
                captures.insert(variable.clone());
            }
            for variable in owner.effect_contract().variables() {
                context.enter_node()?;
                if variable.eligibility() == EffectConstraintEligibility::Rigid
                    && variable.variable().free_parameter().is_some()
                {
                    captures.insert(variable.variable().clone());
                }
            }
            if let Some(imported) = source.imported {
                for parameter in imported.parameters() {
                    context.enter_node()?;
                    if let super::super::ConstraintGenericParameterId::Effect(
                        reference @ GenericEffectReference::Free(_),
                    ) = parameter
                    {
                        captures.insert(reference.clone());
                    }
                }
                for lease in &imported.applications {
                    context.enter_node()?;
                    pending.push(lease.owner.as_ref());
                }
            }
        }
        Ok(captures)
    }
}
