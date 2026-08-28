//! Validated callable resolution products and their construction invariants.

use arcweft_character::id::CharacterId;

use super::super::ParameterExpectedTypeProjection;
use super::{
    Arc, CallableAuthorityRank, CallableCandidateId, CallableDeclarationKey, CallableGroupIndex,
    CallableLimits, CallableName, CallableParameterIndex, CallablePath, CallableRecord,
    CallableSignatureSchema, CheckedCallableId, EquivalentCallableSource, FunctionValueSignatureId,
    LanguageCallableFamily, LocalCallableId, ProjectCallablePath, PromotionCallableId,
    ResolveCallError, ResolvedAssociatedTypeReceiver, TypeKind,
};

use crate::types::constraints::{
    CheckedConstraintSourceProjection, PreparedConstraintSourceProjection,
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
    /// Dialogue origins retain the accepted module/path together with the
    /// operation.  ContentCall therefore cannot lose its canonical owner
    /// while moving through the resolver.
    LanguageDialogue {
        operation: super::super::DialogueCallableId,
        callee: Arc<super::super::ResolvedDialogueCalleeIdentity>,
    },
    Lexical {
        id: LocalCallableId,
        local: arcweft_lang_hir::identity::LocalId,
    },
    FunctionValue {
        id: FunctionValueSignatureId,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedResolvedCallableDefinition {
    id: CallableCandidateId,
    identity: super::PreparedResolvedCallableIdentity,
    origin: SignatureOrigin,
    checked: Option<CheckedCallableId>,
    record: Option<Arc<CallableRecord>>,
    intrinsic_schema: Option<Arc<CallableSignatureSchema>>,
    effect_instantiation: super::PreparedCallableEffectInstantiation,
    instantiation: CallableInstantiation,
    equivalent_sources: Arc<[EquivalentCallableSource]>,
    authority: Option<CallableAuthorityRank>,
    family: super::CallableFamily,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedResolvedCallable {
    definition: Arc<PreparedResolvedCallableDefinition>,
    state: PreparedResolvedCallableState,
}

/// Move-only permission to project one raw source type into the prepared
/// effect namespace owned by an exact callable definition. The token retains
/// only a private typed plan and a borrow of that definition; it cannot be
/// reattached to another schema or overlay.
pub(crate) struct PreparedCallableEffectProjectionToken<'a> {
    definition: &'a PreparedResolvedCallableDefinition,
    plan: PreparedCallableEffectProjectionPlan,
}

enum PreparedCallableEffectProjectionPlan {
    Parameter {
        coordinate: super::super::CallableParameterCoordinate,
        expected: ParameterExpectedTypeProjection,
        source_projection: PreparedConstraintSourceProjection,
    },
    GroupResult {
        current_group: CallableGroupIndex,
    },
    RemainingFunction {
        current_group: CallableGroupIndex,
    },
}

#[derive(Debug, Eq, PartialEq)]
enum PreparedResolvedCallableState {
    Base,
    PreparedContinuation {
        reference: super::super::PreparedCallContinuationRef,
        current_group: CallableGroupIndex,
        function_type: TypeKind,
    },
}

impl PreparedResolvedCallableDefinition {
    fn replay_eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.identity == other.identity
            && self.origin == other.origin
            && self.checked == other.checked
            && self.record == other.record
            && self.intrinsic_schema == other.intrinsic_schema
            && self
                .effect_instantiation
                .replay_eq(&other.effect_instantiation)
            && self.instantiation == other.instantiation
            && self.equivalent_sources == other.equivalent_sources
            && self.authority == other.authority
            && self.family == other.family
    }

    fn schema(&self) -> &CallableSignatureSchema {
        self.record
            .as_ref()
            .map(|record| record.schema())
            .or(self.intrinsic_schema.as_deref())
            .expect("prepared definition construction validates one schema authority")
    }

    fn source_invocation_effects(&self) -> crate::effect_row::EffectRow {
        self.schema()
            .effects()
            .fixed_row()
            .cloned()
            .unwrap_or_else(crate::effect_row::EffectRow::unknown)
    }

    fn source_function_type_from_group(
        &self,
        start: CallableGroupIndex,
    ) -> Result<TypeKind, super::super::CallConstraintInvariant> {
        if self.schema().group(start).is_none() {
            return Err(super::super::CallConstraintInvariant::MalformedSchemaInventory);
        }
        let effects = self.source_invocation_effects();
        let mut result = self.schema().result().clone();
        for group in self.schema().groups().iter().skip(start.get()).rev() {
            let parameters = group
                .parameters()
                .iter()
                .map(|parameter| {
                    parameter
                        .declared_type()
                        .cloned()
                        .ok_or(super::super::CallConstraintInvariant::MalformedSchemaInventory)
                })
                .collect::<Result<Vec<_>, _>>()?;
            result = TypeKind::function_with_effects(parameters, result, effects.clone());
        }
        Ok(result)
    }

    fn projected_function_type_from_group(
        &self,
        start: CallableGroupIndex,
    ) -> Result<TypeKind, super::super::CallConstraintInvariant> {
        let effects = self
            .effect_instantiation
            .project_invocation_effects(self.schema())?;
        self.projected_function_type_from_group_with_effects(start, &effects)
    }

    fn projected_function_type_from_group_with_effects(
        &self,
        start: CallableGroupIndex,
        effects: &crate::effect_row::EffectRow,
    ) -> Result<TypeKind, super::super::CallConstraintInvariant> {
        if self.schema().group(start).is_none() {
            return Err(super::super::CallConstraintInvariant::MalformedSchemaInventory);
        }
        let mut result = self.effect_instantiation.project_result(self.schema())?;
        for group in self.schema().groups().iter().skip(start.get()).rev() {
            let parameters = group
                .parameters()
                .iter()
                .map(|parameter| {
                    self.effect_instantiation
                        .project_parameter(
                            self.schema(),
                            super::super::CallableParameterCoordinate::new(
                                group.index(),
                                parameter.index(),
                            ),
                        )?
                        .ok_or(super::super::CallConstraintInvariant::MalformedSchemaInventory)
                })
                .collect::<Result<Vec<_>, _>>()?;
            result = TypeKind::function_with_effects(parameters, result, effects.clone());
        }
        Ok(result)
    }

    fn source_result_type_for_group(
        &self,
        current_group: CallableGroupIndex,
    ) -> Result<TypeKind, super::super::CallConstraintInvariant> {
        let next = CallableGroupIndex::try_from_usize(
            current_group
                .get()
                .checked_add(1)
                .ok_or(super::super::CallConstraintInvariant::MalformedSchemaInventory)?,
        )
        .map_err(|_| super::super::CallConstraintInvariant::MalformedSchemaInventory)?;
        if matches!(
            self.instantiation,
            CallableInstantiation::Extension { group, .. } if group == next
        ) || self.schema().group(next).is_none()
        {
            if self.schema().group(current_group).is_none() {
                return Err(super::super::CallConstraintInvariant::MalformedSchemaInventory);
            }
            return Ok(self.schema().result().clone());
        }
        self.source_function_type_from_group(next)
    }

    fn projected_result_type_for_group(
        &self,
        current_group: CallableGroupIndex,
    ) -> Result<TypeKind, super::super::CallConstraintInvariant> {
        let next = CallableGroupIndex::try_from_usize(
            current_group
                .get()
                .checked_add(1)
                .ok_or(super::super::CallConstraintInvariant::MalformedSchemaInventory)?,
        )
        .map_err(|_| super::super::CallConstraintInvariant::MalformedSchemaInventory)?;
        if matches!(
            self.instantiation,
            CallableInstantiation::Extension { group, .. } if group == next
        ) || self.schema().group(next).is_none()
        {
            if self.schema().group(current_group).is_none() {
                return Err(super::super::CallConstraintInvariant::MalformedSchemaInventory);
            }
            return self.effect_instantiation.project_result(self.schema());
        }
        self.projected_function_type_from_group(next)
    }
}

impl PreparedCallableEffectProjectionToken<'_> {
    /// Returns the owner-generated projected pattern for scalar/base
    /// constraints. Spread source patterns depend on the checked raw
    /// constructor and are therefore available only while consuming the
    /// token.
    pub(crate) fn projected_type(&self) -> Result<TypeKind, super::super::CallConstraintInvariant> {
        match &self.plan {
            PreparedCallableEffectProjectionPlan::Parameter {
                coordinate,
                expected,
                source_projection: PreparedConstraintSourceProjection::Scalar,
            } => self
                .definition
                .effect_instantiation
                .project_parameter(self.definition.schema(), *coordinate)?
                .map(|declared| expected.apply_to(&declared))
                .ok_or(super::super::CallConstraintInvariant::MalformedSchemaInventory),
            PreparedCallableEffectProjectionPlan::Parameter { .. } => {
                Err(super::super::CallConstraintInvariant::PreparedEffectInstantiationMismatch)
            }
            PreparedCallableEffectProjectionPlan::GroupResult { current_group } => self
                .definition
                .projected_result_type_for_group(*current_group),
            PreparedCallableEffectProjectionPlan::RemainingFunction { current_group } => self
                .definition
                .projected_function_type_from_group(*current_group),
        }
    }

    /// Consumes the definition-bound capability and returns the only lower
    /// input allowed to replace unresolved source tails. The typed plan was
    /// issued from the same definition that built the lower source row; no
    /// lower-normalized expected type participates in this authority.
    pub(crate) fn seal_actual(
        self,
        actual: &TypeKind,
    ) -> Result<TypeKind, super::super::CallConstraintInvariant> {
        let (source, projected) = match &self.plan {
            PreparedCallableEffectProjectionPlan::Parameter {
                coordinate,
                expected: value_projection,
                source_projection,
            } => {
                let source = self
                    .definition
                    .schema()
                    .group(coordinate.group())
                    .and_then(|group| group.parameter(coordinate.parameter()))
                    .and_then(|parameter| parameter.declared_type())
                    .ok_or(super::super::CallConstraintInvariant::MalformedSchemaInventory)?;
                let projected = self
                    .definition
                    .effect_instantiation
                    .project_parameter(self.definition.schema(), *coordinate)?
                    .ok_or(super::super::CallConstraintInvariant::MalformedSchemaInventory)?;
                let source = value_projection.apply_to(source);
                let projected = value_projection.apply_to(&projected);
                match source_projection {
                    PreparedConstraintSourceProjection::Scalar => (source, projected),
                    PreparedConstraintSourceProjection::InferSpreadContainer { .. } => {
                        let checked = CheckedConstraintSourceProjection::derive(
                            *source_projection,
                            actual,
                        )
                        .ok_or(
                            super::super::CallConstraintInvariant::PreparedEffectInstantiationMismatch,
                        )?;
                        (
                            checked.compose_expected(&source),
                            checked.compose_expected(&projected),
                        )
                    }
                }
            }
            PreparedCallableEffectProjectionPlan::GroupResult { current_group } => (
                self.definition
                    .source_result_type_for_group(*current_group)?,
                self.definition
                    .projected_result_type_for_group(*current_group)?,
            ),
            PreparedCallableEffectProjectionPlan::RemainingFunction { current_group } => (
                self.definition
                    .source_function_type_from_group(*current_group)?,
                self.definition
                    .projected_function_type_from_group(*current_group)?,
            ),
        };
        self.definition
            .effect_instantiation
            .seal_source_actual(&source, &projected, actual)
    }
}

