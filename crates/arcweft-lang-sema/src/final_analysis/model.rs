//! Generation-bound checked semantic fact model.

use super::CheckedViewFxApplication;
use super::fx_application::SealedContentFxEdgePlan;
use super::match_edges::NestedPathEvidence;
use super::nominal_semantic::ProjectNominalSemanticDigest;
use super::{
    CallableDeclarationKey, CharacterDialogueCharacterType, CharacterDialogueType, CharacterId,
    CheckedRichTextReport, DeclarationIdentityFamily, DialogueLineId, DialogueTextKey, EffectSet,
    EnvironmentBindingId, ExprId, GenericParameterOwnerId, GenericTypeParameterId, HirFlowIdentity,
    HirItemFamily, HirLiteral, ItemId, LocalId, PatternId, ProjectNominalDeclaration,
    ProjectNominalDeclarationId, PublicId, SemanticTypeDigest, StmtId, TypeKind,
    TypeParameterSubstitutions,
};
use crate::callable::{
    CallableEvaluatedEffect, CallableLogLevel, CallableReceiverMode, CharacterDialoguePatchContext,
    CheckedCallApplicationDigest, CheckedCallableJoin, CheckedCallableJoinDigest, DropCallableId,
    OpenArgumentId,
};
pub use crate::character_dialogue::CharacterDialogueFieldCoordinate;
use crate::checked_compile_time::CheckedCompileTimeScalar;
use crate::checked_rich_text::CheckedContentApplicationId;
use crate::checked_rich_text::Milli;
use crate::semantic_coordinate::{
    AcceptedDeclarationSemanticId, CheckedExpressionCoordinateEvidence, CheckedSemanticPath,
    StableCheckedValueCoordinate,
};
use crate::types::{CharacterField, EntityKind};
use arcweft_core::value::RuntimeAgentField;
use arcweft_id::closed_enum::ClosedEnumValueId;
use arcweft_lang_hir::identity::HirSnapshotId;
use arcweft_lang_hir::symbol::{CallableDeclarationDigest, ExternalDeclarationId};
use arcweft_source::SourceSpan;
use thiserror::Error;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RegisteredSemanticValueId {
    identity: [u8; 32],
    environment_binding: Option<EnvironmentBindingId>,
}

impl RegisteredSemanticValueId {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self {
            identity: bytes,
            environment_binding: None,
        }
    }

    pub(crate) fn for_environment_binding(binding: EnvironmentBindingId) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft-registered-environment-value-v1\0");
        hasher.update(binding.as_str().as_bytes());
        Self {
            identity: *hasher.finalize().as_bytes(),
            environment_binding: Some(binding),
        }
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.identity
    }

    pub const fn environment_binding(&self) -> Option<&EnvironmentBindingId> {
        self.environment_binding.as_ref()
    }
}

/// Exact project callable selected by semantic analysis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedProjectCallable {
    declaration: CallableDeclarationKey,
    owner: ItemId,
}

impl CheckedProjectCallable {
    pub const fn new(declaration: CallableDeclarationKey, owner: ItemId) -> Self {
        Self { declaration, owner }
    }

    pub const fn declaration(&self) -> &CallableDeclarationKey {
        &self.declaration
    }

    pub const fn owner(&self) -> ItemId {
        self.owner
    }
}

/// Closed semantic owner selected for one project entity reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedProjectItemOwner {
    /// Authored declaration bound to this accepted final-HIR generation.
    Retained(ItemId),
    /// Structural Flow owner retained by the same project callable authority
    /// without becoming an ordinary callable target.
    Flow {
        declaration: CallableDeclarationKey,
        item: ItemId,
    },
    /// Registered declaration bound to this accepted project-symbol world.
    External(ExternalDeclarationId),
}

const PROJECT_ITEM_SEMANTIC_DOMAIN: &[u8] = b"arcweft.lang.accepted-project-item.v1\0";

/// Canonical semantic identity of one accepted project item value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct AcceptedProjectItemSemanticId([u8; 32]);

impl AcceptedProjectItemSemanticId {
    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Exact project declaration selected by an entity-reference leaf.
///
/// `semantic_id` is the final checked identity. Public spelling and raw owners
/// remain lookup/diagnostic evidence only. Structural Flow identity binds its
/// accepted module-preserving declaration digest, while other entity families
/// bind their accepted public identity. Character facts also retain the
/// validated [`CharacterId`] selected by registration, so consumers never
/// reconstruct it from source text or fabricate an [`ItemId`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedProjectItem {
    semantic_id: AcceptedProjectItemSemanticId,
    value_type: SemanticTypeDigest,
    diagnostic_public_id: PublicId,
    family: DeclarationIdentityFamily,
    owner: CheckedProjectItemOwner,
    character: Option<CharacterId>,
    value: Option<TypeKind>,
}

impl CheckedProjectItem {
    pub(crate) fn new_flow(declaration: CallableDeclarationKey, item: ItemId) -> Option<Self> {
        let CallableDeclarationKey::Flow(flow) = &declaration else {
            return None;
        };
        let family = DeclarationIdentityFamily::Flow;
        let value_type = project_item_type(family, None)
            .semantic_identity_digest()
            .ok()?;
        let semantic_id = accepted_project_item_semantic_id(
            family,
            value_type,
            &ProjectItemSemanticOwner::Flow(declaration.semantic_digest()),
        );
        Some(Self {
            semantic_id,
            value_type,
            diagnostic_public_id: flow.public_id().clone(),
            family,
            owner: CheckedProjectItemOwner::Flow { declaration, item },
            character: None,
            value: None,
        })
    }

    pub(crate) fn try_new_retained(
        public_id: PublicId,
        family: DeclarationIdentityFamily,
        owner: ItemId,
        value: Option<TypeKind>,
    ) -> Option<Self> {
        crate::types::EntityKind::from_declaration_identity_family(family)?;
        let character = (family == DeclarationIdentityFamily::Character)
            .then(|| CharacterId::try_new(public_id.as_str()).ok())
            .flatten();
        if family == DeclarationIdentityFamily::Character && character.is_none() {
            return None;
        }
        let value_type = project_item_type(family, value.as_ref())
            .semantic_identity_digest()
            .ok()?;
        let semantic_id = accepted_project_item_semantic_id(
            family,
            value_type,
            &ProjectItemSemanticOwner::Entity(&public_id),
        );
        Some(Self {
            semantic_id,
            value_type,
            diagnostic_public_id: public_id,
            family,
            owner: CheckedProjectItemOwner::Retained(owner),
            character,
            value,
        })
    }

    pub(crate) fn new_external_character(
        declaration: ExternalDeclarationId,
        character: CharacterId,
    ) -> Self {
        let family = DeclarationIdentityFamily::Character;
        let public_id = character.as_public_id();
        let value_type = TypeKind::entity_ref(EntityKind::Character)
            .semantic_identity_digest()
            .expect("the Character entity leaf contains no generic references");
        Self {
            semantic_id: accepted_project_item_semantic_id(
                family,
                value_type,
                &ProjectItemSemanticOwner::Entity(&public_id),
            ),
            value_type,
            diagnostic_public_id: public_id,
            family,
            owner: CheckedProjectItemOwner::External(declaration),
            character: Some(character),
            value: None,
        }
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.diagnostic_public_id
    }

    pub(crate) const fn semantic_id(&self) -> AcceptedProjectItemSemanticId {
        self.semantic_id
    }

    pub const fn value_type(&self) -> SemanticTypeDigest {
        self.value_type
    }

    pub(crate) fn has_valid_semantic_identity(&self) -> bool {
        let expected_owner = match &self.owner {
            CheckedProjectItemOwner::Flow { declaration, .. } => {
                ProjectItemSemanticOwner::Flow(declaration.semantic_digest())
            }
            CheckedProjectItemOwner::Retained(_) | CheckedProjectItemOwner::External(_) => {
                ProjectItemSemanticOwner::Entity(&self.diagnostic_public_id)
            }
        };
        let expected =
            accepted_project_item_semantic_id(self.family, self.value_type, &expected_owner);
        expected.as_bytes() == self.semantic_id().as_bytes()
            && project_item_type(self.family, self.value.as_ref())
                .semantic_identity_digest()
                .is_ok_and(|digest| digest == self.value_type)
    }

    pub const fn family(&self) -> DeclarationIdentityFamily {
        self.family
    }

    pub const fn owner(&self) -> &CheckedProjectItemOwner {
        &self.owner
    }

    pub const fn retained_owner(&self) -> Option<ItemId> {
        match &self.owner {
            CheckedProjectItemOwner::Retained(owner) => Some(*owner),
            CheckedProjectItemOwner::Flow { .. } | CheckedProjectItemOwner::External(_) => None,
        }
    }

    pub const fn flow_owner(&self) -> Option<(&CallableDeclarationKey, ItemId)> {
        match &self.owner {
            CheckedProjectItemOwner::Flow { declaration, item } => Some((declaration, *item)),
            CheckedProjectItemOwner::Retained(_) | CheckedProjectItemOwner::External(_) => None,
        }
    }

    pub const fn external_declaration(&self) -> Option<ExternalDeclarationId> {
        match &self.owner {
            CheckedProjectItemOwner::Retained(_) | CheckedProjectItemOwner::Flow { .. } => None,
            CheckedProjectItemOwner::External(declaration) => Some(*declaration),
        }
    }

    /// Returns the exact canonical Character identity retained at selection.
    pub fn character(&self) -> Option<CharacterId> {
        self.character.clone()
    }

    /// Returns the entity-reference type carried by this checked item.
    ///
    /// # Panics
    ///
    /// Panics only if the internal checked-item family invariant is broken.
    /// Construction admits entity-reference declaration families exclusively.
    pub fn ty(&self) -> TypeKind {
        let ty = project_item_type(self.family, self.value.as_ref());
        debug_assert_eq!(ty.semantic_identity_digest(), Ok(self.value_type));
        ty
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor(&self.ty())
    }
}

enum ProjectItemSemanticOwner<'a> {
    Entity(&'a PublicId),
    Flow(CallableDeclarationDigest),
}

