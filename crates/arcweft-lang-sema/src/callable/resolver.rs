//! Validated callable resolver requests and products.

use std::{
    collections::HashMap,
    num::NonZeroU32,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use arcweft_character::id::CharacterId;
use arcweft_lang_hir::symbol::{CallableDeclarationId, ProjectSymbolTable};
use arcweft_lang_syntax::ast::{common::TextRange, module_path::CanonicalModulePath};
use arcweft_lang_syntax::expr::{CallArg, Expr};
use arcweft_source::{SourceDocumentIdentity, SourceSpan};

use crate::{
    checker::TypeExpressionId,
    effect_model::CallableId,
    effect_row::EffectRow,
    env::TypeCheckEnv,
    nominal::ResolvedAssociatedTypeReceiver,
    registration::RegisteredSemanticWorld,
    traits::{TraitCatalog, TraitMethodResolution, TraitPredicate},
    types::TypeKind,
};

use super::{
    AgentIntrinsicSignatureId, AssociatedResolverStep, BuiltinCallableId, CallableAuthorityRank,
    CallableCandidateId, CallableGroupIndex, CallableLimits, CallableLookupKey, CallableName,
    CallableParameterIndex, CallablePath, CallableRecord, CallableSignatureSchema,
    CapacityMethodId, CollectionMethodId, CurriedCallableId, DataLastCallableId, DomainMethodId,
    DropCallableId, EnvironmentCallableId, EnvironmentCallableKind, EnvironmentCallableOwner,
    EquivalentCallableSource, FunctionValueSignatureId, FxCallableSignatureId, FxResolution,
    IntegerMethodId, LanguageCallableFamily, LocalCallableId, OptionConstructorKind,
    PresentationCallableId, PresentationHandleMethodId, ProjectCallablePath, ProjectNameBinding,
    PromotionCallableId, ReceiverMethodKey, ResolveCallError, ResolverWork, ResultConstructorKind,
    SpeakerCallableId, StageMethodId, StandardEnvironmentId, TraitCallableId, TraitCallableSource,
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
    AssociatedType {
        receiver: ResolvedAssociatedTypeReceiver<'a>,
        member: &'a CallableName,
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

/// Proof that a method receiver was evaluated as a value expression.
#[derive(Clone, Copy, Debug)]
struct EvaluatedReceiver<'a> {
    _expression: TypeExpressionId,
    ty: &'a TypeKind,
}

impl<'a> EvaluatedReceiver<'a> {
    const fn new(expression: TypeExpressionId, ty: &'a TypeKind) -> Self {
        Self {
            _expression: expression,
            ty,
        }
    }

    const fn ty(self) -> &'a TypeKind {
        self.ty
    }

    fn value_instantiation(self) -> CallableInstantiation {
        let Self { ty, .. } = self;
        CallableInstantiation::Receiver {
            receiver: ty.clone(),
        }
    }

    fn data_last_instantiation(
        self,
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    ) -> CallableInstantiation {
        let Self { ty, .. } = self;
        CallableInstantiation::DataLast {
            receiver: ty.clone(),
            group,
            parameter,
        }
    }
}

/// Receiver role admitted by trait-method resolution.
#[derive(Clone, Copy, Debug)]
enum MethodReceiver<'a> {
    Value(EvaluatedReceiver<'a>),
    Associated(ResolvedAssociatedTypeReceiver<'a>),
}

impl MethodReceiver<'_> {
    const fn ty(&self) -> &TypeKind {
        match self {
            Self::Value(receiver) => receiver.ty,
            Self::Associated(receiver) => receiver.ty(),
        }
    }

    fn instantiation(self) -> CallableInstantiation {
        match self {
            Self::Value(receiver) => receiver.value_instantiation(),
            Self::Associated(receiver) => CallableInstantiation::TypeReceiver {
                receiver: TypeReceiverInstantiation::from_resolved(receiver),
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum CallResolverAuthority<'a> {
    Accepted {
        current_module: &'a CanonicalModulePath,
        symbols: &'a ProjectSymbolTable,
        world: &'a RegisteredSemanticWorld,
    },
    Detached {
        environment: &'a TypeCheckEnv,
    },
}

enum TypedEnvironmentMethodCandidate<'a> {
    Accepted {
        record: &'a CallableRecord,
        equivalent_sources: Vec<EquivalentCallableSource>,
    },
    Detached {
        id: EnvironmentCallableId,
        schema: Arc<CallableSignatureSchema>,
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
    continuation_base: Option<ResolvedCallable>,
    next_group: CallableGroupIndex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CallSourceContext<'a> {
    Accepted {
        document: &'a SourceDocumentIdentity,
        call_span: Option<&'a SourceSpan>,
        callee_span: Option<&'a SourceSpan>,
    },
    Detached {
        source_len: usize,
        call_range: Option<TextRange>,
        callee_range: Option<TextRange>,
    },
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
pub(crate) enum LexicalCallBinding {
    Callable {
        id: LocalCallableId,
        schema: Arc<CallableSignatureSchema>,
        effects: EffectRow,
    },
    FunctionValue(Box<ResolvedFunctionValueSeed>),
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
    authority: CallResolverAuthority<'a>,
    lexical: &'a LexicalCallableScope,
    expected: Option<&'a TypeKind>,
    traits: &'a TraitCatalog,
    trait_predicates: &'a [TraitPredicate],
    source: CallSourceContext<'a>,
    call_group: CallableGroupIndex,
    expression: TypeExpressionId,
    cancellation: &'a AtomicBool,
    work: &'a mut ResolverWork,
    signature_work: Option<&'a mut super::SignatureQueryWorkMeter>,
    signature_control: Option<&'a dyn SignatureQueryStepControl>,
    limits: &'a CallableLimits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SignatureQueryStep {
    SurfaceTraversal,
    Resolver,
    CandidateMaterialization,
    CandidateProbe,
    CandidateArgumentProbe,
    CandidateComparison,
    SelectedReplay,
}

pub(crate) trait SignatureQueryStepControl {
    fn check_signature_query_step(&self, step: SignatureQueryStep) -> Result<(), ResolveCallError>;
}

impl ResolvedFunctionValueSeed {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        id: FunctionValueSignatureId,
        ty: TypeKind,
        schema: CallableSignatureSchema,
        effect_callable: Option<CallableId>,
        continuation_base: Option<ResolvedCallable>,
        next_group: CallableGroupIndex,
    ) -> Self {
        Self {
            id,
            ty,
            schema,
            effect_callable,
            continuation_base,
            next_group,
        }
    }
}

impl<'a> CallSourceContext<'a> {
    pub(crate) const fn accepted(
        document: &'a SourceDocumentIdentity,
        call_span: Option<&'a SourceSpan>,
        callee_span: Option<&'a SourceSpan>,
    ) -> Self {
        Self::Accepted {
            document,
            call_span,
            callee_span,
        }
    }

    pub(crate) const fn detached(
        source_len: usize,
        call_range: Option<TextRange>,
        callee_range: Option<TextRange>,
    ) -> Self {
        Self::Detached {
            source_len,
            call_range,
            callee_range,
        }
    }

    fn validate(&self, limits: &CallableLimits) -> Result<(), ResolveCallError> {
        let exceeds_source_limit = match self {
            Self::Accepted { document, .. } => {
                document.source_len() > u64::try_from(limits.max_source_bytes()).unwrap_or(u64::MAX)
            }
            Self::Detached { source_len, .. } => *source_len > limits.max_source_bytes(),
        };
        if exceeds_source_limit {
            let actual = match self {
                Self::Accepted { document, .. } => {
                    usize::try_from(document.source_len()).unwrap_or(usize::MAX)
                }
                Self::Detached { source_len, .. } => *source_len,
            };
            return Err(ResolveCallError::Work(
                super::CallableQueryLimitError::SourceBytes {
                    actual,
                    limit: limits.max_source_bytes(),
                },
            ));
        }

        let ranges_are_valid = match self {
            Self::Accepted {
                document,
                call_span,
                callee_span,
            } => call_span
                .iter()
                .copied()
                .chain(callee_span.iter().copied())
                .all(|span| source_span_is_valid(document, span)),
            Self::Detached {
                source_len,
                call_range,
                callee_range,
            } => call_range
                .iter()
                .chain(callee_range.iter())
                .all(|range| range.start() <= range.end() && range.end() <= *source_len),
        };
        if !ranges_are_valid {
            return Err(ResolveCallError::InvalidSourceSpan);
        }
        Ok(())
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

impl<'a> CallResolverAuthority<'a> {
    pub(crate) const fn accepted(
        current_module: &'a CanonicalModulePath,
        symbols: &'a ProjectSymbolTable,
        world: &'a RegisteredSemanticWorld,
    ) -> Self {
        Self::Accepted {
            current_module,
            symbols,
            world,
        }
    }

    pub(crate) const fn detached(environment: &'a TypeCheckEnv) -> Self {
        Self::Detached { environment }
    }

    /// Reports whether a multi-segment path belongs to the ordinary free-call
    /// namespace before an ambiguous dot receiver is tried as a nominal type.
    ///
    /// This lookup never materializes a candidate or schema. The subsequent
    /// free-call resolver remains the sole candidate authority.
    pub(crate) fn qualified_free_path_is_present(
        self,
        path: &CallablePath,
        has_expected_enum_variant: bool,
    ) -> Result<bool, ResolveCallError> {
        if path.len() < 2 {
            return Ok(false);
        }
        if matches!(FxCallableSignatureId::resolve(path), FxResolution::Known(_))
            || has_expected_enum_variant
            || ResultConstructorKind::resolve(path).is_some()
            || OptionConstructorKind::resolve(path).is_some()
            || BuiltinCallableId::resolve(path).is_some()
            || AgentIntrinsicSignatureId::resolve(path).is_some()
            || PresentationCallableId::resolve(path).is_some()
            || PromotionCallableId::resolve(path).is_some()
        {
            return Ok(true);
        }

        match self {
            Self::Accepted {
                current_module,
                symbols,
                world,
            } => {
                let catalog = world.environment().callable_catalog();
                let project_path = ProjectCallablePath::new(
                    symbols.world().package().clone(),
                    current_module.clone(),
                    path.clone(),
                );
                if catalog.project_binding(&project_path).is_some() {
                    return Ok(true);
                }
                catalog
                    .validated_free(path)
                    .map(|candidates| candidates.is_some())
                    .map_err(|reason| corrupt(CallableLookupKey::Free(path.clone()), reason))
            }
            Self::Detached { environment } => {
                Ok(environment.function_type(&path.dotted_name()).is_some())
            }
        }
    }

    fn validate(
        self,
        callee: &CallCallee<'_>,
        source: &CallSourceContext<'_>,
    ) -> Result<(), ResolveCallError> {
        if let CallCallee::AssociatedType { receiver, .. } = callee
            && (receiver.product().recovered() != receiver.ty()
                || receiver.root().recovered() != Some(receiver.ty()))
        {
            return Err(ResolveCallError::InvalidResolvedCallable);
        }
        match (self, source) {
            (
                Self::Accepted {
                    current_module,
                    symbols,
                    world,
                },
                CallSourceContext::Accepted { document, .. },
            ) => {
                if symbols.world() != world.symbols().world()
                    || symbols.revision() != world.symbols().revision()
                    || symbols.world() != world.environment().world()
                    || symbols.revision() != world.environment().symbol_revision()
                {
                    return Err(ResolveCallError::WorldMismatch);
                }
                if symbols.source_identity(current_module) != Some(*document) {
                    return Err(ResolveCallError::SourceIdentityMismatch);
                }
                Ok(())
            }
            (Self::Accepted { .. }, CallSourceContext::Detached { .. })
            | (Self::Detached { .. }, CallSourceContext::Accepted { .. }) => {
                Err(ResolveCallError::SourceIdentityMismatch)
            }
            (Self::Detached { .. }, CallSourceContext::Detached { .. })
                if matches!(callee, CallCallee::AssociatedType { .. }) =>
            {
                Ok(())
            }
            (Self::Detached { .. }, CallSourceContext::Detached { .. }) => {
                Err(ResolveCallError::InvalidResolvedCallable)
            }
        }
    }

    fn accepted_parts(
        self,
    ) -> Result<
        (
            &'a CanonicalModulePath,
            &'a ProjectSymbolTable,
            &'a RegisteredSemanticWorld,
        ),
        ResolveCallError,
    > {
        match self {
            Self::Accepted {
                current_module,
                symbols,
                world,
            } => Ok((current_module, symbols, world)),
            Self::Detached { .. } => Err(ResolveCallError::InvalidResolvedCallable),
        }
    }

    fn typed_environment_method(
        self,
        receiver: &TypeKind,
        member: &CallableName,
    ) -> Result<Option<Vec<TypedEnvironmentMethodCandidate<'a>>>, ResolveCallError> {
        let key = ReceiverMethodKey::new(receiver.clone(), member.clone());
        match self {
            Self::Accepted { world, .. } => {
                let catalog = world.environment().callable_catalog();
                let Some(candidates) = catalog
                    .validated_method(&key)
                    .map_err(|reason| corrupt(CallableLookupKey::Method(key.clone()), reason))?
                else {
                    return Ok(None);
                };
                let mut typed = Vec::new();
                for entry in candidates.as_slice() {
                    let CallableCandidateId::Environment(id) = entry.primary().id() else {
                        continue;
                    };
                    if id.kind() != EnvironmentCallableKind::Method {
                        continue;
                    }
                    if !matches!(
                        entry.primary().schema().validator(),
                        super::CallableValidator::Ordinary
                    ) {
                        return Err(ResolveCallError::InvalidResolvedCallable);
                    }
                    typed.push(TypedEnvironmentMethodCandidate::Accepted {
                        record: entry.primary(),
                        equivalent_sources: entry
                            .equivalent_sources()
                            .iter()
                            .filter(|source| {
                                matches!(
                                    source.id(),
                                    CallableCandidateId::Environment(id)
                                        if id.kind() == EnvironmentCallableKind::Method
                                )
                            })
                            .cloned()
                            .collect(),
                    });
                }
                Ok((!typed.is_empty()).then_some(typed))
            }
            Self::Detached { environment } => {
                let Some(projection) = environment
                    .standard_method_projection(
                        receiver,
                        member,
                        &super::PRODUCTION_CALLABLE_LIMITS,
                    )
                    .map_err(|_| ResolveCallError::InvalidResolvedCallable)?
                else {
                    return Ok(None);
                };
                if projection.id().kind() != EnvironmentCallableKind::Method {
                    return Ok(None);
                }
                Ok(Some(vec![TypedEnvironmentMethodCandidate::Detached {
                    id: projection.id().clone(),
                    schema: Arc::new(projection.schema().clone()),
                }]))
            }
        }
    }
}

#[allow(
    dead_code,
    reason = "some exact request accessors are consumed by subsequent family resolver cuts"
)]
impl<'a> CallResolverRequest<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_new(
        callee: CallCallee<'a>,
        authority: CallResolverAuthority<'a>,
        lexical: &'a LexicalCallableScope,
        expected: Option<&'a TypeKind>,
        traits: &'a TraitCatalog,
        trait_predicates: &'a [TraitPredicate],
        source: CallSourceContext<'a>,
        call_group: CallableGroupIndex,
        expression: TypeExpressionId,
        cancellation: &'a AtomicBool,
        work: &'a mut ResolverWork,
        limits: &'a CallableLimits,
    ) -> Result<Self, ResolveCallError> {
        if cancellation.load(Ordering::Acquire) {
            return Err(ResolveCallError::Cancelled);
        }
        authority.validate(&callee, &source)?;
        source.validate(limits)?;
        Ok(Self {
            callee,
            authority,
            lexical,
            expected,
            traits,
            trait_predicates,
            source,
            call_group,
            expression,
            cancellation,
            work,
            signature_work: None,
            signature_control: None,
            limits,
        })
    }

    pub(crate) fn with_signature_work(
        mut self,
        signature_work: Option<&'a mut super::SignatureQueryWorkMeter>,
    ) -> Self {
        self.signature_work = signature_work;
        self
    }

    pub(crate) fn with_signature_control(
        mut self,
        signature_control: Option<&'a dyn SignatureQueryStepControl>,
    ) -> Self {
        self.signature_control = signature_control;
        self
    }

    pub(crate) const fn callee(&self) -> &CallCallee<'a> {
        &self.callee
    }
    pub(crate) const fn authority(&self) -> CallResolverAuthority<'a> {
        self.authority
    }
    pub(crate) const fn lexical(&self) -> &LexicalCallableScope {
        self.lexical
    }
    pub(crate) const fn expected(&self) -> Option<&TypeKind> {
        self.expected
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
            receiver_expression,
            receiver_type,
            method,
            arguments,
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
        CallCallee::AssociatedType {
            receiver,
            member,
            arguments,
        } => {
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
        CallCallee::Dialogue { id, callee } => {
            match resolve_dialogue_call(&mut request, id, callee) {
                Ok(target) => ResolveCallOutcome::Resolved(target),
                Err(error) => ResolveCallOutcome::Rejected(error),
            }
        }
        CallCallee::FunctionValue { value } => match resolve_function_value(value, &mut request) {
            Ok(target) => ResolveCallOutcome::Resolved(target),
            Err(error) => ResolveCallOutcome::Rejected(error),
        },
    }
}

