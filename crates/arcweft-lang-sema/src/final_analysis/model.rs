//! Generation-bound checked semantic fact model.

use super::{
    AssertionRuntimePolicy, CallableDeclarationKey, CharacterDialogueCharacterType,
    CharacterDialogueType, CharacterId, CharacterNominalType, CheckedRichTextReport,
    DeclarationIdentityFamily, DialogueLineId, DialogueTextKey, EffectSet, EnvironmentBindingId,
    ExprId, GenericTypeOwnerId, GenericTypeParameterId, HirFlowIdentity, HirItemFamily, HirLiteral,
    HirName, ItemId, LocalId, PatternId, ProjectNominalDeclaration, ProjectNominalDeclarationId,
    PublicId, SemanticTypeDigest, TypeKind, TypeParameterSubstitutions,
};
use crate::callable::{CallableEvaluatedEffect, CallableLogLevel, CharacterDialoguePatchContext};
use arcweft_core::value::RuntimeAgentField;
use arcweft_interaction_model::dialogue::CharacterDialogueCustomFieldId;
use arcweft_lang_hir::expr::HirCallArgument;
use arcweft_lang_hir::symbol::ExternalDeclarationId;
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
    value: Option<TypeKind>,
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
        Some(Self {
            public_id,
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
        Self {
            public_id: character.as_public_id(),
            family: DeclarationIdentityFamily::Character,
            owner: CheckedProjectItemOwner::External(declaration),
            character: Some(character),
            value: None,
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
        TypeKind::Ref(crate::types::EntityType::new(kind, self.value.clone()))
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
    /// Method selected through an arbitrary value expression. The enclosing
    /// checked Call owns the exact callable identity; this fact retains the
    /// bound-method shape of its final-HIR callee expression.
    Method {
        name: HirName,
    },
    /// Runtime-supplied field of a nominal record carrying the semantic
    /// `#[dialogue_view]` role. The projection identity is selected by the
    /// environment registry, never reconstructed from its field spelling by
    /// compiler or runtime consumers.
    DialogueView {
        projection: crate::dialogue_view::DialogueProjectionCoordinate,
        name: HirName,
    },
    /// Closed Agent protocol record coordinate selected during type checking.
    AgentField {
        field: RuntimeAgentField,
    },
    /// Field owned by the standard `Progress` value family.
    ProgressField {
        field: crate::types::ProgressField,
    },
    Field {
        nominal: Option<CheckedProjectNominal>,
        ordinal: Option<u32>,
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
    /// Exact outcome and continuation contract owned by one Await expression.
    Await(CheckedAwait),
    /// Exact carrier and nearest lexical propagation boundary for prefix Try.
    Try(CheckedTry),
    /// One implicit callable introduced by partial-application placeholders.
    ImplicitCallable(Box<CheckedImplicitCallable>),
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
    },
    PostfixBracket(PostfixBracketResolution),
}

/// Checked implicit callable introduced by one or more `_` placeholders.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedImplicitCallable {
    parameter: TypeKind,
    result: TypeKind,
    placeholders: Box<[ExprId]>,
    captures: Box<[LocalId]>,
    body_resolution: Box<CheckedExpressionResolution>,
}

impl CheckedImplicitCallable {
    pub fn new(
        parameter: TypeKind,
        result: TypeKind,
        placeholders: Box<[ExprId]>,
        captures: Box<[LocalId]>,
        body_resolution: CheckedExpressionResolution,
    ) -> Self {
        Self {
            parameter,
            result,
            placeholders,
            captures,
            body_resolution: Box::new(body_resolution),
        }
    }

    pub const fn parameter(&self) -> &TypeKind {
        &self.parameter
    }

    pub const fn result(&self) -> &TypeKind {
        &self.result
    }

    pub const fn placeholders(&self) -> &[ExprId] {
        &self.placeholders
    }

    pub const fn captures(&self) -> &[LocalId] {
        &self.captures
    }

    pub fn body_resolution(&self) -> &CheckedExpressionResolution {
        self.body_resolution.as_ref()
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
}

/// Stable semantic coordinate selected for one `CharacterDialogue` patch field.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterDialogueFieldCoordinate {
    Voice,
    Look,
    Stage,
    Portrait,
    Focus,
    Cleanup,
    View,
    SourceLocale,
    Hooks,
    Style,
    RichText,
    InlineFailure,
    Custom(CharacterDialogueCustomFieldId),
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

    /// Returns whether this fact is an application-owned semantic coordinate
    /// whose identity is fixed before ordinary-call candidate evaluation.
    ///
    /// Candidate probes still type-check and account for the authored slot,
    /// but must not erase this fact and reinterpret its entity path outside
    /// the owning dialogue application.
    pub(crate) const fn is_candidate_stable_coordinate(&self) -> bool {
        matches!(
            self.resolution,
            CheckedExpressionResolution::DialogueLineCoordinate(_)
                | CheckedExpressionResolution::DialogueTextKeyCoordinate(_)
        )
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

/// Closed writable place admitted for one final-HIR assignment.
///
/// Assignment never defers place interpretation to runtime lowering.  The
/// accepted language surface is deliberately narrow: a direct local binding
/// projected through one field of its checked project nominal record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedAssignmentPlace {
    local: LocalId,
    nominal: CheckedProjectNominal,
    field_ordinal: u32,
    field_type: TypeKind,
}

impl CheckedAssignmentPlace {
    pub const fn new(
        local: LocalId,
        nominal: CheckedProjectNominal,
        field_ordinal: u32,
        field_type: TypeKind,
    ) -> Self {
        Self {
            local,
            nominal,
            field_ordinal,
            field_type,
        }
    }

    pub const fn local(&self) -> LocalId {
        self.local
    }

    pub const fn nominal(&self) -> &CheckedProjectNominal {
        &self.nominal
    }

    pub const fn field_ordinal(&self) -> u32 {
        self.field_ordinal
    }

    pub const fn field_type(&self) -> &TypeKind {
        &self.field_type
    }
}

/// Complete semantic assignment fact for one final-HIR statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedAssignment {
    place: CheckedAssignmentPlace,
    value_type: TypeKind,
}

impl CheckedAssignment {
    pub const fn new(place: CheckedAssignmentPlace, value_type: TypeKind) -> Self {
        Self { place, value_type }
    }

    pub const fn place(&self) -> &CheckedAssignmentPlace {
        &self.place
    }

    pub const fn value_type(&self) -> &TypeKind {
        &self.value_type
    }
}

/// Semantic role that changes statement lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedStatementRole {
    Ordinary,
    Assignment(Box<CheckedAssignment>),
    Assertion(CheckedAssertionDisposition),
    EvaluatedEffect(Box<CheckedEvaluatedEffect>),
    Iteration(Box<CheckedIteration>),
    Suspension(Box<CheckedSuspensionStatement>),
    Yield,
    UnsafeAudit,
}