fn project_item_type(family: DeclarationIdentityFamily, value: Option<&TypeKind>) -> TypeKind {
    let kind = crate::types::EntityKind::from_declaration_identity_family(family)
        .expect("checked project items only retain entity-reference families");
    TypeKind::Ref(crate::types::EntityType::new(kind, value.cloned()))
}

fn accepted_project_item_semantic_id(
    family: DeclarationIdentityFamily,
    value_type: SemanticTypeDigest,
    owner: &ProjectItemSemanticOwner<'_>,
) -> AcceptedProjectItemSemanticId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(PROJECT_ITEM_SEMANTIC_DOMAIN);
    match owner {
        ProjectItemSemanticOwner::Entity(public_id) => {
            hasher.update(&[0]);
            hasher.update(&[project_item_family_tag(family)]);
            hasher.update(value_type.as_bytes());
            hasher.update(
                &u64::try_from(public_id.as_str().len())
                    .expect("PublicId length fits canonical u64")
                    .to_le_bytes(),
            );
            hasher.update(public_id.as_str().as_bytes());
        }
        ProjectItemSemanticOwner::Flow(declaration) => {
            hasher.update(&[1]);
            hasher.update(&[project_item_family_tag(family)]);
            hasher.update(value_type.as_bytes());
            hasher.update(declaration.as_bytes());
        }
    }
    AcceptedProjectItemSemanticId(hasher.finalize().into())
}

const fn project_item_family_tag(family: DeclarationIdentityFamily) -> u8 {
    match family {
        DeclarationIdentityFamily::Asset => 0,
        DeclarationIdentityFamily::Character => 1,
        DeclarationIdentityFamily::View => 2,
        DeclarationIdentityFamily::Action => 3,
        DeclarationIdentityFamily::Activity => 4,
        DeclarationIdentityFamily::Signal => 5,
        DeclarationIdentityFamily::Metric => 6,
        DeclarationIdentityFamily::Layer => 7,
        DeclarationIdentityFamily::Flow => 8,
        DeclarationIdentityFamily::Proof => 9,
        DeclarationIdentityFamily::Style => 10,
    }
}

/// Exact source Entry selected by one typed `@entry.*` expression leaf.
///
/// Entry declarations are owned by the checked Entry catalog rather than the
/// retained-declaration symbol family. The canonical public ID and exact HIR
/// item owner keep tooling references generation-bound without reconstructing
/// a retained symbol or reparsing source text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedEntryReference {
    binding: crate::entry::CheckedEntryBindingDigest,
    value_type: SemanticTypeDigest,
    diagnostic_public_id: PublicId,
    lookup_owner: ItemId,
}

impl CheckedEntryReference {
    pub(crate) fn seal(
        prepared: super::PreparedEntryReference,
        value_type: SemanticTypeDigest,
        binding: &crate::entry::CheckedEntryBinding,
    ) -> Option<Self> {
        let (diagnostic_public_id, lookup_owner) = prepared.into_parts();
        let expected_value_type = TypeKind::entity_ref(crate::types::EntityKind::Entry)
            .semantic_identity_digest()
            .ok()?;
        if binding.id().public_id() != &diagnostic_public_id
            || binding.source_item() != lookup_owner
            || value_type != expected_value_type
        {
            return None;
        }
        Some(Self {
            binding: *binding.binding_digest(),
            value_type,
            diagnostic_public_id,
            lookup_owner,
        })
    }

    pub const fn binding(&self) -> &crate::entry::CheckedEntryBindingDigest {
        &self.binding
    }

    pub const fn value_type(&self) -> SemanticTypeDigest {
        self.value_type
    }

    pub const fn diagnostic_public_id(&self) -> &PublicId {
        &self.diagnostic_public_id
    }

    pub const fn lookup_owner(&self) -> ItemId {
        self.lookup_owner
    }

    pub fn ty(&self) -> TypeKind {
        let ty = TypeKind::entity_ref(crate::types::EntityKind::Entry);
        debug_assert_eq!(ty.semantic_identity_digest(), Ok(self.value_type));
        ty
    }
}

/// Exact project nominal selected after alias and projection resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedProjectNominal {
    declaration: ProjectNominalDeclarationId,
    owner: ItemId,
    identity: SemanticTypeDigest,
    arguments: Box<[TypeKind]>,
}

impl CheckedProjectNominal {
    pub fn new(
        declaration: ProjectNominalDeclarationId,
        owner: ItemId,
        identity: SemanticTypeDigest,
        arguments: impl Into<Box<[TypeKind]>>,
    ) -> Self {
        Self {
            declaration,
            owner,
            identity,
            arguments: arguments.into(),
        }
    }

    pub const fn declaration(&self) -> &ProjectNominalDeclarationId {
        &self.declaration
    }

    pub const fn owner(&self) -> ItemId {
        self.owner
    }

    pub const fn identity(&self) -> SemanticTypeDigest {
        self.identity
    }

    pub fn arguments(&self) -> &[TypeKind] {
        &self.arguments
    }

    /// Returns the exact semantic nominal type represented by this checked
    /// declaration/argument row. Runtime instance projection uses this owner
    /// API before applying an enclosing frozen call solution; it must not
    /// reconstruct a nominal from declaration and argument fields.
    pub fn ty(&self) -> TypeKind {
        TypeKind::ProjectNominal(crate::types::ProjectNominalType::new(
            self.declaration.clone(),
            self.arguments.to_vec(),
        ))
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor(&self.ty())
    }

    /// Applies this checked nominal instantiation to a declaration-owned type.
    pub fn instantiate_declaration_type(
        &self,
        declaration: &ProjectNominalDeclaration,
        ty: &TypeKind,
    ) -> Option<TypeKind> {
        if self.declaration() != declaration.id()
            || self.arguments.len() != declaration.type_parameters().len()
        {
            return None;
        }
        let mut substitutions = TypeParameterSubstitutions::default();
        for (parameter, argument) in declaration.type_parameters().iter().zip(self.arguments()) {
            let parameter = TypeKind::generic_parameter(GenericTypeParameterId::new(
                GenericParameterOwnerId::Nominal(declaration.id().clone()),
                parameter.ordinal(),
            ));
            if !substitutions.observe(&parameter, argument) {
                return None;
            }
        }
        Some(substitutions.apply(ty))
    }
}

/// Checked meaning of one path or entity-reference expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedValueResolution {
    Local(LocalId),
    /// Runtime-owned line context available only inside an attached plan.
    LineContext,
    /// Standard Character-owned stage API projected from a typed receiver.
    CharacterField {
        receiver: Box<CheckedValueResolution>,
        character: CharacterId,
        field: CharacterField,
    },
    ProjectCallable(CheckedProjectCallable),
    ProjectItem(CheckedProjectItem),
    Entry(CheckedEntryReference),
    Registered(RegisteredSemanticValueId),
    Constant(HirLiteral),
}

impl CheckedValueResolution {
    /// Exact Character identity retained by a checked Character value.
    pub fn character(&self) -> Option<CharacterId> {
        match self {
            Self::ProjectItem(item) => item.character(),
            Self::Local(_)
            | Self::LineContext
            | Self::CharacterField { .. }
            | Self::ProjectCallable(_)
            | Self::Entry(_)
            | Self::Registered(_)
            | Self::Constant(_) => None,
        }
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::CharacterField { receiver, .. } => receiver.visit_types(visitor),
            Self::ProjectItem(item) => item.visit_types(visitor),
            Self::Local(_)
            | Self::LineContext
            | Self::ProjectCallable(_)
            | Self::Entry(_)
            | Self::Registered(_)
            | Self::Constant(_) => Ok(()),
        }
    }
}

/// Checked projection selected for one member expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMethodSelection {
    callable: CheckedCallableJoinDigest,
    receiver_type: SemanticTypeDigest,
    receiver_mode: CallableReceiverMode,
}

impl CheckedMethodSelection {
    pub(crate) fn try_from_join(join: &CheckedCallableJoin) -> Option<Self> {
        let receiver_mode = join.receiver().clone();
        let receiver = match &receiver_mode {
            CallableReceiverMode::None => return None,
            CallableReceiverMode::Value { receiver }
            | CallableReceiverMode::Type { receiver }
            | CallableReceiverMode::Extension { receiver, .. } => receiver,
        };
        Some(Self {
            callable: join.semantic_digest().ok()?,
            receiver_type: receiver.semantic_identity_digest().ok()?,
            receiver_mode,
        })
    }

    pub const fn callable(&self) -> CheckedCallableJoinDigest {
        self.callable
    }

    pub const fn receiver_type(&self) -> SemanticTypeDigest {
        self.receiver_type
    }

    pub const fn receiver_mode(&self) -> &CallableReceiverMode {
        &self.receiver_mode
    }

    pub(crate) fn has_valid_receiver_identity(&self) -> bool {
        let receiver = match &self.receiver_mode {
            CallableReceiverMode::None => return false,
            CallableReceiverMode::Value { receiver }
            | CallableReceiverMode::Type { receiver }
            | CallableReceiverMode::Extension { receiver, .. } => receiver,
        };
        receiver
            .semantic_identity_digest()
            .is_ok_and(|digest| digest == self.receiver_type)
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match &self.receiver_mode {
            CallableReceiverMode::None => Ok(()),
            CallableReceiverMode::Value { receiver }
            | CallableReceiverMode::Type { receiver }
            | CallableReceiverMode::Extension { receiver, .. } => visitor(receiver),
        }
    }
}

impl CheckedSelectResolution {
    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Method(method) => method.visit_types(visitor),
            Self::DialogueView { .. }
            | Self::AgentField { .. }
            | Self::ProgressField { .. }
            | Self::Field(_) => Ok(()),
        }
    }
}

/// Checked projection selected for one member expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedSelectResolution {
    /// Exact Method selected through the once-composed enclosing call join.
    Method(CheckedMethodSelection),
    /// Runtime-supplied field of a nominal record carrying the semantic
    /// `#[dialogue_view]` role. The projection identity is selected by the
    /// environment registry, never reconstructed from its field spelling by
    /// compiler or runtime consumers.
    DialogueView {
        projection: crate::dialogue_view::DialogueProjectionCoordinate,
        field: CheckedFieldSelection,
    },
    /// Closed Agent protocol record coordinate selected during type checking.
    AgentField {
        field: RuntimeAgentField,
    },
    /// Field owned by the standard `Progress` value family.
    ProgressField {
        field: crate::types::ProgressField,
    },
    Field(CheckedFieldSelection),
}

