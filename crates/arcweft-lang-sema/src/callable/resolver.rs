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
pub(crate) use preparation::{
    prepare_final_call_callee, prepare_function_value_origin_query, prepare_language_free_dot_path,
    prepare_presentation_callee_id,
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
    dialogue_application::HirDialogueContentApplication,
    expr::{
        HirAssociatedCallSyntax, HirAssociatedReceiver, HirAssociatedSeparator, HirCallArgument,
        HirCallCallee, HirCallExpr, HirExpr, HirExprKind, HirRecoveredName, HirSelectedMember,
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
        CheckedValueResolution, PreparedExpressionFact,
    },
    nominal::{ResolvedAssociatedTypeReceiver, TypeResolutionReport},
    registration::RegisteredSemanticWorld,
    types::{TypeKind, VariantPayloadOwnerFamily, VariantPayloadShape},
};

use super::CharacterDialoguePatchContext;
use super::{
    AdapterPackageId, AgentIntrinsicSignatureId, BuiltinCallableId, CallCalleeClassificationFact,
    CallConstraintInvariant, CallableAuthorityRank, CallableCandidateId, CallableFamily,
    CallableGroupIndex, CallableLimits, CallableLookupKey, CallableName,
    CallableParameterCoordinate, CallableParameterIndex, CallableParameterPresence, CallablePath,
    CallableRecord, CallableSignatureSchema, CallableValidator, CapacityMethodId,
    CheckedCallableDeclaration, CheckedCallableId, CheckedMethodLookup, CollectionMethodId,
    CorruptCallableCatalogReason, DomainMethodId, EnvironmentCallableId, EnvironmentCallableKind,
    EnvironmentCallableOwner, EquivalentCallableSource, FunctionValueOrdinal,
    FunctionValueSignatureId, FxCallableSignatureId, FxResolution, IntegerMethodId,
    LanguageCallableFamily, LineContextMethodId, LineScheduleCallableId, LocalCallableId,
    OptionConstructorKind, PresentationCallableId, PresentationHandleMethodId,
    PresentationSchemaContext, ProjectCallablePath, ProjectNameBinding, PromotionCallableId,
    ReceiverMethodKey, ResolveCallError, ResolverWork, ResultConstructorKind, StageMethodId,
    StandardEnvironmentId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedCallCallee<'a> {
    Free {
        path: &'a CallablePath,
        project: Option<&'a CallableDeclarationKey>,
        scope: PreparedFreeCallScope,
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
    ExpectedEnum { expected: TypeKind },
    ValueReceiver { source: ExprId, actual: TypeKind },
    AssociatedType { actual: TypeKind },
    DialogueCallee,
    DialogueApplication,
    FunctionValue { actual: TypeKind },
    NonCallable,
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

/// Schema-sealed source role for one non-runtime Dialogue application
/// operand.  Content and line-plan rows are relative to the enclosing checked
/// application site; the target keeps its exact HIR expression source so C1
/// can pair it with the corresponding stable expression coordinate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum PreparedDialogueCallOperandSource {
    Target { expression: ExprId },
    Content,
    LinePlan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedDialogueCallOperand {
    source: PreparedDialogueCallOperandSource,
    coordinate: CallableParameterCoordinate,
    actual: TypeKind,
}

impl PreparedDialogueCallOperand {
    pub(crate) const fn source(&self) -> PreparedDialogueCallOperandSource {
        self.source
    }

    pub(crate) const fn coordinate(&self) -> CallableParameterCoordinate {
        self.coordinate
    }

    pub(crate) const fn actual(&self) -> &TypeKind {
        &self.actual
    }
}

/// The sole prepared authority for the structural operands of a Dialogue
/// content application.  These operands participate in schema admission and
/// lower constraint closure but are never projected as authored/runtime call
/// arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedDialogueCallConstraintInputs {
    candidate: CallableCandidateId,
    schema: super::CallableSignatureSchemaDigest,
    group: CallableGroupIndex,
    operands: Box<[PreparedDialogueCallOperand]>,
}

/// Mutually exclusive source inventory for one prepared call.  Ordinary HIR
/// calls own an authored mapper seal; Dialogue content applications own their
/// schema-sealed structural operands here and therefore have no authored
/// mapping at all.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedCallInputProjection {
    Authored(super::PreparedCallArgumentMapping),
    SemanticOnly(PreparedDialogueCallConstraintInputs),
}

impl PreparedCallInputProjection {
    pub(crate) fn authored(&self) -> Option<&super::PreparedCallArgumentMapping> {
        match self {
            Self::Authored(mapping) => Some(mapping),
            Self::SemanticOnly(_) => None,
        }
    }

    pub(crate) const fn semantic_only(&self) -> Option<&PreparedDialogueCallConstraintInputs> {
        match self {
            Self::Authored(_) => None,
            Self::SemanticOnly(inputs) => Some(inputs),
        }
    }

