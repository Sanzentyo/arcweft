//! Validated callable resolver requests and products.

mod effect_instantiation;
mod outcome;
mod preparation;
mod prepared_identity;
mod resolution;

pub(crate) use effect_instantiation::{
    CheckedCallableEffectInstantiation, PreparedCallableEffectInstantiation,
    PreparedCallableEffectInstantiationEvidence,
};
pub use outcome::{
    CallableInstantiation, CharacterOwnerSource, NonCallableSource, ResolvedCharacterOwner,
    ResolvedNonCallableTarget, SignatureOrigin, TypeReceiverInstantiation, UnknownCallKind,
    UnknownCallTarget,
};
pub(crate) use outcome::{
    DetachedPreparedResolvedCallable, PreparedCallableDefinitionKey, PreparedResolvedCallable,
    PreparedResolvedCallableDefinition, PreparedResolvedCallableDefinitionBatch,
    PreparedResolvedCallableDefinitionSealInput, PreparedResolvedCallableDetachArena,
};
pub(crate) use outcome::{NonEmptyResolvedCandidates, ResolveCallOutcome, ResolvedCallTarget};
use preparation::classify_prepared_callee;
#[cfg(test)]
pub(crate) use preparation::prepare_function_value_origin_query;
pub(crate) use preparation::{
    prepare_final_call_callee, prepare_function_value_origin_query_with_pending_captures,
    prepare_language_free_dot_path, prepare_presentation_callee_id,
};
pub(crate) use prepared_identity::{
    PreparedCaptureIdentityRow, PreparedDialogueCalleeIdentity,
    PreparedFunctionValueOriginIdentity, PreparedResolvedCallableIdentity,
};
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
    dialogue_application::{
        HirAttachedContentApplication, HirAttachedContentApplicationFamily, HirDialogueContentId,
    },
    expr::{
        HirAssociatedCallSyntax, HirAssociatedReceiver, HirAssociatedSeparator, HirCallArgument,
        HirCallArgumentOrdinal, HirCallCallee, HirCallInvocation, HirExpr, HirExprKind,
        HirRecoveredName, HirSelectedMember,
    },
    identity::{ExprId, HirModuleId, LocalId, TypeId},
    leaf::{HirPath, HirPathRoot, HirPathSegment, HirPathValue},
    module::HirModule,
    project::{HirLocalValueOrigin, HirProjectEvaluationTopology, HirProjectView},
    source_index::{HirExprSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite},
    symbol::{
        CallableDeclarationKey, ProjectSymbolTable, ProjectValueLookup, ProjectValueLookupError,
    },
};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use thiserror::Error;

use crate::{
    effect_model::CallableId,
    final_analysis::{
        CheckedCaptureAuthorityViolation, CheckedExpression, CheckedExpressionResolution,
        CheckedValueResolution, PreparedExpressionFact, PreparedOwnerBoundResolution,
    },
    nominal::{ResolvedAssociatedTypeReceiver, TypeResolutionReport},
    registration::RegisteredSemanticWorld,
    types::{TypeKind, VariantPayloadShape},
};

use super::CharacterDialoguePatchContext;
use super::{
    AdapterPackageId, AgentIntrinsicSignatureId, BuiltinCallableId, CallCalleeClassificationFact,
    CallConstraintInvariant, CallableAuthorityRank, CallableCandidateId, CallableFamily,
    CallableGroupIndex, CallableLimits, CallableLookupKey, CallableName,
    CallableParameterCoordinate, CallableParameterIndex, CallableParameterPresence, CallablePath,
    CallableRecord, CallableSignatureSchema, CallableSignatureSchemaDigest, CallableValidator,
    CapacityMethodId, CheckedCallableDeclaration, CheckedCallableId, CheckedMethodLookup,
    CollectionMethodId, CorruptCallableCatalogReason, DomainMethodId, EnvironmentCallableId,
    EnvironmentCallableKind, EnvironmentCallableOwner, EquivalentCallableSource,
    FunctionValueOrdinal, FunctionValueSignatureId, IntegerMethodId, LanguageCallableFamily,
    LineContextMethodId, LineScheduleCallableId, LocalCallableId, OptionConstructorKind,
    PresentationCallableId, PresentationHandleMethodId, PresentationSchemaContext,
    ProjectCallablePath, ProjectNameBinding, PromotionCallableId, ReceiverMethodKey,
    ResolveCallError, ResolverWork, ResultConstructorKind, StageMethodId, StandardEnvironmentId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedCallCallee<'a> {
    Free {
        path: &'a CallablePath,
        project: Option<&'a CallableDeclarationKey>,
        scope: PreparedFreeCallScope,
        context: PreparedFreeCallContext,
    },
    EnumConstructor {
        seed: &'a AcceptedEnumVariantCase,
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
        result: super::DialogueCallableResultContext<'a>,
    },
    FunctionValue {
        value: &'a PreparedFunctionValueCallee,
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

/// Context carried into free-call resolution. Attached Content calls are
/// resolved from the catalog's exact implicit head; they do not carry a
/// lossy operation discriminator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreparedFreeCallContext {
    Ordinary,
    AttachedContent,
}

/// Owned pre-resolver callee selected from one final-HIR call and already
/// checked child facts.
///
/// The associated receiver borrows the complete nominal report that proved the
/// exact `TypeId`; every other resolver input is owned so no source substring,
/// detached syntax node, or temporary label has to outlive preparation.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PreparedFinalCallCallee<'a> {
    Free {
        path: Box<CallablePath>,
        project: Option<Box<CallableDeclarationKey>>,
        scope: PreparedFreeCallScope,
        context: PreparedFreeCallContext,
    },
    EnumConstructor {
        seed: Box<AcceptedEnumVariantCase>,
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
        value: Box<PreparedFunctionValueCallee>,
    },
    NonCallableValue {
        expression: ExprId,
        ty: Box<TypeKind>,
    },
}

