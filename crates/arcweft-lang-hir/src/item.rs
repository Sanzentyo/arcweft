//! Final source-item and declaration-member records.
//!
//! Every semantic child is an existing qualified HIR ID or an HIR-owned
//! value. Revision-bound ranges and exact syntax identities remain in the
//! source index. The declaration-member arena uses the owning [`ItemId`] plus
//! a zero-based source ordinal and deliberately does not add a ninth raw HIR
//! ID kind.

use arcweft_id::DeclarationIdentityFamily;
use thiserror::Error;

use crate::expr::HirCallArgument;
use crate::identity::{ExprId, HirModuleId, ItemId, LocalId, PatternId, ScopeId, StmtId, TypeId};
use crate::leaf::{HirName, HirPath, HirPathValue};

mod callable;
mod entry;
mod flow;
mod host;
mod member_index;
mod nominal;
mod retained;
mod source;
mod trait_impl;

pub use self::host::{
    HirCapabilityAssociatedType, HirCapabilityFunction, HirCapabilityMember, HirErrorItem,
    HirExternCapabilityItem, HirItemIssue,
};
pub(crate) use self::member_index::HirDeclarationMemberIndexBuilder;
pub use self::member_index::{HirDeclarationMemberIndex, HirDeclarationMemberIndexResolveError};
pub use self::retained::{
    HirAccessibilityPolicy, HirActionDeclaration, HirActivityDeclaration, HirActivityLifecycle,
    HirActivityMode, HirActivityPortMember, HirCapturePolicy, HirCharacterAssignmentState,
    HirCharacterDeclaration, HirCharacterDisplayNameMember, HirCharacterMemberRecovery,
    HirCharacterSurfaceAlias, HirDeclarationMember, HirDeclarationMemberArena,
    HirDeclarationMemberId, HirDeclarationMemberIssue, HirDeclarationMemberKind,
    HirDeclarationMemberPoisonState, HirDeclarationMemberResolveError, HirHitTestPolicy,
    HirInputPolicy, HirLayerAssignmentState, HirLayerDeclaration, HirLayerExpressionMember,
    HirLayerKind, HirLayerKindIssue, HirLayerMemberPayload, HirLayerMemberValue,
    HirLayerPolicyMember, HirLayerReferenceMember, HirMetricAssignmentState,
    HirMetricBucketsMember, HirMetricBucketsValue, HirMetricDeclaration, HirMetricKind,
    HirMetricKindIssue, HirMetricLabelMember, HirMetricUnitMember, HirMetricUnitValue,
    HirPublicIdOrigin, HirRenderPhase, HirRetainedHeader, HirRetainedName, HirRetainedPublicId,
    HirRetainedPublicIdIssue, HirSignalDeclaration, HirViewDeclaration, HirViewExportMember,
};
pub use self::trait_impl::{
    HirImplAssociatedType, HirImplFunction, HirImplItem, HirImplMember, HirMethodParameter,
    HirMethodParameterGroup, HirMethodReceiver, HirMethodReceiverKind, HirTraitAssociatedType,
    HirTraitFunction, HirTraitItem, HirTraitMember,
};

pub use self::callable::{
    HirCallableSignature, HirContractScopes, HirFunctionBody, HirFunctionItem,
    HirFunctionParameterGroup, HirFunctionSignature, HirGenericParameter, HirParameter,
    HirParameterKind, HirPredicate, HirPredicateBody, HirProof, HirProofBody, HirWherePredicate,
    ProofTrust, TrustReason, TrustReasonError,
};
pub use self::entry::{
    HirEntryBody, HirEntryDeclaration, HirEntryGoto, HirEntryId, HirEntryKind, HirEntryKindIssue,
    HirEntryMember, HirEntryOption, HirEntryOptionValue, HirEntryPathBinding, HirEntryPathValue,
    HirEntryPunctuationState, HirEntryRoute, HirEntryRouteBinding, HirEntryRouteBindings,
    HirEntryTarget, HirEntryTypeBinding, HirHttpMethod, HirHttpMethodIssue, HirHttpMethodValue,
    HirRoutePath, HirRoutePathIssue, HirRoutePathValue,
};
pub use self::flow::{
    HirContractCondition, HirContractMode, HirContractOperandList, HirFlowContractClause,
    HirFlowIdentity, HirFlowIssue, HirFlowIssueClass, HirFlowIssueOwner, HirFlowItem,
    HirFlowPoison, HirFlowResultLocal, HirFlowReturn,
};
pub use self::host::{
    HirBenchItem, HirStyleAssignOperation, HirStyleAssignOperationIssue, HirStyleBodyIssue,
    HirStyleBodyItem, HirStyleCombinator, HirStyleDeclaration, HirStyleEnvironment,
    HirStyleEnvironmentClause, HirStyleEnvironmentComparison, HirStyleEnvironmentComparisonIssue,
    HirStyleEnvironmentField, HirStyleEnvironmentFieldIssue, HirStyleItem, HirStyleName,
    HirStyleNameIssue, HirStyleRule, HirStyleSelector, HirStyleSelectorIssue,
    HirStyleSelectorSequence, HirStyleToken, HirStyleTokenIssue, HirTestItem, HirTestKind,
    HirTestKindIssue,
};
pub use self::nominal::{
    HirEnumItem, HirEnumVariant, HirResourceDeclaration, HirResourceField, HirStructField,
    HirStructItem, HirTypeAliasItem,
};
pub use self::source::{
    HirSourceBackpressurePolicy, HirSourceBackpressureValue, HirSourceBody,
    HirSourceBoundedArgument, HirSourceChildState, HirSourceEventIssue, HirSourceEventPattern,
    HirSourceExpressionValue, HirSourceHandler, HirSourceHandlerBody, HirSourceHeaders,
    HirSourceId, HirSourceItem, HirSourceOverflowPolicy, HirSourceOverflowValue,
    HirSourcePatternValue, HirSourcePolicyBinding, HirSourcePolicyIssue, HirSourcePrivacyPolicy,
    HirSourcePrivacyValue, HirSourcePunctuationState, HirSourceReplayPolicy, HirSourceReplayValue,
    HirSourceRequiredSlot,
};
/// Exact source-backed top-level item inventory.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirItemFamily {
    Module,
    Use,
    Flow,
    Function,
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
    Error,
}

