//! Shared resolver dispatch, candidate materialization, and exact work charging.

use super::{
    AgentIntrinsicSignatureId, Arc, BuiltinCallableId, CallResolverRequest, CallableCandidateId,
    CallableDeclarationKey, CallableGroupIndex, CallableInstantiation, CallableLookupKey,
    CallableName, CallableParameterIndex, CallablePath, CallableRecord, CallableSignatureSchema,
    CapacityMethodId, CollectionMethodId, DataLastCallableId, DomainMethodId, DropCallableId,
    EnvironmentCallableOwner, EquivalentCallableSource, EvaluatedReceiver, FxCallableSignatureId,
    FxResolution, HirCallArgument, HirExprKind, HirModule, IntegerMethodId, LanguageCallableFamily,
    NonCallableSource, NonEmptyResolvedCandidates, OptionConstructorKind, Ordering,
    PreparedCallCallee, PreparedFreeCallScope, PresentationCallableId, PresentationHandleMethodId,
    ProjectCallablePath, ProjectNameBinding, PromotionCallableId, ReceiverMethodKey,
    ResolveCallError, ResolveCallOutcome, ResolvedAssociatedTypeReceiver, ResolvedCallTarget,
    ResolvedCallable, ResolvedFunctionValue, ResolvedFunctionValueSeed, ResolvedNonCallableTarget,
    ResultConstructorKind, SignatureOrigin, StageMethodId, TypeKind, TypeReceiverInstantiation,
    TypedEnvironmentMethodCandidate, UnknownCallKind, UnknownCallTarget, call_shape_is_viable,
};

pub(crate) fn resolve_call_target(mut request: CallResolverRequest<'_>) -> ResolveCallOutcome {
    if let Err(error) = request.work.record_resolver_invocation() {
        return ResolveCallOutcome::Rejected(error.into());
    }
    if let Err(error) = check_query_step(&mut request) {
        return ResolveCallOutcome::Rejected(error);
    }
    let arguments = request.call().arguments();
    match request.callee.clone() {
        PreparedCallCallee::Free {
            path,
            project,
            scope,
            ..
        } => {
            let path = path.clone();
            match resolve_free_call(&mut request, &path, project, scope) {
                Ok(Some(target)) => ResolveCallOutcome::Resolved(target),
                Ok(None) => ResolveCallOutcome::Missing(UnknownCallTarget::new(
                    UnknownCallKind::Free,
                    Some(path),
                    None,
                    None,
                )),
                Err(error) => ResolveCallOutcome::Rejected(error),
            }
        }
        PreparedCallCallee::Selected {
            receiver_expression,
            receiver_type,
            method,
        } => {
            let receiver_type = receiver_type.clone();
            let method = method.clone();
            let receiver = EvaluatedReceiver::new(receiver_expression, &receiver_type);
            match resolve_selected_call(&mut request, receiver, &method, arguments) {
                Ok(Some(target)) => ResolveCallOutcome::Resolved(target),
                Ok(None) => ResolveCallOutcome::Missing(UnknownCallTarget::new(
                    UnknownCallKind::Method,
                    None,
                    Some(receiver_type),
                    Some(method),
                )),
                Err(error) => ResolveCallOutcome::Rejected(error),
            }
        }
        PreparedCallCallee::AssociatedType { receiver, member } => {
            let receiver_type = receiver.ty().clone();
            let member = member.clone();
            match resolve_associated_type_call(&mut request, receiver, &member, arguments) {
                Ok(Some(target)) => ResolveCallOutcome::Resolved(target),
                Ok(None) => ResolveCallOutcome::Missing(UnknownCallTarget::new(
                    UnknownCallKind::AssociatedType,
                    None,
                    Some(receiver_type),
                    Some(member),
                )),
                Err(error) => ResolveCallOutcome::Rejected(error),
            }
        }
        PreparedCallCallee::FunctionValue { value } => {
            match resolve_function_value(value, &mut request) {
                Ok(target) => ResolveCallOutcome::Resolved(target),
                Err(error) => ResolveCallOutcome::Rejected(error),
            }
        }
        PreparedCallCallee::NonCallableValue { ty, .. } => {
            ResolveCallOutcome::Resolved(ResolvedCallTarget::NonCallable(
                ResolvedNonCallableTarget::new(NonCallableSource::EvaluatedExpression, ty.clone()),
            ))
        }
    }
}