/// Owned callee authority sealed before resolver execution.  The constraint
/// gate consumes this carrier instead of receiving a raw receiver expression
/// or detached function-value type alongside the resolved candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedCallCalleeConstraintInputs {
    Free,
    EnumConstructor,
    ValueReceiver { source: ExprId, actual: TypeKind },
    AssociatedType { actual: TypeKind },
    DialogueCallee,
    DialogueApplication,
    StaticContentCallee(PreparedStaticContentCallee),
    FunctionValue { actual: TypeKind },
    NonCallable,
}

/// Static language/content callee evidence used by Object proxy calls.
///
/// The HIR expression is retained only as a source-owner coordinate. Its
/// callee fact is intentionally not published: the exact content identity and
/// callable schema are the complete semantic authority for candidate checks.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct PreparedStaticContentCallee {
    expression: ExprId,
    identity: super::ContentCallableIdentity,
    schema: CallableSignatureSchemaDigest,
}

impl PreparedStaticContentCallee {
    pub(crate) const fn new(
        expression: ExprId,
        identity: super::ContentCallableIdentity,
        schema: CallableSignatureSchemaDigest,
    ) -> Self {
        Self {
            expression,
            identity,
            schema,
        }
    }

    pub(crate) const fn expression(self) -> ExprId {
        self.expression
    }

    pub(crate) const fn identity(self) -> super::ContentCallableIdentity {
        self.identity
    }

    pub(crate) const fn schema(self) -> CallableSignatureSchemaDigest {
        self.schema
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedImplicitExtensionReceiver {
    source: ExprId,
    actual: TypeKind,
}

impl PreparedImplicitExtensionReceiver {
    pub(crate) const fn new(source: ExprId, actual: TypeKind) -> Self {
        Self { source, actual }
    }

    pub(crate) const fn source(&self) -> ExprId {
        self.source
    }

    pub(crate) const fn actual(&self) -> &TypeKind {
        &self.actual
    }
}

/// Closed owner family for semantic call operands.  Additional semantic
/// producers (for example an Object text-proxy type) must enter this algebra
/// explicitly; no stringly or open fallback can share the prepared row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum PreparedCallSemanticOperandOwner {
    DialogueApplication,
    TextProxyObject,
}

/// Typed role of a semantic call operand.  The source expression is retained
/// separately on each row, including the enclosing application for content
/// and line-plan roles.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum PreparedCallSemanticOperandRole {
    DialogueTarget,
    DialogueContent,
    DialogueLinePlan,
    TextProxyNominalDiscriminator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedCallSemanticOperand {
    DialogueTarget {
        source: ExprId,
        coordinate: CallableParameterCoordinate,
        actual: TypeKind,
    },
    DialogueContent {
        source: ExprId,
        coordinate: CallableParameterCoordinate,
        actual: TypeKind,
    },
    DialogueLinePlan {
        source: ExprId,
        coordinate: CallableParameterCoordinate,
        actual: TypeKind,
    },
    TextProxyNominalDiscriminator {
        argument: HirCallArgumentOrdinal,
        source: ExprId,
        coordinate: CallableParameterCoordinate,
        actual: TypeKind,
    },
}

impl PreparedCallSemanticOperand {
    pub(crate) const fn text_proxy_object(
        source: ExprId,
        argument: HirCallArgumentOrdinal,
        coordinate: CallableParameterCoordinate,
        actual: TypeKind,
    ) -> Self {
        Self::TextProxyNominalDiscriminator {
            argument,
            source,
            coordinate,
            actual,
        }
    }

    pub(crate) const fn owner(&self) -> PreparedCallSemanticOperandOwner {
        match self {
            Self::DialogueTarget { .. }
            | Self::DialogueContent { .. }
            | Self::DialogueLinePlan { .. } => {
                PreparedCallSemanticOperandOwner::DialogueApplication
            }
            Self::TextProxyNominalDiscriminator { .. } => {
                PreparedCallSemanticOperandOwner::TextProxyObject
            }
        }
    }

    pub(crate) const fn source(&self) -> ExprId {
        match self {
            Self::DialogueTarget { source, .. }
            | Self::DialogueContent { source, .. }
            | Self::DialogueLinePlan { source, .. }
            | Self::TextProxyNominalDiscriminator { source, .. } => *source,
        }
    }

    pub(crate) const fn argument(&self) -> Option<HirCallArgumentOrdinal> {
        match self {
            Self::TextProxyNominalDiscriminator { argument, .. } => Some(*argument),
            Self::DialogueTarget { .. }
            | Self::DialogueContent { .. }
            | Self::DialogueLinePlan { .. } => None,
        }
    }

