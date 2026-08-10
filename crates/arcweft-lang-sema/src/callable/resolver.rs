//! Validated callable resolver requests and products.

mod outcome;
mod preparation;
mod resolution;

pub use outcome::{
    CallableInstantiation, CharacterOwnerSource, NonCallableSource, NonEmptyResolvedCandidates,
    ResolveCallOutcome, ResolvedCallTarget, ResolvedCallable, ResolvedCharacterOwner,
    ResolvedFunctionValue, ResolvedNonCallableTarget, SignatureOrigin, TypeReceiverInstantiation,
    UnknownCallKind, UnknownCallTarget,
};
use preparation::classify_prepared_callee;
pub(crate) use preparation::{prepare_final_call_callee, prepare_language_free_dot_path};
use resolution::corrupt;
pub(crate) use resolution::resolve_call_target;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use arcweft_lang_hir::{
    dialogue_application::HirDialogueContentApplication,
    expr::{
        HirAssociatedCallSyntax, HirAssociatedReceiver, HirAssociatedSeparator, HirCallArgument,
        HirCallCallee, HirCallExpr, HirExpr, HirExprKind, HirRecoveredName,
    },
    identity::{ExprId, HirModuleId, TypeId},
    leaf::{HirPath, HirPathRoot, HirPathSegment, HirPathValue},
    module::HirModule,
    project::HirProjectView,
    source_index::{HirExprSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite},
    symbol::{
        CallableDeclarationKey, ProjectSymbolTable, ProjectValueLookup, ProjectValueLookupError,
    },
};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use thiserror::Error;

use crate::{
    effect_model::CallableId,
    final_analysis::{CheckedExpression, CheckedExpressionResolution, CheckedValueResolution},
    nominal::{ResolvedAssociatedTypeReceiver, TypeResolutionReport},
    registration::RegisteredSemanticWorld,
    types::TypeKind,
};

use super::CharacterDialoguePatchContext;
use super::{
    AdapterPackageId, AgentIntrinsicSignatureId, BuiltinCallableId, CallCalleeClassificationFact,
    CallTargetFact, CallTargetFacts, CallableAuthorityRank, CallableCandidateId, CallableFamily,
    CallableGroupIndex, CallableIdentityError, CallableLimits, CallableLookupKey, CallableName,
    CallableParameterIndex, CallableParameterType, CallablePath, CallableRecord,
    CallableSignatureSchema, CapacityMethodId, CheckedCallableDeclaration, CheckedCallableId,
    CheckedMethodLookup, CollectionMethodId, CorruptCallableCatalogReason, CurriedCallableId,
    DataLastCallableId, DomainMethodId, DropCallableId, EnvironmentCallableId,
    EnvironmentCallableKind, EnvironmentCallableOwner, EquivalentCallableSource,
    FunctionValueOrdinal, FunctionValueSignatureId, FxCallableSignatureId, FxResolution,
    IntegerMethodId, LanguageCallableFamily, LocalCallableId, OptionConstructorKind,
    PresentationCallableId, PresentationHandleMethodId, PresentationSchemaContext,
    ProjectCallablePath, ProjectNameBinding, PromotionCallableId, ReceiverMethodKey,
    ResolveCallError, ResolverWork, ResultConstructorKind, StageMethodId, StandardEnvironmentId,
    call_shape_is_viable,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedCallCallee<'a> {
    Free {
        path: &'a CallablePath,
        project: Option<&'a CallableDeclarationKey>,
        scope: PreparedFreeCallScope,
        enum_variant: Option<&'a ResolvedEnumSeed>,
    },
    Selected {
        receiver_expression: ExprId,
        receiver_type: &'a TypeKind,
        method: &'a CallableName,
    },
    AssociatedType {
        receiver: ResolvedAssociatedTypeReceiver<'a>,
        member: &'a CallableName,
    },
    Dialogue {
        id: super::DialogueCallableId,
        callee: &'a super::DialogueCalleeIdentity,
        patch_context: CharacterDialoguePatchContext,
    },
    FunctionValue {
        value: &'a ResolvedFunctionValueSeed,
    },
    NonCallableValue {
        expression: ExprId,
        ty: &'a TypeKind,
    },
}

