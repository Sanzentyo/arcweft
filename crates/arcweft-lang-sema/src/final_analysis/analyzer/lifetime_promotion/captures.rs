//! Borrowed callable captures evidenced by checked value origins.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_lang_hir::{
    identity::{ExprId, ItemId, LocalId},
    item::{HirFunctionBody, HirItemKind},
    module::HirModule,
    project::HirLocalValueOrigin,
    source_index::HirCallableSourceOwner,
    symbol::{CallableDeclarationKey, CallableDeclarationOwner},
};

use super::{Analyzer, FinalSemanticAnalysisError, ReceiverBorrowQuery, TypeStorage};
use crate::{
    callable::{
        CallableCandidateId, CallableParameterCoordinate, CallableParameterPassing,
        CheckedCallSite, PreparedCallGraphIngress, PreparedCallSiteDelivery,
        PreparedCaptureIdentityRow, PreparedFunctionValueOriginEvidence,
        PreparedFunctionValueOriginProducer, PreparedFunctionValueOriginProgress,
        PreparedFunctionValueOriginPurpose, PreparedFunctionValueOriginQueryError,
        PreparedResolvedCallable, prepare_function_value_origin_query_with_pending_captures,
    },
    final_analysis::{
        FinalCallConstraintFailure, PreparedExpressionFact,
        analyzer::{
            AnalyzerExpressionContext, CallAnalysisFailure,
            calls::{AnalyzerPreparedCallPrefix, CallAnalysisInvariant},
            expression_error::AnalyzerExpressionError,
        },
        model::CallableResultSource,
    },
    types::{TypeKind, TypeProjectionError},
};

#[path = "captures/invocation.rs"]
mod invocation;

#[path = "captures/continuation.rs"]
mod continuation;

#[path = "captures/inputs.rs"]
mod inputs;

#[path = "captures/localization.rs"]
mod localization;

#[path = "captures/projection.rs"]
mod projection;

#[path = "captures/worklist.rs"]
mod worklist;

#[path = "captures/structural.rs"]
mod structural;

use self::invocation::StorageLexicalSite;
use self::worklist::{
    BodyProjection, BorrowDependencies, CaptureWorklist, CarrierStorage, ClosureKind, FrameOwner,
    Invocation, InvokedValue, Observation, PositionalStorage, StorageFrame, StorageProjection,
    ValueOrigin, ValueSpecialization,
};

type PendingCaptures = Arc<BTreeMap<ExprId, Box<[PreparedCaptureIdentityRow]>>>;

struct CaptureEvaluation {
    root: ExprId,
    captures: PendingCaptures,
    results: BTreeMap<CallableDeclarationKey, (ItemId, Box<[CallableResultSource]>)>,
    work: CaptureWorklist,
}

enum CallStorage {
    Borrowed,
    SourceResult(CallableDeclarationKey),
    SourcePrefix(Arc<PreparedResolvedCallable>),
    ValueResult(ExprId),
    NoWitness,
}

enum StorageOrigin {
    Borrowed,
    Checked(PreparedFunctionValueOriginEvidence),
    Unavailable,
}