impl HirItemFamily {
    /// All source item families in grammar inventory order.
    pub const ALL: [Self; 26] = [
        Self::Module,
        Self::Use,
        Self::Flow,
        Self::Function,
        Self::Predicate,
        Self::Proof,
        Self::Trait,
        Self::Impl,
        Self::Enum,
        Self::Struct,
        Self::TypeAlias,
        Self::Resource,
        Self::Character,
        Self::View,
        Self::Action,
        Self::Activity,
        Self::Signal,
        Self::Metric,
        Self::Layer,
        Self::Entry,
        Self::ExternCapability,
        Self::Test,
        Self::Bench,
        Self::Source,
        Self::Style,
        Self::Error,
    ];
}

/// One immutable item-arena record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirItem {
    scope: ScopeId,
    prefix: HirItemPrefix,
    kind: HirItemKind,
    members: Box<[HirDeclarationMemberId]>,
    state: HirItemPoisonState,
}

impl HirItem {
    pub(crate) fn try_new(
        owner: ItemId,
        scope: ScopeId,
        prefix: HirItemPrefix,
        kind: HirItemKind,
        members: Box<[HirDeclarationMemberId]>,
    ) -> Result<Self, HirItemInvariantError> {
        let state = if matches!(kind, HirItemKind::Error(_)) {
            HirItemPoisonState::Poisoned(HirItemIssue::UnclassifiedSyntax)
        } else {
            HirItemPoisonState::Clean
        };
        Self::try_new_with_state(owner, scope, prefix, kind, members, state)
    }

    pub(crate) fn try_new_with_state(
        owner: ItemId,
        scope: ScopeId,
        prefix: HirItemPrefix,
        kind: HirItemKind,
        members: Box<[HirDeclarationMemberId]>,
        state: HirItemPoisonState,
    ) -> Result<Self, HirItemInvariantError> {
        validate_module(owner.module(), scope.module())?;
        prefix.validate_module(owner.module())?;
        kind.validate_module(owner.module())?;
        validate_member_ids(owner, kind.family(), &members)?;
        kind.validate_member_row(owner, &members)?;
        if !item_state_matches_kind(&kind, state) {
            return Err(HirItemInvariantError::InvalidPoisonState);
        }
        Ok(Self {
            scope,
            prefix,
            kind,
            members,
            state,
        })
    }

    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    pub const fn prefix(&self) -> &HirItemPrefix {
        &self.prefix
    }

    pub const fn kind(&self) -> &HirItemKind {
        &self.kind
    }

    pub const fn family(&self) -> HirItemFamily {
        self.kind.family()
    }

    pub const fn members(&self) -> &[HirDeclarationMemberId] {
        &self.members
    }

    /// Returns the typed recovery state retained by this item family.
    pub const fn state(&self) -> &HirItemPoisonState {
        &self.state
    }

    pub const fn is_poisoned(&self) -> bool {
        self.state.is_poisoned()
    }

