//! Shared resolver dispatch, candidate materialization, and exact work charging.

use super::{
    AgentIntrinsicSignatureId, Arc, BuiltinCallableId, CallResolverRequest, CallableCandidateId,
    CallableDeclarationKey, CallableInstantiation, CallableLookupKey, CallableName, CallablePath,
    CallableRecord, CallableSignatureSchema, CapacityMethodId, CollectionMethodId, DomainMethodId,
    EnvironmentCallableOwner, EquivalentCallableSource, EvaluatedReceiver, FxCallableSignatureId,
    FxResolution, HirCallArgument, IntegerMethodId, LanguageCallableFamily, LineContextMethodId,
    LineScheduleCallableId, NonCallableSource, NonEmptyResolvedCandidates, OptionConstructorKind,
    Ordering, PreparedCallCallee, PreparedFreeCallScope, PreparedFunctionValueCallee,
    PreparedFunctionValueOriginProducer, PreparedResolvedCallable, PresentationCallableId,
    PresentationHandleMethodId, ProjectCallablePath, ProjectNameBinding, PromotionCallableId,
    ReceiverMethodKey, ResolveCallError, ResolveCallOutcome, ResolvedAssociatedTypeReceiver,
    ResolvedCallTarget, ResolvedFunctionValueSeed, ResolvedNonCallableTarget,
    ResultConstructorKind, SignatureOrigin, StageMethodId, TypeKind, TypeReceiverInstantiation,
    TypedEnvironmentMethodCandidate, UnknownCallKind, UnknownCallTarget,
};
use crate::callable::CallConstraintInvariant;
use crate::callable::{DialogueCallableId, DialogueCalleeIdentity, DialogueSchemaContext};

pub(crate) fn resolve_call_target(mut request: CallResolverRequest<'_>) -> ResolveCallOutcome {
    if let Err(error) = request.work.record_resolver_invocation() {
        return ResolveCallOutcome::Rejected(error.into());
    }
    if let Err(error) = check_query_step(&mut request) {
        return ResolveCallOutcome::Rejected(error);
    }
    if request.parenthesized_call().is_none()
        && !matches!(
            request.callee,
            PreparedCallCallee::Dialogue {
                id: DialogueCallableId::ContentApplication,
                ..
            }
        )
    {
        return ResolveCallOutcome::Rejected(ResolveCallError::InvalidResolvedCallable);
    }
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
            let arguments = request
                .parenthesized_call()
                .expect("selected calls are parenthesized")
                .arguments();
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
            let arguments = request
                .parenthesized_call()
                .expect("associated calls are parenthesized")
                .arguments();
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
        PreparedCallCallee::Dialogue {
            id,
            callee,
            patch_context,
            result,
        } => match resolve_dialogue_call(&mut request, id, callee, patch_context, result) {
            Ok(target) => ResolveCallOutcome::Resolved(target),
            Err(error) => ResolveCallOutcome::Rejected(error),
        },
        PreparedCallCallee::FunctionValue { value } => {
            match resolve_function_value(value, &mut request) {
                Ok(target) => ResolveCallOutcome::Resolved(target),
                Err(ResolveFunctionValueError::Invariant(error)) => {
                    ResolveCallOutcome::Invariant(error)
                }
                Err(ResolveFunctionValueError::Resolver(error)) => {
                    ResolveCallOutcome::Rejected(error)
                }
            }
        }
        PreparedCallCallee::NonCallableValue { ty, .. } => {
            ResolveCallOutcome::Resolved(ResolvedCallTarget::NonCallable(
                ResolvedNonCallableTarget::new(NonCallableSource::EvaluatedExpression, ty.clone()),
            ))
        }
    }
}

