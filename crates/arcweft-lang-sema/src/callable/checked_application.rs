//! Final checked-call authority and its canonical version-1 encoders.
//!
//! Prepared resolver objects and graph references stop at the C-seal boundary.
//! Every constructor consumes typed evidence, derives its own digest, and
//! retains no caller-supplied bytes or digest. A callable is one shared base in
//! either its base state or the exact continuation state produced earlier.

use std::{collections::BTreeSet, sync::Arc};

use arcweft_id::dialogue::{DialogueLineId, DialogueTextKey};
use arcweft_lang_hir::{expr::HirCallArgumentOrdinal, identity::ExprId, scope::CaptureAccess};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;

use crate::{
    effect_row::{EffectRow, EffectRowTail, EffectVar, EffectVarIssuer},
    semantic_coordinate::{
        CheckedBindingCoordinateEvidence, CheckedExpressionCoordinateEvidence, CheckedSemanticPath,
        StableCheckedBindingCoordinate, StableCheckedValueCoordinate,
    },
    types::{
        ArrayLength, CheckedConstraintContainerConstructor, CheckedConstraintSourceProjection,
        GenericConstParameterId, MapKind, SemanticTypeDigest, TypeKind,
        constraints::TypeConstraintSolution,
    },
};

use super::{
    AgentIntrinsicSignatureId, BuiltinCallableId, CallConstraintInvariant,
    CallableArgumentSemanticAction, CallableArgumentSlotIndex, CallableAuthorityRank,
    CallableCandidateId, CallableFamily, CallableGroupIndex, CallableLimits,
    CallableParameterAdmission, CallableParameterCoordinate, CallableParameterIndex,
    CallableParameterValueAlternative, CallableReceiverMode, CallableSignatureSchema,
    CallableSignatureSchemaDigest, CapabilityCallableId, CheckedCallArgumentSlotSource,
    CheckedCallableDigest, CheckedCallableId, CheckedSemanticValueEvidence, CollectionMethodId,
    DialogueCallableId, DropCallableId, EquivalentCallableSource, FloatWidth, FunctionValueOrdinal,
    FxCallableSignatureId, IntegerMethodId, LanguageCallableFamily, LineContextMethodId,
    LineScheduleCallableId, MathCallableId, OpenArgumentId, OptionConstructorKind,
    PreparedCallableEffectInstantiationEvidence, PreparedCaptureIdentityRow,
    PreparedDialogueCalleeIdentity, PreparedFunctionValueOriginIdentity,
    PreparedResolvedCallableDefinitionSealInput, PreparedResolvedCallableIdentity,
    PresentationCallableId, PresentationHandleMethodId, ProbeComparisonOperator,
    PromotionCallableId, ResolvedCharacterOwner, ResultConstructorKind, SignatureOrigin,
    StageMethodId, StdFloatCallableId, StdFloatOperation, TypeReceiverInstantiation,
    VariantPayloadRequirement, VectorDimensions,
};

const RESOLVED_CALLABLE_DOMAIN: &[u8] = b"arcweft.lang.resolved-callable.v1\0";
const FROZEN_SOLUTION_DOMAIN: &[u8] = b"arcweft.lang.call-type-solution.v1\0";
const CANDIDATE_INVENTORY_DOMAIN: &[u8] = b"arcweft.lang.call-candidate-inventory.v1\0";
const CONTINUATION_DOMAIN: &[u8] = b"arcweft.lang.call-continuation.v1\0";
const APPLICATION_CORE_DOMAIN: &[u8] = b"arcweft.lang.checked-call-application-core.v1\0";
const APPLICATION_DOMAIN: &[u8] = b"arcweft.lang.checked-call-application.v1\0";

macro_rules! digest_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

digest_type!(ResolvedCallableDigest);
digest_type!(FrozenCallTypeSolutionDigest);
digest_type!(CheckedCallCandidateInventoryDigest);
digest_type!(CheckedCallApplicationCoreDigest);
digest_type!(CheckedCallContinuationDigest);
digest_type!(CheckedCallApplicationDigest);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedDialogueCalleeIdentity {
    Character {
        character: crate::types::CharacterDialogueCharacterType,
    },
    CharacterDialogue {
        character: crate::types::CharacterDialogueCharacterType,
    },
    Content {
        module: CanonicalModulePath,
        path: super::CallablePath,
    },
}

impl ResolvedDialogueCalleeIdentity {
    pub(crate) fn from_callee(
        callee: &super::DialogueCalleeIdentity,
        module: &CanonicalModulePath,
    ) -> Self {
        match callee {
            super::DialogueCalleeIdentity::Character { character } => Self::Character {
                character: character.clone(),
            },
            super::DialogueCalleeIdentity::CharacterDialogue { character } => {
                Self::CharacterDialogue {
                    character: character.clone(),
                }
            }
            super::DialogueCalleeIdentity::Content { path } => Self::Content {
                module: module.clone(),
                path: path.clone(),
            },
        }
    }
}

