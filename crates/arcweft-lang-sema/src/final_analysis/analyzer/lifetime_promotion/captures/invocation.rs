//! Evaluation of checked callable bodies in their creation and invocation frames.

use std::{collections::BTreeSet, rc::Rc};

use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::{ExprId, LocalId, PatternId},
    item::HirItemKind,
    module::HirModule,
    pattern::{
        HirPatternBinding, HirPatternChildRole, HirPatternField, HirPatternKind,
        HirPatternSequenceRest,
    },
    project::{HirBindingSite, HirExpressionBindingRole, HirSemanticPathStep},
    scope::{HirScopeKind, HirScopeOwner},
    source_index::HirCallableSourceOwner,
    symbol::{CallableDeclarationKey, CallableDeclarationOwner},
};

use crate::{
    callable::{CallableGroupIndex, CallableParameterCoordinate, CallableParameterIndex},
    final_analysis::{
        CheckedExpressionResolution, CheckedValueResolution, PreparedExpressionFact,
        model::CallableResultSource, prepared::PreparedRecordPatternSource,
    },
};

use super::{
    Analyzer, AnalyzerExpressionContext, CaptureEvaluation, FinalSemanticAnalysisError,
    ReceiverBorrowQuery,
    inputs::{InvocationInputs, SuffixInput},
    localization::RetainedBodyProjection,
    worklist::{
        BodyProjection, BorrowDependencies, CaptureWorklist, CarrierStorage, ClosureKind,
        FrameOwner, Invocation, InvokedValue, Observation, ParameterDemand, PositionalStorage,
        StorageFrameId, StorageProjection, ValueOrigin,
    },
};

struct BoundInvocation {
    call: Invocation,
    frame: StorageFrameId,
    inputs: InvocationInputs,
}

#[derive(Clone, Copy)]
pub(super) enum StorageLexicalSite {
    Expression(ExprId),
    Binding(LocalId),
}

pub(super) struct SourceInvocation {
    call: Invocation,
    declaration: CallableDeclarationKey,
    inputs: InvocationInputs,
    projection: BodyProjection,
    parent: Option<StorageFrameId>,
}

impl SourceInvocation {
    pub(super) fn selected(call: Invocation, declaration: CallableDeclarationKey) -> Self {
        Self {
            call,
            declaration,
            inputs: InvocationInputs::selected(call, None),
            projection: BodyProjection::SelectedCall(call.site),
            parent: Some(call.caller),
        }
    }

    pub(super) fn retained(
        prefix: Invocation,
        suffix: Vec<SuffixInput>,
        terminal: Invocation,
        declaration: CallableDeclarationKey,
        projection: RetainedBodyProjection,
    ) -> Self {
        Self {
            call: terminal,
            declaration,
            inputs: InvocationInputs {
                prefix,
                requester: None,
                suffix,
            },
            projection: BodyProjection::RetainedPrefix(Rc::new(projection)),
            parent: None,
        }
    }
}