    /// Returns the typed roots whose descendants belong to a presentation
    /// product rather than runtime semantic fact publication.
    ///
    /// This match is intentionally exhaustive. Adding an item family must
    /// reconsider whether its typed owners enter runtime lowering.
    pub(crate) fn presentation_semantic_roots(&self) -> Option<HirPresentationSemanticRoots> {
        let mut attribute_expressions = self.prefix.attribute_expression_roots();
        match &self.kind {
            HirItemKind::View(view) => Some(HirPresentationSemanticRoots {
                scope: Some(view.callable_scope()),
                expressions: attribute_expressions,
                types: Vec::new(),
            }),
            HirItemKind::Style(style) => {
                attribute_expressions.extend(style.value_expression_roots());
                Some(HirPresentationSemanticRoots {
                    scope: None,
                    expressions: attribute_expressions,
                    types: style.value_type_roots(),
                })
            }
            HirItemKind::Module(_)
            | HirItemKind::Use(_)
            | HirItemKind::Flow(_)
            | HirItemKind::Function(_)
            | HirItemKind::Predicate(_)
            | HirItemKind::Proof(_)
            | HirItemKind::Trait(_)
            | HirItemKind::Impl(_)
            | HirItemKind::Enum(_)
            | HirItemKind::Struct(_)
            | HirItemKind::TypeAlias(_)
            | HirItemKind::Resource(_)
            | HirItemKind::Character(_)
            | HirItemKind::Action(_)
            | HirItemKind::Activity(_)
            | HirItemKind::Signal(_)
            | HirItemKind::Metric(_)
            | HirItemKind::Layer(_)
            | HirItemKind::Entry(_)
            | HirItemKind::ExternCapability(_)
            | HirItemKind::Test(_)
            | HirItemKind::Bench(_)
            | HirItemKind::Source(_)
            | HirItemKind::Error(_) => None,
        }
    }
}

/// Typed entry roots for one presentation-owned item product.
pub(crate) struct HirPresentationSemanticRoots {
    scope: Option<ScopeId>,
    expressions: Vec<ExprId>,
    types: Vec<TypeId>,
}

impl HirPresentationSemanticRoots {
    pub(crate) const fn scope(&self) -> Option<ScopeId> {
        self.scope
    }

    pub(crate) fn expressions(&self) -> impl Iterator<Item = ExprId> + '_ {
        self.expressions.iter().copied()
    }

    pub(crate) fn types(&self) -> impl Iterator<Item = TypeId> + '_ {
        self.types.iter().copied()
    }
}

fn item_state_matches_kind(kind: &HirItemKind, state: HirItemPoisonState) -> bool {
    if matches!(kind, HirItemKind::Error(_)) {
        return matches!(
            state,
            HirItemPoisonState::Poisoned(
                HirItemIssue::UnclassifiedSyntax | HirItemIssue::TransactionalChildFailure
            )
        );
    }
    if matches!(
        state,
        HirItemPoisonState::Poisoned(
            HirItemIssue::UnclassifiedSyntax | HirItemIssue::TransactionalChildFailure
        )
    ) {
        return false;
    }

    match kind {
        HirItemKind::Module(module) => module.path().recovery().is_none() || state.is_poisoned(),
        HirItemKind::Use(declaration) => !declaration.has_recovery() || state.is_poisoned(),
        HirItemKind::Flow(declaration) => declaration.has_recovery() == state.is_poisoned(),
        HirItemKind::View(declaration) => !declaration.has_recovery() || state.is_poisoned(),
        HirItemKind::Layer(declaration) => !declaration.has_recovery() || state.is_poisoned(),
        HirItemKind::Entry(declaration) => {
            !declaration.has_structural_recovery() || state.is_poisoned()
        }
        HirItemKind::Source(declaration) => {
            !declaration.has_structural_recovery() || state.is_poisoned()
        }
        HirItemKind::Style(declaration) => !declaration.has_recovery() || state.is_poisoned(),
        HirItemKind::ExternCapability(declaration) => {
            !declaration.has_recovery() || state.is_poisoned()
        }
        HirItemKind::Trait(declaration) => {
            !declaration.has_structural_recovery() || state.is_poisoned()
        }
        HirItemKind::Impl(declaration) => {
            !declaration.has_structural_recovery() || state.is_poisoned()
        }
        HirItemKind::Test(declaration) => !declaration.has_recovery() || state.is_poisoned(),
        HirItemKind::Bench(declaration) => !declaration.has_recovery() || state.is_poisoned(),
        _ => true,
    }
}

impl crate::arena::HirArenaPayload for HirItem {
    fn is_poisoned(&self) -> bool {
        self.is_poisoned()
    }
}

/// Executability state of one recognized source-item family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirItemPoisonState {
    Clean,
    Poisoned(HirItemIssue),
}

impl HirItemPoisonState {
    pub const fn is_poisoned(&self) -> bool {
        matches!(self, Self::Poisoned(_))
    }
}

/// Common typed declaration prefix stored exactly once per item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirItemPrefix {
    documentation: Option<HirDocumentation>,
    attributes: Box<[HirAttribute]>,
    visibility: Option<HirVisibility>,
}

impl HirItemPrefix {
    pub(crate) const fn new(
        documentation: Option<HirDocumentation>,
        attributes: Box<[HirAttribute]>,
        visibility: Option<HirVisibility>,
    ) -> Self {
        Self {
            documentation,
            attributes,
            visibility,
        }
    }

