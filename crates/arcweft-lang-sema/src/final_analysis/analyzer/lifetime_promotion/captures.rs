//! Borrowed storage evidenced by checked function-value origins.

use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::Arc,
};

use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::{ExprId, ItemId},
    item::{HirFunctionBody, HirItemKind},
    module::HirModule,
    project::{
        HirControlTransferTarget, HirLocalValueOrigin, HirLoopTargetFamily, HirReturnContext,
        HirSemanticBodyOwner,
    },
    source_index::HirCallableSourceOwner,
    symbol::{CallableDeclarationKey, CallableDeclarationOwner},
};

use super::{Analyzer, FinalSemanticAnalysisError, ReceiverBorrowQuery};
use crate::{
    callable::{
        CallableCandidateId, CheckedCallSite, PreparedCallGraphIngress, PreparedCallSiteDelivery,
        PreparedCaptureIdentityRow, PreparedFunctionValueOriginEvidence,
        PreparedFunctionValueOriginProducer, PreparedFunctionValueOriginProgress,
        PreparedFunctionValueOriginQueryError,
        prepare_function_value_origin_query_with_pending_captures,
    },
    final_analysis::{
        CheckedMatchArmFact, FinalCallConstraintFailure, PreparedExpressionFact,
        PreparedOwnerBoundResolution,
        analyzer::{
            AnalyzerExpressionContext, CallAnalysisFailure, calls::CallAnalysisInvariant,
            expression_error::AnalyzerExpressionError,
        },
    },
    types::{ScopedType, TypeKind, TypeProjectionError},
};

#[path = "captures/invocation.rs"]
mod invocation;

#[path = "captures/worklist.rs"]
mod worklist;

use self::worklist::{
    BorrowDependencies, CaptureWorklist, Invocation, Observation, StorageFrame, TypeStorage,
    ValueOrigin,
};

type PendingCaptures = Arc<BTreeMap<ExprId, Box<[PreparedCaptureIdentityRow]>>>;

struct CaptureEvaluation {
    root: ExprId,
    captures: PendingCaptures,
    results: BTreeMap<CallableDeclarationKey, (ItemId, Rc<[ExprId]>)>,
    closure_results: BTreeMap<ExprId, Rc<[ExprId]>>,
    work: CaptureWorklist,
}

enum CallStorage {
    Borrowed,
    SourceResult(CallableDeclarationKey),
    ValueResult(ExprId),
    NoWitness,
}