#[path = "model/variant_owner.rs"]
mod variant_owner;
pub use variant_owner::{
    CheckedVariantCase, CheckedVariantOwner, CheckedVariantOwnerError, CheckedVariantOwnerKind,
    CheckedVariantResolution,
};
pub(crate) use variant_owner::{PreparedVariantCaseSeed, PreparedVariantOwnerSeed};

#[cfg(test)]
#[path = "model/variant_tests.rs"]
mod variant_tests;

#[path = "model/stage_look.rs"]
mod stage_look;
pub use stage_look::CheckedStageLook;
#[path = "model/record.rs"]
mod record;
pub use record::{
    CheckedExpressionRecordField, CheckedFieldSelection, CheckedRecordBindingSource,
    CheckedRecordExpressionSource, CheckedRecordPattern, CheckedRecordPatternField,
    CheckedRecordPatternOwner, CheckedRecordPatternRest, CheckedRecordPatternSource,
    CheckedRecordPatternSourceRef, CheckedRecordValueSource,
};
#[path = "model/capture.rs"]
mod capture;
pub(crate) use capture::CheckedImplicitCallableIdentityEvidence;
pub use capture::{
    CheckedCapture, CheckedCaptureAuthorityViolation, CheckedClosure, CheckedImplicitCallable,
    CheckedImplicitCallableBody, CheckedImplicitCallableIdentity, CheckedImplicitCapture,
    CheckedImplicitCaptureOccurrence, CheckedImplicitParameter, CheckedImplicitParameterOccurrence,
};

#[path = "model/pipe.rs"]
mod pipe;
pub use pipe::{
    CheckedPipe, CheckedPipeBindingIdentity, CheckedPipeLeft, CheckedPipeLeftOccurrence,
};

#[path = "model/dialogue_line_plan.rs"]
mod dialogue_line_plan;
pub use dialogue_line_plan::{
    CheckedDialogueEffectCapture, CheckedDialogueEffectPlan, CheckedDialogueEffectSite,
    CheckedDialogueEffectSiteOrdinal, CheckedDialogueEffectTrigger,
};

/// Semantic payload needed in addition to the final-HIR expression family.
///
/// The original resolution remains the authority for source-owned identity
/// (including a call site for an RGB proxy scalar or a value identity for a
/// PublicId).  The scalar is an additional closed, checked payload rather than
/// a replacement or source-spelling reconstruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCompileTimeScalarExpression {
    value: CheckedCompileTimeScalar,
    original: Box<CheckedExpressionResolution>,
}

impl CheckedCompileTimeScalarExpression {
    pub(crate) fn new(
        value: CheckedCompileTimeScalar,
        original: CheckedExpressionResolution,
    ) -> Self {
        Self {
            value,
            original: Box::new(original),
        }
    }

    pub const fn value(&self) -> &CheckedCompileTimeScalar {
        &self.value
    }

    pub const fn original(&self) -> &CheckedExpressionResolution {
        &self.original
    }
}

