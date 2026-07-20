//! Validated callable resolver requests and products.

use std::{
    collections::HashMap,
    num::NonZeroU32,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use arcweft_character::id::{CharacterId, CharacterPartId};
use arcweft_lang_hir::symbol::{CallableDeclarationId, ProjectSymbolTable};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::expr::{CallArg, Expr};
use arcweft_source::{SourceDocumentIdentity, SourceSpan};

use crate::{
    checker::TypeExpressionId,
    effect_model::CallableId,
    effect_row::EffectRow,
    registration::RegisteredSemanticWorld,
    traits::{TraitCatalog, TraitMethodResolution, TraitPredicate},
    types::TypeKind,
};

use super::{
    AgentIntrinsicSignatureId, BuiltinCallableId, CallableAuthorityRank, CallableCandidateId,
    CallableGroupIndex, CallableLimits, CallableLookupKey, CallableName, CallableParameterIndex,
    CallablePath, CallableRecord, CallableSignatureSchema, CapacityMethodId, CollectionMethodId,
    CurriedCallableId, DataLastCallableId, DomainMethodId, DropCallableId,
    EnvironmentCallableOwner, EquivalentCallableSource, FunctionValueSignatureId,
    FxCallableSignatureId, FxResolution, IntegerMethodId, LanguageCallableFamily, LocalCallableId,
    OptionConstructorKind, PresentationCallableId, PresentationHandleMethodId, ProjectCallablePath,
    ProjectNameBinding, PromotionCallableId, ReceiverMethodKey, ResolveCallError, ResolverWork,
    ResultConstructorKind, SpeakerCallableId, TraitCallableId, TraitCallableSource,
    TraitImplementationIndex, call_shape_is_viable,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CallCallee<'a> {
    Free {
        path: &'a CallablePath,
        enum_variant: Option<&'a ResolvedEnumSeed>,
    },
    Selected {
        receiver_expression: TypeExpressionId,
        receiver_type: &'a TypeKind,
        method: &'a CallableName,
        arguments: &'a [CallArg],
    },
    Dialogue {
        id: super::DialogueCallableId,
        callee: &'a super::DialogueCalleeIdentity,
    },
    FunctionValue {
        value: &'a ResolvedFunctionValueSeed,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedEnumSeed {
    id: super::EnumVariantSignatureId,
    expected: TypeKind,
    schema: CallableSignatureSchema,
}

impl ResolvedEnumSeed {
    pub(crate) fn new(
        id: super::EnumVariantSignatureId,
        expected: TypeKind,
        schema: CallableSignatureSchema,
    ) -> Self {
        Self {
            id,
            expected,
            schema,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedFunctionValueSeed {
    id: FunctionValueSignatureId,
    ty: TypeKind,
    schema: CallableSignatureSchema,
    effect_callable: Option<CallableId>,
    source_candidate: Option<CallableCandidateId>,
    next_group: CallableGroupIndex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(
    dead_code,
    reason = "call-source accessors are consumed when the typed call-surface cut supplies exact spans"
)]
pub(crate) struct CallSourceContext<'a> {
    document: &'a SourceDocumentIdentity,
    call_span: Option<&'a SourceSpan>,
    callee_span: Option<&'a SourceSpan>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct LexicalCallableScope {
    bindings: HashMap<CallableName, LexicalCallBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(
    dead_code,
    reason = "callable and function-value lexical bindings belong to the following ordered resolver cuts"
)]
#[allow(
    clippy::large_enum_variant,
    reason = "the exact lexical resolver contract keeps the typed function-value seed inline"
)]
pub(crate) enum LexicalCallBinding {
    Callable {
        id: LocalCallableId,
        schema: Arc<CallableSignatureSchema>,
        effects: EffectRow,
    },
    FunctionValue(ResolvedFunctionValueSeed),
    Speaker {
        id: SpeakerCallableId,
        schema: Arc<CallableSignatureSchema>,
    },
    NonCallable {
        ty: TypeKind,
    },
}

#[allow(
    dead_code,
    reason = "expected type, traits, call group, and expression are consumed by subsequent family resolver cuts"
)]
pub(crate) struct CallResolverRequest<'a> {
    callee: CallCallee<'a>,
    lexical: &'a LexicalCallableScope,
    expected: Option<&'a TypeKind>,
    current_module: &'a CanonicalModulePath,
    symbols: &'a ProjectSymbolTable,
    world: &'a RegisteredSemanticWorld,
    traits: &'a TraitCatalog,
    trait_predicates: &'a [TraitPredicate],
    source: CallSourceContext<'a>,
    call_group: CallableGroupIndex,
    expression: TypeExpressionId,
    cancellation: &'a AtomicBool,
    work: &'a mut ResolverWork,
    limits: &'a CallableLimits,
}

impl ResolvedFunctionValueSeed {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        id: FunctionValueSignatureId,
        ty: TypeKind,
        schema: CallableSignatureSchema,
        effect_callable: Option<CallableId>,
        source_candidate: Option<CallableCandidateId>,
        next_group: CallableGroupIndex,
    ) -> Self {
        Self {
            id,
            ty,
            schema,
            effect_callable,
            source_candidate,
            next_group,
        }
    }
}

#[allow(
    dead_code,
    reason = "call-source accessors are consumed when the typed call-surface cut supplies exact spans"
)]
impl<'a> CallSourceContext<'a> {
    pub(crate) const fn new(
        document: &'a SourceDocumentIdentity,
        call_span: Option<&'a SourceSpan>,
        callee_span: Option<&'a SourceSpan>,
    ) -> Self {
        Self {
            document,
            call_span,
            callee_span,
        }
    }

    pub(crate) const fn document(&self) -> &SourceDocumentIdentity {
        self.document
    }
    pub(crate) const fn call_span(&self) -> Option<&SourceSpan> {
        self.call_span
    }
    pub(crate) const fn callee_span(&self) -> Option<&SourceSpan> {
        self.callee_span
    }
}

impl LexicalCallableScope {
    pub(crate) fn binding(&self, name: &CallableName) -> Option<&LexicalCallBinding> {
        self.bindings.get(name)
    }