impl PreparedResolvedCallable {
    pub(crate) fn replay_eq(&self, other: &Self) -> bool {
        if !self.definition.replay_eq(&other.definition) {
            return false;
        }
        match (&self.state, &other.state) {
            (PreparedResolvedCallableState::Base, PreparedResolvedCallableState::Base) => true,
            (
                PreparedResolvedCallableState::PreparedContinuation {
                    reference: left_reference,
                    current_group: left_group,
                    function_type: left_type,
                },
                PreparedResolvedCallableState::PreparedContinuation {
                    reference: right_reference,
                    current_group: right_group,
                    function_type: right_type,
                },
            ) => {
                left_reference == right_reference
                    && left_group == right_group
                    && left_type == right_type
            }
            _ => false,
        }
    }
}

/// Opaque index of one shared prepared definition inside the one consuming
/// detach batch.  It is deliberately not a semantic identity and has no
/// canonical encoder.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct PreparedCallableDefinitionKey(u32);

impl PreparedCallableDefinitionKey {
    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Move-only definition consumed by the final C sealer.  Every outer prepared
/// candidate is detached before the arena attempts to unwrap these shared
/// definition `Arc`s, so base and continuation candidates may safely share one
/// exact authority.
pub(crate) struct PreparedResolvedCallableDefinitionSealInput {
    pub(crate) id: CallableCandidateId,
    pub(crate) identity: super::PreparedResolvedCallableIdentity,
    pub(crate) origin: SignatureOrigin,
    pub(crate) checked: super::super::ResolvedCallableCheckedDefinition,
    pub(crate) effect_instantiation: super::PreparedCallableEffectInstantiation,
    pub(crate) instantiation: CallableInstantiation,
    pub(crate) equivalent_sources: Arc<[EquivalentCallableSource]>,
    pub(crate) authority: Option<CallableAuthorityRank>,
    pub(crate) family: super::CallableFamily,
}

impl PreparedResolvedCallableDefinitionSealInput {
    pub(crate) const fn identity(&self) -> &super::PreparedResolvedCallableIdentity {
        &self.identity
    }

