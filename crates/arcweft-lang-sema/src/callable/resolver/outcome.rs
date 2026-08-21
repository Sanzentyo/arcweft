//! Validated callable resolution products and their construction invariants.

use std::num::NonZeroU32;

use arcweft_character::id::CharacterId;

use super::{
    Arc, CallableAuthorityRank, CallableCandidateId, CallableDeclarationKey, CallableGroupIndex,
    CallableId, CallableLimits, CallableName, CallableParameterIndex, CallablePath, CallableRecord,
    CallableSignatureSchema, CheckedCallableId, CurriedCallableId, EquivalentCallableSource,
    FunctionValueSignatureId, LanguageCallableFamily, LocalCallableId, ProjectCallablePath,
    PromotionCallableId, ResolveCallError, ResolvedAssociatedTypeReceiver, TypeKind,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignatureOrigin {
    Project {
        declaration: CallableDeclarationKey,
        binding: Option<ProjectCallablePath>,
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
    checked: Option<CheckedCallableId>,
    record: Option<Arc<CallableRecord>>,
    intrinsic_schema: Option<Arc<CallableSignatureSchema>>,
    instantiation: CallableInstantiation,
    equivalent_sources: Arc<[EquivalentCallableSource]>,
    authority: Option<CallableAuthorityRank>,
    family: super::CallableFamily,
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
    Extension {
        receiver: TypeKind,
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
}

impl ResolvedCallable {
    pub(crate) fn try_from_intrinsic(
        id: CallableCandidateId,
        origin: SignatureOrigin,
        schema: Arc<CallableSignatureSchema>,
        instantiation: CallableInstantiation,
        equivalent_sources: Vec<EquivalentCallableSource>,
        limits: &CallableLimits,
    ) -> Result<Self, ResolveCallError> {
        if schema.effects().fixed_row().is_none() {
            return Err(ResolveCallError::InvalidResolvedCallable);
        }
        let family = id.intrinsic_family();
        Self::try_new_state(
            id,
            origin,
            None,
            None,
            Some(schema),
            instantiation,
            equivalent_sources,
            None,
            family,
            limits,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_from_checked_record(
        checked: CheckedCallableId,
        record: Arc<CallableRecord>,
        origin: SignatureOrigin,
        instantiation: CallableInstantiation,
        equivalent_sources: Vec<EquivalentCallableSource>,
        authority: Option<CallableAuthorityRank>,
        limits: &CallableLimits,
    ) -> Result<Self, ResolveCallError> {
        if !checked_matches_record(&checked, &record) || authority != Some(record.authority()) {
            return Err(ResolveCallError::InvalidResolvedCallable);
        }
        let id = record.id().clone();
        let family = record.family();
        Self::try_new_state(
            id,
            origin,
            Some(checked),
            Some(record),
            None,
            instantiation,
            equivalent_sources,
            authority,
            family,
            limits,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn try_new_state(
        id: CallableCandidateId,
        origin: SignatureOrigin,
        checked: Option<CheckedCallableId>,
        record: Option<Arc<CallableRecord>>,
        intrinsic_schema: Option<Arc<CallableSignatureSchema>>,
        instantiation: CallableInstantiation,
        equivalent_sources: Vec<EquivalentCallableSource>,
        authority: Option<CallableAuthorityRank>,
        family: super::CallableFamily,
        limits: &CallableLimits,
    ) -> Result<Self, ResolveCallError> {
        let backing_is_valid = match (&checked, &record, &intrinsic_schema) {
            (Some(checked), Some(record), None) => {
                checked_matches_record(checked, record)
                    && underlying_candidate(&id) == record.id()
                    && authority == Some(record.authority())
                    && expected_family(&id, Some(record)) == family
            }
            (None, None, Some(schema)) => {
                schema.effects().fixed_row().is_some()
                    && authority.is_none()
                    && expected_family(&id, None) == family
            }
            _ => false,
        };
        if equivalent_sources.len().saturating_add(1) > limits.max_candidates_per_call()
            || !backing_is_valid
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
            let schema = record
                .as_ref()
                .map(|record| record.schema())
                .or(intrinsic_schema.as_deref())
                .ok_or(ResolveCallError::InvalidResolvedCallable)?;
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
            checked,
            record,
            intrinsic_schema,
            instantiation,
            equivalent_sources: equivalent_sources.into(),
            authority,
            family,
        })
    }
    pub const fn id(&self) -> &CallableCandidateId {
        &self.id
    }
    pub const fn family(&self) -> super::CallableFamily {
        self.family
    }
    pub const fn origin(&self) -> &SignatureOrigin {
        &self.origin
    }
    pub const fn checked(&self) -> Option<&CheckedCallableId> {
        self.checked.as_ref()
    }
    pub const fn record(&self) -> Option<&Arc<CallableRecord>> {
        self.record.as_ref()
    }
    /// Returns the single validated schema authority.
    ///
    /// # Panics
    ///
    /// Panics only if a value bypassed `ResolvedCallable` construction.
    pub fn schema(&self) -> &CallableSignatureSchema {
        self.record
            .as_ref()
            .map(|record| record.schema())
            .or(self.intrinsic_schema.as_deref())
            .expect("ResolvedCallable construction validates one schema authority")
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
            CallableInstantiation::Curried { group, .. } => *group,
            CallableInstantiation::Extension { .. }
            | CallableInstantiation::None
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
        Self::try_new_state(
            CallableCandidateId::Curried(id),
            self.origin.clone(),
            self.checked.clone(),
            self.record.clone(),
            self.intrinsic_schema.clone(),
            CallableInstantiation::Curried { base, group },
            self.equivalent_sources.to_vec(),
            self.authority,
            self.family,
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
        if self.checked.is_some() || self.record.is_some() || self.intrinsic_schema.is_none() {
            return Err(ResolveCallError::InvalidResolvedCallable);
        }
        Self::try_from_intrinsic(
            self.id.clone(),
            self.origin.clone(),
            Arc::new(schema),
            CallableInstantiation::Character { owner },
            self.equivalent_sources.to_vec(),
            limits,
        )
    }
}

fn underlying_candidate(id: &CallableCandidateId) -> &CallableCandidateId {
    match id {
        CallableCandidateId::Curried(id) => underlying_candidate(id.base()),
        id => id,
    }
}

fn expected_family(
    id: &CallableCandidateId,
    record: Option<&Arc<CallableRecord>>,
) -> super::CallableFamily {
    if let Some(record) = record {
        record.family()
    } else {
        underlying_candidate(id).intrinsic_family()
    }
}

fn checked_matches_record(checked: &CheckedCallableId, record: &CallableRecord) -> bool {
    matches!(
        (checked.declaration(), record.id()),
        (
            super::CheckedCallableDeclaration::Project(checked),
            CallableCandidateId::Project(record),
        ) if checked == record
    ) || matches!(
        (checked.declaration(), record.id()),
        (
            super::CheckedCallableDeclaration::Detached(checked),
            CallableCandidateId::Detached(record),
        ) if checked == record
    ) || matches!(
        (checked.declaration(), record.id()),
        (
            super::CheckedCallableDeclaration::Environment(checked),
            CallableCandidateId::Environment(record),
        ) if checked == record
    ) || matches!(
        (checked.declaration(), record.id()),
        (
            super::CheckedCallableDeclaration::Standard(checked),
            CallableCandidateId::Standard(record),
        ) if checked == record
    )
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
            | (
                CallableCandidateId::LineContextMethod(_),
                LanguageCallableFamily::LineContextMethod
            )
            | (
                CallableCandidateId::LineSchedule(_),
                LanguageCallableFamily::LineSchedule
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
        (CallableCandidateId::EnumVariant(_), CallableInstantiation::ExpectedEnum { .. })
        | (CallableCandidateId::Option(_), CallableInstantiation::Option { .. })
        | (
            CallableCandidateId::Project(_)
            | CallableCandidateId::Environment(_)
            | CallableCandidateId::Standard(_),
            CallableInstantiation::Extension { .. },
        )
        | (
            CallableCandidateId::Presentation(_),
            CallableInstantiation::Character { .. } | CallableInstantiation::None,
        )
        | (
            CallableCandidateId::Dialogue(_),
            CallableInstantiation::Character { .. } | CallableInstantiation::None,
        )
        | (
            CallableCandidateId::CollectionMethod(_)
            | CallableCandidateId::PresentationHandleMethod(_)
            | CallableCandidateId::IntegerMethod(_)
            | CallableCandidateId::DomainMethod(_)
            | CallableCandidateId::StageMethod(_)
            | CallableCandidateId::LineContextMethod(_),
            CallableInstantiation::Receiver { .. },
        )
        | (CallableCandidateId::Environment(_), CallableInstantiation::TypeReceiver { .. })
        | (
            CallableCandidateId::Fx(_)
            | CallableCandidateId::Builtin(_)
            | CallableCandidateId::Agent(_)
            | CallableCandidateId::Project(_)
            | CallableCandidateId::Detached(_)
            | CallableCandidateId::Environment(_)
            | CallableCandidateId::Standard(_)
            | CallableCandidateId::Local(_)
            | CallableCandidateId::FunctionValue(_)
            | CallableCandidateId::LineSchedule(_)
            | CallableCandidateId::Drop(_)
            | CallableCandidateId::Promotion(_),
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