/// Root behavior retained while the shared resolver consumes a free-call path.
///
/// Explicit project roots never enter the unqualified language, lexical, or
/// registered-environment namespaces. Project resolution already selected an
/// exact declaration through the final symbol table before this value exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreparedFreeCallScope {
    Implicit,
    ExplicitProject,
}

/// Owned pre-resolver callee selected from one final-HIR call and already
/// checked child facts.
///
/// The associated receiver borrows the complete nominal report that proved the
/// exact `TypeId`; every other resolver input is owned so no source substring,
/// detached syntax node, or temporary label has to outlive preparation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedFinalCallCallee<'a> {
    Free {
        path: Box<CallablePath>,
        project: Option<Box<CallableDeclarationKey>>,
        scope: PreparedFreeCallScope,
        enum_variant: Option<Box<ResolvedEnumSeed>>,
    },
    Selected {
        receiver_expression: ExprId,
        receiver_type: Box<TypeKind>,
        method: CallableName,
    },
    AssociatedType {
        receiver: ResolvedAssociatedTypeReceiver<'a>,
        member: CallableName,
    },
    Dialogue {
        id: super::DialogueCallableId,
        callee: super::DialogueCalleeIdentity,
        patch_context: CharacterDialoguePatchContext,
    },
    FunctionValue {
        value: Box<ResolvedFunctionValueSeed>,
    },
    NonCallableValue {
        expression: ExprId,
        ty: Box<TypeKind>,
    },
}

impl PreparedFinalCallCallee<'_> {
    pub(crate) fn as_borrowed(&self) -> PreparedCallCallee<'_> {
        match self {
            Self::Free {
                path,
                project,
                scope,
                enum_variant,
            } => PreparedCallCallee::Free {
                path,
                project: project.as_deref(),
                scope: *scope,
                enum_variant: enum_variant.as_deref(),
            },
            Self::Selected {
                receiver_expression,
                receiver_type,
                method,
            } => PreparedCallCallee::Selected {
                receiver_expression: *receiver_expression,
                receiver_type,
                method,
            },
            Self::AssociatedType { receiver, member } => PreparedCallCallee::AssociatedType {
                receiver: *receiver,
                member,
            },
            Self::Dialogue {
                id,
                callee,
                patch_context,
            } => PreparedCallCallee::Dialogue {
                id: *id,
                callee,
                patch_context: *patch_context,
            },
            Self::FunctionValue { value } => PreparedCallCallee::FunctionValue { value },
            Self::NonCallableValue { expression, ty } => PreparedCallCallee::NonCallableValue {
                expression: *expression,
                ty,
            },
        }
    }
}

/// Immutable child facts needed to prepare one final-HIR call.
#[derive(Clone, Copy)]
pub(crate) struct FinalCallCalleeFacts<'a> {
    expressions: &'a BTreeMap<ExprId, CheckedExpression>,
    calls: &'a BTreeMap<ExprId, super::CallTargetFacts>,
    nominal_receivers: &'a BTreeMap<TypeId, TypeResolutionReport>,
    enum_variants: &'a BTreeMap<ExprId, ResolvedEnumSeed>,
}

impl<'a> FinalCallCalleeFacts<'a> {
    pub(crate) const fn new(
        expressions: &'a BTreeMap<ExprId, CheckedExpression>,
        calls: &'a BTreeMap<ExprId, super::CallTargetFacts>,
        nominal_receivers: &'a BTreeMap<TypeId, TypeResolutionReport>,
        enum_variants: &'a BTreeMap<ExprId, ResolvedEnumSeed>,
    ) -> Self {
        Self {
            expressions,
            calls,
            nominal_receivers,
            enum_variants,
        }
    }
}