    pub(crate) fn schema(&self) -> &CallableSignatureSchema {
        self.checked.schema()
    }
}

/// Stage-one detached candidate.  The opaque definition key is the only
/// retained relation between candidates that shared one prepared definition.
/// A continuation additionally carries only its issuer-bound graph reference
/// and the preparation-time projections that the dependency-first sealer must
/// compare with the already checked continuation.
pub(crate) enum DetachedPreparedResolvedCallable {
    Base {
        definition: PreparedCallableDefinitionKey,
    },
    PreparedContinuation {
        definition: PreparedCallableDefinitionKey,
        reference: super::super::PreparedCallContinuationRef,
        current_group: CallableGroupIndex,
        function_type: TypeKind,
    },
}

impl DetachedPreparedResolvedCallable {
    pub(crate) const fn definition(&self) -> PreparedCallableDefinitionKey {
        match self {
            Self::Base { definition } | Self::PreparedContinuation { definition, .. } => {
                *definition
            }
        }
    }
}

/// Stage-one owner for all candidate objects in one consumed prepared graph.
/// Definitions are deduplicated only by exact `Arc` identity; pointer values
/// never become semantic data or deterministic ordering inputs.
pub(crate) struct PreparedResolvedCallableDetachArena {
    definitions: Vec<Arc<PreparedResolvedCallableDefinition>>,
}

impl PreparedResolvedCallableDetachArena {
    pub(crate) const fn new() -> Self {
        Self {
            definitions: Vec::new(),
        }
    }