/// Complete checked producer/execution authority for one attached-content
/// application.  A content value, a content-result call, and an emission call
/// have different runtime ownership and therefore remain distinct closed
/// variants rather than being inferred from an optional call fact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedContentApplication {
    Value {
        id: CheckedContentApplicationId,
        source: crate::checked_rich_text::CheckedContentValueSource,
    },
    ContentResultCall {
        id: CheckedContentApplicationId,
        application: CheckedCallApplicationDigest,
    },
    EmissionCall {
        id: CheckedContentApplicationId,
        application: CheckedCallApplicationDigest,
        edges: CheckedContentApplicationEdges,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedContentApplicationEdges {
    kind: CheckedContentApplicationEdgeKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CheckedContentApplicationEdgeKind {
    Ordinary,
    Fx(SealedContentFxEdgePlan),
}

impl CheckedContentApplicationEdges {
    pub(crate) const fn ordinary() -> Self {
        Self {
            kind: CheckedContentApplicationEdgeKind::Ordinary,
        }
    }

    pub(crate) const fn fx(plan: SealedContentFxEdgePlan) -> Self {
        Self {
            kind: CheckedContentApplicationEdgeKind::Fx(plan),
        }
    }

    pub const fn is_ordinary(&self) -> bool {
        matches!(&self.kind, CheckedContentApplicationEdgeKind::Ordinary)
    }

    pub const fn is_fx(&self) -> bool {
        matches!(&self.kind, CheckedContentApplicationEdgeKind::Fx(_))
    }

    pub(crate) const fn fx_plan(&self) -> Option<&SealedContentFxEdgePlan> {
        match &self.kind {
            CheckedContentApplicationEdgeKind::Ordinary => None,
            CheckedContentApplicationEdgeKind::Fx(plan) => Some(plan),
        }
    }
}

impl CheckedContentApplication {
    pub(crate) const fn value(
        id: CheckedContentApplicationId,
        source: crate::checked_rich_text::CheckedContentValueSource,
    ) -> Self {
        Self::Value { id, source }
    }

    pub(crate) const fn content_result_call(
        id: CheckedContentApplicationId,
        application: CheckedCallApplicationDigest,
    ) -> Self {
        Self::ContentResultCall { id, application }
    }

    pub(crate) fn emission_call(
        id: CheckedContentApplicationId,
        application: CheckedCallApplicationDigest,
        edges: CheckedContentApplicationEdges,
    ) -> Result<Self, ()> {
        if let Some(plan) = edges.fx_plan() {
            let StableCheckedValueCoordinate::Expression(path) = plan.outer().site().coordinate()
            else {
                return Err(());
            };
            if path != id.path() {
                return Err(());
            }
        }
        Ok(Self::EmissionCall {
            id,
            application,
            edges,
        })
    }

    pub const fn id(&self) -> &CheckedContentApplicationId {
        match self {
            Self::Value { id, .. }
            | Self::ContentResultCall { id, .. }
            | Self::EmissionCall { id, .. } => id,
        }
    }

    pub const fn value_source(
        &self,
    ) -> Option<&crate::checked_rich_text::CheckedContentValueSource> {
        match self {
            Self::Value { source, .. } => Some(source),
            Self::ContentResultCall { .. } | Self::EmissionCall { .. } => None,
        }
    }

    pub const fn application(&self) -> Option<CheckedCallApplicationDigest> {
        match self {
            Self::Value { .. } => None,
            Self::ContentResultCall { application, .. }
            | Self::EmissionCall { application, .. } => Some(*application),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedExpressionResolution {
    Structural,
    Literal(HirLiteral),
    Value(CheckedValueResolution),
    Select(CheckedSelectResolution),
    Nominal(CheckedProjectNominal),
    Variant(CheckedVariantResolution),
    /// Exact member of a presentation-owned closed enum domain. These
    /// domains are not project nominal variants and therefore do not enter
    /// the general variant-payload owner algebra.
    CompileTimeEnum(ClosedEnumValueId),
    /// Exact registered manifest look selected under the Stage API's typed parameter.
    StageLook(CheckedStageLook),
    /// Canonical effect identity selected from an authored effect-clause path.
    Effect(crate::effects::EffectId),
    Call,
    /// Exact outcome and continuation contract owned by one Await expression.
    Await(CheckedAwait),
    /// Exact project Flow targets selected for compact Choice `goto` arms.
    Choice(CheckedChoice),
    /// Exact carrier and nearest lexical propagation boundary for prefix Try.
    Try(CheckedTry),
    /// One implicit callable introduced by partial-application placeholders.
    ImplicitCallable(Box<CheckedImplicitCallable>),
    /// One explicit closure with the exact accepted HIR capture rows retained
    /// by its terminal checked producer fact.
    Closure(CheckedClosure),
    /// One placeholder bound by its checked implicit callable owner.
    ImplicitParameter(CheckedImplicitParameter),
    /// One once-evaluated pipeline and its checked pipe-left uses.
    Pipe(CheckedPipe),
    /// One `^` placeholder bound by its checked pipeline owner.
    PipeLeft(CheckedPipeLeft),
    /// A call whose execution contract belongs to the retained View program,
    /// rather than to the ordinary callable catalog.
    ViewCall(CheckedViewCall),
    /// One `.fx(value)` modifier whose producer, definition, parameter
    /// decisions, and closed/reactive bindings were sealed by final sema.
    ViewFxApplication(Box<CheckedViewFxApplication>),
    /// A property value admitted by the final-HIR Style checker.
    StyleValue(arcweft_view::style::ViewSpecifiedValue),
    /// A compile-time callable leaf selected by a language-owned checker.
    CompileTimeCallee(CheckedCompileTimeCallee),
    /// A compile-time value describing one exact semantic type.
    TypeValue(CheckedTypeValue),
    /// Exact scalar value retained together with its original checked
    /// expression resolution. The original resolution carries source-owned
    /// call/value identity; this wrapper adds the reduced scalar payload.
    CompileTimeScalar(CheckedCompileTimeScalarExpression),
    /// Exact accepted dialogue-line target selected for an entity-reference
    /// leaf under the `DialogueLine` expected family.
    DialogueLineReference(DialogueLineId),
    /// Immediate `id` metadata owned by one accepted dialogue application.
    DialogueLineCoordinate(DialogueLineId),
    /// Immediate `text_key` metadata owned by one accepted dialogue application.
    DialogueTextKeyCoordinate(DialogueTextKey),
    CharacterDialogueFactory(CheckedCharacterDialogueFactory),
    CharacterDialogueReconfigure(CheckedCharacterDialogueReconfigure),
    DialogueApplication {
        target: CheckedCharacterDialogueTarget,
        application_patch: Option<CheckedCharacterDialoguePatch>,
        rich_text: Box<CheckedRichTextReport>,
        line_result: TypeKind,
    },
    /// Stable identity join for an attached-content producer. The complete
    /// producer/body carrier is owned by its checked `ContentInsert` token.
    ContentApplication(Box<CheckedContentApplication>),
    PostfixBracket(PostfixBracketResolution),
}

impl CheckedExpressionResolution {
    /// Returns the unique prepared/final call site required by this checked
    /// semantic resolution. Raw HIR Call syntax is deliberately not enough:
    /// structural Call-shaped operands remain outside the callable graph.
    pub(crate) const fn checked_call_site(
        &self,
        owner: ExprId,
    ) -> Option<crate::callable::CheckedCallSite> {
        match self {
            Self::Call
            | Self::CharacterDialogueFactory(_)
            | Self::CharacterDialogueReconfigure(_)
            | Self::ViewFxApplication(_) => Some(crate::callable::CheckedCallSite::HirCall(owner)),
            Self::DialogueApplication { .. } => Some(
                crate::callable::CheckedCallSite::AttachedContentApplication {
                    expression: owner,
                    family: crate::callable::CheckedAttachedContentApplicationFamily::DialogueLine,
                },
            ),
            Self::ContentApplication(application) => match &**application {
                CheckedContentApplication::Value { .. } => None,
                CheckedContentApplication::ContentResultCall { .. }
                | CheckedContentApplication::EmissionCall { .. } => Some(
                    crate::callable::CheckedCallSite::AttachedContentApplication {
                        expression: owner,
                        family:
                            crate::callable::CheckedAttachedContentApplicationFamily::ContentCall,
                    },
                ),
            },
            Self::Structural
            | Self::Literal(_)
            | Self::Value(_)
            | Self::Select(_)
            | Self::Nominal(_)
            | Self::Variant(_)
            | Self::CompileTimeEnum(_)
            | Self::StageLook(_)
            | Self::Effect(_)
            | Self::Await(_)
            | Self::Choice(_)
            | Self::Try(_)
            | Self::Closure(_)
            | Self::ImplicitParameter(_)
            | Self::Pipe(_)
            | Self::PipeLeft(_)
            | Self::ViewCall(_)
            | Self::StyleValue(_)
            | Self::CompileTimeCallee(_)
            | Self::TypeValue(_)
            | Self::DialogueLineReference(_)
            | Self::DialogueLineCoordinate(_)
            | Self::DialogueTextKeyCoordinate(_)
            | Self::PostfixBracket(_) => None,
            Self::ImplicitCallable(callable) => match callable.body() {
                CheckedImplicitCallableBody::Plain(resolution) => {
                    resolution.checked_call_site(owner)
                }
                CheckedImplicitCallableBody::Try(_) | CheckedImplicitCallableBody::Pipe(_) => None,
            },
            Self::CompileTimeScalar(value) => value.original().checked_call_site(owner),
        }
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Value(value) => value.visit_types(visitor),
            Self::Select(selection) => selection.visit_types(visitor),
            Self::Nominal(nominal) => nominal.visit_types(visitor),
            Self::Variant(variant) => variant.visit_types(visitor),
            Self::CompileTimeEnum(_) => Ok(()),
            Self::Choice(choice) => {
                for goto in choice.gotos() {
                    goto.target().visit_types(visitor)?;
                }
                Ok(())
            }
            Self::Try(checked) => checked.visit_types(visitor),
            Self::ImplicitCallable(callable) => callable.visit_types(visitor),
            Self::CharacterDialogueFactory(factory) => factory.visit_types(visitor),
            Self::CharacterDialogueReconfigure(reconfigure) => reconfigure.visit_types(visitor),
            Self::DialogueApplication {
                target,
                application_patch,
                rich_text: _,
                line_result,
            } => {
                target.visit_types(visitor)?;
                if let Some(patch) = application_patch {
                    patch.visit_types(visitor)?;
                }
                visitor(line_result)
            }
            Self::ContentApplication(_) => Ok(()),
            Self::TypeValue(value) => value.visit_types(visitor),
            Self::CompileTimeScalar(value) => value.original().visit_types(visitor),
            Self::Structural
            | Self::Literal(_)
            | Self::StageLook(_)
            | Self::Effect(_)
            | Self::Call
            | Self::Await(_)
            | Self::Closure(_)
            | Self::ImplicitParameter(_)
            | Self::Pipe(_)
            | Self::PipeLeft(_)
            | Self::ViewCall(_)
            | Self::ViewFxApplication(_)
            | Self::StyleValue(_)
            | Self::CompileTimeCallee(_)
            | Self::DialogueLineReference(_)
            | Self::DialogueLineCoordinate(_)
            | Self::DialogueTextKeyCoordinate(_)
            | Self::PostfixBracket(_) => Ok(()),
        }
    }
}

/// One compact Choice arm whose `goto` target was resolved against the exact
/// accepted project-symbol generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedChoiceGoto {
    arm: u32,
    target: CheckedProjectItem,
}

impl CheckedChoiceGoto {
    pub const fn new(arm: u32, target: CheckedProjectItem) -> Self {
        Self { arm, target }
    }

    pub const fn arm(&self) -> u32 {
        self.arm
    }

    pub const fn target(&self) -> &CheckedProjectItem {
        &self.target
    }
}

/// Checked semantic additions to one final-HIR Choice expression.
///
/// Candidate structure, labels, conditions, and output expressions remain
/// owned by final HIR. Only non-expression `goto` targets need an additional
/// semantic selection fact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedChoice {
    public_id: Option<PublicId>,
    option_ids: Box<[PublicId]>,
    gotos: Box<[CheckedChoiceGoto]>,
}

impl CheckedChoice {
    pub fn new(
        public_id: Option<PublicId>,
        option_ids: impl Into<Box<[PublicId]>>,
        gotos: impl Into<Box<[CheckedChoiceGoto]>>,
    ) -> Self {
        Self {
            public_id,
            option_ids: option_ids.into(),
            gotos: gotos.into(),
        }
    }

    pub const fn public_id(&self) -> Option<&PublicId> {
        self.public_id.as_ref()
    }

    pub fn option_ids(&self) -> &[PublicId] {
        &self.option_ids
    }

    pub fn gotos(&self) -> &[CheckedChoiceGoto] {
        &self.gotos
    }
}

/// Closed carrier consumed by one prefix Try expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedTryCarrier {
    Result {
        success: TypeKind,
        residual: Box<TypeKind>,
    },
    Option {
        success: TypeKind,
    },
}

impl CheckedTryCarrier {
    pub const fn success(&self) -> &TypeKind {
        match self {
            Self::Result { success, .. } | Self::Option { success } => success,
        }
    }

    pub fn residual(&self) -> Option<&TypeKind> {
        match self {
            Self::Result { residual, .. } => Some(residual.as_ref()),
            Self::Option { .. } => None,
        }
    }

    pub(crate) fn from_operand_type(operand: &TypeKind) -> Option<Self> {
        match operand {
            TypeKind::Result { ok, error } => Some(Self::Result {
                success: ok.as_ref().clone(),
                residual: error.clone(),
            }),
            TypeKind::Option(value) => Some(Self::Option {
                success: value.as_ref().clone(),
            }),
            _ => None,
        }
    }

    pub(crate) fn as_type(&self) -> TypeKind {
        match self {
            Self::Result { success, residual } => TypeKind::Result {
                ok: Box::new(success.clone()),
                error: residual.clone(),
            },
            Self::Option { success } => TypeKind::Option(Box::new(success.clone())),
        }
    }

    pub(crate) fn accepts_boundary_type(&self, boundary: &TypeKind) -> bool {
        match (self, boundary) {
            (Self::Result { residual, .. }, TypeKind::Result { error, .. }) => {
                error.accepts(residual)
            }
            (Self::Option { .. }, TypeKind::Option(_)) => true,
            _ => false,
        }
    }

    pub(crate) const fn has_boundary_family(&self, boundary: &TypeKind) -> bool {
        matches!(
            (self, boundary),
            (Self::Result { .. }, TypeKind::Result { .. })
                | (Self::Option { .. }, TypeKind::Option(_))
        )
    }

    pub(crate) fn is_infallible(&self) -> bool {
        matches!(
            self,
            Self::Result { residual, .. } if matches!(residual.as_ref(), TypeKind::Never)
        )
    }

    pub(crate) fn semantic_type_digest(
        &self,
    ) -> Result<SemanticTypeDigest, crate::types::GenericScopeError> {
        self.as_type().semantic_identity_digest()
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Result { success, residual } => {
                visitor(success)?;
                visitor(residual)
            }
            Self::Option { success } => visitor(success),
        }
    }
}

/// Exact checked operand child of one prefix Try expression.
///
/// The HIR owner is retained only as generation-local lookup evidence. The
/// accepted coordinate and complete checked type are the semantic payload used
/// by downstream projections. The owner-bound seal issues this row from the
/// unique structural `Operand` edge; consumers must never rediscover it from
/// raw HIR.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CheckedTryOperand {
    lookup_owner: ExprId,
    coordinate: CheckedSemanticPath,
    value_type: TypeKind,
}

impl CheckedTryOperand {
    pub(crate) fn from_evidence(
        evidence: CheckedExpressionCoordinateEvidence,
        value_type: TypeKind,
    ) -> Self {
        Self {
            lookup_owner: evidence.owner(),
            coordinate: evidence.into_coordinate(),
            value_type,
        }
    }

    pub(in crate::final_analysis) const fn lookup_owner(&self) -> ExprId {
        self.lookup_owner
    }

    pub const fn coordinate(&self) -> &CheckedSemanticPath {
        &self.coordinate
    }

    pub const fn value_type(&self) -> &TypeKind {
        &self.value_type
    }
}

/// Typed failure while sealing or validating one Try operand projection.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CheckedTryOperandAuthorityViolation {
    #[error("checked Try {owner:?} has no unique Operand child edge")]
    MissingOperand { owner: ExprId },
    #[error("checked Try {owner:?} has duplicate Operand child edges")]
    DuplicateOperand { owner: ExprId },
    #[error("checked Try {owner:?} operand child differs: expected {expected:?}, found {actual:?}")]
    OperandChildMismatch {
        owner: ExprId,
        expected: ExprId,
        actual: ExprId,
    },
    #[error("checked Try {owner:?} operand type differs: expected {expected:?}, found {actual:?}")]
    OperandTypeMismatch {
        owner: ExprId,
        expected: Box<TypeKind>,
        actual: Box<TypeKind>,
    },
    #[error("checked Try {owner:?} operand coordinate differs from the accepted child edge")]
    OperandCoordinateMismatch {
        owner: ExprId,
        expected: Box<CheckedSemanticPath>,
        actual: Box<CheckedSemanticPath>,
    },
    #[error("checked Try {owner:?} propagation boundary does not accept its operand carrier")]
    BoundaryMismatch {
        owner: ExprId,
        carrier: Box<TypeKind>,
        boundary: Option<Box<TypeKind>>,
    },
}

