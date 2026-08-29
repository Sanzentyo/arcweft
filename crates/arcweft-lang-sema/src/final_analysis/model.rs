//! Generation-bound checked semantic fact model.

use super::match_edges::NestedPathEvidence;
use super::{
    CallableDeclarationKey, CharacterDialogueCharacterType, CharacterDialogueType, CharacterId,
    CharacterNominalType, CheckedRichTextReport, DeclarationIdentityFamily, DialogueLineId,
    DialogueTextKey, EffectSet, EnvironmentBindingId, ExprId, GenericParameterOwnerId,
    GenericTypeParameterId, HirFlowIdentity, HirItemFamily, HirLiteral, HirSnapshotId, ItemId,
    LocalId, PatternId, ProjectNominalDeclaration, ProjectNominalDeclarationId, PublicId,
    SemanticTypeDigest, TypeKind, TypeParameterSubstitutions,
};
use crate::callable::{
    CallableEvaluatedEffect, CallableLogLevel, CallableReceiverMode, CharacterDialoguePatchContext,
    CheckedCallableJoin, CheckedCallableJoinDigest, DropCallableId, OpenArgumentId,
};
pub use crate::character_dialogue::CharacterDialogueFieldCoordinate;
use crate::types::{
    AcceptedVariantCaseSemanticId, CharacterField, EntityKind, VariantPayloadOwnerFamily,
    VariantPayloadShape, VariantPayloadType,
};
use arcweft_core::value::RuntimeAgentField;
use arcweft_lang_hir::symbol::{CallableDeclarationDigest, ExternalDeclarationId};
use arcweft_source::SourceSpan;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
        let value_type = project_item_type(family, None).semantic_identity_digest();
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
        let value_type = project_item_type(family, value.as_ref()).semantic_identity_digest();
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
        let value_type = project_item_type(family, None).semantic_identity_digest();
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
            && project_item_type(self.family, self.value.as_ref()).semantic_identity_digest()
                == self.value_type
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
        debug_assert_eq!(ty.semantic_identity_digest(), self.value_type);
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
        let expected_value_type =
            TypeKind::entity_ref(crate::types::EntityKind::Entry).semantic_identity_digest();
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
        debug_assert_eq!(ty.semantic_identity_digest(), self.value_type);
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

    pub(crate) fn ty(&self) -> TypeKind {
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
            let parameter = TypeKind::GenericParam(GenericTypeParameterId::new(
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
            callable: join.semantic_digest(),
            receiver_type: receiver.semantic_identity_digest(),
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
        receiver.semantic_identity_digest() == self.receiver_type
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

/// One declaration-ordered case retained by its complete checked owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedVariantCase {
    ordinal: u32,
    semantic_id: AcceptedVariantCaseSemanticId,
    payload: VariantPayloadShape,
    diagnostic_name: Option<String>,
}

impl CheckedVariantCase {
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub(crate) const fn semantic_id(&self) -> AcceptedVariantCaseSemanticId {
        self.semantic_id
    }

    pub const fn payload(&self) -> &VariantPayloadShape {
        &self.payload
    }

    pub fn diagnostic_name(&self) -> Option<&str> {
        self.diagnostic_name.as_deref()
    }

    fn payload_type(
        &self,
        owner_family: VariantPayloadOwnerFamily,
        owner_type: SemanticTypeDigest,
    ) -> Option<Option<TypeKind>> {
        if self.payload.is_unit() {
            return Some(None);
        }
        VariantPayloadType::try_new(
            owner_family,
            owner_type,
            self.ordinal,
            self.semantic_id,
            self.payload.clone(),
        )
        .ok()
        .map(|payload| Some(TypeKind::VariantPayload(Box::new(payload))))
    }
}

/// Exact semantic owner selected for one enum case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedVariantOwner {
    Project {
        nominal: CheckedProjectNominal,
        semantic_type: SemanticTypeDigest,
        cases: Box<[CheckedVariantCase]>,
    },
    CharacterNominal {
        nominal: CharacterNominalType,
        semantic_type: SemanticTypeDigest,
        cases: Box<[CheckedVariantCase]>,
    },
    BuiltinClosed {
        nominal: EnvironmentBindingId,
        semantic_type: SemanticTypeDigest,
        cases: Box<[CheckedVariantCase]>,
    },
    RuntimeBuiltin {
        owner: arcweft_core::pattern::RuntimeBuiltinVariantIdentity,
        semantic_type: SemanticTypeDigest,
        cases: Box<[CheckedVariantCase]>,
    },
    Option {
        item: TypeKind,
        cases: [CheckedVariantCase; 2],
    },
    Result {
        ok: TypeKind,
        error: TypeKind,
        cases: [CheckedVariantCase; 2],
    },
}

impl CheckedVariantOwner {
    pub(crate) const fn payload_owner_family(&self) -> VariantPayloadOwnerFamily {
        match self {
            Self::Project { .. } => VariantPayloadOwnerFamily::Project,
            Self::CharacterNominal { .. } => VariantPayloadOwnerFamily::CharacterNominal,
            Self::BuiltinClosed { .. } => VariantPayloadOwnerFamily::BuiltinClosed,
            Self::RuntimeBuiltin { .. } => VariantPayloadOwnerFamily::RuntimeBuiltin,
            Self::Option { .. } => VariantPayloadOwnerFamily::Option,
            Self::Result { .. } => VariantPayloadOwnerFamily::Result,
        }
    }

    #[allow(
        dead_code,
        reason = "C2.4 digest-ordered Project seed sealing consumes this exact final-row constructor"
    )]
    pub(crate) fn try_project(
        nominal: CheckedProjectNominal,
        cases: impl IntoIterator<Item = (Option<TypeKind>, Option<String>)>,
    ) -> Option<Self> {
        let semantic_type = nominal.identity();
        Some(Self::Project {
            nominal,
            semantic_type,
            cases: checked_variant_cases(VariantPayloadOwnerFamily::Project, semantic_type, cases)?,
        })
    }

    pub(crate) fn try_project_shapes(
        nominal: CheckedProjectNominal,
        cases: impl IntoIterator<Item = (VariantPayloadShape, Option<String>)>,
    ) -> Option<Self> {
        let semantic_type = nominal.identity();
        let cases = cases
            .into_iter()
            .enumerate()
            .map(|(ordinal, (payload, diagnostic_name))| {
                let ordinal = u32::try_from(ordinal).ok()?;
                payload
                    .has_valid_rows(VariantPayloadOwnerFamily::Project, semantic_type, ordinal)
                    .then(|| {
                        checked_variant_case(
                            VariantPayloadOwnerFamily::Project,
                            semantic_type,
                            ordinal,
                            payload,
                            diagnostic_name,
                        )
                    })
            })
            .collect::<Option<Vec<_>>>()?
            .into_boxed_slice();
        Some(Self::Project {
            nominal,
            semantic_type,
            cases,
        })
    }

    pub(crate) fn try_character_nominal(
        nominal: CharacterNominalType,
        names: impl IntoIterator<Item = String>,
    ) -> Option<Self> {
        let semantic_type = TypeKind::CharacterNominal(nominal.clone()).semantic_identity_digest();
        let cases = names.into_iter().map(|name| (None, Some(name)));
        Some(Self::CharacterNominal {
            nominal,
            semantic_type,
            cases: checked_variant_cases(
                VariantPayloadOwnerFamily::CharacterNominal,
                semantic_type,
                cases,
            )?,
        })
    }

    #[cfg(test)]
    pub(crate) fn try_builtin_closed(
        nominal: EnvironmentBindingId,
        semantic_type: SemanticTypeDigest,
        cases: impl IntoIterator<Item = (Option<TypeKind>, Option<String>)>,
    ) -> Option<Self> {
        Some(Self::BuiltinClosed {
            nominal,
            semantic_type,
            cases: checked_variant_cases(
                VariantPayloadOwnerFamily::BuiltinClosed,
                semantic_type,
                cases,
            )?,
        })
    }

    /// Constructs the exact accepted environment-owned closed variant row.
    /// Case order/payload semantics and the runtime-builtin distinction are
    /// selected here once for both checked patterns and semantic catalogs.
    pub(crate) fn try_environment(
        schema: &crate::env::EnvironmentEnumSchema,
        ty: &TypeKind,
    ) -> Option<Self> {
        let semantic_type = ty.semantic_identity_digest();
        match ty {
            TypeKind::AgentResourceBody => Some(Self::RuntimeBuiltin {
                owner: arcweft_core::pattern::RuntimeBuiltinVariantIdentity::AgentResourceBody,
                semantic_type,
                cases: checked_environment_variant_cases(
                    VariantPayloadOwnerFamily::RuntimeBuiltin,
                    semantic_type,
                    schema,
                )?,
            }),
            TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::AgentBinaryEncoding) => {
                Some(Self::RuntimeBuiltin {
                    owner:
                        arcweft_core::pattern::RuntimeBuiltinVariantIdentity::AgentBinaryEncoding,
                    semantic_type,
                    cases: checked_environment_variant_cases(
                        VariantPayloadOwnerFamily::RuntimeBuiltin,
                        semantic_type,
                        schema,
                    )?,
                })
            }
            _ => Some(Self::BuiltinClosed {
                nominal: schema.owner().clone(),
                semantic_type,
                cases: checked_environment_variant_cases(
                    VariantPayloadOwnerFamily::BuiltinClosed,
                    semantic_type,
                    schema,
                )?,
            }),
        }
    }

    /// # Panics
    ///
    /// Panics only if the fixed one-field `Option` payload shape violates its
    /// checked variant invariant.
    pub fn option(item: TypeKind) -> Self {
        let semantic_type = TypeKind::Option(Box::new(item.clone())).semantic_identity_digest();
        Self::Option {
            item: item.clone(),
            cases: [
                checked_variant_case(
                    VariantPayloadOwnerFamily::Option,
                    semantic_type,
                    0,
                    VariantPayloadShape::try_tuple(
                        VariantPayloadOwnerFamily::Option,
                        semantic_type,
                        0,
                        [item],
                    )
                    .expect("one Option payload field is representable"),
                    Some("Some".into()),
                ),
                checked_variant_case(
                    VariantPayloadOwnerFamily::Option,
                    semantic_type,
                    1,
                    VariantPayloadShape::Unit,
                    Some("None".into()),
                ),
            ],
        }
    }

    /// # Panics
    ///
    /// Panics only if the fixed `Result` payload shape violates its checked
    /// variant invariant.
    pub fn result(ok: TypeKind, error: TypeKind) -> Self {
        let semantic_type = TypeKind::Result {
            ok: Box::new(ok.clone()),
            error: Box::new(error.clone()),
        }
        .semantic_identity_digest();
        Self::Result {
            ok: ok.clone(),
            error: error.clone(),
            cases: [
                checked_variant_case(
                    VariantPayloadOwnerFamily::Result,
                    semantic_type,
                    0,
                    VariantPayloadShape::try_tuple(
                        VariantPayloadOwnerFamily::Result,
                        semantic_type,
                        0,
                        [ok],
                    )
                    .expect("one Result payload field is representable"),
                    Some("Ok".into()),
                ),
                checked_variant_case(
                    VariantPayloadOwnerFamily::Result,
                    semantic_type,
                    1,
                    VariantPayloadShape::try_tuple(
                        VariantPayloadOwnerFamily::Result,
                        semantic_type,
                        1,
                        [error],
                    )
                    .expect("one Result payload field is representable"),
                    Some("Err".into()),
                ),
            ],
        }
    }

    pub const fn project(&self) -> Option<&CheckedProjectNominal> {
        match self {
            Self::Project { nominal, .. } => Some(nominal),
            Self::CharacterNominal { .. }
            | Self::BuiltinClosed { .. }
            | Self::RuntimeBuiltin { .. }
            | Self::Option { .. }
            | Self::Result { .. } => None,
        }
    }

    pub fn cases(&self) -> &[CheckedVariantCase] {
        match self {
            Self::Project { cases, .. }
            | Self::CharacterNominal { cases, .. }
            | Self::BuiltinClosed { cases, .. }
            | Self::RuntimeBuiltin { cases, .. } => cases,
            Self::Option { cases, .. } | Self::Result { cases, .. } => cases,
        }
    }

    pub fn semantic_type(&self) -> SemanticTypeDigest {
        match self {
            Self::Project { semantic_type, .. }
            | Self::CharacterNominal { semantic_type, .. }
            | Self::BuiltinClosed { semantic_type, .. }
            | Self::RuntimeBuiltin { semantic_type, .. } => *semantic_type,
            Self::Option { item, .. } => {
                TypeKind::Option(Box::new(item.clone())).semantic_identity_digest()
            }
            Self::Result { ok, error, .. } => TypeKind::Result {
                ok: Box::new(ok.clone()),
                error: Box::new(error.clone()),
            }
            .semantic_identity_digest(),
        }
    }

    pub fn case(&self, ordinal: u32) -> Option<&CheckedVariantCase> {
        usize::try_from(ordinal)
            .ok()
            .and_then(|index| self.cases().get(index))
            .filter(|case| case.ordinal == ordinal)
    }

    pub fn case_payload_type(&self, ordinal: u32) -> Option<Option<TypeKind>> {
        self.case(ordinal)?
            .payload_type(self.payload_owner_family(), self.semantic_type())
    }

    pub(crate) fn has_valid_case_rows(&self) -> bool {
        let (owner_family, semantic_type) = match self {
            Self::Project {
                nominal,
                semantic_type,
                ..
            } => {
                if nominal.identity() != *semantic_type {
                    return false;
                }
                (VariantPayloadOwnerFamily::Project, *semantic_type)
            }
            Self::CharacterNominal {
                nominal,
                semantic_type,
                ..
            } => {
                if TypeKind::CharacterNominal(nominal.clone()).semantic_identity_digest()
                    != *semantic_type
                {
                    return false;
                }
                (VariantPayloadOwnerFamily::CharacterNominal, *semantic_type)
            }
            Self::BuiltinClosed { semantic_type, .. } => {
                (VariantPayloadOwnerFamily::BuiltinClosed, *semantic_type)
            }
            Self::RuntimeBuiltin { semantic_type, .. } => {
                (VariantPayloadOwnerFamily::RuntimeBuiltin, *semantic_type)
            }
            Self::Option { item, .. } => (
                VariantPayloadOwnerFamily::Option,
                TypeKind::Option(Box::new(item.clone())).semantic_identity_digest(),
            ),
            Self::Result { ok, error, .. } => (
                VariantPayloadOwnerFamily::Result,
                TypeKind::Result {
                    ok: Box::new(ok.clone()),
                    error: Box::new(error.clone()),
                }
                .semantic_identity_digest(),
            ),
        };
        self.cases().iter().enumerate().all(|(ordinal, case)| {
            u32::try_from(ordinal).is_ok_and(|ordinal| {
                case.ordinal == ordinal
                    && checked_variant_payload_has_no_poison(&case.payload)
                    && checked_variant_payload_is_valid(
                        owner_family,
                        semantic_type,
                        ordinal,
                        &case.payload,
                    )
                    && case.semantic_id
                        == AcceptedVariantCaseSemanticId::issue(
                            owner_family,
                            semantic_type,
                            ordinal,
                            &case.payload,
                        )
            })
        })
    }

    pub(crate) fn has_same_diagnostic_schema(&self, other: &Self) -> bool {
        self == other
            && self
                .cases()
                .iter()
                .zip(other.cases())
                .all(|(left, right)| left.payload.has_same_diagnostic_schema(&right.payload))
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Project { nominal, cases, .. } => {
                nominal.visit_types(visitor)?;
                for case in cases {
                    visit_checked_variant_payload_types(case.payload(), visitor)?;
                }
                Ok(())
            }
            Self::CharacterNominal { cases, .. }
            | Self::BuiltinClosed { cases, .. }
            | Self::RuntimeBuiltin { cases, .. } => {
                for case in cases {
                    visit_checked_variant_payload_types(case.payload(), visitor)?;
                }
                Ok(())
            }
            Self::Option { item, cases } => {
                visitor(item)?;
                for case in cases {
                    visit_checked_variant_payload_types(case.payload(), visitor)?;
                }
                Ok(())
            }
            Self::Result { ok, error, cases } => {
                visitor(ok)?;
                visitor(error)?;
                for case in cases {
                    visit_checked_variant_payload_types(case.payload(), visitor)?;
                }
                Ok(())
            }
        }
    }
}

