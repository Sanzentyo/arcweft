//! Borrowed storage evidenced by checked function-value origins.

use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::Arc,
};

use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::{ExprId, ItemId, LocalId},
    item::{HirFunctionBody, HirItemKind, HirParameterKind},
    module::HirModule,
    pattern::{HirPatternBinding, HirPatternKind},
    project::{
        HirBindingSite, HirControlTransferTarget, HirLocalValueOrigin, HirLoopTargetFamily,
        HirReturnContext, HirSemanticBodyOwner,
    },
    source_index::HirCallableSourceOwner,
    symbol::{CallableDeclarationKey, CallableDeclarationOwner},
};

use super::{Analyzer, FinalSemanticAnalysisError, ReceiverBorrowQuery};
use crate::{
    callable::{
        CallableCandidateId, CallableParameterCoordinate, CheckedCallSite,
        PreparedCallGraphIngress, PreparedCallSiteDelivery, PreparedCaptureIdentityRow,
        PreparedFunctionValueOriginEvidence, PreparedFunctionValueOriginProducer,
        PreparedFunctionValueOriginProgress, PreparedFunctionValueOriginQueryError,
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
    types::{ArrayLength, ScopedType, TypeKind, TypeProjectionError},
};

#[path = "captures/worklist.rs"]
mod worklist;

use self::worklist::{
    ApplicationStorage, BorrowDependencies, CaptureWorklist, StorageFrame, TypeStorage, ValueOrigin,
};

type PendingCaptures = Arc<BTreeMap<ExprId, Box<[PreparedCaptureIdentityRow]>>>;

struct CaptureEvaluation {
    root: ExprId,
    captures: PendingCaptures,
    results: BTreeMap<CallableDeclarationKey, (ItemId, Rc<[ExprId]>)>,
    work: CaptureWorklist,
}

enum CallStorage {
    Borrowed,
    SourceResult(CallableDeclarationKey),
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
        let frame = query.work.frame(origin)?;
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
        let mut next = Vec::new();
        if let PreparedFunctionValueOriginProducer::Call(site) = evidence.producer() {
            match self.call_storage(*site, &frame.applications, receiver, &mut next)? {
                CallStorage::Borrowed => return Ok(BorrowDependencies::Borrowed),
                CallStorage::SourceResult(declaration) => {
                    return self.source_result_dependencies(
                        context,
                        origin,
                        &frame,
                        (*site, declaration),
                        query,
                        receiver,
                    );
                }
                CallStorage::NoWitness => {}
            }
        }
        let mut dependencies =
            self.capture_dependencies(module, &frame, &evidence, receiver, &mut next)?;
        if dependencies.is_borrowed() {
            return Ok(dependencies);
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
                    frame: origin.frame,
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

    fn source_result_dependencies(
        &mut self,
        context: &AnalyzerExpressionContext<'_>,
        origin: ValueOrigin,
        caller: &StorageFrame,
        (site, declaration): (CheckedCallSite, CallableDeclarationKey),
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
        let storage = self.application_storage(site, caller, receiver)?;
        let (item, values) = query
            .results
            .get(&declaration)
            .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
        let frame = query.work.intern(storage, *item, caller, receiver)?;
        let mut dependencies = BorrowDependencies::default();
        for &expression in values.iter() {
            dependencies.include(query.work.demand(
                ValueOrigin { expression, frame },
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
                site,
                origin,
                &parameters,
                &mut query.work,
                receiver,
            ),
        }
    }

    fn capture_dependencies(
        &self,
        module: &HirModule,
        frame: &StorageFrame,
        evidence: &PreparedFunctionValueOriginEvidence,
        receiver: &mut ReceiverBorrowQuery<'_>,
        pending: &mut Vec<ExprId>,
    ) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
        let mut parameters = BTreeSet::new();
        if let PreparedFunctionValueOriginProducer::Lexical { local } = evidence.producer()
            && let Some(coordinate) = self.frame_parameter(module, *local, frame)?
        {
            receiver.charge(1)?;
            parameters.insert(coordinate);
        }
        let local_origins = self
            .topology
            .module(module.module_id())
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?
            .local_origins();
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
                    if let Some(HirLocalValueOrigin::DirectInitializer(initializer)) =
                        local_origins.origin(local)
                    {
                        pending.push(initializer);
                    } else if let Some(coordinate) = self.frame_parameter(module, local, frame)? {
                        parameters.insert(coordinate);
                    }
                }
                TypeStorage::Ordinary => {}
            }
        }
        Ok(BorrowDependencies::Parameters(parameters))
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

    /// Equal storage shapes agree on the type observations made by this
    /// query. Value provenance stays symbolic in each result dependency.
    fn application_storage(
        &self,
        site: CheckedCallSite,
        caller: &StorageFrame,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<ApplicationStorage, FinalSemanticAnalysisError> {
        let graph = self
            .facts
            .prepared_calls()
            .map_err(FinalSemanticAnalysisError::from)?;
        graph
            .project_site_payload(
                site,
                |prefix| {
                    let application = prefix.application();
                    let type_arguments = application.body_type_arguments();
                    let const_arguments = application.body_const_arguments();
                    let argument_count = type_arguments
                        .len()
                        .checked_add(const_arguments.len())
                        .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
                    receiver.charge(
                        u64::try_from(argument_count)
                            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                    )?;
                    let types = type_arguments
                        .map(|ty| {
                            self.inspect_value_storage(ty.value(), &caller.applications, receiver)
                        })
                        .collect::<Result<Box<[_]>, _>>()?;
                    let zero_lengths = const_arguments
                        .map(|length| {
                            self.project_zero_length(length.value(), &caller.applications, receiver)
                        })
                        .collect::<Result<Box<[_]>, _>>()?;
                    Ok(ApplicationStorage {
                        site,
                        types,
                        zero_lengths,
                    })
                },
                |_| Err(FinalSemanticAnalysisError::InvalidOwner),
            )
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?
    }

    fn substitute_call_parameters(
        &self,
        site: CheckedCallSite,
        origin: ValueOrigin,
        parameters: &BTreeSet<CallableParameterCoordinate>,
        work: &mut CaptureWorklist,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
        if parameters.is_empty() {
            return Ok(BorrowDependencies::default());
        }
        let graph = self
            .facts
            .prepared_calls()
            .map_err(FinalSemanticAnalysisError::from)?;
        graph
            .project_site_payload(
                site,
                |prefix| {
                    let enclosing = prefix.record().enclosing_callable();
                    let mut input = prefix;
                    let mut dependencies = BorrowDependencies::default();
                    loop {
                        receiver.charge(1)?;
                        if input.record().enclosing_callable() == enclosing {
                            let group = input.application().completed_group();
                            for &coordinate in parameters {
                                receiver.charge(1)?;
                                if coordinate.group() == group
                                    && let Some(expression) =
                                        input.parameter_value_source(coordinate)
                                {
                                    dependencies.include(work.demand(
                                        ValueOrigin {
                                            expression,
                                            frame: origin.frame,
                                        },
                                        origin,
                                        receiver,
                                    )?);
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
                            .map_err(|error| call_storage_error(site, error))?;
                    }
                    Ok(dependencies)
                },
                |_| Err(FinalSemanticAnalysisError::InvalidOwner),
            )
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?
    }

    fn project_zero_length(
        &self,
        length: &ArrayLength,
        applications: &[CheckedCallSite],
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<bool, FinalSemanticAnalysisError> {
        receiver.charge(1)?;
        if let ArrayLength::Const(length) = length {
            return Ok(*length == 0);
        }
        let graph = self
            .facts
            .prepared_calls()
            .map_err(FinalSemanticAnalysisError::from)?;
        let mut projected = length.clone();
        for site in applications.iter().rev().copied() {
            projected = graph
                .project_site_payload(
                    site,
                    |prefix| {
                        prefix
                            .application()
                            .project_body_length(&projected, receiver)
                            .map_err(storage_projection_error)
                    },
                    |_| Err(FinalSemanticAnalysisError::InvalidOwner),
                )
                .ok_or(FinalSemanticAnalysisError::InvalidOwner)??;
        }
        Ok(matches!(projected, ArrayLength::Const(0)))
    }

    fn frame_parameter(
        &self,
        module: &HirModule,
        local: LocalId,
        frame: &StorageFrame,
    ) -> Result<Option<CallableParameterCoordinate>, FinalSemanticAnalysisError> {
        let Some(owner) = frame.item else {
            return Ok(None);
        };
        Ok(self
            .whole_parameter_binding(module, local)?
            .and_then(|(item, coordinate)| (item == owner).then_some(coordinate)))
    }

    fn whole_parameter_binding(
        &self,
        module: &HirModule,
        local: LocalId,
    ) -> Result<Option<(ItemId, CallableParameterCoordinate)>, FinalSemanticAnalysisError> {
        let local_origins = self
            .topology
            .module(module.module_id())
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?
            .local_origins();
        let Some(binding) = local_origins.binding(local) else {
            return Err(FinalSemanticAnalysisError::InvalidOwner);
        };
        let HirBindingSite::DeclarationParameter {
            item,
            owner: HirCallableSourceOwner::Item,
            group,
            parameter,
        } = binding.site()
        else {
            return Ok(None);
        };
        if binding.is_mutable_binding() {
            return Ok(None);
        }
        let item_source = module
            .resolve_item(item)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let HirItemKind::Function(function) = item_source.kind() else {
            return Ok(None);
        };
        let group = crate::callable::CallableGroupIndex::try_from_usize(group as usize)
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        let parameter = crate::callable::CallableParameterIndex::try_from_usize(parameter as usize)
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        let parameter_source = function
            .parameter_groups()
            .get(group.get())
            .and_then(|group| group.parameters().get(parameter.get()))
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        let pattern = module
            .resolve_pattern(parameter_source.pattern())
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if parameter_source.kind() == HirParameterKind::RestPositional
            || !matches!(pattern.kind(),
                HirPatternKind::Binding(HirPatternBinding::Bound { local: bound, .. })
                | HirPatternKind::TypedBinding { binding: HirPatternBinding::Bound { local: bound, .. }, .. }
                | HirPatternKind::WholeBinding { binding: HirPatternBinding::Bound { local: bound, .. }, .. }
                if *bound == local)
        {
            return Ok(None);
        }
        Ok(Some((
            item,
            CallableParameterCoordinate::new(group, parameter),
        )))
    }

    fn call_storage(
        &self,
        site: CheckedCallSite,
        applications: &[CheckedCallSite],
        receiver: &mut ReceiverBorrowQuery<'_>,
        pending: &mut Vec<ExprId>,
    ) -> Result<CallStorage, FinalSemanticAnalysisError> {
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
                            _ => CallStorage::NoWitness,
                        });
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