/// Generation-bound expression evidence paired with its accepted semantic
/// coordinate. The raw owner is retained only for runtime/CPS validation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedTryExpressionBoundary {
    lookup_owner: ExprId,
    coordinate: CheckedSemanticPath,
}

impl CheckedTryExpressionBoundary {
    pub(crate) fn from_evidence(evidence: CheckedExpressionCoordinateEvidence) -> Self {
        Self {
            lookup_owner: evidence.owner(),
            coordinate: evidence.into_coordinate(),
        }
    }

    pub const fn lookup_owner(&self) -> ExprId {
        self.lookup_owner
    }

    pub const fn coordinate(&self) -> &CheckedSemanticPath {
        &self.coordinate
    }
}

/// Function-site Try boundary. Explicit closure sites carry only their
/// accepted expression boundary; implicit sites additionally join the
/// callable identity issued by the owner-bound seal.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedTryFunctionSite {
    Explicit(CheckedTryExpressionBoundary),
    Implicit {
        site: CheckedTryExpressionBoundary,
        callable: CheckedImplicitCallableIdentity,
    },
}

impl CheckedTryFunctionSite {
    pub const fn site(&self) -> &CheckedTryExpressionBoundary {
        match self {
            Self::Explicit(site) => site,
            Self::Implicit { site, .. } => site,
        }
    }

    pub const fn callable(&self) -> Option<CheckedImplicitCallableIdentity> {
        match self {
            Self::Explicit(_) => None,
            Self::Implicit { callable, .. } => Some(*callable),
        }
    }
}

/// Accepted callable declaration receiving one Try residual. The declaration
/// key remains the typed catalog lookup; the accepted semantic ID is the
/// stable root identity consumed by transcript/runtime authorities.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedTryCallableBoundary {
    declaration: CallableDeclarationKey,
    accepted: AcceptedDeclarationSemanticId,
}

impl CheckedTryCallableBoundary {
    pub(crate) fn new(
        declaration: CallableDeclarationKey,
        accepted: AcceptedDeclarationSemanticId,
    ) -> Self {
        Self {
            declaration,
            accepted,
        }
    }

    pub const fn declaration(&self) -> &CallableDeclarationKey {
        &self.declaration
    }

    pub const fn accepted(&self) -> AcceptedDeclarationSemanticId {
        self.accepted
    }
}

/// Nearest typed lexical owner that receives one Try residual.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedTryBoundaryOwner {
    Infallible,
    CarrierBlock(CheckedTryExpressionBoundary),
    FunctionSite(CheckedTryFunctionSite),
    Callable(CheckedTryCallableBoundary),
}

impl CheckedTryBoundaryOwner {
    pub const fn expression_boundary(&self) -> Option<&CheckedTryExpressionBoundary> {
        match self {
            Self::CarrierBlock(boundary) => Some(boundary),
            Self::FunctionSite(site) => Some(site.site()),
            Self::Infallible | Self::Callable(_) => None,
        }
    }
}

/// Complete boundary payload for one prefix Try expression.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CheckedTryBoundary {
    boundary_type: TypeKind,
    owner: CheckedTryBoundaryOwner,
}

impl CheckedTryBoundary {
    pub(crate) fn new(boundary_type: TypeKind, owner: CheckedTryBoundaryOwner) -> Self {
        Self {
            boundary_type,
            owner,
        }
    }

    pub const fn boundary_type(&self) -> &TypeKind {
        &self.boundary_type
    }

    pub const fn owner(&self) -> &CheckedTryBoundaryOwner {
        &self.owner
    }
}

/// Complete checked meaning of one prefix Try expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedTry {
    operand: CheckedTryOperand,
    carrier: CheckedTryCarrier,
    boundary: CheckedTryBoundary,
}

impl CheckedTry {
    pub(crate) const fn new(
        operand: CheckedTryOperand,
        carrier: CheckedTryCarrier,
        boundary: CheckedTryBoundary,
    ) -> Self {
        Self {
            operand,
            carrier,
            boundary,
        }
    }

    pub const fn operand(&self) -> &CheckedTryOperand {
        &self.operand
    }

    pub const fn carrier(&self) -> &CheckedTryCarrier {
        &self.carrier
    }

    pub const fn boundary(&self) -> &CheckedTryBoundary {
        &self.boundary
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor(self.operand.value_type())?;
        self.carrier.visit_types(visitor)?;
        visitor(self.boundary.boundary_type())
    }
}

/// One typed observer for an Await's pending publications.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedAwaitPendingObserver {
    pattern: PatternId,
}

impl CheckedAwaitPendingObserver {
    pub const fn new(pattern: PatternId) -> Self {
        Self { pattern }
    }

    pub const fn pattern(&self) -> PatternId {
        self.pattern
    }
}

/// Typed semantics of one Await expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedAwait {
    operand: ExprId,
    observers: Box<[CheckedAwaitPendingObserver]>,
}

impl CheckedAwait {
    pub fn new(operand: ExprId, observers: impl Into<Box<[CheckedAwaitPendingObserver]>>) -> Self {
        Self {
            operand,
            observers: observers.into(),
        }
    }

    pub const fn operand(&self) -> ExprId {
        self.operand
    }

    pub fn observers(&self) -> &[CheckedAwaitPendingObserver] {
        &self.observers
    }
}

/// Typed runtime-value target selected for `CharacterDialogue` construction or use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedCharacterDialogueTarget {
    Character {
        expression: ExprId,
        item: Option<Box<CheckedProjectItem>>,
        character: CharacterDialogueCharacterType,
    },
    Dialogue {
        expression: ExprId,
        ty: CharacterDialogueType,
    },
}

impl CheckedCharacterDialogueTarget {
    pub const fn expression(&self) -> ExprId {
        match self {
            Self::Character { expression, .. } | Self::Dialogue { expression, .. } => *expression,
        }
    }

    pub const fn character(&self) -> &CharacterDialogueCharacterType {
        match self {
            Self::Character { character, .. } => character,
            Self::Dialogue { ty, .. } => ty.character(),
        }
    }

    pub fn result_type(&self) -> CharacterDialogueType {
        CharacterDialogueType::new(self.character().clone())
    }

    /// Exact semantic type of the application target. Structural Dialogue
    /// operand sealing uses this owner projection instead of reconstructing a
    /// type from the target's variant at each consumer.
    pub fn ty(&self) -> TypeKind {
        match self {
            Self::Character { .. } => TypeKind::entity_ref(EntityKind::Character),
            Self::Dialogue { ty, .. } => TypeKind::CharacterDialogue(ty.clone()),
        }
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        if let Self::Character {
            item: Some(item), ..
        } = self
        {
            item.visit_types(visitor)?;
        }
        visitor(&self.ty())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCharacterDialoguePatch {
    context: CharacterDialoguePatchContext,
    fields: Box<[CheckedCharacterDialoguePatchField]>,
    source: SourceSpan,
}

impl CheckedCharacterDialoguePatch {
    pub fn new(
        context: CharacterDialoguePatchContext,
        fields: impl Into<Box<[CheckedCharacterDialoguePatchField]>>,
        source: SourceSpan,
    ) -> Self {
        Self {
            context,
            fields: fields.into(),
            source,
        }
    }

    pub const fn context(&self) -> CharacterDialoguePatchContext {
        self.context
    }

    pub const fn fields(&self) -> &[CheckedCharacterDialoguePatchField] {
        &self.fields
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        for field in self.fields() {
            match field.operation() {
                CheckedPatchOperation::Set { ty, .. } => visitor(ty)?,
                CheckedPatchOperation::Clear => {}
            }
        }
        Ok(())
    }
}

/// Compile-time operation carried by one source-ordered patch field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedPatchOperation {
    Set { value: ExprId, ty: TypeKind },
    Clear,
}

/// One source-ordered, typed `CharacterDialogue` patch contribution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCharacterDialoguePatchField {
    coordinate: CharacterDialogueFieldCoordinate,
    operation: CheckedPatchOperation,
    source: SourceSpan,
}

impl CheckedCharacterDialoguePatchField {
    pub const fn new(
        coordinate: CharacterDialogueFieldCoordinate,
        operation: CheckedPatchOperation,
        source: SourceSpan,
    ) -> Self {
        Self {
            coordinate,
            operation,
            source,
        }
    }

    pub const fn coordinate(&self) -> &CharacterDialogueFieldCoordinate {
        &self.coordinate
    }

    pub const fn operation(&self) -> &CheckedPatchOperation {
        &self.operation
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCharacterDialogueFactory {
    target: CheckedCharacterDialogueTarget,
    patch: CheckedCharacterDialoguePatch,
}

impl CheckedCharacterDialogueFactory {
    pub const fn new(
        target: CheckedCharacterDialogueTarget,
        patch: CheckedCharacterDialoguePatch,
    ) -> Self {
        Self { target, patch }
    }

    pub const fn target(&self) -> &CheckedCharacterDialogueTarget {
        &self.target
    }

    pub const fn patch(&self) -> &CheckedCharacterDialoguePatch {
        &self.patch
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        self.target.visit_types(visitor)?;
        self.patch.visit_types(visitor)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCharacterDialogueReconfigure {
    target: CheckedCharacterDialogueTarget,
    patch: CheckedCharacterDialoguePatch,
}

impl CheckedCharacterDialogueReconfigure {
    pub const fn new(
        target: CheckedCharacterDialogueTarget,
        patch: CheckedCharacterDialoguePatch,
    ) -> Self {
        Self { target, patch }
    }

    pub const fn target(&self) -> &CheckedCharacterDialogueTarget {
        &self.target
    }

    pub const fn patch(&self) -> &CheckedCharacterDialoguePatch {
        &self.patch
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        self.target.visit_types(visitor)?;
        self.patch.visit_types(visitor)
    }
}

/// Closed semantic classification for a call executed by the View evaluator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedViewCall {
    Element(arcweft_view::ViewElementKind),
    Text,
    RichText,
}

/// Exact semantic identity of one compile-time callable leaf.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedCompileTimeCallee {
    View(crate::types::ViewCallableId),
    Style(crate::types::StyleCallableId),
}

/// Fixed-dimensional vector value. The component count is checked when the
/// value is constructed; no `Vec2`/`Vec3`/`Vec4` nominal-name fallback exists.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedCompileTimeVector {
    dimensions: u8,
    components: Box<[Milli]>,
}

impl CheckedCompileTimeVector {
    pub fn new(dimensions: u8, components: Vec<Milli>) -> Option<Self> {
        if !(2..=4).contains(&dimensions) || components.len() != usize::from(dimensions) {
            return None;
        }
        Some(Self {
            dimensions,
            components: components.into_boxed_slice(),
        })
    }