/// Checked enum case selected for an expression or pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedVariantResolution {
    owner: CheckedVariantOwner,
    selected_ordinal: u32,
}

impl CheckedVariantResolution {
    pub(crate) fn try_new(owner: CheckedVariantOwner, selected_ordinal: u32) -> Option<Self> {
        owner.case(selected_ordinal)?;
        Some(Self {
            owner,
            selected_ordinal,
        })
    }

    pub const fn owner(&self) -> &CheckedVariantOwner {
        &self.owner
    }

    pub const fn ordinal(&self) -> u32 {
        self.selected_ordinal
    }

    /// Returns the exact owner row selected by `selected_ordinal`.
    ///
    /// # Panics
    ///
    /// Panics only if crate-internal memory corruption violates the private
    /// constructor invariant.
    pub fn selected(&self) -> &CheckedVariantCase {
        self.owner
            .case(self.selected_ordinal)
            .expect("checked variant resolution retains one exact owner case")
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        self.owner.visit_types(visitor)
    }
}

fn checked_variant_cases(
    owner_family: VariantPayloadOwnerFamily,
    semantic_type: SemanticTypeDigest,
    cases: impl IntoIterator<Item = (Option<TypeKind>, Option<String>)>,
) -> Option<Box<[CheckedVariantCase]>> {
    cases
        .into_iter()
        .enumerate()
        .map(|(ordinal, (payload, diagnostic_name))| {
            let ordinal = u32::try_from(ordinal).ok()?;
            Some(checked_variant_case(
                owner_family,
                semantic_type,
                ordinal,
                match payload {
                    Some(payload) => checked_variant_payload_from_pattern_type(
                        owner_family,
                        semantic_type,
                        ordinal,
                        payload,
                    )?,
                    None => VariantPayloadShape::Unit,
                },
                diagnostic_name,
            ))
        })
        .collect::<Option<Vec<_>>>()
        .map(Vec::into_boxed_slice)
}

