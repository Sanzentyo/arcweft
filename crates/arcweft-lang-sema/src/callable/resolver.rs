//! Validated callable resolver products.

use std::{num::NonZeroU32, sync::Arc};

use arcweft_character::id::{CharacterId, CharacterPartId};
use arcweft_lang_hir::symbol::CallableDeclarationId;

use crate::{effect_model::CallableId, types::TypeKind};

use super::{
    CallableAuthorityRank, CallableCandidateId, CallableGroupIndex, CallableLimits, CallableName,
    CallableParameterIndex, CallablePath, CallableSignatureSchema, EquivalentCallableSource,
    FunctionValueSignatureId, LanguageCallableFamily, LocalCallableId, ProjectCallablePath,
    PromotionCallableId, ResolveCallError, TraitCallableId,
};

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
    #[allow(dead_code, reason = "constructed by the shared resolver migration cut")]
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