    pub(crate) fn insert(&mut self, name: CallableName, binding: LexicalCallBinding) {
        self.bindings.insert(name, binding);
    }
}

#[allow(
    dead_code,
    reason = "some exact request accessors are consumed by subsequent family resolver cuts"
)]
impl<'a> CallResolverRequest<'a> {
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::result_large_err)]
    pub(crate) fn try_new(
        callee: CallCallee<'a>,
        lexical: &'a LexicalCallableScope,
        expected: Option<&'a TypeKind>,
        current_module: &'a CanonicalModulePath,
        symbols: &'a ProjectSymbolTable,
        world: &'a RegisteredSemanticWorld,
        traits: &'a TraitCatalog,
        trait_predicates: &'a [TraitPredicate],
        source: CallSourceContext<'a>,
        call_group: CallableGroupIndex,
        expression: TypeExpressionId,
        cancellation: &'a AtomicBool,
        work: &'a mut ResolverWork,
        limits: &'a CallableLimits,
    ) -> Result<Self, ResolveCallError> {
        if cancellation.load(Ordering::Relaxed) {
            return Err(ResolveCallError::Cancelled);
        }
        if symbols.world() != world.symbols().world()
            || symbols.revision() != world.symbols().revision()
            || symbols.world() != world.environment().world()
            || symbols.revision() != world.environment().symbol_revision()
        {
            return Err(ResolveCallError::WorldMismatch);
        }
        if symbols.source_identity(current_module) != Some(source.document) {
            return Err(ResolveCallError::SourceIdentityMismatch);
        }
        if source.document.source_len()
            > u64::try_from(limits.max_source_bytes()).unwrap_or(u64::MAX)
        {
            return Err(ResolveCallError::Work(
                super::CallableQueryLimitError::SourceBytes {
                    actual: usize::try_from(source.document.source_len()).unwrap_or(usize::MAX),
                    limit: limits.max_source_bytes(),
                },
            ));
        }
        if source
            .call_span
            .into_iter()
            .chain(source.callee_span)
            .any(|span| !source_span_is_valid(source.document, span))
        {
            return Err(ResolveCallError::InvalidSourceSpan);
        }
        Ok(Self {
            callee,
            lexical,
            expected,
            current_module,
            symbols,
            world,
            traits,
            trait_predicates,
            source,
            call_group,
            expression,
            cancellation,
            work,
            limits,
        })
    }

    pub(crate) const fn callee(&self) -> &CallCallee<'a> {
        &self.callee
    }
    pub(crate) const fn lexical(&self) -> &LexicalCallableScope {
        self.lexical
    }
    pub(crate) const fn expected(&self) -> Option<&TypeKind> {
        self.expected
    }
    pub(crate) const fn current_module(&self) -> &CanonicalModulePath {
        self.current_module
    }
    pub(crate) const fn symbols(&self) -> &ProjectSymbolTable {
        self.symbols
    }
    pub(crate) const fn world(&self) -> &RegisteredSemanticWorld {
        self.world
    }
    pub(crate) const fn traits(&self) -> &TraitCatalog {
        self.traits
    }
    pub(crate) const fn trait_predicates(&self) -> &[TraitPredicate] {
        self.trait_predicates
    }
    pub(crate) const fn source(&self) -> &CallSourceContext<'a> {
        &self.source
    }
    pub(crate) const fn call_group(&self) -> CallableGroupIndex {
        self.call_group
    }
    pub(crate) const fn expression(&self) -> TypeExpressionId {
        self.expression
    }
    pub(crate) const fn cancellation(&self) -> &AtomicBool {
        self.cancellation
    }
    pub(crate) const fn limits(&self) -> &CallableLimits {
        self.limits
    }
}

fn source_span_is_valid(document: &SourceDocumentIdentity, span: &SourceSpan) -> bool {
    let range = span.range();
    span.source() == document
        && range.start() <= range.end()
        && u64::try_from(range.end()).is_ok_and(|end| end <= document.source_len())
}

