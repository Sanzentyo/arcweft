//! Generation-bound checked semantic fact model.

use super::{
    AssertionRuntimePolicy, CallableDeclarationKey, CharacterId, CharacterNominalType,
    CheckedRichTextReport, DeclarationIdentityFamily, DialogueLineId, EffectSet,
    EnvironmentBindingId, ExprId, GenericTypeOwnerId, GenericTypeParameterId, HirFlowIdentity,
    HirItemFamily, HirLiteral, HirName, ItemId, LocalId, ProjectNominalDeclaration,
    ProjectNominalDeclarationId, PublicId, SemanticTypeDigest, TypeKind,
    TypeParameterSubstitutions,
};
use arcweft_lang_hir::symbol::ExternalDeclarationId;

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

/// Exact project declaration selected by an entity-reference leaf.
///
/// The public ID is the stable publication projection. The closed owner is the
/// semantic identity: in particular, structural Flow owners retain their exact
/// module-preserving declaration key even when two modules derive the same
/// public spelling. Character facts also retain the validated [`CharacterId`]
/// selected by registration, so consumers never reconstruct it from source
/// text or fabricate an [`ItemId`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedProjectItem {
    public_id: PublicId,
    family: DeclarationIdentityFamily,
    owner: CheckedProjectItemOwner,
    character: Option<CharacterId>,
}

impl CheckedProjectItem {
    pub(crate) fn new_flow(declaration: CallableDeclarationKey, item: ItemId) -> Option<Self> {
        let CallableDeclarationKey::Flow(flow) = &declaration else {
            return None;
        };
        Some(Self {
            public_id: flow.public_id().clone(),
            family: DeclarationIdentityFamily::Flow,
            owner: CheckedProjectItemOwner::Flow { declaration, item },
            character: None,
        })
    }

    pub(crate) fn try_new_retained(
        public_id: PublicId,
        family: DeclarationIdentityFamily,
        owner: ItemId,
    ) -> Option<Self> {
        crate::types::EntityKind::from_declaration_identity_family(family)?;
        let character = (family == DeclarationIdentityFamily::Character)
            .then(|| CharacterId::try_new(public_id.as_str()).ok())
            .flatten();
        if family == DeclarationIdentityFamily::Character && character.is_none() {
            return None;
        }
        Some(Self {
            public_id,
            family,
            owner: CheckedProjectItemOwner::Retained(owner),
            character,
        })
    }

    pub(crate) fn new_external_character(
        declaration: ExternalDeclarationId,
        character: CharacterId,
    ) -> Self {
        Self {
            public_id: character.as_public_id(),
            family: DeclarationIdentityFamily::Character,
            owner: CheckedProjectItemOwner::External(declaration),
            character: Some(character),
        }
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.public_id
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
        let kind = crate::types::EntityKind::from_declaration_identity_family(self.family)
            .expect("checked project items only retain entity-reference families");
        TypeKind::Ref(crate::types::EntityType::new(kind, None))
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
    public_id: PublicId,
    owner: ItemId,
}

impl CheckedEntryReference {
    pub(crate) const fn new(public_id: PublicId, owner: ItemId) -> Self {
        Self { public_id, owner }
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.public_id
    }

    pub const fn owner(&self) -> ItemId {
        self.owner
    }

    pub fn ty(&self) -> TypeKind {
        TypeKind::entity_ref(crate::types::EntityKind::Entry)
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
                GenericTypeOwnerId::Nominal(declaration.id().clone()),
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
    ProjectCallable(CheckedProjectCallable),
    ProjectItem(CheckedProjectItem),
    Entry(CheckedEntryReference),
    Registered(RegisteredSemanticValueId),
    Constant(HirLiteral),
}

/// Checked projection selected for one member expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedSelectResolution {
    /// Runtime-supplied field of a nominal record carrying the semantic
    /// `#[dialogue_view]` role. The projection identity is selected by the
    /// environment registry, never reconstructed from its field spelling by
    /// compiler or runtime consumers.
    DialogueView {
        projection: crate::dialogue_view::DialogueViewProjection,
        name: HirName,
    },
    Field {
        nominal: Option<CheckedProjectNominal>,
        name: HirName,
    },
    TupleElement {
        ordinal: u32,
    },
    RecordElement {
        nominal: Option<CheckedProjectNominal>,
        ordinal: u32,
        name: HirName,
    },
}

/// Exact semantic owner selected for one enum case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedBuiltinVariantCase {
    name: String,
    payload: Option<TypeKind>,
}