fn resolve_associated_type_call(
    request: &mut CallResolverRequest<'_>,
    receiver: ResolvedAssociatedTypeReceiver<'_>,
    member: &CallableName,
    arguments: &[HirCallArgument],
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    let receiver_type = receiver.ty();
    if let Some(target) = resolve_associated_environment_method(request, receiver, member)? {
        return Ok(Some(target));
    }

    check_query_step(request)?;
    if let Some(id) = CapacityMethodId::resolve_associated(receiver_type, member, arguments.len())
        .map_err(|_| ResolveCallError::InvalidResolvedCallable)?
    {
        let schema = id.signature_schema();
        let target = resolved_language_method(
            request,
            CallableCandidateId::CapacityMethod(id),
            LanguageCallableFamily::CapacityMethod,
            schema,
            CallableInstantiation::TypeReceiver {
                receiver: TypeReceiverInstantiation::from_resolved(receiver),
            },
        )?;
        return Ok(Some(target));
    }

    resolve_checked_method(
        request,
        receiver_type,
        member,
        CallableInstantiation::TypeReceiver {
            receiver: TypeReceiverInstantiation::from_resolved(receiver),
        },
    )
}

fn resolve_associated_environment_method(
    request: &mut CallResolverRequest<'_>,
    receiver: ResolvedAssociatedTypeReceiver<'_>,
    member: &CallableName,
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    let receiver_type = receiver.ty();
    check_query_step(request)?;
    let Some(seeds) = request
        .authority
        .typed_environment_method(receiver_type, member)?
    else {
        return Ok(None);
    };
    let mut candidates = Vec::with_capacity(seeds.len());
    for seed in seeds {
        candidates.push(materialize_typed_environment_method(
            &seed, &receiver, request,
        )?);
    }
    NonEmptyResolvedCandidates::try_new(candidates, request.limits)
        .map(ResolvedCallTarget::Candidates)
        .map(Some)
}

fn materialize_typed_environment_method(
    seed: &TypedEnvironmentMethodCandidate<'_>,
    receiver: &ResolvedAssociatedTypeReceiver<'_>,
    request: &mut CallResolverRequest<'_>,
) -> Result<ResolvedCallable, ResolveCallError> {
    let instantiation = CallableInstantiation::TypeReceiver {
        receiver: TypeReceiverInstantiation::from_resolved(*receiver),
    };
    resolve_catalog_record(
        seed.record,
        &seed.equivalent_sources,
        None,
        instantiation,
        request,
    )
}

fn resolve_function_value(
    seed: &ResolvedFunctionValueSeed,
    request: &mut CallResolverRequest<'_>,
) -> Result<ResolvedCallTarget, ResolveCallError> {
    check_query_step(request)?;
    if let Some(base) = &seed.continuation_base {
        let candidate = base.try_curried(seed.next_group, request.limits)?;
        return NonEmptyResolvedCandidates::try_new(vec![candidate], request.limits)
            .map(ResolvedCallTarget::Candidates);
    }
    let callable = ResolvedCallable::try_from_intrinsic(
        CallableCandidateId::FunctionValue(seed.id.clone()),
        SignatureOrigin::FunctionValue {
            id: seed.id.clone(),
        },
        Arc::new(seed.schema.clone()),
        CallableInstantiation::None,
        Vec::new(),
        request.limits,
    )?;
    ResolvedFunctionValue::try_new(
        seed.id.clone(),
        callable,
        seed.ty.clone(),
        seed.effect_callable.clone(),
        None,
        seed.next_group,
    )
    .map(|value| ResolvedCallTarget::FunctionValue(Box::new(value)))
}

