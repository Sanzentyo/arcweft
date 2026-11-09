//! Evaluation of checked callable bodies in their creation and invocation frames.

use std::{collections::BTreeSet, rc::Rc};

use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::{ExprId, LocalId, ScopeId},
    item::{HirItemKind, HirParameterKind},
    module::HirModule,
    pattern::{HirPatternBinding, HirPatternKind},
    project::{HirBindingSite, HirExpressionBindingRole},
    scope::{HirScopeKind, HirScopeOwner},
    source_index::HirCallableSourceOwner,
    symbol::{CallableDeclarationKey, CallableDeclarationOwner},
};

use crate::{
    callable::CallableParameterCoordinate,
    final_analysis::{CheckedExpressionResolution, CheckedValueResolution, PreparedExpressionFact},
};

use super::{
    Analyzer, AnalyzerExpressionContext, CaptureEvaluation, FinalSemanticAnalysisError,
    ReceiverBorrowQuery, call_storage_error,
    worklist::{
        BodyProjection, BorrowDependencies, CaptureWorklist, FrameOwner, Invocation, InvocationId,
        InvokedValue, ParameterDemand, StorageFrameId, ValueOrigin,
    },
};

#[derive(Clone, Copy)]
struct BoundInvocation {
    call: Invocation,
    frame: StorageFrameId,
    requester: Option<InvocationId>,
}

pub(super) struct SourceInvocation {
    call: Invocation,
    declaration: CallableDeclarationKey,
    requester: Option<InvocationId>,
    projection: BodyProjection,
    parent: StorageFrameId,
}

impl SourceInvocation {
    pub(super) fn selected(call: Invocation, declaration: CallableDeclarationKey) -> Self {
        Self {
            call,
            declaration,
            requester: None,
            projection: BodyProjection::SelectedCall(call.site),
            parent: call.caller,
        }
    }
}