/// Complete semantic disposition for one suspension statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedSuspensionStatement {
    Wait,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedEffectField {
    name: String,
    value: ExprId,
}

impl CheckedEffectField {
    pub fn new(name: impl Into<String>, value: ExprId) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn value(&self) -> ExprId {
        self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedEvaluatedEffect {
    Log {
        level: CallableLogLevel,
        message: ExprId,
        fields: Box<[CheckedEffectField]>,
    },
    SignalWrite {
        target: ExprId,
        value: ExprId,
    },
    MetricWrite {
        target: ExprId,
        value: ExprId,
    },
    EmitEvent {
        event: ExprId,
        fields: Box<[CheckedEffectField]>,
    },
    Panic {
        message: ExprId,
    },
    Fail {
        message: ExprId,
    },
    Bail {
        message: ExprId,
    },
    Ensure {
        condition: ExprId,
        message: ExprId,
    },
}

impl CheckedEvaluatedEffect {
    pub const fn disposition(&self) -> CallableEvaluatedEffect {
        match self {
            Self::Log { level, .. } => CallableEvaluatedEffect::Log(*level),
            Self::SignalWrite { .. } => CallableEvaluatedEffect::SignalWrite,
            Self::MetricWrite { .. } => CallableEvaluatedEffect::MetricWrite,
            Self::EmitEvent { .. } => CallableEvaluatedEffect::EmitEvent,
            Self::Panic { .. } => CallableEvaluatedEffect::Panic,
            Self::Fail { .. } => CallableEvaluatedEffect::Fail,
            Self::Bail { .. } => CallableEvaluatedEffect::Bail,
            Self::Ensure { .. } => CallableEvaluatedEffect::Ensure,
        }
    }

    pub(crate) fn try_from_call(
        effect: CallableEvaluatedEffect,
        arguments: &[HirCallArgument],
    ) -> Option<Self> {
        match effect {
            CallableEvaluatedEffect::Log(level) => {
                let (message, fields) = checked_message_and_fields(arguments, "message")?;
                Some(Self::Log {
                    level,
                    message,
                    fields,
                })
            }
            CallableEvaluatedEffect::EmitEvent => {
                let (event, fields) = checked_message_and_fields(arguments, "event")?;
                Some(Self::EmitEvent { event, fields })
            }
            CallableEvaluatedEffect::SignalWrite => {
                let values = checked_fixed_effect_arguments(arguments, &["target", "value"], 2)?;
                Some(Self::SignalWrite {
                    target: values[0]?,
                    value: values[1]?,
                })
            }
            CallableEvaluatedEffect::MetricWrite => {
                let values = checked_fixed_effect_arguments(arguments, &["target", "value"], 2)?;
                Some(Self::MetricWrite {
                    target: values[0]?,
                    value: values[1]?,
                })
            }
            CallableEvaluatedEffect::Panic
            | CallableEvaluatedEffect::Fail
            | CallableEvaluatedEffect::Bail => {
                let values = checked_fixed_effect_arguments(arguments, &["message"], 1)?;
                let message = values[0]?;
                Some(match effect {
                    CallableEvaluatedEffect::Panic => Self::Panic { message },
                    CallableEvaluatedEffect::Fail => Self::Fail { message },
                    CallableEvaluatedEffect::Bail => Self::Bail { message },
                    _ => unreachable!("the grouped evaluated-effect family is exhaustive"),
                })
            }
            CallableEvaluatedEffect::Ensure => {
                let values =
                    checked_fixed_effect_arguments(arguments, &["condition", "message"], 2)?;
                Some(Self::Ensure {
                    condition: values[0]?,
                    message: values[1]?,
                })
            }
        }
    }
}

fn checked_message_and_fields(
    arguments: &[HirCallArgument],
    head_name: &str,
) -> Option<(ExprId, Box<[CheckedEffectField]>)> {
    let mut head = None;
    let mut names = Vec::<String>::new();
    let mut fields = Vec::new();
    for (ordinal, argument) in arguments.iter().enumerate() {
        match argument {
            HirCallArgument::Spread { .. } => return None,
            HirCallArgument::Named { .. }
                if argument
                    .resolved_name()
                    .is_some_and(|name| name.as_str() == head_name) =>
            {
                if head.replace(argument.value()).is_some() {
                    return None;
                }
            }
            HirCallArgument::Positional { .. } if head.is_none() => {
                head = Some(argument.value());
            }
            HirCallArgument::Named { .. } | HirCallArgument::Positional { .. } => {
                let name = argument
                    .resolved_name()
                    .map_or_else(|| format!("arg{ordinal}"), |name| name.as_str().to_owned());
                if names.contains(&name) {
                    return None;
                }
                names.push(name.clone());
                fields.push(CheckedEffectField::new(name, argument.value()));
            }
        }
    }
    Some((head?, fields.into_boxed_slice()))
}

fn checked_fixed_effect_arguments(
    arguments: &[HirCallArgument],
    names: &[&str],
    required: usize,
) -> Option<Vec<Option<ExprId>>> {
    let mut values = vec![None; names.len()];
    let mut next_positional = 0;
    for argument in arguments {
        let index = match argument {
            HirCallArgument::Positional { .. } => {
                while values.get(next_positional).is_some_and(Option::is_some) {
                    next_positional += 1;
                }
                let index = next_positional;
                next_positional += 1;
                index
            }
            HirCallArgument::Named { .. } => names.iter().position(|candidate| {
                argument
                    .resolved_name()
                    .is_some_and(|name| *candidate == name.as_str())
            })?,
            HirCallArgument::Spread { .. } => return None,
        };
        let slot = values.get_mut(index)?;
        if slot.replace(argument.value()).is_some() {
            return None;
        }
    }
    values
        .iter()
        .take(required)
        .all(Option::is_some)
        .then_some(values)
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