impl Analyzer<'_, '_, '_> {
    pub(super) fn invoke_producer(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        origin: ValueOrigin,
        (producer, invoked): (ExprId, InvokedValue),
        query: &mut CaptureEvaluation,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<Option<BorrowDependencies>, FinalSemanticAnalysisError> {
        let call = query.work.invocation(invoked.id)?;
        let Some(fact) = self.facts.expressions().get(&producer) else {
            return Ok(None);
        };
        if let Some(callable) = fact.implicit_callable() {
            let frame = query.work.intern_closure(
                producer,
                origin.frame,
                ClosureKind::Implicit,
                receiver,
            )?;
            return self
                .result_dependencies(
                    origin,
                    BoundInvocation {
                        call,
                        frame,
                        inputs: InvocationInputs::selected(call, Some(invoked.id)),
                    },
                    callable.body().value_results(),
                    &mut query.work,
                    receiver,
                )
                .map(Some);
        }
        let Some(fact) = fact.complete() else {
            return Ok(None);
        };
        if let CheckedExpressionResolution::Closure(closure) = fact.resolution() {
            let frame = query.work.intern_closure(
                producer,
                origin.frame,
                ClosureKind::Explicit,
                receiver,
            )?;
            return self
                .result_dependencies(
                    origin,
                    BoundInvocation {
                        call,
                        frame,
                        inputs: InvocationInputs::selected(call, Some(invoked.id)),
                    },
                    closure.value_results(),
                    &mut query.work,
                    receiver,
                )
                .map(Some);
        }
        if let CheckedExpressionResolution::Value(CheckedValueResolution::ProjectCallable(callable)) =
            fact.resolution()
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
            source.inputs.requester = Some(invoked.id);
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
                source.parent = Some(parent);
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
            inputs,
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
                .insert(declaration.clone(), (item, values.into_boxed_slice()));
            query.captures = self
                .pending_capture_identity_rows()
                .map_err(|error| error.into_public(query.root))?;
        }
        let (item, values) = query
            .results
            .get(&declaration)
            .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
        let parent = parent.map(|parent| query.work.frame(parent)).transpose()?;
        let parent = parent
            .as_ref()
            .map_or(&[][..], |frame| frame.projections.as_ref());
        let storage = self.application_storage(call.site, *item, &projection, parent, receiver)?;
        let frame = query.work.intern(storage, parent, projection, receiver)?;
        self.result_dependencies(
            origin,
            BoundInvocation {
                call,
                frame,
                inputs,
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
        values: &[CallableResultSource],
        work: &mut CaptureWorklist,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
        let mut dependencies = BorrowDependencies::default();
        for &source in values {
            let (expression, observation) = match source {
                CallableResultSource::Returned(expression) => (expression, binding.call.result),
                CallableResultSource::TrySuccess(expression) => {
                    let frame = work.frame(binding.frame)?;
                    let carrier = self
                        .storage_expression_fact(expression, &frame)
                        .and_then(PreparedExpressionFact::try_carrier)
                        .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                    let Some(payload) = work
                        .carrier_payload(binding.call.result, CarrierStorage::success(carrier))?
                    else {
                        continue;
                    };
                    (expression, payload)
                }
                CallableResultSource::ResultResidual(operand) => {
                    let error = CarrierStorage::ResultErr;
                    let Some(payload) = work.carrier_payload(binding.call.result, error)? else {
                        continue;
                    };
                    let observation =
                        work.project(StorageProjection::Carrier(error), payload, receiver)?;
                    (operand, observation)
                }
            };
            dependencies.include(work.demand(
                ValueOrigin {
                    expression,
                    frame: binding.frame,
                    observation,
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
            BorrowDependencies::Parameters(parameters) => self.substitute_call_parameters(
                (binding.frame, binding.inputs),
                origin,
                parameters,
                work,
                receiver,
            ),
        }
    }

    pub(super) fn lexical_frame(
        &self,
        module: &HirModule,
        site: StorageLexicalSite,
        mut frame: StorageFrameId,
        work: &CaptureWorklist,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<StorageFrameId, FinalSemanticAnalysisError> {
        if !matches!(work.frame(frame)?.owner, FrameOwner::Closure { .. }) {
            return Ok(frame);
        }
        let topology = self
            .topology
            .module(module.module_id())
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        let (mut scope, expression) = match site {
            StorageLexicalSite::Expression(owner) => (
                Some(
                    module
                        .resolve_expr(owner)
                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
                        .scope(),
                ),
                Some(owner),
            ),
            StorageLexicalSite::Binding(local) => (
                Some(
                    module
                        .resolve_local(local)
                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
                        .scope(),
                ),
                topology
                    .local_origins()
                    .binding(local)
                    .and_then(|binding| binding.binding_expression()),
            ),
        };
        let mut closures = BTreeSet::new();
        while let FrameOwner::Closure {
            producer,
            creator,
            kind,
        } = work.frame(frame)?.owner
        {
            receiver.charge(1)?;
            let belongs = match kind {
                ClosureKind::Explicit => {
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
                    closures.contains(&producer)
                }
                ClosureKind::Implicit if producer.module() == module.module_id() => {
                    let Some(expression) = expression else {
                        frame = creator;
                        continue;
                    };
                    let region = topology
                        .expression_uses()
                        .implicit_callable_region(producer)
                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                    region.try_contains_executed_expression(
                        expression,
                        &|boundary| {
                            self.facts
                                .expressions()
                                .get(&boundary)
                                .is_some_and(PreparedExpressionFact::creates_implicit_callable)
                        },
                        || receiver.charge(1),
                    )?
                }
                ClosureKind::Implicit => false,
            };
            if belongs {
                break;
            }
            frame = creator;
        }
        Ok(frame)
    }

    pub(super) fn implicit_parameter_demand(
        callable: ExprId,
        origin: ValueOrigin,
        work: &CaptureWorklist,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
        let mut frame = origin.frame;
        while let FrameOwner::Closure {
            producer,
            creator,
            kind,
        } = work.frame(frame)?.owner
        {
            receiver.charge(1)?;
            if producer == callable && kind == ClosureKind::Implicit {
                let parameter = CallableParameterIndex::try_from_usize(0)
                    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
                return Ok(BorrowDependencies::Parameters(BTreeSet::from([
                    ParameterDemand {
                        frame,
                        coordinate: CallableParameterCoordinate::new(
                            CallableGroupIndex::ZERO,
                            parameter,
                        ),
                        observation: origin.observation,
                        invocation_caller: None,
                    },
                ])));
            }
            frame = creator;
        }
        Ok(BorrowDependencies::default())
    }

    pub(super) fn parameter_demand(
        &self,
        module: &HirModule,
        local: LocalId,
        origin: ValueOrigin,
        work: &mut CaptureWorklist,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<Option<ParameterDemand>, FinalSemanticAnalysisError> {
        let frame = self.lexical_frame(
            module,
            StorageLexicalSite::Binding(local),
            origin.frame,
            work,
            receiver,
        )?;
        let Some((coordinate, pattern)) =
            self.parameter_pattern(module, local, work.frame(frame)?.owner)?
        else {
            return Ok(None);
        };
        let Some(observation) = self.parameter_storage_observation(
            module,
            (pattern, local),
            origin.observation,
            work,
            receiver,
        )?
        else {
            return Ok(None);
        };
        Ok(Some(ParameterDemand {
            frame,
            coordinate,
            observation,
            invocation_caller: None,
        }))
    }

    fn parameter_pattern(
        &self,
        module: &HirModule,
        local: LocalId,
        owner: FrameOwner,
    ) -> Result<Option<(CallableParameterCoordinate, PatternId)>, FinalSemanticAnalysisError> {
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
                (group, parameter, source.pattern())
            }
            (
                FrameOwner::Closure {
                    producer,
                    kind: ClosureKind::Explicit,
                    ..
                },
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
        let group = crate::callable::CallableGroupIndex::try_from_usize(group as usize)
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        let parameter = crate::callable::CallableParameterIndex::try_from_usize(parameter as usize)
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        Ok(Some((
            CallableParameterCoordinate::new(group, parameter),
            pattern,
        )))
    }

    fn parameter_storage_observation(
        &self,
        module: &HirModule,
        (root, local): (PatternId, LocalId),
        mut observation: Observation,
        work: &mut CaptureWorklist,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<Option<Observation>, FinalSemanticAnalysisError> {
        let resolve = |pattern| {
            module
                .resolve_pattern(pattern)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)
        };
        if pattern_binds_whole_value(resolve(root)?.kind(), local) {
            return Ok(Some(observation));
        }
        let root_path = self
            .topology
            .semantic_path(root.into())
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        let binding_path = self
            .topology
            .semantic_path(local.into())
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        let Some((last, path)) = binding_path
            .path()
            .steps()
            .strip_prefix(root_path.path().steps())
            .filter(|_| binding_path.root() == root_path.root())
            .and_then(<[_]>::split_last)
        else {
            return Err(FinalSemanticAnalysisError::InvalidOwner);
        };
        receiver.charge(path.len() as u64 + 1)?;
        let mut selected = Vec::new();
        let mut pattern = root;
        for step in path {
            let kind = resolve(pattern)?.kind();
            match (kind, step) {
                (
                    HirPatternKind::Tuple { elements },
                    HirSemanticPathStep::Pattern(HirPatternChildRole::Element { ordinal }),
                )
                | (
                    HirPatternKind::BracketSequence { elements, .. },
                    HirSemanticPathStep::Pattern(HirPatternChildRole::Element { ordinal }),
                ) => {
                    let family = if matches!(kind, HirPatternKind::Tuple { .. }) {
                        PositionalStorage::Tuple
                    } else {
                        PositionalStorage::Sequence
                    };
                    selected.push(StorageProjection::Element {
                        family,
                        ordinal: *ordinal,
                    });
                    pattern = *elements
                        .get(*ordinal as usize)
                        .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
                }
                (
                    HirPatternKind::WholeBinding {
                        pattern: nested, ..
                    },
                    HirSemanticPathStep::Pattern(HirPatternChildRole::NestedPattern),
                ) => {
                    pattern = *nested;
                }
                (
                    HirPatternKind::Record { .. },
                    HirSemanticPathStep::Pattern(HirPatternChildRole::RecordField { field }),
                ) => {
                    let Some((selection, PreparedRecordPatternSource::Pattern(child))) =
                        self.formal_record_field(module, pattern, *field, receiver)?
                    else {
                        return Ok(None);
                    };
                    selected.push(selection);
                    pattern = child;
                }
                _ => return Ok(None),
            }
        }
        let kind = resolve(pattern)?.kind();
        match (kind, last) {
            (
                _,
                HirSemanticPathStep::Pattern(
                    HirPatternChildRole::BindingLocal
                    | HirPatternChildRole::TypedBindingLocal
                    | HirPatternChildRole::WholeBindingLocal,
                ),
            ) if pattern_binds_whole_value(kind, local) => {}
            (
                HirPatternKind::Record { .. },
                HirSemanticPathStep::Pattern(HirPatternChildRole::RecordShorthandLocal { field }),
            ) => {
                let Some((selection, PreparedRecordPatternSource::Binding(bound))) =
                    self.formal_record_field(module, pattern, *field, receiver)?
                else {
                    return Ok(None);
                };
                if bound != local {
                    return Err(FinalSemanticAnalysisError::InvalidOwner);
                }
                selected.push(selection);
            }
            (
                HirPatternKind::Record { fields, .. },
                HirSemanticPathStep::Pattern(HirPatternChildRole::RecordRestLocal { field }),
            ) if matches!(fields.get(*field as usize), Some(HirPatternField::Rest { binding: Some(bound) }) if *bound == local) =>
                {}
            (
                HirPatternKind::BracketSequence {
                    elements,
                    rest: HirPatternSequenceRest::Bound(bound),
                },
                HirSemanticPathStep::Pattern(HirPatternChildRole::SequenceRestLocal),
            ) if *bound == local => {
                let from = u32::try_from(elements.len())
                    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
                let Some(tail) = work.project_sequence_tail(from, observation, receiver)? else {
                    return Ok(None);
                };
                observation = tail;
            }
            _ => return Ok(None),
        }
        for projection in selected.into_iter().rev() {
            observation = work.project(projection, observation, receiver)?;
        }
        Ok(Some(observation))
    }

    fn formal_record_field(
        &self,
        module: &HirModule,
        pattern: PatternId,
        source_ordinal: u32,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<Option<(StorageProjection, PreparedRecordPatternSource)>, FinalSemanticAnalysisError>
    {
        let HirPatternKind::Record { path, fields } = module
            .resolve_pattern(pattern)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
            .kind()
        else {
            return Err(FinalSemanticAnalysisError::InvalidOwner);
        };
        let ty = self
            .facts
            .patterns()
            .get(&pattern)
            .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern })?;
        let crate::types::TypeKind::ProjectNominal(nominal) = ty else {
            return Ok(None);
        };
        receiver.charge(fields.len() as u64)?;
        let prepared = self.prepare_record_pattern(module, pattern, path, fields, ty)?;
        let field = prepared
            .fields()
            .iter()
            .find(|field| field.source_ordinal() == source_ordinal)
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        Ok(Some((
            StorageProjection::Field {
                nominal: nominal.declaration().clone(),
                ordinal: field.coordinate().declaration_ordinal(),
            },
            field.source(),
        )))
    }
}

fn pattern_binds_whole_value(kind: &HirPatternKind, local: LocalId) -> bool {
    matches!(kind,
        HirPatternKind::Binding(HirPatternBinding::Bound { local: bound, .. })
        | HirPatternKind::TypedBinding { binding: HirPatternBinding::Bound { local: bound, .. }, .. }
        | HirPatternKind::WholeBinding { binding: HirPatternBinding::Bound { local: bound, .. }, .. }
        if *bound == local)
}