/// Typed terminal failure before the shared resolver is entered.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum PrepareFinalCallCalleeError {
    #[error("call expression is absent from the accepted final HIR module")]
    InvalidCallExpression { expression: ExprId },
    #[error("call callee child is absent from staged semantic facts")]
    MissingExpressionFact { expression: ExprId },
    #[error("call callee path cannot be represented by the typed callable path owner")]
    InvalidValuePath { expression: ExprId },
    #[error("call callee has no authored source span")]
    MissingValueSource { expression: ExprId },
    #[error("project value lookup failed")]
    ProjectValueLookup {
        #[source]
        error: Box<ProjectValueLookupError>,
    },
    #[error("project value lookup and staged semantic value disagree")]
    ProjectValueFactMismatch { expression: ExprId },
    #[error("associated receiver has no complete nominal resolution report")]
    MissingNominalReceiver { receiver: TypeId },
    #[error("associated receiver nominal report is not complete")]
    InvalidNominalReceiver { receiver: TypeId },
    #[error("call callee retains structural recovery and cannot enter the resolver")]
    RecoveredCallee,
    #[error("function-value callee has no closed function type")]
    InvalidFunctionValue { expression: ExprId },
    #[error("function-value callable schema could not be constructed")]
    InvalidFunctionSchema,
}

/// Proof that a method receiver was evaluated as a value expression.
#[derive(Clone, Copy, Debug)]
struct EvaluatedReceiver<'a> {
    _expression: ExprId,
    ty: &'a TypeKind,
}

impl<'a> EvaluatedReceiver<'a> {
    const fn new(expression: ExprId, ty: &'a TypeKind) -> Self {
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

#[derive(Clone, Copy)]
pub(crate) struct CallResolverAuthority<'a> {
    project: HirProjectView<'a>,
    module: &'a HirModule,
    symbols: &'a ProjectSymbolTable,
    world: &'a RegisteredSemanticWorld,
}

struct TypedEnvironmentMethodCandidate<'a> {
    record: &'a Arc<CallableRecord>,
    equivalent_sources: Vec<EquivalentCallableSource>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedEnumSeed {
    id: super::EnumVariantSignatureId,
    expected: TypeKind,
    schema: CallableSignatureSchema,
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

pub(crate) struct CallResolverRequest<'a> {
    callee: PreparedCallCallee<'a>,
    authority: CallResolverAuthority<'a>,
    checked: CheckedCallResolverAuthority<'a>,
    expected: Option<&'a TypeKind>,
    call: Option<&'a HirCallExpr>,
    classification: CallCalleeClassificationFact,
    call_group: CallableGroupIndex,
    cancellation: &'a AtomicBool,
    work: &'a mut ResolverWork,
    limits: &'a CallableLimits,
}

/// Immutable authorities and controls for one final-HIR call-resolution query.
///
/// The source expression is validated against `authority` before the request
/// becomes observable. Grouping these values prevents resolver construction
/// from becoming a positional list of unrelated identities and budgets.
pub(crate) struct CallResolverContext<'a> {
    pub(crate) authority: CallResolverAuthority<'a>,
    pub(crate) checked: CheckedCallResolverAuthority<'a>,
    pub(crate) expected: Option<&'a TypeKind>,
    pub(crate) call_group: CallableGroupIndex,
    pub(crate) expression: ExprId,
    pub(crate) cancellation: &'a AtomicBool,
    pub(crate) limits: &'a CallableLimits,
}