fn checked_variant_case(
    owner_family: VariantPayloadOwnerFamily,
    semantic_type: SemanticTypeDigest,
    ordinal: u32,
    payload: VariantPayloadShape,
    diagnostic_name: Option<String>,
) -> CheckedVariantCase {
    let semantic_id =
        AcceptedVariantCaseSemanticId::issue(owner_family, semantic_type, ordinal, &payload);
    CheckedVariantCase {
        ordinal,
        semantic_id,
        payload,
        diagnostic_name,
    }
}

fn checked_environment_variant_cases(
    owner_family: VariantPayloadOwnerFamily,
    semantic_type: SemanticTypeDigest,
    schema: &crate::env::EnvironmentEnumSchema,
) -> Option<Box<[CheckedVariantCase]>> {
    schema
        .variants()
        .iter()
        .enumerate()
        .map(|(case_ordinal, variant)| {
            let case_ordinal = u32::try_from(case_ordinal).ok()?;
            let payload = match variant.payload() {
                crate::env::EnumVariantPayload::Unit => VariantPayloadShape::Unit,
                crate::env::EnumVariantPayload::Tuple(items) => VariantPayloadShape::try_tuple(
                    owner_family,
                    semantic_type,
                    case_ordinal,
                    items.iter().cloned(),
                )
                .ok()?,
                crate::env::EnumVariantPayload::Record(fields) => VariantPayloadShape::try_record(
                    owner_family,
                    semantic_type,
                    case_ordinal,
                    fields
                        .iter()
                        .map(|field| (field.name().to_owned(), field.ty().clone())),
                )
                .ok()?,
            };
            Some(checked_variant_case(
                owner_family,
                semantic_type,
                case_ordinal,
                payload,
                Some(variant.name().to_owned()),
            ))
        })
        .collect::<Option<Vec<_>>>()
        .map(Vec::into_boxed_slice)
}