#[allow(
    clippy::too_many_lines,
    reason = "the ordered selected-family chain is the canonical precedence table"
)]
fn resolve_selected_call(
    request: &mut CallResolverRequest<'_>,
    receiver: EvaluatedReceiver<'_>,
    method: &CallableName,
    arguments: &[HirCallArgument],
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    let receiver_type = receiver.ty();
    check_query_step(request)?;
    if let Some(id) = DropCallableId::resolve(method) {
        return resolved_language_method(
            request,
            CallableCandidateId::Drop(id),
            LanguageCallableFamily::Drop,
            id.signature_schema(),
            CallableInstantiation::None,
        )
        .map(Some);
    }

    check_query_step(request)?;
    if let Some(id @ (DomainMethodId::Traverse | DomainMethodId::Parallel)) =
        DomainMethodId::resolve(receiver_type, method)
    {
        let schema = id.signature_schema(receiver_type);
        return resolved_language_method(
            request,
            CallableCandidateId::DomainMethod(id),
            LanguageCallableFamily::DomainMethod,
            schema,
            receiver.value_instantiation(),
        )
        .map(Some);
    }

    if let Some(target) = resolve_selected_environment_method(request, receiver_type, method)? {
        return Ok(Some(target));
    }

    check_query_step(request)?;
    if let Some(id) = CollectionMethodId::resolve(method) {
        let schema = id.signature_schema(receiver_type);
        return resolved_language_method(
            request,
            CallableCandidateId::CollectionMethod(id),
            LanguageCallableFamily::CollectionMethod,
            schema,
            receiver.value_instantiation(),
        )
        .map(Some);
    }

    check_query_step(request)?;
    if let Some(id) = PresentationHandleMethodId::resolve(receiver_type, method) {
        return resolved_language_method(
            request,
            CallableCandidateId::PresentationHandleMethod(id),
            LanguageCallableFamily::PresentationHandleMethod,
            id.signature_schema(),
            receiver.value_instantiation(),
        )
        .map(Some);
    }

    check_query_step(request)?;
    if let Some(id) = IntegerMethodId::resolve(receiver_type, method) {
        let schema = id.signature_schema(receiver_type);
        return resolved_language_method(
            request,
            CallableCandidateId::IntegerMethod(id),
            LanguageCallableFamily::IntegerMethod,
            schema,
            receiver.value_instantiation(),
        )
        .map(Some);
    }

    check_query_step(request)?;
    if let Some(id) = DomainMethodId::resolve(receiver_type, method) {
        let schema = id.signature_schema(receiver_type);
        return resolved_language_method(
            request,
            CallableCandidateId::DomainMethod(id),
            LanguageCallableFamily::DomainMethod,
            schema,
            receiver.value_instantiation(),
        )
        .map(Some);
    }

    check_query_step(request)?;
    if let Some(id) = StageMethodId::resolve(receiver_type, method, arguments.len()) {
        return resolved_language_method(
            request,
            CallableCandidateId::StageMethod(id),
            LanguageCallableFamily::StageMethod,
            id.signature_schema(),
            receiver.value_instantiation(),
        )
        .map(Some);
    }

    check_query_step(request)?;
    if let Some(id) = CapacityMethodId::resolve(receiver_type, method, arguments.len()) {
        let schema = id.signature_schema();
        return resolved_language_method(
            request,
            CallableCandidateId::CapacityMethod(id),
            LanguageCallableFamily::CapacityMethod,
            schema,
            receiver.value_instantiation(),
        )
        .map(Some);
    }

    check_query_step(request)?;
    if let Some(target) = resolve_checked_method(
        request,
        receiver_type,
        method,
        receiver.value_instantiation(),
    )? {
        return Ok(Some(target));
    }

    check_query_step(request)?;
    if let Some(target) = resolve_data_last_method(request, receiver, method, arguments)? {
        return Ok(Some(target));
    }

    Ok(None)
}