fn resolve_dialogue_call(
    request: &mut CallResolverRequest<'_>,
    id: DialogueCallableId,
    callee: &DialogueCalleeIdentity,
    patch_context: super::CharacterDialoguePatchContext,
    result: crate::callable::DialogueCallableResultContext<'_>,
) -> Result<ResolvedCallTarget, ResolveCallError> {
    if !id.supports_callee(callee) {
        return Err(ResolveCallError::InvalidResolvedCallable);
    }
    let schema = id
        .signature_schema(DialogueSchemaContext {
            callee,
            module: request.authority.module().key().path(),
            custom_fields: request
                .authority
                .world()
                .environment()
                .character_dialogue_fields(),
            patch_context,
            result,
        })
        .map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
    let instantiation = match callee {
        DialogueCalleeIdentity::Character { character }
        | DialogueCalleeIdentity::CharacterDialogue { character } => match character {
            crate::types::CharacterDialogueCharacterType::Exact(character) => {
                CallableInstantiation::Character {
                    owner: super::ResolvedCharacterOwner::new(
                        character.clone(),
                        super::CharacterOwnerSource::EntityReference,
                    ),
                }
            }
            crate::types::CharacterDialogueCharacterType::Any => CallableInstantiation::None,
        },
        DialogueCalleeIdentity::Content { .. } => CallableInstantiation::None,
    };
    check_query_step(request)?;
    let callable = PreparedResolvedCallable::try_from_intrinsic(
        CallableCandidateId::Dialogue(id),
        SignatureOrigin::LanguageDialogue {
            operation: id,
            callee: Arc::new(super::super::ResolvedDialogueCalleeIdentity::from_callee(
                callee,
                request.authority.module().key().path(),
            )),
        },
        Arc::new(schema),
        instantiation,
        Vec::new(),
        request.limits,
    )?;
    NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
        .map(ResolvedCallTarget::Candidates)
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
) -> Result<PreparedResolvedCallable, ResolveCallError> {
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

enum ResolveFunctionValueError {
    Resolver(ResolveCallError),
    Invariant(CallConstraintInvariant),
}

impl From<ResolveCallError> for ResolveFunctionValueError {
    fn from(error: ResolveCallError) -> Self {
        Self::Resolver(error)
    }
}

fn resolve_function_value(
    value: &PreparedFunctionValueCallee,
    request: &mut CallResolverRequest<'_>,
) -> Result<ResolvedCallTarget, ResolveFunctionValueError> {
    check_query_step(request)?;
    let (id, schema) = match value.seed() {
        ResolvedFunctionValueSeed::Lexical {
            id,
            schema,
            effect_callable: _,
        } => {
            let PreparedFunctionValueOriginProducer::Lexical { local } = value.origin().producer()
            else {
                return Err(ResolveFunctionValueError::Resolver(
                    ResolveCallError::InvalidResolvedCallable,
                ));
            };
            let callable = PreparedResolvedCallable::try_from_intrinsic_with_lexical(
                id.clone(),
                *local,
                Arc::new(schema.clone()),
                request.limits,
            )?;
            return NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
                .map(ResolvedCallTarget::Candidates)
                .map_err(ResolveFunctionValueError::Resolver);
        }
        ResolvedFunctionValueSeed::Independent {
            id,
            schema,
            effect_callable: _,
        } => (id, schema),
        ResolvedFunctionValueSeed::PreparedContinuation { reference } => {
            let candidate = request
                .prepared_continuations
                .resolve_prepared_continuation(reference, value.actual())
                .map_err(ResolveFunctionValueError::Invariant)?;
            return NonEmptyResolvedCandidates::try_new_prepared(vec![candidate], request.limits)
                .map(ResolvedCallTarget::Candidates)
                .map_err(ResolveFunctionValueError::Resolver);
        }
    };
    let callable = PreparedResolvedCallable::try_from_intrinsic_with_function_value(
        CallableCandidateId::FunctionValue(id.clone()),
        SignatureOrigin::FunctionValue { id: id.clone() },
        value.origin(),
        Arc::new(schema.clone()),
        CallableInstantiation::None,
        Vec::new(),
        request.limits,
    )?;
    NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
        .map(ResolvedCallTarget::Candidates)
        .map_err(ResolveFunctionValueError::Resolver)
}

#[allow(
    clippy::too_many_lines,
    reason = "the selected-call candidate inventory is one exhaustive typed collection"
)]
fn resolve_selected_call(
    request: &mut CallResolverRequest<'_>,
    receiver: EvaluatedReceiver<'_>,
    method: &CallableName,
    arguments: &[HirCallArgument],
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    let receiver_type = receiver.ty();
    let mut candidates = Vec::new();

    check_query_step(request)?;
    if let Some(id) = LineContextMethodId::resolve(receiver_type, method, arguments.len()) {
        candidates.push(prepare_language_method(
            request,
            CallableCandidateId::LineContextMethod(id),
            LanguageCallableFamily::LineContextMethod,
            id.signature_schema(),
            receiver.value_instantiation(),
        )?);
    }

    check_query_step(request)?;
    if let Some(id) = CollectionMethodId::resolve(method)
        && let Some(schema) = id.signature_schema(receiver_type)
    {
        candidates.push(prepare_language_method(
            request,
            CallableCandidateId::CollectionMethod(id),
            LanguageCallableFamily::CollectionMethod,
            schema,
            receiver.value_instantiation(),
        )?);
    }

    check_query_step(request)?;
    if let Some(id) = PresentationHandleMethodId::resolve(receiver_type, method) {
        candidates.push(prepare_language_method(
            request,
            CallableCandidateId::PresentationHandleMethod(id),
            LanguageCallableFamily::PresentationHandleMethod,
            id.signature_schema(),
            receiver.value_instantiation(),
        )?);
    }

    check_query_step(request)?;
    if let Some(id) = IntegerMethodId::resolve(receiver_type, method) {
        let schema = id.signature_schema(receiver_type);
        candidates.push(prepare_language_method(
            request,
            CallableCandidateId::IntegerMethod(id),
            LanguageCallableFamily::IntegerMethod,
            schema,
            receiver.value_instantiation(),
        )?);
    }

    check_query_step(request)?;
    if let Some(id) = DomainMethodId::resolve(receiver_type, method)
        && let Some(schema) = id.signature_schema(receiver_type)
    {
        candidates.push(prepare_language_method(
            request,
            CallableCandidateId::DomainMethod(id),
            LanguageCallableFamily::DomainMethod,
            schema,
            receiver.value_instantiation(),
        )?);
    }

    check_query_step(request)?;
    if let Some(id) = StageMethodId::resolve(receiver_type, method, arguments.len()) {
        candidates.push(prepare_language_method(
            request,
            CallableCandidateId::StageMethod(id),
            LanguageCallableFamily::StageMethod,
            id.signature_schema(receiver_type),
            receiver.value_instantiation(),
        )?);
    }

    check_query_step(request)?;
    if let Some(id) = CapacityMethodId::resolve(receiver_type, method, arguments.len()) {
        let schema = id.signature_schema();
        candidates.push(prepare_language_method(
            request,
            CallableCandidateId::CapacityMethod(id),
            LanguageCallableFamily::CapacityMethod,
            schema,
            receiver.value_instantiation(),
        )?);
    }

    check_query_step(request)?;
    candidates.extend(resolve_checked_method_candidates(
        request,
        receiver.ty(),
        method,
        receiver.value_instantiation(),
    )?);

    if candidates.is_empty() {
        return Ok(None);
    }
    NonEmptyResolvedCandidates::try_new(candidates, request.limits)
        .map(ResolvedCallTarget::Candidates)
        .map(Some)
}