/// Prepared-reference-free diagnostic origin. Stable hashing is owned by
/// [`ResolvedCallableStableIdentity`], never this issuer view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedCallableOrigin {
    Project {
        declaration: arcweft_lang_hir::symbol::CallableDeclarationKey,
        binding: Option<super::ProjectCallablePath>,
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
    LanguageDialogue {
        operation: DialogueCallableId,
        callee: ResolvedDialogueCalleeIdentity,
    },
    Lexical {
        binding: StableCheckedBindingCoordinate,
    },
    FunctionValue {
        expression: CheckedSemanticPath,
        ordinal: FunctionValueOrdinal,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedCaptureMode {
    Read,
    Reassign,
}

impl From<CaptureAccess> for CheckedCaptureMode {
    fn from(value: CaptureAccess) -> Self {
        match value {
            CaptureAccess::Read => Self::Read,
            CaptureAccess::Reassign => Self::Reassign,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCaptureSignatureRow {
    binding: StableCheckedBindingCoordinate,
    mode: CheckedCaptureMode,
    ty: SemanticTypeDigest,
}

impl CheckedCaptureSignatureRow {
    pub const fn binding(&self) -> &StableCheckedBindingCoordinate {
        &self.binding
    }
    pub const fn mode(&self) -> CheckedCaptureMode {
        self.mode
    }
    pub const fn ty(&self) -> SemanticTypeDigest {
        self.ty
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedLexicalCallableIdentity {
    binding: StableCheckedBindingCoordinate,
    effects: EffectRow,
}

impl CheckedLexicalCallableIdentity {
    pub const fn binding(&self) -> &StableCheckedBindingCoordinate {
        &self.binding
    }
    pub const fn effects(&self) -> &EffectRow {
        &self.effects
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedFunctionValueIdentity {
    expression: CheckedSemanticPath,
    ordinal: FunctionValueOrdinal,
    function_type: SemanticTypeDigest,
    effects: EffectRow,
    captures: Box<[CheckedCaptureSignatureRow]>,
}

impl CheckedFunctionValueIdentity {
    pub const fn expression(&self) -> &CheckedSemanticPath {
        &self.expression
    }
    pub const fn ordinal(&self) -> FunctionValueOrdinal {
        self.ordinal
    }
    pub const fn function_type(&self) -> SemanticTypeDigest {
        self.function_type
    }
    pub const fn effects(&self) -> &EffectRow {
        &self.effects
    }
    pub fn captures(&self) -> &[CheckedCaptureSignatureRow] {
        &self.captures
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedContentCallableCoordinate {
    module: CanonicalModulePath,
    path: super::CallablePath,
}

impl CheckedContentCallableCoordinate {
    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }
    pub const fn path(&self) -> &super::CallablePath {
        &self.path
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedDialogueCallableIdentity {
    operation: DialogueCallableId,
    content: Option<CheckedContentCallableCoordinate>,
}

impl CheckedDialogueCallableIdentity {
    pub const fn operation(&self) -> DialogueCallableId {
        self.operation
    }
    pub const fn content(&self) -> Option<&CheckedContentCallableCoordinate> {
        self.content.as_ref()
    }

    pub(crate) fn seal(
        operation: DialogueCallableId,
        callee: PreparedDialogueCalleeIdentity,
    ) -> Result<Self, CallConstraintInvariant> {
        let content = match (operation, callee) {
            (
                DialogueCallableId::ContentCall,
                PreparedDialogueCalleeIdentity::Content { module, path },
            ) => Some(CheckedContentCallableCoordinate { module, path }),
            (
                DialogueCallableId::CharacterFactory,
                PreparedDialogueCalleeIdentity::Character { .. },
            )
            | (
                DialogueCallableId::CharacterReconfigure,
                PreparedDialogueCalleeIdentity::CharacterDialogue { .. },
            )
            | (
                DialogueCallableId::ContentApplication,
                PreparedDialogueCalleeIdentity::Character { .. }
                | PreparedDialogueCalleeIdentity::CharacterDialogue { .. },
            ) => None,
            _ => return Err(CallConstraintInvariant::PreparedBaseMismatch),
        };
        Ok(Self { operation, content })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedDomainMethodIdentity {
    FxSampleOrdinalPhase,
    ObservedObjectRequireRole,
    MapGet {
        key: SemanticTypeDigest,
        value: SemanticTypeDigest,
    },
    ProbeCompare {
        value: SemanticTypeDigest,
        operation: ProbeComparisonOperator,
    },
    DiagnosticsHasError,
    RagContextPackSummary,
    Context,
    WithContext,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedCapacityOperation {
    WithCapacity,
    Trim,
    ToString,
    Pop,
    PopFront,
    Collect,
    Push,
    Reserve,
    ShrinkTo,
    Shrink,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCapacityMethodIdentity {
    operation: CheckedCapacityOperation,
    receiver: SemanticTypeDigest,
    arity: u16,
}

impl CheckedCapacityMethodIdentity {
    pub const fn operation(&self) -> CheckedCapacityOperation {
        self.operation
    }
    pub const fn receiver(&self) -> SemanticTypeDigest {
        self.receiver
    }
    pub const fn arity(&self) -> u16 {
        self.arity
    }

    pub(crate) fn seal(id: super::CapacityMethodId) -> Result<Self, CallConstraintInvariant> {
        let operation = match id.method().as_str() {
            "with_capacity" => CheckedCapacityOperation::WithCapacity,
            "trim" => CheckedCapacityOperation::Trim,
            "to_string" => CheckedCapacityOperation::ToString,
            "pop" => CheckedCapacityOperation::Pop,
            "pop_front" => CheckedCapacityOperation::PopFront,
            "collect" => CheckedCapacityOperation::Collect,
            "push" => CheckedCapacityOperation::Push,
            "reserve" => CheckedCapacityOperation::Reserve,
            "shrink_to" => CheckedCapacityOperation::ShrinkTo,
            "shrink" => CheckedCapacityOperation::Shrink,
            _ => return Err(CallConstraintInvariant::PreparedBaseMismatch),
        };
        let arity = u16::try_from(id.arity())
            .map_err(|_| CallConstraintInvariant::InvalidPreparedNodeState)?;
        Ok(Self {
            operation,
            receiver: id.receiver().semantic_identity_digest(),
            arity,
        })
    }
}

/// Exhaustive stable identity for every language-owned callable family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedLanguageCallableIdentity {
    Fx(FxCallableSignatureId),
    EnumConstructor {
        owner: SemanticTypeDigest,
        case: u32,
    },
    Result(ResultConstructorKind),
    Option(OptionConstructorKind),
    Builtin(BuiltinCallableId),
    Agent(AgentIntrinsicSignatureId),
    Presentation(PresentationCallableId),
    Dialogue(CheckedDialogueCallableIdentity),
    Collection(CollectionMethodId),
    PresentationHandle(PresentationHandleMethodId),
    Integer(IntegerMethodId),
    Domain(CheckedDomainMethodIdentity),
    Capacity(CheckedCapacityMethodIdentity),
    Stage(StageMethodId),
    LineContext(LineContextMethodId),
    LineSchedule(LineScheduleCallableId),
    Drop(DropCallableId),
    Promotion(PromotionCallableId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedCallableStableIdentity {
    Catalog(CheckedCallableDigest),
    Language(CheckedLanguageCallableIdentity),
    Lexical(CheckedLexicalCallableIdentity),
    FunctionValue(CheckedFunctionValueIdentity),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResolvedCallableCheckedDefinition {
    Catalog {
        id: CheckedCallableId,
        record: Arc<super::CallableRecord>,
    },
    Intrinsic {
        schema: Arc<CallableSignatureSchema>,
    },
}

impl ResolvedCallableCheckedDefinition {
    pub(crate) fn schema(&self) -> &CallableSignatureSchema {
        match self {
            Self::Catalog { record, .. } => record.schema(),
            Self::Intrinsic { schema } => schema,
        }
    }
    fn checked_id(&self) -> Option<&CheckedCallableId> {
        match self {
            Self::Catalog { id, .. } => Some(id),
            Self::Intrinsic { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedCallableIssuerEvidence {
    id: CallableCandidateId,
    origin: ResolvedCallableOrigin,
}

impl ResolvedCallableIssuerEvidence {
    pub fn id(&self) -> &CallableCandidateId {
        &self.id
    }
    pub const fn origin(&self) -> &ResolvedCallableOrigin {
        &self.origin
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedCallableAuthority {
    stable: ResolvedCallableStableIdentity,
    checked: ResolvedCallableCheckedDefinition,
    issuer: ResolvedCallableIssuerEvidence,
    family: CallableFamily,
    rank: Option<CallableAuthorityRank>,
    equivalent_sources: Arc<[EquivalentCallableSource]>,
}

impl ResolvedCallableAuthority {
    pub const fn stable(&self) -> &ResolvedCallableStableIdentity {
        &self.stable
    }
    pub const fn issuer(&self) -> &ResolvedCallableIssuerEvidence {
        &self.issuer
    }
    pub fn family(&self) -> CallableFamily {
        self.family
    }
    pub const fn rank(&self) -> Option<CallableAuthorityRank> {
        self.rank
    }
    pub fn equivalent_sources(&self) -> &[EquivalentCallableSource] {
        &self.equivalent_sources
    }
    pub fn schema(&self) -> &CallableSignatureSchema {
        self.checked.schema()
    }
    pub fn checked_id(&self) -> Option<&CheckedCallableId> {
        self.checked.checked_id()
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        self.schema().visit_types(visitor)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedCallableBaseInstantiation {
    None,
    ExpectedEnum {
        expected: TypeKind,
    },
    Result {
        kind: ResultConstructorKind,
    },
    Option,
    Character {
        owner: ResolvedCharacterOwner,
    },
    Receiver {
        receiver: TypeKind,
    },
    TypeReceiver {
        receiver: TypeReceiverInstantiation,
    },
    Extension {
        receiver: TypeKind,
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
}

impl ResolvedCallableBaseInstantiation {
    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::ExpectedEnum { expected }
            | Self::Receiver { receiver: expected }
            | Self::Extension {
                receiver: expected, ..
            } => visitor(expected),
            Self::TypeReceiver { receiver } => visitor(receiver.receiver()),
            Self::None | Self::Result { .. } | Self::Option | Self::Character { .. } => Ok(()),
        }
    }
}

impl From<super::CallableInstantiation> for ResolvedCallableBaseInstantiation {
    fn from(value: super::CallableInstantiation) -> Self {
        match value {
            super::CallableInstantiation::None => Self::None,
            super::CallableInstantiation::ExpectedEnum { expected } => {
                Self::ExpectedEnum { expected }
            }
            super::CallableInstantiation::Result { kind } => Self::Result { kind },
            super::CallableInstantiation::Option => Self::Option,
            super::CallableInstantiation::Character { owner } => Self::Character { owner },
            super::CallableInstantiation::Receiver { receiver } => Self::Receiver { receiver },
            super::CallableInstantiation::TypeReceiver { receiver } => {
                Self::TypeReceiver { receiver }
            }
            super::CallableInstantiation::Extension {
                receiver,
                group,
                parameter,
            } => Self::Extension {
                receiver,
                group,
                parameter,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedCallableBase {
    authority: Arc<ResolvedCallableAuthority>,
    instantiation: ResolvedCallableBaseInstantiation,
    effect_instantiation: super::CheckedCallableEffectInstantiation,
    digest: ResolvedCallableDigest,
}

/// Move-only C-seal permission to prove that one raw execution source maps to
/// an exact checked parameter effect position. It borrows the already sealed
/// base and stores only typed coordinates/projections, never a second schema
/// or projected type authority.
pub(crate) struct CheckedCallableEffectProjectionToken<'a> {
    base: &'a ResolvedCallableBase,
    coordinate: CallableParameterCoordinate,
    expected: super::ParameterExpectedTypeProjection,
    source_projection: CheckedConstraintSourceProjection,
}

pub(crate) struct CheckedCaptureSignatureSeal {
    pub(crate) binding: CheckedBindingCoordinateEvidence,
    pub(crate) mode: CaptureAccess,
    pub(crate) ty: TypeKind,
}

pub(crate) enum ResolvedCallableStableIdentitySeal {
    Catalog,
    Language,
    Lexical {
        binding: CheckedBindingCoordinateEvidence,
        effects: EffectRow,
    },
    FunctionValue {
        expression: CheckedExpressionCoordinateEvidence,
        ordinal: FunctionValueOrdinal,
        function_type: TypeKind,
        effects: EffectRow,
        captures: Box<[CheckedCaptureSignatureSeal]>,
    },
}

pub(crate) struct ResolvedCallableBaseSeal {
    pub(crate) definition: PreparedResolvedCallableDefinitionSealInput,
    pub(crate) stable: ResolvedCallableStableIdentitySeal,
}

impl ResolvedCallableBase {
    pub(crate) fn seal(
        input: ResolvedCallableBaseSeal,
    ) -> Result<Arc<Self>, CallConstraintInvariant> {
        let PreparedResolvedCallableDefinitionSealInput {
            id,
            identity,
            origin,
            checked,
            effect_instantiation,
            instantiation,
            equivalent_sources,
            authority,
            family,
        } = input.definition;
        let instantiation = ResolvedCallableBaseInstantiation::from(instantiation);
        let stable = ResolvedCallableStableIdentity::seal(
            identity,
            &id,
            family,
            &instantiation,
            &checked,
            input.stable,
        )?;
        validate_definition(&id, family, authority, &stable, &checked)?;
        let origin = seal_origin(origin, &stable)?;
        let mut encoder = CheckedCallCanonicalEncoder::new(RESOLVED_CALLABLE_DOMAIN);
        encoder.tag(0);
        encoder.stable_callable_identity(&stable)?;
        encoder.digest(checked.schema().semantic_digest().as_bytes());
        encoder.base_instantiation(&instantiation)?;
        let digest = ResolvedCallableDigest(encoder.finish());
        let checked_effect_issuer = EffectVarIssuer::for_checked_callable(digest.as_bytes());
        let effect_instantiation = effect_instantiation.into_checked(checked_effect_issuer);
        Ok(Arc::new(Self {
            authority: Arc::new(ResolvedCallableAuthority {
                stable,
                checked,
                issuer: ResolvedCallableIssuerEvidence { id, origin },
                family,
                rank: authority,
                equivalent_sources,
            }),
            instantiation,
            effect_instantiation,
            digest,
        }))
    }

    pub const fn authority(&self) -> &Arc<ResolvedCallableAuthority> {
        &self.authority
    }
    pub const fn instantiation(&self) -> &ResolvedCallableBaseInstantiation {
        &self.instantiation
    }
    pub(crate) const fn effect_instantiation(&self) -> &super::CheckedCallableEffectInstantiation {
        &self.effect_instantiation
    }
    pub const fn digest(&self) -> ResolvedCallableDigest {
        self.digest
    }
    pub fn schema(&self) -> &CallableSignatureSchema {
        self.authority.schema()
    }
    pub(crate) const fn base_call_group(&self) -> CallableGroupIndex {
        CallableGroupIndex::ZERO
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        self.authority.visit_types(visitor)?;
        self.instantiation.visit_types(visitor)
    }

    pub(crate) fn next_group_for(&self, current: CallableGroupIndex) -> Option<CallableGroupIndex> {
        let next = CallableGroupIndex::try_from_usize(current.get().checked_add(1)?).ok()?;
        self.schema()
            .group(next)
            .is_some()
            .then_some(next)
            .filter(|next| {
                !matches!(
                    self.instantiation(),
                    ResolvedCallableBaseInstantiation::Extension { group, .. } if group == next
                )
            })
    }

    /// Builds the canonical projected callable type from this checked base.
    /// Nested rows come only from the checked instantiation; the supplied row
    /// controls the invocation boundary and is never used to reconstruct raw
    /// schema children.
    pub(crate) fn callable_type_with_invocation_effects(
        &self,
        invocation: &EffectRow,
    ) -> Result<TypeKind, CallConstraintInvariant> {
        projected_function_type_with_invocation_effects(
            self.schema(),
            self.base_call_group(),
            &self.effect_instantiation,
            invocation,
        )
    }

    pub(crate) fn issue_parameter_effect_projection(
        &self,
        coordinate: CallableParameterCoordinate,
        expected: &super::ParameterExpectedTypeProjection,
        source_projection: &CheckedConstraintSourceProjection,
    ) -> Result<CheckedCallableEffectProjectionToken<'_>, CallConstraintInvariant> {
        self.schema()
            .group(coordinate.group())
            .and_then(|group| group.parameter(coordinate.parameter()))
            .and_then(|parameter| parameter.declared_type())
            .ok_or(CallConstraintInvariant::MalformedSchemaInventory)?;
        self.effect_instantiation
            .project_parameter(self.schema(), coordinate)?
            .ok_or(CallConstraintInvariant::MalformedSchemaInventory)?;
        Ok(CheckedCallableEffectProjectionToken {
            base: self,
            coordinate,
            expected: expected.clone(),
            source_projection: source_projection.clone(),
        })
    }

    fn result_type_for_group(
        &self,
        current: CallableGroupIndex,
        solution: &FrozenCallTypeSolution,
    ) -> Result<TypeKind, CallConstraintInvariant> {
        let next = CallableGroupIndex::try_from_usize(
            current
                .get()
                .checked_add(1)
                .ok_or(CallConstraintInvariant::PreparedGroupMismatch)?,
        )
        .map_err(|_| CallConstraintInvariant::PreparedGroupMismatch)?;
        let declared = if matches!(
            self.instantiation(),
            ResolvedCallableBaseInstantiation::Extension { group, .. } if *group == next
        ) || self.schema().group(next).is_none()
        {
            self.effect_instantiation.project_result(self.schema())?
        } else {
            remaining_function_type(self.schema(), next, &self.effect_instantiation)?
        };
        Ok(solution.apply(&declared))
    }
}

impl CheckedCallableEffectProjectionToken<'_> {
    pub(crate) fn seal_actual(
        self,
        actual: &TypeKind,
        solution: &FrozenCallTypeSolution,
    ) -> Result<TypeKind, CallConstraintInvariant> {
        let source = self
            .base
            .schema()
            .group(self.coordinate.group())
            .and_then(|group| group.parameter(self.coordinate.parameter()))
            .and_then(|parameter| parameter.declared_type())
            .ok_or(CallConstraintInvariant::MalformedSchemaInventory)?;
        let projected = self
            .base
            .effect_instantiation
            .project_parameter(self.base.schema(), self.coordinate)?
            .ok_or(CallConstraintInvariant::MalformedSchemaInventory)?;
        let source = self
            .source_projection
            .compose_expected(&self.expected.apply_to(source));
        let projected = self
            .source_projection
            .compose_expected(&self.expected.apply_to(&projected));
        let projected_actual = self
            .base
            .effect_instantiation
            .seal_source_actual(&source, &projected, actual)?;
        Ok(solution.apply(&projected_actual))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedCallableState {
    Base,
    Continuation(Arc<CheckedCallContinuation>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedCallable {
    base: Arc<ResolvedCallableBase>,
    state: ResolvedCallableState,
    digest: ResolvedCallableDigest,
}

impl ResolvedCallable {
    pub(crate) fn from_base(base: Arc<ResolvedCallableBase>) -> Arc<Self> {
        let digest = base.digest();
        Arc::new(Self {
            base,
            state: ResolvedCallableState::Base,
            digest,
        })
    }

    pub(crate) fn try_from_continuation(
        base: Arc<ResolvedCallableBase>,
        continuation: Arc<CheckedCallContinuation>,
        prepared_group: CallableGroupIndex,
        prepared_function_type: &TypeKind,
    ) -> Result<Arc<Self>, CallConstraintInvariant> {
        if !Arc::ptr_eq(&base, continuation.base())
            || continuation.next_group() != prepared_group
            || continuation.function_type() != prepared_function_type
        {
            return Err(CallConstraintInvariant::PreparedBaseMismatch);
        }
        let mut encoder = CheckedCallCanonicalEncoder::new(RESOLVED_CALLABLE_DOMAIN);
        encoder.tag(1);
        encoder.digest(continuation.digest().as_bytes());
        let digest = ResolvedCallableDigest(encoder.finish());
        Ok(Arc::new(Self {
            base,
            state: ResolvedCallableState::Continuation(continuation),
            digest,
        }))
    }

    pub const fn base(&self) -> &Arc<ResolvedCallableBase> {
        &self.base
    }
    pub const fn state(&self) -> &ResolvedCallableState {
        &self.state
    }
    /// The sole checked-stage classification of runtime value dispatch.
    pub(crate) fn requires_value_callee(&self) -> bool {
        matches!(&self.state, ResolvedCallableState::Continuation(_))
            || self.base.authority().stable().requires_value_callee()
    }
    pub fn id(&self) -> &CallableCandidateId {
        self.base.authority().issuer().id()
    }
    pub fn family(&self) -> CallableFamily {
        self.base.authority().family()
    }
    pub fn origin(&self) -> &ResolvedCallableOrigin {
        self.base.authority().issuer().origin()
    }
    pub fn checked(&self) -> Option<&CheckedCallableId> {
        self.base.authority().checked_id()
    }
    pub fn schema(&self) -> &CallableSignatureSchema {
        self.base.schema()
    }
    pub fn instantiation(&self) -> &ResolvedCallableBaseInstantiation {
        self.base.instantiation()
    }
    pub fn authority_rank(&self) -> Option<CallableAuthorityRank> {
        self.base.authority().rank()
    }
    pub fn equivalent_sources(&self) -> &[EquivalentCallableSource] {
        self.base.authority().equivalent_sources()
    }
    pub const fn digest(&self) -> ResolvedCallableDigest {
        self.digest
    }
    pub(crate) fn call_group(&self) -> CallableGroupIndex {
        match &self.state {
            ResolvedCallableState::Base => self.base.base_call_group(),
            ResolvedCallableState::Continuation(continuation) => continuation.next_group(),
        }
    }
    fn semantic_authority_eq(&self, other: &Self) -> bool {
        self == other
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        self.base.visit_types(visitor)?;
        self.state.visit_types(visitor)
    }
}

impl ResolvedCallableState {
    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Base => Ok(()),
            Self::Continuation(continuation) => continuation.visit_types(visitor),
        }
    }
}

pub(crate) struct FrozenCallTypeSolutionSeed {
    schema: CallableSignatureSchemaDigest,
    completed_group: CallableGroupIndex,
    solution: Arc<TypeConstraintSolution>,
    effect_instantiation: PreparedCallableEffectInstantiationEvidence,
}

impl FrozenCallTypeSolutionSeed {
    pub(super) fn from_prepared(
        schema: CallableSignatureSchemaDigest,
        completed_group: CallableGroupIndex,
        solution: Arc<TypeConstraintSolution>,
        effect_instantiation: PreparedCallableEffectInstantiationEvidence,
    ) -> Self {
        Self {
            schema,
            completed_group,
            solution,
            effect_instantiation,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedDeferredContinuationParameter {
    parameter: crate::types::GenericTypeParameterId,
    first_remaining_group: CallableGroupIndex,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedDeferredContinuationConstParameter {
    parameter: GenericConstParameterId,
    first_remaining_group: CallableGroupIndex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallEffectBinding {
    variable: EffectVar,
    value: EffectRow,
}

impl CheckedCallEffectBinding {
    pub const fn variable(&self) -> EffectVar {
        self.variable
    }
    pub const fn value(&self) -> &EffectRow {
        &self.value
    }
}

impl CheckedDeferredContinuationParameter {
    pub const fn parameter(&self) -> &crate::types::GenericTypeParameterId {
        &self.parameter
    }
    pub const fn first_remaining_group(&self) -> CallableGroupIndex {
        self.first_remaining_group
    }
}

impl CheckedDeferredContinuationConstParameter {
    pub const fn parameter(&self) -> &GenericConstParameterId {
        &self.parameter
    }
    pub const fn first_remaining_group(&self) -> CallableGroupIndex {
        self.first_remaining_group
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct FrozenCallTypeSolution {
    base: ResolvedCallableDigest,
    schema: CallableSignatureSchemaDigest,
    completed_group: CallableGroupIndex,
    solution: Arc<TypeConstraintSolution>,
    effect_bindings: Box<[CheckedCallEffectBinding]>,
    deferred: Box<[CheckedDeferredContinuationParameter]>,
    deferred_consts: Box<[CheckedDeferredContinuationConstParameter]>,
    digest: FrozenCallTypeSolutionDigest,
}

impl std::fmt::Debug for FrozenCallTypeSolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrozenCallTypeSolution")
            .field("base", &self.base)
            .field("schema", &self.schema)
            .field("completed_group", &self.completed_group)
            .field("effect_binding_count", &self.effect_bindings.len())
            .field("deferred_count", &self.deferred.len())
            .field("deferred_const_count", &self.deferred_consts.len())
            .field("digest", &self.digest)
            .finish()
    }
}

impl FrozenCallTypeSolution {
    pub(crate) fn seal(
        seed: FrozenCallTypeSolutionSeed,
        base: &Arc<ResolvedCallableBase>,
    ) -> Result<Arc<Self>, CallConstraintInvariant> {
        if seed.schema != base.schema().semantic_digest()
            || base.schema().group(seed.completed_group).is_none()
        {
            return Err(CallConstraintInvariant::PreparedSchemaMismatch);
        }
        if !seed
            .effect_instantiation
            .matches_checked(base.effect_instantiation())
        {
            return Err(CallConstraintInvariant::PreparedEffectInstantiationMismatch);
        }
        let authorized_ordinals = base
            .effect_instantiation()
            .variables()
            .map(EffectVar::index)
            .collect::<BTreeSet<_>>();
        let solution = Arc::new(
            seed.solution
                .checked_rebind_effect_issuer(
                    seed.effect_instantiation.issuer(),
                    base.effect_instantiation().issuer(),
                    &authorized_ordinals,
                )
                .map_err(|_| CallConstraintInvariant::PreparedEffectInstantiationMismatch)?,
        );
        let implicit_extension_group = match base.instantiation() {
            ResolvedCallableBaseInstantiation::Extension { group, .. } => Some(*group),
            _ => None,
        };
        let mut deferred = base
            .schema()
            .generic_inventory()
            .types()
            .iter()
            .filter_map(|entry| match (entry.role(), entry.first_use()) {
                (
                    super::CallableSchemaGenericRole::Candidate,
                    super::CallableGenericFirstUse::Group(group),
                ) if group > seed.completed_group && Some(group) != implicit_extension_group => {
                    Some(CheckedDeferredContinuationParameter {
                        parameter: entry.parameter().clone(),
                        first_remaining_group: group,
                    })
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        deferred.sort_by(|left, right| {
            generic_parameter_digest(left.parameter())
                .cmp(&generic_parameter_digest(right.parameter()))
                .then_with(|| {
                    left.first_remaining_group()
                        .cmp(&right.first_remaining_group())
                })
        });
        if deferred.windows(2).any(|rows| rows[0] >= rows[1]) {
            return Err(CallConstraintInvariant::PreparedDeferredMismatch);
        }
        let mut deferred_consts = base
            .schema()
            .generic_inventory()
            .consts()
            .iter()
            .filter_map(|entry| match (entry.role(), entry.first_use()) {
                (
                    super::CallableSchemaGenericRole::Candidate,
                    super::CallableGenericFirstUse::Group(group),
                ) if group > seed.completed_group && Some(group) != implicit_extension_group => {
                    Some(CheckedDeferredContinuationConstParameter {
                        parameter: entry.parameter().clone(),
                        first_remaining_group: group,
                    })
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        deferred_consts.sort_by(|left, right| {
            ArrayLength::Generic(left.parameter.clone())
                .canonical_checked_bytes()
                .cmp(&ArrayLength::Generic(right.parameter.clone()).canonical_checked_bytes())
                .then_with(|| left.first_remaining_group.cmp(&right.first_remaining_group))
        });
        if deferred_consts.windows(2).any(|rows| rows[0] >= rows[1]) {
            return Err(CallConstraintInvariant::PreparedDeferredMismatch);
        }
        let mut bindings = solution
            .bindings()
            .map(|(parameter, value)| {
                (
                    generic_parameter_digest(parameter),
                    value.semantic_identity_digest(),
                )
            })
            .collect::<Vec<_>>();
        bindings.sort_by_key(|(parameter, _)| *parameter);
        if bindings.windows(2).any(|rows| rows[0].0 >= rows[1].0) {
            return Err(CallConstraintInvariant::PreparedDeferredMismatch);
        }
        let mut const_bindings = solution
            .const_bindings()
            .map(|(parameter, value)| {
                let parameter = ArrayLength::Generic(parameter.clone())
                    .canonical_checked_bytes()
                    .ok_or(CallConstraintInvariant::PreparedDeferredMismatch)?;
                let value = value
                    .canonical_checked_bytes()
                    .ok_or(CallConstraintInvariant::PreparedDeferredMismatch)?;
                Ok((parameter, value))
            })
            .collect::<Result<Vec<_>, _>>()?;
        const_bindings.sort_by(|left, right| left.0.cmp(&right.0));
        if const_bindings.windows(2).any(|rows| rows[0].0 >= rows[1].0) {
            return Err(CallConstraintInvariant::PreparedDeferredMismatch);
        }
        let mut effect_bindings = solution
            .effect_bindings()
            .map(|(variable, value)| CheckedCallEffectBinding {
                variable: *variable,
                value: value.clone(),
            })
            .collect::<Vec<_>>();
        effect_bindings.sort_by(|left, right| {
            left.variable
                .issuer()
                .as_bytes()
                .cmp(right.variable.issuer().as_bytes())
                .then_with(|| left.variable.index().cmp(&right.variable.index()))
        });
        if effect_bindings.windows(2).any(|rows| {
            rows[0].variable.issuer() == rows[1].variable.issuer()
                && rows[0].variable.index() == rows[1].variable.index()
        }) {
            return Err(CallConstraintInvariant::PreparedDeferredMismatch);
        }
        let mut encoder = CheckedCallCanonicalEncoder::new(FROZEN_SOLUTION_DOMAIN);
        encoder.digest(base.digest().as_bytes());
        encoder.digest(seed.schema.as_bytes());
        encoder.index(seed.completed_group.get())?;
        encoder.count(bindings.len())?;
        for (parameter, value) in &bindings {
            encoder.tag(0);
            encoder.digest(parameter.as_bytes());
            encoder.digest(value.as_bytes());
        }
        encoder.count(const_bindings.len())?;
        for (parameter, value) in &const_bindings {
            encoder.tag(1);
            encoder.bytes(parameter)?;
            encoder.bytes(value)?;
        }
        encoder.count(effect_bindings.len())?;
        for binding in &effect_bindings {
            encoder.digest(binding.variable().issuer().as_bytes());
            encoder.u32(binding.variable().index());
            encoder.effect_row(binding.value())?;
        }
        encoder.count(deferred.len())?;
        for row in &deferred {
            encoder.tag(2);
            encoder.digest(generic_parameter_digest(row.parameter()).as_bytes());
            encoder.index(row.first_remaining_group().get())?;
        }
        encoder.count(deferred_consts.len())?;
        for row in &deferred_consts {
            encoder.tag(3);
            encoder.bytes(
                &ArrayLength::Generic(row.parameter().clone())
                    .canonical_checked_bytes()
                    .ok_or(CallConstraintInvariant::PreparedDeferredMismatch)?,
            )?;
            encoder.index(row.first_remaining_group().get())?;
        }
        let digest = FrozenCallTypeSolutionDigest(encoder.finish());
        Ok(Arc::new(Self {
            base: base.digest(),
            schema: seed.schema,
            completed_group: seed.completed_group,
            solution,
            effect_bindings: effect_bindings.into_boxed_slice(),
            deferred: deferred.into_boxed_slice(),
            deferred_consts: deferred_consts.into_boxed_slice(),
            digest,
        }))
    }

    pub const fn base(&self) -> ResolvedCallableDigest {
        self.base
    }
    pub const fn schema(&self) -> CallableSignatureSchemaDigest {
        self.schema
    }
    pub const fn completed_group(&self) -> CallableGroupIndex {
        self.completed_group
    }
    pub fn effect_bindings(&self) -> &[CheckedCallEffectBinding] {
        &self.effect_bindings
    }
    pub fn deferred(&self) -> &[CheckedDeferredContinuationParameter] {
        &self.deferred
    }
    pub fn deferred_consts(&self) -> &[CheckedDeferredContinuationConstParameter] {
        &self.deferred_consts
    }
    pub const fn digest(&self) -> FrozenCallTypeSolutionDigest {
        self.digest
    }
    pub(crate) fn apply(&self, ty: &TypeKind) -> TypeKind {
        self.solution.apply(ty)
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        for (_, value) in self.solution.bindings() {
            visitor(value)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct PreparedCandidateIndex(u32);

impl PreparedCandidateIndex {
    pub(crate) fn try_from_usize(value: usize) -> Result<Self, CallConstraintInvariant> {
        u32::try_from(value)
            .map(Self)
            .map_err(|_| CallConstraintInvariant::InvalidPreparedNodeState)
    }
    const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedCandidateIndex(u32);

impl CheckedCandidateIndex {
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCandidateInventory {
    candidates: Arc<[Arc<ResolvedCallable>]>,
    selected: CheckedCandidateIndex,
    digest: CheckedCallCandidateInventoryDigest,
}

impl CheckedCandidateInventory {
    pub(crate) fn seal(
        mut candidates: Vec<Arc<ResolvedCallable>>,
        selected: PreparedCandidateIndex,
        limits: &CallableLimits,
    ) -> Result<Self, CallConstraintInvariant> {
        if candidates.is_empty()
            || candidates.len() > limits.max_candidates_per_call()
            || selected.index() >= candidates.len()
        {
            return Err(CallConstraintInvariant::InvalidPreparedNodeState);
        }
        let selected_candidate = Arc::clone(&candidates[selected.index()]);
        candidates.sort_by_key(|candidate| candidate.digest());
        let mut canonical: Vec<Arc<ResolvedCallable>> = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            if let Some(previous) = canonical.last()
                && previous.digest() == candidate.digest()
            {
                if !previous.semantic_authority_eq(&candidate) {
                    return Err(CallConstraintInvariant::PreparedBaseMismatch);
                }
                continue;
            }
            canonical.push(candidate);
        }
        let selected = canonical
            .iter()
            .position(|candidate| candidate.semantic_authority_eq(&selected_candidate))
            .ok_or(CallConstraintInvariant::PreparedBaseMismatch)
            .and_then(|index| {
                u32::try_from(index)
                    .map(CheckedCandidateIndex)
                    .map_err(|_| CallConstraintInvariant::InvalidPreparedNodeState)
            })?;
        let mut encoder = CheckedCallCanonicalEncoder::new(CANDIDATE_INVENTORY_DOMAIN);
        encoder.count(canonical.len())?;
        for candidate in &canonical {
            encoder.digest(candidate.digest().as_bytes());
        }
        encoder.u32(selected.get());
        let digest = CheckedCallCandidateInventoryDigest(encoder.finish());
        Ok(Self {
            candidates: canonical.into(),
            selected,
            digest,
        })
    }

    pub fn candidates(&self) -> &[Arc<ResolvedCallable>] {
        &self.candidates
    }
    pub const fn selected_index(&self) -> CheckedCandidateIndex {
        self.selected
    }
    pub fn selected(&self) -> &Arc<ResolvedCallable> {
        &self.candidates[self.selected.get() as usize]
    }
    pub const fn digest(&self) -> CheckedCallCandidateInventoryDigest {
        self.digest
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        for candidate in self.candidates() {
            candidate.visit_types(visitor)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedCallCalleeExecution {
    Direct,
    Value { source: CheckedCallExecutionSource },
}

/// C1-issued stable identity paired with the generation-local application
/// site.  The raw site remains available for HIR consumers, but construction
/// cannot combine it with an unrelated stable coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallApplicationSite {
    raw: super::CheckedCallSite,
    coordinate: StableCheckedValueCoordinate,
}

impl CheckedCallApplicationSite {
    pub(crate) fn seal(
        raw: super::CheckedCallSite,
        evidence: CheckedExpressionCoordinateEvidence,
    ) -> Result<Self, CallConstraintInvariant> {
        if raw.expression() != evidence.owner() {
            return Err(CallConstraintInvariant::PreparedCallSiteMismatch);
        }
        Ok(Self {
            raw,
            coordinate: StableCheckedValueCoordinate::Expression(evidence.into_coordinate()),
        })
    }

    pub const fn raw(&self) -> super::CheckedCallSite {
        self.raw
    }

    pub const fn coordinate(&self) -> &StableCheckedValueCoordinate {
        &self.coordinate
    }
}

/// C1-issued proof paired with the generation-local operand source needed by
/// lowering. Construction is crate-private and occurs only after the C sealer
/// resolves `raw.owner()` through the semantic-coordinate index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallExecutionSource {
    raw: CheckedCallArgumentSlotSource,
    coordinate: StableCheckedValueCoordinate,
}

/// C1-stable source of a schema operand that participates in semantic
/// admission but is not a runtime call argument. Site-relative structural
/// sources retain the checked application coordinate; the target retains its
/// separately checked expression source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedCallSemanticOperandSource {
    DialogueTarget(CheckedCallExecutionSource),
    DialogueContent {
        application: StableCheckedValueCoordinate,
    },
    DialogueLinePlan {
        application: StableCheckedValueCoordinate,
    },
    DialogueApplicationId {
        argument: HirCallArgumentOrdinal,
        source: CheckedCallExecutionSource,
        id: DialogueLineId,
    },
    DialogueApplicationTextKey {
        argument: HirCallArgumentOrdinal,
        source: CheckedCallExecutionSource,
        key: DialogueTextKey,
    },
}

impl CheckedCallSemanticOperandSource {
    pub const fn coordinate(&self) -> &StableCheckedValueCoordinate {
        match self {
            Self::DialogueTarget(source) => source.coordinate(),
            Self::DialogueContent { application } | Self::DialogueLinePlan { application } => {
                application
            }
            Self::DialogueApplicationId { source, .. }
            | Self::DialogueApplicationTextKey { source, .. } => source.coordinate(),
        }
    }
}

impl CheckedCallExecutionSource {
    pub(crate) fn seal(
        raw: CheckedCallArgumentSlotSource,
        evidence: CheckedExpressionCoordinateEvidence,
    ) -> Result<Self, CallConstraintInvariant> {
        if raw.owner() != evidence.owner() {
            return Err(CallConstraintInvariant::PreparedCallSiteMismatch);
        }
        let coordinate = StableCheckedValueCoordinate::Expression(evidence.into_coordinate());
        Ok(Self { raw, coordinate })
    }
    pub const fn raw(&self) -> CheckedCallArgumentSlotSource {
        self.raw
    }
    pub const fn owner(&self) -> ExprId {
        self.raw.owner()
    }
    pub const fn coordinate(&self) -> &StableCheckedValueCoordinate {
        &self.coordinate
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedCallReceiverProjection {
    None,
    SemanticOnly {
        mode: CallableReceiverMode,
        ty: TypeKind,
    },
    Operand {
        mode: CallableReceiverMode,
        ty: TypeKind,
        source: CheckedCallExecutionSource,
        abi_position: u32,
    },
}

impl CheckedCallReceiverProjection {
    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        let (mode, ty) = match self {
            Self::None => return Ok(()),
            Self::SemanticOnly { mode, ty } | Self::Operand { mode, ty, .. } => (mode, ty),
        };
        match mode {
            CallableReceiverMode::None => {}
            CallableReceiverMode::Value { receiver }
            | CallableReceiverMode::Type { receiver }
            | CallableReceiverMode::Extension { receiver, .. } => visitor(receiver)?,
        }
        visitor(ty)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedCallArgumentPassing {
    Positional,
    Named,
    Spread,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedCallOperandDestination {
    Parameter(CallableParameterCoordinate),
    Open(OpenArgumentId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableParameterAlternativeIndex(u32);

impl CallableParameterAlternativeIndex {
    pub(crate) const fn from_checked_ordinal(ordinal: u32) -> Self {
        Self(ordinal)
    }
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedCallSemanticSelection {
    Unchecked,
    Checked {
        alternative: CallableParameterAlternativeIndex,
        evidence: CheckedSemanticValueEvidence,
    },
}

impl CheckedCallSemanticSelection {
    pub const fn alternative(&self) -> Option<CallableParameterAlternativeIndex> {
        match self {
            Self::Unchecked => None,
            Self::Checked { alternative, .. } => Some(*alternative),
        }
    }

    pub const fn evidence(&self) -> Option<&CheckedSemanticValueEvidence> {
        match self {
            Self::Unchecked => None,
            Self::Checked { evidence, .. } => Some(evidence),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallExecutionSlot {
    slot: CallableArgumentSlotIndex,
    source: CheckedCallExecutionSource,
    abi_position: u32,
    destination: CheckedCallOperandDestination,
    source_projection: CheckedConstraintSourceProjection,
    selection: CheckedCallSemanticSelection,
    inferred: TypeKind,
    expected: Option<TypeKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallSemanticOperand {
    source: CheckedCallSemanticOperandSource,
    destination: CallableParameterCoordinate,
    source_projection: CheckedConstraintSourceProjection,
    selection: CheckedCallSemanticSelection,
    inferred: TypeKind,
    expected: Option<TypeKind>,
}

impl CheckedCallSemanticOperand {
    pub const fn source(&self) -> &CheckedCallSemanticOperandSource {
        &self.source
    }

    pub const fn destination(&self) -> CallableParameterCoordinate {
        self.destination
    }

    pub const fn source_projection(&self) -> &CheckedConstraintSourceProjection {
        &self.source_projection
    }

    pub const fn selection(&self) -> &CheckedCallSemanticSelection {
        &self.selection
    }

    pub const fn inferred(&self) -> &TypeKind {
        &self.inferred
    }

    pub const fn expected(&self) -> Option<&TypeKind> {
        self.expected.as_ref()
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match &self.source_projection {
            CheckedConstraintSourceProjection::Scalar => {}
            CheckedConstraintSourceProjection::SpreadContainer(constructor) => match constructor {
                CheckedConstraintContainerConstructor::Vec
                | CheckedConstraintContainerConstructor::Seq
                | CheckedConstraintContainerConstructor::Slice
                | CheckedConstraintContainerConstructor::Array { .. } => {}
                CheckedConstraintContainerConstructor::MapValue { key, .. } => visitor(key)?,
            },
        }
        visitor(&self.inferred)?;
        if let Some(expected) = &self.expected {
            visitor(expected)?;
        }
        Ok(())
    }
}

impl CheckedCallExecutionSlot {
    pub const fn slot(&self) -> CallableArgumentSlotIndex {
        self.slot
    }
    pub const fn source(&self) -> &CheckedCallExecutionSource {
        &self.source
    }
    pub const fn abi_position(&self) -> u32 {
        self.abi_position
    }
    pub const fn destination(&self) -> &CheckedCallOperandDestination {
        &self.destination
    }
    pub const fn source_projection(&self) -> &CheckedConstraintSourceProjection {
        &self.source_projection
    }
    pub const fn selection(&self) -> &CheckedCallSemanticSelection {
        &self.selection
    }
    pub const fn inferred(&self) -> &TypeKind {
        &self.inferred
    }
    pub const fn expected(&self) -> Option<&TypeKind> {
        self.expected.as_ref()
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match &self.source_projection {
            CheckedConstraintSourceProjection::Scalar => {}
            CheckedConstraintSourceProjection::SpreadContainer(constructor) => match constructor {
                CheckedConstraintContainerConstructor::Vec
                | CheckedConstraintContainerConstructor::Seq
                | CheckedConstraintContainerConstructor::Slice
                | CheckedConstraintContainerConstructor::Array { .. } => {}
                CheckedConstraintContainerConstructor::MapValue { key, .. } => visitor(key)?,
            },
        }
        visitor(&self.inferred)?;
        if let Some(expected) = &self.expected {
            visitor(expected)?;
        }
        Ok(())
    }

    pub fn semantic_action(
        &self,
        selected: &ResolvedCallable,
    ) -> Option<CallableArgumentSemanticAction> {
        match &self.destination {
            CheckedCallOperandDestination::Open(_) => Some(CallableArgumentSemanticAction::Supply),
            CheckedCallOperandDestination::Parameter(coordinate) => {
                let parameter = selected
                    .schema()
                    .group(coordinate.group())?
                    .parameter(coordinate.parameter())?;
                match parameter.admission() {
                    CallableParameterAdmission::UncheckedSupply => {
                        Some(CallableArgumentSemanticAction::Supply)
                    }
                    CallableParameterAdmission::Checked { rule, .. } => {
                        let alternative = self.selection.alternative()?;
                        rule.alternatives()
                            .nth(alternative.get() as usize)
                            .map(CallableParameterValueAlternative::action)
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallExecutionArgument {
    argument: HirCallArgumentOrdinal,
    passing: CheckedCallArgumentPassing,
    slots: Box<[CheckedCallExecutionSlot]>,
}

impl CheckedCallExecutionArgument {
    pub const fn argument(&self) -> HirCallArgumentOrdinal {
        self.argument
    }
    pub const fn passing(&self) -> CheckedCallArgumentPassing {
        self.passing
    }
    pub fn slots(&self) -> &[CheckedCallExecutionSlot] {
        &self.slots
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        for slot in self.slots() {
            slot.visit_types(visitor)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallExecutionProjection {
    receiver: CheckedCallReceiverProjection,
    arguments: Box<[CheckedCallExecutionArgument]>,
    semantic_operands: Box<[CheckedCallSemanticOperand]>,
}

/// Explicit projection order for the one final runtime-operand inventory.
/// Source order is receiver-first followed by authored argument/slot order;
/// ABI order is the contiguous position order validated during C sealing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedCallRuntimeOperandOrder {
    Source,
    Abi,
}

/// Borrowed row from the final checked execution projection. Receiver and
/// argument operands cannot be queried through separate incomplete lists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedCallRuntimeOperand<'a> {
    Receiver {
        mode: &'a CallableReceiverMode,
        ty: &'a TypeKind,
        source: &'a CheckedCallExecutionSource,
        abi_position: u32,
    },
    Argument {
        argument: HirCallArgumentOrdinal,
        passing: CheckedCallArgumentPassing,
        slot: &'a CheckedCallExecutionSlot,
    },
}

impl<'a> CheckedCallRuntimeOperand<'a> {
    pub const fn source(self) -> &'a CheckedCallExecutionSource {
        match self {
            Self::Receiver { source, .. } => source,
            Self::Argument { slot, .. } => slot.source(),
        }
    }

    pub const fn inferred(self) -> &'a TypeKind {
        match self {
            Self::Receiver { ty, .. } => ty,
            Self::Argument { slot, .. } => slot.inferred(),
        }
    }

    pub const fn abi_position(self) -> u32 {
        match self {
            Self::Receiver { abi_position, .. } => abi_position,
            Self::Argument { slot, .. } => slot.abi_position(),
        }
    }
}

pub(crate) struct CheckedCallExecutionSlotSeal {
    pub(crate) slot: CallableArgumentSlotIndex,
    pub(crate) source: CheckedCallExecutionSource,
    pub(crate) abi_position: u32,
    pub(crate) destination: CheckedCallOperandDestination,
    pub(crate) source_projection: CheckedConstraintSourceProjection,
    pub(crate) selection: CheckedCallSemanticSelection,
    pub(crate) inferred: TypeKind,
    pub(crate) expected: Option<TypeKind>,
}

pub(crate) struct CheckedCallSemanticOperandSeal {
    pub(crate) source: CheckedCallSemanticOperandSource,
    pub(crate) destination: CallableParameterCoordinate,
    pub(crate) source_projection: CheckedConstraintSourceProjection,
    pub(crate) selection: CheckedCallSemanticSelection,
    pub(crate) inferred: TypeKind,
    pub(crate) expected: Option<TypeKind>,
}

pub(crate) struct CheckedCallExecutionArgumentSeal {
    pub(crate) argument: HirCallArgumentOrdinal,
    pub(crate) passing: CheckedCallArgumentPassing,
    pub(crate) slots: Box<[CheckedCallExecutionSlotSeal]>,
}

pub(crate) struct CheckedCallExecutionProjectionSeal {
    pub(crate) receiver: CheckedCallReceiverProjection,
    pub(crate) arguments: Box<[CheckedCallExecutionArgumentSeal]>,
    pub(crate) semantic_operands: Box<[CheckedCallSemanticOperandSeal]>,
}

impl CheckedCallExecutionProjection {
    fn seal(
        input: CheckedCallExecutionProjectionSeal,
        selected: &ResolvedCallable,
        solution: &FrozenCallTypeSolution,
        current_group: CallableGroupIndex,
        site: &CheckedCallApplicationSite,
    ) -> Result<Self, CallConstraintInvariant> {
        validate_receiver(&input.receiver, selected)?;
        let mut abi_positions = Vec::new();
        if let CheckedCallReceiverProjection::Operand { abi_position, .. } = &input.receiver {
            abi_positions.push(*abi_position);
        }
        let mut arguments = Vec::with_capacity(input.arguments.len());
        for (argument_index, argument) in input.arguments.into_vec().into_iter().enumerate() {
            if usize::from(argument.argument.get()) != argument_index {
                return Err(CallConstraintInvariant::MalformedMapperSeal);
            }
            let mut slots = Vec::with_capacity(argument.slots.len());
            for (slot_index, slot) in argument.slots.into_vec().into_iter().enumerate() {
                if slot.slot.get() != slot_index {
                    return Err(CallConstraintInvariant::MalformedMapperSeal);
                }
                validate_execution_slot(&slot, selected, solution, current_group)?;
                abi_positions.push(slot.abi_position);
                slots.push(CheckedCallExecutionSlot {
                    slot: slot.slot,
                    source: slot.source,
                    abi_position: slot.abi_position,
                    destination: slot.destination,
                    source_projection: slot.source_projection,
                    selection: slot.selection,
                    inferred: slot.inferred,
                    expected: slot.expected,
                });
            }
            arguments.push(CheckedCallExecutionArgument {
                argument: argument.argument,
                passing: argument.passing,
                slots: slots.into_boxed_slice(),
            });
        }
        abi_positions.sort_unstable();
        if abi_positions
            .iter()
            .enumerate()
            .any(|(expected, actual)| u32::try_from(expected).ok() != Some(*actual))
        {
            return Err(CallConstraintInvariant::MalformedMapperSeal);
        }
        let mut semantic_operands = Vec::with_capacity(input.semantic_operands.len());
        for operand in input.semantic_operands.into_vec() {
            validate_parameter_projection(
                operand.destination,
                &operand.source_projection,
                &operand.selection,
                &operand.inferred,
                operand.expected.as_ref(),
                selected,
                solution,
                current_group,
            )?;
            semantic_operands.push(CheckedCallSemanticOperand {
                source: operand.source,
                destination: operand.destination,
                source_projection: operand.source_projection,
                selection: operand.selection,
                inferred: operand.inferred,
                expected: operand.expected,
            });
        }
        validate_semantic_operand_inventory(site, selected, &arguments, &semantic_operands)?;
        Ok(Self {
            receiver: input.receiver,
            arguments: arguments.into_boxed_slice(),
            semantic_operands: semantic_operands.into_boxed_slice(),
        })
    }

    pub const fn receiver(&self) -> &CheckedCallReceiverProjection {
        &self.receiver
    }
    pub fn arguments(&self) -> &[CheckedCallExecutionArgument] {
        &self.arguments
    }
    pub fn semantic_operands(&self) -> &[CheckedCallSemanticOperand] {
        &self.semantic_operands
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        self.receiver.visit_types(visitor)?;
        for argument in self.arguments() {
            argument.visit_types(visitor)?;
        }
        for operand in self.semantic_operands() {
            operand.visit_types(visitor)?;
        }
        Ok(())
    }

    /// Project every physical runtime operand through one final authority.
    /// The returned rows borrow the sealed receiver/argument carriers; no
    /// caller reconstructs ABI order or silently drops the receiver.
    pub fn ordered_runtime_operands(
        &self,
        order: CheckedCallRuntimeOperandOrder,
    ) -> Box<[CheckedCallRuntimeOperand<'_>]> {
        let receiver = match &self.receiver {
            CheckedCallReceiverProjection::Operand {
                mode,
                ty,
                source,
                abi_position,
            } => Some(CheckedCallRuntimeOperand::Receiver {
                mode,
                ty,
                source,
                abi_position: *abi_position,
            }),
            CheckedCallReceiverProjection::None
            | CheckedCallReceiverProjection::SemanticOnly { .. } => None,
        };
        let mut operands = receiver
            .into_iter()
            .chain(self.arguments.iter().flat_map(|argument| {
                argument
                    .slots()
                    .iter()
                    .map(move |slot| CheckedCallRuntimeOperand::Argument {
                        argument: argument.argument(),
                        passing: argument.passing(),
                        slot,
                    })
            }))
            .collect::<Vec<_>>();
        if order == CheckedCallRuntimeOperandOrder::Abi {
            operands.sort_by_key(|operand| operand.abi_position());
        }
        operands.into_boxed_slice()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallApplicationCore {
    site: CheckedCallApplicationSite,
    current_group: CallableGroupIndex,
    candidates: CheckedCandidateInventory,
    solution: Arc<FrozenCallTypeSolution>,
    callee: CheckedCallCalleeExecution,
    execution: CheckedCallExecutionProjection,
    effects: EffectRow,
    digest: CheckedCallApplicationCoreDigest,
}

pub(crate) struct CheckedCallApplicationCoreSeal {
    pub(crate) site: CheckedCallApplicationSite,
    pub(crate) current_group: CallableGroupIndex,
    pub(crate) candidates: CheckedCandidateInventory,
    pub(crate) solution: Arc<FrozenCallTypeSolution>,
    pub(crate) callee: CheckedCallCalleeExecution,
    pub(crate) execution: CheckedCallExecutionProjectionSeal,
    pub(crate) effects: EffectRow,
}

impl CheckedCallApplicationCore {
    pub(crate) fn seal(
        input: CheckedCallApplicationCoreSeal,
    ) -> Result<Arc<Self>, CallConstraintInvariant> {
        let selected = input.candidates.selected();
        if input.solution.base() != selected.base().digest()
            || input.solution.schema() != selected.schema().semantic_digest()
            || input.solution.completed_group() != input.current_group
            || selected.call_group() != input.current_group
        {
            return Err(CallConstraintInvariant::PreparedBaseMismatch);
        }
        validate_callee(&input.callee, selected)?;
        if let Some(fixed) = selected.schema().effects().fixed_row()
            && fixed != &input.effects
        {
            return Err(CallConstraintInvariant::PreparedSchemaMismatch);
        }
        let execution = CheckedCallExecutionProjection::seal(
            input.execution,
            selected,
            &input.solution,
            input.current_group,
            &input.site,
        )?;
        let mut encoder = CheckedCallCanonicalEncoder::new(APPLICATION_CORE_DOMAIN);
        encoder.digest(input.candidates.digest().as_bytes());
        encoder.index(input.current_group.get())?;
        encoder.digest(input.solution.digest().as_bytes());
        encoder.callee_execution(&input.callee);
        encoder.receiver(execution.receiver())?;
        encoder.count(execution.arguments().len())?;
        for argument in execution.arguments() {
            encoder.execution_argument(argument)?;
        }
        encoder.count(execution.semantic_operands().len())?;
        for operand in execution.semantic_operands() {
            encoder.semantic_operand(operand)?;
        }
        encoder.effect_row(&input.effects)?;
        let digest = CheckedCallApplicationCoreDigest(encoder.finish());
        Ok(Arc::new(Self {
            site: input.site,
            current_group: input.current_group,
            candidates: input.candidates,
            solution: input.solution,
            callee: input.callee,
            execution,
            effects: input.effects,
            digest,
        }))
    }

    pub const fn site(&self) -> super::CheckedCallSite {
        self.site.raw()
    }
    pub const fn stable_site(&self) -> &StableCheckedValueCoordinate {
        self.site.coordinate()
    }
    pub const fn current_group(&self) -> CallableGroupIndex {
        self.current_group
    }
    pub const fn candidates(&self) -> &CheckedCandidateInventory {
        &self.candidates
    }
    pub const fn solution(&self) -> &Arc<FrozenCallTypeSolution> {
        &self.solution
    }
    pub const fn callee(&self) -> &CheckedCallCalleeExecution {
        &self.callee
    }
    pub const fn execution(&self) -> &CheckedCallExecutionProjection {
        &self.execution
    }
    /// Returns the exact normalized type receiver only for the sealed direct
    /// type-receiver execution pair.  Consumers cannot infer associated-type
    /// dispatch from a direct callee alone because value-receiver and free
    /// calls are direct as well.
    pub fn direct_type_receiver(&self) -> Option<&TypeKind> {
        match (
            &self.callee,
            self.execution.receiver(),
            self.candidates.selected().instantiation(),
        ) {
            (
                CheckedCallCalleeExecution::Direct,
                CheckedCallReceiverProjection::SemanticOnly {
                    mode: CallableReceiverMode::Type { receiver: mode },
                    ty,
                },
                ResolvedCallableBaseInstantiation::TypeReceiver { receiver },
            ) if mode == ty && receiver.receiver() == ty => Some(ty),
            _ => None,
        }
    }
    pub const fn effects(&self) -> &EffectRow {
        &self.effects
    }
    pub const fn digest(&self) -> CheckedCallApplicationCoreDigest {
        self.digest
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        self.candidates.visit_types(visitor)?;
        self.solution.visit_types(visitor)?;
        self.execution.visit_types(visitor)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallContinuation {
    base: Arc<ResolvedCallableBase>,
    next_group: CallableGroupIndex,
    inherited_solution: Arc<FrozenCallTypeSolution>,
    prefix_call_site: super::CheckedCallSite,
    prefix_application_site: StableCheckedValueCoordinate,
    prefix_application_core: CheckedCallApplicationCoreDigest,
    function_type: TypeKind,
    digest: CheckedCallContinuationDigest,
}

impl CheckedCallContinuation {
    fn seal_from_core(
        core: &Arc<CheckedCallApplicationCore>,
        next_group: CallableGroupIndex,
        function_type: TypeKind,
    ) -> Result<Arc<Self>, CallConstraintInvariant> {
        let base = Arc::clone(core.candidates().selected().base());
        if base.next_group_for(core.current_group()) != Some(next_group)
            || !matches!(function_type, TypeKind::Function { .. })
        {
            return Err(CallConstraintInvariant::PreparedFunctionTypeMismatch);
        }
        let mut encoder = CheckedCallCanonicalEncoder::new(CONTINUATION_DOMAIN);
        encoder.digest(base.digest().as_bytes());
        encoder.index(next_group.get())?;
        encoder.digest(core.solution().digest().as_bytes());
        encoder.bytes(
            &core
                .stable_site()
                .canonical_bytes()
                .map_err(|_| CallConstraintInvariant::InvalidPreparedNodeState)?,
        )?;
        encoder.digest(core.digest().as_bytes());
        encoder.digest(function_type.semantic_identity_digest().as_bytes());
        let digest = CheckedCallContinuationDigest(encoder.finish());
        Ok(Arc::new(Self {
            base,
            next_group,
            inherited_solution: Arc::clone(core.solution()),
            prefix_call_site: core.site(),
            prefix_application_site: core.stable_site().clone(),
            prefix_application_core: core.digest(),
            function_type,
            digest,
        }))
    }

    pub const fn base(&self) -> &Arc<ResolvedCallableBase> {
        &self.base
    }
    pub const fn next_group(&self) -> CallableGroupIndex {
        self.next_group
    }
    pub const fn inherited_solution(&self) -> &Arc<FrozenCallTypeSolution> {
        &self.inherited_solution
    }
    pub const fn prefix_call_site(&self) -> super::CheckedCallSite {
        self.prefix_call_site
    }
    pub const fn prefix_application_site(&self) -> &StableCheckedValueCoordinate {
        &self.prefix_application_site
    }
    pub const fn prefix_application_core(&self) -> CheckedCallApplicationCoreDigest {
        self.prefix_application_core
    }
    pub const fn function_type(&self) -> &TypeKind {
        &self.function_type
    }
    pub const fn digest(&self) -> CheckedCallContinuationDigest {
        self.digest
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        self.base.visit_types(visitor)?;
        self.inherited_solution.visit_types(visitor)?;
        visitor(&self.function_type)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedCallResult {
    Value(TypeKind),
    Continuation(Arc<CheckedCallContinuation>),
}

impl CheckedCallResult {
    pub fn ty(&self) -> &TypeKind {
        match self {
            Self::Value(value) => value,
            Self::Continuation(continuation) => continuation.function_type(),
        }
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Value(value) => visitor(value),
            Self::Continuation(continuation) => continuation.visit_types(visitor),
        }
    }
}

pub(crate) enum CheckedCallResultSeal {
    Value { prepared: TypeKind },
    Continuation { prepared: TypeKind },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallApplication {
    core: Arc<CheckedCallApplicationCore>,
    result: CheckedCallResult,
    digest: CheckedCallApplicationDigest,
}

impl CheckedCallApplication {
    pub(crate) fn seal(
        core: Arc<CheckedCallApplicationCore>,
        expected: CheckedCallResultSeal,
    ) -> Result<Self, CallConstraintInvariant> {
        let base = core.candidates().selected().base();
        let projected = base.result_type_for_group(core.current_group(), core.solution())?;
        let result = match (base.next_group_for(core.current_group()), expected) {
            (None, CheckedCallResultSeal::Value { prepared }) if prepared == projected => {
                if !core.solution().deferred().is_empty()
                    || !core.solution().deferred_consts().is_empty()
                {
                    return Err(CallConstraintInvariant::PreparedDeferredMismatch);
                }
                CheckedCallResult::Value(projected)
            }
            (Some(next), CheckedCallResultSeal::Continuation { prepared })
                if prepared == projected =>
            {
                CheckedCallResult::Continuation(CheckedCallContinuation::seal_from_core(
                    &core, next, projected,
                )?)
            }
            _ => return Err(CallConstraintInvariant::PreparedFunctionTypeMismatch),
        };
        let mut encoder = CheckedCallCanonicalEncoder::new(APPLICATION_DOMAIN);
        encoder.digest(core.digest().as_bytes());
        match &result {
            CheckedCallResult::Value(value) => {
                encoder.tag(0);
                encoder.digest(value.semantic_identity_digest().as_bytes());
            }
            CheckedCallResult::Continuation(continuation) => {
                encoder.tag(1);
                encoder.digest(continuation.digest().as_bytes());
            }
        }
        let digest = CheckedCallApplicationDigest(encoder.finish());
        Ok(Self {
            core,
            result,
            digest,
        })
    }

    pub const fn core(&self) -> &Arc<CheckedCallApplicationCore> {
        &self.core
    }
    pub const fn result(&self) -> &CheckedCallResult {
        &self.result
    }
    pub const fn digest(&self) -> CheckedCallApplicationDigest {
        self.digest
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        self.core.visit_types(visitor)?;
        self.result.visit_types(visitor)
    }
}

impl ResolvedCallableStableIdentity {
    const fn requires_value_callee(&self) -> bool {
        matches!(self, Self::Lexical(_) | Self::FunctionValue(_))
    }

    fn seal(
        prepared: PreparedResolvedCallableIdentity,
        candidate: &CallableCandidateId,
        family: CallableFamily,
        instantiation: &ResolvedCallableBaseInstantiation,
        checked: &ResolvedCallableCheckedDefinition,
        input: ResolvedCallableStableIdentitySeal,
    ) -> Result<Self, CallConstraintInvariant> {
        match (prepared, input) {
            (
                PreparedResolvedCallableIdentity::Catalog(prepared),
                ResolvedCallableStableIdentitySeal::Catalog,
            ) => {
                let ResolvedCallableCheckedDefinition::Catalog { id, .. } = checked else {
                    return Err(CallConstraintInvariant::PreparedSchemaMismatch);
                };
                if &prepared != id {
                    return Err(CallConstraintInvariant::PreparedBaseMismatch);
                }
                Ok(Self::Catalog(id.semantic_digest()))
            }
            (
                PreparedResolvedCallableIdentity::Language(prepared),
                ResolvedCallableStableIdentitySeal::Language,
            ) => {
                if !matches!(checked, ResolvedCallableCheckedDefinition::Intrinsic { .. }) {
                    return Err(CallConstraintInvariant::PreparedSchemaMismatch);
                }
                Ok(Self::Language(prepared.into_checked(
                    candidate,
                    family,
                    instantiation,
                )?))
            }
            (
                PreparedResolvedCallableIdentity::Lexical { local: prepared },
                ResolvedCallableStableIdentitySeal::Lexical { binding, effects },
            ) => {
                let local = binding.owner();
                if prepared != local
                    || !matches!(candidate, CallableCandidateId::Local(id) if id.local() == local)
                    || family != CallableFamily::Lexical
                    || checked.schema().effects().fixed_row() != Some(&effects)
                {
                    return Err(CallConstraintInvariant::PreparedBaseMismatch);
                }
                Ok(Self::Lexical(CheckedLexicalCallableIdentity {
                    binding: binding.into_coordinate(),
                    effects,
                }))
            }
            (
                PreparedResolvedCallableIdentity::FunctionValue {
                    producer:
                        PreparedFunctionValueOriginIdentity::IndependentExpression {
                            producer: prepared_producer,
                        },
                    ordinal: prepared_ordinal,
                    captures: prepared_captures,
                },
                ResolvedCallableStableIdentitySeal::FunctionValue {
                    expression,
                    ordinal,
                    function_type,
                    effects,
                    captures,
                },
            ) => {
                let producer = expression.owner();
                if prepared_producer != producer
                    || prepared_ordinal != ordinal
                    || family != CallableFamily::FunctionValue
                    || !matches!(candidate, CallableCandidateId::FunctionValue(id)
                        if id.expression() == producer && id.ordinal() == ordinal)
                    || checked.schema().effects().fixed_row() != Some(&effects)
                    || schema_function_type(checked.schema())? != function_type
                {
                    return Err(CallConstraintInvariant::PreparedBaseMismatch);
                }
                Ok(Self::FunctionValue(CheckedFunctionValueIdentity {
                    expression: expression.into_coordinate(),
                    ordinal,
                    function_type: function_type.semantic_identity_digest(),
                    effects,
                    captures: seal_captures(prepared_captures, captures)?,
                }))
            }
            _ => Err(CallConstraintInvariant::PreparedBaseMismatch),
        }
    }
}

fn seal_captures(
    prepared: Box<[PreparedCaptureIdentityRow]>,
    checked: Box<[CheckedCaptureSignatureSeal]>,
) -> Result<Box<[CheckedCaptureSignatureRow]>, CallConstraintInvariant> {
    if prepared.len() != checked.len() {
        return Err(CallConstraintInvariant::PreparedBaseMismatch);
    }
    let mut checked = checked.into_vec();
    for row in &prepared {
        let matches = checked
            .iter()
            .filter(|candidate| {
                candidate.binding.owner() == row.local() && candidate.mode == row.mode()
            })
            .count();
        if matches != 1 {
            return Err(CallConstraintInvariant::PreparedBaseMismatch);
        }
    }
    checked.sort_by(|left, right| left.binding.coordinate().cmp(right.binding.coordinate()));
    if checked
        .windows(2)
        .any(|rows| rows[0].binding.coordinate() == rows[1].binding.coordinate())
    {
        return Err(CallConstraintInvariant::PreparedBaseMismatch);
    }
    Ok(checked
        .into_iter()
        .map(|row| CheckedCaptureSignatureRow {
            binding: row.binding.into_coordinate(),
            mode: row.mode.into(),
            ty: row.ty.semantic_identity_digest(),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice())
}

fn validate_definition(
    candidate: &CallableCandidateId,
    family: CallableFamily,
    authority: Option<CallableAuthorityRank>,
    stable: &ResolvedCallableStableIdentity,
    checked: &ResolvedCallableCheckedDefinition,
) -> Result<(), CallConstraintInvariant> {
    match (stable, checked) {
        (
            ResolvedCallableStableIdentity::Catalog(digest),
            ResolvedCallableCheckedDefinition::Catalog { id, record },
        ) if *digest == id.semantic_digest()
            && record.id() == candidate
            && record.family() == family
            && Some(record.authority()) == authority =>
        {
            Ok(())
        }
        (
            ResolvedCallableStableIdentity::Language(_)
            | ResolvedCallableStableIdentity::Lexical(_)
            | ResolvedCallableStableIdentity::FunctionValue(_),
            ResolvedCallableCheckedDefinition::Intrinsic { schema },
        ) if authority.is_none() && schema.effects().fixed_row().is_some() => Ok(()),
        _ => Err(CallConstraintInvariant::PreparedSchemaMismatch),
    }
}

fn seal_origin(
    origin: SignatureOrigin,
    stable: &ResolvedCallableStableIdentity,
) -> Result<ResolvedCallableOrigin, CallConstraintInvariant> {
    match (origin, stable) {
        (
            SignatureOrigin::Project {
                declaration,
                binding,
            },
            ResolvedCallableStableIdentity::Catalog(_),
        ) => Ok(ResolvedCallableOrigin::Project {
            declaration,
            binding,
        }),
        (SignatureOrigin::Standard { owner, id }, ResolvedCallableStableIdentity::Catalog(_)) => {
            Ok(ResolvedCallableOrigin::Standard { owner, id })
        }
        (SignatureOrigin::Adapter { package, id }, ResolvedCallableStableIdentity::Catalog(_)) => {
            Ok(ResolvedCallableOrigin::Adapter { package, id })
        }
        (
            SignatureOrigin::Language { family },
            ResolvedCallableStableIdentity::Language(identity),
        ) if language_family(identity) == family => Ok(ResolvedCallableOrigin::Language { family }),
        (
            SignatureOrigin::LanguageDialogue { operation, callee },
            ResolvedCallableStableIdentity::Language(CheckedLanguageCallableIdentity::Dialogue(
                identity,
            )),
        ) if identity.operation() == operation
            && dialogue_origin_matches_checked(operation, &callee, identity) =>
        {
            Ok(ResolvedCallableOrigin::LanguageDialogue {
                operation,
                callee: (*callee).clone(),
            })
        }
        (SignatureOrigin::Lexical { .. }, ResolvedCallableStableIdentity::Lexical(identity)) => {
            Ok(ResolvedCallableOrigin::Lexical {
                binding: identity.binding().clone(),
            })
        }
        (
            SignatureOrigin::FunctionValue { id },
            ResolvedCallableStableIdentity::FunctionValue(identity),
        ) if id.ordinal() == identity.ordinal() => Ok(ResolvedCallableOrigin::FunctionValue {
            expression: identity.expression().clone(),
            ordinal: identity.ordinal(),
        }),
        _ => Err(CallConstraintInvariant::PreparedBaseMismatch),
    }
}

fn dialogue_origin_matches_checked(
    operation: DialogueCallableId,
    callee: &ResolvedDialogueCalleeIdentity,
    checked: &CheckedDialogueCallableIdentity,
) -> bool {
    match (operation, callee, checked.content()) {
        (
            DialogueCallableId::ContentCall,
            ResolvedDialogueCalleeIdentity::Content { module, path },
            Some(content),
        ) => content.module() == module && content.path() == path,
        (
            DialogueCallableId::CharacterFactory,
            ResolvedDialogueCalleeIdentity::Character { .. },
            None,
        )
        | (
            DialogueCallableId::CharacterReconfigure,
            ResolvedDialogueCalleeIdentity::CharacterDialogue { .. },
            None,
        )
        | (
            DialogueCallableId::ContentApplication,
            ResolvedDialogueCalleeIdentity::Character { .. }
            | ResolvedDialogueCalleeIdentity::CharacterDialogue { .. },
            None,
        ) => true,
        _ => false,
    }
}

fn language_family(identity: &CheckedLanguageCallableIdentity) -> LanguageCallableFamily {
    match identity {
        CheckedLanguageCallableIdentity::Fx(_) => LanguageCallableFamily::Fx,
        CheckedLanguageCallableIdentity::EnumConstructor { .. } => {
            LanguageCallableFamily::EnumConstructor
        }
        CheckedLanguageCallableIdentity::Result(_) => LanguageCallableFamily::ResultConstructor,
        CheckedLanguageCallableIdentity::Option(_) => LanguageCallableFamily::OptionConstructor,
        CheckedLanguageCallableIdentity::Builtin(_) => LanguageCallableFamily::Builtin,
        CheckedLanguageCallableIdentity::Agent(_) => LanguageCallableFamily::Agent,
        CheckedLanguageCallableIdentity::Presentation(_) => LanguageCallableFamily::Presentation,
        CheckedLanguageCallableIdentity::Dialogue(_) => LanguageCallableFamily::Dialogue,
        CheckedLanguageCallableIdentity::Collection(_) => LanguageCallableFamily::CollectionMethod,
        CheckedLanguageCallableIdentity::PresentationHandle(_) => {
            LanguageCallableFamily::PresentationHandleMethod
        }
        CheckedLanguageCallableIdentity::Integer(_) => LanguageCallableFamily::IntegerMethod,
        CheckedLanguageCallableIdentity::Domain(_) => LanguageCallableFamily::DomainMethod,
        CheckedLanguageCallableIdentity::Capacity(_) => LanguageCallableFamily::CapacityMethod,
        CheckedLanguageCallableIdentity::Stage(_) => LanguageCallableFamily::StageMethod,
        CheckedLanguageCallableIdentity::LineContext(_) => {
            LanguageCallableFamily::LineContextMethod
        }
        CheckedLanguageCallableIdentity::LineSchedule(_) => LanguageCallableFamily::LineSchedule,
        CheckedLanguageCallableIdentity::Drop(_) => LanguageCallableFamily::Drop,
        CheckedLanguageCallableIdentity::Promotion(
            PromotionCallableId::Promote | PromotionCallableId::PromoteUnchecked,
        ) => LanguageCallableFamily::Promote,
        CheckedLanguageCallableIdentity::Promotion(PromotionCallableId::Assume) => {
            LanguageCallableFamily::Assume
        }
    }
}

fn validate_callee(
    callee: &CheckedCallCalleeExecution,
    selected: &ResolvedCallable,
) -> Result<(), CallConstraintInvariant> {
    let value_dispatch = matches!(selected.state(), ResolvedCallableState::Continuation(_))
        || matches!(
            selected.base().authority().stable(),
            ResolvedCallableStableIdentity::Lexical(_)
                | ResolvedCallableStableIdentity::FunctionValue(_)
        );
    match (value_dispatch, callee) {
        (false, CheckedCallCalleeExecution::Direct)
        | (true, CheckedCallCalleeExecution::Value { .. }) => Ok(()),
        _ => Err(CallConstraintInvariant::PreparedBaseMismatch),
    }
}

fn validate_receiver(
    receiver: &CheckedCallReceiverProjection,
    selected: &ResolvedCallable,
) -> Result<(), CallConstraintInvariant> {
    if matches!(selected.state(), ResolvedCallableState::Continuation(_)) {
        return matches!(receiver, CheckedCallReceiverProjection::None)
            .then_some(())
            .ok_or(CallConstraintInvariant::PreparedBaseMismatch);
    }
    let instantiation = selected.instantiation();
    let valid = match (receiver, instantiation) {
        (
            CheckedCallReceiverProjection::None,
            ResolvedCallableBaseInstantiation::None
            | ResolvedCallableBaseInstantiation::ExpectedEnum { .. }
            | ResolvedCallableBaseInstantiation::Result { .. }
            | ResolvedCallableBaseInstantiation::Option
            | ResolvedCallableBaseInstantiation::Character { .. },
        ) => true,
        (
            CheckedCallReceiverProjection::SemanticOnly {
                mode: CallableReceiverMode::Type { receiver: mode },
                ty,
            },
            ResolvedCallableBaseInstantiation::TypeReceiver { receiver },
        ) => mode == ty && receiver.receiver() == ty,
        (
            CheckedCallReceiverProjection::Operand {
                mode: CallableReceiverMode::Value { receiver: mode },
                ty,
                ..
            },
            ResolvedCallableBaseInstantiation::Receiver { receiver },
        ) => mode == ty && receiver == ty,
        (
            CheckedCallReceiverProjection::Operand {
                mode:
                    CallableReceiverMode::Extension {
                        receiver: mode,
                        group: mode_group,
                        parameter: mode_parameter,
                    },
                ty,
                ..
            },
            ResolvedCallableBaseInstantiation::Extension {
                receiver,
                group,
                parameter,
            },
        ) => mode == ty && receiver == ty && mode_group == group && mode_parameter == parameter,
        _ => false,
    };
    valid
        .then_some(())
        .ok_or(CallConstraintInvariant::PreparedBaseMismatch)
}

fn validate_execution_slot(
    slot: &CheckedCallExecutionSlotSeal,
    selected: &ResolvedCallable,
    solution: &FrozenCallTypeSolution,
    current_group: CallableGroupIndex,
) -> Result<(), CallConstraintInvariant> {
    match &slot.destination {
        CheckedCallOperandDestination::Open(open) => {
            if open.schema() != selected.schema().semantic_digest()
                || selected.schema().argument_policy().unknown_named()
                    != super::UnknownNamedArgumentPolicy::OpenSupply
                || !selected.schema().allows_open_name(open.binding())
                || slot.expected.is_some()
                || !matches!(slot.selection, CheckedCallSemanticSelection::Unchecked)
            {
                return Err(CallConstraintInvariant::MalformedMapperSeal);
            }
        }
        CheckedCallOperandDestination::Parameter(coordinate) => {
            validate_parameter_projection(
                *coordinate,
                &slot.source_projection,
                &slot.selection,
                &slot.inferred,
                slot.expected.as_ref(),
                selected,
                solution,
                current_group,
            )?;
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "one closed parameter projection validates the lower-produced type, evidence, and schema coordinate together"
)]
fn validate_parameter_projection(
    coordinate: CallableParameterCoordinate,
    source_projection: &CheckedConstraintSourceProjection,
    selection: &CheckedCallSemanticSelection,
    inferred: &TypeKind,
    actual_expected: Option<&TypeKind>,
    selected: &ResolvedCallable,
    solution: &FrozenCallTypeSolution,
    current_group: CallableGroupIndex,
) -> Result<(), CallConstraintInvariant> {
    if coordinate.group() != current_group {
        return Err(CallConstraintInvariant::PreparedGroupMismatch);
    }
    let parameter = selected
        .schema()
        .group(coordinate.group())
        .and_then(|group| group.parameter(coordinate.parameter()))
        .ok_or(CallConstraintInvariant::MalformedSchemaInventory)?;
    match parameter.admission() {
        CallableParameterAdmission::UncheckedSupply => {
            if actual_expected.is_some()
                || !matches!(selection, CheckedCallSemanticSelection::Unchecked)
            {
                return Err(CallConstraintInvariant::MalformedMapperSeal);
            }
        }
        CallableParameterAdmission::Checked { rule, .. } => {
            let declared = selected
                .base()
                .effect_instantiation()
                .project_parameter(selected.schema(), coordinate)?
                .ok_or(CallConstraintInvariant::MalformedSchemaInventory)?;
            let CheckedCallSemanticSelection::Checked {
                alternative,
                evidence,
            } = selection
            else {
                return Err(CallConstraintInvariant::MalformedMapperSeal);
            };
            let alternative_index = alternative.get() as usize;
            let alternative = rule
                .alternative(alternative_index)
                .ok_or(CallConstraintInvariant::MalformedMapperSeal)?;
            let expected =
                expected_for_alternative(source_projection, &declared, alternative, solution);
            let checked_declared = solution.apply(&declared);
            if actual_expected != Some(&expected)
                || !rule.selects(alternative_index, &checked_declared, evidence)
                || !expected.accepts(inferred)
            {
                return Err(CallConstraintInvariant::MalformedMapperSeal);
            }
        }
    }
    Ok(())
}

fn validate_semantic_operand_inventory(
    site: &CheckedCallApplicationSite,
    selected: &ResolvedCallable,
    arguments: &[CheckedCallExecutionArgument],
    operands: &[CheckedCallSemanticOperand],
) -> Result<(), CallConstraintInvariant> {
    let is_dialogue_application =
        selected.id() == &CallableCandidateId::Dialogue(DialogueCallableId::ContentApplication);
    if is_dialogue_application {
        if !arguments.is_empty()
            || !matches!(site.raw(), super::CheckedCallSite::DialogueApplication(_))
            || !matches!(operands, [_, _] | [_, _, _])
        {
            return Err(CallConstraintInvariant::MalformedMapperSeal);
        }
        let expected_sources = [0, 1, if operands.len() == 3 { 2 } else { usize::MAX }];
        for (ordinal, operand) in operands.iter().enumerate() {
            if operand.destination().group() != CallableGroupIndex::ZERO
                || operand.destination().parameter().get() != expected_sources[ordinal]
            {
                return Err(CallConstraintInvariant::MalformedMapperSeal);
            }
            let source_matches = match (ordinal, operand.source()) {
                (0, CheckedCallSemanticOperandSource::DialogueTarget(_)) => true,
                (1, CheckedCallSemanticOperandSource::DialogueContent { application }) => {
                    application == site.coordinate()
                }
                (2, CheckedCallSemanticOperandSource::DialogueLinePlan { application }) => {
                    application == site.coordinate()
                }
                _ => false,
            };
            if !source_matches {
                return Err(CallConstraintInvariant::MalformedMapperSeal);
            }
        }
        return Ok(());
    }

    if !matches!(site.raw(), super::CheckedCallSite::HirCall(_)) {
        return Err(CallConstraintInvariant::MalformedMapperSeal);
    }
    let mut previous_argument = None;
    let mut destinations = BTreeSet::new();
    for operand in operands {
        let coordinate = operand.destination();
        let parameter = selected
            .schema()
            .group(coordinate.group())
            .and_then(|group| group.parameter(coordinate.parameter()))
            .ok_or(CallConstraintInvariant::MalformedSchemaInventory)?;
        let argument = match (parameter.consumer(), operand.source()) {
            (
                super::CallableParameterConsumer::DialogueApplicationMetadata(
                    super::DialogueApplicationMetadataCoordinate::Id,
                ),
                CheckedCallSemanticOperandSource::DialogueApplicationId { argument, .. },
            ) => *argument,
            (
                super::CallableParameterConsumer::DialogueApplicationMetadata(
                    super::DialogueApplicationMetadataCoordinate::TextKey,
                ),
                CheckedCallSemanticOperandSource::DialogueApplicationTextKey { argument, .. },
            ) => *argument,
            _ => return Err(CallConstraintInvariant::MalformedMapperSeal),
        };
        let argument_index = usize::from(argument.get());
        if !destinations.insert(coordinate)
            || previous_argument.is_some_and(|previous| previous >= argument)
            || arguments
                .get(argument_index)
                .is_none_or(|argument| !argument.slots().is_empty())
        {
            return Err(CallConstraintInvariant::MalformedMapperSeal);
        }
        previous_argument = Some(argument);
    }
    Ok(())
}

fn expected_for_alternative(
    source_projection: &CheckedConstraintSourceProjection,
    declared: &TypeKind,
    alternative: CallableParameterValueAlternative<'_>,
    solution: &FrozenCallTypeSolution,
) -> TypeKind {
    let value_expected = solution.apply(&alternative.expected().apply_to(declared));
    source_projection.compose_expected(&value_expected)
}

fn remaining_function_type(
    schema: &CallableSignatureSchema,
    first_group: CallableGroupIndex,
    effects: &super::CheckedCallableEffectInstantiation,
) -> Result<TypeKind, CallConstraintInvariant> {
    let invocation = effects.project_invocation_effects(schema)?;
    projected_function_type_with_invocation_effects(schema, first_group, effects, &invocation)
}

fn projected_function_type_with_invocation_effects(
    schema: &CallableSignatureSchema,
    first_group: CallableGroupIndex,
    effects: &super::CheckedCallableEffectInstantiation,
    invocation: &EffectRow,
) -> Result<TypeKind, CallConstraintInvariant> {
    if schema.group(first_group).is_none() {
        return Err(CallConstraintInvariant::MalformedSchemaInventory);
    }
    let mut result = effects.project_result(schema)?;
    for group in schema.groups().iter().skip(first_group.get()).rev() {
        let parameters = group
            .parameters()
            .iter()
            .map(|parameter| {
                effects
                    .project_parameter(
                        schema,
                        CallableParameterCoordinate::new(group.index(), parameter.index()),
                    )
                    .ok()
                    .flatten()
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(CallConstraintInvariant::MalformedSchemaInventory)?;
        result = TypeKind::function_with_effects(parameters, result, invocation.clone());
    }
    Ok(result)
}

fn schema_function_type(
    schema: &CallableSignatureSchema,
) -> Result<TypeKind, CallConstraintInvariant> {
    let effects = schema
        .effects()
        .fixed_row()
        .ok_or(CallConstraintInvariant::PreparedSchemaMismatch)?;
    let mut result = schema.result().clone();
    for group in schema.groups().iter().rev() {
        let parameters = group
            .parameters()
            .iter()
            .map(|parameter| parameter.declared_type().cloned())
            .collect::<Option<Vec<_>>>()
            .ok_or(CallConstraintInvariant::MalformedSchemaInventory)?;
        result = TypeKind::function_with_effects(parameters, result, effects.clone());
    }
    Ok(result)
}

fn generic_parameter_digest(
    parameter: &crate::types::GenericTypeParameterId,
) -> SemanticTypeDigest {
    TypeKind::GenericParam(parameter.clone()).semantic_identity_digest()
}

/// The sole version-1 callable encoder. It is private, cannot finish into a
/// caller-selected digest type, and accepts only typed owners.
struct CheckedCallCanonicalEncoder {
    hasher: blake3::Hasher,
}

impl CheckedCallCanonicalEncoder {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain);
        Self { hasher }
    }
    fn finish(self) -> [u8; 32] {
        *self.hasher.finalize().as_bytes()
    }
    fn tag(&mut self, value: u8) {
        self.hasher.update(&[value]);
    }
    fn u32(&mut self, value: u32) {
        self.hasher.update(&value.to_le_bytes());
    }
    fn u64(&mut self, value: u64) {
        self.hasher.update(&value.to_le_bytes());
    }
    fn digest(&mut self, value: &[u8; 32]) {
        self.hasher.update(value);
    }

    fn count(&mut self, value: usize) -> Result<(), CallConstraintInvariant> {
        self.u32(
            u32::try_from(value).map_err(|_| CallConstraintInvariant::InvalidPreparedNodeState)?,
        );
        Ok(())
    }
    fn index(&mut self, value: usize) -> Result<(), CallConstraintInvariant> {
        self.u32(
            u32::try_from(value).map_err(|_| CallConstraintInvariant::InvalidPreparedNodeState)?,
        );
        Ok(())
    }
    fn bytes(&mut self, value: &[u8]) -> Result<(), CallConstraintInvariant> {
        self.u64(
            u64::try_from(value.len())
                .map_err(|_| CallConstraintInvariant::InvalidPreparedNodeState)?,
        );
        self.hasher.update(value);
        Ok(())
    }

    fn stable_callable_identity(
        &mut self,
        identity: &ResolvedCallableStableIdentity,
    ) -> Result<(), CallConstraintInvariant> {
        match identity {
            ResolvedCallableStableIdentity::Catalog(digest) => {
                self.tag(0);
                self.digest(digest.as_bytes());
            }
            ResolvedCallableStableIdentity::Language(identity) => {
                self.tag(1);
                self.language_identity(identity)?;
            }
            ResolvedCallableStableIdentity::Lexical(identity) => {
                self.tag(2);
                self.bytes(
                    &identity
                        .binding()
                        .canonical_bytes()
                        .map_err(|_| CallConstraintInvariant::InvalidPreparedNodeState)?,
                )?;
                self.effect_row(identity.effects())?;
            }
            ResolvedCallableStableIdentity::FunctionValue(identity) => {
                self.tag(3);
                self.bytes(
                    &identity
                        .expression()
                        .canonical_bytes()
                        .map_err(|_| CallConstraintInvariant::InvalidPreparedNodeState)?,
                )?;
                self.index(identity.ordinal().get())?;
                self.digest(identity.function_type().as_bytes());
                self.effect_row(identity.effects())?;
                self.count(identity.captures().len())?;
                for capture in identity.captures() {
                    self.bytes(
                        &capture
                            .binding()
                            .canonical_bytes()
                            .map_err(|_| CallConstraintInvariant::InvalidPreparedNodeState)?,
                    )?;
                    self.tag(match capture.mode() {
                        CheckedCaptureMode::Read => 0,
                        CheckedCaptureMode::Reassign => 1,
                    });
                    self.digest(capture.ty().as_bytes());
                }
            }
        }
        Ok(())
    }

    fn language_identity(
        &mut self,
        identity: &CheckedLanguageCallableIdentity,
    ) -> Result<(), CallConstraintInvariant> {
        match identity {
            CheckedLanguageCallableIdentity::Fx(id) => {
                self.tag(0);
                self.tag(fx_tag(*id));
            }
            CheckedLanguageCallableIdentity::EnumConstructor { owner, case } => {
                self.tag(1);
                self.digest(owner.as_bytes());
                self.u32(*case);
            }
            CheckedLanguageCallableIdentity::Result(kind) => {
                self.tag(2);
                self.tag(result_constructor_tag(*kind));
            }
            CheckedLanguageCallableIdentity::Option(OptionConstructorKind::Some) => {
                self.tag(3);
                self.tag(0);
            }
            CheckedLanguageCallableIdentity::Builtin(id) => {
                self.tag(4);
                self.builtin(id)?;
            }
            CheckedLanguageCallableIdentity::Agent(id) => {
                self.tag(5);
                self.tag(agent_tag(*id));
            }
            CheckedLanguageCallableIdentity::Presentation(id) => {
                self.tag(6);
                self.tag(presentation_tag(*id));
            }
            CheckedLanguageCallableIdentity::Dialogue(identity) => {
                self.tag(7);
                self.tag(dialogue_tag(identity.operation()));
                match identity.content() {
                    None => self.tag(0),
                    Some(content) => {
                        self.tag(1);
                        self.canonical_module_path(content.module())?;
                        self.callable_path(content.path())?;
                    }
                }
            }
            CheckedLanguageCallableIdentity::Collection(id) => {
                self.tag(8);
                self.tag(collection_tag(*id));
            }
            CheckedLanguageCallableIdentity::PresentationHandle(id) => {
                self.tag(9);
                self.tag(presentation_handle_tag(*id));
            }
            CheckedLanguageCallableIdentity::Integer(id) => {
                self.tag(10);
                self.tag(integer_tag(*id));
            }
            CheckedLanguageCallableIdentity::Domain(id) => {
                self.tag(11);
                self.domain(id);
            }
            CheckedLanguageCallableIdentity::Capacity(id) => {
                self.tag(12);
                self.tag(capacity_tag(id.operation()));
                self.digest(id.receiver().as_bytes());
                self.u32(u32::from(id.arity()));
            }
            CheckedLanguageCallableIdentity::Stage(id) => {
                self.tag(13);
                self.tag(stage_tag(*id));
            }
            CheckedLanguageCallableIdentity::LineContext(id) => {
                self.tag(14);
                self.tag(line_context_tag(*id));
            }
            CheckedLanguageCallableIdentity::LineSchedule(id) => {
                self.tag(15);
                self.tag(line_schedule_tag(*id));
            }
            CheckedLanguageCallableIdentity::Drop(id) => {
                self.tag(16);
                self.tag(drop_tag(*id));
            }
            CheckedLanguageCallableIdentity::Promotion(id) => {
                self.tag(17);
                self.tag(promotion_tag(*id));
            }
        }
        Ok(())
    }

    fn builtin(&mut self, id: &BuiltinCallableId) -> Result<(), CallConstraintInvariant> {
        match id {
            BuiltinCallableId::InlineFailureFallback => self.tag(0),
            BuiltinCallableId::Panic => self.tag(1),
            BuiltinCallableId::Fail => self.tag(2),
            BuiltinCallableId::Bail => self.tag(3),
            BuiltinCallableId::Ensure => self.tag(4),
            BuiltinCallableId::Rgb => self.tag(5),
            BuiltinCallableId::Sin => self.tag(6),
            BuiltinCallableId::Cos => self.tag(7),
            BuiltinCallableId::Vector { dimensions } => {
                self.tag(8);
                self.tag(match dimensions {
                    VectorDimensions::Two => 0,
                    VectorDimensions::Three => 1,
                    VectorDimensions::Four => 2,
                });
            }
            BuiltinCallableId::Math(id) => {
                self.tag(9);
                self.tag(match id {
                    MathCallableId::MatMulF32 => 0,
                    MathCallableId::MatrixAddF32 => 1,
                    MathCallableId::MatMulF64 => 2,
                    MathCallableId::MatrixAddF64 => 3,
                    MathCallableId::TensorAddF32 => 4,
                    MathCallableId::TensorAddF64 => 5,
                });
            }
            BuiltinCallableId::StdFloat(id) => {
                self.tag(10);
                self.std_float(*id)?;
            }
            BuiltinCallableId::Capability(CapabilityCallableId::EventEmit) => {
                self.tag(11);
                self.tag(0);
            }
            BuiltinCallableId::Reduction(super::ReductionConstructorKind::Unchanged) => {
                self.tag(12);
                self.tag(0);
            }
        }
        Ok(())
    }

    fn std_float(&mut self, id: StdFloatCallableId) -> Result<(), CallConstraintInvariant> {
        let width = id.width();
        let operation = id.operation();
        if matches!(
            (width, operation),
            (FloatWidth::F32, StdFloatOperation::ToF32)
                | (FloatWidth::F64, StdFloatOperation::ToF64)
        ) {
            return Err(CallConstraintInvariant::PreparedBaseMismatch);
        }
        self.tag(match width {
            FloatWidth::F32 => 0,
            FloatWidth::F64 => 1,
        });
        self.tag(std_float_operation_tag(operation));
        Ok(())
    }

    fn domain(&mut self, id: &CheckedDomainMethodIdentity) {
        match id {
            CheckedDomainMethodIdentity::FxSampleOrdinalPhase => self.tag(0),
            CheckedDomainMethodIdentity::ObservedObjectRequireRole => self.tag(1),
            CheckedDomainMethodIdentity::MapGet { key, value } => {
                self.tag(2);
                self.digest(key.as_bytes());
                self.digest(value.as_bytes());
            }
            CheckedDomainMethodIdentity::ProbeCompare { value, operation } => {
                self.tag(3);
                self.digest(value.as_bytes());
                self.tag(probe_comparison_tag(*operation));
            }
            CheckedDomainMethodIdentity::DiagnosticsHasError => self.tag(4),
            CheckedDomainMethodIdentity::RagContextPackSummary => self.tag(5),
            CheckedDomainMethodIdentity::Context => self.tag(6),
            CheckedDomainMethodIdentity::WithContext => self.tag(7),
        }
    }

    fn canonical_module_path(
        &mut self,
        path: &CanonicalModulePath,
    ) -> Result<(), CallConstraintInvariant> {
        self.count(path.segments().len())?;
        for segment in path.segments() {
            self.bytes(segment.as_str().as_bytes())?;
        }
        Ok(())
    }
    fn callable_path(&mut self, path: &super::CallablePath) -> Result<(), CallConstraintInvariant> {
        self.count(path.segments().len())?;
        for segment in path.segments() {
            self.bytes(segment.as_str().as_bytes())?;
        }
        Ok(())
    }

    fn base_instantiation(
        &mut self,
        instantiation: &ResolvedCallableBaseInstantiation,
    ) -> Result<(), CallConstraintInvariant> {
        match instantiation {
            ResolvedCallableBaseInstantiation::None => self.tag(0),
            ResolvedCallableBaseInstantiation::ExpectedEnum { expected } => {
                self.tag(1);
                self.digest(expected.semantic_identity_digest().as_bytes());
            }
            ResolvedCallableBaseInstantiation::Result { kind } => {
                self.tag(2);
                self.tag(result_constructor_tag(*kind));
            }
            ResolvedCallableBaseInstantiation::Option => self.tag(3),
            ResolvedCallableBaseInstantiation::Character { owner } => {
                self.tag(4);
                self.bytes(owner.character().as_str().as_bytes())?;
                match owner.source() {
                    super::CharacterOwnerSource::EntityReference => self.tag(0),
                }
            }
            ResolvedCallableBaseInstantiation::Receiver { receiver } => {
                self.tag(5);
                self.digest(receiver.semantic_identity_digest().as_bytes());
            }
            ResolvedCallableBaseInstantiation::TypeReceiver { receiver } => {
                self.tag(6);
                self.digest(receiver.receiver().semantic_identity_digest().as_bytes());
            }
            ResolvedCallableBaseInstantiation::Extension {
                receiver,
                group,
                parameter,
            } => {
                self.tag(7);
                self.digest(receiver.semantic_identity_digest().as_bytes());
                self.index(group.get())?;
                self.index(parameter.get())?;
            }
        }
        Ok(())
    }

    fn callee_execution(&mut self, callee: &CheckedCallCalleeExecution) {
        self.tag(match callee {
            CheckedCallCalleeExecution::Direct => 0,
            CheckedCallCalleeExecution::Value { .. } => 1,
        });
    }
    fn receiver(
        &mut self,
        receiver: &CheckedCallReceiverProjection,
    ) -> Result<(), CallConstraintInvariant> {
        match receiver {
            CheckedCallReceiverProjection::None => self.tag(0),
            CheckedCallReceiverProjection::SemanticOnly { mode, ty } => {
                self.tag(1);
                self.receiver_mode(mode);
                self.digest(ty.semantic_identity_digest().as_bytes());
            }
            CheckedCallReceiverProjection::Operand {
                mode,
                ty,
                abi_position,
                ..
            } => {
                self.tag(2);
                self.receiver_mode(mode);
                self.digest(ty.semantic_identity_digest().as_bytes());
                self.u32(*abi_position);
            }
        }
        Ok(())
    }
    fn receiver_mode(&mut self, mode: &CallableReceiverMode) {
        self.tag(match mode {
            CallableReceiverMode::None => 0,
            CallableReceiverMode::Value { .. } => 1,
            CallableReceiverMode::Type { .. } => 2,
            CallableReceiverMode::Extension { .. } => 3,
        });
    }

    fn execution_argument(
        &mut self,
        argument: &CheckedCallExecutionArgument,
    ) -> Result<(), CallConstraintInvariant> {
        self.u32(u32::from(argument.argument().get()));
        self.tag(match argument.passing() {
            CheckedCallArgumentPassing::Positional => 0,
            CheckedCallArgumentPassing::Named => 1,
            CheckedCallArgumentPassing::Spread => 2,
        });
        self.count(argument.slots().len())?;
        for slot in argument.slots() {
            self.execution_slot(slot)?;
        }
        Ok(())
    }

    fn execution_slot(
        &mut self,
        slot: &CheckedCallExecutionSlot,
    ) -> Result<(), CallConstraintInvariant> {
        self.index(slot.slot().get())?;
        match slot.source().raw() {
            CheckedCallArgumentSlotSource::Expression(_) => self.tag(0),
            CheckedCallArgumentSlotSource::CompactNumericElement { ordinal, .. } => {
                self.tag(1);
                self.u32(ordinal);
            }
        }
        self.u32(slot.abi_position());
        self.operand_projection(
            slot.destination(),
            slot.source_projection(),
            slot.selection(),
            slot.inferred(),
            slot.expected(),
        )
    }

    fn semantic_operand(
        &mut self,
        operand: &CheckedCallSemanticOperand,
    ) -> Result<(), CallConstraintInvariant> {
        match operand.source() {
            CheckedCallSemanticOperandSource::DialogueTarget(source) => {
                self.tag(0);
                self.bytes(
                    &source
                        .coordinate()
                        .canonical_bytes()
                        .map_err(|_| CallConstraintInvariant::InvalidPreparedNodeState)?,
                )?;
            }
            CheckedCallSemanticOperandSource::DialogueContent { application } => {
                self.tag(1);
                self.bytes(
                    &application
                        .canonical_bytes()
                        .map_err(|_| CallConstraintInvariant::InvalidPreparedNodeState)?,
                )?;
            }
            CheckedCallSemanticOperandSource::DialogueLinePlan { application } => {
                self.tag(2);
                self.bytes(
                    &application
                        .canonical_bytes()
                        .map_err(|_| CallConstraintInvariant::InvalidPreparedNodeState)?,
                )?;
            }
            CheckedCallSemanticOperandSource::DialogueApplicationId {
                argument,
                source,
                id,
            } => {
                self.tag(3);
                self.u32(u32::from(argument.get()));
                self.bytes(
                    &source
                        .coordinate()
                        .canonical_bytes()
                        .map_err(|_| CallConstraintInvariant::InvalidPreparedNodeState)?,
                )?;
                self.bytes(id.as_str().as_bytes())?;
            }
            CheckedCallSemanticOperandSource::DialogueApplicationTextKey {
                argument,
                source,
                key,
            } => {
                self.tag(4);
                self.u32(u32::from(argument.get()));
                self.bytes(
                    &source
                        .coordinate()
                        .canonical_bytes()
                        .map_err(|_| CallConstraintInvariant::InvalidPreparedNodeState)?,
                )?;
                self.bytes(key.as_str().as_bytes())?;
            }
        }
        self.operand_projection(
            &CheckedCallOperandDestination::Parameter(operand.destination()),
            operand.source_projection(),
            operand.selection(),
            operand.inferred(),
            operand.expected(),
        )
    }

    fn operand_projection(
        &mut self,
        destination: &CheckedCallOperandDestination,
        source_projection: &CheckedConstraintSourceProjection,
        selection: &CheckedCallSemanticSelection,
        inferred: &TypeKind,
        expected: Option<&TypeKind>,
    ) -> Result<(), CallConstraintInvariant> {
        match destination {
            CheckedCallOperandDestination::Parameter(coordinate) => {
                self.tag(0);
                self.index(coordinate.group().get())?;
                self.index(coordinate.parameter().get())?;
            }
            CheckedCallOperandDestination::Open(open) => {
                self.tag(1);
                self.digest(open.schema().as_bytes());
                self.bytes(open.binding().as_str().as_bytes())?;
            }
        }
        match source_projection {
            CheckedConstraintSourceProjection::Scalar => self.tag(0),
            CheckedConstraintSourceProjection::SpreadContainer(constructor) => {
                self.tag(1);
                self.container_constructor(constructor)?;
            }
        }
        match selection {
            CheckedCallSemanticSelection::Unchecked => self.tag(0),
            CheckedCallSemanticSelection::Checked {
                alternative,
                evidence,
            } => {
                self.tag(1);
                self.u32(alternative.get());
                match evidence {
                    CheckedSemanticValueEvidence::VariantCase {
                        owner,
                        ordinal,
                        payload,
                    } => {
                        self.tag(0);
                        self.digest(owner.as_bytes());
                        self.u32(*ordinal);
                        self.tag(match payload {
                            VariantPayloadRequirement::Unit => 0,
                            VariantPayloadRequirement::Present => 1,
                        });
                    }
                    CheckedSemanticValueEvidence::NoVariantCase => self.tag(1),
                }
            }
        }
        self.digest(inferred.semantic_identity_digest().as_bytes());
        match expected {
            None => self.tag(0),
            Some(expected) => {
                self.tag(1);
                self.digest(expected.semantic_identity_digest().as_bytes());
            }
        }
        Ok(())
    }

    fn container_constructor(
        &mut self,
        constructor: &CheckedConstraintContainerConstructor,
    ) -> Result<(), CallConstraintInvariant> {
        match constructor {
            CheckedConstraintContainerConstructor::Vec => self.tag(0),
            CheckedConstraintContainerConstructor::Seq => self.tag(1),
            CheckedConstraintContainerConstructor::Slice => self.tag(2),
            CheckedConstraintContainerConstructor::Array { len } => {
                self.tag(3);
                self.array_length(len)?;
            }
            CheckedConstraintContainerConstructor::MapValue { kind, key } => {
                self.tag(4);
                self.tag(match kind {
                    MapKind::Ordered => 0,
                    MapKind::Sorted => 1,
                    MapKind::BTree => 2,
                });
                self.digest(key.semantic_identity_digest().as_bytes());
            }
        }
        Ok(())
    }

    fn array_length(&mut self, length: &ArrayLength) -> Result<(), CallConstraintInvariant> {
        let canonical = length
            .canonical_checked_bytes()
            .ok_or(CallConstraintInvariant::MalformedMapperSeal)?;
        self.hasher.update(&canonical);
        Ok(())
    }

    fn effect_row(&mut self, effects: &EffectRow) -> Result<(), CallConstraintInvariant> {
        match effects.tail() {
            EffectRowTail::Unknown => {
                self.tag(0);
                self.tag(0);
            }
            EffectRowTail::Closed => {
                self.tag(1);
                self.tag(0);
            }
            EffectRowTail::Variable(variable) => {
                self.tag(2);
                self.tag(1);
                self.u32(variable.index());
            }
        }
        let mut concrete = effects
            .concrete()
            .iter()
            .map(|effect| effect.semantic_digest())
            .collect::<Vec<_>>();
        concrete.sort_unstable();
        self.count(concrete.len())?;
        for effect in concrete {
            self.digest(effect.as_bytes());
        }
        Ok(())
    }
}

const fn result_constructor_tag(kind: ResultConstructorKind) -> u8 {
    match kind {
        ResultConstructorKind::Ok => 0,
        ResultConstructorKind::Err => 1,
    }
}

const fn fx_tag(id: FxCallableSignatureId) -> u8 {
    match id {
        FxCallableSignatureId::Style => 0,
        FxCallableSignatureId::Text => 1,
        FxCallableSignatureId::Color => 2,
        FxCallableSignatureId::Transform => 3,
        FxCallableSignatureId::Mask => 4,
        FxCallableSignatureId::Filter => 5,
        FxCallableSignatureId::Shader => 6,
        FxCallableSignatureId::Transition => 7,
        FxCallableSignatureId::Conditional => 8,
        FxCallableSignatureId::Stack => 9,
    }
}

const fn agent_tag(id: AgentIntrinsicSignatureId) -> u8 {
    match id {
        AgentIntrinsicSignatureId::Observe => 0,
        AgentIntrinsicSignatureId::Expect => 1,
        AgentIntrinsicSignatureId::Deny => 2,
        AgentIntrinsicSignatureId::Checkpoint => 3,
        AgentIntrinsicSignatureId::Note => 4,
        AgentIntrinsicSignatureId::Attach => 5,
        AgentIntrinsicSignatureId::ChoiceAction => 6,
        AgentIntrinsicSignatureId::Viewport => 7,
        AgentIntrinsicSignatureId::Layer => 8,
        AgentIntrinsicSignatureId::Object => 9,
        AgentIntrinsicSignatureId::Capture => 10,
        AgentIntrinsicSignatureId::ReadResource => 11,
        AgentIntrinsicSignatureId::EntityMeta => 12,
        AgentIntrinsicSignatureId::ProjectNeighbors => 13,
        AgentIntrinsicSignatureId::Signal => 14,
        AgentIntrinsicSignatureId::Metric => 15,
        AgentIntrinsicSignatureId::StatePath => 16,
        AgentIntrinsicSignatureId::ObservationPath => 17,
        AgentIntrinsicSignatureId::State => 18,
        AgentIntrinsicSignatureId::Observation => 19,
        AgentIntrinsicSignatureId::Diagnostics => 20,
        AgentIntrinsicSignatureId::Exists => 21,
        AgentIntrinsicSignatureId::ActionEnabled => 22,
        AgentIntrinsicSignatureId::All => 23,
        AgentIntrinsicSignatureId::Any => 24,
        AgentIntrinsicSignatureId::Not => 25,
        AgentIntrinsicSignatureId::Wait => 26,
        AgentIntrinsicSignatureId::AdvanceText => 27,
        AgentIntrinsicSignatureId::ViewportPoint => 28,
        AgentIntrinsicSignatureId::PointerClick => 29,
        AgentIntrinsicSignatureId::Invoke => 30,
        AgentIntrinsicSignatureId::RagQuery => 31,
    }
}

const fn presentation_tag(id: PresentationCallableId) -> u8 {
    match id {
        PresentationCallableId::View => 0,
        PresentationCallableId::Menu => 1,
        PresentationCallableId::Overlay => 2,
        PresentationCallableId::Background => 3,
        PresentationCallableId::Image => 4,
        PresentationCallableId::PlayerViewport => 5,
        PresentationCallableId::Show => 6,
        PresentationCallableId::RefBackground => 7,
        PresentationCallableId::RefShow => 8,
        PresentationCallableId::ClearBackground => 9,
        PresentationCallableId::Hide => 10,
    }
}
const fn dialogue_tag(id: DialogueCallableId) -> u8 {
    match id {
        DialogueCallableId::CharacterFactory => 0,
        DialogueCallableId::CharacterReconfigure => 1,
        DialogueCallableId::ContentApplication => 2,
        DialogueCallableId::ContentCall => 3,
    }
}
const fn collection_tag(id: CollectionMethodId) -> u8 {
    match id {
        CollectionMethodId::Len => 0,
        CollectionMethodId::Filter => 2,
        CollectionMethodId::Sum => 3,
        CollectionMethodId::Contains => 4,
    }
}
const fn presentation_handle_tag(id: PresentationHandleMethodId) -> u8 {
    match id {
        PresentationHandleMethodId::Show => 0,
        PresentationHandleMethodId::Hide => 1,
        PresentationHandleMethodId::Unmount => 2,
        PresentationHandleMethodId::Release => 3,
        PresentationHandleMethodId::Destroy => 4,
        PresentationHandleMethodId::OverlayPop => 5,
    }
}
const fn integer_tag(id: IntegerMethodId) -> u8 {
    match id {
        IntegerMethodId::Clamp => 0,
        IntegerMethodId::Min => 1,
        IntegerMethodId::Max => 2,
    }
}
const fn capacity_tag(id: CheckedCapacityOperation) -> u8 {
    match id {
        CheckedCapacityOperation::WithCapacity => 0,
        CheckedCapacityOperation::Trim => 1,
        CheckedCapacityOperation::ToString => 2,
        CheckedCapacityOperation::Pop => 3,
        CheckedCapacityOperation::PopFront => 4,
        CheckedCapacityOperation::Collect => 5,
        CheckedCapacityOperation::Push => 6,
        CheckedCapacityOperation::Reserve => 7,
        CheckedCapacityOperation::ShrinkTo => 8,
        CheckedCapacityOperation::Shrink => 9,
    }
}
const fn stage_tag(id: StageMethodId) -> u8 {
    match id {
        StageMethodId::Acquire => 0,
        StageMethodId::Look => 1,
    }
}
const fn line_context_tag(id: LineContextMethodId) -> u8 {
    match id {
        LineContextMethodId::VoiceHandle => 0,
    }
}
const fn line_schedule_tag(id: LineScheduleCallableId) -> u8 {
    match id {
        LineScheduleCallableId::At => 0,
    }
}
const fn drop_tag(id: DropCallableId) -> u8 {
    match id {
        DropCallableId::Drop => 0,
        DropCallableId::DropWithPolicy => 1,
        DropCallableId::DropOptional => 2,
        DropCallableId::OnDrop => 3,
    }
}
const fn promotion_tag(id: PromotionCallableId) -> u8 {
    match id {
        PromotionCallableId::Promote => 0,
        PromotionCallableId::PromoteUnchecked => 1,
        PromotionCallableId::Assume => 2,
    }
}
const fn probe_comparison_tag(id: ProbeComparisonOperator) -> u8 {
    match id {
        ProbeComparisonOperator::Eq => 0,
        ProbeComparisonOperator::NotEq => 1,
        ProbeComparisonOperator::Greater => 2,
        ProbeComparisonOperator::GreaterOrEqual => 3,
        ProbeComparisonOperator::Less => 4,
        ProbeComparisonOperator::LessOrEqual => 5,
    }
}
const fn std_float_operation_tag(id: StdFloatOperation) -> u8 {
    match id {
        StdFloatOperation::Abs => 0,
        StdFloatOperation::Floor => 1,
        StdFloatOperation::Ceil => 2,
        StdFloatOperation::Round => 3,
        StdFloatOperation::Trunc => 4,
        StdFloatOperation::Fract => 5,
        StdFloatOperation::Sqrt => 6,
        StdFloatOperation::Sin => 7,
        StdFloatOperation::Cos => 8,
        StdFloatOperation::Tan => 9,
        StdFloatOperation::Exp => 10,
        StdFloatOperation::Exp2 => 11,
        StdFloatOperation::Ln => 12,
        StdFloatOperation::Log2 => 13,
        StdFloatOperation::Log10 => 14,
        StdFloatOperation::Powf => 15,
        StdFloatOperation::Atan2 => 16,
        StdFloatOperation::MulAdd => 17,
        StdFloatOperation::IsNan => 18,
        StdFloatOperation::IsInfinite => 19,
        StdFloatOperation::IsFinite => 20,
        StdFloatOperation::IsSignPositive => 21,
        StdFloatOperation::IsSignNegative => 22,
        StdFloatOperation::ToBits => 23,
        StdFloatOperation::FromBits => 24,
        StdFloatOperation::ToF32 => 25,
        StdFloatOperation::ToF64 => 26,
    }
}