/// The one checked-callable selection authority admitted while a final
/// semantic generation is being built or queried.
///
/// The pending form borrows the same consuming catalog transaction that is
/// later frozen and published. It exposes only structural checked identity and
/// the exact accepted record pointer needed by resolution; inferred effect
/// rows remain unavailable until the transaction is complete. This is a build
/// phase of the final authority, not a second catalog or compatibility reader.
#[derive(Clone, Copy)]
pub(crate) enum CheckedCallResolverAuthority<'a> {
    Pending(&'a super::CheckedCallableCatalogBuilder),
    Frozen(&'a super::CheckedCallableCatalog),
}

impl<'a> CheckedCallResolverAuthority<'a> {
    fn checked_for_candidate(
        self,
        candidate: &CallableCandidateId,
    ) -> Result<&'a CheckedCallableId, super::CheckedCallableLookupError> {
        match self {
            Self::Pending(builder) => builder
                .pending_by_candidate(candidate)
                .map(super::checked_catalog::PendingCheckedCallable::id),
            Self::Frozen(catalog) => catalog.checked_for_candidate(candidate),
        }
    }

    fn record(
        self,
        id: &CheckedCallableId,
    ) -> Result<&'a Arc<CallableRecord>, super::CheckedCallableLookupError> {
        match self {
            Self::Pending(builder) => builder
                .pending_by_id(id)
                .map(super::checked_catalog::PendingCheckedCallable::record),
            Self::Frozen(catalog) => catalog
                .callable(id)
                .map(super::CheckedCallableFacts::record),
        }
    }

    fn method(self, key: &ReceiverMethodKey) -> super::CheckedMethodLookup {
        match self {
            Self::Pending(builder) => builder.method(key),
            Self::Frozen(catalog) => catalog.method(key),
        }
    }
}

impl<'a> From<&'a super::CheckedCallableCatalog> for CheckedCallResolverAuthority<'a> {
    fn from(catalog: &'a super::CheckedCallableCatalog) -> Self {
        Self::Frozen(catalog)
    }
}

impl<'a> From<&'a super::CheckedCallableCatalogBuilder> for CheckedCallResolverAuthority<'a> {
    fn from(builder: &'a super::CheckedCallableCatalogBuilder) -> Self {
        Self::Pending(builder)
    }
}

impl<'a> CallResolverAuthority<'a> {
    pub(crate) const fn accepted(
        project: HirProjectView<'a>,
        module: &'a HirModule,
        symbols: &'a ProjectSymbolTable,
        world: &'a RegisteredSemanticWorld,
    ) -> Self {
        Self {
            project,
            module,
            symbols,
            world,
        }
    }

    fn validate(
        self,
        callee: &PreparedCallCallee<'_>,
        expression: ExprId,
        limits: &CallableLimits,
    ) -> Result<(&'a HirCallExpr, CallCalleeClassificationFact), ResolveCallError> {
        let expression = self.validate_expression(expression, limits)?;
        let HirExprKind::Call(call) = expression.kind() else {
            return Err(ResolveCallError::InvalidResolvedCallable);
        };
        let classification = classify_prepared_callee(callee, call, self.module.module_id())?;
        Ok((call, classification))
    }

    fn validate_dialogue_application(
        self,
        callee: &PreparedCallCallee<'_>,
        expression: ExprId,
        limits: &CallableLimits,
    ) -> Result<
        (
            &'a HirDialogueContentApplication,
            CallCalleeClassificationFact,
        ),
        ResolveCallError,
    > {
        let expression = self.validate_expression(expression, limits)?;
        let HirExprKind::DialogueContentApplication(application) = expression.kind() else {
            return Err(ResolveCallError::InvalidResolvedCallable);
        };
        let PreparedCallCallee::Dialogue { id, callee, .. } = callee else {
            return Err(ResolveCallError::InvalidResolvedCallable);
        };
        if *id != super::DialogueCallableId::ContentApplication || !id.supports_callee(callee) {
            return Err(ResolveCallError::InvalidResolvedCallable);
        }
        Ok((
            application,
            CallCalleeClassificationFact::Value {
                expression: application.target(),
            },
        ))
    }