    pub(crate) const fn role(&self) -> PreparedCallSemanticOperandRole {
        match self {
            Self::DialogueTarget { .. } => PreparedCallSemanticOperandRole::DialogueTarget,
            Self::DialogueContent { .. } => PreparedCallSemanticOperandRole::DialogueContent,
            Self::DialogueLinePlan { .. } => PreparedCallSemanticOperandRole::DialogueLinePlan,
            Self::TextProxyNominalDiscriminator { .. } => {
                PreparedCallSemanticOperandRole::TextProxyNominalDiscriminator
            }
        }
    }

    pub(crate) const fn coordinate(&self) -> CallableParameterCoordinate {
        match self {
            Self::DialogueTarget { coordinate, .. }
            | Self::DialogueContent { coordinate, .. }
            | Self::DialogueLinePlan { coordinate, .. }
            | Self::TextProxyNominalDiscriminator { coordinate, .. } => *coordinate,
        }
    }

    pub(crate) const fn actual(&self) -> &TypeKind {
        match self {
            Self::DialogueTarget { actual, .. }
            | Self::DialogueContent { actual, .. }
            | Self::DialogueLinePlan { actual, .. }
            | Self::TextProxyNominalDiscriminator { actual, .. } => actual,
        }
    }
}

/// Candidate-local attached-content input selected from one exact HIR
/// application. The callable schema owns presence, admission role, and
/// execution behavior; this carrier owns only whether a body was supplied and
/// the raw identity of that supplied content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreparedCallAttachedContentOperand {
    Omitted,
    Present { source: HirDialogueContentId },
}

impl PreparedCallAttachedContentOperand {
    pub(crate) const fn present(source: HirDialogueContentId) -> Self {
        Self::Present { source }
    }
}

/// Complete prepared call input authority.  Every call owns an ordinary
/// argument mapping (which may be empty) and a typed semantic-operand ledger
/// at the same time; the two projections are never mutually exclusive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedCallInputs {
    mapping: super::PreparedCallArgumentMapping,
    semantic_operands: Box<[PreparedCallSemanticOperand]>,
    attached_content: Option<PreparedCallAttachedContentOperand>,
}

impl PreparedCallInputs {
    pub(crate) fn new(
        mapping: super::PreparedCallArgumentMapping,
        semantic_operands: Box<[PreparedCallSemanticOperand]>,
        attached_content: Option<PreparedCallAttachedContentOperand>,
    ) -> Self {
        Self {
            mapping,
            semantic_operands,
            attached_content,
        }
    }

    /// Seals the structural inputs owned by a Dialogue content application.
    /// The ordinary mapping is deliberately present and empty so all call
    /// paths retain one composite authority.
    pub(crate) fn dialogue_application(
        candidate: &PreparedResolvedCallable,
        application: ExprId,
        target_expression: ExprId,
        target_actual: TypeKind,
        has_line_plan: bool,
    ) -> Result<Self, CallConstraintInvariant> {
        if candidate.id()
            != &CallableCandidateId::Dialogue(super::DialogueCallableId::ContentApplication)
            || candidate.call_group() != CallableGroupIndex::ZERO
            || candidate.schema().validator()
                != &CallableValidator::Dialogue(super::DialogueCallableId::ContentApplication)
        {
            return Err(CallConstraintInvariant::MalformedSchemaInventory);
        }
        let group = candidate
            .schema()
            .group(candidate.call_group())
            .ok_or(CallConstraintInvariant::MalformedSchemaInventory)?;
        let [target, content, line_plan] = group.parameters() else {
            return Err(CallConstraintInvariant::MalformedSchemaInventory);
        };
        if target.index().get() != 0
            || content.index().get() != 1
            || line_plan.index().get() != 2
            || target.presence() != CallableParameterPresence::Required
            || content.presence() != CallableParameterPresence::Required
            || line_plan.presence() != CallableParameterPresence::Optional
        {
            return Err(CallConstraintInvariant::MalformedSchemaInventory);
        }
        let content_actual = content
            .declared_type()
            .cloned()
            .ok_or(CallConstraintInvariant::MalformedSchemaInventory)?;
        let line_plan_actual = line_plan
            .declared_type()
            .cloned()
            .ok_or(CallConstraintInvariant::MalformedSchemaInventory)?;
        let group_index = group.index();
        let mut semantic_operands = Vec::with_capacity(if has_line_plan { 3 } else { 2 });
        semantic_operands.push(PreparedCallSemanticOperand::DialogueTarget {
            source: target_expression,
            coordinate: CallableParameterCoordinate::new(group_index, target.index()),
            actual: target_actual,
        });
        semantic_operands.push(PreparedCallSemanticOperand::DialogueContent {
            source: application,
            coordinate: CallableParameterCoordinate::new(group_index, content.index()),
            actual: content_actual,
        });
        if has_line_plan {
            semantic_operands.push(PreparedCallSemanticOperand::DialogueLinePlan {
                source: application,
                coordinate: CallableParameterCoordinate::new(group_index, line_plan.index()),
                actual: line_plan_actual,
            });
        }
        Ok(Self::new(
            super::PreparedCallArgumentMapping::empty(
                candidate.id().clone(),
                candidate.schema().semantic_digest(),
                group_index,
                usize::from(!has_line_plan),
            ),
            semantic_operands.into_boxed_slice(),
            None,
        ))
    }