fn checked_variant_payload_is_valid(
    owner_family: VariantPayloadOwnerFamily,
    semantic_type: SemanticTypeDigest,
    case_ordinal: u32,
    payload: &VariantPayloadShape,
) -> bool {
    payload.has_valid_rows(owner_family, semantic_type, case_ordinal)
}

fn checked_variant_payload_from_pattern_type(
    owner_family: VariantPayloadOwnerFamily,
    semantic_type: SemanticTypeDigest,
    case_ordinal: u32,
    payload: TypeKind,
) -> Option<VariantPayloadShape> {
    VariantPayloadShape::try_tuple(owner_family, semantic_type, case_ordinal, [payload]).ok()
}

fn checked_variant_payload_has_no_poison(payload: &VariantPayloadShape) -> bool {
    payload
        .visit_types(&mut |ty| -> Result<(), ()> {
            (!ty.contains_nominal_poison()).then_some(()).ok_or(())
        })
        .is_ok()
}

fn visit_checked_variant_payload_types<E>(
    payload: &VariantPayloadShape,
    visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
) -> Result<(), E> {
    payload.visit_types(visitor)
}

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
pub use capture::{
    CheckedCapture, CheckedCaptureAuthorityViolation, CheckedClosure, CheckedImplicitCallable,
    CheckedImplicitCaptureUse,
};