fn resolve_checked_method(
    request: &mut CallResolverRequest<'_>,
    receiver: &TypeKind,
    method: &CallableName,
    instantiation: CallableInstantiation,
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    let key = ReceiverMethodKey::new(receiver.clone(), method.clone());
    let id = match request.checked.method(&key) {
        super::CheckedMethodLookup::Absent => return Ok(None),
        super::CheckedMethodLookup::Unique(id) => id,
        super::CheckedMethodLookup::Ambiguous(candidates) => {
            return Err(ResolveCallError::AmbiguousTraitMethod { candidates });
        }
        super::CheckedMethodLookup::Inaccessible(candidates) => {
            return Err(ResolveCallError::InaccessibleMethod { candidates });
        }
    };
    let record = request
        .checked
        .record(&id)
        .map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
    if record.method_role().is_none() || record.key() != &CallableLookupKey::Method(key) {
        return Err(ResolveCallError::InvalidResolvedCallable);
    }
    let record = Arc::clone(record);
    let callable = resolve_catalog_record(&record, &[], None, instantiation, request)?;
    NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
        .map(ResolvedCallTarget::Candidates)
        .map(Some)
}

fn resolve_data_last_method(
    request: &mut CallResolverRequest<'_>,
    receiver: EvaluatedReceiver<'_>,
    method: &CallableName,
    arguments: &[HirCallArgument],
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    let mut bases = Vec::new();
    let path = CallablePath::try_new([method.clone()])
        .map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
    let (current_module, symbols, world) = request.authority.parts();
    let project_path = ProjectCallablePath::new(
        symbols.world().package().clone(),
        current_module.clone(),
        path.clone(),
    );
    if let Some(binding) = world
        .environment()
        .callable_catalog()
        .project_binding(&project_path)
    {
        check_query_step(request)?;
        match resolve_project_binding(binding, &project_path, request)? {
            ResolvedCallTarget::Candidates(candidates) => {
                bases.extend(candidates.as_slice().iter().cloned());
            }
            ResolvedCallTarget::NonCallable(_) => return Ok(None),
            ResolvedCallTarget::FunctionValue(_) => {
                return Err(ResolveCallError::InvalidResolvedCallable);
            }
        }
    }

    let catalog = world.environment().callable_catalog();
    if let Some(candidates) = catalog
        .validated_free(&path)
        .map_err(|reason| corrupt(CallableLookupKey::Free(path.clone()), reason))?
    {
        for entry in candidates.as_slice() {
            check_query_step(request)?;
            bases.push(resolve_catalog_record(
                entry.primary(),
                entry.equivalent_sources(),
                None,
                CallableInstantiation::None,
                request,
            )?);
        }
    }
    let mut seen = std::collections::HashSet::new();
    bases.retain(|candidate| seen.insert(candidate.id().clone()));
    finish_data_last_candidates(request, receiver, arguments, bases)
}