    pub(crate) fn detach(
        &mut self,
        callable: Arc<PreparedResolvedCallable>,
    ) -> Result<DetachedPreparedResolvedCallable, super::super::CallConstraintInvariant> {
        let callable = Arc::try_unwrap(callable)
            .map_err(|_| super::super::CallConstraintInvariant::InvalidPreparedNodeState)?;
        let PreparedResolvedCallable { definition, state } = callable;
        let key = self.register_definition(definition)?;
        Ok(match state {
            PreparedResolvedCallableState::Base => {
                DetachedPreparedResolvedCallable::Base { definition: key }
            }
            PreparedResolvedCallableState::PreparedContinuation {
                reference,
                current_group,
                function_type,
            } => DetachedPreparedResolvedCallable::PreparedContinuation {
                definition: key,
                reference,
                current_group,
                function_type,
            },
        })
    }

    fn register_definition(
        &mut self,
        definition: Arc<PreparedResolvedCallableDefinition>,
    ) -> Result<PreparedCallableDefinitionKey, super::super::CallConstraintInvariant> {
        if let Some(index) = self
            .definitions
            .iter()
            .position(|candidate| Arc::ptr_eq(candidate, &definition))
        {
            return u32::try_from(index)
                .map(PreparedCallableDefinitionKey)
                .map_err(|_| super::super::CallConstraintInvariant::InvalidPreparedNodeState);
        }
        let index = u32::try_from(self.definitions.len())
            .map_err(|_| super::super::CallConstraintInvariant::InvalidPreparedNodeState)?;
        self.definitions.push(definition);
        Ok(PreparedCallableDefinitionKey(index))
    }