#[allow(clippy::result_large_err)]
pub(crate) fn resolve_call_target(mut request: CallResolverRequest<'_>) -> ResolveCallOutcome {
    if let Err(error) = check_query_step(&mut request) {
        return ResolveCallOutcome::Rejected(error);
    }
    match request.callee.clone() {
        CallCallee::Free { path, .. } => {
            let path = path.clone();
            match resolve_free_call(&mut request, &path) {
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
        CallCallee::Selected {
            receiver_type,
            method,
            arguments,
            ..
        } => {
            let receiver_type = receiver_type.clone();
            let method = method.clone();
            match resolve_selected_call(&mut request, &receiver_type, &method, arguments) {
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
        CallCallee::Dialogue { id, callee } => match resolve_dialogue_call(&request, id, callee) {
            Ok(target) => ResolveCallOutcome::Resolved(target),
            Err(error) => ResolveCallOutcome::Rejected(error),
        },
        CallCallee::FunctionValue { value } => {
            match resolve_function_value(value, request.limits) {
                Ok(target) => ResolveCallOutcome::Resolved(target),
                Err(error) => ResolveCallOutcome::Rejected(error),
            }
        }
    }
}

#[allow(clippy::result_large_err)]
fn resolve_dialogue_call(
    request: &CallResolverRequest<'_>,
    id: super::DialogueCallableId,
    callee: &super::DialogueCalleeIdentity,
) -> Result<ResolvedCallTarget, ResolveCallError> {
    use super::{DialogueCalleeIdentity, DialogueSchemaContext};

    if super::DialogueCallableId::resolve(callee) != id {
        return Err(ResolveCallError::InvalidResolvedCallable);
    }
    let schema = id
        .signature_schema(DialogueSchemaContext {
            callee,
            environment: request.world.environment(),
        })
        .map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
    let instantiation = match callee {
        DialogueCalleeIdentity::Speaker { character }
        | DialogueCalleeIdentity::SpeakerPreset { character } => CallableInstantiation::Character {
            owner: ResolvedCharacterOwner::new(
                character.clone(),
                CharacterOwnerSource::ExternalOwner,
            ),
        },
        DialogueCalleeIdentity::Content { .. } => CallableInstantiation::None,
    };
    let callable = ResolvedCallable::try_new(
        CallableCandidateId::Dialogue(id),
        SignatureOrigin::Language {
            family: LanguageCallableFamily::Dialogue,
        },
        Arc::new(schema),
        instantiation,
        Vec::new(),
        None,
        request.limits,
    )?;
    NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
        .map(ResolvedCallTarget::Candidates)
}

#[allow(clippy::result_large_err)]
fn resolve_function_value(
    seed: &ResolvedFunctionValueSeed,
    limits: &CallableLimits,
) -> Result<ResolvedCallTarget, ResolveCallError> {
    let callable = ResolvedCallable::try_new(
        CallableCandidateId::FunctionValue(seed.id.clone()),
        SignatureOrigin::FunctionValue {
            id: seed.id.clone(),
        },
        Arc::new(seed.schema.clone()),
        CallableInstantiation::None,
        Vec::new(),
        None,
        limits,
    )?;
    ResolvedFunctionValue::try_new(
        seed.id.clone(),
        callable,
        seed.ty.clone(),
        seed.effect_callable.clone(),
        seed.source_candidate.clone(),
        seed.next_group,
    )
    .map(ResolvedCallTarget::FunctionValue)
}

#[allow(
    clippy::result_large_err,
    clippy::too_many_lines,
    reason = "the ordered selected-family chain is the canonical precedence table"
)]
fn resolve_selected_call(
    request: &mut CallResolverRequest<'_>,
    receiver_type: &TypeKind,
    method: &CallableName,
    arguments: &[CallArg],
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
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
            CallableInstantiation::Receiver {
                receiver: receiver_type.clone(),
            },
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
            CallableInstantiation::Receiver {
                receiver: receiver_type.clone(),
            },
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
            CallableInstantiation::Receiver {
                receiver: receiver_type.clone(),
            },
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
            CallableInstantiation::Receiver {
                receiver: receiver_type.clone(),
            },
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
            CallableInstantiation::Receiver {
                receiver: receiver_type.clone(),
            },
        )
        .map(Some);
    }

    check_query_step(request)?;
    if let Some((id, result)) = CapacityMethodId::resolve(receiver_type, method, arguments.len()) {
        let schema = id.signature_schema(result);
        return resolved_language_method(
            request,
            CallableCandidateId::CapacityMethod(id),
            LanguageCallableFamily::CapacityMethod,
            schema,
            CallableInstantiation::Receiver {
                receiver: receiver_type.clone(),
            },
        )
        .map(Some);
    }

    check_query_step(request)?;
    if let Some(target) = resolve_trait_method(request, receiver_type, method)? {
        return Ok(Some(target));
    }

    check_query_step(request)?;
    if let Some(target) = resolve_data_last_method(request, receiver_type, method, arguments)? {
        return Ok(Some(target));
    }

    Ok(None)
}

#[allow(clippy::result_large_err)]
fn resolve_data_last_method(
    request: &mut CallResolverRequest<'_>,
    receiver_type: &TypeKind,
    method: &CallableName,
    arguments: &[CallArg],
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    let mut bases = Vec::new();
    if let Some(binding) = request.lexical.binding(method) {
        if let LexicalCallBinding::Callable { .. } = binding
            && let ResolvedCallTarget::Candidates(candidates) =
                resolve_lexical_binding(method, binding, request.limits)?
        {
            bases.extend(candidates.as_slice().iter().cloned());
        }
        return finish_data_last_candidates(request, receiver_type, arguments, bases);
    }

    let path = CallablePath::try_new([method.clone()])
        .map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
    let project_path = ProjectCallablePath::new(
        request.symbols.world().package().clone(),
        request.current_module.clone(),
        path.clone(),
    );
    if let Some(binding) = request
        .world
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

    if let Some(candidates) = request.world.environment().callable_catalog().free(&path) {
        for entry in candidates.as_slice() {
            check_query_step(request)?;
            bases.push(resolve_catalog_record(
                entry.primary(),
                entry.equivalent_sources(),
                None,
                request,
            )?);
        }
    }
    let mut seen = std::collections::HashSet::new();
    bases.retain(|candidate| seen.insert(candidate.id().clone()));
    finish_data_last_candidates(request, receiver_type, arguments, bases)
}