#[path = "model/dialogue_line_plan.rs"]
mod dialogue_line_plan;
pub use dialogue_line_plan::{
    CheckedDialogueEffectSite, CheckedDialogueEffectSiteOrdinal, CheckedDialogueEffectTrigger,
    CheckedDialogueLinePlan,
};

/// Semantic payload needed in addition to the final-HIR expression family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedExpressionResolution {
    Structural,
    Literal(HirLiteral),
    Value(CheckedValueResolution),
    Select(CheckedSelectResolution),
    Nominal(CheckedProjectNominal),
    Variant(CheckedVariantResolution),
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
    ImplicitParameter {
        callable: ExprId,
    },
    /// One once-evaluated pipeline and its checked pipe-left uses.
    Pipe(CheckedPipe),
    /// One `^` placeholder bound by its checked pipeline owner.
    PipeLeft {
        pipe: ExprId,
    },
    /// A call whose execution contract belongs to the retained View program,
    /// rather than to the ordinary callable catalog.
    ViewCall(CheckedViewCall),
    /// The typed callee leaf of a retained View call.
    ViewCallee(CheckedViewCallee),
    /// A property value admitted by the final-HIR Style checker.
    StyleValue(arcweft_view::style::ViewSpecifiedValue),
    /// The typed callee leaf of a Style value constructor.
    StyleCallee(CheckedStyleCallee),
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
        line_plan: CheckedDialogueLinePlan,
        line_result: TypeKind,
    },
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
            | Self::CharacterDialogueReconfigure(_) => {
                Some(crate::callable::CheckedCallSite::HirCall(owner))
            }
            Self::DialogueApplication { .. } => {
                Some(crate::callable::CheckedCallSite::DialogueApplication(owner))
            }
            Self::Structural
            | Self::Literal(_)
            | Self::Value(_)
            | Self::Select(_)
            | Self::Nominal(_)
            | Self::Variant(_)
            | Self::StageLook(_)
            | Self::Effect(_)
            | Self::Await(_)
            | Self::Choice(_)
            | Self::Try(_)
            | Self::ImplicitCallable(_)
            | Self::Closure(_)
            | Self::ImplicitParameter { .. }
            | Self::Pipe(_)
            | Self::PipeLeft { .. }
            | Self::ViewCall(_)
            | Self::ViewCallee(_)
            | Self::StyleValue(_)
            | Self::StyleCallee(_)
            | Self::DialogueLineReference(_)
            | Self::DialogueLineCoordinate(_)
            | Self::DialogueTextKeyCoordinate(_)
            | Self::PostfixBracket(_) => None,
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
            Self::Choice(choice) => {
                for goto in choice.gotos() {
                    goto.target().visit_types(visitor)?;
                }
                Ok(())
            }
            Self::Try(checked) => checked.carrier().visit_types(visitor),
            Self::ImplicitCallable(callable) => callable.visit_types(visitor),
            Self::CharacterDialogueFactory(factory) => factory.visit_types(visitor),
            Self::CharacterDialogueReconfigure(reconfigure) => reconfigure.visit_types(visitor),
            Self::DialogueApplication {
                target,
                application_patch,
                rich_text: _,
                line_plan: _,
                line_result,
            } => {
                target.visit_types(visitor)?;
                if let Some(patch) = application_patch {
                    patch.visit_types(visitor)?;
                }
                visitor(line_result)
            }
            Self::Structural
            | Self::Literal(_)
            | Self::StageLook(_)
            | Self::Effect(_)
            | Self::Call
            | Self::Await(_)
            | Self::Closure(_)
            | Self::ImplicitParameter { .. }
            | Self::Pipe(_)
            | Self::PipeLeft { .. }
            | Self::ViewCall(_)
            | Self::ViewCallee(_)
            | Self::StyleValue(_)
            | Self::StyleCallee(_)
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