fn finish_data_last_candidates(
    request: &mut CallResolverRequest<'_>,
    receiver: EvaluatedReceiver<'_>,
    arguments: &[HirCallArgument],
    bases: Vec<ResolvedCallable>,
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    let receiver_type = receiver.ty();
    let mut candidates = Vec::new();
    for base in bases {
        check_query_step(request)?;
        let Some((group, parameter)) = data_last_receiver_coordinate(
            request.authority.module(),
            base.schema(),
            request.call_group,
            receiver_type,
            arguments,
        ) else {
            continue;
        };
        let id = DataLastCallableId::try_new(base.id().clone(), group, parameter, base.schema())
            .map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
        check_query_step(request)?;
        candidates.push(base.try_data_last(
            id,
            receiver.data_last_instantiation(group, parameter),
            request.limits,
        )?);
    }
    match candidates.as_slice() {
        [] => Ok(None),
        [candidate] => NonEmptyResolvedCandidates::try_new(vec![candidate.clone()], request.limits)
            .map(ResolvedCallTarget::Candidates)
            .map(Some),
        _ => {
            let ids = candidates
                .iter()
                .filter_map(|candidate| match candidate.id() {
                    CallableCandidateId::DataLast(id) => Some(id.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            Err(ResolveCallError::DataLastAmbiguity {
                candidates: ids.into(),
            })
        }
    }
}

fn data_last_receiver_coordinate(
    request_module: &HirModule,
    schema: &CallableSignatureSchema,
    current_group: CallableGroupIndex,
    receiver_type: &TypeKind,
    arguments: &[HirCallArgument],
) -> Option<(CallableGroupIndex, CallableParameterIndex)> {
    if let Some(group) = schema.group(current_group)
        && let Some(parameter) = group.parameters().last()
        && let super::CallableParameterType::Exact(expected) = parameter.ty()
        && expected.accepts(receiver_type)
        && authored_argument_slot_count(request_module, arguments) + 1 == group.parameters().len()
    {
        return Some((current_group, parameter.index()));
    }

    let next_group = CallableGroupIndex::try_from_usize(current_group.get() + 1).ok()?;
    let next = schema.group(next_group)?;
    let [parameter] = next.parameters() else {
        return None;
    };
    let super::CallableParameterType::Exact(expected) = parameter.ty() else {
        return None;
    };
    (expected.accepts(receiver_type)
        && call_shape_is_viable(request_module, schema, current_group, arguments))
    .then_some((next_group, parameter.index()))
}

fn authored_argument_slot_count(module: &HirModule, arguments: &[HirCallArgument]) -> usize {
    arguments
        .iter()
        .map(|argument| match argument {
            HirCallArgument::Spread { .. } => {
                module
                    .resolve_expr(argument.value())
                    .map_or(1, |value| match value.kind() {
                        HirExprKind::BracketSequence(sequence) => sequence.elements().len(),
                        HirExprKind::NumericBracketSequence(sequence) => sequence.elements().len(),
                        _ => 1,
                    })
            }
            HirCallArgument::Positional { .. } | HirCallArgument::Named { .. } => 1,
        })
        .sum()
}

fn resolved_language_method(
    request: &mut CallResolverRequest<'_>,
    id: CallableCandidateId,
    family: LanguageCallableFamily,
    schema: CallableSignatureSchema,
    instantiation: CallableInstantiation,
) -> Result<ResolvedCallTarget, ResolveCallError> {
    check_query_step(request)?;
    let callable = ResolvedCallable::try_from_intrinsic(
        id,
        SignatureOrigin::Language { family },
        Arc::new(schema),
        instantiation,
        Vec::new(),
        request.limits,
    )?;
    NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
        .map(ResolvedCallTarget::Candidates)
}

fn resolve_selected_environment_method(
    request: &mut CallResolverRequest<'_>,
    receiver_type: &TypeKind,
    method: &CallableName,
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    check_query_step(request)?;
    let key = ReceiverMethodKey::new(receiver_type.clone(), method.clone());
    let (_, _, world) = request.authority.parts();
    let catalog = world.environment().callable_catalog();
    let Some(candidates) = catalog
        .validated_method(&key)
        .map_err(|reason| corrupt(CallableLookupKey::Method(key.clone()), reason))?
    else {
        return Ok(None);
    };
    let mut resolved = Vec::with_capacity(candidates.len().get() as usize);
    for entry in candidates.as_slice() {
        check_query_step(request)?;
        resolved.push(resolve_catalog_record(
            entry.primary(),
            entry.equivalent_sources(),
            None,
            CallableInstantiation::None,
            request,
        )?);
    }
    NonEmptyResolvedCandidates::try_new(resolved, request.limits)
        .map(ResolvedCallTarget::Candidates)
        .map(Some)
}

#[allow(
    clippy::too_many_lines,
    reason = "the ordered free-family chain is the canonical precedence table"
)]
fn resolve_free_call(
    request: &mut CallResolverRequest<'_>,
    path: &CallablePath,
    project: Option<&CallableDeclarationKey>,
    scope: PreparedFreeCallScope,
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    if scope == PreparedFreeCallScope::ExplicitProject {
        return project
            .map(|declaration| resolve_exact_project_callable(declaration, path, request))
            .transpose();
    }
    check_query_step(request)?;
    if let FxResolution::Known(id) = FxCallableSignatureId::resolve(path) {
        check_query_step(request)?;
        let callable = ResolvedCallable::try_from_intrinsic(
            CallableCandidateId::Fx(id),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::Fx,
            },
            Arc::new(id.signature_schema()),
            CallableInstantiation::None,
            Vec::new(),
            request.limits,
        )?;
        return NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
            .map(ResolvedCallTarget::Candidates)
            .map(Some);
    }

    check_query_step(request)?;
    let enum_variant = match &request.callee {
        PreparedCallCallee::Free {
            enum_variant: Some(seed),
            ..
        } => Some((*seed).clone()),
        _ => None,
    };
    if let Some(seed) = enum_variant {
        check_query_step(request)?;
        let callable = ResolvedCallable::try_from_intrinsic(
            CallableCandidateId::EnumVariant(seed.id.clone()),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::EnumConstructor,
            },
            Arc::new(seed.schema.clone()),
            CallableInstantiation::ExpectedEnum {
                expected: seed.expected.clone(),
            },
            Vec::new(),
            request.limits,
        )?;
        return NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
            .map(ResolvedCallTarget::Candidates)
            .map(Some);
    }

    check_query_step(request)?;
    if let Some(kind) = ResultConstructorKind::resolve(path) {
        let expected = request
            .expected
            .filter(|expected| matches!(expected, TypeKind::Result { .. }))
            .cloned();
        check_query_step(request)?;
        let callable = ResolvedCallable::try_from_intrinsic(
            CallableCandidateId::Result(kind),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::ResultConstructor,
            },
            Arc::new(kind.instantiated_signature_schema(expected.as_ref())),
            CallableInstantiation::Result { kind, expected },
            Vec::new(),
            request.limits,
        )?;
        return NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
            .map(ResolvedCallTarget::Candidates)
            .map(Some);
    }

    check_query_step(request)?;
    if let Some(kind) = OptionConstructorKind::resolve(path) {
        let expected = request
            .expected
            .filter(|expected| matches!(expected, TypeKind::Option(_)))
            .cloned();
        check_query_step(request)?;
        let callable = ResolvedCallable::try_from_intrinsic(
            CallableCandidateId::Option(kind),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::OptionConstructor,
            },
            Arc::new(kind.instantiated_signature_schema(expected.as_ref())),
            CallableInstantiation::Option { expected },
            Vec::new(),
            request.limits,
        )?;
        return NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
            .map(ResolvedCallTarget::Candidates)
            .map(Some);
    }

    check_query_step(request)?;
    if let Some(id) = BuiltinCallableId::resolve(path) {
        let schema = Arc::new(match id {
            BuiltinCallableId::Reduction(kind) => {
                kind.instantiated_signature_schema(request.expected)
            }
            _ => id.signature_schema(),
        });
        check_query_step(request)?;
        let callable = ResolvedCallable::try_from_intrinsic(
            CallableCandidateId::Builtin(id),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::Builtin,
            },
            schema,
            CallableInstantiation::None,
            Vec::new(),
            request.limits,
        )?;
        return NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
            .map(ResolvedCallTarget::Candidates)
            .map(Some);
    }

    check_query_step(request)?;
    if let Some(id) = AgentIntrinsicSignatureId::resolve(path) {
        let schema = Arc::new(id.signature_schema());
        check_query_step(request)?;
        let callable = ResolvedCallable::try_from_intrinsic(
            CallableCandidateId::Agent(id),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::Agent,
            },
            schema,
            CallableInstantiation::None,
            Vec::new(),
            request.limits,
        )?;
        return NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
            .map(ResolvedCallTarget::Candidates)
            .map(Some);
    }

    check_query_step(request)?;
    if let Some(id) = PresentationCallableId::resolve(path) {
        let schema = id
            .checker_signature_schema()
            .map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
        check_query_step(request)?;
        let callable = ResolvedCallable::try_from_intrinsic(
            CallableCandidateId::Presentation(id),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::Presentation,
            },
            Arc::new(schema),
            CallableInstantiation::None,
            Vec::new(),
            request.limits,
        )?;
        return NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
            .map(ResolvedCallTarget::Candidates)
            .map(Some);
    }

    check_query_step(request)?;
    let (current_module, symbols, world) = request.authority.parts();
    let project_path = ProjectCallablePath::new(
        symbols.world().package().clone(),
        current_module.clone(),
        path.clone(),
    );
    if let Some(declaration) = project {
        return resolve_exact_project_callable(declaration, path, request).map(Some);
    }
    if let Some(binding) = world
        .environment()
        .callable_catalog()
        .project_binding(&project_path)
    {
        check_query_step(request)?;
        return resolve_project_binding(binding, &project_path, request).map(Some);
    }

    check_query_step(request)?;
    let catalog = world.environment().callable_catalog();
    if let Some(candidates) = catalog
        .validated_free(path)
        .map_err(|reason| corrupt(CallableLookupKey::Free(path.clone()), reason))?
    {
        let mut resolved = Vec::with_capacity(candidates.len().get() as usize);
        for entry in candidates.as_slice() {
            check_query_step(request)?;
            resolved.push(resolve_catalog_record(
                entry.primary(),
                entry.equivalent_sources(),
                None,
                CallableInstantiation::None,
                request,
            )?);
        }
        return NonEmptyResolvedCandidates::try_new(resolved, request.limits)
            .map(ResolvedCallTarget::Candidates)
            .map(Some);
    }

    check_query_step(request)?;
    if let Some(id) = PromotionCallableId::resolve(path) {
        check_query_step(request)?;
        let callable = ResolvedCallable::try_from_intrinsic(
            CallableCandidateId::Promotion(id),
            SignatureOrigin::Language {
                family: match id {
                    PromotionCallableId::Promote | PromotionCallableId::PromoteUnchecked => {
                        LanguageCallableFamily::Promote
                    }
                    PromotionCallableId::Assume => LanguageCallableFamily::Assume,
                },
            },
            Arc::new(id.signature_schema()),
            CallableInstantiation::None,
            Vec::new(),
            request.limits,
        )?;
        return NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
            .map(ResolvedCallTarget::Candidates)
            .map(Some);
    }

    Ok(None)
}