    /// Stage two: after every graph candidate has been detached, consume each
    /// unique shared definition exactly once.  Any surviving prepared owner is
    /// an authority leak and rejects rather than cloning a replacement.
    pub(crate) fn finish(
        self,
    ) -> Result<PreparedResolvedCallableDefinitionBatch, super::super::CallConstraintInvariant>
    {
        let definitions = self
            .definitions
            .into_iter()
            .map(|definition| {
                let definition = Arc::try_unwrap(definition)
                    .map_err(|_| super::super::CallConstraintInvariant::InvalidPreparedNodeState)?;
                let PreparedResolvedCallableDefinition {
                    id,
                    identity,
                    origin,
                    checked,
                    record,
                    intrinsic_schema,
                    effect_instantiation,
                    instantiation,
                    equivalent_sources,
                    authority,
                    family,
                } = definition;
                let checked = match (checked, record, intrinsic_schema) {
                    (Some(id), Some(record), None) => {
                        super::super::ResolvedCallableCheckedDefinition::Catalog { id, record }
                    }
                    (None, None, Some(schema)) => {
                        super::super::ResolvedCallableCheckedDefinition::Intrinsic { schema }
                    }
                    _ => {
                        return Err(super::super::CallConstraintInvariant::PreparedSchemaMismatch);
                    }
                };
                Ok(Some(PreparedResolvedCallableDefinitionSealInput {
                    id,
                    identity,
                    origin,
                    checked,
                    effect_instantiation,
                    instantiation,
                    equivalent_sources,
                    authority,
                    family,
                }))
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        Ok(PreparedResolvedCallableDefinitionBatch { definitions })
    }
}

/// Move-only stage-two batch.  Definition inputs are taken lazily in graph
/// dependency order, preserving deterministic first-error order while still
/// guaranteeing one final base per shared prepared definition.
pub(crate) struct PreparedResolvedCallableDefinitionBatch {
    definitions: Box<[Option<PreparedResolvedCallableDefinitionSealInput>]>,
}

impl PreparedResolvedCallableDefinitionBatch {
    pub(crate) fn get(
        &self,
        key: PreparedCallableDefinitionKey,
    ) -> Result<&PreparedResolvedCallableDefinitionSealInput, super::super::CallConstraintInvariant>
    {
        self.definitions
            .get(key.index())
            .and_then(Option::as_ref)
            .ok_or(super::super::CallConstraintInvariant::InvalidPreparedNodeState)
    }

    pub(crate) fn take(
        &mut self,
        key: PreparedCallableDefinitionKey,
    ) -> Result<PreparedResolvedCallableDefinitionSealInput, super::super::CallConstraintInvariant>
    {
        self.definitions
            .get_mut(key.index())
            .and_then(Option::take)
            .ok_or(super::super::CallConstraintInvariant::InvalidPreparedNodeState)
    }

    /// Affine completion proof for the definition arena. Dropping a batch with
    /// an unconsumed definition would silently lose prepared semantic
    /// authority, so the graph finalizer must consume this method after its
    /// dependency-ordered node walk.
    pub(crate) fn finish(self) -> Result<(), super::super::CallConstraintInvariant> {
        if self.definitions.iter().any(Option::is_some) {
            Err(super::super::CallConstraintInvariant::InvalidPreparedNodeState)
        } else {
            Ok(())
        }
    }
}

impl PreparedResolvedCallable {
    pub(crate) fn prepared_continuation(
        &self,
    ) -> Option<&super::super::PreparedCallContinuationRef> {
        match &self.state {
            PreparedResolvedCallableState::PreparedContinuation { reference, .. } => {
                Some(reference)
            }
            PreparedResolvedCallableState::Base => None,
        }
    }

    pub(crate) fn call_group(&self) -> CallableGroupIndex {
        match &self.state {
            PreparedResolvedCallableState::Base => self.base_call_group(),
            PreparedResolvedCallableState::PreparedContinuation { current_group, .. } => {
                *current_group
            }
        }
    }

    pub(crate) fn prepared_function_type(&self) -> Option<&TypeKind> {
        match &self.state {
            PreparedResolvedCallableState::Base => None,
            PreparedResolvedCallableState::PreparedContinuation { function_type, .. } => {
                Some(function_type)
            }
        }
    }

    /// The sole prepared-stage classification of runtime value dispatch.
    /// Continuations are invoked through the prior application value; lexical
    /// and independent function-value identities are value callees at their
    /// base group as well.
    pub(crate) fn requires_value_callee(&self) -> bool {
        matches!(
            &self.state,
            PreparedResolvedCallableState::PreparedContinuation { .. }
        ) || self.definition.identity.requires_value_callee()
    }

    pub(crate) fn try_from_prepared_continuation<P, U>(
        graph: &super::super::PreparedCallGraph<P, U>,
        reference: &super::super::PreparedCallContinuationRef,
        actual: &TypeKind,
    ) -> Result<Self, super::super::CallConstraintInvariant>
    where
        P: super::super::PreparedCallPrefixPayload<Unselected = U>,
    {
        let seed = graph.continuation_candidate_seed(reference)?;
        let (definition, reference, current_group, function_type) = seed.into_candidate_parts();
        if &function_type != actual {
            return Err(super::super::CallConstraintInvariant::PreparedFunctionTypeMismatch);
        }
        Ok(Self {
            definition,
            state: PreparedResolvedCallableState::PreparedContinuation {
                reference,
                current_group,
                function_type,
            },
        })
    }
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

impl PreparedResolvedCallable {
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
            None,
            None,
        )
    }

    pub(crate) fn try_from_intrinsic_with_enum_seed(
        id: CallableCandidateId,
        origin: SignatureOrigin,
        seed: &super::AcceptedEnumVariantCase,
        schema: Arc<CallableSignatureSchema>,
        instantiation: CallableInstantiation,
        equivalent_sources: Vec<EquivalentCallableSource>,
        limits: &CallableLimits,
    ) -> Result<Self, ResolveCallError> {
        if schema.effects().fixed_row().is_none() {
            return Err(ResolveCallError::InvalidResolvedCallable);
        }
        let CallableCandidateId::EnumVariant(candidate) = &id else {
            return Err(ResolveCallError::InvalidResolvedCallable);
        };
        let CallableInstantiation::ExpectedEnum { expected } = &instantiation else {
            return Err(ResolveCallError::InvalidResolvedCallable);
        };
        if seed.id() != candidate
            || seed.expected() != expected
            || seed.diagnostic_name() != candidate.variant()
            || seed.schema.semantic_digest() != schema.semantic_digest()
        {
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
            Some(seed),
            None,
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
            None,
            None,
        )
    }

    pub(crate) fn try_from_intrinsic_with_function_value(
        id: CallableCandidateId,
        origin: SignatureOrigin,
        function_origin: &super::PreparedFunctionValueOriginEvidence,
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
            None,
            Some(function_origin),
        )
    }