    pub const fn documentation(&self) -> Option<&HirDocumentation> {
        self.documentation.as_ref()
    }

    pub const fn attributes(&self) -> &[HirAttribute] {
        &self.attributes
    }

    pub const fn visibility(&self) -> Option<HirVisibility> {
        self.visibility
    }

    fn attribute_expression_roots(&self) -> Vec<ExprId> {
        self.attributes
            .iter()
            .flat_map(HirAttribute::arguments)
            .map(HirCallArgument::value)
            .collect()
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        for attribute in &self.attributes {
            validate_call_arguments(expected, attribute.arguments())?;
        }
        Ok(())
    }
}

/// HIR-owned documentation content without source coordinates.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDocumentation(Box<str>);

impl HirDocumentation {
    pub(crate) const fn new(markdown: Box<str>) -> Self {
        Self(markdown)
    }

    pub fn markdown(&self) -> &str {
        &self.0
    }
}

/// Required ordinary declaration/member name without a fabricated recovery spelling.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRequiredName {
    Resolved(HirName),
    Missing,
    Invalid,
}

impl HirRequiredName {
    pub const fn resolved(&self) -> Option<&HirName> {
        match self {
            Self::Resolved(name) => Some(name),
            Self::Missing | Self::Invalid => None,
        }
    }

    pub const fn is_recovered(&self) -> bool {
        !matches!(self, Self::Resolved(_))
    }
}

/// One structured outer attribute.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirAttribute {
    path: HirPath,
    arguments: Box<[HirCallArgument]>,
}

impl HirAttribute {
    pub(crate) const fn new(path: HirPath, arguments: Box<[HirCallArgument]>) -> Self {
        Self { path, arguments }
    }

    pub const fn path(&self) -> &HirPath {
        &self.path
    }

    pub const fn arguments(&self) -> &[HirCallArgument] {
        &self.arguments
    }
}

/// HIR-owned source visibility.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirVisibility {
    Public,
    Crate,
    Super,
}

/// Closed final payload for each source item family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirItemKind {
    Module(HirModuleDeclaration),
    Use(HirUseDeclaration),
    Flow(HirFlowItem),
    Function(HirFunctionItem),
    Predicate(HirPredicate),
    Proof(HirProof),
    Trait(HirTraitItem),
    Impl(HirImplItem),
    Enum(HirEnumItem),
    Struct(HirStructItem),
    TypeAlias(HirTypeAliasItem),
    Resource(HirResourceDeclaration),
    Character(HirCharacterDeclaration),
    View(HirViewDeclaration),
    Action(HirActionDeclaration),
    Activity(HirActivityDeclaration),
    Signal(HirSignalDeclaration),
    Metric(HirMetricDeclaration),
    Layer(HirLayerDeclaration),
    Entry(HirEntryDeclaration),
    ExternCapability(HirExternCapabilityItem),
    Test(HirTestItem),
    Bench(HirBenchItem),
    Source(HirSourceItem),
    Style(HirStyleItem),
    Error(HirErrorItem),
}

impl HirItemKind {
    pub const fn family(&self) -> HirItemFamily {
        match self {
            Self::Module(_) => HirItemFamily::Module,
            Self::Use(_) => HirItemFamily::Use,
            Self::Flow(_) => HirItemFamily::Flow,
            Self::Function(_) => HirItemFamily::Function,
            Self::Predicate(_) => HirItemFamily::Predicate,
            Self::Proof(_) => HirItemFamily::Proof,
            Self::Trait(_) => HirItemFamily::Trait,
            Self::Impl(_) => HirItemFamily::Impl,
            Self::Enum(_) => HirItemFamily::Enum,
            Self::Struct(_) => HirItemFamily::Struct,
            Self::TypeAlias(_) => HirItemFamily::TypeAlias,
            Self::Resource(_) => HirItemFamily::Resource,
            Self::Character(_) => HirItemFamily::Character,
            Self::View(_) => HirItemFamily::View,
            Self::Action(_) => HirItemFamily::Action,
            Self::Activity(_) => HirItemFamily::Activity,
            Self::Signal(_) => HirItemFamily::Signal,
            Self::Metric(_) => HirItemFamily::Metric,
            Self::Layer(_) => HirItemFamily::Layer,
            Self::Entry(_) => HirItemFamily::Entry,
            Self::ExternCapability(_) => HirItemFamily::ExternCapability,
            Self::Test(_) => HirItemFamily::Test,
            Self::Bench(_) => HirItemFamily::Bench,
            Self::Source(_) => HirItemFamily::Source,
            Self::Style(_) => HirItemFamily::Style,
            Self::Error(_) => HirItemFamily::Error,
        }
    }