#[allow(clippy::result_large_err)]
fn finish_data_last_candidates(
    request: &mut CallResolverRequest<'_>,
    receiver_type: &TypeKind,
    arguments: &[CallArg],
    bases: Vec<ResolvedCallable>,
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    let mut candidates = Vec::new();
    for base in bases {
        check_query_step(request)?;
        let Some((group, parameter)) = data_last_receiver_coordinate(
            base.schema(),
            request.call_group,
            receiver_type,
            arguments,
        ) else {
            continue;
        };
        let id = DataLastCallableId::try_new(base.id().clone(), group, parameter, base.schema())
            .map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
        let schema = base.schema().clone();
        let origin = base.origin().clone();
        let equivalent_sources = base.equivalent_sources().to_vec();
        let authority = base.authority();
        candidates.push(ResolvedCallable::try_new(
            CallableCandidateId::DataLast(id),
            origin,
            Arc::new(schema),
            CallableInstantiation::DataLast {
                receiver: receiver_type.clone(),
                group,
                parameter,
            },
            equivalent_sources,
            authority,
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
    schema: &CallableSignatureSchema,
    current_group: CallableGroupIndex,
    receiver_type: &TypeKind,
    arguments: &[CallArg],
) -> Option<(CallableGroupIndex, CallableParameterIndex)> {
    if let Some(group) = schema.group(current_group)
        && let Some(parameter) = group.parameters().last()
        && let super::CallableParameterType::Exact(expected) = parameter.ty()
        && expected.accepts(receiver_type)
        && authored_argument_slot_count(arguments) + 1 == group.parameters().len()
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
    (expected.accepts(receiver_type) && call_shape_is_viable(schema, current_group, arguments))
        .then_some((next_group, parameter.index()))
}

fn authored_argument_slot_count(arguments: &[CallArg]) -> usize {
    arguments
        .iter()
        .map(|argument| match argument {
            CallArg::Spread { value } => match value.as_ref() {
                Expr::BracketSeq(items) => items.len(),
                Expr::NumericBracketSeq(sequence) => sequence.len(),
                _ => 1,
            },
            CallArg::Positional(_) | CallArg::Named { .. } => 1,
        })
        .sum()
}

#[allow(clippy::result_large_err)]
fn resolve_trait_method(
    request: &mut CallResolverRequest<'_>,
    receiver_type: &TypeKind,
    method: &CallableName,
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    match request
        .traits
        .resolve_method(receiver_type, method.as_str(), request.trait_predicates)
    {
        TraitMethodResolution::Missing => Ok(None),
        TraitMethodResolution::Inherent {
            implementation,
            method: selected,
        } => {
            let id = trait_callable_id(
                request,
                None,
                method,
                implementation.index(),
                TraitCallableSource::Inherent,
            )?;
            resolved_trait_method(request, receiver_type, id, &selected).map(Some)
        }
        TraitMethodResolution::Unique {
            witness,
            trait_id,
            method: selected,
        } => {
            let implementation = witness
                .and_then(|witness| request.traits.witness(witness))
                .map_or_else(|| trait_id.index(), |witness| witness.impl_id().index());
            let id = trait_callable_id(
                request,
                Some(trait_id),
                method,
                implementation,
                TraitCallableSource::Predicate,
            )?;
            resolved_trait_method(request, receiver_type, id, &selected).map(Some)
        }
        TraitMethodResolution::Ambiguous(candidates) => {
            let mut ids = Vec::with_capacity(candidates.len());
            for candidate in candidates {
                check_query_step(request)?;
                let implementation = candidate
                    .witness
                    .and_then(|witness| request.traits.witness(witness))
                    .map_or_else(
                        || candidate.trait_id.index(),
                        |witness| witness.impl_id().index(),
                    );
                ids.push(trait_callable_id(
                    request,
                    Some(candidate.trait_id),
                    method,
                    implementation,
                    TraitCallableSource::Predicate,
                )?);
            }
            ids.sort();
            ids.dedup();
            Err(ResolveCallError::AmbiguousTraitMethod {
                candidates: ids.into(),
            })
        }
    }
}

#[allow(clippy::result_large_err)]
fn trait_callable_id(
    request: &CallResolverRequest<'_>,
    trait_id: Option<crate::traits::TraitId>,
    method: &CallableName,
    implementation: usize,
    source: TraitCallableSource,
) -> Result<TraitCallableId, ResolveCallError> {
    let name = trait_id
        .and_then(|trait_id| request.traits.trait_name(trait_id))
        .unwrap_or("inherent");
    let name =
        CallableName::try_new(name).map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
    let trait_name =
        CallablePath::try_new([name]).map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
    let implementation = TraitImplementationIndex::try_from_usize(implementation)
        .map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
    Ok(TraitCallableId::new(
        trait_name,
        method.clone(),
        implementation,
        source,
    ))
}

#[allow(clippy::result_large_err)]
fn resolved_trait_method(
    request: &mut CallResolverRequest<'_>,
    receiver_type: &TypeKind,
    id: TraitCallableId,
    method: &crate::traits::TraitMethodImpl,
) -> Result<ResolvedCallTarget, ResolveCallError> {
    let result = request
        .traits
        .resolve_type_projections(method.return_type().clone(), request.trait_predicates)
        .map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
    let signature = method.call_signature(result);
    let schema = signature
        .callable_schema(
            EffectRow::closed(crate::effects::EffectSet::new()),
            super::CallableValidator::Trait(id.clone()),
            request.limits,
        )
        .map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
    let callable = ResolvedCallable::try_new(
        CallableCandidateId::TraitMethod(id.clone()),
        SignatureOrigin::Trait { id },
        Arc::new(schema),
        CallableInstantiation::Receiver {
            receiver: receiver_type.clone(),
        },
        Vec::new(),
        None,
        request.limits,
    )?;
    NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
        .map(ResolvedCallTarget::Candidates)
}

#[allow(clippy::result_large_err)]
fn resolved_language_method(
    request: &mut CallResolverRequest<'_>,
    id: CallableCandidateId,
    family: LanguageCallableFamily,
    schema: CallableSignatureSchema,
    instantiation: CallableInstantiation,
) -> Result<ResolvedCallTarget, ResolveCallError> {
    let callable = ResolvedCallable::try_new(
        id,
        SignatureOrigin::Language { family },
        Arc::new(schema),
        instantiation,
        Vec::new(),
        None,
        request.limits,
    )?;
    NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
        .map(ResolvedCallTarget::Candidates)
}

#[allow(clippy::result_large_err)]
fn resolve_selected_environment_method(
    request: &mut CallResolverRequest<'_>,
    receiver_type: &TypeKind,
    method: &CallableName,
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    check_query_step(request)?;
    let key = ReceiverMethodKey::new(receiver_type.clone(), method.clone());
    let Some(candidates) = request.world.environment().callable_catalog().method(&key) else {
        return Ok(None);
    };
    let mut resolved = Vec::with_capacity(candidates.len().get() as usize);
    for entry in candidates.as_slice() {
        check_query_step(request)?;
        resolved.push(resolve_catalog_record(
            entry.primary(),
            entry.equivalent_sources(),
            None,
            request,
        )?);
    }
    NonEmptyResolvedCandidates::try_new(resolved, request.limits)
        .map(ResolvedCallTarget::Candidates)
        .map(Some)
}

#[allow(
    clippy::result_large_err,
    clippy::too_many_lines,
    reason = "the ordered free-family chain is the canonical precedence table"
)]
fn resolve_free_call(
    request: &mut CallResolverRequest<'_>,
    path: &CallablePath,
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    check_query_step(request)?;
    if let FxResolution::Known(id) = FxCallableSignatureId::resolve(path) {
        let callable = ResolvedCallable::try_new(
            CallableCandidateId::Fx(id),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::Fx,
            },
            Arc::new(id.signature_schema()),
            CallableInstantiation::None,
            Vec::new(),
            None,
            request.limits,
        )?;
        return NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
            .map(ResolvedCallTarget::Candidates)
            .map(Some);
    }

    check_query_step(request)?;
    if let CallCallee::Free {
        enum_variant: Some(seed),
        ..
    } = &request.callee
    {
        let callable = ResolvedCallable::try_new(
            CallableCandidateId::EnumVariant(seed.id.clone()),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::EnumConstructor,
            },
            Arc::new(seed.schema.clone()),
            CallableInstantiation::ExpectedEnum {
                expected: seed.expected.clone(),
            },
            Vec::new(),
            None,
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
        let callable = ResolvedCallable::try_new(
            CallableCandidateId::Result(kind),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::ResultConstructor,
            },
            Arc::new(kind.instantiated_signature_schema(expected.as_ref())),
            CallableInstantiation::Result { kind, expected },
            Vec::new(),
            None,
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
        let callable = ResolvedCallable::try_new(
            CallableCandidateId::Option(kind),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::OptionConstructor,
            },
            Arc::new(kind.instantiated_signature_schema(expected.as_ref())),
            CallableInstantiation::Option { expected },
            Vec::new(),
            None,
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
        let callable = ResolvedCallable::try_new(
            CallableCandidateId::Builtin(id),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::Builtin,
            },
            schema,
            CallableInstantiation::None,
            Vec::new(),
            None,
            request.limits,
        )?;
        return NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
            .map(ResolvedCallTarget::Candidates)
            .map(Some);
    }

    check_query_step(request)?;
    if let Some(id) = AgentIntrinsicSignatureId::resolve(path) {
        let schema = Arc::new(id.signature_schema());
        let callable = ResolvedCallable::try_new(
            CallableCandidateId::Agent(id),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::Agent,
            },
            schema,
            CallableInstantiation::None,
            Vec::new(),
            None,
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
        let callable = ResolvedCallable::try_new(
            CallableCandidateId::Presentation(id),
            SignatureOrigin::Language {
                family: LanguageCallableFamily::Presentation,
            },
            Arc::new(schema),
            CallableInstantiation::None,
            Vec::new(),
            None,
            request.limits,
        )?;
        return NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
            .map(ResolvedCallTarget::Candidates)
            .map(Some);
    }

    check_query_step(request)?;
    if let [name] = path.segments()
        && let Some(binding) = request.lexical.binding(name)
    {
        check_query_step(request)?;
        return resolve_lexical_binding(name, binding, request.limits).map(Some);
    }

    check_query_step(request)?;
    let project_path = ProjectCallablePath::new(
        request.symbols.world().package().clone(),
        request.current_module.clone(),
        path.clone(),
    );
    if let Some(binding) = request
        .world
        .environment()
        .callable_catalog()
        .project_binding(&project_path)
    {
        check_query_step(request)?;
        return resolve_project_binding(binding, &project_path, request).map(Some);
    }

    check_query_step(request)?;
    if let Some(candidates) = request.world.environment().callable_catalog().free(path) {
        let mut resolved = Vec::with_capacity(candidates.len().get() as usize);
        for entry in candidates.as_slice() {
            check_query_step(request)?;
            resolved.push(resolve_catalog_record(
                entry.primary(),
                entry.equivalent_sources(),
                None,
                request,
            )?);
        }
        return NonEmptyResolvedCandidates::try_new(resolved, request.limits)
            .map(ResolvedCallTarget::Candidates)
            .map(Some);
    }

    check_query_step(request)?;
    if let Some(id) = PromotionCallableId::resolve(path) {
        let callable = ResolvedCallable::try_new(
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
            None,
            request.limits,
        )?;
        return NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
            .map(ResolvedCallTarget::Candidates)
            .map(Some);
    }

    Ok(None)
}

