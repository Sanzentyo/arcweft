//! Borrowed storage evidenced by checked function-value origins.

use std::{collections::BTreeSet, rc::Rc, sync::Arc};

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
    types::{ScopedType, TypeKind, TypeProjectionError},
};

type PendingCaptures = Arc<std::collections::BTreeMap<ExprId, Box<[PreparedCaptureIdentityRow]>>>;

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
struct ValueOrigin {
    expression: ExprId,
    applications: Rc<[CheckedCallSite]>,
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
        let mut captures = self
            .pending_capture_identity_rows()
            .map_err(|error| error.into_public(root))?;
        let mut pending = vec![ValueOrigin {
            expression: root,
            applications: Rc::from([]),
        }];
        let mut visited = BTreeSet::new();
        let mut prepared_bodies = BTreeSet::new();
        while let Some(origin) = pending.pop() {
            receiver.charge(1)?;
            if !visited.insert(origin.clone()) {
                continue;
            }
            let module = self.module(origin.expression.module())?;
            let Some(evidence) =
                self.checked_function_origin(module, &origin, &captures, receiver)?
            else {
                continue;
            };
            let mut next = Vec::new();
            if let PreparedFunctionValueOriginProducer::Call(site) = evidence.producer() {
                match self.call_storage(*site, &origin.applications, receiver, &mut next)? {
                    CallStorage::Borrowed => return Ok(true),
                    CallStorage::SourceResult(declaration)
                        if !origin.applications.contains(site) =>
                    {
                        // Expand a checked site once per path. Other reachable
                        // exits remain in the worklist when a call is recursive.
                        receiver.charge(
                            u64::try_from(origin.applications.len())
                                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?
                                + 1,
                        )?;
                        let mut sites = origin.applications.to_vec();
                        sites.push(*site);
                        let sites: Rc<[_]> = Rc::from(sites);
                        let (values, prepared) = self.source_function_values(
                            context,
                            &declaration,
                            &mut prepared_bodies,
                        )?;
                        if prepared {
                            captures = self
                                .pending_capture_identity_rows()
                                .map_err(|error| error.into_public(root))?;
                        }
                        receiver.charge(
                            u64::try_from(values.len())
                                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                        )?;
                        pending.extend(values.into_iter().map(|expression| ValueOrigin {
                            expression,
                            applications: Rc::clone(&sites),
                        }));
                    }
                    CallStorage::SourceResult(_) | CallStorage::NoWitness => {}
                }
            }
            if self.capture_value_origins(module, &origin, &evidence, receiver, &mut pending)? {
                return Ok(true);
            }
            if let PreparedFunctionValueOriginProducer::IndependentExpression { producer } =
                evidence.producer()
            {
                self.append_structural_function_results(module, *producer, receiver, &mut next)?;
            }
            pending.extend(next.into_iter().map(|expression| ValueOrigin {
                expression,
                applications: Rc::clone(&origin.applications),
            }));
        }
        Ok(false)
    }

    fn capture_value_origins(
        &self,
        module: &HirModule,
        origin: &ValueOrigin,
        evidence: &PreparedFunctionValueOriginEvidence,
        receiver: &mut ReceiverBorrowQuery<'_>,
        pending: &mut Vec<ValueOrigin>,
    ) -> Result<bool, FinalSemanticAnalysisError> {
        if let PreparedFunctionValueOriginProducer::Lexical { local } = evidence.producer()
            && let Some(source) = self.parameter_value_origin(module, *local, origin, receiver)?
        {
            pending.push(source);
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
            let (borrowed, callable) =
                self.inspect_value_storage(ty, &origin.applications, receiver)?;
            if borrowed {
                return Ok(true);
            }
            if callable {
                if let Some(HirLocalValueOrigin::DirectInitializer(initializer)) =
                    local_origins.origin(local)
                {
                    pending.push(ValueOrigin {
                        expression: initializer,
                        applications: Rc::clone(&origin.applications),
                    });
                } else if let Some(source) =
                    self.parameter_value_origin(module, local, origin, receiver)?
                {
                    pending.push(source);
                }
            }
        }
        Ok(false)
    }

    fn checked_function_origin(
        &self,
        module: &HirModule,
        origin: &ValueOrigin,
        captures: &PendingCaptures,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<Option<PreparedFunctionValueOriginEvidence>, FinalSemanticAnalysisError> {
        let expression = origin.expression;
        let expressions = self.facts.expressions();
        let Some(fact) = expressions.get(&expression) else {
            return Ok(None);
        };
        if !self.has_function_origin_fact(fact, &origin.applications, receiver)? {
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
                    if !self.has_function_origin_fact(fact, &origin.applications, receiver)? {
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
    ) -> Result<(bool, bool), FinalSemanticAnalysisError> {
        let projected = self.project_value_storage(ty, applications, receiver)?;
        let ty = projected.as_ref().map_or(ty, |term| term.view().value());
        Ok((
            receiver.contains_borrow(ty, &self.types)?,
            matches!(ty, TypeKind::Function { .. }),
        ))
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
                projected = Some(graph.project_site_payload(site, |prefix| {
                    prefix.application().project_body_storage(input, receiver)
                        .map_err(|error| match error {
                            TypeProjectionError::Control(error) => error,
                            TypeProjectionError::Instantiation(error) =>
                                crate::final_analysis::NominalSchemaProjectionError::DeclarationProjection(error).into(),
                        })
                }, |_| Err(FinalSemanticAnalysisError::InvalidOwner))
                    .ok_or(FinalSemanticAnalysisError::InvalidOwner)??);
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

    /// Joins a whole parameter binding to the selected argument in its group.
    /// The supplying expression is evaluated in the caller's instance.
    fn parameter_value_origin(
        &self,
        module: &HirModule,
        local: LocalId,
        origin: &ValueOrigin,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<Option<ValueOrigin>, FinalSemanticAnalysisError> {
        let Some((site, caller)) = origin.applications.split_last() else {
            return Ok(None);
        };
        let Some((item, coordinate)) = self.whole_parameter_binding(module, local)? else {
            return Ok(None);
        };
        let graph = self
            .facts
            .prepared_calls()
            .map_err(FinalSemanticAnalysisError::from)?;
        let expression = graph
            .project_site_payload(
                *site,
                |prefix| {
                    let CallableCandidateId::Project(declaration) =
                        prefix.application().selected().id()
                    else {
                        return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
                    };
                    let selected = self
                        .topology
                        .declaration(declaration)
                        .map_err(|_| FinalSemanticAnalysisError::InvalidCallableOwner)?;
                    if selected.body().source_item() != item {
                        return Ok(None);
                    }
                    let enclosing = prefix.record().enclosing_callable();
                    let mut input = prefix;
                    while input.application().completed_group() != coordinate.group() {
                        receiver.charge(1)?;
                        let Some(previous) = input.application().selected().prepared_continuation()
                        else {
                            return Err(FinalSemanticAnalysisError::InvalidOwner);
                        };
                        input = graph.continuation_prefix(previous).map_err(|error| {
                            FinalSemanticAnalysisError::CallConstraintFailure(
                                FinalCallConstraintFailure::new(
                                    site.expression(),
                                    CallAnalysisFailure::Invariant(
                                        CallAnalysisInvariant::Constraint(error),
                                    ),
                                ),
                            )
                        })?;
                    }
                    if input.record().enclosing_callable() != enclosing {
                        return Ok(None);
                    }
                    Ok(input.parameter_value_source(coordinate))
                },
                |_| Err(FinalSemanticAnalysisError::InvalidOwner),
            )
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)??;
        let Some(expression) = expression else {
            return Ok(None);
        };
        receiver.charge(
            u64::try_from(caller.len())
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?
                + 1,
        )?;
        Ok(Some(ValueOrigin {
            expression,
            applications: Rc::from(caller),
        }))
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
        let classify = |error| {
            FinalSemanticAnalysisError::CallConstraintFailure(FinalCallConstraintFailure::new(
                owner,
                CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(error)),
            ))
        };
        let stage = PreparedCallGraphIngress::new(graph)
            .value_delivery_at(site, ty)
            .map_err(classify)?;
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
                        let (borrowed, callable) =
                            self.inspect_value_storage(ty, applications, receiver)?;
                        if borrowed {
                            return Ok(CallStorage::Borrowed);
                        }
                        if callable {
                            pending.push(source);
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
        prepared: &mut BTreeSet<ItemId>,
    ) -> Result<(Vec<ExprId>, bool), FinalSemanticAnalysisError> {
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
        let newly_prepared = prepared.insert(item);
        if newly_prepared {
            // A selected declaration can occur after its caller. Use the
            // caller's fact transaction while checking the declaration body.
            let expected = self.registered_callable_result(declaration)?;
            let yields = self
                .check_function_body_type(context, module, body, &expected)
                .map_err(|error| error.into_public(*tail))?;
            if yields != 0 {
                return Err(FinalSemanticAnalysisError::InvalidFunctionExecution { owner: item });
            }
        }
        let values = self.checked_body_value_sources(module, statements, *tail,
            |target| matches!(target, HirControlTransferTarget::Return { context: HirReturnContext::Item(owner) } if *owner == item),
            true,
        ).map_err(|error| error.into_public(*tail))?;
        Ok((values, newly_prepared))
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