    pub(crate) fn candidate(&self) -> Option<&CallableCandidateId> {
        match self {
            Self::Authored(mapping) => mapping.candidate(),
            Self::SemanticOnly(inputs) => Some(inputs.candidate()),
        }
    }

    pub(crate) fn schema(&self) -> Option<super::CallableSignatureSchemaDigest> {
        match self {
            Self::Authored(mapping) => Some(mapping.schema()),
            Self::SemanticOnly(inputs) => Some(inputs.schema()),
        }
    }

    pub(crate) fn group(&self) -> Option<CallableGroupIndex> {
        match self {
            Self::Authored(mapping) => Some(mapping.group()),
            Self::SemanticOnly(inputs) => Some(inputs.group()),
        }
    }

    pub(crate) fn omitted_parameters(&self) -> usize {
        match self {
            Self::Authored(mapping) => mapping.omitted_parameters(),
            Self::SemanticOnly(inputs) => usize::from(inputs.operands().len() == 2),
        }
    }

    pub(crate) fn unchecked_or_open_slots(&self) -> usize {
        self.authored().map_or(
            0,
            super::PreparedCallArgumentMapping::unchecked_or_open_slots,
        )
    }

    /// Expression-backed semantic dependencies in source order. Structural
    /// content/line-plan markers are site-relative and therefore do not mint
    /// fake expression owners.
    pub(crate) fn expression_sources(&self) -> Box<[ExprId]> {
        match self {
            Self::Authored(mapping) => mapping.owned_expression_sources(),
            Self::SemanticOnly(inputs) => inputs
                .operands()
                .iter()
                .filter_map(|operand| match operand.source() {
                    PreparedDialogueCallOperandSource::Target { expression } => Some(expression),
                    PreparedDialogueCallOperandSource::Content
                    | PreparedDialogueCallOperandSource::LinePlan => None,
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }
}

impl PreparedDialogueCallConstraintInputs {
    pub(crate) fn seal(
        candidate: &PreparedResolvedCallable,
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
        let mut operands = Vec::with_capacity(if has_line_plan { 3 } else { 2 });
        operands.push(PreparedDialogueCallOperand {
            source: PreparedDialogueCallOperandSource::Target {
                expression: target_expression,
            },
            coordinate: CallableParameterCoordinate::new(group_index, target.index()),
            actual: target_actual,
        });
        operands.push(PreparedDialogueCallOperand {
            source: PreparedDialogueCallOperandSource::Content,
            coordinate: CallableParameterCoordinate::new(group_index, content.index()),
            actual: content_actual,
        });
        if has_line_plan {
            operands.push(PreparedDialogueCallOperand {
                source: PreparedDialogueCallOperandSource::LinePlan,
                coordinate: CallableParameterCoordinate::new(group_index, line_plan.index()),
                actual: line_plan_actual,
            });
        }
        Ok(Self {
            candidate: candidate.id().clone(),
            schema: candidate.schema().semantic_digest(),
            group: group_index,
            operands: operands.into_boxed_slice(),
        })
    }

    pub(crate) fn validates(&self, candidate: &PreparedResolvedCallable) -> bool {
        self.candidate == *candidate.id()
            && self.schema == candidate.schema().semantic_digest()
            && self.group == candidate.call_group()
    }

    pub(crate) const fn candidate(&self) -> &CallableCandidateId {
        &self.candidate
    }

    pub(crate) const fn schema(&self) -> super::CallableSignatureSchemaDigest {
        self.schema
    }

    pub(crate) const fn group(&self) -> CallableGroupIndex {
        self.group
    }

    pub(crate) fn operands(&self) -> &[PreparedDialogueCallOperand] {
        &self.operands
    }
}

impl PreparedCallCalleeConstraintInputs {
    pub(crate) const fn is_function_value(&self) -> bool {
        matches!(self, Self::FunctionValue { .. })
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
            (
                Self::ExpectedEnum { expected },
                CallableInstantiation::ExpectedEnum {
                    expected: instantiated,
                },
            ) if expected == instantiated => Ok(None),
            (
                Self::Free,
                CallableInstantiation::None
                | CallableInstantiation::Result { .. }
                | CallableInstantiation::Option
                | CallableInstantiation::Character { .. },
            ) => Ok(None),
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
            Self::EnumConstructor { seed } => PreparedCallCalleeConstraintInputs::ExpectedEnum {
                expected: seed.expected.clone(),
            },
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
            } => PreparedCallCallee::Free {
                path,
                project: project.as_deref(),
                scope: *scope,
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
    case_ordinal: u32,
    expected: TypeKind,
    schema: CallableSignatureSchema,
}

impl AcceptedEnumVariantCase {
    pub(crate) fn try_from_checked(
        checked: &PreparedExpressionFact,
        limits: &CallableLimits,
    ) -> Result<Option<Self>, super::CallableSchemaError> {
        let (owner, ordinal, payload, expected) = match checked {
            PreparedExpressionFact::ProjectVariant(prepared) => {
                let Some(selected_index) = usize::try_from(prepared.selected_ordinal()).ok() else {
                    return Ok(None);
                };
                let selected = prepared
                    .owner()
                    .cases()
                    .get(selected_index)
                    .filter(|case| case.ordinal() == prepared.selected_ordinal());
                let Some(selected) = selected else {
                    return Ok(None);
                };
                let owner = prepared.owner().nominal().identity();
                let payload = match selected.payload() {
                    None => VariantPayloadShape::Unit,
                    Some(payload) => VariantPayloadShape::try_tuple(
                        VariantPayloadOwnerFamily::Project,
                        owner,
                        selected.ordinal(),
                        [payload.clone()],
                    )
                    .map_err(|_| {
                        super::CallableSchemaError::FamilyInvariant {
                            family: super::CallableFamily::EnumConstructor,
                            code: super::CallableFamilyInvariantCode::InvalidParameterType,
                        }
                    })?,
                };
                (
                    owner,
                    selected.ordinal(),
                    payload,
                    prepared.owner().nominal().ty(),
                )
            }
            PreparedExpressionFact::Complete(checked) => {
                let CheckedExpressionResolution::Variant(variant) = checked.resolution() else {
                    return Ok(None);
                };
                if !variant.owner().has_valid_case_rows() {
                    return Ok(None);
                }
                let selected = variant.selected();
                if checked.ty().semantic_identity_digest() != variant.owner().semantic_type() {
                    return Ok(None);
                }
                (
                    variant.owner().semantic_type(),
                    selected.ordinal(),
                    selected.payload().clone(),
                    checked.ty().clone(),
                )
            }
            PreparedExpressionFact::DialogueApplication(_)
            | PreparedExpressionFact::Method(_)
            | PreparedExpressionFact::Entry(_)
            | PreparedExpressionFact::ProjectField(_)
            | PreparedExpressionFact::ProjectRecord(_) => return Ok(None),
        };
        if checked.ty() != &expected {
            return Ok(None);
        }
        let id = super::EnumVariantSignatureId::new(owner, ordinal);
        let schema = CallableSignatureSchema::for_accepted_enum_case(
            id.clone(),
            &payload,
            expected.clone(),
            limits,
        )?;
        Ok(Some(Self {
            id,
            case_ordinal: ordinal,
            expected,
            schema,
        }))
    }

    pub(crate) const fn id(&self) -> &super::EnumVariantSignatureId {
        &self.id
    }

    pub(crate) const fn case_ordinal(&self) -> u32 {
        self.case_ordinal
    }

    pub(crate) const fn expected(&self) -> &TypeKind {
        &self.expected
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
            CheckedExpressionResolution::ImplicitCallable(callable) => {
                callable.validate_authority(&self.topology, producer)
            }
            CheckedExpressionResolution::Closure(closure) => {
                closure.validate_authority(&self.topology, producer)
            }
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
            | CheckedCaptureAuthorityViolation::MissingExpressionUse { .. }
            | CheckedCaptureAuthorityViolation::MissingLocalBinding { .. }
            | CheckedCaptureAuthorityViolation::InternalLocalBinding { .. }
            | CheckedCaptureAuthorityViolation::DuplicateUse { .. }
            | CheckedCaptureAuthorityViolation::PlaceholderEvidenceMismatch
            | CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch => {
                PreparedFunctionValueOriginQueryError::CaptureEvidenceMismatch(violation)
            }
        })?;
        Ok(captures
            .iter()
            .map(|capture| super::PreparedCaptureIdentityRow::new(capture.local(), capture.mode()))
            .collect::<Vec<_>>()
            .into_boxed_slice())
    }

    fn start(
        topology: Arc<HirProjectEvaluationTopology>,
        module: &HirModule,
        callee: ExprId,
    ) -> Self {
        Self {
            topology,
            module: module.module_id(),
            callee,
            current: callee,
            visited: std::collections::BTreeSet::new(),
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
    call: Option<&'a HirCallExpr>,
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
    ) -> Result<(&'a HirCallExpr, CallCalleeClassificationFact), ResolveCallError> {
        let expression = self.validate_expression(expression, limits)?;
        let HirExprKind::Call(call) = expression.kind() else {
            return Err(ResolveCallError::InvalidResolvedCallable);
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
            &'a HirDialogueContentApplication,
            CallCalleeClassificationFact,
        ),
        ResolveCallError,
    > {
        let expression = self.validate_expression(expression, limits)?;
        let HirExprKind::DialogueContentApplication(application) = expression.kind() else {
            return Err(ResolveCallError::InvalidResolvedCallable);
        };
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

    pub(crate) const fn parenthesized_call(&self) -> Option<&'a HirCallExpr> {
        self.call
    }
    pub(crate) const fn classification(&self) -> CallCalleeClassificationFact {
        self.classification
    }
}