fn resolve_checked_method(
    request: &mut CallResolverRequest<'_>,
    receiver: &TypeKind,
    method: &CallableName,
    instantiation: CallableInstantiation,
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    let key = ReceiverMethodKey::new(receiver.clone(), method.clone());
    let lookup = request.checked.exact_method(&key);
    let resolved =
        materialize_checked_method_candidates(request, &key, method, instantiation, lookup)?;
    if resolved.is_empty() {
        return Ok(None);
    }
    NonEmptyResolvedCandidates::try_new(resolved, request.limits)
        .map(ResolvedCallTarget::Candidates)
        .map(Some)
}

fn resolve_checked_method_candidates(
    request: &mut CallResolverRequest<'_>,
    receiver: &TypeKind,
    method: &CallableName,
    instantiation: CallableInstantiation,
) -> Result<Vec<PreparedResolvedCallable>, ResolveCallError> {
    let key = ReceiverMethodKey::new(receiver.clone(), method.clone());
    let lookup = request.checked.method(&key);
    materialize_checked_method_candidates(request, &key, method, instantiation, lookup)
}

fn materialize_checked_method_candidates(
    request: &mut CallResolverRequest<'_>,
    key: &ReceiverMethodKey,
    method: &CallableName,
    instantiation: CallableInstantiation,
    lookup: super::CheckedMethodLookup,
) -> Result<Vec<PreparedResolvedCallable>, ResolveCallError> {
    let ids = match lookup {
        super::CheckedMethodLookup::Absent => return Ok(Vec::new()),
        super::CheckedMethodLookup::Candidates(candidates) => candidates,
        super::CheckedMethodLookup::Inaccessible(candidates) => {
            return Err(ResolveCallError::InaccessibleMethod { candidates });
        }
    };
    let mut resolved = Vec::with_capacity(ids.len());
    for id in ids.iter() {
        let record = request
            .checked
            .record(id)
            .map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
        let candidate_instantiation = if let Some(extension) = record.schema().extension_receiver()
        {
            if record.extension_method_name() != Some(method) {
                return Err(ResolveCallError::InvalidResolvedCallable);
            }
            let CallableInstantiation::Receiver { receiver } = &instantiation else {
                return Ok(Vec::new());
            };
            CallableInstantiation::Extension {
                receiver: receiver.clone(),
                group: extension.group(),
                parameter: extension.parameter(),
            }
        } else if record.receiver_method_key().as_ref() == Some(key) {
            instantiation.clone()
        } else {
            return Err(ResolveCallError::InvalidResolvedCallable);
        };
        resolved.push(resolve_catalog_record(
            record,
            &[],
            None,
            candidate_instantiation,
            request,
        )?);
    }
    Ok(resolved)
}