    /// Returns every authored effect-identity expression owned by this item in
    /// source order.
    ///
    /// Conditions and state/resource contract operands are deliberately not
    /// effect identities. Flow `effects` operands and `no_effect` operands do
    /// share the same typed effect projection as ordinary Function and extern
    /// capability effect clauses.
    pub fn effect_expression_roots(&self) -> Vec<ExprId> {
        match self {
            Self::Function(function) => function
                .effect_clauses()
                .iter()
                .flat_map(HirContractOperandList::operands)
                .copied()
                .collect(),
            Self::Flow(flow) => flow
                .contracts()
                .iter()
                .flat_map(|contract| match contract {
                    HirFlowContractClause::Effects(operands) => operands.operands(),
                    HirFlowContractClause::NoEffect { expression } => {
                        std::slice::from_ref(expression)
                    }
                    HirFlowContractClause::Requires(_)
                    | HirFlowContractClause::Ensures(_)
                    | HirFlowContractClause::Invariant(_)
                    | HirFlowContractClause::Assume { .. }
                    | HirFlowContractClause::Reads(_)
                    | HirFlowContractClause::Modifies(_)
                    | HirFlowContractClause::Decreases { .. } => &[],
                })
                .copied()
                .collect(),
            Self::ExternCapability(capability) => capability
                .members()
                .iter()
                .filter_map(|member| match member {
                    HirCapabilityMember::Function(function) => Some(function.effects()),
                    HirCapabilityMember::AssociatedType(_) | HirCapabilityMember::Error => None,
                })
                .flatten()
                .copied()
                .collect(),
            Self::Module(_)
            | Self::Use(_)
            | Self::Predicate(_)
            | Self::Proof(_)
            | Self::Trait(_)
            | Self::Impl(_)
            | Self::Enum(_)
            | Self::Struct(_)
            | Self::TypeAlias(_)
            | Self::Resource(_)
            | Self::Character(_)
            | Self::View(_)
            | Self::Action(_)
            | Self::Activity(_)
            | Self::Signal(_)
            | Self::Metric(_)
            | Self::Layer(_)
            | Self::Entry(_)
            | Self::Test(_)
            | Self::Bench(_)
            | Self::Source(_)
            | Self::Style(_)
            | Self::Error(_) => Vec::new(),
        }
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        match self {
            Self::Module(_) | Self::Use(_) | Self::Error(_) => Ok(()),
            Self::Flow(item) => item.validate_module(expected),
            Self::Function(item) => item.validate_module(expected),
            Self::Predicate(item) => item.validate_module(expected),
            Self::Proof(item) => item.validate_module(expected),
            Self::Trait(item) => item.validate_module(expected),
            Self::Impl(item) => item.validate_module(expected),
            Self::Enum(item) => item.validate_module(expected),
            Self::Struct(item) => item.validate_module(expected),
            Self::TypeAlias(item) => item.validate_module(expected),
            Self::Resource(item) => item.validate_module(expected),
            Self::Character(item) => item.validate_module(expected),
            Self::View(item) => item.validate_module(expected),
            Self::Action(item) => item.validate_module(expected),
            Self::Activity(item) => item.validate_module(expected),
            Self::Signal(item) => item.validate_module(expected),
            Self::Metric(item) => item.validate_module(expected),
            Self::Layer(item) => item.validate_module(expected),
            Self::Entry(item) => item.validate_module(expected),
            Self::ExternCapability(item) => item.validate_module(expected),
            Self::Test(item) => item.validate_module(expected),
            Self::Bench(item) => item.validate_module(expected),
            Self::Source(item) => item.validate_module(expected),
            Self::Style(item) => item.validate_module(expected),
        }
    }

    fn validate_member_row(
        &self,
        owner: ItemId,
        members: &[HirDeclarationMemberId],
    ) -> Result<(), HirItemInvariantError> {
        match self {
            Self::View(view) => view.validate_member_row(owner, members),
            Self::Layer(layer) => layer.validate_member_row(owner, members),
            _ => Ok(()),
        }
    }
}

/// One module declaration path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirModuleDeclaration {
    path: HirPathValue,
}

impl HirModuleDeclaration {
    pub(crate) const fn new(path: HirPathValue) -> Self {
        Self { path }
    }

    pub const fn path(&self) -> &HirPathValue {
        &self.path
    }
}

/// One flattened semantic use declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirUseDeclaration {
    bindings: Box<[HirUseBinding]>,
}

impl HirUseDeclaration {
    pub(crate) fn try_new(bindings: Box<[HirUseBinding]>) -> Result<Self, HirItemInvariantError> {
        if bindings.is_empty() {
            return Err(HirItemInvariantError::EmptyUseDeclaration);
        }
        Ok(Self { bindings })
    }

    pub const fn bindings(&self) -> &[HirUseBinding] {
        &self.bindings
    }

    pub(crate) const fn recovered(bindings: Box<[HirUseBinding]>) -> Self {
        Self { bindings }
    }