impl Analyzer<'_, '_, '_> {
    pub(super) fn invoke_producer(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        module: &HirModule,
        origin: ValueOrigin,
        (producer, invoked): (ExprId, InvokedValue),
        query: &mut CaptureEvaluation,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<Option<BorrowDependencies>, FinalSemanticAnalysisError> {
        let call = query.work.invocation(invoked.id)?;
        let source = module
            .resolve_expr(producer)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if let HirExprKind::Closure(closure) = source.kind() {
            if let std::collections::btree_map::Entry::Vacant(entry) =
                query.closure_results.entry(producer)
            {
                let values = self
                    .checked_closure_value_sources(module, producer, closure.body())
                    .map_err(|error| error.into_public(closure.body()))?;
                receiver.charge(
                    u64::try_from(values.len())
                        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                )?;
                entry.insert(Rc::from(values));
            }
            let frame = query
                .work
                .intern_closure(producer, origin.frame, receiver)?;
            return self
                .result_dependencies(
                    origin,
                    BoundInvocation {
                        call,
                        frame,
                        requester: Some(invoked.id),
                    },
                    &query.closure_results[&producer],
                    &mut query.work,
                    receiver,
                )
                .map(Some);
        }
        if let Some(fact) = self
            .facts
            .expressions()
            .get(&producer)
            .and_then(PreparedExpressionFact::complete)
            && let CheckedExpressionResolution::Value(CheckedValueResolution::ProjectCallable(
                callable,
            )) = fact.resolution()
            && callable.declaration().owner() == CallableDeclarationOwner::Function
        {
            let item = self
                .module(callable.owner().module())?
                .resolve_item(callable.owner())
                .map_err(|_| FinalSemanticAnalysisError::InvalidCallableOwner)?;
            let HirItemKind::Function(function) = item.kind() else {
                return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
            };
            let declaration = callable.declaration().clone();
            let mut source = SourceInvocation::selected(call, declaration);
            source.requester = Some(invoked.id);
            if !function.generic_parameters().is_empty() {
                let (parent, witness) = if let Some(specialization) = invoked.specialization {
                    (
                        specialization.frame,
                        self.value_specialization(specialization.owner)?,
                    )
                } else {
                    let Some(witness) = self.invocation_specialization(call.site, receiver)? else {
                        return Ok(None);
                    };
                    (call.caller, witness)
                };
                if fact.source_value_type() != Some(witness.source_type()) {
                    return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
                }
                source.parent = parent;
                source.projection =
                    self.specialized_body_projection(&source.declaration, witness, receiver)?;
            }
            return self
                .source_result_dependencies(context, origin, source, query, receiver)
                .map(Some);
        }
        Ok(None)
    }

    pub(super) fn source_result_dependencies(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        origin: ValueOrigin,
        SourceInvocation {
            call,
            declaration,
            requester,
            projection,
            parent,
        }: SourceInvocation,
        query: &mut CaptureEvaluation,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
        if !query.results.contains_key(&declaration) {
            let (item, values) = self.source_function_values(context, &declaration)?;
            receiver.charge(
                u64::try_from(values.len())
                    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
            )?;
            query
                .results
                .insert(declaration.clone(), (item, Rc::from(values)));
            query.captures = self
                .pending_capture_identity_rows()
                .map_err(|error| error.into_public(query.root))?;
        }
        let (item, values) = query
            .results
            .get(&declaration)
            .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
        let parent = query.work.frame(parent)?;
        let storage = self.application_storage(call.site, *item, &projection, &parent, receiver)?;
        let frame = query.work.intern(storage, &parent, projection, receiver)?;
        self.result_dependencies(
            origin,
            BoundInvocation {
                call,
                frame,
                requester,
            },
            values,
            &mut query.work,
            receiver,
        )
    }

    fn result_dependencies(
        &self,
        origin: ValueOrigin,
        binding: BoundInvocation,
        values: &[ExprId],
        work: &mut CaptureWorklist,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
        let mut dependencies = BorrowDependencies::default();
        for &expression in values {
            dependencies.include(work.demand(
                ValueOrigin {
                    expression,
                    frame: binding.frame,
                    observation: binding.call.result,
                },
                origin,
                receiver,
            )?);
            if dependencies.is_borrowed() {
                return Ok(dependencies);
            }
        }
        match dependencies {
            BorrowDependencies::Borrowed => Ok(BorrowDependencies::Borrowed),
            BorrowDependencies::Parameters(parameters) => {
                self.substitute_call_parameters(binding, origin, parameters, work, receiver)
            }
        }
    }

    fn substitute_call_parameters(
        &self,
        binding: BoundInvocation,
        origin: ValueOrigin,
        parameters: BTreeSet<ParameterDemand>,
        work: &mut CaptureWorklist,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
        let mut mapped = Vec::new();
        let mut external = BTreeSet::new();
        for parameter in parameters {
            receiver.charge(1)?;
            if parameter.frame == binding.frame && parameter.invocation_caller.is_none() {
                mapped.push(parameter);
            } else {
                external.insert(parameter);
            }
        }
        let mut dependencies = BorrowDependencies::Parameters(external);
        if mapped.is_empty() {
            return Ok(dependencies);
        }
        let graph = self
            .facts
            .prepared_calls()
            .map_err(FinalSemanticAnalysisError::from)?;
        graph
            .project_site_payload(
                binding.call.site,
                |prefix| {
                    let enclosing = prefix.record().enclosing_callable();
                    let mut input = prefix;
                    loop {
                        receiver.charge(1)?;
                        if input.record().enclosing_callable() == enclosing {
                            let group = input.application().completed_group();
                            for &parameter in &mapped {
                                receiver.charge(1)?;
                                if parameter.coordinate.group() == group
                                    && let Some(expression) =
                                        input.parameter_value_source(parameter.coordinate)
                                {
                                    let mut supplied = work.demand(
                                        ValueOrigin {
                                            expression,
                                            frame: binding.call.caller,
                                            observation: parameter.observation,
                                        },
                                        origin,
                                        receiver,
                                    )?;
                                    if let Some(requester) = binding.requester {
                                        supplied = supplied.map_parameters(|mut demand| {
                                            if demand.invocation_caller.is_none() {
                                                demand.invocation_caller = Some(requester);
                                            }
                                            demand
                                        });
                                    }
                                    dependencies.include(supplied);
                                    if dependencies.is_borrowed() {
                                        return Ok(dependencies);
                                    }
                                }
                            }
                        }
                        let Some(previous) = input.application().selected().prepared_continuation()
                        else {
                            break;
                        };
                        input = graph
                            .continuation_prefix(previous)
                            .map_err(|error| call_storage_error(binding.call.site, error))?;
                    }
                    Ok(dependencies)
                },
                |_| Err(FinalSemanticAnalysisError::InvalidOwner),
            )
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?
    }

    pub(super) fn lexical_frame(
        module: &HirModule,
        scope: ScopeId,
        mut frame: StorageFrameId,
        work: &CaptureWorklist,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<StorageFrameId, FinalSemanticAnalysisError> {
        if !matches!(work.frame(frame)?.owner, FrameOwner::Closure { .. }) {
            return Ok(frame);
        }
        let mut closures = BTreeSet::new();
        let mut scope = Some(scope);
        while let Some(id) = scope {
            receiver.charge(1)?;
            let lexical = module
                .resolve_scope(id)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            if lexical.kind() == HirScopeKind::Closure
                && let HirScopeOwner::Expr(owner) = lexical.owner()
            {
                closures.insert(*owner);
            }
            scope = lexical.parent();
        }
        while let FrameOwner::Closure { producer, creator } = work.frame(frame)?.owner {
            receiver.charge(1)?;
            if closures.contains(&producer) {
                break;
            }
            frame = creator;
        }
        Ok(frame)
    }

    pub(super) fn parameter_demand(
        &self,
        module: &HirModule,
        local: LocalId,
        origin: ValueOrigin,
        work: &CaptureWorklist,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<Option<ParameterDemand>, FinalSemanticAnalysisError> {
        let local_scope = module
            .resolve_local(local)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
            .scope();
        let frame = Self::lexical_frame(module, local_scope, origin.frame, work, receiver)?;
        let Some(coordinate) =
            self.whole_parameter_coordinate(module, local, work.frame(frame)?.owner)?
        else {
            return Ok(None);
        };
        Ok(Some(ParameterDemand {
            frame,
            coordinate,
            observation: origin.observation,
            invocation_caller: None,
        }))
    }

    fn whole_parameter_coordinate(
        &self,
        module: &HirModule,
        local: LocalId,
        owner: FrameOwner,
    ) -> Result<Option<CallableParameterCoordinate>, FinalSemanticAnalysisError> {
        if owner == FrameOwner::Root {
            return Ok(None);
        }
        let origins = self
            .topology
            .module(module.module_id())
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?
            .local_origins();
        let binding = origins
            .binding(local)
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        if binding.is_mutable_binding() {
            return Ok(None);
        }
        let (group, parameter, pattern) = match (owner, binding.site()) {
            (
                FrameOwner::Function(item),
                HirBindingSite::DeclarationParameter {
                    item: source,
                    owner: HirCallableSourceOwner::Item,
                    group,
                    parameter,
                },
            ) if item == source => {
                let item_source = module
                    .resolve_item(item)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                let HirItemKind::Function(function) = item_source.kind() else {
                    return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
                };
                let source = function
                    .parameter_groups()
                    .get(group as usize)
                    .and_then(|group| group.parameters().get(parameter as usize))
                    .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
                if source.kind() == HirParameterKind::RestPositional {
                    return Ok(None);
                }
                (group, parameter, source.pattern())
            }
            (
                FrameOwner::Closure { producer, .. },
                HirBindingSite::Expression {
                    expression,
                    role: HirExpressionBindingRole::ClosureParameter { parameter },
                },
            ) if producer == expression => {
                let expression = module
                    .resolve_expr(producer)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                let HirExprKind::Closure(closure) = expression.kind() else {
                    return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
                };
                let source = closure
                    .parameters()
                    .get(parameter as usize)
                    .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
                (0, parameter, source.pattern())
            }
            _ => return Ok(None),
        };
        let pattern = module
            .resolve_pattern(pattern)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if !matches!(pattern.kind(),
            HirPatternKind::Binding(HirPatternBinding::Bound { local: bound, .. })
            | HirPatternKind::TypedBinding { binding: HirPatternBinding::Bound { local: bound, .. }, .. }
            | HirPatternKind::WholeBinding { binding: HirPatternBinding::Bound { local: bound, .. }, .. }
            if *bound == local)
        {
            return Ok(None);
        }
        let group = crate::callable::CallableGroupIndex::try_from_usize(group as usize)
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        let parameter = crate::callable::CallableParameterIndex::try_from_usize(parameter as usize)
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        Ok(Some(CallableParameterCoordinate::new(group, parameter)))
    }
}