    pub(crate) fn try_from_intrinsic_with_lexical(
        id: LocalCallableId,
        local: arcweft_lang_hir::identity::LocalId,
        schema: Arc<CallableSignatureSchema>,
        limits: &CallableLimits,
    ) -> Result<Self, ResolveCallError> {
        let candidate = CallableCandidateId::Local(id.clone());
        let family = candidate.intrinsic_family();
        Self::try_new_state(
            candidate,
            SignatureOrigin::Lexical { id, local },
            None,
            None,
            Some(schema),
            CallableInstantiation::None,
            Vec::new(),
            None,
            family,
            limits,
            None,
            None,
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
        enum_seed: Option<&super::AcceptedEnumVariantCase>,
        function_origin: Option<&super::PreparedFunctionValueOriginEvidence>,
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
        let identity = super::PreparedResolvedCallableIdentity::try_from_parts(
            &id,
            &origin,
            checked.as_ref(),
            &instantiation,
            enum_seed,
            function_origin,
        )
        .ok_or(ResolveCallError::InvalidResolvedCallable)?;
        let schema = record
            .as_ref()
            .map(|record| record.schema())
            .or(intrinsic_schema.as_deref())
            .ok_or(ResolveCallError::InvalidResolvedCallable)?;
        let effect_instantiation = super::PreparedCallableEffectInstantiation::seal(schema)
            .map_err(|_| ResolveCallError::InvalidResolvedCallable)?;
        Ok(Self {
            definition: Arc::new(PreparedResolvedCallableDefinition {
                id,
                identity,
                origin,
                checked,
                record,
                intrinsic_schema,
                effect_instantiation,
                instantiation,
                equivalent_sources: equivalent_sources.into(),
                authority,
                family,
            }),
            state: PreparedResolvedCallableState::Base,
        })
    }
    pub fn id(&self) -> &CallableCandidateId {
        &self.definition.id
    }
    pub(crate) fn definition(&self) -> Arc<PreparedResolvedCallableDefinition> {
        Arc::clone(&self.definition)
    }
    pub fn family(&self) -> super::CallableFamily {
        self.definition.family
    }
    pub fn origin(&self) -> &SignatureOrigin {
        &self.definition.origin
    }
    pub fn checked(&self) -> Option<&CheckedCallableId> {
        self.definition.checked.as_ref()
    }
    pub fn record(&self) -> Option<&Arc<CallableRecord>> {
        self.definition.record.as_ref()
    }
    /// Returns the single validated schema authority.
    ///
    /// # Panics
    ///
    /// Panics only if a value bypassed `PreparedResolvedCallable` construction.
    pub fn schema(&self) -> &CallableSignatureSchema {
        self.definition
            .record
            .as_ref()
            .map(|record| record.schema())
            .or(self.definition.intrinsic_schema.as_deref())
            .expect("PreparedResolvedCallable construction validates one schema authority")
    }

    pub(crate) fn constraint_parameter_type(
        &self,
        coordinate: super::super::CallableParameterCoordinate,
    ) -> Result<Option<TypeKind>, super::super::CallConstraintInvariant> {
        self.definition
            .effect_instantiation
            .project_parameter(self.schema(), coordinate)
    }

    pub(crate) fn constraint_result_type(
        &self,
    ) -> Result<TypeKind, super::super::CallConstraintInvariant> {
        self.definition
            .effect_instantiation
            .project_result(self.schema())
    }

    /// Projects the complete callable type through this definition's sole
    /// higher-order effect overlay while using the caller-supplied checked
    /// invocation row only for each curried application boundary.
    pub(crate) fn constraint_callable_type_with_invocation_effects(
        &self,
        effects: &crate::effect_row::EffectRow,
    ) -> Result<TypeKind, super::super::CallConstraintInvariant> {
        self.definition
            .projected_function_type_from_group_with_effects(self.base_call_group(), effects)
    }

    pub(crate) fn issue_parameter_effect_projection(
        &self,
        coordinate: super::super::CallableParameterCoordinate,
        expected: &ParameterExpectedTypeProjection,
        source_projection: PreparedConstraintSourceProjection,
    ) -> Result<PreparedCallableEffectProjectionToken<'_>, super::super::CallConstraintInvariant>
    {
        self.schema()
            .group(coordinate.group())
            .and_then(|group| group.parameter(coordinate.parameter()))
            .and_then(|parameter| parameter.declared_type())
            .ok_or(super::super::CallConstraintInvariant::MalformedSchemaInventory)?;
        self.definition
            .effect_instantiation
            .project_parameter(self.schema(), coordinate)?
            .ok_or(super::super::CallConstraintInvariant::MalformedSchemaInventory)?;
        Ok(PreparedCallableEffectProjectionToken {
            definition: &self.definition,
            plan: PreparedCallableEffectProjectionPlan::Parameter {
                coordinate,
                expected: expected.clone(),
                source_projection,
            },
        })
    }

    pub(crate) fn issue_group_result_effect_projection(
        &self,
        current_group: CallableGroupIndex,
    ) -> Result<PreparedCallableEffectProjectionToken<'_>, super::super::CallConstraintInvariant>
    {
        self.definition
            .source_result_type_for_group(current_group)?;
        self.definition
            .projected_result_type_for_group(current_group)?;
        Ok(PreparedCallableEffectProjectionToken {
            definition: &self.definition,
            plan: PreparedCallableEffectProjectionPlan::GroupResult { current_group },
        })
    }

    pub(crate) fn issue_remaining_function_effect_projection(
        &self,
        current_group: CallableGroupIndex,
    ) -> Result<PreparedCallableEffectProjectionToken<'_>, super::super::CallConstraintInvariant>
    {
        self.definition
            .source_function_type_from_group(current_group)?;
        self.definition
            .projected_function_type_from_group(current_group)?;
        Ok(PreparedCallableEffectProjectionToken {
            definition: &self.definition,
            plan: PreparedCallableEffectProjectionPlan::RemainingFunction { current_group },
        })
    }

    pub(crate) fn prepared_effect_instantiation(
        &self,
    ) -> &super::PreparedCallableEffectInstantiation {
        &self.definition.effect_instantiation
    }
    pub fn instantiation(&self) -> &CallableInstantiation {
        &self.definition.instantiation
    }
    pub fn equivalent_sources(&self) -> &[EquivalentCallableSource] {
        &self.definition.equivalent_sources
    }
    pub fn authority(&self) -> Option<CallableAuthorityRank> {
        self.definition.authority
    }

    pub(crate) fn base_call_group(&self) -> CallableGroupIndex {
        match &self.definition.instantiation {
            CallableInstantiation::Extension { .. }
            | CallableInstantiation::None
            | CallableInstantiation::ExpectedEnum { .. }
            | CallableInstantiation::Result { .. }
            | CallableInstantiation::Option
            | CallableInstantiation::Character { .. }
            | CallableInstantiation::Receiver { .. }
            | CallableInstantiation::TypeReceiver { .. } => CallableGroupIndex::ZERO,
        }
    }

    /// Returns the exact next curried group accepted by this callable for a
    /// call at `current_group`.
    ///
    /// The extension-receiver group is consumed by the dot receiver itself;
    /// it is therefore not exposed as a curried continuation.  This is the
    /// same group rule used while committing [`CallTargetFacts`], kept on the
    /// callable owner so later consumers cannot invent a second projection.
    pub(crate) fn next_group_for(
        &self,
        current_group: CallableGroupIndex,
    ) -> Option<CallableGroupIndex> {
        let next = CallableGroupIndex::try_from_usize(current_group.get().checked_add(1)?).ok()?;
        self.schema()
            .group(next)
            .is_some()
            .then_some(next)
            .filter(|next| {
                !matches!(
                    self.instantiation(),
                    CallableInstantiation::Extension { group, .. } if group == next
                )
            })
    }

    /// Projects the result owned by one exact selected group.
    ///
    /// A non-final group returns the typed function for all remaining groups;
    /// a final group returns the callable's declared result.  Unknown effect
    /// tails remain unknown in the projected function type, matching the
    /// existing resolver's typed partial-call rule.
    pub(crate) fn result_type_for_group(
        &self,
        current_group: CallableGroupIndex,
    ) -> Option<TypeKind> {
        self.definition
            .projected_result_type_for_group(current_group)
            .ok()
    }
}

fn underlying_candidate(id: &CallableCandidateId) -> &CallableCandidateId {
    id
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
        (
            CallableCandidateId::Local(id),
            SignatureOrigin::Lexical {
                id: origin_id,
                local,
            },
            None,
        ) => id == origin_id && id.local() == *local,
        (
            CallableCandidateId::FunctionValue(id),
            SignatureOrigin::FunctionValue { id: origin_id },
            None,
        ) => id == origin_id,
        (id, SignatureOrigin::Language { family }, None) => language_origin_matches(id, *family),
        (
            CallableCandidateId::Dialogue(candidate),
            SignatureOrigin::LanguageDialogue { operation, .. },
            None,
        ) => candidate == operation,
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
        (CallableCandidateId::Result(id_kind), CallableInstantiation::Result { kind }) => {
            id_kind == kind
        }
        (CallableCandidateId::EnumVariant(_), CallableInstantiation::ExpectedEnum { .. })
        | (CallableCandidateId::Option(_), CallableInstantiation::Option)
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
            | CallableCandidateId::LineContextMethod(_)
            | CallableCandidateId::Drop(_),
            CallableInstantiation::Receiver { .. },
        )
        | (CallableCandidateId::Environment(_), CallableInstantiation::Receiver { .. })
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

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ResolvedCallTarget {
    Candidates(NonEmptyResolvedCandidates),
    NonCallable(ResolvedNonCallableTarget),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct NonEmptyResolvedCandidates {
    candidates: Box<[PreparedResolvedCallable]>,
}
impl NonEmptyResolvedCandidates {
    pub(crate) fn try_new(
        candidates: Vec<PreparedResolvedCallable>,
        limits: &CallableLimits,
    ) -> Result<Self, ResolveCallError> {
        Self::try_new_prepared(candidates, limits)
    }

    pub(crate) fn try_new_prepared(
        candidates: Vec<PreparedResolvedCallable>,
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
            candidates: candidates.into_boxed_slice(),
        })
    }
    pub(crate) fn first(&self) -> &PreparedResolvedCallable {
        &self.candidates[0]
    }
    pub(crate) fn into_shared(
        self,
    ) -> Result<Vec<Arc<PreparedResolvedCallable>>, ResolveCallError> {
        Ok(self
            .candidates
            .into_vec()
            .into_iter()
            .map(Arc::new)
            .collect())
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

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ResolveCallOutcome {
    Resolved(ResolvedCallTarget),
    Invariant(super::super::CallConstraintInvariant),
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