    fn validate_expression(
        self,
        expression: ExprId,
        limits: &CallableLimits,
    ) -> Result<&'a HirExpr, ResolveCallError> {
        if self.symbols.world() != self.world.symbols().world()
            || self.symbols.revision() != self.world.symbols().revision()
            || self.symbols.world() != self.world.environment().world()
            || self.symbols.revision() != self.world.environment().symbol_revision()
            || self.project.package() != self.module.key().package()
            || self.symbols.world().package() != self.project.package()
        {
            return Err(ResolveCallError::WorldMismatch);
        }
        let Some(project_module) = self.project.module(self.module.key().path()) else {
            return Err(ResolveCallError::WorldMismatch);
        };
        if !std::ptr::eq(project_module.as_ref(), self.module) {
            return Err(ResolveCallError::WorldMismatch);
        }
        if self.symbols.source_identity(self.module.key().path())
            != Some(self.module.provenance().source_identity())
        {
            return Err(ResolveCallError::SourceIdentityMismatch);
        }
        let source_len = usize::try_from(self.module.provenance().source_identity().source_len())
            .unwrap_or(usize::MAX);
        if source_len > limits.max_source_bytes() {
            return Err(ResolveCallError::Work(
                super::CallableQueryLimitError::SourceBytes {
                    actual: source_len,
                    limit: limits.max_source_bytes(),
                },
            ));
        }
        self.module
            .resolve_expr(expression)
            .map_err(|_| ResolveCallError::InvalidResolvedCallable)
    }

    const fn parts(
        self,
    ) -> (
        &'a CanonicalModulePath,
        &'a ProjectSymbolTable,
        &'a RegisteredSemanticWorld,
    ) {
        (self.module.key().path(), self.symbols, self.world)
    }

    const fn module(self) -> &'a HirModule {
        self.module
    }

    const fn world(self) -> &'a RegisteredSemanticWorld {
        self.world
    }

    fn typed_environment_method(
        self,
        receiver: &TypeKind,
        member: &CallableName,
    ) -> Result<Option<Vec<TypedEnvironmentMethodCandidate<'a>>>, ResolveCallError> {
        let key = ReceiverMethodKey::new(receiver.clone(), member.clone());
        let catalog = self.world.environment().callable_catalog();
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
            typed.push(TypedEnvironmentMethodCandidate {
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
}

impl<'a> CallResolverRequest<'a> {
    pub(crate) fn try_new(
        callee: PreparedCallCallee<'a>,
        context: &CallResolverContext<'a>,
        work: &'a mut ResolverWork,
    ) -> Result<Self, ResolveCallError> {
        if context.cancellation.load(Ordering::Acquire) {
            return Err(ResolveCallError::Cancelled);
        }
        let (call, classification) =
            context
                .authority
                .validate(&callee, context.expression, context.limits)?;
        Ok(Self {
            callee,
            authority: context.authority,
            checked: context.checked,
            expected: context.expected,
            call: Some(call),
            classification,
            call_group: context.call_group,
            cancellation: context.cancellation,
            work,
            limits: context.limits,
        })
    }

    pub(crate) fn try_new_dialogue_application(
        callee: PreparedCallCallee<'a>,
        context: &CallResolverContext<'a>,
        work: &'a mut ResolverWork,
    ) -> Result<Self, ResolveCallError> {
        if context.cancellation.load(Ordering::Acquire) {
            return Err(ResolveCallError::Cancelled);
        }
        let (_, classification) = context.authority.validate_dialogue_application(
            &callee,
            context.expression,
            context.limits,
        )?;
        Ok(Self {
            callee,
            authority: context.authority,
            checked: context.checked,
            expected: context.expected,
            call: None,
            classification,
            call_group: context.call_group,
            cancellation: context.cancellation,
            work,
            limits: context.limits,
        })
    }

    pub(crate) const fn parenthesized_call(&self) -> Option<&'a HirCallExpr> {
        self.call
    }
    pub(crate) const fn classification(&self) -> CallCalleeClassificationFact {
        self.classification
    }
}