/// Checked once-only pipe binding and every `^` use owned by it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedPipe {
    left: ExprId,
    right: ExprId,
    placeholders: Box<[ExprId]>,
}

impl CheckedPipe {
    pub const fn new(left: ExprId, right: ExprId, placeholders: Box<[ExprId]>) -> Self {
        Self {
            left,
            right,
            placeholders,
        }
    }

    pub const fn left(&self) -> ExprId {
        self.left
    }

    pub const fn right(&self) -> ExprId {
        self.right
    }

    pub const fn placeholders(&self) -> &[ExprId] {
        &self.placeholders
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

/// Nearest typed lexical owner that receives one Try residual.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedTryBoundary {
    Infallible,
    CarrierBlock(ExprId),
    FunctionSite(ExprId),
    Callable(ItemId),
}

/// Complete checked meaning of one prefix Try expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedTry {
    operand: ExprId,
    carrier: CheckedTryCarrier,
    boundary: CheckedTryBoundary,
}

impl CheckedTry {
    pub const fn new(
        operand: ExprId,
        carrier: CheckedTryCarrier,
        boundary: CheckedTryBoundary,
    ) -> Self {
        Self {
            operand,
            carrier,
            boundary,
        }
    }

    pub const fn operand(&self) -> ExprId {
        self.operand
    }