    fn has_recovery(&self) -> bool {
        self.bindings.is_empty() || self.bindings.iter().any(HirUseBinding::has_recovery)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirUseBinding {
    path: HirPathValue,
    alias: Option<HirName>,
    kind: HirUseBindingKind,
}

impl HirUseBinding {
    pub(crate) const fn new(
        path: HirPathValue,
        alias: Option<HirName>,
        kind: HirUseBindingKind,
    ) -> Self {
        Self { path, alias, kind }
    }

    pub const fn path(&self) -> &HirPathValue {
        &self.path
    }

    pub const fn alias(&self) -> Option<&HirName> {
        self.alias.as_ref()
    }

    pub const fn kind(&self) -> HirUseBindingKind {
        self.kind
    }

    const fn has_recovery(&self) -> bool {
        self.path.recovery().is_some()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirUseBindingKind {
    Item,
    Glob,
}

/// Item/member construction failure detected before transaction publication.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum HirItemInvariantError {
    #[error("HIR item child belongs to module {actual:?}, expected {expected:?}")]
    ForeignChild {
        expected: HirModuleId,
        actual: HirModuleId,
    },
    #[error("Flow parameter uses a callable shape not admitted by Flow")]
    InvalidFlowParameterShape,
    #[error("Flow callable, contract, and body scopes must have distinct identities")]
    FlowScopeIdentityCollision,
    #[error("Flow result-local presence disagrees with its Ensures clauses")]
    InvalidFlowResultLocal,
    #[error("Flow poison references item {actual:?}, expected {expected:?}")]
    FlowIssueItemOwner { expected: ItemId, actual: ItemId },
    #[error("Flow poison contains related issues without a primary issue")]
    InvalidFlowPoison,
    #[error("source declaration has neither a retained ID nor an ordinary name")]
    MissingSourceIdentity,
    #[error("source recovery payload is inconsistent with its typed issue")]
    InvalidSourceRecovery,
    #[error("a clean use declaration requires at least one flattened binding")]
    EmptyUseDeclaration,
    #[error("a HIR where predicate requires at least one typed bound")]
    EmptyWhereBounds,
    #[error("an ordinary Function requires at least one parameter group")]
    EmptyFunctionParameterGroups,
    #[error("a Trait/Impl method requires at least one parameter group")]
    EmptyMethodParameterGroups,
    #[error("a method receiver must bind exactly one local, got {actual}")]
    MethodReceiverBindingCount { actual: usize },
    #[error("method body scope {body:?} does not match callable scope {callable:?}")]
    MethodBodyScopeMismatch { callable: ScopeId, body: ScopeId },
    #[error("item payload and typed poison state disagree")]
    InvalidPoisonState,
    #[error("Action parameters cannot have defaults")]
    ActionParameterDefault,
    #[error("View parameters must use the fixed-parameter shape")]
    ViewParameterShape,
    #[error("Activity port member belongs to {actual:?}, expected {expected:?}")]
    ActivityPortOwner { expected: ItemId, actual: ItemId },
    #[error("Activity input/output rows contain the same declaration member")]
    DuplicateActivityPortMember,
    #[error("Activity port resolved-name and local ownership disagree")]
    ActivityPortLocalMismatch,
    #[error("declaration member belongs to {actual:?}, expected {expected:?}")]
    DeclarationMemberOwner { expected: ItemId, actual: ItemId },
    #[error("declaration member {member:?} is referenced more than once by its item payload")]
    DuplicateDeclarationMemberReference { member: HirDeclarationMemberId },
    #[error("callable, requires, and ensures scopes must have distinct identities")]
    ContractScopeIdentityCollision,
    #[error("retained item family does not match its retained header")]
    RetainedFamilyMismatch,
    #[error("declaration member ordinal cannot represent source position {position}")]
    MemberOrdinalOverflow { position: usize },
    #[error("declaration members are not contiguous: expected {expected:?}, got {actual:?}")]
    NonContiguousMember {
        expected: HirDeclarationMemberId,
        actual: HirDeclarationMemberId,
    },
    #[error("declaration member {member:?} is not admitted by item family {family:?}")]
    WrongMemberFamily {
        member: HirDeclarationMemberId,
        family: HirItemFamily,
    },
    #[error("item family {family:?} cannot own declaration members")]
    MembersNotAllowed { family: HirItemFamily },
    #[error("module member index already contains an arena for {owner:?}")]
    DuplicateMemberArenaOwner { owner: ItemId },
    #[error("member arena owner does not match staged item: expected {expected:?}, got {actual:?}")]
    MemberArenaOwnerMismatch { expected: ItemId, actual: ItemId },
    #[error(
        "member arena family does not match item {owner:?}: item {item_family:?}, arena {arena_family:?}"
    )]
    MemberArenaFamilyMismatch {
        owner: ItemId,
        item_family: HirItemFamily,
        arena_family: HirItemFamily,
    },
    #[error("item {owner:?} has no declaration members and must not publish an empty arena")]
    MemberArenaNotRequired { owner: ItemId },
    #[error("member arena does not preserve the exact member order of item {owner:?}")]
    MemberArenaItemOrderMismatch { owner: ItemId },
    #[error("item payload does not preserve the exact declaration-member row of item {owner:?}")]
    ItemPayloadMemberRowMismatch { owner: ItemId },
    #[error("entry route path must be absolute and contain no control characters")]
    InvalidRoutePath,
    #[error("entry recovery payload is inconsistent with its typed issue")]
    InvalidEntryRecovery,
}

fn validate_module(
    expected: HirModuleId,
    actual: HirModuleId,
) -> Result<(), HirItemInvariantError> {
    if actual == expected {
        Ok(())
    } else {
        Err(HirItemInvariantError::ForeignChild { expected, actual })
    }
}

fn validate_expr(expected: HirModuleId, expression: ExprId) -> Result<(), HirItemInvariantError> {
    validate_module(expected, expression.module())
}

fn validate_optional_expr(
    expected: HirModuleId,
    expression: Option<ExprId>,
) -> Result<(), HirItemInvariantError> {
    if let Some(expression) = expression {
        validate_expr(expected, expression)?;
    }
    Ok(())
}

fn validate_exprs(
    expected: HirModuleId,
    expressions: &[ExprId],
) -> Result<(), HirItemInvariantError> {
    for expression in expressions {
        validate_expr(expected, *expression)?;
    }
    Ok(())
}

fn validate_type(expected: HirModuleId, ty: TypeId) -> Result<(), HirItemInvariantError> {
    validate_module(expected, ty.module())
}

fn validate_optional_type(
    expected: HirModuleId,
    ty: Option<TypeId>,
) -> Result<(), HirItemInvariantError> {
    if let Some(ty) = ty {
        validate_type(expected, ty)?;
    }
    Ok(())
}

fn validate_types(expected: HirModuleId, types: &[TypeId]) -> Result<(), HirItemInvariantError> {
    for ty in types {
        validate_type(expected, *ty)?;
    }
    Ok(())
}

fn validate_pattern(
    expected: HirModuleId,
    pattern: PatternId,
) -> Result<(), HirItemInvariantError> {
    validate_module(expected, pattern.module())
}

fn validate_optional_pattern(
    expected: HirModuleId,
    pattern: Option<PatternId>,
) -> Result<(), HirItemInvariantError> {
    if let Some(pattern) = pattern {
        validate_pattern(expected, pattern)?;
    }
    Ok(())
}

fn validate_scope(expected: HirModuleId, scope: ScopeId) -> Result<(), HirItemInvariantError> {
    validate_module(expected, scope.module())
}

fn validate_statements(
    expected: HirModuleId,
    statements: &[StmtId],
) -> Result<(), HirItemInvariantError> {
    for statement in statements {
        validate_module(expected, statement.module())?;
    }
    Ok(())
}

fn validate_locals(expected: HirModuleId, locals: &[LocalId]) -> Result<(), HirItemInvariantError> {
    for local in locals {
        validate_module(expected, local.module())?;
    }
    Ok(())
}

fn validate_call_arguments(
    expected: HirModuleId,
    arguments: &[HirCallArgument],
) -> Result<(), HirItemInvariantError> {
    for argument in arguments {
        validate_expr(expected, argument.value())?;
    }
    Ok(())
}

fn validate_generic_parameters(
    expected: HirModuleId,
    parameters: &[HirGenericParameter],
) -> Result<(), HirItemInvariantError> {
    for parameter in parameters {
        validate_types(expected, parameter.bounds())?;
    }
    Ok(())
}

fn validate_parameters(
    expected: HirModuleId,
    parameters: &[HirParameter],
) -> Result<(), HirItemInvariantError> {
    for parameter in parameters {
        validate_pattern(expected, parameter.pattern())?;
        validate_type(expected, parameter.ty())?;
        validate_optional_expr(expected, parameter.default())?;
        validate_locals(expected, parameter.locals())?;
    }
    Ok(())
}

fn validate_function_parameter_groups(
    expected: HirModuleId,
    groups: &[HirFunctionParameterGroup],
) -> Result<(), HirItemInvariantError> {
    if groups.is_empty() {
        return Err(HirItemInvariantError::EmptyFunctionParameterGroups);
    }
    for group in groups {
        validate_parameters(expected, group.parameters())?;
    }
    Ok(())
}

fn validate_where_predicates(
    expected: HirModuleId,
    predicates: &[HirWherePredicate],
) -> Result<(), HirItemInvariantError> {
    for predicate in predicates {
        validate_type(expected, predicate.subject())?;
        validate_types(expected, predicate.bounds())?;
    }
    Ok(())
}

fn validate_signature(
    expected: HirModuleId,
    generic_parameters: &[HirGenericParameter],
    parameters: &[HirParameter],
    where_predicates: &[HirWherePredicate],
    requires: &[ExprId],
    ensures: &[ExprId],
    return_type: TypeId,
) -> Result<(), HirItemInvariantError> {
    validate_generic_parameters(expected, generic_parameters)?;
    validate_parameters(expected, parameters)?;
    validate_where_predicates(expected, where_predicates)?;
    validate_exprs(expected, requires)?;
    validate_exprs(expected, ensures)?;
    validate_type(expected, return_type)
}

#[allow(
    clippy::too_many_arguments,
    reason = "the validator mirrors the closed ordinary-function signature schema without an intermediate carrier"
)]
fn validate_function_signature(
    expected: HirModuleId,
    generic_parameters: &[HirGenericParameter],
    parameter_groups: &[HirFunctionParameterGroup],
    where_predicates: &[HirWherePredicate],
    requires: &[ExprId],
    ensures: &[ExprId],
    effects: &[HirContractOperandList],
    return_type: Option<TypeId>,
) -> Result<(), HirItemInvariantError> {
    validate_generic_parameters(expected, generic_parameters)?;
    validate_function_parameter_groups(expected, parameter_groups)?;
    validate_where_predicates(expected, where_predicates)?;
    validate_exprs(expected, requires)?;
    validate_exprs(expected, ensures)?;
    for effect_clause in effects {
        validate_exprs(expected, effect_clause.operands())?;
    }
    validate_optional_type(expected, return_type)
}