fn resolve_associated_type_call(
    request: &mut CallResolverRequest<'_>,
    receiver: ResolvedAssociatedTypeReceiver<'_>,
    member: &CallableName,
    arguments: &[CallArg],
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    let receiver_type = receiver.ty();
    if let Some(target) = resolve_associated_environment_method(request, receiver, member)? {
        return Ok(Some(target));
    }

    check_query_step(request)?;
    request
        .work
        .record_associated_step(AssociatedResolverStep::CapacitySelector)?;
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
        request
            .work
            .record_associated_step(AssociatedResolverStep::CapacityMaterialization)?;
        return Ok(Some(target));
    }

    check_query_step(request)?;
    request
        .work
        .record_associated_step(AssociatedResolverStep::TraitResolution)?;
    resolve_trait_method(request, MethodReceiver::Associated(receiver), member)
}

fn resolve_associated_environment_method(
    request: &mut CallResolverRequest<'_>,
    receiver: ResolvedAssociatedTypeReceiver<'_>,
    member: &CallableName,
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    let receiver_type = receiver.ty();
    check_query_step(request)?;
    request
        .work
        .record_associated_step(AssociatedResolverStep::TypedEnvironmentLookup)?;
    let Some(seeds) = request
        .authority
        .typed_environment_method(receiver_type, member)?
    else {
        return Ok(None);
    };
    let mut candidates = Vec::with_capacity(seeds.len());
    for seed in seeds {
        candidates.push(materialize_typed_environment_method(
            seed, receiver, request,
        )?);
    }
    NonEmptyResolvedCandidates::try_new(candidates, request.limits)
        .map(ResolvedCallTarget::Candidates)
        .map(Some)
}