impl Analyzer<'_, '_, '_> {
    /// Follows only the selected value origins and storage evidence. Executable
    /// promotion still requires a complete ownership and lifetime proof.
    pub(super) fn value_has_borrowed_captures(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        root: ExprId,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<bool, FinalSemanticAnalysisError> {
        let mut query = CaptureEvaluation {
            root,
            captures: self
                .pending_capture_identity_rows()
                .map_err(|error| error.into_public(root))?,
            results: BTreeMap::new(),
            work: CaptureWorklist::new(root),
        };
        while let Some(origin) = query.work.next()? {
            receiver.charge(1)?;
            let dependencies =
                self.evaluate_capture_origin(context, origin, &mut query, receiver)?;
            if dependencies.is_borrowed() && query.work.is_root(origin) {
                return Ok(true);
            }
            query.work.record(origin, dependencies, receiver)?;
        }
        Ok(false)
    }

    fn evaluate_capture_origin(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        origin: ValueOrigin,
        query: &mut CaptureEvaluation,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
        let module = self.module(origin.expression.module())?;
        let frame = query.work.frame(origin.frame)?;
        let implicit_frame = matches!(
            frame.owner,
            FrameOwner::Closure {
                kind: ClosureKind::Implicit,
                ..
            }
        );
        if implicit_frame {
            let lexical = self.lexical_frame(
                module,
                StorageLexicalSite::Expression(origin.expression),
                origin.frame,
                &query.work,
                receiver,
            )?;
            if lexical != origin.frame {
                return query.work.demand(
                    ValueOrigin {
                        frame: lexical,
                        ..origin
                    },
                    origin,
                    receiver,
                );
            }
        }
        let evidence =
            match self.checked_storage_origin(module, origin, &frame, &query.captures, receiver)? {
                StorageOrigin::Borrowed => return Ok(BorrowDependencies::Borrowed),
                StorageOrigin::Checked(evidence) => evidence,
                StorageOrigin::Unavailable => return Ok(BorrowDependencies::default()),
            };
        let mut observation = origin.observation;
        if let Observation::Invoke(invoked) = &mut observation
            && let Some(owner) = evidence.storage_specialization()
        {
            let specialization = ValueSpecialization {
                owner,
                frame: self.lexical_frame(
                    module,
                    StorageLexicalSite::Expression(owner),
                    origin.frame,
                    &query.work,
                    receiver,
                )?,
            };
            if let Some(existing) = invoked.specialization
                && existing != specialization
            {
                return Err(FinalSemanticAnalysisError::InvalidOwner);
            }
            invoked.specialization = Some(specialization);
        }
        let producer = match evidence.producer() {
            PreparedFunctionValueOriginProducer::Lexical { local } => {
                let mut parameters = BTreeSet::new();
                if let Some(parameter) = self.parameter_demand(
                    module,
                    *local,
                    ValueOrigin {
                        observation,
                        ..origin
                    },
                    &mut query.work,
                    receiver,
                )? {
                    receiver.charge(1)?;
                    parameters.insert(parameter);
                }
                return Ok(BorrowDependencies::Parameters(parameters));
            }
            PreparedFunctionValueOriginProducer::Call(site)
            | PreparedFunctionValueOriginProducer::PreparedContinuation(site) => site.expression(),
            PreparedFunctionValueOriginProducer::Declaration { producer, .. }
            | PreparedFunctionValueOriginProducer::IndependentExpression { producer } => *producer,
        };
        let producer_frame = if implicit_frame && producer == origin.expression {
            origin.frame
        } else {
            self.lexical_frame(
                module,
                StorageLexicalSite::Expression(producer),
                origin.frame,
                &query.work,
                receiver,
            )?
        };
        if producer_frame != origin.frame || observation != origin.observation {
            return query.work.demand(
                ValueOrigin {
                    expression: producer,
                    frame: producer_frame,
                    observation,
                },
                origin,
                receiver,
            );
        }
        self.producer_dependencies(
            context,
            origin,
            (&frame, module),
            &evidence,
            query,
            receiver,
        )
    }

    fn producer_dependencies(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        origin: ValueOrigin,
        (frame, module): (&StorageFrame, &HirModule),
        evidence: &PreparedFunctionValueOriginEvidence,
        query: &mut CaptureEvaluation,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
        let mut next = Vec::new();
        if let PreparedFunctionValueOriginProducer::Call(site) = evidence.producer() {
            let call = Invocation {
                site: *site,
                caller: origin.frame,
                result: origin.observation,
            };
            match self.call_storage(call, frame, &query.work, receiver, &mut next)? {
                CallStorage::Borrowed => return Ok(BorrowDependencies::Borrowed),
                CallStorage::SourceResult(declaration) => {
                    return self.source_result_dependencies(
                        context,
                        origin,
                        invocation::SourceInvocation::selected(call, declaration),
                        query,
                        receiver,
                    );
                }
                CallStorage::SourcePrefix(selected) => {
                    return self
                        .invoke_returned_prefix(context, origin, call, &selected, query, receiver);
                }
                CallStorage::ValueResult(callee) => {
                    let id = query.work.intern_invocation(call, receiver)?;
                    return Ok(query
                        .work
                        .demand(
                            ValueOrigin {
                                expression: callee,
                                observation: Observation::Invoke(InvokedValue {
                                    id,
                                    specialization: None,
                                }),
                                ..origin
                            },
                            origin,
                            receiver,
                        )?
                        .map_parameters(|mut parameter| {
                            if parameter.invocation_caller == Some(id) {
                                parameter.invocation_caller = None;
                            }
                            parameter
                        }));
                }
                CallStorage::NoWitness => {}
            }
        }
        if let PreparedFunctionValueOriginProducer::IndependentExpression { producer } =
            evidence.producer()
            && let Some(callable) = self
                .storage_expression_fact(*producer, frame)
                .and_then(PreparedExpressionFact::implicit_parameter_owner)
        {
            return Self::implicit_parameter_demand(callable, origin, &query.work, receiver);
        }
        let mut dependencies = BorrowDependencies::default();
        match origin.observation {
            Observation::Stored => {
                dependencies = self.stored_captures(
                    module,
                    origin,
                    frame,
                    evidence,
                    &mut query.work,
                    receiver,
                )?;
                if dependencies.is_borrowed() {
                    return Ok(dependencies);
                }
            }
            Observation::Invoke(invoked) => {
                if let PreparedFunctionValueOriginProducer::IndependentExpression { producer } =
                    evidence.producer()
                    && !frame.owner.executes_implicit_body(*producer)
                    && let Some(dependencies) = self.invoke_producer(
                        context,
                        origin,
                        (*producer, invoked),
                        query,
                        receiver,
                    )?
                {
                    return Ok(dependencies);
                }
            }
            Observation::Project(_) => {}
        }
        if let PreparedFunctionValueOriginProducer::IndependentExpression { producer } =
            evidence.producer()
        {
            dependencies.include(self.structural_dependencies(
                *producer,
                origin,
                (module, frame),
                &mut query.work,
                receiver,
            )?);
        }
        for (expression, observation) in next {
            if dependencies.is_borrowed() {
                break;
            }
            dependencies.include(query.work.demand(
                ValueOrigin {
                    expression,
                    observation,
                    ..origin
                },
                origin,
                receiver,
            )?);
        }
        Ok(dependencies)
    }

    fn stored_captures(
        &self,
        module: &HirModule,
        origin: ValueOrigin,
        frame: &StorageFrame,
        evidence: &PreparedFunctionValueOriginEvidence,
        work: &mut CaptureWorklist,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
        let mut dependencies = BorrowDependencies::default();
        for capture in evidence.captures() {
            dependencies.include(self.stored_local_dependencies(
                module,
                origin,
                (capture.local(), origin.observation),
                frame,
                work,
                receiver,
            )?);
            if dependencies.is_borrowed() {
                break;
            }
        }
        Ok(dependencies)
    }

    fn stored_local_dependencies(
        &self,
        module: &HirModule,
        origin: ValueOrigin,
        (local, observation): (LocalId, Observation),
        frame: &StorageFrame,
        work: &mut CaptureWorklist,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
        receiver.charge(1)?;
        let ty = self
            .facts
            .locals()
            .get(&local)
            .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable { owner: local })?;
        let storage = if matches!(observation, Observation::Project(_)) {
            TypeStorage::Callable
        } else {
            self.inspect_value_storage(ty, &frame.projections, receiver)?
        };
        match storage {
            TypeStorage::Borrowed => Ok(BorrowDependencies::Borrowed),
            TypeStorage::Ordinary => Ok(BorrowDependencies::default()),
            TypeStorage::Callable => {
                let local_origins = self
                    .topology
                    .module(module.module_id())
                    .ok_or(FinalSemanticAnalysisError::InvalidOwner)?
                    .local_origins();
                if let Some(HirLocalValueOrigin::DirectInitializer(expression)) =
                    local_origins.origin(local)
                {
                    return work.demand(
                        ValueOrigin {
                            expression,
                            observation,
                            ..origin
                        },
                        origin,
                        receiver,
                    );
                }
                let parameter = self.parameter_demand(
                    module,
                    local,
                    ValueOrigin {
                        observation,
                        ..origin
                    },
                    work,
                    receiver,
                )?;
                Ok(BorrowDependencies::Parameters(
                    parameter.into_iter().collect(),
                ))
            }
        }
    }

    fn checked_storage_origin(
        &self,
        module: &HirModule,
        origin: ValueOrigin,
        frame: &StorageFrame,
        captures: &PendingCaptures,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<StorageOrigin, FinalSemanticAnalysisError> {
        let expression = origin.expression;
        let expressions = self.facts.expressions();
        let Some(fact) = self.storage_expression_fact(expression, frame) else {
            return Ok(StorageOrigin::Unavailable);
        };
        match self.origin_fact_storage(fact, origin.observation, &frame.projections, receiver)? {
            TypeStorage::Borrowed => return Ok(StorageOrigin::Borrowed),
            TypeStorage::Ordinary => return Ok(StorageOrigin::Unavailable),
            TypeStorage::Callable => {}
        }
        let mut progress = prepare_function_value_origin_query_with_pending_captures(
            Arc::clone(&self.topology),
            module,
            expression,
            Some(fact),
            Arc::clone(captures),
            PreparedFunctionValueOriginPurpose::Storage,
        );
        loop {
            match progress {
                Ok(PreparedFunctionValueOriginProgress::Ready(evidence)) => {
                    return Ok(StorageOrigin::Checked(evidence));
                }
                Ok(PreparedFunctionValueOriginProgress::Need(need)) => {
                    receiver.charge(1)?;
                    let requested = need.expression();
                    let Some(fact) = expressions.get(&requested) else {
                        return Ok(StorageOrigin::Unavailable);
                    };
                    match self.origin_fact_storage(
                        fact,
                        origin.observation,
                        &frame.projections,
                        receiver,
                    )? {
                        TypeStorage::Borrowed => return Ok(StorageOrigin::Borrowed),
                        TypeStorage::Ordinary => return Ok(StorageOrigin::Unavailable),
                        TypeStorage::Callable => {}
                    }
                    progress = need.resume(requested, fact, module);
                }
                Err(PreparedFunctionValueOriginQueryError::Composite) => {
                    return Ok(StorageOrigin::Unavailable);
                }
                Err(error) => {
                    return Err(self
                        .map_function_value_origin_query_error(expression, error)
                        .into_public(expression));
                }
            }
        }
    }

    fn storage_expression_fact(
        &self,
        expression: ExprId,
        frame: &StorageFrame,
    ) -> Option<&PreparedExpressionFact> {
        let fact = self.facts.expressions().get(&expression)?;
        if frame.owner.executes_implicit_body(expression) {
            return Some(fact.implicit_callable()?.body().expression());
        }
        Some(fact)
    }

    fn origin_fact_storage(
        &self,
        fact: &PreparedExpressionFact,
        observation: Observation,
        projections: &[BodyProjection],
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<TypeStorage, FinalSemanticAnalysisError> {
        if fact.implicit_callable().is_some() {
            return Ok(TypeStorage::Callable);
        }
        let storage = fact.value_type().map_or(Ok(TypeStorage::Ordinary), |ty| {
            if matches!(observation, Observation::Project(_)) && !matches!(ty, TypeKind::Never) {
                return Ok(TypeStorage::Callable);
            }
            self.inspect_value_storage(ty, projections, receiver)
        })?;
        if storage == TypeStorage::Callable
            && !matches!(
                fact,
                PreparedExpressionFact::Complete(_)
                    | PreparedExpressionFact::ProjectRecord(_)
                    | PreparedExpressionFact::ProjectField(_)
                    | PreparedExpressionFact::Variant(_)
            )
            && fact.try_carrier().is_none()
            && fact.implicit_parameter_owner().is_none()
        {
            return Ok(TypeStorage::Ordinary);
        }
        Ok(storage)
    }

    fn call_storage(
        &self,
        call: Invocation,
        frame: &StorageFrame,
        work: &CaptureWorklist,
        receiver: &mut ReceiverBorrowQuery<'_>,
        pending: &mut Vec<(ExprId, Observation)>,
    ) -> Result<CallStorage, FinalSemanticAnalysisError> {
        let site = call.site;
        let owner = site.expression();
        let ty = self
            .storage_expression_fact(owner, frame)
            .and_then(|fact| {
                fact.complete()
                    .and_then(crate::final_analysis::CheckedExpression::source_value_type)
                    .or_else(|| fact.value_type())
            })
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
        let graph = self
            .facts
            .prepared_calls()
            .map_err(|error| AnalyzerExpressionError::fact(error).into_public(owner))?;
        let stage = PreparedCallGraphIngress::new(graph)
            .value_delivery_at(site, ty)
            .map_err(|error| call_storage_error(site, error))?;
        graph
            .project_site_payload(
                site,
                |prefix| {
                    let selected = prefix.application().selected();
                    if matches!(stage, PreparedCallSiteDelivery::Value) {
                        let constructor = matches!(
                            selected.id(),
                            CallableCandidateId::EnumVariant(_)
                                | CallableCandidateId::Result(_)
                                | CallableCandidateId::Option(_)
                                | CallableCandidateId::RecordConstructor(_)
                        );
                        if !constructor {
                            return Ok(match selected.id() {
                                CallableCandidateId::Project(declaration)
                                    if declaration.owner()
                                        == CallableDeclarationOwner::Function =>
                                {
                                    CallStorage::SourceResult(declaration.clone())
                                }
                                _ => prefix
                                    .record()
                                    .function_value_origin()
                                    .map_or(CallStorage::NoWitness, |callee| {
                                        CallStorage::ValueResult(callee.callee())
                                    }),
                            });
                        }
                        if let Some((selection, remaining)) = work.selected_storage(call.result)? {
                            projected_constructor_sources(
                                prefix,
                                ty,
                                (selection, remaining),
                                work,
                                receiver,
                                pending,
                            )?;
                            return Ok(CallStorage::NoWitness);
                        }
                    }
                    if matches!(call.result, Observation::Invoke(_)) {
                        let selected = prefix.application().selected_shared();
                        return Ok(match selected.id() {
                            CallableCandidateId::Project(declaration)
                                if declaration.owner() == CallableDeclarationOwner::Function =>
                            {
                                CallStorage::SourcePrefix(Arc::clone(selected))
                            }
                            _ => CallStorage::NoWitness,
                        });
                    }
                    if call.result != Observation::Stored {
                        return Ok(CallStorage::NoWitness);
                    }
                    for (source, ty) in prefix.record().value_inputs() {
                        receiver.charge(1)?;
                        match self.inspect_value_storage(ty, &frame.projections, receiver)? {
                            TypeStorage::Borrowed => return Ok(CallStorage::Borrowed),
                            TypeStorage::Callable => pending.push((source, Observation::Stored)),
                            TypeStorage::Ordinary => {}
                        }
                    }
                    if let Some(callee) = prefix.record().function_value_origin() {
                        receiver.charge(1)?;
                        pending.push((callee.callee(), Observation::Stored));
                    }
                    Ok(CallStorage::NoWitness)
                },
                |_| Err(FinalSemanticAnalysisError::InvalidOwner),
            )
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?
    }

    fn source_function_values(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        declaration: &CallableDeclarationKey,
    ) -> Result<(ItemId, Vec<CallableResultSource>), FinalSemanticAnalysisError> {
        let view = self
            .topology
            .declaration(declaration)
            .map_err(|_| FinalSemanticAnalysisError::InvalidCallableOwner)?;
        let item = view.body().source_item();
        let module = self.module(view.module().module())?;
        let item_source = module
            .resolve_item(item)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let (HirCallableSourceOwner::Item, HirItemKind::Function(function)) =
            (view.body().source_owner(), item_source.kind())
        else {
            return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
        };
        let body = function.body();
        let HirFunctionBody::Block {
            statements, tail, ..
        } = body
        else {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
        };
        // A selected declaration can occur after its caller. Check it in
        // the caller's fact transaction before demanding its result sites.
        let expected = self.registered_callable_result(declaration)?;
        let yields = self
            .check_function_body_type(context, module, body, &expected)
            .map_err(|error| error.into_public(*tail))?;
        if yields != 0 {
            return Err(FinalSemanticAnalysisError::InvalidFunctionExecution { owner: item });
        }
        let values = self
            .checked_function_value_sources(module, (item, declaration), statements, *tail)
            .map_err(|error| error.into_public(*tail))?;
        Ok((item, values))
    }
}

fn projected_constructor_sources(
    prefix: &AnalyzerPreparedCallPrefix,
    ty: &TypeKind,
    (selection, remaining): (&StorageProjection, Observation),
    work: &CaptureWorklist,
    receiver: &mut ReceiverBorrowQuery<'_>,
    pending: &mut Vec<(ExprId, Observation)>,
) -> Result<(), FinalSemanticAnalysisError> {
    use crate::callable::ResultConstructorKind;
    let selected = prefix.application().selected();
    let carrier = match selected.id() {
        CallableCandidateId::Option(_) => Some(CarrierStorage::OptionItem),
        CallableCandidateId::Result(ResultConstructorKind::Ok) => Some(CarrierStorage::ResultOk),
        CallableCandidateId::Result(ResultConstructorKind::Err) => Some(CarrierStorage::ResultErr),
        CallableCandidateId::EnumVariant(case) => CarrierStorage::for_case(ty, case.case()),
        _ => None,
    };
    let (ordinal, passing, observation) = match (selected.id(), selection) {
        (_, StorageProjection::Carrier(expected)) if carrier == Some(*expected) => {
            for (source, _) in prefix.record().value_inputs() {
                receiver.charge(1)?;
                pending.push((source, remaining));
            }
            return Ok(());
        }
        (
            CallableCandidateId::RecordConstructor(_),
            StorageProjection::Field { nominal, ordinal },
        ) if matches!(ty, TypeKind::ProjectNominal(result) if result.declaration() == nominal) => {
            (*ordinal, None, remaining)
        }
        (CallableCandidateId::EnumVariant(case), StorageProjection::VariantPayload(expected))
            if case == expected =>
        {
            match work.selected_storage(remaining)? {
                Some((
                    StorageProjection::Element {
                        family: PositionalStorage::Tuple,
                        ordinal,
                    },
                    observation,
                )) => (
                    *ordinal,
                    Some(CallableParameterPassing::PositionalOnly),
                    observation,
                ),
                Some((
                    StorageProjection::VariantField {
                        case: owner,
                        ordinal,
                    },
                    observation,
                )) if owner == case => (
                    *ordinal,
                    Some(CallableParameterPassing::NamedOnly),
                    observation,
                ),
                None if remaining == Observation::Stored => {
                    for (source, _) in prefix.record().value_inputs() {
                        receiver.charge(1)?;
                        pending.push((source, remaining));
                    }
                    return Ok(());
                }
                _ => return Ok(()),
            }
        }
        _ => return Ok(()),
    };
    let group = prefix.application().completed_group();
    let formal = selected
        .schema()
        .group(group)
        .and_then(|group| group.parameters().get(ordinal as usize))
        .filter(|formal| passing.is_none_or(|passing| passing == formal.passing()))
        .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
    if let Some(source) =
        prefix.parameter_value_source(CallableParameterCoordinate::new(group, formal.index()))
    {
        receiver.charge(1)?;
        pending.push((source, observation));
    }
    Ok(())
}

fn call_storage_error(
    site: CheckedCallSite,
    error: crate::callable::CallConstraintInvariant,
) -> FinalSemanticAnalysisError {
    FinalSemanticAnalysisError::CallConstraintFailure(FinalCallConstraintFailure::new(
        site.expression(),
        CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error)),
    ))
}

fn storage_projection_error(
    error: TypeProjectionError<FinalSemanticAnalysisError>,
) -> FinalSemanticAnalysisError {
    match error {
        TypeProjectionError::Control(error) => error,
        TypeProjectionError::Instantiation(error) => {
            crate::final_analysis::NominalSchemaProjectionError::DeclarationProjection(error).into()
        }
    }
}