fn validate_contract_scopes(
    expected: HirModuleId,
    callable: ScopeId,
    requires: ScopeId,
    ensures: ScopeId,
) -> Result<(), HirItemInvariantError> {
    validate_scope(expected, callable)?;
    validate_scope(expected, requires)?;
    validate_scope(expected, ensures)?;
    if callable == requires || callable == ensures || requires == ensures {
        return Err(HirItemInvariantError::ContractScopeIdentityCollision);
    }
    Ok(())
}

fn validate_function_body(
    expected: HirModuleId,
    body: &HirFunctionBody,
) -> Result<(), HirItemInvariantError> {
    match body {
        HirFunctionBody::Error(expression) => validate_expr(expected, *expression),
        HirFunctionBody::Block {
            scope,
            statements,
            tail,
        } => {
            validate_scope(expected, *scope)?;
            validate_statements(expected, statements)?;
            validate_expr(expected, *tail)
        }
    }
}

fn validate_predicate_body(
    expected: HirModuleId,
    body: &HirPredicateBody,
) -> Result<(), HirItemInvariantError> {
    match body {
        HirPredicateBody::Expression { scope, expression }
        | HirPredicateBody::Error { scope, expression } => {
            validate_scope(expected, *scope)?;
            validate_expr(expected, *expression)
        }
        HirPredicateBody::Block {
            scope,
            statements,
            tail,
        } => {
            validate_scope(expected, *scope)?;
            validate_statements(expected, statements)?;
            validate_expr(expected, *tail)
        }
    }
}