impl CheckedBuiltinVariantCase {
    pub fn new(name: impl Into<String>, payload: Option<TypeKind>) -> Self {
        Self {
            name: name.into(),
            payload,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn payload(&self) -> Option<&TypeKind> {
        self.payload.as_ref()
    }
}

/// Exact semantic owner selected for one enum case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedVariantOwner {
    Project(CheckedProjectNominal),
    CharacterNominal {
        nominal: CharacterNominalType,
        cases: Box<[String]>,
    },
    BuiltinClosed {
        nominal: EnvironmentBindingId,
        semantic_identity: SemanticTypeDigest,
        cases: Box<[CheckedBuiltinVariantCase]>,
    },
    Option {
        item: TypeKind,
    },
    Result {
        ok: TypeKind,
        error: TypeKind,
    },
}

impl CheckedVariantOwner {
    pub const fn project(&self) -> Option<&CheckedProjectNominal> {
        match self {
            Self::Project(nominal) => Some(nominal),
            Self::CharacterNominal { .. }
            | Self::BuiltinClosed { .. }
            | Self::Option { .. }
            | Self::Result { .. } => None,
        }
    }
}

/// Checked enum case selected for an expression or pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedVariantResolution {
    owner: CheckedVariantOwner,
    ordinal: u32,
    name: HirName,
}

impl CheckedVariantResolution {
    pub const fn new(owner: CheckedVariantOwner, ordinal: u32, name: HirName) -> Self {
        Self {
            owner,
            ordinal,
            name,
        }
    }

    pub const fn owner(&self) -> &CheckedVariantOwner {
        &self.owner
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub const fn name(&self) -> &HirName {
        &self.name
    }
}

/// Semantic payload needed in addition to the final-HIR expression family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedExpressionResolution {
    Structural,
    Literal(HirLiteral),
    Value(CheckedValueResolution),
    Select(CheckedSelectResolution),
    Nominal(CheckedProjectNominal),
    Variant(CheckedVariantResolution),
    /// Canonical effect identity selected from an authored effect-clause path.
    Effect(crate::effects::EffectId),
    Call,
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
    DialogueApplication {
        character: ItemId,
        rich_text: Box<CheckedRichTextReport>,
    },
    PostfixBracket(PostfixBracketResolution),
}

/// Closed semantic classification for a call executed by the View evaluator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedViewCall {
    Element(arcweft_view::ViewElementKind),
    Text,
    RichText,
    Modifier {
        member: arcweft_lang_hir::leaf::HirName,
    },
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

/// Closed checked fact for one live expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExpression {
    ty: TypeKind,
    type_selection: CheckedTypeSelection,
    effects: EffectSet,
    resolution: CheckedExpressionResolution,
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

    pub const fn resolution(&self) -> &CheckedExpressionResolution {
        &self.resolution
    }
}

/// Extra semantic payload for one live pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedPatternResolution {
    Structural,
    Literal(HirLiteral),
    Entity(CheckedProjectItem),
    Nominal(CheckedProjectNominal),
    Variant(CheckedVariantResolution),
}

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
}

/// Built-in iteration families whose runtime behavior is language-owned.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedIteratorFamily {
    Range,
    Seq,
    Stream,
    Vec,
    Array,
    Slice,
}

/// Generation-bound identity of the selected trait authority.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedTraitIdentity {
    Project(ItemId),
    StandardIterator,
    StandardIntoIterator,
}

/// Generation-bound trait conformance used by iteration lowering.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedTraitConformance {
    implementation: ItemId,
    trait_identity: CheckedTraitIdentity,
    method: u16,
}

impl CheckedTraitConformance {
    pub const fn new(
        implementation: ItemId,
        trait_identity: CheckedTraitIdentity,
        method: u16,
    ) -> Self {
        Self {
            implementation,
            trait_identity,
            method,
        }
    }

    pub const fn implementation(&self) -> ItemId {
        self.implementation
    }

    pub const fn trait_identity(&self) -> &CheckedTraitIdentity {
        &self.trait_identity
    }

    pub const fn method(&self) -> u16 {
        self.method
    }
}

/// Checked iteration dispatch for one final-HIR `for` statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedIteration {
    Builtin {
        family: CheckedIteratorFamily,
        item: TypeKind,
    },
    Witness {
        source: TypeKind,
        item: TypeKind,
        into_iter: TypeKind,
        into_iterator: CheckedTraitConformance,
        iterator: CheckedTraitConformance,
    },
    IteratorWitness {
        source: TypeKind,
        item: TypeKind,
        iterator: CheckedTraitConformance,
    },
}

/// Final assertion disposition after proof/debug policy admission.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedAssertionDisposition {
    /// Awaiting compile-time verifier admission. This never enters runtime lowering.
    PendingProof,
    Discharged,
    Runtime(AssertionRuntimePolicy),
    OmittedDebug,
}

/// Semantic role that changes statement lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedStatementRole {
    Ordinary,
    Assertion(CheckedAssertionDisposition),
    Iteration(Box<CheckedIteration>),
    Suspension,
    Yield,
    UnsafeAudit,
}

/// Closed checked fact for one live statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedStatement {
    effects: EffectSet,
    role: CheckedStatementRole,
}

impl CheckedStatement {
    pub const fn new(effects: EffectSet, role: CheckedStatementRole) -> Self {
        Self { effects, role }
    }

    pub const fn effects(&self) -> &EffectSet {
        &self.effects
    }

    pub const fn role(&self) -> &CheckedStatementRole {
        &self.role
    }
}

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
    Source,
    Style,
}

impl CheckedItemRole {
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
            Self::Source => HirItemFamily::Source,
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

    pub const fn role(&self) -> CheckedBindingRole {
        self.role
    }
}