    pub const fn dimensions(&self) -> u8 {
        self.dimensions
    }

    pub const fn components(&self) -> &[Milli] {
        &self.components
    }
}

/// Shared checked compile-time value algebra.
///
/// This is the authority used by closed Content and Fx source parameters and
/// by text-proxy field evaluation. A complete Fx application is deliberately
/// not a scalar value; its definition, bindings, and site are retained by the
/// context-sealed `CheckedContentFxApplication` or `CheckedViewFxApplication`
/// at the owning Content/View boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedCompileTimeValue {
    Scalar(CheckedCompileTimeScalar),
    Enum(ClosedEnumValueId),
    Vector(CheckedCompileTimeVector),
    Seed32(u32),
}

impl arcweft_rich_text_schema::RichTextPredicateValueView for CheckedCompileTimeValue {
    fn predicate_bool(&self) -> Option<bool> {
        match self {
            Self::Scalar(CheckedCompileTimeScalar::Bool(value)) => Some(*value),
            _ => None,
        }
    }

    fn predicate_enum_variant(&self) -> Option<u16> {
        match self {
            Self::Enum(value) => Some(value.variant()),
            _ => None,
        }
    }
}

impl CheckedCompileTimeValue {
    pub const fn scalar(value: CheckedCompileTimeScalar) -> Self {
        Self::Scalar(value)
    }

    pub const fn semantic_tag(&self) -> u8 {
        match self {
            Self::Scalar(CheckedCompileTimeScalar::Bool(_)) => 0,
            Self::Scalar(CheckedCompileTimeScalar::Int(_)) => 1,
            Self::Scalar(CheckedCompileTimeScalar::Milli(_)) => 2,
            Self::Scalar(CheckedCompileTimeScalar::Ratio(_)) => 3,
            Self::Scalar(CheckedCompileTimeScalar::Length(_)) => 4,
            Self::Scalar(CheckedCompileTimeScalar::Angle(_)) => 5,
            Self::Scalar(CheckedCompileTimeScalar::Duration(_)) => 6,
            Self::Enum(_) => 7,
            Self::Scalar(CheckedCompileTimeScalar::PublicId(_)) => 8,
            Self::Scalar(CheckedCompileTimeScalar::Text(_)) => 9,
            Self::Scalar(CheckedCompileTimeScalar::Color(_)) => 10,
            Self::Vector(_) => 11,
            Self::Seed32(_) => 12,
            Self::Scalar(CheckedCompileTimeScalar::Enum(_)) => 13,
        }
    }
}

/// Private payload of an exact project-nominal type value.
///
/// The semantic-definition digest is issued by the project nominal catalog at
/// the C2 seal. Keeping it in the payload makes a digest-free `CheckedTypeValue`
/// unrepresentable after publication.
#[derive(Clone, Debug, Eq, PartialEq)]
struct CheckedProjectNominalTypeValue {
    nominal: CheckedProjectNominal,
    semantic_definition_digest: ProjectNominalSemanticDigest,
}

/// Exact type value selected by compile-time type-value checking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedTypeValue {
    payload: CheckedProjectNominalTypeValue,
}

impl CheckedTypeValue {
    pub(super) fn from_semantic_definition(
        nominal: CheckedProjectNominal,
        semantic_definition_digest: ProjectNominalSemanticDigest,
    ) -> Self {
        Self {
            payload: CheckedProjectNominalTypeValue {
                nominal,
                semantic_definition_digest,
            },
        }
    }

    pub const fn nominal(&self) -> &CheckedProjectNominal {
        &self.payload.nominal
    }

    pub(crate) fn ty(&self) -> TypeKind {
        TypeKind::MetaType(Box::new(self.nominal().ty()))
    }

    pub(crate) const fn semantic_definition_digest(&self) -> ProjectNominalSemanticDigest {
        self.payload.semantic_definition_digest
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor(&self.ty())
    }
}

/// The one semantic interpretation selected for a bounded postfix-bracket
/// ambiguity. The selected candidate keeps its immutable final-HIR identity;
/// semantic analysis never rewrites the source-backed parent expression.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PostfixBracketResolution {
    Index { candidate: ExprId },
    Dialogue { candidate: ExprId },
}

impl PostfixBracketResolution {
    /// Returns the exact candidate root selected for this postfix expression.
    pub const fn candidate(self) -> ExprId {
        match self {
            Self::Index { candidate } | Self::Dialogue { candidate } => candidate,
        }
    }
}

/// Provenance of the final type selected for one expression.
///
/// This is semantic evidence, not syntax reconstruction. In particular, LSP
/// inlay hints consume [`Self::DefaultNumericFallback`] directly instead of
/// inferring a default from literal spelling or from an obsolete checker
/// sidecar.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedTypeSelection {
    /// The expression or its owning declaration supplied an explicit type.
    Explicit,
    /// A checked expected type selected the expression type.
    Expected,
    /// The expression family determines its type without an expected type.
    Inferred,
    /// An unconstrained numeric expression used the language default.
    DefaultNumericFallback,
}

/// One accepted ordinary-Match arm coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMatchArmFact {
    guard: Option<ExprId>,
    value: ExprId,
}

impl CheckedMatchArmFact {
    pub const fn new(guard: Option<ExprId>, value: ExprId) -> Self {
        Self { guard, value }
    }

    pub const fn guard(&self) -> Option<ExprId> {
        self.guard
    }

    pub const fn value(&self) -> ExprId {
        self.value
    }
}

/// Complete checked evidence for one ordinary Match expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMatchFact {
    scrutinee: ExprId,
    arms: Box<[CheckedMatchArmFact]>,
}

impl CheckedMatchFact {
    pub fn new(scrutinee: ExprId, arms: impl Into<Box<[CheckedMatchArmFact]>>) -> Self {
        Self {
            scrutinee,
            arms: arms.into(),
        }
    }

    pub const fn scrutinee(&self) -> ExprId {
        self.scrutinee
    }

    pub fn arms(&self) -> &[CheckedMatchArmFact] {
        &self.arms
    }
}

/// Closed checked fact for one live expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedTypedExpressionResult {
    ty: TypeKind,
    type_selection: CheckedTypeSelection,
}

impl CheckedTypedExpressionResult {
    pub const fn new(ty: TypeKind, type_selection: CheckedTypeSelection) -> Self {
        Self { ty, type_selection }
    }

    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }

    pub const fn type_selection(&self) -> CheckedTypeSelection {
        self.type_selection
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedNonValueExpressionResult {
    ContentEmission(crate::callable::ContentCallableIdentity),
}

impl CheckedNonValueExpressionResult {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedExpressionResult {
    Value(CheckedTypedExpressionResult),
    NonValue(CheckedNonValueExpressionResult),
    /// A rejected or ambiguous expression has no value evidence. Diagnostic
    /// candidate signatures remain on its call facts, outside value typing.
    Unavailable,
}

impl CheckedExpressionResult {
    pub const fn value_type(&self) -> Option<&TypeKind> {
        match self {
            Self::Value(value) => Some(value.ty()),
            Self::NonValue(_) | Self::Unavailable => None,
        }
    }

    pub const fn type_selection(&self) -> Option<CheckedTypeSelection> {
        match self {
            Self::Value(value) => Some(value.type_selection()),
            Self::NonValue(_) | Self::Unavailable => None,
        }
    }
}

/// Whether an expression result is retained by runtime execution.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedRuntimeValueDisposition {
    Retain,
    Omit,
}

/// Typed reason for a structural expression's runtime value disposition.
///
/// The reason is part of the checked expression authority.  Consumers do not
/// infer omission from resolution spelling, a missing call row, or a side
/// table assembled after semantic publication.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedStructuralExecutionReason {
    Value,
    Literal,
    Structural,
    CompileTimeOnly,
    RejectedCall,
    ContentEmission,
    DialogueApplication,
    ContentValue,
    PostfixBracket,
}

/// Runtime callee mode retained by one checked call execution plan.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedExpressionCallCallee {
    Static,
    RuntimeReceiver,
}

/// Sole consumer of one accepted Call application at runtime lowering.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CheckedCallExecutionConsumer {
    Runtime,
    DialogueApplication { owner: ExprId, line: DialogueLineId },
}

/// Stable role occupied by one evaluated-effect expression in its owner.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedEvaluatedEffectRole {
    Application {
        application: CheckedCallApplicationDigest,
    },
    StatementRoot {
        statement: StmtId,
    },
    DialogueEffectSite {
        owner: ExprId,
        root: ExprId,
        ordinal: CheckedDialogueEffectSiteOrdinal,
    },
    DropPolicy {
        application: CheckedCallApplicationDigest,
    },
}

/// Closed execution plan retained directly by every final checked
/// expression.  Structural retention/omission, ordinary call callee/result,
/// and evaluated-effect ownership are sealed together before publication.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CheckedExpressionExecutionPlan {
    Structural {
        value: CheckedRuntimeValueDisposition,
        reason: CheckedStructuralExecutionReason,
        evaluated_effect_roles: Box<[CheckedEvaluatedEffectRole]>,
    },
    Call {
        application: CheckedCallApplicationDigest,
        result: CheckedRuntimeValueDisposition,
        callee: CheckedExpressionCallCallee,
        consumer: CheckedCallExecutionConsumer,
        evaluated_effect_roles: Box<[CheckedEvaluatedEffectRole]>,
    },
}

impl CheckedExpressionExecutionPlan {
    pub(crate) fn structural(
        value: CheckedRuntimeValueDisposition,
        reason: CheckedStructuralExecutionReason,
    ) -> Self {
        Self::Structural {
            value,
            reason,
            evaluated_effect_roles: Box::new([]),
        }
    }