#[allow(clippy::result_large_err)]
fn resolve_lexical_binding(
    name: &CallableName,
    binding: &LexicalCallBinding,
    limits: &CallableLimits,
) -> Result<ResolvedCallTarget, ResolveCallError> {
    match binding {
        LexicalCallBinding::Callable {
            id,
            schema,
            effects,
        } => {
            let _ = effects;
            let callable = ResolvedCallable::try_new(
                CallableCandidateId::Local(id.clone()),
                SignatureOrigin::Lexical { id: id.clone() },
                Arc::clone(schema),
                CallableInstantiation::None,
                Vec::new(),
                None,
                limits,
            )?;
            NonEmptyResolvedCandidates::try_new(vec![callable], limits)
                .map(ResolvedCallTarget::Candidates)
        }
        LexicalCallBinding::FunctionValue(seed) => resolve_function_value(seed, limits),
        LexicalCallBinding::Speaker { id, schema } => {
            let callable = ResolvedCallable::try_new(
                CallableCandidateId::Speaker(id.clone()),
                SignatureOrigin::Language {
                    family: LanguageCallableFamily::Speaker,
                },
                Arc::clone(schema),
                CallableInstantiation::None,
                Vec::new(),
                None,
                limits,
            )?;
            NonEmptyResolvedCandidates::try_new(vec![callable], limits)
                .map(ResolvedCallTarget::Candidates)
        }
        LexicalCallBinding::NonCallable { ty } => Ok(ResolvedCallTarget::NonCallable(
            ResolvedNonCallableTarget::new(
                NonCallableSource::Lexical { name: name.clone() },
                ty.clone(),
            ),
        )),
    }
}