fn validate_proof_body(
    expected: HirModuleId,
    body: &HirProofBody,
) -> Result<(), HirItemInvariantError> {
    match body {
        HirProofBody::Expression { scope, expression }
        | HirProofBody::Error { scope, expression } => {
            validate_scope(expected, *scope)?;
            validate_expr(expected, *expression)
        }
        HirProofBody::Block {
            scope,
            statements,
            tail,
        } => {
            validate_scope(expected, *scope)?;
            validate_statements(expected, statements)?;
            validate_expr(expected, *tail)
        }
    }
}

fn validate_member_ids(
    owner: ItemId,
    family: HirItemFamily,
    members: &[HirDeclarationMemberId],
) -> Result<(), HirItemInvariantError> {
    if !members.is_empty()
        && !matches!(
            family,
            HirItemFamily::Character
                | HirItemFamily::View
                | HirItemFamily::Activity
                | HirItemFamily::Metric
                | HirItemFamily::Layer
        )
    {
        return Err(HirItemInvariantError::MembersNotAllowed { family });
    }
    for (position, member) in members.iter().enumerate() {
        let ordinal = u32::try_from(position)
            .map_err(|_| HirItemInvariantError::MemberOrdinalOverflow { position })?;
        let expected = HirDeclarationMemberId::new(owner, ordinal);
        if *member != expected {
            return Err(HirItemInvariantError::NonContiguousMember {
                expected,
                actual: *member,
            });
        }
    }
    Ok(())
}

fn validate_retained_family(
    header: &HirRetainedHeader,
    expected: DeclarationIdentityFamily,
) -> Result<(), HirItemInvariantError> {
    if header.family() == expected {
        Ok(())
    } else {
        Err(HirItemInvariantError::RetainedFamilyMismatch)
    }
}

#[cfg(test)]
mod tests;