    pub(crate) fn call(
        application: CheckedCallApplicationDigest,
        result: CheckedRuntimeValueDisposition,
        callee: CheckedExpressionCallCallee,
    ) -> Self {
        Self::Call {
            application,
            result,
            callee,
            consumer: CheckedCallExecutionConsumer::Runtime,
            evaluated_effect_roles: Box::new([]),
        }
    }

    pub const fn value(&self) -> CheckedRuntimeValueDisposition {
        match self {
            Self::Structural { value, .. } => *value,
            Self::Call { result, .. } => *result,
        }
    }

    pub const fn structural_reason(&self) -> Option<CheckedStructuralExecutionReason> {
        match self {
            Self::Structural { reason, .. } => Some(*reason),
            Self::Call { .. } => None,
        }
    }

    pub const fn call_application(&self) -> Option<CheckedCallApplicationDigest> {
        match self {
            Self::Structural { .. } => None,
            Self::Call { application, .. } => Some(*application),
        }
    }

    pub const fn call_callee(&self) -> Option<CheckedExpressionCallCallee> {
        match self {
            Self::Structural { .. } => None,
            Self::Call { callee, .. } => Some(*callee),
        }
    }

    pub const fn evaluated_effect_roles(&self) -> &[CheckedEvaluatedEffectRole] {
        match self {
            Self::Structural {
                evaluated_effect_roles,
                ..
            }
            | Self::Call {
                evaluated_effect_roles,
                ..
            } => evaluated_effect_roles,
        }
    }

    /// Returns whether this call is executed as an evaluated-effect carrier
    /// rather than as an ordinary runtime call. The sealed role ledger is the
    /// sole authority for this distinction.
    pub fn is_evaluated_effect_carrier(&self) -> bool {
        self.evaluated_effect_roles().iter().any(|role| {
            matches!(
                role,
                CheckedEvaluatedEffectRole::Application { .. }
                    | CheckedEvaluatedEffectRole::StatementRoot { .. }
                    | CheckedEvaluatedEffectRole::DialogueEffectSite { .. }
            )
        })
    }

    pub fn executes_as_runtime_call(&self) -> bool {
        matches!(
            self,
            Self::Call {
                consumer: CheckedCallExecutionConsumer::Runtime,
                ..
            }
        ) && self.evaluated_effect_roles().is_empty()
    }

    pub(crate) fn with_dialogue_consumer(
        self,
        owner: ExprId,
        line: DialogueLineId,
    ) -> Result<Self, Self> {
        match self {
            Self::Call {
                application,
                callee,
                evaluated_effect_roles,
                ..
            } => Ok(Self::Call {
                application,
                result: CheckedRuntimeValueDisposition::Omit,
                callee,
                consumer: CheckedCallExecutionConsumer::DialogueApplication { owner, line },
                evaluated_effect_roles,
            }),
            structural => Err(structural),
        }
    }

    pub(crate) fn with_evaluated_effect_roles(
        self,
        roles: impl Into<Box<[CheckedEvaluatedEffectRole]>>,
    ) -> Self {
        let roles = roles.into();
        match self {
            Self::Structural { value, reason, .. } => Self::Structural {
                value: if roles.is_empty() {
                    value
                } else {
                    CheckedRuntimeValueDisposition::Omit
                },
                reason,
                evaluated_effect_roles: roles,
            },
            Self::Call {
                application,
                result,
                callee,
                consumer,
                ..
            } => Self::Call {
                application,
                result: if roles.is_empty() {
                    result
                } else {
                    CheckedRuntimeValueDisposition::Omit
                },
                callee,
                consumer,
                evaluated_effect_roles: roles,
            },
        }
    }
}

/// Closed checked fact for one live expression.
/// The fact owns its payload on the heap so evaluator and transaction frames
/// move one owner instead of retaining every checked atom on the native stack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExpression {
    data: Box<CheckedExpressionData>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CheckedExpressionData {
    result: CheckedExpressionResult,
    effects: EffectSet,
    resolution: CheckedExpressionResolution,
    execution: CheckedExpressionExecutionPlan,
    match_fact: Option<CheckedMatchFact>,
    nested_path_evidence: Option<Result<NestedPathEvidence, super::CheckedChildEdgeError>>,
}

impl CheckedExpression {
    pub(crate) fn unavailable_call() -> Self {
        Self {
            data: Box::new(CheckedExpressionData {
                result: CheckedExpressionResult::Unavailable,
                effects: EffectSet::new(),
                resolution: CheckedExpressionResolution::Call,
                execution: CheckedExpressionExecutionPlan::structural(
                    CheckedRuntimeValueDisposition::Omit,
                    CheckedStructuralExecutionReason::RejectedCall,
                ),
                match_fact: None,
                nested_path_evidence: None,
            }),
        }
    }

    pub fn value(
        ty: TypeKind,
        type_selection: CheckedTypeSelection,
        effects: EffectSet,
        resolution: CheckedExpressionResolution,
    ) -> Self {
        Self {
            data: Box::new(CheckedExpressionData {
                result: CheckedExpressionResult::Value(CheckedTypedExpressionResult::new(
                    ty,
                    type_selection,
                )),
                effects,
                resolution,
                execution: CheckedExpressionExecutionPlan::structural(
                    CheckedRuntimeValueDisposition::Retain,
                    CheckedStructuralExecutionReason::Value,
                ),
                match_fact: None,
                nested_path_evidence: None,
            }),
        }
    }

    pub fn content_emission(
        callable: crate::callable::ContentCallableIdentity,
        effects: EffectSet,
        resolution: CheckedExpressionResolution,
    ) -> Self {
        Self {
            data: Box::new(CheckedExpressionData {
                result: CheckedExpressionResult::NonValue(
                    CheckedNonValueExpressionResult::ContentEmission(callable),
                ),
                effects,
                resolution,
                execution: CheckedExpressionExecutionPlan::structural(
                    CheckedRuntimeValueDisposition::Omit,
                    CheckedStructuralExecutionReason::ContentEmission,
                ),
                match_fact: None,
                nested_path_evidence: None,
            }),
        }
    }

    pub const fn result(&self) -> &CheckedExpressionResult {
        &self.data.result
    }

    pub const fn value_type(&self) -> Option<&TypeKind> {
        self.data.result.value_type()
    }

    pub const fn type_selection(&self) -> Option<CheckedTypeSelection> {
        self.data.result.type_selection()
    }

    pub(crate) fn required_type_selection(
        &self,
        owner: ExprId,
    ) -> Result<CheckedTypeSelection, super::FinalSemanticAnalysisError> {
        self.type_selection()
            .ok_or(super::FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })
    }

    pub const fn effects(&self) -> &EffectSet {
        &self.data.effects
    }

    /// Replaces the analyzer's prepared effect row with the completed
    /// bottom-up execution fold while preserving every other checked atom.
    #[must_use]
    pub(crate) fn with_completed_effects(mut self, effects: EffectSet) -> Self {
        self.data.effects = effects;
        self
    }

    /// Replaces only the semantic resolution while retaining the already
    /// checked result, effects, nested Match payload, and path evidence.
    #[must_use]
    pub(crate) fn with_resolution(mut self, resolution: CheckedExpressionResolution) -> Self {
        self.data.resolution = resolution;
        self
    }

    pub const fn resolution(&self) -> &CheckedExpressionResolution {
        &self.data.resolution
    }

    /// Final sema-owned execution authority for this expression.
    pub const fn execution_plan(&self) -> &CheckedExpressionExecutionPlan {
        &self.data.execution
    }

    /// Replaces only the execution plan after all call/content/effect seals
    /// are available, retaining the expression's checked semantic payload.
    #[must_use]
    pub(crate) fn with_execution_plan(mut self, execution: CheckedExpressionExecutionPlan) -> Self {
        self.data.execution = execution;
        self
    }

    /// Returns the execution-local use represented directly by this final
    /// expression fact.
    ///
    /// This deliberately recognizes only the direct `Value(Local)` shape.
    /// A compile-time scalar may retain an original local-shaped expression
    /// for semantic provenance, but that provenance is not an execution use
    /// and must not enter the implicit-callable capture ledger.
    pub const fn execution_local_use(&self) -> Option<LocalId> {
        match self.resolution() {
            CheckedExpressionResolution::Value(CheckedValueResolution::Local(local)) => {
                Some(*local)
            }
            _ => None,
        }
    }

    /// Returns the exact postfix candidate selected by this checked fact.
    /// HIR remains the authority for validating that the candidate belongs to
    /// the source-backed postfix owner.
    #[must_use]
    pub(crate) const fn selected_postfix_candidate(&self) -> Option<ExprId> {
        match self.data.resolution {
            CheckedExpressionResolution::PostfixBracket(resolution) => Some(resolution.candidate()),
            _ => None,
        }
    }

    /// Adds the checker-owned ordinary Match evidence to this expression.
    #[must_use]
    pub(crate) fn with_match_fact(mut self, fact: CheckedMatchFact) -> Self {
        self.data.match_fact = Some(fact);
        self
    }

    /// Returns the exact checked Match evidence, when this owner is a Match.
    pub const fn match_fact(&self) -> Option<&CheckedMatchFact> {
        self.data.match_fact.as_ref()
    }

    /// Returns accepted path-keyed nested child evidence for this owner.
    pub fn nested_path_evidence(
        &self,
    ) -> Option<&Result<NestedPathEvidence, super::CheckedChildEdgeError>> {
        self.data.nested_path_evidence.as_ref()
    }

    #[must_use]
    pub(crate) fn with_nested_path_evidence(
        mut self,
        evidence: Result<NestedPathEvidence, super::CheckedChildEdgeError>,
    ) -> Self {
        self.data.nested_path_evidence = Some(evidence);
        self
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        if let Some(ty) = self.value_type() {
            visitor(ty)?;
        }
        self.data.resolution.visit_types(visitor)
    }
}

/// Extra semantic payload for one live pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedPatternResolution {
    Structural,
    Literal(HirLiteral),
    Entity(CheckedProjectItem),
    Record(CheckedRecordPattern),
    Variant(CheckedVariantResolution),
    TypedBinding(CheckedTypedBinding),
}