fn resolve_exact_project_callable(
    declaration: &CallableDeclarationKey,
    path: &CallablePath,
    request: &mut CallResolverRequest<'_>,
) -> Result<ResolvedCallTarget, ResolveCallError> {
    check_query_step(request)?;
    let (current_module, symbols, world) = request.authority.parts();
    let project_path = ProjectCallablePath::new(
        symbols.world().package().clone(),
        current_module.clone(),
        path.clone(),
    );
    let record = world
        .environment()
        .callable_catalog()
        .project_record(declaration)
        .ok_or_else(|| {
            corrupt(
                CallableLookupKey::Free(path.clone()),
                super::CorruptCallableCatalogReason::MissingRecord,
            )
        })?
        .clone();
    if record.id() != &CallableCandidateId::Project(declaration.clone()) {
        return Err(corrupt(
            CallableLookupKey::Free(path.clone()),
            super::CorruptCallableCatalogReason::WrongAuthority,
        ));
    }
    let callable = resolve_catalog_record(
        &record,
        &[],
        Some(&project_path),
        CallableInstantiation::None,
        request,
    )?;
    NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
        .map(ResolvedCallTarget::Candidates)
}

fn resolve_project_binding(
    binding: &ProjectNameBinding,
    path: &ProjectCallablePath,
    request: &mut CallResolverRequest<'_>,
) -> Result<ResolvedCallTarget, ResolveCallError> {
    let (_, _, world) = request.authority.parts();
    match binding {
        ProjectNameBinding::Callable(declaration) => {
            let record = world
                .environment()
                .callable_catalog()
                .project_record(declaration)
                .ok_or_else(|| {
                    corrupt(
                        record_key(path),
                        super::CorruptCallableCatalogReason::MissingRecord,
                    )
                })?
                .clone();
            let callable = resolve_catalog_record(
                &record,
                &[],
                Some(path),
                CallableInstantiation::None,
                request,
            )?;
            NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
                .map(ResolvedCallTarget::Candidates)
        }
        ProjectNameBinding::AmbiguousCallables { declarations } => {
            Err(ResolveCallError::AmbiguousOverload {
                candidates: declarations
                    .iter()
                    .cloned()
                    .map(CallableCandidateId::Project)
                    .collect(),
            })
        }
        ProjectNameBinding::Environment(id) => {
            let record = world
                .environment()
                .callable_catalog()
                .environment_record(id)
                .ok_or_else(|| {
                    corrupt(
                        record_key(path),
                        super::CorruptCallableCatalogReason::MissingRecord,
                    )
                })?
                .clone();
            let callable = resolve_catalog_record(
                &record,
                &[],
                Some(path),
                CallableInstantiation::None,
                request,
            )?;
            NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
                .map(ResolvedCallTarget::Candidates)
        }
        ProjectNameBinding::NonCallable { path, ty } => Ok(ResolvedCallTarget::NonCallable(
            ResolvedNonCallableTarget::new(
                NonCallableSource::Project { path: path.clone() },
                ty.clone(),
            ),
        )),
    }
}