    pub(crate) fn validates(&self, candidate: &PreparedResolvedCallable) -> bool {
        self.mapping.candidate() == Some(candidate.id())
            && self.mapping.schema() == candidate.schema().semantic_digest()
            && self.mapping.group() == candidate.call_group()
            && match (
                candidate
                    .schema()
                    .attached_content()
                    .filter(|parameter| parameter.group() == candidate.call_group()),
                self.attached_content,
            ) {
                (None, None) => true,
                (Some(parameter), Some(PreparedCallAttachedContentOperand::Omitted)) => {
                    parameter.presence() != CallableParameterPresence::Required
                }
                (Some(_), Some(PreparedCallAttachedContentOperand::Present { .. })) => true,
                (None, Some(_)) | (Some(_), None) => false,
            }
    }

    pub(crate) fn mapping(&self) -> &super::PreparedCallArgumentMapping {
        &self.mapping
    }

    pub(crate) fn semantic_operands(&self) -> &[PreparedCallSemanticOperand] {
        &self.semantic_operands
    }

    pub(crate) const fn attached_content(&self) -> Option<PreparedCallAttachedContentOperand> {
        self.attached_content
    }

    pub(crate) fn candidate(&self) -> Option<&CallableCandidateId> {
        self.mapping.candidate()
    }

    pub(crate) const fn schema(&self) -> super::CallableSignatureSchemaDigest {
        self.mapping.schema()
    }

    pub(crate) const fn group(&self) -> CallableGroupIndex {
        self.mapping.group()
    }

    pub(crate) fn omitted_parameters(&self) -> usize {
        self.mapping.omitted_parameters()
            + match self.attached_content {
                Some(PreparedCallAttachedContentOperand::Omitted) => 1,
                None | Some(PreparedCallAttachedContentOperand::Present { .. }) => 0,
            }
    }

    pub(crate) const fn unchecked_or_open_slots(&self) -> usize {
        self.mapping.unchecked_or_open_slots()
    }

    pub(crate) fn expression_sources(&self) -> Box<[ExprId]> {
        let mut sources = self.mapping.owned_expression_sources().into_vec();
        sources.extend(self.semantic_operands.iter().filter_map(|operand| {
            (operand.owner() == PreparedCallSemanticOperandOwner::DialogueApplication
                && operand.role() == PreparedCallSemanticOperandRole::DialogueTarget)
                .then_some(operand.source())
        }));
        sources.into_boxed_slice()
    }
}

impl PreparedCallCalleeConstraintInputs {
    pub(crate) const fn is_function_value(&self) -> bool {
        matches!(self, Self::FunctionValue { .. })
    }

    pub(crate) fn validates_candidate(&self, candidate: &PreparedResolvedCallable) -> bool {
        match self {
            Self::StaticContentCallee(static_callee) => {
                candidate.id() == &super::CallableCandidateId::Content(static_callee.identity())
                    && candidate.schema().semantic_digest() == static_callee.schema()
            }
            Self::Free
            | Self::EnumConstructor
            | Self::ValueReceiver { .. }
            | Self::AssociatedType { .. }
            | Self::DialogueCallee
            | Self::DialogueApplication
            | Self::FunctionValue { .. }
            | Self::NonCallable => true,
        }
    }

    /// Classifies an unresolved-dot source whose base path had no value fact.
    /// A language/free namespace has no callee expression to publish. An
    /// associated-type receiver publishes only its exact checked receiver
    /// type. No call result can be substituted for either role.
    pub(crate) fn nominal_callee_expression_type<'a>(
        &'a self,
        instantiation: &'a CallableInstantiation,
    ) -> Result<Option<&'a TypeKind>, super::CallConstraintInvariant> {
        match (self, instantiation) {
            (Self::EnumConstructor, CallableInstantiation::EnumConstructor) => Ok(None),
            (
                Self::Free,
                CallableInstantiation::None
                | CallableInstantiation::Result { .. }
                | CallableInstantiation::Option
                | CallableInstantiation::Character { .. },
            ) => Ok(None),
            (Self::StaticContentCallee(_), CallableInstantiation::None) => Ok(None),
            (Self::AssociatedType { actual }, CallableInstantiation::TypeReceiver { receiver })
                if actual == receiver.receiver() =>
            {
                Ok(Some(actual))
            }
            _ => Err(super::CallConstraintInvariant::PreparedBaseMismatch),
        }
    }
}