#[path = "model/typed_binding.rs"]
mod typed_binding;
pub use typed_binding::CheckedTypedBinding;

/// Closed checked fact for one live pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedPattern {
    ty: TypeKind,
    resolution: CheckedPatternResolution,
}

impl CheckedPattern {
    pub const fn new(ty: TypeKind, resolution: CheckedPatternResolution) -> Self {
        Self { ty, resolution }
    }

    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }

    pub const fn resolution(&self) -> &CheckedPatternResolution {
        &self.resolution
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor(self.ty())?;
        match self.resolution() {
            CheckedPatternResolution::Entity(item) => item.visit_types(visitor),
            CheckedPatternResolution::Record(record) => record.visit_types(visitor),
            CheckedPatternResolution::Variant(variant) => variant.visit_types(visitor),
            CheckedPatternResolution::TypedBinding(binding) => visitor(binding.annotation()),
            CheckedPatternResolution::Structural | CheckedPatternResolution::Literal(_) => Ok(()),
        }
    }
}

#[path = "model/evaluated_effect.rs"]
mod evaluated_effect;
pub use evaluated_effect::{
    CheckedDropFade, CheckedDropFadeOperand, CheckedDropInvocation, CheckedDropPolicySource,
    CheckedEffectField, CheckedEvaluatedEffect, CheckedEvaluatedEffectOperand,
    CheckedEvaluatedEffectOperation, CheckedExplicitDropPolicy,
};

#[path = "model/statement.rs"]
mod statement;
pub use statement::{
    CheckedAssertionDisposition, CheckedAssignment, CheckedAssignmentPlace,
    CheckedIncludeFlowTarget, CheckedIteration, CheckedIteratorFamily, CheckedScopeIdentity,
    CheckedSelectBranchHead, CheckedSelectStatement, CheckedSelectStatementView, CheckedStatement,
    CheckedStatementPayload, CheckedSuspensionStatement, CheckedTraitConformance,
    CheckedTraitIdentity, CheckedTrigger, CheckedTriggerView, CheckedUnsafeAudit,
};

/// Invocation behavior of one ordinary function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedFunctionExecution {
    DirectFrame,
    StreamFactory {
        item: TypeKind,
        error: TypeKind,
        own_scope_yields: u32,
    },
}

/// Whether an ordinary callable may directly suspend its current frame.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedSuspensionRole {
    NonSuspending,
    MaySuspend,
}

/// Whether one checked executable can remain a structural expression body or
/// requires the ordinary Flow control algebra. This is independent from
/// effects and suspension: an otherwise pure project call is still a typed
/// `ProjectCall` control transfer.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedExecutableControlRole {
    ExpressionCompatible,
    FlowRequired,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedOrdinaryFunctionEmission {
    ExpressionFunctionSite,
    ExecutableFunctionSite,
    StreamFactoryUnsupported,
}

impl CheckedOrdinaryFunctionEmission {
    /// Returns whether the checked ordinary function has one admitted
    /// structured function-site body family. Runtime lowering must consume
    /// this selection; source effect-clause spelling is not an execution
    /// selector.
    pub const fn is_supported(self) -> bool {
        matches!(
            self,
            Self::ExpressionFunctionSite | Self::ExecutableFunctionSite
        )
    }

    pub const fn diagnostic_code(self) -> &'static str {
        match self {
            Self::ExpressionFunctionSite => "compiler.runtime_emission.expression_function_site",
            Self::ExecutableFunctionSite => "compiler.runtime_emission.executable_function_site",
            Self::StreamFactoryUnsupported => {
                "compiler.runtime_emission.stream_factory_unsupported"
            }
        }
    }
}

/// Exact semantic role for every executable final-HIR item family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedItemRole {
    Module,
    Use,
    Flow {
        identity: HirFlowIdentity,
    },
    Function {
        execution: CheckedFunctionExecution,
        suspension: CheckedSuspensionRole,
    },
    Predicate,
    Proof,
    Trait,
    Impl,
    Enum,
    Struct,
    TypeAlias,
    Resource,
    Character,
    View,
    Action,
    Activity,
    Signal,
    Metric,
    Layer,
    Entry,
    ExternCapability,
    Test,
    Bench,
    Style,
}

impl CheckedItemRole {
    /// Stable family coordinate used by the accepted item-root authority.
    ///
    /// This is deliberately a direct exhaustive mapping.  The recovered
    /// family has no accepted tag and therefore cannot enter a catalog.
    pub const fn accepted_item_family_tag(&self) -> u8 {
        match self {
            Self::Module => 0,
            Self::Use => 1,
            Self::Flow { .. } => 2,
            Self::Function { .. } => 3,
            Self::Predicate => 4,
            Self::Proof => 5,
            Self::Trait => 6,
            Self::Impl => 7,
            Self::Enum => 8,
            Self::Struct => 9,
            Self::TypeAlias => 10,
            Self::Resource => 11,
            Self::Character => 12,
            Self::View => 13,
            Self::Action => 14,
            Self::Activity => 15,
            Self::Signal => 16,
            Self::Metric => 17,
            Self::Layer => 18,
            Self::Entry => 19,
            Self::ExternCapability => 20,
            Self::Test => 21,
            Self::Bench => 22,
            Self::Style => 23,
        }
    }

    pub const fn family(&self) -> HirItemFamily {
        match self {
            Self::Module => HirItemFamily::Module,
            Self::Use => HirItemFamily::Use,
            Self::Flow { .. } => HirItemFamily::Flow,
            Self::Function { .. } => HirItemFamily::Function,
            Self::Predicate => HirItemFamily::Predicate,
            Self::Proof => HirItemFamily::Proof,
            Self::Trait => HirItemFamily::Trait,
            Self::Impl => HirItemFamily::Impl,
            Self::Enum => HirItemFamily::Enum,
            Self::Struct => HirItemFamily::Struct,
            Self::TypeAlias => HirItemFamily::TypeAlias,
            Self::Resource => HirItemFamily::Resource,
            Self::Character => HirItemFamily::Character,
            Self::View => HirItemFamily::View,
            Self::Action => HirItemFamily::Action,
            Self::Activity => HirItemFamily::Activity,
            Self::Signal => HirItemFamily::Signal,
            Self::Metric => HirItemFamily::Metric,
            Self::Layer => HirItemFamily::Layer,
            Self::Entry => HirItemFamily::Entry,
            Self::ExternCapability => HirItemFamily::ExternCapability,
            Self::Test => HirItemFamily::Test,
            Self::Bench => HirItemFamily::Bench,
            Self::Style => HirItemFamily::Style,
        }
    }
}

/// Closed checked fact for one live item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedItem {
    effects: EffectSet,
    role: CheckedItemRole,
}

impl CheckedItem {
    pub const fn new(effects: EffectSet, role: CheckedItemRole) -> Self {
        Self { effects, role }
    }

    /// Closed exposed effects for structural Flow execution. Ordinary
    /// callable effects are owned by the checked callable catalog.
    pub const fn effects(&self) -> &EffectSet {
        &self.effects
    }

    pub const fn role(&self) -> &CheckedItemRole {
        &self.role
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self.role() {
            CheckedItemRole::Function {
                execution: CheckedFunctionExecution::StreamFactory { item, error, .. },
                ..
            } => {
                visitor(item)?;
                visitor(error)
            }
            CheckedItemRole::Function {
                execution: CheckedFunctionExecution::DirectFrame,
                ..
            }
            | CheckedItemRole::Module
            | CheckedItemRole::Use
            | CheckedItemRole::Flow { .. }
            | CheckedItemRole::Predicate
            | CheckedItemRole::Proof
            | CheckedItemRole::Trait
            | CheckedItemRole::Impl
            | CheckedItemRole::Enum
            | CheckedItemRole::Struct
            | CheckedItemRole::TypeAlias
            | CheckedItemRole::Resource
            | CheckedItemRole::Character
            | CheckedItemRole::View
            | CheckedItemRole::Action
            | CheckedItemRole::Activity
            | CheckedItemRole::Signal
            | CheckedItemRole::Metric
            | CheckedItemRole::Layer
            | CheckedItemRole::Entry
            | CheckedItemRole::ExternCapability
            | CheckedItemRole::Test
            | CheckedItemRole::Bench
            | CheckedItemRole::Style => Ok(()),
        }
    }
}

/// Type of one lexical local or captured binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedBinding {
    ty: TypeKind,
    role: CheckedBindingRole,
}

/// Closed semantic role retained with one lexical binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedBindingRole {
    Ordinary,
    DialogueViewParameter,
}

impl CheckedBinding {
    pub const fn new(ty: TypeKind) -> Self {
        Self {
            ty,
            role: CheckedBindingRole::Ordinary,
        }
    }

    pub const fn with_role(ty: TypeKind, role: CheckedBindingRole) -> Self {
        Self { ty, role }
    }

    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor(self.ty())
    }

    pub const fn role(&self) -> CheckedBindingRole {
        self.role
    }
}

/// Stable digest of one checked expression semantic transcript.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedExpressionSemanticDigest([u8; 32]);

impl CheckedExpressionSemanticDigest {
    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Stable digest of one checked pattern semantic transcript.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedPatternSemanticDigest([u8; 32]);

impl CheckedPatternSemanticDigest {
    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Stable constructor-domain evidence used by exact Match coverage.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedCoverageDomainDigest([u8; 32]);

impl CheckedCoverageDomainDigest {
    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Stable digest of one complete generic Match product.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedMatchSemanticDigest([u8; 32]);

impl CheckedMatchSemanticDigest {
    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Compiler-local lookup evidence for one Match in an exact accepted HIR
/// module snapshot.  This is intentionally non-Serde and carries no semantic
/// payload; the query revalidates it before constructing the transaction-local
/// product.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct CheckedMatchRef {
    snapshot: HirSnapshotId,
    expression: ExprId,
}

impl CheckedMatchRef {
    pub(crate) const fn new(snapshot: HirSnapshotId, expression: ExprId) -> Self {
        Self {
            snapshot,
            expression,
        }
    }

    pub const fn snapshot(self) -> HirSnapshotId {
        self.snapshot
    }

    pub const fn expression(self) -> ExprId {
        self.expression
    }
}