fn resolved_language_method(
    request: &mut CallResolverRequest<'_>,
    id: CallableCandidateId,
    family: LanguageCallableFamily,
    schema: CallableSignatureSchema,
    instantiation: CallableInstantiation,
) -> Result<ResolvedCallTarget, ResolveCallError> {
    let callable = prepare_language_method(request, id, family, schema, instantiation)?;
    NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
        .map(ResolvedCallTarget::Candidates)
}

fn prepare_language_method(
    request: &mut CallResolverRequest<'_>,
    id: CallableCandidateId,
    family: LanguageCallableFamily,
    schema: CallableSignatureSchema,
    instantiation: CallableInstantiation,
) -> Result<PreparedResolvedCallable, ResolveCallError> {
    check_query_step(request)?;
    PreparedResolvedCallable::try_from_intrinsic(
        id,
        SignatureOrigin::Language { family },
        Arc::new(schema),
        instantiation,
        Vec::new(),
        request.limits,
    )
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
        let callable = PreparedResolvedCallable::try_from_intrinsic(
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
        } => Some(*seed),
        _ => None,
    };
    if let Some(seed) = enum_variant {
        check_query_step(request)?;
        let callable = PreparedResolvedCallable::try_from_intrinsic_with_enum_seed(
            CallableCandidateId::EnumVariant(seed.id.clone()),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::EnumConstructor,
            },
            &seed,
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
        check_query_step(request)?;
        let callable = PreparedResolvedCallable::try_from_intrinsic(
            CallableCandidateId::Result(kind),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::ResultConstructor,
            },
            Arc::new(kind.signature_schema()),
            CallableInstantiation::Result { kind },
            Vec::new(),
            request.limits,
        )?;
        return NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
            .map(ResolvedCallTarget::Candidates)
            .map(Some);
    }

    check_query_step(request)?;
    if let Some(kind) = OptionConstructorKind::resolve(path) {
        check_query_step(request)?;
        let callable = PreparedResolvedCallable::try_from_intrinsic(
            CallableCandidateId::Option(kind),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::OptionConstructor,
            },
            Arc::new(kind.signature_schema()),
            CallableInstantiation::Option,
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
            BuiltinCallableId::Reduction(kind) => kind
                .accepted_signature_schema(
                    request.authority.world().environment().nominal_catalog(),
                )
                .ok_or(ResolveCallError::InvalidResolvedCallable)?,
            _ => id
                .closed_signature_schema()
                .ok_or(ResolveCallError::InvalidResolvedCallable)?,
        });
        check_query_step(request)?;
        let callable = PreparedResolvedCallable::try_from_intrinsic(
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
        let callable = PreparedResolvedCallable::try_from_intrinsic(
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
    if let Some(id) = LineScheduleCallableId::resolve(path) {
        check_query_step(request)?;
        let callable = PreparedResolvedCallable::try_from_intrinsic(
            CallableCandidateId::LineSchedule(id),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::LineSchedule,
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
    if let Some(id) = PresentationCallableId::resolve(path) {
        let (_, _, world) = request.authority.parts();
        let schema = id
            .signature_schema(super::PresentationSchemaContext {
                owner: request.presentation_character_owner,
                environment: world.environment(),
            })
            .map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
        let instantiation = request
            .presentation_character_owner
            .cloned()
            .map(|owner| CallableInstantiation::Character { owner })
            .unwrap_or(CallableInstantiation::None);
        check_query_step(request)?;
        let callable = PreparedResolvedCallable::try_from_intrinsic(
            CallableCandidateId::Presentation(id),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::Presentation,
            },
            Arc::new(schema),
            instantiation,
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
        let implicit_extensions = request
            .implicit_extension_receiver
            .as_ref()
            .map(|_| {
                candidates
                    .as_slice()
                    .iter()
                    .filter(|entry| entry.primary().schema().extension_receiver().is_some())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let selected_entries = if implicit_extensions.is_empty() {
            candidates.as_slice().iter().collect::<Vec<_>>()
        } else {
            implicit_extensions
        };
        let mut resolved = Vec::with_capacity(selected_entries.len());
        for entry in selected_entries {
            check_query_step(request)?;
            let instantiation = match (
                request.implicit_extension_receiver.as_ref(),
                entry.primary().schema().extension_receiver(),
            ) {
                (Some(receiver), Some(extension)) => CallableInstantiation::Extension {
                    receiver: receiver.actual().clone(),
                    group: extension.group(),
                    parameter: extension.parameter(),
                },
                _ => CallableInstantiation::None,
            };
            resolved.push(resolve_catalog_record(
                entry.primary(),
                entry.equivalent_sources(),
                None,
                instantiation,
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
        let callable = PreparedResolvedCallable::try_from_intrinsic(
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
) -> Result<PreparedResolvedCallable, ResolveCallError> {
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
    PreparedResolvedCallable::try_from_checked_record(
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