impl PreparedFinalCallCallee<'_> {
    pub(crate) fn into_function_value_origin(self) -> Option<PreparedFunctionValueOriginEvidence> {
        match self {
            Self::FunctionValue { value } => Some(value.into_origin()),
            Self::Free { .. }
            | Self::EnumConstructor { .. }
            | Self::Selected { .. }
            | Self::AssociatedType { .. }
            | Self::Dialogue { .. }
            | Self::NonCallableValue { .. } => None,
        }
    }

    pub(crate) fn constraint_inputs(&self) -> PreparedCallCalleeConstraintInputs {
        match self {
            Self::Free { .. } => PreparedCallCalleeConstraintInputs::Free,
            Self::EnumConstructor { .. } => PreparedCallCalleeConstraintInputs::EnumConstructor,
            Self::Selected {
                receiver_expression,
                receiver_type,
                ..
            } => PreparedCallCalleeConstraintInputs::ValueReceiver {
                source: *receiver_expression,
                actual: receiver_type.as_ref().clone(),
            },
            Self::AssociatedType { receiver, .. } => {
                PreparedCallCalleeConstraintInputs::AssociatedType {
                    actual: receiver.ty().clone(),
                }
            }
            Self::Dialogue { .. } => PreparedCallCalleeConstraintInputs::DialogueCallee,
            Self::FunctionValue { value } => PreparedCallCalleeConstraintInputs::FunctionValue {
                actual: value.actual().clone(),
            },
            Self::NonCallableValue { .. } => PreparedCallCalleeConstraintInputs::NonCallable,
        }
    }

    pub(crate) fn as_borrowed(&self) -> PreparedCallCallee<'_> {
        match self {
            Self::Free {
                path,
                project,
                scope,
                context,
            } => PreparedCallCallee::Free {
                path,
                project: project.as_deref(),
                scope: *scope,
                context: *context,
            },
            Self::EnumConstructor { seed } => PreparedCallCallee::EnumConstructor { seed },
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
                result: super::DialogueCallableResultContext::Declared,
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
pub(crate) struct FinalCallCalleeFacts<'a, P, U> {
    expressions: &'a BTreeMap<ExprId, PreparedExpressionFact>,
    prepared_calls: super::PreparedCallGraphIngress<'a, P, U>,
    nominal_receivers: &'a BTreeMap<TypeId, TypeResolutionReport>,
    function_value_origin: Option<PreparedFunctionValueOriginEvidence>,
}