impl Analyzer<'_, '_, '_> {
    /// Follows only the selected value origins and storage evidence. Executable
    /// promotion still requires a complete ownership and lifetime proof.
    pub(super) fn function_value_has_borrowed_captures(
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
            closure_results: BTreeMap::new(),
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
        let frame = query.work.frame(origin.frame)?;
        let module = self.module(origin.expression.module())?;
        let Some(evidence) = self.checked_function_origin(
            module,
            origin.expression,
            &frame,
            &query.captures,
            receiver,
        )?
        else {
            return Ok(BorrowDependencies::default());
        };
        let producer = match evidence.producer() {
            PreparedFunctionValueOriginProducer::Lexical { local } => {
                let mut parameters = BTreeSet::new();
                if let Some(parameter) =
                    self.parameter_demand(module, *local, origin, &query.work, receiver)?
                {
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
        let producer_scope = module
            .resolve_expr(producer)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
            .scope();
        let producer_frame =
            self.lexical_frame(module, producer_scope, origin.frame, &query.work, receiver)?;
        if producer_frame != origin.frame {
            return query.work.demand(
                ValueOrigin {
                    expression: producer,
                    frame: producer_frame,
                    ..origin
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
            match self.call_storage(call, &frame.applications, receiver, &mut next)? {
                CallStorage::Borrowed => return Ok(BorrowDependencies::Borrowed),
                CallStorage::SourceResult(declaration) => {
                    return self.source_result_dependencies(
                        context,
                        origin,
                        (call, None, declaration),
                        query,
                        receiver,
                    );
                }
                CallStorage::ValueResult(callee) => {
                    let id = query.work.intern_invocation(call, receiver)?;
                    return Ok(query
                        .work
                        .demand(
                            ValueOrigin {
                                expression: callee,
                                observation: Observation::Invoke(id),
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
            Observation::Invoke(id) => {
                if let PreparedFunctionValueOriginProducer::IndependentExpression { producer } =
                    evidence.producer()
                    && let Some(dependencies) = self.invoke_producer(
                        context,
                        module,
                        origin,
                        (*producer, id),
                        query,
                        receiver,
                    )?
                {
                    return Ok(dependencies);
                }
            }
        }
        if let PreparedFunctionValueOriginProducer::IndependentExpression { producer } =
            evidence.producer()
        {
            self.append_structural_function_results(module, *producer, receiver, &mut next)?;
        }
        for expression in next {
            dependencies.include(query.work.demand(
                ValueOrigin {
                    expression,
                    ..origin
                },
                origin,
                receiver,
            )?);
            if dependencies.is_borrowed() {
                break;
            }
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
        let local_origins = self
            .topology
            .module(module.module_id())
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?
            .local_origins();
        let mut parameters = BTreeSet::new();
        let mut dependencies = BorrowDependencies::default();
        for capture in evidence.captures() {
            receiver.charge(1)?;
            let local = capture.local();
            let ty = self
                .facts
                .locals()
                .get(&local)
                .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable { owner: local })?;
            match self.inspect_value_storage(ty, &frame.applications, receiver)? {
                TypeStorage::Borrowed => return Ok(BorrowDependencies::Borrowed),
                TypeStorage::Callable => {
                    if let Some(HirLocalValueOrigin::DirectInitializer(expression)) =
                        local_origins.origin(local)
                    {
                        dependencies.include(work.demand(
                            ValueOrigin {
                                expression,
                                ..origin
                            },
                            origin,
                            receiver,
                        )?);
                        if dependencies.is_borrowed() {
                            return Ok(dependencies);
                        }
                    } else if let Some(parameter) =
                        self.parameter_demand(module, local, origin, work, receiver)?
                    {
                        parameters.insert(parameter);
                    }
                }
                TypeStorage::Ordinary => {}
            }
        }
        dependencies.include(BorrowDependencies::Parameters(parameters));
        Ok(dependencies)
    }

    fn checked_function_origin(
        &self,
        module: &HirModule,
        expression: ExprId,
        frame: &StorageFrame,
        captures: &PendingCaptures,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<Option<PreparedFunctionValueOriginEvidence>, FinalSemanticAnalysisError> {
        let expressions = self.facts.expressions();
        let Some(fact) = expressions.get(&expression) else {
            return Ok(None);
        };
        if !self.has_function_origin_fact(fact, &frame.applications, receiver)? {
            return Ok(None);
        }
        let mut progress = prepare_function_value_origin_query_with_pending_captures(
            Arc::clone(&self.topology),
            module,
            expression,
            expressions,
            Arc::clone(captures),
        );
        loop {
            match progress {
                Ok(PreparedFunctionValueOriginProgress::Ready(evidence)) => {
                    return Ok(Some(evidence));
                }
                Ok(PreparedFunctionValueOriginProgress::Need(need)) => {
                    receiver.charge(1)?;
                    let requested = need.expression();
                    let Some(fact) = expressions.get(&requested) else {
                        return Ok(None);
                    };
                    if !self.has_function_origin_fact(fact, &frame.applications, receiver)? {
                        return Ok(None);
                    }
                    progress = need.resume(requested, fact, module);
                }
                Err(PreparedFunctionValueOriginQueryError::Composite) => return Ok(None),
                Err(error) => {
                    return Err(self
                        .map_function_value_origin_query_error(expression, error)
                        .into_public(expression));
                }
            }
        }
    }

    fn inspect_value_storage(
        &self,
        ty: &TypeKind,
        applications: &[CheckedCallSite],
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<TypeStorage, FinalSemanticAnalysisError> {
        let projected = self.project_value_storage(ty, applications, receiver)?;
        let ty = projected.as_ref().map_or(ty, |term| term.view().value());
        if receiver.contains_borrow(ty, &self.types)? {
            Ok(TypeStorage::Borrowed)
        } else if matches!(ty, TypeKind::Function { .. }) {
            Ok(TypeStorage::Callable)
        } else {
            Ok(TypeStorage::Ordinary)
        }
    }

    fn project_value_storage(
        &self,
        ty: &TypeKind,
        applications: &[CheckedCallSite],
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<Option<ScopedType>, FinalSemanticAnalysisError> {
        let mut projected: Option<ScopedType> = None;
        if !applications.is_empty() {
            let graph = self
                .facts
                .prepared_calls()
                .map_err(FinalSemanticAnalysisError::from)?;
            // The innermost declaration owns the captured type template.
            // Its replacements can name parameters of an enclosing caller.
            for site in applications.iter().rev().copied() {
                receiver.charge(1)?;
                let input = projected.as_ref().map_or(ty, |term| term.view().value());
                projected = Some(
                    graph
                        .project_site_payload(
                            site,
                            |prefix| {
                                prefix
                                    .application()
                                    .project_body_storage(input, receiver)
                                    .map_err(storage_projection_error)
                            },
                            |_| Err(FinalSemanticAnalysisError::InvalidOwner),
                        )
                        .ok_or(FinalSemanticAnalysisError::InvalidOwner)??,
                );
            }
        }
        Ok(projected)
    }

    fn has_function_origin_fact(
        &self,
        fact: &PreparedExpressionFact,
        applications: &[CheckedCallSite],
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<bool, FinalSemanticAnalysisError> {
        if let PreparedExpressionFact::OwnerBound(prepared) = fact {
            return Ok(matches!(
                prepared.resolution(),
                PreparedOwnerBoundResolution::ImplicitCallable(_)
            ));
        }
        match fact.complete().and_then(|checked| checked.value_type()) {
            Some(TypeKind::Function { .. }) => Ok(true),
            Some(ty @ TypeKind::GenericParam(_)) => Ok(self
                .project_value_storage(ty, applications, receiver)?
                .is_some_and(|term| matches!(term.view().value(), TypeKind::Function { .. }))),
            _ => Ok(false),
        }
    }

    fn call_storage(
        &self,
        call: Invocation,
        applications: &[CheckedCallSite],
        receiver: &mut ReceiverBorrowQuery<'_>,
        pending: &mut Vec<ExprId>,
    ) -> Result<CallStorage, FinalSemanticAnalysisError> {
        let site = call.site;
        let owner = site.expression();
        let ty = self
            .facts
            .expressions()
            .get(&owner)
            .and_then(PreparedExpressionFact::value_type)
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
                    if matches!(stage, PreparedCallSiteDelivery::Value) {
                        return Ok(match prefix.application().selected().id() {
                            CallableCandidateId::Project(declaration)
                                if declaration.owner() == CallableDeclarationOwner::Function =>
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
                    if call.result != Observation::Stored {
                        return Ok(CallStorage::NoWitness);
                    }
                    for (source, ty) in prefix.record().value_inputs() {
                        receiver.charge(1)?;
                        match self.inspect_value_storage(ty, applications, receiver)? {
                            TypeStorage::Borrowed => return Ok(CallStorage::Borrowed),
                            TypeStorage::Callable => pending.push(source),
                            TypeStorage::Ordinary => {}
                        }
                    }
                    if let Some(callee) = prefix.record().function_value_origin() {
                        receiver.charge(1)?;
                        pending.push(callee.callee());
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
    ) -> Result<(ItemId, Vec<ExprId>), FinalSemanticAnalysisError> {
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
        let values = self.checked_body_value_sources(module, statements, *tail,
            |target| matches!(target, HirControlTransferTarget::Return { context: HirReturnContext::Item(owner) } if *owner == item),
            true,
        ).map_err(|error| error.into_public(*tail))?;
        Ok((item, values))
    }

    fn append_structural_function_results(
        &self,
        module: &HirModule,
        producer: ExprId,
        receiver: &mut ReceiverBorrowQuery<'_>,
        pending: &mut Vec<ExprId>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let kind = module
            .resolve_expr(producer)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
            .kind();
        match kind {
            HirExprKind::Block(block) => pending.push(block.tail()),
            HirExprKind::NamedBlock(block) => pending.push(block.tail()),
            HirExprKind::If(branches) => {
                receiver.charge(2)?;
                pending.push(branches.else_branch());
                pending.push(branches.then_branch());
            }
            HirExprKind::IfLet(branches) => {
                receiver.charge(2)?;
                pending.push(branches.else_branch());
                pending.push(branches.then_branch());
            }
            HirExprKind::Match(_) => {
                let matched = self
                    .facts
                    .expressions()
                    .get(&producer)
                    .and_then(PreparedExpressionFact::complete)
                    .and_then(crate::final_analysis::CheckedExpression::match_fact)
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                receiver.charge(
                    u64::try_from(matched.arms().len())
                        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                )?;
                pending.extend(matched.arms().iter().rev().map(CheckedMatchArmFact::value));
            }
            HirExprKind::Loop(body) => {
                let values = self
                    .checked_body_value_sources(
                        module,
                        body.statements(),
                        body.tail(),
                        |target| {
                            matches!(target, HirControlTransferTarget::Loop {
                        family: HirLoopTargetFamily::LoopExpression, body_owner, ..
                    } if *body_owner == HirSemanticBodyOwner::direct_expression(producer))
                        },
                        false,
                    )
                    .map_err(|error| error.into_public(producer))?;
                receiver.charge(
                    u64::try_from(values.len())
                        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                )?;
                pending.extend(values);
            }
            _ => {}
        }
        Ok(())
    }
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