    pub const fn carrier(&self) -> &CheckedTryCarrier {
        &self.carrier
    }

    pub const fn boundary(&self) -> CheckedTryBoundary {
        self.boundary
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

/// Closed semantic classification for the source callee of a View call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedViewCallee {
    Element(arcweft_view::ViewElementKind),
    Text,
    RichText,
}

/// Closed constructors whose meaning belongs to Style value checking.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedStyleCallee {
    Rgba,
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
pub struct CheckedExpression {
    ty: TypeKind,
    type_selection: CheckedTypeSelection,
    effects: EffectSet,
    resolution: CheckedExpressionResolution,
    match_fact: Option<CheckedMatchFact>,
    nested_path_evidence: Option<Result<NestedPathEvidence, super::CheckedChildEdgeError>>,
}

impl CheckedExpression {
    pub const fn new(
        ty: TypeKind,
        type_selection: CheckedTypeSelection,
        effects: EffectSet,
        resolution: CheckedExpressionResolution,
    ) -> Self {
        Self {
            ty,
            type_selection,
            effects,
            resolution,
            match_fact: None,
            nested_path_evidence: None,
        }
    }

    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }

    pub const fn type_selection(&self) -> CheckedTypeSelection {
        self.type_selection
    }

    pub const fn effects(&self) -> &EffectSet {
        &self.effects
    }