fn materialize_typed_environment_method(
    seed: TypedEnvironmentMethodCandidate<'_>,
    receiver: ResolvedAssociatedTypeReceiver<'_>,
    request: &mut CallResolverRequest<'_>,
) -> Result<ResolvedCallable, ResolveCallError> {
    let instantiation = CallableInstantiation::TypeReceiver {
        receiver: TypeReceiverInstantiation::from_resolved(receiver),
    };
    match seed {
        TypedEnvironmentMethodCandidate::Accepted {
            record,
            equivalent_sources,
        } => resolve_catalog_record(record, &equivalent_sources, None, instantiation, request),
        TypedEnvironmentMethodCandidate::Detached { id, schema } => {
            check_query_step(request)?;
            ResolvedCallable::try_new(
                CallableCandidateId::Environment(id.clone()),
                SignatureOrigin::Standard {
                    owner: StandardEnvironmentId::Core,
                    id,
                },
                schema,
                instantiation,
                Vec::new(),
                Some(CallableAuthorityRank::Standard),
                request.limits,
            )
        }
    }
}

fn resolve_dialogue_call(
    request: &mut CallResolverRequest<'_>,
    id: super::DialogueCallableId,
    callee: &super::DialogueCalleeIdentity,
) -> Result<ResolvedCallTarget, ResolveCallError> {
    use super::{DialogueCalleeIdentity, DialogueSchemaContext};

    if super::DialogueCallableId::resolve(callee) != id {
        return Err(ResolveCallError::InvalidResolvedCallable);
    }
    let (_, _, world) = request.authority.accepted_parts()?;
    let schema = id
        .signature_schema(DialogueSchemaContext {
            callee,
            environment: world.environment(),
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
    check_query_step(request)?;
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
    let callable = ResolvedCallable::try_new(
        CallableCandidateId::FunctionValue(seed.id.clone()),
        SignatureOrigin::FunctionValue {
            id: seed.id.clone(),
        },
        Arc::new(seed.schema.clone()),
        CallableInstantiation::None,
        Vec::new(),
        None,
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
    arguments: &[CallArg],
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
    if let Some(target) = resolve_trait_method(request, MethodReceiver::Value(receiver), method)? {
        return Ok(Some(target));
    }

    check_query_step(request)?;
    if let Some(target) = resolve_data_last_method(request, receiver, method, arguments)? {
        return Ok(Some(target));
    }

    Ok(None)
}

fn resolve_data_last_method(
    request: &mut CallResolverRequest<'_>,
    receiver: EvaluatedReceiver<'_>,
    method: &CallableName,
    arguments: &[CallArg],
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    let mut bases = Vec::new();
    if let Some(binding) = request.lexical.binding(method) {
        if let LexicalCallBinding::Callable { .. } = binding
            && let ResolvedCallTarget::Candidates(candidates) =
                resolve_lexical_binding(method, binding, request)?
        {
            bases.extend(candidates.as_slice().iter().cloned());
        }
        return finish_data_last_candidates(request, receiver, arguments, bases);
    }

    let path = CallablePath::try_new([method.clone()])
        .map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
    let (current_module, symbols, world) = request.authority.accepted_parts()?;
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
    arguments: &[CallArg],
    bases: Vec<ResolvedCallable>,
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    let receiver_type = receiver.ty();
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
        check_query_step(request)?;
        candidates.push(ResolvedCallable::try_new(
            CallableCandidateId::DataLast(id),
            origin,
            Arc::new(schema),
            receiver.data_last_instantiation(group, parameter),
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

fn resolve_trait_method(
    request: &mut CallResolverRequest<'_>,
    receiver: MethodReceiver<'_>,
    method: &CallableName,
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    match request
        .traits
        .resolve_method(receiver.ty(), method.as_str(), request.trait_predicates)
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
            resolved_trait_method(request, id, &selected, receiver).map(Some)
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
            resolved_trait_method(request, id, &selected, receiver).map(Some)
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

fn resolved_trait_method(
    request: &mut CallResolverRequest<'_>,
    id: TraitCallableId,
    method: &crate::traits::TraitMethodImpl,
    receiver: MethodReceiver<'_>,
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
    check_query_step(request)?;
    let callable = ResolvedCallable::try_new(
        CallableCandidateId::TraitMethod(id.clone()),
        SignatureOrigin::Trait { id },
        Arc::new(schema),
        receiver.instantiation(),
        Vec::new(),
        None,
        request.limits,
    )?;
    NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
        .map(ResolvedCallTarget::Candidates)
}

fn resolved_language_method(
    request: &mut CallResolverRequest<'_>,
    id: CallableCandidateId,
    family: LanguageCallableFamily,
    schema: CallableSignatureSchema,
    instantiation: CallableInstantiation,
) -> Result<ResolvedCallTarget, ResolveCallError> {
    check_query_step(request)?;
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

fn resolve_selected_environment_method(
    request: &mut CallResolverRequest<'_>,
    receiver_type: &TypeKind,
    method: &CallableName,
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    check_query_step(request)?;
    let key = ReceiverMethodKey::new(receiver_type.clone(), method.clone());
    let (_, _, world) = request.authority.accepted_parts()?;
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
) -> Result<Option<ResolvedCallTarget>, ResolveCallError> {
    check_query_step(request)?;
    if let FxResolution::Known(id) = FxCallableSignatureId::resolve(path) {
        check_query_step(request)?;
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
    let enum_variant = match &request.callee {
        CallCallee::Free {
            enum_variant: Some(seed),
            ..
        } => Some((*seed).clone()),
        _ => None,
    };
    if let Some(seed) = enum_variant {
        check_query_step(request)?;
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
        check_query_step(request)?;
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
        check_query_step(request)?;
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
        check_query_step(request)?;
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
        check_query_step(request)?;
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
        check_query_step(request)?;
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
        return resolve_lexical_binding(name, binding, request).map(Some);
    }

    check_query_step(request)?;
    let (current_module, symbols, world) = request.authority.accepted_parts()?;
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

fn resolve_lexical_binding(
    name: &CallableName,
    binding: &LexicalCallBinding,
    request: &mut CallResolverRequest<'_>,
) -> Result<ResolvedCallTarget, ResolveCallError> {
    match binding {
        LexicalCallBinding::Callable {
            id,
            schema,
            effects,
        } => {
            let _ = effects;
            check_query_step(request)?;
            let callable = ResolvedCallable::try_new(
                CallableCandidateId::Local(id.clone()),
                SignatureOrigin::Lexical { id: id.clone() },
                Arc::clone(schema),
                CallableInstantiation::None,
                Vec::new(),
                None,
                request.limits,
            )?;
            NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
                .map(ResolvedCallTarget::Candidates)
        }
        LexicalCallBinding::FunctionValue(seed) => resolve_function_value(seed, request),
        LexicalCallBinding::Speaker { id, schema } => {
            check_query_step(request)?;
            let callable = ResolvedCallable::try_new(
                CallableCandidateId::Speaker(id.clone()),
                SignatureOrigin::Language {
                    family: LanguageCallableFamily::Speaker,
                },
                Arc::clone(schema),
                CallableInstantiation::None,
                Vec::new(),
                None,
                request.limits,
            )?;
            NonEmptyResolvedCandidates::try_new(vec![callable], request.limits)
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

fn resolve_project_binding(
    binding: &ProjectNameBinding,
    path: &ProjectCallablePath,
    request: &mut CallResolverRequest<'_>,
) -> Result<ResolvedCallTarget, ResolveCallError> {
    let (_, _, world) = request.authority.accepted_parts()?;
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
    record: &CallableRecord,
    equivalent_sources: &[EquivalentCallableSource],
    project_path: Option<&ProjectCallablePath>,
    instantiation: CallableInstantiation,
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
    let (_, _, world) = request.authority.accepted_parts()?;
    let reachable = match record.id() {
        CallableCandidateId::Project(id) => world
            .environment()
            .callable_catalog()
            .project_record(id)
            .is_some_and(|accepted| accepted.as_ref() == record),
        CallableCandidateId::Environment(id) => world
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
        instantiation,
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

fn check_query_step(request: &mut CallResolverRequest<'_>) -> Result<(), ResolveCallError> {
    if let Some(control) = request.signature_control {
        control.check_signature_query_step(SignatureQueryStep::Resolver)?;
    } else if request.cancellation.load(Ordering::Acquire) {
        return Err(ResolveCallError::Cancelled);
    }
    if let Some(signature_work) = request.signature_work.as_deref_mut() {
        signature_work
            .charge(super::SignatureWorkKind::Resolver, 1)
            .map_err(|error| match error {
                super::SignatureAccountingError::Limit(error) => {
                    ResolveCallError::SignatureLimit(error)
                }
                super::SignatureAccountingError::Arithmetic { counter } => {
                    ResolveCallError::SignatureArithmeticOverflow { counter }
                }
            })?;
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

/// Opaque proof that a callable receiver came from complete nominal type resolution.
///
/// The normalized type remains observable for semantic facts, but constructing
/// this carrier is reserved for the associated-receiver projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeReceiverInstantiation {
    receiver: TypeKind,
}

impl TypeReceiverInstantiation {
    pub(crate) fn from_resolved(receiver: ResolvedAssociatedTypeReceiver<'_>) -> Self {
        Self {
            receiver: receiver.ty().clone(),
        }
    }

    /// Returns the exact normalized receiver type.
    pub const fn receiver(&self) -> &TypeKind {
        &self.receiver
    }
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
    TypeReceiver {
        receiver: TypeReceiverInstantiation,
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
    pub(crate) fn call_shape_is_viable(
        &self,
        group: CallableGroupIndex,
        arguments: &[arcweft_lang_syntax::expr::CallArg],
    ) -> bool {
        let implicit = match &self.instantiation {
            CallableInstantiation::DataLast {
                group: implicit_group,
                parameter,
                ..
            } if *implicit_group == group => Some(*parameter),
            _ => None,
        };
        super::arguments::call_shape_is_viable_with_implicit(
            &self.schema,
            group,
            arguments,
            implicit,
        )
    }

    pub(crate) fn try_new(
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
                    candidate: Box::new(base.clone()),
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

    pub const fn call_group(&self) -> CallableGroupIndex {
        match &self.instantiation {
            CallableInstantiation::Curried { group, .. }
            | CallableInstantiation::DataLast { group, .. } => *group,
            CallableInstantiation::None
            | CallableInstantiation::ExpectedEnum { .. }
            | CallableInstantiation::Result { .. }
            | CallableInstantiation::Option { .. }
            | CallableInstantiation::Character { .. }
            | CallableInstantiation::Receiver { .. }
            | CallableInstantiation::TypeReceiver { .. } => CallableGroupIndex::ZERO,
        }
    }

    pub(crate) fn try_curried(
        &self,
        group: CallableGroupIndex,
        limits: &CallableLimits,
    ) -> Result<Self, ResolveCallError> {
        let base = match &self.id {
            CallableCandidateId::Curried(id) => id.base().clone(),
            CallableCandidateId::DataLast(id) => id.callable().clone(),
            id => id.clone(),
        };
        let id = CurriedCallableId::try_new(base.clone(), group).map_err(|error| match error {
            super::CallableIdentityError::InvalidCurriedGroup { group, .. } => {
                ResolveCallError::InvalidCallGroup {
                    candidate: Box::new(base.clone()),
                    group,
                }
            }
            _ => ResolveCallError::InvalidResolvedCallable,
        })?;
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

    pub(crate) fn try_with_presentation_character_owner(
        &self,
        owner: ResolvedCharacterOwner,
        environment: &crate::registration::RegisteredTypeCheckEnv,
        limits: &CallableLimits,
    ) -> Result<Self, ResolveCallError> {
        let CallableCandidateId::Presentation(id) = &self.id else {
            return Err(ResolveCallError::InvalidResolvedCallable);
        };
        let schema = (*id)
            .signature_schema(super::PresentationSchemaContext {
                owner: Some(&owner),
                environment,
            })
            .map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
        Self::try_new(
            self.id.clone(),
            self.origin.clone(),
            Arc::new(schema),
            CallableInstantiation::Character { owner },
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
            | (
                CallableCandidateId::StageMethod(_),
                LanguageCallableFamily::StageMethod
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
            | CallableCandidateId::StageMethod(_),
            CallableInstantiation::Receiver { .. },
        )
        | (
            CallableCandidateId::Environment(_) | CallableCandidateId::TraitMethod(_),
            CallableInstantiation::TypeReceiver { .. },
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
        (
            CallableCandidateId::CapacityMethod(id),
            CallableInstantiation::TypeReceiver { receiver },
        ) => id.method().as_str() == "with_capacity" && id.receiver() == receiver.receiver(),
        (CallableCandidateId::CapacityMethod(id), CallableInstantiation::Receiver { receiver }) => {
            id.method().as_str() != "with_capacity" && id.receiver() == receiver
        }
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
pub enum ResolvedCallTarget {
    Candidates(NonEmptyResolvedCandidates),
    FunctionValue(Box<ResolvedFunctionValue>),
    NonCallable(ResolvedNonCallableTarget),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NonEmptyResolvedCandidates {
    candidates: Arc<[ResolvedCallable]>,
}
impl NonEmptyResolvedCandidates {
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
    AssociatedType,
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
    ExternalOwner,
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