impl<'a, P, U> FinalCallCalleeFacts<'a, P, U> {
    pub(crate) const fn new(
        expressions: &'a BTreeMap<ExprId, PreparedExpressionFact>,
        prepared_calls: super::PreparedCallGraphIngress<'a, P, U>,
        nominal_receivers: &'a BTreeMap<TypeId, TypeResolutionReport>,
        function_value_origin: Option<PreparedFunctionValueOriginEvidence>,
    ) -> Self {
        Self {
            expressions,
            prepared_calls,
            nominal_receivers,
            function_value_origin,
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
    #[error("call callee has no value result")]
    MissingValueType { expression: ExprId },
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
    #[error("function-value callee has no prepared origin evidence")]
    MissingFunctionValueOrigin { expression: ExprId },
    #[error("non-function callee retains function-value origin evidence")]
    UnexpectedFunctionValueOrigin { expression: ExprId },
    #[error("function-value origin evidence names a different callee")]
    InvalidFunctionValueOrigin { expression: ExprId },
    #[error("function-value callable schema could not be constructed")]
    InvalidFunctionSchema,
    #[error("checked enum variant cannot issue one accepted constructor case")]
    InvalidEnumVariantAuthority { expression: ExprId },
    #[error("checked Character project item has no exact Character identity")]
    InvalidCharacterIdentity { expression: ExprId },
    #[error("prepared continuation ingress is invalid: {0}")]
    PreparedContinuationInvariant(#[source] CallConstraintInvariant),
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

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AcceptedEnumVariantCase {
    id: super::EnumVariantSignatureId,
    schema: CallableSignatureSchema,
}

impl AcceptedEnumVariantCase {
    pub(crate) fn try_from_prepared(
        authority: CallResolverAuthority<'_>,
        prepared: &crate::final_analysis::PreparedVariantExpression,
        limits: &CallableLimits,
    ) -> Result<Self, super::CallableSchemaError> {
        let selected = prepared
            .owner()
            .case(prepared.selected_ordinal())
            .expect("prepared variant retains its selected case");
        let expected = prepared.owner().ty();
        let owner = expected.semantic_identity_digest()?;
        let payload = match selected.payload() {
            None => VariantPayloadShape::Unit,
            Some(payload) => payload
                .try_seal(
                    prepared.owner().payload_owner_family(),
                    owner,
                    selected.ordinal(),
                )
                .map_err(|_| super::CallableSchemaError::FamilyInvariant {
                    family: super::CallableFamily::EnumConstructor,
                    code: super::CallableFamilyInvariantCode::InvalidParameterType,
                })?,
        };
        let id = super::EnumVariantSignatureId::new(owner, selected.ordinal());
        let (issuer, template) = super::CallableGenericParameterIssuer::for_enum_constructor_type(
            &expected,
            authority.symbols,
        )?;
        if expected != template {
            return Err(super::CallableSchemaError::InvalidCandidateIssuer);
        }
        let schema = CallableSignatureSchema::for_accepted_enum_case(
            id.clone(),
            &payload,
            expected.clone(),
            issuer,
            limits,
        )?;
        Ok(Self { id, schema })
    }

    pub(crate) const fn id(&self) -> &super::EnumVariantSignatureId {
        &self.id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResolvedFunctionValueSeed {
    Lexical {
        id: LocalCallableId,
        schema: CallableSignatureSchema,
        effect_callable: Option<CallableId>,
    },
    Independent {
        id: FunctionValueSignatureId,
        schema: CallableSignatureSchema,
        effect_callable: Option<CallableId>,
    },
    PreparedContinuation {
        reference: super::PreparedCallContinuationRef,
    },
}

#[derive(Debug, Eq, Error, PartialEq)]
pub(crate) enum PreparedFunctionValueOriginQueryError {
    #[error("composite local function value cannot be prepared")]
    Composite,
    #[error("function-value local origin cycle")]
    Cycle,
    #[error("function-value local origin is invalid")]
    Invalid,
    #[error("terminal function-value capture fact belongs to another HIR topology: {0}")]
    CaptureTopologyMismatch(CheckedCaptureAuthorityViolation),
    #[error("terminal function-value capture fact names another producer: {0}")]
    CaptureProducerMismatch(CheckedCaptureAuthorityViolation),
    #[error("terminal function-value capture evidence differs from HIR topology: {0}")]
    CaptureEvidenceMismatch(CheckedCaptureAuthorityViolation),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PreparedFunctionValueOriginProducer {
    /// A typed call-origin observation.  The prepared graph later resolves
    /// this site to either a continuation or an independent terminal value.
    Call(super::CheckedCallSite),
    PreparedContinuation(super::CheckedCallSite),
    Lexical {
        local: LocalId,
    },
    IndependentExpression {
        producer: ExprId,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedFunctionValueOriginEvidence {
    callee: ExprId,
    producer: PreparedFunctionValueOriginProducer,
    captures: Box<[super::PreparedCaptureIdentityRow]>,
}

impl PreparedFunctionValueOriginEvidence {
    fn new(
        callee: ExprId,
        producer: PreparedFunctionValueOriginProducer,
        captures: impl Into<Box<[super::PreparedCaptureIdentityRow]>>,
    ) -> Self {
        Self {
            callee,
            producer,
            captures: captures.into(),
        }
    }

    pub(crate) const fn producer(&self) -> &PreparedFunctionValueOriginProducer {
        &self.producer
    }

    pub(crate) const fn captures(&self) -> &[super::PreparedCaptureIdentityRow] {
        &self.captures
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        ExprId,
        PreparedFunctionValueOriginProducer,
        Box<[super::PreparedCaptureIdentityRow]>,
    ) {
        (self.callee, self.producer, self.captures)
    }
}

pub(crate) struct PreparedFunctionValueOriginQuery {
    topology: Arc<HirProjectEvaluationTopology>,
    module: HirModuleId,
    callee: ExprId,
    current: ExprId,
    visited: std::collections::BTreeSet<LocalId>,
    pending_capture_rows: Arc<BTreeMap<ExprId, Box<[PreparedCaptureIdentityRow]>>>,
}

pub(crate) struct PreparedFunctionValueOriginNeed {
    query: PreparedFunctionValueOriginQuery,
    expression: ExprId,
}

impl PreparedFunctionValueOriginNeed {
    pub(crate) const fn expression(&self) -> ExprId {
        self.expression
    }

    pub(crate) fn resume(
        self,
        owner: ExprId,
        checked: &PreparedExpressionFact,
        module: &HirModule,
    ) -> Result<PreparedFunctionValueOriginProgress, PreparedFunctionValueOriginQueryError> {
        if owner != self.expression
            || owner != self.query.current
            || module.module_id() != self.query.module
        {
            return Err(PreparedFunctionValueOriginQueryError::Invalid);
        }
        self.query.advance_with_fact(module, checked)
    }
}

pub(crate) enum PreparedFunctionValueOriginProgress {
    Need(PreparedFunctionValueOriginNeed),
    Ready(PreparedFunctionValueOriginEvidence),
}

impl PreparedFunctionValueOriginQuery {
    fn validated_terminal_capture_rows(
        &self,
        producer: ExprId,
        fact: &CheckedExpression,
    ) -> Result<Box<[super::PreparedCaptureIdentityRow]>, PreparedFunctionValueOriginQueryError>
    {
        let captures = match fact.resolution() {
            CheckedExpressionResolution::ImplicitCallable(callable) => callable
                .validate_authority(&self.topology, producer)
                .map(|captures| {
                    captures
                        .iter()
                        .map(|capture| {
                            super::PreparedCaptureIdentityRow::new(
                                capture.lookup_local(),
                                capture.mode(),
                            )
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice()
                }),
            CheckedExpressionResolution::Closure(closure) => closure
                .validate_authority(&self.topology, producer)
                .map(|captures| {
                    captures
                        .iter()
                        .map(|capture| {
                            super::PreparedCaptureIdentityRow::new(capture.local(), capture.mode())
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice()
                }),
            _ => return Ok(Box::new([])),
        }
        .map_err(|violation| match &violation {
            CheckedCaptureAuthorityViolation::TopologyMismatch => {
                PreparedFunctionValueOriginQueryError::CaptureTopologyMismatch(violation)
            }
            CheckedCaptureAuthorityViolation::ProducerMismatch { .. } => {
                PreparedFunctionValueOriginQueryError::CaptureProducerMismatch(violation)
            }
            CheckedCaptureAuthorityViolation::MissingProducer { .. }
            | CheckedCaptureAuthorityViolation::GenericScope(_)
            | CheckedCaptureAuthorityViolation::MissingExpressionUse { .. }
            | CheckedCaptureAuthorityViolation::MissingLocalBinding { .. }
            | CheckedCaptureAuthorityViolation::InternalLocalBinding { .. }
            | CheckedCaptureAuthorityViolation::DuplicateUse { .. }
            | CheckedCaptureAuthorityViolation::PlaceholderEvidenceMismatch
            | CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch
            | CheckedCaptureAuthorityViolation::IdentityCoordinateEncoding => {
                PreparedFunctionValueOriginQueryError::CaptureEvidenceMismatch(violation)
            }
        })?;
        Ok(captures)
    }

    fn start(
        topology: Arc<HirProjectEvaluationTopology>,
        module: &HirModule,
        callee: ExprId,
        pending_capture_rows: Arc<BTreeMap<ExprId, Box<[PreparedCaptureIdentityRow]>>>,
    ) -> Self {
        Self {
            topology,
            module: module.module_id(),
            callee,
            current: callee,
            visited: std::collections::BTreeSet::new(),
            pending_capture_rows,
        }
    }

    fn local_origins(
        &self,
        module: &HirModule,
    ) -> Result<
        &arcweft_lang_hir::project::HirLocalBindingOriginIndex,
        PreparedFunctionValueOriginQueryError,
    > {
        let topology = self
            .topology
            .module(self.module)
            .ok_or(PreparedFunctionValueOriginQueryError::Invalid)?;
        (topology.snapshot() == module.snapshot_id())
            .then_some(topology.local_origins())
            .ok_or(PreparedFunctionValueOriginQueryError::Invalid)
    }

    fn advance(
        self,
        module: &HirModule,
        checked: &BTreeMap<ExprId, PreparedExpressionFact>,
    ) -> Result<PreparedFunctionValueOriginProgress, PreparedFunctionValueOriginQueryError> {
        self.local_origins(module)?;
        let record = module
            .resolve_expr(self.current)
            .map_err(|_| PreparedFunctionValueOriginQueryError::Invalid)?;
        if matches!(record.kind(), HirExprKind::Call(_)) {
            return Ok(PreparedFunctionValueOriginProgress::Ready(
                PreparedFunctionValueOriginEvidence::new(
                    self.callee,
                    PreparedFunctionValueOriginProducer::Call(super::CheckedCallSite::HirCall(
                        self.current,
                    )),
                    Vec::new(),
                ),
            ));
        }
        let Some(fact) = checked.get(&self.current) else {
            return Ok(PreparedFunctionValueOriginProgress::Need(
                PreparedFunctionValueOriginNeed {
                    expression: self.current,
                    query: self,
                },
            ));
        };
        self.advance_with_fact(module, fact)
    }

    fn advance_with_fact(
        mut self,
        module: &HirModule,
        fact: &PreparedExpressionFact,
    ) -> Result<PreparedFunctionValueOriginProgress, PreparedFunctionValueOriginQueryError> {
        if let PreparedExpressionFact::OwnerBound(owner_bound) = fact {
            if matches!(
                owner_bound.resolution(),
                PreparedOwnerBoundResolution::ImplicitCallable(_)
            ) {
                let captures = self
                    .pending_capture_rows
                    .get(&self.current)
                    .cloned()
                    .ok_or(PreparedFunctionValueOriginQueryError::Invalid)?;
                return Ok(PreparedFunctionValueOriginProgress::Ready(
                    PreparedFunctionValueOriginEvidence::new(
                        self.callee,
                        PreparedFunctionValueOriginProducer::IndependentExpression {
                            producer: self.current,
                        },
                        captures,
                    ),
                ));
            }
            return Err(PreparedFunctionValueOriginQueryError::Invalid);
        }
        let fact = fact
            .complete()
            .ok_or(PreparedFunctionValueOriginQueryError::Invalid)?;
        let CheckedExpressionResolution::Value(CheckedValueResolution::Local(local)) =
            fact.resolution()
        else {
            return Ok(PreparedFunctionValueOriginProgress::Ready(
                PreparedFunctionValueOriginEvidence::new(
                    self.callee,
                    PreparedFunctionValueOriginProducer::IndependentExpression {
                        producer: self.current,
                    },
                    self.validated_terminal_capture_rows(self.current, fact)?,
                ),
            ));
        };
        if !self.visited.insert(*local) {
            return Err(PreparedFunctionValueOriginQueryError::Cycle);
        }
        let local_origins = self.local_origins(module)?;
        let origin = local_origins
            .origin(*local)
            .ok_or(PreparedFunctionValueOriginQueryError::Invalid)?;
        match origin {
            HirLocalValueOrigin::DirectInitializer(initializer) => {
                if initializer.module() != module.module_id() {
                    return Err(PreparedFunctionValueOriginQueryError::Invalid);
                }
                self.current = initializer;
                let initializer_record = module
                    .resolve_expr(initializer)
                    .map_err(|_| PreparedFunctionValueOriginQueryError::Invalid)?;
                if matches!(initializer_record.kind(), HirExprKind::Call(_)) {
                    return Ok(PreparedFunctionValueOriginProgress::Ready(
                        PreparedFunctionValueOriginEvidence::new(
                            self.callee,
                            PreparedFunctionValueOriginProducer::Call(
                                super::CheckedCallSite::HirCall(initializer),
                            ),
                            Vec::new(),
                        ),
                    ));
                }
                Ok(PreparedFunctionValueOriginProgress::Need(
                    PreparedFunctionValueOriginNeed {
                        expression: initializer,
                        query: self,
                    },
                ))
            }
            HirLocalValueOrigin::Independent => Ok(PreparedFunctionValueOriginProgress::Ready(
                PreparedFunctionValueOriginEvidence::new(
                    self.callee,
                    PreparedFunctionValueOriginProducer::Lexical { local: *local },
                    Vec::new(),
                ),
            )),
            HirLocalValueOrigin::Composite => Err(PreparedFunctionValueOriginQueryError::Composite),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedFunctionValueCallee {
    actual: TypeKind,
    origin: PreparedFunctionValueOriginEvidence,
    seed: ResolvedFunctionValueSeed,
}

impl PreparedFunctionValueCallee {
    pub(crate) fn new(
        actual: TypeKind,
        origin: PreparedFunctionValueOriginEvidence,
        seed: ResolvedFunctionValueSeed,
    ) -> Self {
        Self {
            actual,
            origin,
            seed,
        }
    }

    pub(crate) const fn actual(&self) -> &TypeKind {
        &self.actual
    }

    fn into_origin(self) -> PreparedFunctionValueOriginEvidence {
        self.origin
    }

    pub(crate) const fn origin(&self) -> &PreparedFunctionValueOriginEvidence {
        &self.origin
    }

    pub(crate) const fn seed(&self) -> &ResolvedFunctionValueSeed {
        &self.seed
    }
}

pub(crate) struct CallResolverRequest<'a> {
    callee: PreparedCallCallee<'a>,
    authority: CallResolverAuthority<'a>,
    checked: CheckedCallResolverAuthority<'a>,
    presentation_character_owner: Option<&'a ResolvedCharacterOwner>,
    call: Option<&'a HirCallInvocation>,
    classification: CallCalleeClassificationFact,
    cancellation: &'a AtomicBool,
    prepared_continuations: &'a dyn super::PreparedCallContinuationAuthority,
    work: &'a mut ResolverWork,
    limits: &'a CallableLimits,
    implicit_extension_receiver: Option<PreparedImplicitExtensionReceiver>,
}

/// Immutable authorities and controls for one final-HIR call-resolution query.
///
/// The source expression is validated against `authority` before the request
/// becomes observable. Grouping these values prevents resolver construction
/// from becoming a positional list of unrelated identities and budgets.
pub(crate) struct CallResolverContext<'a> {
    pub(crate) authority: CallResolverAuthority<'a>,
    pub(crate) checked: CheckedCallResolverAuthority<'a>,
    pub(crate) presentation_character_owner: Option<&'a ResolvedCharacterOwner>,
    pub(crate) expression: ExprId,
    pub(crate) cancellation: &'a AtomicBool,
    pub(crate) prepared_continuations: &'a dyn super::PreparedCallContinuationAuthority,
    pub(crate) limits: &'a CallableLimits,
    pub(crate) implicit_extension_receiver: Option<PreparedImplicitExtensionReceiver>,
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

    fn exact_method(self, key: &ReceiverMethodKey) -> super::CheckedMethodLookup {
        match self {
            Self::Pending(builder) => builder.exact_method(key),
            Self::Frozen(catalog) => catalog.exact_method(key),
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
    ) -> Result<(&'a HirCallInvocation, CallCalleeClassificationFact), ResolveCallError> {
        let expression = self.validate_expression(expression, limits)?;
        let call = match expression.kind() {
            HirExprKind::Call(call) => call,
            HirExprKind::AttachedContentApplication(application) => {
                let HirAttachedContentApplicationFamily::ContentCall { invocation, .. } =
                    application.family()
                else {
                    return Err(ResolveCallError::InvalidResolvedCallable);
                };
                invocation
            }
            _ => return Err(ResolveCallError::InvalidResolvedCallable),
        };
        let classification = classify_prepared_callee(callee, call, self.module)?;
        Ok((call, classification))
    }

    fn validate_dialogue_application(
        self,
        callee: &PreparedCallCallee<'_>,
        expression: ExprId,
        limits: &CallableLimits,
    ) -> Result<
        (
            &'a HirAttachedContentApplication,
            CallCalleeClassificationFact,
        ),
        ResolveCallError,
    > {
        let expression = self.validate_expression(expression, limits)?;
        let HirExprKind::AttachedContentApplication(application) = expression.kind() else {
            return Err(ResolveCallError::InvalidResolvedCallable);
        };
        let HirAttachedContentApplicationFamily::DialogueLine {
            target,
            plan,
            coordinates,
        } = application.family()
        else {
            return Err(ResolveCallError::InvalidResolvedCallable);
        };
        let _ = (plan, coordinates);
        let PreparedCallCallee::Dialogue {
            id,
            callee,
            result: super::DialogueCallableResultContext::ContentApplication { .. },
            ..
        } = callee
        else {
            return Err(ResolveCallError::InvalidResolvedCallable);
        };
        if *id != super::DialogueCallableId::ContentApplication || !id.supports_callee(callee) {
            return Err(ResolveCallError::InvalidResolvedCallable);
        }
        Ok((
            application,
            CallCalleeClassificationFact::Value {
                expression: *target,
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
            presentation_character_owner: context.presentation_character_owner,
            call: Some(call),
            classification,
            cancellation: context.cancellation,
            prepared_continuations: context.prepared_continuations,
            work,
            limits: context.limits,
            implicit_extension_receiver: context.implicit_extension_receiver.clone(),
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
            presentation_character_owner: context.presentation_character_owner,
            call: None,
            classification,
            cancellation: context.cancellation,
            prepared_continuations: context.prepared_continuations,
            work,
            limits: context.limits,
            implicit_extension_receiver: None,
        })
    }

    pub(crate) const fn parenthesized_call(&self) -> Option<&'a HirCallInvocation> {
        self.call
    }
    pub(crate) const fn classification(&self) -> CallCalleeClassificationFact {
        self.classification
    }
}