    /// Replaces the analyzer's prepared effect row with the completed
    /// bottom-up execution fold while preserving every other checked atom.
    #[must_use]
    pub(crate) fn with_completed_effects(mut self, effects: EffectSet) -> Self {
        self.effects = effects;
        self
    }

    pub const fn resolution(&self) -> &CheckedExpressionResolution {
        &self.resolution
    }

    /// Returns the exact postfix candidate selected by this checked fact.
    /// HIR remains the authority for validating that the candidate belongs to
    /// the source-backed postfix owner.
    pub(crate) const fn selected_postfix_candidate(&self) -> Option<ExprId> {
        match self.resolution {
            CheckedExpressionResolution::PostfixBracket(resolution) => Some(resolution.candidate()),
            _ => None,
        }
    }

    /// Adds the checker-owned ordinary Match evidence to this expression.
    #[must_use]
    pub(crate) fn with_match_fact(mut self, fact: CheckedMatchFact) -> Self {
        self.match_fact = Some(fact);
        self
    }

    /// Returns the exact checked Match evidence, when this owner is a Match.
    pub const fn match_fact(&self) -> Option<&CheckedMatchFact> {
        self.match_fact.as_ref()
    }

    /// Returns accepted path-keyed nested child evidence for this owner.
    pub fn nested_path_evidence(
        &self,
    ) -> Option<&Result<NestedPathEvidence, super::CheckedChildEdgeError>> {
        self.nested_path_evidence.as_ref()
    }

    #[must_use]
    pub(crate) fn with_nested_path_evidence(
        mut self,
        evidence: Result<NestedPathEvidence, super::CheckedChildEdgeError>,
    ) -> Self {
        self.nested_path_evidence = Some(evidence);
        self
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor(self.ty())?;
        self.resolution.visit_types(visitor)
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedOrdinaryFunctionEmission {
    PureDirectFrame,
    EffectfulDirectFrameUnsupported,
    SuspendingDirectFrameUnsupported,
    StreamFactoryUnsupported,
}

impl CheckedOrdinaryFunctionEmission {
    pub const fn diagnostic_code(self) -> &'static str {
        match self {
            Self::PureDirectFrame => "compiler.runtime_emission.pure_direct_frame",
            Self::EffectfulDirectFrameUnsupported => {
                "compiler.runtime_emission.effectful_function_unsupported"
            }
            Self::SuspendingDirectFrameUnsupported => {
                "compiler.runtime_emission.suspending_function_unsupported"
            }
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

    pub fn ordinary_function_emission(
        &self,
        effects: &EffectSet,
    ) -> Option<CheckedOrdinaryFunctionEmission> {
        let Self::Function {
            execution,
            suspension,
        } = self
        else {
            return None;
        };
        Some(match (execution, suspension, effects.is_empty()) {
            (CheckedFunctionExecution::DirectFrame, CheckedSuspensionRole::NonSuspending, true) => {
                CheckedOrdinaryFunctionEmission::PureDirectFrame
            }
            (
                CheckedFunctionExecution::DirectFrame,
                CheckedSuspensionRole::NonSuspending,
                false,
            ) => CheckedOrdinaryFunctionEmission::EffectfulDirectFrameUnsupported,
            (CheckedFunctionExecution::DirectFrame, CheckedSuspensionRole::MaySuspend, _) => {
                CheckedOrdinaryFunctionEmission::SuspendingDirectFrameUnsupported
            }
            (CheckedFunctionExecution::StreamFactory { .. }, _, _) => {
                CheckedOrdinaryFunctionEmission::StreamFactoryUnsupported
            }
        })
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
/// module snapshot.
///
/// The raw expression identity is deliberately retained only for session
/// lookup. Neither field participates in persistent or semantic identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CheckedMatchRef {
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
