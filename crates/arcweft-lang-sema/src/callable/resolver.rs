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
use arcweft_source::{SourceDocumentIdentity, SourceSpan};

use crate::{
    checker::TypeExpressionId, effect_model::CallableId, effect_row::EffectRow,
    registration::RegisteredSemanticWorld, traits::TraitCatalog, types::TypeKind,
};

use super::{
    CallableAuthorityRank, CallableCandidateId, CallableGroupIndex, CallableLimits,
    CallableLookupKey, CallableName, CallableParameterIndex, CallablePath, CallableRecord,
    CallableSignatureSchema, EnvironmentCallableOwner, EquivalentCallableSource,
    FunctionValueSignatureId, LanguageCallableFamily, LocalCallableId, ProjectCallablePath,
    ProjectNameBinding, PromotionCallableId, ReceiverMethodKey, ResolveCallError, ResolverWork,
    TraitCallableId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(
    dead_code,
    reason = "dialogue and function-value inputs belong to the following ordered resolver cuts"
)]
pub(crate) enum CallCallee<'a> {
    Free {
        path: &'a CallablePath,
    },
    Selected {
        receiver_expression: TypeExpressionId,
        receiver_type: &'a TypeKind,
        method: &'a CallableName,
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
#[allow(
    dead_code,
    reason = "function-value seeds are consumed by the following ordered resolver cut"
)]
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
    source: CallSourceContext<'a>,
    call_group: CallableGroupIndex,
    expression: TypeExpressionId,
    cancellation: &'a AtomicBool,
    work: &'a mut ResolverWork,
    limits: &'a CallableLimits,
}

#[allow(
    dead_code,
    reason = "function-value seeds are consumed by the following ordered resolver cut"
)]
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

    pub(crate) const fn id(&self) -> &FunctionValueSignatureId {
        &self.id
    }
    pub(crate) const fn ty(&self) -> &TypeKind {
        &self.ty
    }
    pub(crate) const fn schema(&self) -> &CallableSignatureSchema {
        &self.schema
    }
    pub(crate) const fn effect_callable(&self) -> Option<&CallableId> {
        self.effect_callable.as_ref()
    }
    pub(crate) const fn source_candidate(&self) -> Option<&CallableCandidateId> {
        self.source_candidate.as_ref()
    }
    pub(crate) const fn next_group(&self) -> CallableGroupIndex {
        self.next_group
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

    pub(crate) fn from_non_callable_bindings(
        bindings: impl IntoIterator<Item = (CallableName, TypeKind)>,
    ) -> Self {
        Self {
            bindings: bindings
                .into_iter()
                .map(|(name, ty)| (name, LexicalCallBinding::NonCallable { ty }))
                .collect(),
        }
    }

    #[cfg(test)]
    #[allow(
        dead_code,
        reason = "crate-owned corrupt and lexical resolver fixtures use this mutation boundary incrementally"
    )]
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
        CallCallee::Free { path } => {
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
            ..
        } => {
            let receiver_type = receiver_type.clone();
            let method = method.clone();
            match resolve_selected_environment_method(&mut request, &receiver_type, &method) {
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
        CallCallee::Dialogue { .. } => ResolveCallOutcome::Missing(UnknownCallTarget::new(
            UnknownCallKind::Dialogue,
            None,
            None,
            None,
        )),
        CallCallee::FunctionValue { .. } => ResolveCallOutcome::Missing(UnknownCallTarget::new(
            UnknownCallKind::Free,
            None,
            None,
            None,
        )),
    }
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

#[allow(clippy::result_large_err)]
fn resolve_free_call(
    request: &mut CallResolverRequest<'_>,
    path: &CallablePath,
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
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
    let Some(candidates) = request.world.environment().callable_catalog().free(path) else {
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
        LexicalCallBinding::FunctionValue(seed) => {
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
}

fn origin_matches(
    id: &CallableCandidateId,
    origin: &SignatureOrigin,
    authority: Option<CallableAuthorityRank>,
) -> bool {
    if let CallableCandidateId::Curried(id) = id {
        return origin_matches(id.base(), origin, authority);
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
            | (
                CallableCandidateId::DataLast(_),
                LanguageCallableFamily::DataLast
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