fn resolve_catalog_record(
    record: &Arc<CallableRecord>,
    equivalent_sources: &[EquivalentCallableSource],
    project_path: Option<&ProjectCallablePath>,
    instantiation: CallableInstantiation,
    request: &mut CallResolverRequest<'_>,
) -> Result<ResolvedCallable, ResolveCallError> {
    check_query_step(request)?;
    let checked_id = request
        .checked
        .checked_for_candidate(record.id())
        .map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
    let checked_record = request
        .checked
        .record(checked_id)
        .map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
    if !Arc::ptr_eq(checked_record, record) {
        return Err(ResolveCallError::InvalidResolvedCallable);
    }
    let origin = match record.id() {
        CallableCandidateId::Project(declaration) => SignatureOrigin::Project {
            declaration: declaration.clone(),
            binding: project_path.cloned(),
        },
        CallableCandidateId::Environment(id) => match id.owner() {
            EnvironmentCallableOwner::Standard(owner) => SignatureOrigin::Standard {
                owner: *owner,
                id: id.clone(),
            },
            EnvironmentCallableOwner::Adapter(package) => SignatureOrigin::Adapter {
                package: package.clone(),
                id: id.clone(),
            },
        },
        _ => return Err(ResolveCallError::InvalidResolvedCallable),
    };
    let (_, _, world) = request.authority.parts();
    let reachable = match record.id() {
        CallableCandidateId::Project(id) => world
            .environment()
            .callable_catalog()
            .project_record(id)
            .is_some_and(|accepted| Arc::ptr_eq(accepted, record)),
        CallableCandidateId::Environment(id) => world
            .environment()
            .callable_catalog()
            .environment_record(id)
            .is_some_and(|accepted| Arc::ptr_eq(accepted, record)),
        _ => false,
    };
    if !reachable {
        return Err(corrupt(
            record.key().clone(),
            super::CorruptCallableCatalogReason::MissingRecord,
        ));
    }
    ResolvedCallable::try_from_checked_record(
        checked_id.clone(),
        Arc::clone(record),
        origin,
        instantiation,
        equivalent_sources.to_vec(),
        Some(record.authority()),
        request.limits,
    )
}

fn record_key(path: &ProjectCallablePath) -> CallableLookupKey {
    CallableLookupKey::Free(path.path().clone())
}

pub(super) fn corrupt(
    key: CallableLookupKey,
    reason: super::CorruptCallableCatalogReason,
) -> ResolveCallError {
    ResolveCallError::CorruptCatalog {
        key: Box::new(key),
        reason,
    }
}

fn check_query_step(request: &mut CallResolverRequest<'_>) -> Result<(), ResolveCallError> {
    if request.cancellation.load(Ordering::Acquire) {
        return Err(ResolveCallError::Cancelled);
    }
    request.work.charge(1).map_err(ResolveCallError::Work)
}