#[allow(clippy::result_large_err)]
fn resolve_project_binding(
    binding: &ProjectNameBinding,
    path: &ProjectCallablePath,
    request: &mut CallResolverRequest<'_>,
) -> Result<ResolvedCallTarget, ResolveCallError> {
    match binding {
        ProjectNameBinding::Callable(declaration) => {
            let record = request
                .world
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
            let callable = resolve_catalog_record(&record, &[], Some(path), request)?;
            NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
                .map(ResolvedCallTarget::Candidates)
        }
        ProjectNameBinding::Environment(id) => {
            let record = request
                .world
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
            let callable = resolve_catalog_record(&record, &[], Some(path), request)?;
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

#[allow(clippy::result_large_err)]
fn resolve_catalog_record(
    record: &CallableRecord,
    equivalent_sources: &[EquivalentCallableSource],
    project_path: Option<&ProjectCallablePath>,
    request: &mut CallResolverRequest<'_>,
) -> Result<ResolvedCallable, ResolveCallError> {
    check_query_step(request)?;
    let origin = match record.id() {
        CallableCandidateId::Project(declaration) => SignatureOrigin::Project {
            declaration: declaration.clone(),
            path: project_path
                .cloned()
                .ok_or(ResolveCallError::InvalidResolvedCallable)?,
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
    let reachable = match record.id() {
        CallableCandidateId::Project(id) => request
            .world
            .environment()
            .callable_catalog()
            .project_record(id)
            .is_some_and(|accepted| accepted.as_ref() == record),
        CallableCandidateId::Environment(id) => request
            .world
            .environment()
            .callable_catalog()
            .environment_record(id)
            .is_some_and(|accepted| accepted.as_ref() == record),
        _ => false,
    };
    if !reachable {
        return Err(corrupt(
            record.key().clone(),
            super::CorruptCallableCatalogReason::MissingRecord,
        ));
    }
    ResolvedCallable::try_new(
        record.id().clone(),
        origin,
        Arc::new(record.schema().clone()),
        CallableInstantiation::None,
        equivalent_sources.to_vec(),
        Some(record.authority()),
        request.limits,
    )
}

fn record_key(path: &ProjectCallablePath) -> CallableLookupKey {
    CallableLookupKey::Free(path.path().clone())
}

fn corrupt(
    key: CallableLookupKey,
    reason: super::CorruptCallableCatalogReason,
) -> ResolveCallError {
    ResolveCallError::CorruptCatalog { key, reason }
}

#[allow(clippy::result_large_err)]
fn check_query_step(request: &mut CallResolverRequest<'_>) -> Result<(), ResolveCallError> {
    if request.cancellation.load(Ordering::Relaxed) {
        return Err(ResolveCallError::Cancelled);
    }
    request.work.charge(1).map_err(ResolveCallError::Work)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignatureOrigin {
    Project {
        declaration: CallableDeclarationId,
        path: ProjectCallablePath,
    },
    Standard {
        owner: super::StandardEnvironmentId,
        id: super::EnvironmentCallableId,
    },
    Adapter {
        package: super::AdapterPackageId,
        id: super::EnvironmentCallableId,
    },
    Language {
        family: LanguageCallableFamily,
    },
    Trait {
        id: TraitCallableId,
    },
    Lexical {
        id: LocalCallableId,
    },
    FunctionValue {
        id: FunctionValueSignatureId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedCallable {
    id: CallableCandidateId,
    origin: SignatureOrigin,
    schema: Arc<CallableSignatureSchema>,
    instantiation: CallableInstantiation,
    equivalent_sources: Arc<[EquivalentCallableSource]>,
    authority: Option<CallableAuthorityRank>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableInstantiation {
    None,
    ExpectedEnum {
        expected: TypeKind,
    },
    Result {
        kind: super::ResultConstructorKind,
        expected: Option<TypeKind>,
    },
    Option {
        expected: Option<TypeKind>,
    },
    Character {
        owner: ResolvedCharacterOwner,
    },
    Receiver {
        receiver: TypeKind,
    },
    Curried {
        base: CallableCandidateId,
        group: CallableGroupIndex,
    },
    DataLast {
        receiver: TypeKind,
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
}

impl ResolvedCallable {
    #[allow(
        clippy::result_large_err,
        reason = "the typed query error preserves the offending candidate identity"
    )]
    pub fn try_new(
        id: CallableCandidateId,
        origin: SignatureOrigin,
        schema: Arc<CallableSignatureSchema>,
        instantiation: CallableInstantiation,
        equivalent_sources: Vec<EquivalentCallableSource>,
        authority: Option<CallableAuthorityRank>,
        limits: &CallableLimits,
    ) -> Result<Self, ResolveCallError> {
        if equivalent_sources.len().saturating_add(1) > limits.max_candidates_per_call()
            || !origin_matches(&id, &origin, authority)
            || !instantiation_matches(&id, &instantiation)
        {
            return Err(ResolveCallError::InvalidResolvedCallable);
        }
        let mut ids = std::collections::HashSet::new();
        ids.insert(id.clone());
        if equivalent_sources
            .iter()
            .any(|source| !ids.insert(source.id().clone()))
        {
            return Err(ResolveCallError::InvalidResolvedCallable);
        }
        if let (
            CallableCandidateId::Curried(curried),
            CallableInstantiation::Curried { base, group },
        ) = (&id, &instantiation)
        {
            debug_assert_eq!(curried.base(), base);
            debug_assert_eq!(curried.next_group(), *group);
            if schema.group(*group).is_none() {
                return Err(ResolveCallError::InvalidCallGroup {
                    candidate: base.clone(),
                    group: *group,
                });
            }
        }
        Ok(Self {
            id,
            origin,
            schema,
            instantiation,
            equivalent_sources: equivalent_sources.into(),
            authority,
        })
    }
    pub const fn id(&self) -> &CallableCandidateId {
        &self.id
    }
    pub const fn origin(&self) -> &SignatureOrigin {
        &self.origin
    }
    pub fn schema(&self) -> &CallableSignatureSchema {
        &self.schema
    }
    pub const fn instantiation(&self) -> &CallableInstantiation {
        &self.instantiation
    }
    pub fn equivalent_sources(&self) -> &[EquivalentCallableSource] {
        &self.equivalent_sources
    }
    pub const fn authority(&self) -> Option<CallableAuthorityRank> {
        self.authority
    }

    #[allow(
        clippy::result_large_err,
        reason = "the typed resolver error preserves the complete offending candidate"
    )]
    pub(crate) fn try_curried(
        &self,
        group: CallableGroupIndex,
        limits: &CallableLimits,
    ) -> Result<Self, ResolveCallError> {
        if self.schema.group(group).is_none() {
            return Err(ResolveCallError::InvalidCallGroup {
                candidate: self.id.clone(),
                group,
            });
        }
        let base = match &self.id {
            CallableCandidateId::Curried(id) => id.base().clone(),
            CallableCandidateId::DataLast(id) => id.callable().clone(),
            id => id.clone(),
        };
        let id = CurriedCallableId::try_new(base.clone(), group)
            .map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
        Self::try_new(
            CallableCandidateId::Curried(id),
            self.origin.clone(),
            Arc::clone(&self.schema),
            CallableInstantiation::Curried { base, group },
            self.equivalent_sources.to_vec(),
            self.authority,
            limits,
        )
    }
}

fn origin_matches(
    id: &CallableCandidateId,
    origin: &SignatureOrigin,
    authority: Option<CallableAuthorityRank>,
) -> bool {
    if let CallableCandidateId::Curried(id) = id {
        return origin_matches(id.base(), origin, authority);
    }
    if let CallableCandidateId::DataLast(id) = id {
        return origin_matches(id.callable(), origin, authority);
    }
    match (id, origin, authority) {
        (
            CallableCandidateId::Project(id),
            SignatureOrigin::Project { declaration, .. },
            Some(CallableAuthorityRank::Project),
        ) => id == declaration,
        (
            CallableCandidateId::Environment(id),
            SignatureOrigin::Standard { id: origin_id, .. },
            Some(CallableAuthorityRank::Standard),
        )
        | (
            CallableCandidateId::Environment(id),
            SignatureOrigin::Adapter { id: origin_id, .. },
            Some(CallableAuthorityRank::Adapter),
        ) => id == origin_id,
        (CallableCandidateId::TraitMethod(id), SignatureOrigin::Trait { id: origin_id }, None) => {
            id == origin_id
        }
        (CallableCandidateId::Local(id), SignatureOrigin::Lexical { id: origin_id }, None) => {
            id == origin_id
        }
        (
            CallableCandidateId::FunctionValue(id),
            SignatureOrigin::FunctionValue { id: origin_id },
            None,
        ) => id == origin_id,
        (id, SignatureOrigin::Language { family }, None) => language_origin_matches(id, *family),
        _ => false,
    }
}

const fn language_origin_matches(id: &CallableCandidateId, family: LanguageCallableFamily) -> bool {
    matches!(
        (id, family),
        (CallableCandidateId::Fx(_), LanguageCallableFamily::Fx)
            | (
                CallableCandidateId::EnumVariant(_),
                LanguageCallableFamily::EnumConstructor
            )
            | (
                CallableCandidateId::Result(_),
                LanguageCallableFamily::ResultConstructor
            )
            | (
                CallableCandidateId::Option(_),
                LanguageCallableFamily::OptionConstructor
            )
            | (
                CallableCandidateId::Builtin(_),
                LanguageCallableFamily::Builtin
            )
            | (CallableCandidateId::Agent(_), LanguageCallableFamily::Agent)
            | (
                CallableCandidateId::Presentation(_),
                LanguageCallableFamily::Presentation
            )
            | (
                CallableCandidateId::Dialogue(_),
                LanguageCallableFamily::Dialogue
            )
            | (
                CallableCandidateId::CollectionMethod(_),
                LanguageCallableFamily::CollectionMethod
            )
            | (
                CallableCandidateId::PresentationHandleMethod(_),
                LanguageCallableFamily::PresentationHandleMethod
            )
            | (
                CallableCandidateId::IntegerMethod(_),
                LanguageCallableFamily::IntegerMethod
            )
            | (
                CallableCandidateId::DomainMethod(_),
                LanguageCallableFamily::DomainMethod
            )
            | (
                CallableCandidateId::CapacityMethod(_),
                LanguageCallableFamily::CapacityMethod
            )
            | (CallableCandidateId::Drop(_), LanguageCallableFamily::Drop)
            | (
                CallableCandidateId::Promotion(
                    PromotionCallableId::Promote | PromotionCallableId::PromoteUnchecked
                ),
                LanguageCallableFamily::Promote
            )
            | (
                CallableCandidateId::Promotion(PromotionCallableId::Assume),
                LanguageCallableFamily::Assume
            )
            | (
                CallableCandidateId::Speaker(_),
                LanguageCallableFamily::Speaker
            )
    )
}

fn instantiation_matches(id: &CallableCandidateId, instantiation: &CallableInstantiation) -> bool {
    match (id, instantiation) {
        (CallableCandidateId::Result(id_kind), CallableInstantiation::Result { kind, .. }) => {
            id_kind == kind
        }
        (CallableCandidateId::Curried(id), CallableInstantiation::Curried { base, group }) => {
            id.base() == base && id.next_group() == *group
        }
        (
            CallableCandidateId::DataLast(id),
            CallableInstantiation::DataLast {
                group, parameter, ..
            },
        ) => id.receiver_group() == *group && id.receiver_parameter() == *parameter,
        (CallableCandidateId::EnumVariant(_), CallableInstantiation::ExpectedEnum { .. })
        | (CallableCandidateId::Option(_), CallableInstantiation::Option { .. })
        | (
            CallableCandidateId::Presentation(_) | CallableCandidateId::Dialogue(_),
            CallableInstantiation::Character { .. } | CallableInstantiation::None,
        )
        | (
            CallableCandidateId::CollectionMethod(_)
            | CallableCandidateId::PresentationHandleMethod(_)
            | CallableCandidateId::IntegerMethod(_)
            | CallableCandidateId::DomainMethod(_)
            | CallableCandidateId::TraitMethod(_)
            | CallableCandidateId::CapacityMethod(_),
            CallableInstantiation::Receiver { .. },
        )
        | (
            CallableCandidateId::Fx(_)
            | CallableCandidateId::Builtin(_)
            | CallableCandidateId::Agent(_)
            | CallableCandidateId::Project(_)
            | CallableCandidateId::Environment(_)
            | CallableCandidateId::Local(_)
            | CallableCandidateId::FunctionValue(_)
            | CallableCandidateId::Drop(_)
            | CallableCandidateId::Promotion(_)
            | CallableCandidateId::Speaker(_),
            CallableInstantiation::None,
        ) => true,
        _ => false,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFunctionValue {
    id: FunctionValueSignatureId,
    callable: ResolvedCallable,
    function_type: TypeKind,
    effect_callable: Option<CallableId>,
    source_candidate: Option<CallableCandidateId>,
    current_group: CallableGroupIndex,
}
impl ResolvedFunctionValue {
    #[allow(
        clippy::result_large_err,
        reason = "the typed query error preserves the offending candidate identity"
    )]
    pub fn try_new(
        id: FunctionValueSignatureId,
        callable: ResolvedCallable,
        function_type: TypeKind,
        effect_callable: Option<CallableId>,
        source_candidate: Option<CallableCandidateId>,
        current_group: CallableGroupIndex,
    ) -> Result<Self, ResolveCallError> {
        if !matches!(function_type, TypeKind::Function { .. })
            || callable.id() != &CallableCandidateId::FunctionValue(id.clone())
            || callable.schema().group(current_group).is_none()
        {
            return Err(ResolveCallError::InvalidResolvedCallable);
        }
        Ok(Self {
            id,
            callable,
            function_type,
            effect_callable,
            source_candidate,
            current_group,
        })
    }
    pub const fn id(&self) -> &FunctionValueSignatureId {
        &self.id
    }
    pub const fn callable(&self) -> &ResolvedCallable {
        &self.callable
    }
    pub const fn function_type(&self) -> &TypeKind {
        &self.function_type
    }
    pub const fn effect_callable(&self) -> Option<&CallableId> {
        self.effect_callable.as_ref()
    }
    pub const fn source_candidate(&self) -> Option<&CallableCandidateId> {
        self.source_candidate.as_ref()
    }
    pub const fn current_group(&self) -> CallableGroupIndex {
        self.current_group
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "resolver products preserve the exact typed function-value contract"
)]
pub enum ResolvedCallTarget {
    Candidates(NonEmptyResolvedCandidates),
    FunctionValue(ResolvedFunctionValue),
    NonCallable(ResolvedNonCallableTarget),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NonEmptyResolvedCandidates {
    candidates: Arc<[ResolvedCallable]>,
}
impl NonEmptyResolvedCandidates {
    #[allow(
        clippy::result_large_err,
        reason = "the typed query error preserves the offending candidate identity"
    )]
    pub(crate) fn try_new(
        candidates: Vec<ResolvedCallable>,
        limits: &CallableLimits,
    ) -> Result<Self, ResolveCallError> {
        if candidates.is_empty() {
            return Err(ResolveCallError::InvalidResolvedCallable);
        }
        if candidates.len() > limits.max_candidates_per_call() {
            return Err(ResolveCallError::CandidateLimit {
                actual: candidates.len(),
                limit: limits.max_candidates_per_call(),
            });
        }
        let mut ids = std::collections::HashSet::new();
        if candidates
            .iter()
            .any(|candidate| !ids.insert(candidate.id().clone()))
        {
            return Err(ResolveCallError::InvalidResolvedCallable);
        }
        Ok(Self {
            candidates: candidates.into(),
        })
    }
    pub fn first(&self) -> &ResolvedCallable {
        &self.candidates[0]
    }
    pub fn as_slice(&self) -> &[ResolvedCallable] {
        &self.candidates
    }
    pub fn len(&self) -> NonZeroU32 {
        let len = u32::try_from(self.candidates.len()).unwrap_or(u32::MAX);
        NonZeroU32::new(len).unwrap_or(NonZeroU32::MIN)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedNonCallableTarget {
    source: NonCallableSource,
    ty: TypeKind,
}
impl ResolvedNonCallableTarget {
    pub fn new(source: NonCallableSource, ty: TypeKind) -> Self {
        Self { source, ty }
    }
    pub const fn source(&self) -> &NonCallableSource {
        &self.source
    }
    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NonCallableSource {
    Lexical { name: CallableName },
    Project { path: ProjectCallablePath },
    EvaluatedExpression,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "the accepted query outcome retains its exact typed resolver product"
)]
pub enum ResolveCallOutcome {
    Resolved(ResolvedCallTarget),
    Missing(UnknownCallTarget),
    Rejected(ResolveCallError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnknownCallTarget {
    kind: UnknownCallKind,
    path: Option<CallablePath>,
    receiver: Option<TypeKind>,
    method: Option<CallableName>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnknownCallKind {
    Free,
    Method,
    Dialogue,
}
impl UnknownCallTarget {
    pub fn new(
        kind: UnknownCallKind,
        path: Option<CallablePath>,
        receiver: Option<TypeKind>,
        method: Option<CallableName>,
    ) -> Self {
        Self {
            kind,
            path,
            receiver,
            method,
        }
    }
    pub const fn kind(&self) -> UnknownCallKind {
        self.kind
    }
    pub const fn path(&self) -> Option<&CallablePath> {
        self.path.as_ref()
    }
    pub const fn receiver(&self) -> Option<&TypeKind> {
        self.receiver.as_ref()
    }
    pub const fn method(&self) -> Option<&CallableName> {
        self.method.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedCharacterOwner {
    character: CharacterId,
    source: CharacterOwnerSource,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterOwnerSource {
    EntityReference,
    LexicalBinding { name: CallableName },
    ProjectBinding { path: ProjectCallablePath },
    ExternalOwner,
    SpeakerValue,
    SpeakerPresetValue,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterOwnerResolution {
    Known(ResolvedCharacterOwner),
    Missing,
    NonCharacter { actual: TypeKind },
    UnknownExternalOwner,
    UnknownPart { part: CharacterPartId },
    Poisoned,
}
impl ResolvedCharacterOwner {
    pub fn new(character: CharacterId, source: CharacterOwnerSource) -> Self {
        Self { character, source }
    }
    pub const fn character(&self) -> &CharacterId {
        &self.character
    }
    pub const fn source(&self) -> &CharacterOwnerSource {
        &self.source
    }
}
