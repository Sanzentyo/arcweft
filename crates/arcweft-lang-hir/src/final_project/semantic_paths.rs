//! Snapshot-bound declaration and item-root semantic path construction.

use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
    sync::Arc,
};

use thiserror::Error;

use super::{AcceptedHirProjectSymbolGeneration, HirExecutableProjectView};
use crate::{
    body_edges::{
        HirBodyChild, HirBodyChildEdge, HirBodyChildRole, HirBodyKind, HirBodyProjection,
        HirBodyProjectionError,
    },
    expr::{
        HirCallValue, HirExprKind, HirExpressionChildRole, HirExpressionOwnedBodyProjectionError,
        HirExpressionOwnedBodyRole, HirExpressionOwnedChild, HirExpressionOwnedChildEdgeError,
        HirNestedExpressionPathSegment, HirPlaceholderKind,
    },
    identity::{CaptureId, ExprId, HirSnapshotId, ItemId, LocalId, PatternId, StmtId},
    item::{
        HirDeclarationMemberKind, HirImplMember, HirItemKind, HirItemPrefix, HirMethodParameter,
        HirMethodParameterGroup, HirParameter,
    },
    module::HirModule,
    pattern::{HirPatternBinding, HirPatternChild, HirPatternChildRole, HirPatternKind},
    scope::{CaptureAccess, HirLocalKind, HirScopeOwner},
    source_index::HirCallableSourceOwner,
    stmt::{
        HirContextualStmtBody, HirSelectBranchHead, HirSelectStmt, HirStatementBodyProjectionError,
        HirStatementBodyRole, HirStatementChild, HirStatementChildRole, HirStmtEvaluationStep,
        HirStmtEvaluationStepError, HirStmtKind,
    },
    symbol::CallableDeclarationKey,
};

/// Closed declaration-body role vocabulary for executable callable owners.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDeclarationBodyRootRole {
    FunctionBody,
    PredicateBody,
    ProofBody,
    FlowBody,
    ImplFunctionBody,
    ViewValue { ordinal: u32 },
}

/// Closed parameter-root role vocabulary.
///
/// Parameter patterns and defaults are declaration roots, but they are not
/// body children. Keeping their role family separate prevents a body root
/// from representing an impossible parameter child.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDeclarationParameterRootRole {
    Pattern { group: u32, parameter: u32 },
    Default { group: u32, parameter: u32 },
}

/// Closed contract-root role vocabulary.
///
/// Effect operands are evaluated before a callable body and therefore belong
/// to the declaration contract inventory rather than the body inventory.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDeclarationContractRootRole {
    Requires {
        ordinal: u32,
    },
    Ensures {
        ordinal: u32,
    },
    Invariant {
        ordinal: u32,
    },
    Assume,
    Reads {
        ordinal: u32,
    },
    Modifies {
        ordinal: u32,
    },
    Decreases,
    EffectOperand {
        clause: u32,
        family: HirFlowContractRootFamily,
        operand: u32,
    },
}

/// Owner of one typed outer-attribute expression root.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirItemAttributeOwner {
    Item,
    InlineMember { member: u16 },
    CapabilityMember { member: u16 },
}

/// Typed style-root path preserving authored rule/environment nesting.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirStyleRootPath(Box<[HirStyleRootPathSegment]>);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStyleRootPathSegment {
    Token { ordinal: u32 },
    Rule { ordinal: u32 },
    Declaration { ordinal: u32 },
    Environment { ordinal: u32 },
    Clause { ordinal: u32 },
}

impl HirStyleRootPath {
    fn new(segments: Vec<HirStyleRootPathSegment>) -> Self {
        Self(segments.into_boxed_slice())
    }

    pub const fn segments(&self) -> &[HirStyleRootPathSegment] {
        &self.0
    }
}

/// Typed Layer expression-member field.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLayerExpressionRootField {
    Z,
    Visible,
    Transform,
}

/// Typed recovery owner for an item expression retained through recovery.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirItemRecoveryRootOwner {
    Item,
    Attribute { attribute: u32, argument: u32 },
    DeclarationMember { member: u32 },
}

/// Closed typed item-root role vocabulary for project-wide executable roots.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDeclarationItemRootRole {
    AttributeArgument {
        owner: HirItemAttributeOwner,
        attribute: u32,
        argument: u32,
    },
    ActivityRequires {
        ordinal: u32,
    },
    ActivityEnsures {
        ordinal: u32,
    },
    ResourceField {
        field: u32,
    },
    CharacterDisplayName {
        member: u32,
    },
    MetricUnit {
        member: u32,
    },
    MetricBuckets {
        member: u32,
        ordinal: u32,
    },
    LayerField {
        member: u32,
        field: HirLayerExpressionRootField,
    },
    EntryOption {
        member: u32,
    },
    Style {
        path: HirStyleRootPath,
    },
    TestBody,
    BenchBody,
    Recovery {
        owner: HirItemRecoveryRootOwner,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFlowContractRootFamily {
    Requires,
    Ensures,
    Invariant,
    Assume,
    Effects,
    NoEffect,
    Reads,
    Modifies,
    Decreases,
}

/// One expression or body root owned directly by a top-level item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirItemEvaluationRoot {
    role: HirDeclarationItemRootRole,
    projection: HirBodyProjection,
}

impl HirItemEvaluationRoot {
    pub const fn role(&self) -> &HirDeclarationItemRootRole {
        &self.role
    }

    pub const fn projection(&self) -> &HirBodyProjection {
        &self.projection
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSemanticPathStep {
    DeclarationBody(HirDeclarationBodyRootRole),
    DeclarationContract(HirDeclarationContractRootRole),
    DeclarationItem(HirDeclarationItemRootRole),
    ExpressionOwned(HirExpressionOwnedBodyRole),
    Body(HirBodyChildRole),
    Statement(HirStatementChildRole),
    StatementBody(HirStatementBodyRole),
    Expression(HirExpressionChildRole),
    MatchPattern { arm: u32 },
    Pattern(HirPatternChildRole),
    ParameterPattern { group: u32, parameter: u32 },
    ParameterDefault { group: u32, parameter: u32 },
    DeclarationMember { member: u32 },
    DeclarationResult,
}

/// Session-only expression hop used by sema to join the HIR role with the
/// exact checked edge fact. Raw IDs never enter [`HirSemanticPathStep`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirExpressionSemanticHop {
    parent: ExprId,
    child: ExprId,
    role: HirExpressionChildRole,
}

impl HirExpressionSemanticHop {
    pub const fn parent(&self) -> ExprId {
        self.parent
    }

    pub const fn child(&self) -> ExprId {
        self.child
    }

    pub const fn role(&self) -> &HirExpressionChildRole {
        &self.role
    }
}

/// One declaration-relative path row shared by every HIR owner family.
///
/// The hop sequence is retained beside the structural steps instead of in a
/// parallel expression-only map.  This makes a row for a statement, pattern,
/// or local reached below an expression carry the same checked expression
/// ancestry as the expression-owned descendants below it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSemanticOwnerPath {
    steps: Box<[HirSemanticPathStep]>,
    hops: Box<[HirExpressionSemanticHop]>,
}

impl HirSemanticOwnerPath {
    const fn new(steps: Box<[HirSemanticPathStep]>, hops: Box<[HirExpressionSemanticHop]>) -> Self {
        Self { steps, hops }
    }

    pub const fn steps(&self) -> &[HirSemanticPathStep] {
        &self.steps
    }

    pub const fn hops(&self) -> &[HirExpressionSemanticHop] {
        &self.hops
    }
}

/// Typed owner of one semantic body container.
///
/// Declaration and item roots are scoped by the containing path index, while
/// nested expression and statement bodies carry the exact owner and role that
/// distinguishes siblings with the same structural path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirSemanticBodyOwner(HirSemanticBodyOwnerKind);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum HirSemanticBodyOwnerKind {
    Declaration(HirDeclarationBodyRootRole),
    Item(HirDeclarationItemRootRole),
    Expression(ExprId),
    ExpressionOwned {
        expression: ExprId,
        role: HirExpressionOwnedBodyRole,
    },
    Statement {
        statement: StmtId,
        role: HirStatementBodyRole,
    },
}

/// Raw-ID-free body owner role retained across the HIR/sema boundary.
///
/// This projection is exhaustive over the private owner representation, so a
/// future owner family cannot silently degrade to an existing stable family.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSemanticBodyOwnerRole {
    Declaration(HirDeclarationBodyRootRole),
    Item(HirDeclarationItemRootRole),
    Expression,
    ExpressionOwned(HirExpressionOwnedBodyRole),
    Statement(HirStatementBodyRole),
}

/// Error returned when a semantic body owner is built from a role that is not
/// a body-bearing expression-owned role.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirSemanticBodyOwnerError {
    #[error("expression-owned role {role:?} does not identify a body container")]
    NonBodyBearingExpressionOwnedRole { role: HirExpressionOwnedBodyRole },
}

impl HirSemanticBodyOwner {
    pub const fn declaration(role: HirDeclarationBodyRootRole) -> Self {
        Self(HirSemanticBodyOwnerKind::Declaration(role))
    }

    pub const fn item(role: HirDeclarationItemRootRole) -> Self {
        Self(HirSemanticBodyOwnerKind::Item(role))
    }

    pub const fn direct_expression(expression: ExprId) -> Self {
        Self(HirSemanticBodyOwnerKind::Expression(expression))
    }

    /// Builds an expression-owned body owner only for an actual nested body.
    pub fn try_expression_owned(
        expression: ExprId,
        role: HirExpressionOwnedBodyRole,
    ) -> Result<Self, HirSemanticBodyOwnerError> {
        if role.is_body_bearing() {
            Ok(Self(HirSemanticBodyOwnerKind::ExpressionOwned {
                expression,
                role,
            }))
        } else {
            Err(HirSemanticBodyOwnerError::NonBodyBearingExpressionOwnedRole { role })
        }
    }

    pub const fn statement_body(statement: StmtId, role: HirStatementBodyRole) -> Self {
        Self(HirSemanticBodyOwnerKind::Statement { statement, role })
    }

    const fn kind(&self) -> &HirSemanticBodyOwnerKind {
        &self.0
    }

    pub fn semantic_role(&self) -> HirSemanticBodyOwnerRole {
        match self.kind() {
            HirSemanticBodyOwnerKind::Declaration(role) => {
                HirSemanticBodyOwnerRole::Declaration(*role)
            }
            HirSemanticBodyOwnerKind::Item(role) => HirSemanticBodyOwnerRole::Item(role.clone()),
            HirSemanticBodyOwnerKind::Expression(_) => HirSemanticBodyOwnerRole::Expression,
            HirSemanticBodyOwnerKind::ExpressionOwned { role, .. } => {
                HirSemanticBodyOwnerRole::ExpressionOwned(role.clone())
            }
            HirSemanticBodyOwnerKind::Statement { role, .. } => {
                HirSemanticBodyOwnerRole::Statement(*role)
            }
        }
    }

    pub const fn expression_owner(&self) -> Option<ExprId> {
        match self.kind() {
            HirSemanticBodyOwnerKind::Expression(expression)
            | HirSemanticBodyOwnerKind::ExpressionOwned { expression, .. } => Some(*expression),
            HirSemanticBodyOwnerKind::Declaration(_)
            | HirSemanticBodyOwnerKind::Item(_)
            | HirSemanticBodyOwnerKind::Statement { .. } => None,
        }
    }

    pub const fn statement_owner(&self) -> Option<StmtId> {
        match self.kind() {
            HirSemanticBodyOwnerKind::Statement { statement, .. } => Some(*statement),
            HirSemanticBodyOwnerKind::Declaration(_)
            | HirSemanticBodyOwnerKind::Item(_)
            | HirSemanticBodyOwnerKind::Expression(_)
            | HirSemanticBodyOwnerKind::ExpressionOwned { .. } => None,
        }
    }

    pub const fn declaration_role(&self) -> Option<HirDeclarationBodyRootRole> {
        match self.kind() {
            HirSemanticBodyOwnerKind::Declaration(role) => Some(*role),
            HirSemanticBodyOwnerKind::Item(_)
            | HirSemanticBodyOwnerKind::Expression(_)
            | HirSemanticBodyOwnerKind::ExpressionOwned { .. }
            | HirSemanticBodyOwnerKind::Statement { .. } => None,
        }
    }

    pub const fn item_role(&self) -> Option<&HirDeclarationItemRootRole> {
        match self.kind() {
            HirSemanticBodyOwnerKind::Item(role) => Some(role),
            HirSemanticBodyOwnerKind::Declaration(_)
            | HirSemanticBodyOwnerKind::Expression(_)
            | HirSemanticBodyOwnerKind::ExpressionOwned { .. }
            | HirSemanticBodyOwnerKind::Statement { .. } => None,
        }
    }

    pub const fn expression_owned_role(&self) -> Option<&HirExpressionOwnedBodyRole> {
        match self.kind() {
            HirSemanticBodyOwnerKind::ExpressionOwned { role, .. } => Some(role),
            HirSemanticBodyOwnerKind::Declaration(_)
            | HirSemanticBodyOwnerKind::Item(_)
            | HirSemanticBodyOwnerKind::Expression(_)
            | HirSemanticBodyOwnerKind::Statement { .. } => None,
        }
    }

    pub const fn statement_role(&self) -> Option<HirStatementBodyRole> {
        match self.kind() {
            HirSemanticBodyOwnerKind::Statement { role, .. } => Some(*role),
            HirSemanticBodyOwnerKind::Declaration(_)
            | HirSemanticBodyOwnerKind::Item(_)
            | HirSemanticBodyOwnerKind::Expression(_)
            | HirSemanticBodyOwnerKind::ExpressionOwned { .. } => None,
        }
    }
}

/// One source-ordered body container and its typed HIR child edges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSemanticBodyRow {
    owner: HirSemanticBodyOwner,
    path: HirSemanticOwnerPath,
    projection: HirBodyProjection,
}

/// Kind of control transfer retained by the HIR topology.
///
/// The spelling of a control-transfer label is deliberately absent.  Labels
/// are not target declarations in the current final HIR, so a labeled use is
/// rejected while the topology is being built rather than being retained as
/// unresolved semantic identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirControlTransferKind {
    Out,
    Break,
    Continue,
}

/// Lexical construct family that can receive a `break` or `continue`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLoopTargetFamily {
    LoopExpression,
    WhileStatement,
    WhileLetStatement,
    ForStatement,
}

/// Typed target resolved for one accepted control-transfer statement.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirControlTransferTarget {
    Output {
        application: ExprId,
    },
    Loop {
        family: HirLoopTargetFamily,
        body_owner: HirSemanticBodyOwner,
    },
}

impl HirControlTransferTarget {
    pub(crate) const fn output(application: ExprId) -> Self {
        Self::Output { application }
    }

    pub(crate) const fn loop_target(
        family: HirLoopTargetFamily,
        body_owner: HirSemanticBodyOwner,
    ) -> Self {
        Self::Loop { family, body_owner }
    }

    pub const fn output_application(&self) -> Option<ExprId> {
        match self {
            Self::Output { application } => Some(*application),
            Self::Loop { .. } => None,
        }
    }

    pub const fn loop_family(&self) -> Option<HirLoopTargetFamily> {
        match self {
            Self::Output { .. } => None,
            Self::Loop { family, .. } => Some(*family),
        }
    }

    pub const fn loop_body_owner(&self) -> Option<&HirSemanticBodyOwner> {
        match self {
            Self::Output { .. } => None,
            Self::Loop { body_owner, .. } => Some(body_owner),
        }
    }
}

/// One root-local semantic control-transfer row.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirControlTransferRow {
    statement: StmtId,
    kind: HirControlTransferKind,
    target: HirControlTransferTarget,
}

impl HirControlTransferRow {
    pub(crate) const fn new(
        statement: StmtId,
        kind: HirControlTransferKind,
        target: HirControlTransferTarget,
    ) -> Self {
        Self {
            statement,
            kind,
            target,
        }
    }

    pub const fn statement(&self) -> StmtId {
        self.statement
    }

    pub const fn kind(&self) -> HirControlTransferKind {
        self.kind
    }

    pub const fn target(&self) -> &HirControlTransferTarget {
        &self.target
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
enum HirSemanticBodyRowError {
    #[error("a semantic body row has no structural path")]
    EmptyPath,
}

impl HirSemanticBodyRow {
    fn try_new(
        owner: HirSemanticBodyOwner,
        path: HirSemanticOwnerPath,
        projection: HirBodyProjection,
    ) -> Result<Self, HirSemanticBodyRowError> {
        if path.steps().is_empty() {
            return Err(HirSemanticBodyRowError::EmptyPath);
        }
        Ok(Self {
            owner,
            path,
            projection,
        })
    }

    fn try_validate(&self) -> Result<(), HirSemanticBodyRowError> {
        if self.path.steps().is_empty() {
            return Err(HirSemanticBodyRowError::EmptyPath);
        }
        Ok(())
    }

    pub const fn owner(&self) -> &HirSemanticBodyOwner {
        &self.owner
    }

    pub const fn kind(&self) -> HirBodyKind {
        self.projection.kind()
    }

    pub const fn path(&self) -> &HirSemanticOwnerPath {
        &self.path
    }

    pub const fn children(&self) -> &[HirBodyChildEdge] {
        self.projection.children()
    }

    pub const fn projection(&self) -> &HirBodyProjection {
        &self.projection
    }
}

/// Root identity for one snapshot-bound semantic path index.
///
/// Declaration indexes are keyed by their callable declaration. Item indexes
/// are keyed only by the authored item entry coordinate; they deliberately do
/// not mint or expose a semantic declaration identity for non-callable roots.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSemanticPathRoot {
    Declaration(CallableDeclarationKey),
    Item {
        item: ItemId,
        entry_ordinal: u32,
        role: HirItemEvaluationEntryRole,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSemanticPathIndex {
    root: HirSemanticPathRoot,
    snapshot: HirSnapshotId,
    expressions: BTreeMap<ExprId, HirSemanticOwnerPath>,
    statements: BTreeMap<StmtId, HirSemanticOwnerPath>,
    patterns: BTreeMap<PatternId, HirSemanticOwnerPath>,
    locals: BTreeMap<LocalId, HirSemanticOwnerPath>,
    body_rows: Box<[HirSemanticBodyRow]>,
    control_transfers: Box<[HirControlTransferRow]>,
}

/// Closed owner vocabulary for raw HIR identities that can have one accepted
/// structural semantic path.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSemanticPathOwnerId {
    Expression(ExprId),
    Statement(StmtId),
    Pattern(PatternId),
    Local(LocalId),
}

impl HirSemanticPathOwnerId {
    pub const fn module(self) -> crate::identity::HirModuleId {
        match self {
            Self::Expression(owner) => owner.module(),
            Self::Statement(owner) => owner.module(),
            Self::Pattern(owner) => owner.module(),
            Self::Local(owner) => owner.module(),
        }
    }

    fn path_in(self, index: &HirSemanticPathIndex) -> Option<&HirSemanticOwnerPath> {
        match self {
            Self::Expression(owner) => index.expression(owner),
            Self::Statement(owner) => index.statement(owner),
            Self::Pattern(owner) => index.pattern(owner),
            Self::Local(owner) => index.local(owner),
        }
    }
}

impl From<ExprId> for HirSemanticPathOwnerId {
    fn from(owner: ExprId) -> Self {
        Self::Expression(owner)
    }
}

impl From<StmtId> for HirSemanticPathOwnerId {
    fn from(owner: StmtId) -> Self {
        Self::Statement(owner)
    }
}

impl From<PatternId> for HirSemanticPathOwnerId {
    fn from(owner: PatternId) -> Self {
        Self::Pattern(owner)
    }
}

impl From<LocalId> for HirSemanticPathOwnerId {
    fn from(owner: LocalId) -> Self {
        Self::Local(owner)
    }
}

/// Borrowed proof that one raw HIR owner occurs at exactly one rooted path in
/// the sealed project topology.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirSemanticPathLocation<'topology> {
    owner: HirSemanticPathOwnerId,
    snapshot: HirSnapshotId,
    root: &'topology HirSemanticPathRoot,
    path: &'topology HirSemanticOwnerPath,
}

/// Exact typed key for one semantic body container in the sealed project.
///
/// The accepted root is part of the key because declaration and item body
/// roles are root-relative, while nested raw owners remain generation-local
/// lookup evidence only.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirSemanticBodyLocator {
    root: HirSemanticPathRoot,
    owner: HirSemanticBodyOwner,
}

impl HirSemanticBodyLocator {
    pub const fn new(root: HirSemanticPathRoot, owner: HirSemanticBodyOwner) -> Self {
        Self { root, owner }
    }

    pub const fn root(&self) -> &HirSemanticPathRoot {
        &self.root
    }

    pub const fn owner(&self) -> &HirSemanticBodyOwner {
        &self.owner
    }
}

/// Borrowed proof that a body locator resolves to exactly one validated row in
/// one accepted HIR snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirSemanticBodyLocation<'topology> {
    snapshot: HirSnapshotId,
    root: &'topology HirSemanticPathRoot,
    row: &'topology HirSemanticBodyRow,
}

/// Borrowed proof that one control-transfer statement resolves to exactly one
/// root-local target in the sealed project topology.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirControlTransferLocation<'topology> {
    snapshot: HirSnapshotId,
    root: &'topology HirSemanticPathRoot,
    path: &'topology HirSemanticOwnerPath,
    row: &'topology HirControlTransferRow,
}

impl<'topology> HirControlTransferLocation<'topology> {
    pub const fn snapshot(self) -> HirSnapshotId {
        self.snapshot
    }

    pub const fn root(self) -> &'topology HirSemanticPathRoot {
        self.root
    }

    pub const fn path(self) -> &'topology HirSemanticOwnerPath {
        self.path
    }

    pub const fn row(self) -> &'topology HirControlTransferRow {
        self.row
    }

    pub const fn statement(self) -> StmtId {
        self.row.statement()
    }

    pub const fn kind(self) -> HirControlTransferKind {
        self.row.kind()
    }

    pub const fn target(self) -> &'topology HirControlTransferTarget {
        self.row.target()
    }
}

impl<'topology> HirSemanticBodyLocation<'topology> {
    pub const fn snapshot(self) -> HirSnapshotId {
        self.snapshot
    }

    pub const fn root(self) -> &'topology HirSemanticPathRoot {
        self.root
    }

    pub const fn row(self) -> &'topology HirSemanticBodyRow {
        self.row
    }
}

impl<'topology> HirSemanticPathLocation<'topology> {
    pub const fn owner(self) -> HirSemanticPathOwnerId {
        self.owner
    }

    pub const fn snapshot(self) -> HirSnapshotId {
        self.snapshot
    }

    pub const fn root(self) -> &'topology HirSemanticPathRoot {
        self.root
    }

    pub const fn path(self) -> &'topology HirSemanticOwnerPath {
        self.path
    }
}

impl HirSemanticPathIndex {
    pub const fn root(&self) -> &HirSemanticPathRoot {
        &self.root
    }

    pub const fn snapshot(&self) -> HirSnapshotId {
        self.snapshot
    }

    pub fn expression(&self, owner: ExprId) -> Option<&HirSemanticOwnerPath> {
        self.expressions.get(&owner)
    }

    pub fn statement(&self, owner: StmtId) -> Option<&HirSemanticOwnerPath> {
        self.statements.get(&owner)
    }

    pub fn pattern(&self, owner: PatternId) -> Option<&HirSemanticOwnerPath> {
        self.patterns.get(&owner)
    }

    pub fn local(&self, owner: LocalId) -> Option<&HirSemanticOwnerPath> {
        self.locals.get(&owner)
    }

    /// Returns the source-ordered semantic body rows retained by this index.
    /// Empty bodies remain rows so consumers never have to infer authored
    /// containers from the absence of children.
    pub const fn body_rows(&self) -> &[HirSemanticBodyRow] {
        &self.body_rows
    }

    /// Returns the root-local control-transfer rows in statement-ID order.
    pub const fn control_transfers(&self) -> &[HirControlTransferRow] {
        &self.control_transfers
    }

    pub fn control_transfer(&self, statement: StmtId) -> Option<&HirControlTransferRow> {
        self.control_transfers
            .iter()
            .find(|row| row.statement() == statement)
    }

    pub fn body_row(&self, owner: &HirSemanticBodyOwner) -> Option<&HirSemanticBodyRow> {
        self.body_rows.iter().find(|row| row.owner() == owner)
    }

    /// Borrows the complete local-owner inventory without exposing the
    /// backing map or creating another ownership side table.
    pub fn locals(&self) -> impl Iterator<Item = (LocalId, &HirSemanticOwnerPath)> {
        self.locals.iter().map(|(owner, path)| (*owner, path))
    }

    fn owner_paths(&self) -> impl Iterator<Item = (HirSemanticPathOwnerId, &HirSemanticOwnerPath)> {
        self.expressions
            .iter()
            .map(|(owner, path)| (HirSemanticPathOwnerId::Expression(*owner), path))
            .chain(
                self.statements
                    .iter()
                    .map(|(owner, path)| (HirSemanticPathOwnerId::Statement(*owner), path)),
            )
            .chain(
                self.patterns
                    .iter()
                    .map(|(owner, path)| (HirSemanticPathOwnerId::Pattern(*owner), path)),
            )
            .chain(
                self.locals
                    .iter()
                    .map(|(owner, path)| (HirSemanticPathOwnerId::Local(*owner), path)),
            )
    }

    fn validate_root_paths(&self) -> Result<(), HirSemanticPathError> {
        let mut structural_paths = BTreeMap::new();
        for (owner, path) in self.owner_paths() {
            if owner.module() != self.snapshot.module() {
                return Err(HirSemanticPathError::OwnerModuleMismatch {
                    owner,
                    snapshot: self.snapshot,
                });
            }
            if path.steps().is_empty() {
                return Err(HirSemanticPathError::InvalidOwnerPath { owner });
            }
            let root_valid = match &self.root {
                HirSemanticPathRoot::Declaration(_) => {
                    let steps = path.steps();
                    !steps.iter().any(|step| {
                        matches!(
                            step,
                            HirSemanticPathStep::DeclarationItem(_)
                                | HirSemanticPathStep::DeclarationMember { .. }
                        )
                    }) && !steps.iter().enumerate().any(|(index, step)| {
                        matches!(step, HirSemanticPathStep::DeclarationResult)
                            && (index != 0 || steps.len() != 1)
                    })
                }
                HirSemanticPathRoot::Item { role, .. } => {
                    let steps = path.steps();
                    let first_valid =
                        matches!(steps.first(), Some(HirSemanticPathStep::DeclarationItem(_)))
                            || matches!(
                                (role, steps.first()),
                                (
                                    HirItemEvaluationEntryRole::Item,
                                    Some(HirSemanticPathStep::DeclarationMember { .. })
                                )
                            );
                    first_valid
                        && !steps.iter().enumerate().any(|(index, step)| {
                            matches!(
                                step,
                                HirSemanticPathStep::DeclarationBody(_)
                                    | HirSemanticPathStep::DeclarationContract(_)
                                    | HirSemanticPathStep::ParameterPattern { .. }
                                    | HirSemanticPathStep::ParameterDefault { .. }
                                    | HirSemanticPathStep::DeclarationResult
                            ) || (index > 0
                                && matches!(
                                    step,
                                    HirSemanticPathStep::DeclarationItem(_)
                                        | HirSemanticPathStep::DeclarationMember { .. }
                                ))
                        })
                        && (role != &HirItemEvaluationEntryRole::Item
                            || !matches!(
                                steps.first(),
                                Some(HirSemanticPathStep::DeclarationMember { .. })
                            )
                            || matches!(
                                steps.last(),
                                Some(HirSemanticPathStep::DeclarationMember { .. })
                            ))
                }
            };
            if !root_valid {
                return Err(HirSemanticPathError::InvalidOwnerPath { owner });
            }
            let mut hops = path.hops().iter();
            for role in path.steps().iter().filter_map(|step| match step {
                HirSemanticPathStep::Expression(role) => Some(role),
                _ => None,
            }) {
                let Some(hop) = hops.next() else {
                    return Err(HirSemanticPathError::InvalidExpressionHops { owner });
                };
                if hop.role() != role
                    || hop.parent().module() != self.snapshot.module()
                    || hop.child().module() != self.snapshot.module()
                {
                    return Err(HirSemanticPathError::InvalidExpressionHops { owner });
                }
            }
            if hops.next().is_some() {
                return Err(HirSemanticPathError::InvalidExpressionHops { owner });
            }
            if let Some(first) = structural_paths.insert(path.steps(), owner) {
                return Err(HirSemanticPathError::DuplicateStructuralPath {
                    first,
                    second: owner,
                });
            }
        }
        self.validate_body_rows()?;
        self.validate_control_transfers()?;
        Ok(())
    }

    fn validate_body_rows(&self) -> Result<(), HirSemanticPathError> {
        let mut owners = BTreeSet::new();
        for row in &self.body_rows {
            if !owners.insert(row.owner()) {
                return Err(HirSemanticPathError::DuplicateBodyOwner {
                    owner: row.owner().clone(),
                });
            }
            let root_owner_valid = match &self.root {
                HirSemanticPathRoot::Declaration(_) => row.owner().item_role().is_none(),
                HirSemanticPathRoot::Item { .. } => row.owner().declaration_role().is_none(),
            };
            if !root_owner_valid {
                return Err(HirSemanticPathError::InvalidBodyRow {
                    owner: row.owner().clone(),
                });
            }
            row.try_validate()
                .map_err(|_| HirSemanticPathError::InvalidBodyRow {
                    owner: row.owner().clone(),
                })?;
            validate_body_owner_kind(row)?;
            validate_body_owner_path(self, row)?;
            for edge in row.children() {
                match edge.child() {
                    HirBodyChild::Expression(owner) if owner.module() != self.snapshot.module() => {
                        return Err(HirSemanticPathError::InvalidBodyRow {
                            owner: row.owner().clone(),
                        });
                    }
                    HirBodyChild::Statement(owner) if owner.module() != self.snapshot.module() => {
                        return Err(HirSemanticPathError::InvalidBodyRow {
                            owner: row.owner().clone(),
                        });
                    }
                    HirBodyChild::Expression(_) | HirBodyChild::Statement(_) => {}
                }
                validate_body_child_path(self, row, edge)?;
            }
        }
        Ok(())
    }

    fn validate_control_transfers(&self) -> Result<(), HirSemanticPathError> {
        let mut statements = BTreeSet::new();
        for row in &self.control_transfers {
            let statement = row.statement();
            if !statements.insert(statement) {
                return Err(HirSemanticPathError::DuplicateControlTransfer { statement });
            }
            if statement.module() != self.snapshot.module() {
                return Err(HirSemanticPathError::ControlTransferModuleMismatch {
                    statement,
                    snapshot: self.snapshot,
                });
            }
            let Some(statement_path) = self.statement(statement) else {
                return Err(HirSemanticPathError::InvalidControlTransferRow { statement });
            };
            match (row.kind(), row.target()) {
                (HirControlTransferKind::Out, HirControlTransferTarget::Output { application }) => {
                    if application.module() != self.snapshot.module() {
                        return Err(HirSemanticPathError::ControlTransferModuleMismatch {
                            statement,
                            snapshot: self.snapshot,
                        });
                    }
                    let Some(application_path) = self.expression(*application) else {
                        return Err(HirSemanticPathError::InvalidControlTransferRow { statement });
                    };
                    if !output_path_is_descendant(application_path, statement_path) {
                        return Err(HirSemanticPathError::InvalidControlTransferRow { statement });
                    }
                }
                (
                    HirControlTransferKind::Break | HirControlTransferKind::Continue,
                    HirControlTransferTarget::Loop { family, body_owner },
                ) => {
                    if body_owner
                        .expression_owner()
                        .is_some_and(|owner| owner.module() != self.snapshot.module())
                        || body_owner
                            .statement_owner()
                            .is_some_and(|owner| owner.module() != self.snapshot.module())
                    {
                        return Err(HirSemanticPathError::ControlTransferModuleMismatch {
                            statement,
                            snapshot: self.snapshot,
                        });
                    }
                    let Some(body_row) = self.body_row(body_owner) else {
                        return Err(HirSemanticPathError::InvalidControlTransferRow { statement });
                    };
                    if !loop_target_family_matches(*family, body_owner)
                        || !path_is_prefix(body_row.path(), statement_path)
                    {
                        return Err(HirSemanticPathError::InvalidControlTransferRow { statement });
                    }
                }
                _ => {
                    return Err(HirSemanticPathError::InvalidControlTransferRow { statement });
                }
            }
        }
        Ok(())
    }
}

fn path_is_prefix(prefix: &HirSemanticOwnerPath, path: &HirSemanticOwnerPath) -> bool {
    path.steps().starts_with(prefix.steps()) && path.hops().starts_with(prefix.hops())
}

fn output_path_is_descendant(
    application_path: &HirSemanticOwnerPath,
    statement_path: &HirSemanticOwnerPath,
) -> bool {
    if !path_is_prefix(application_path, statement_path)
        || statement_path.steps().len() <= application_path.steps().len()
    {
        return false;
    }
    statement_path.steps()[application_path.steps().len()..]
        .iter()
        .any(|step| {
            matches!(
                step,
                HirSemanticPathStep::ExpressionOwned(
                    HirExpressionOwnedBodyRole::DialogueLinePlanStatement { .. }
                )
            )
        })
}

fn loop_target_family_matches(family: HirLoopTargetFamily, owner: &HirSemanticBodyOwner) -> bool {
    matches!(
        (family, owner.semantic_role()),
        (
            HirLoopTargetFamily::LoopExpression,
            HirSemanticBodyOwnerRole::Expression
        ) | (
            HirLoopTargetFamily::WhileStatement,
            HirSemanticBodyOwnerRole::Statement(HirStatementBodyRole::While),
        ) | (
            HirLoopTargetFamily::WhileLetStatement,
            HirSemanticBodyOwnerRole::Statement(HirStatementBodyRole::WhileLet),
        ) | (
            HirLoopTargetFamily::ForStatement,
            HirSemanticBodyOwnerRole::Statement(HirStatementBodyRole::For),
        )
    )
}

fn validate_body_owner_kind(row: &HirSemanticBodyRow) -> Result<(), HirSemanticPathError> {
    let valid = match row.owner().kind() {
        HirSemanticBodyOwnerKind::Declaration(HirDeclarationBodyRootRole::FlowBody) => {
            row.kind() == HirBodyKind::Thread
        }
        HirSemanticBodyOwnerKind::Declaration(
            HirDeclarationBodyRootRole::FunctionBody | HirDeclarationBodyRootRole::ImplFunctionBody,
        )
        | HirSemanticBodyOwnerKind::Item(
            HirDeclarationItemRootRole::TestBody | HirDeclarationItemRootRole::BenchBody,
        ) => row.kind() == HirBodyKind::Ordinary,
        HirSemanticBodyOwnerKind::Declaration(HirDeclarationBodyRootRole::ViewValue { .. }) => {
            row.kind() == HirBodyKind::Expression
        }
        HirSemanticBodyOwnerKind::Declaration(
            HirDeclarationBodyRootRole::PredicateBody | HirDeclarationBodyRootRole::ProofBody,
        ) => matches!(row.kind(), HirBodyKind::Expression | HirBodyKind::Ordinary),
        HirSemanticBodyOwnerKind::Item(HirDeclarationItemRootRole::Recovery { .. }) => false,
        HirSemanticBodyOwnerKind::Item(_) => row.kind() == HirBodyKind::Expression,
        HirSemanticBodyOwnerKind::Expression(_) => {
            matches!(row.kind(), HirBodyKind::Ordinary | HirBodyKind::Thread)
        }
        HirSemanticBodyOwnerKind::ExpressionOwned { role, .. } => match role {
            HirExpressionOwnedBodyRole::AwaitBranchBody { .. } => {
                matches!(row.kind(), HirBodyKind::Ordinary | HirBodyKind::Thread)
            }
            HirExpressionOwnedBodyRole::ChoiceOptionSelectBody { .. }
            | HirExpressionOwnedBodyRole::ChoicePlanTimeoutBody { .. }
            | HirExpressionOwnedBodyRole::ChoicePlanCancelBody { .. }
            | HirExpressionOwnedBodyRole::ChoicePlanOnSelectBody { .. } => {
                row.kind() == HirBodyKind::Thread
            }
            _ => false,
        },
        HirSemanticBodyOwnerKind::Statement { role, .. } => match role {
            HirStatementBodyRole::LetElse
            | HirStatementBodyRole::Defer
            | HirStatementBodyRole::On
            | HirStatementBodyRole::UnsafeLifetime => row.kind() == HirBodyKind::Ordinary,
            HirStatementBodyRole::MatchArm { .. } => matches!(
                row.kind(),
                HirBodyKind::Expression | HirBodyKind::Ordinary | HirBodyKind::Thread
            ),
            HirStatementBodyRole::Then
            | HirStatementBodyRole::Else
            | HirStatementBodyRole::While
            | HirStatementBodyRole::WhileLet
            | HirStatementBodyRole::For
            | HirStatementBodyRole::SelectBranch { .. }
            | HirStatementBodyRole::SourceLocale
            | HirStatementBodyRole::Scope => {
                matches!(row.kind(), HirBodyKind::Ordinary | HirBodyKind::Thread)
            }
        },
    };
    if valid {
        Ok(())
    } else {
        Err(HirSemanticPathError::InvalidBodyRow {
            owner: row.owner().clone(),
        })
    }
}

fn validate_body_owner_path(
    index: &HirSemanticPathIndex,
    row: &HirSemanticBodyRow,
) -> Result<(), HirSemanticPathError> {
    let path = row.path();
    let invalid = || HirSemanticPathError::InvalidBodyRow {
        owner: row.owner().clone(),
    };
    match row.owner().kind() {
        HirSemanticBodyOwnerKind::Declaration(role) => {
            if path.steps() != [HirSemanticPathStep::DeclarationBody(*role)]
                || !path.hops().is_empty()
            {
                return Err(invalid());
            }
        }
        HirSemanticBodyOwnerKind::Item(role) => {
            if path.steps() != [HirSemanticPathStep::DeclarationItem(role.clone())]
                || !path.hops().is_empty()
            {
                return Err(invalid());
            }
        }
        HirSemanticBodyOwnerKind::Expression(expression) => {
            if index.expression(*expression) != Some(path) {
                return Err(invalid());
            }
        }
        HirSemanticBodyOwnerKind::ExpressionOwned { expression, role } => {
            if !role.is_body_bearing() {
                return Err(invalid());
            }
            let Some(parent) = index.expression(*expression) else {
                return Err(invalid());
            };
            if path.hops() != parent.hops()
                || path.steps().len() != parent.steps().len() + 1
                || path.steps().split_at(parent.steps().len()).0 != parent.steps()
                || path.steps().last() != Some(&HirSemanticPathStep::ExpressionOwned(role.clone()))
            {
                return Err(invalid());
            }
        }
        HirSemanticBodyOwnerKind::Statement { statement, role } => {
            let Some(parent) = index.statement(*statement) else {
                return Err(invalid());
            };
            if path.hops() != parent.hops()
                || path.steps().len() != parent.steps().len() + 1
                || path.steps().split_at(parent.steps().len()).0 != parent.steps()
                || path.steps().last() != Some(&HirSemanticPathStep::StatementBody(*role))
            {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

fn validate_body_child_path(
    index: &HirSemanticPathIndex,
    row: &HirSemanticBodyRow,
    edge: &HirBodyChildEdge,
) -> Result<(), HirSemanticPathError> {
    let child_path = match edge.child() {
        HirBodyChild::Expression(owner) => index.expression(owner),
        HirBodyChild::Statement(owner) => index.statement(owner),
    };
    let Some(child_path) = child_path else {
        return Err(HirSemanticPathError::InvalidBodyRow {
            owner: row.owner().clone(),
        });
    };

    let direct = |parent: &HirSemanticOwnerPath, step: HirSemanticPathStep| {
        child_path.hops() == parent.hops()
            && child_path.steps().len() == parent.steps().len() + 1
            && child_path.steps()[..parent.steps().len()] == *parent.steps()
            && child_path.steps().last() == Some(&step)
    };
    let joined = match row.owner().kind() {
        HirSemanticBodyOwnerKind::Declaration(_) | HirSemanticBodyOwnerKind::Item(_) => {
            if row.kind() == HirBodyKind::Expression {
                child_path == row.path()
            } else {
                direct(row.path(), HirSemanticPathStep::Body(edge.role()))
            }
        }
        HirSemanticBodyOwnerKind::Expression(expression) => match edge.role() {
            HirBodyChildRole::Tail => {
                let Some(HirSemanticPathStep::Expression(role)) = child_path.steps().last() else {
                    return Err(HirSemanticPathError::InvalidBodyRow {
                        owner: row.owner().clone(),
                    });
                };
                matches!(
                    role,
                    HirExpressionChildRole::BlockTail | HirExpressionChildRole::LoopTail
                ) && child_path.steps().len() == row.path().steps().len() + 1
                    && child_path.steps()[..row.path().steps().len()] == *row.path().steps()
                    && child_path.hops().len() == row.path().hops().len() + 1
                    && child_path.hops()[..row.path().hops().len()] == *row.path().hops()
                    && child_path.hops().last().is_some_and(|hop| {
                        hop.parent() == *expression
                            && hop.child()
                                == match edge.child() {
                                    HirBodyChild::Expression(child) => child,
                                    HirBodyChild::Statement(_) => return false,
                                }
                            && hop.role() == role
                    })
            }
            _ => direct(row.path(), HirSemanticPathStep::Body(edge.role())),
        },
        HirSemanticBodyOwnerKind::ExpressionOwned { .. } => {
            direct(row.path(), HirSemanticPathStep::Body(edge.role()))
        }
        HirSemanticBodyOwnerKind::Statement { statement, role } => {
            let Some(parent) = index.statement(*statement) else {
                return Err(HirSemanticPathError::InvalidBodyRow {
                    owner: row.owner().clone(),
                });
            };
            match row.kind() {
                HirBodyKind::Expression => {
                    let HirStatementBodyRole::MatchArm { arm } = role else {
                        return Err(HirSemanticPathError::InvalidBodyRow {
                            owner: row.owner().clone(),
                        });
                    };
                    direct(
                        parent,
                        HirSemanticPathStep::Statement(HirStatementChildRole::MatchValue {
                            arm: *arm,
                        }),
                    )
                }
                HirBodyKind::Ordinary => {
                    let HirBodyChildRole::Statement { ordinal } = edge.role() else {
                        return Err(HirSemanticPathError::InvalidBodyRow {
                            owner: row.owner().clone(),
                        });
                    };
                    direct(
                        parent,
                        HirSemanticPathStep::Statement(HirStatementChildRole::BodyItem {
                            body: *role,
                            ordinal,
                        }),
                    )
                }
                HirBodyKind::Thread => direct(row.path(), HirSemanticPathStep::Body(edge.role())),
            }
        }
    };
    if joined {
        Ok(())
    } else {
        Err(HirSemanticPathError::InvalidBodyRow {
            owner: row.owner().clone(),
        })
    }
}

/// One typed parameter root retained separately from body roots because a
/// parameter pattern is not a heterogeneous body child.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDeclarationParameterRoot {
    role: HirDeclarationParameterRootRole,
    child: HirDeclarationParameterRootChild,
}

impl HirDeclarationParameterRoot {
    pub const fn role(&self) -> HirDeclarationParameterRootRole {
        self.role
    }

    pub const fn child(&self) -> HirDeclarationParameterRootChild {
        self.child
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirDeclarationParameterRootChild {
    Pattern(PatternId),
    Expression(ExprId),
}

/// One typed declaration root consumed by the path builder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDeclarationBodyRoot {
    role: HirDeclarationBodyRootRole,
    projection: HirBodyProjection,
}

/// One typed contract root retained separately from body roots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDeclarationContractRoot {
    role: HirDeclarationContractRootRole,
    child: ExprId,
}

/// Exact value-origin classification retained by the project topology.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLocalValueOrigin {
    DirectInitializer(ExprId),
    Independent,
    Composite,
}

/// One binding site retained with its exact statement, pattern, and value
/// identities. Every local has one closed typed site; independent value
/// origins are represented by [`HirLocalValueOrigin::Independent`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirLocalBindingOrigin {
    local: LocalId,
    site: HirBindingSite,
    binding_expression: Option<ExprId>,
    pattern: Option<PatternId>,
    origin: HirLocalValueOrigin,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirBindingSite {
    DeclarationParameter {
        item: ItemId,
        owner: HirCallableSourceOwner,
        group: u32,
        parameter: u32,
    },
    Statement {
        statement: StmtId,
        role: HirLocalBindingStatementRole,
    },
    Expression {
        expression: ExprId,
        role: HirExpressionBindingRole,
    },
    Member {
        item: ItemId,
        member: u32,
        role: HirMemberBindingRole,
    },
    FlowResult {
        item: ItemId,
    },
    PostconditionResult {
        item: ItemId,
        owner: HirCallableSourceOwner,
    },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirExpressionBindingRole {
    ClosureParameter {
        parameter: u32,
    },
    IfLet,
    MatchArm {
        arm: u32,
    },
    AwaitBranch {
        branch: u32,
    },
    ChoiceFor {
        path: crate::expr::HirNestedExpressionPath,
    },
    ChoiceMatchArm {
        path: crate::expr::HirNestedExpressionPath,
        arm: u32,
    },
    ChoiceOptionFor {
        path: crate::expr::HirNestedExpressionPath,
    },
    ChoicePlanOnSelect {
        path: crate::expr::HirNestedExpressionPath,
    },
    ChoicePlanCancelTrigger {
        path: crate::expr::HirNestedExpressionPath,
    },
    DialogueLinePlanLet {
        path: crate::expr::HirNestedExpressionPath,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirMemberBindingRole {
    ActivityInput,
    ActivityOutput,
    MethodReceiver,
    MethodParameter { group: u32, parameter: u32 },
}

impl HirLocalBindingOrigin {
    pub const fn local(&self) -> LocalId {
        self.local
    }

    pub fn site(&self) -> HirBindingSite {
        self.site.clone()
    }

    pub const fn statement(&self) -> Option<StmtId> {
        match &self.site {
            HirBindingSite::Statement { statement, .. } => Some(*statement),
            HirBindingSite::DeclarationParameter { .. }
            | HirBindingSite::Expression { .. }
            | HirBindingSite::Member { .. }
            | HirBindingSite::FlowResult { .. }
            | HirBindingSite::PostconditionResult { .. } => None,
        }
    }

    pub const fn pattern(&self) -> Option<PatternId> {
        self.pattern
    }

    /// Expression region that lexically owns this binding, when the binding
    /// is introduced from within an expression-owned statement or pattern.
    pub const fn binding_expression(&self) -> Option<ExprId> {
        self.binding_expression
    }

    pub const fn value(&self) -> Option<ExprId> {
        match self.origin {
            HirLocalValueOrigin::DirectInitializer(value) => Some(value),
            HirLocalValueOrigin::Independent | HirLocalValueOrigin::Composite => None,
        }
    }

    pub const fn statement_role(&self) -> Option<HirLocalBindingStatementRole> {
        match &self.site {
            HirBindingSite::Statement { role, .. } => Some(*role),
            HirBindingSite::DeclarationParameter { .. }
            | HirBindingSite::Expression { .. }
            | HirBindingSite::Member { .. }
            | HirBindingSite::FlowResult { .. }
            | HirBindingSite::PostconditionResult { .. } => None,
        }
    }

    pub const fn origin(&self) -> HirLocalValueOrigin {
        self.origin
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLocalBindingStatementRole {
    Let,
    LetElse,
    LetChoice,
    LetScope,
    LetActionReceive,
    IfLet,
    MatchArm { arm: u32 },
    WhileLet,
    For,
    SelectPattern { branch: u32 },
    SelectBinding { branch: u32 },
    OnTrigger,
}

/// Snapshot-bound local binding-origin index built as part of evaluation
/// topology construction. The module resolver does not expose an independent
/// statement-arena scan; this index is sealed with the topology snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirLocalBindingOriginIndex {
    snapshot: HirSnapshotId,
    origins: BTreeMap<LocalId, HirLocalBindingOrigin>,
}

impl HirLocalBindingOriginIndex {
    pub const fn snapshot(&self) -> HirSnapshotId {
        self.snapshot
    }

    pub fn origin(&self, local: LocalId) -> Option<HirLocalValueOrigin> {
        self.origins.get(&local).map(HirLocalBindingOrigin::origin)
    }

    pub fn binding(&self, local: LocalId) -> Option<&HirLocalBindingOrigin> {
        self.origins.get(&local)
    }

    pub fn rows(&self) -> impl Iterator<Item = &HirLocalBindingOrigin> {
        self.origins.values()
    }
}

/// One capture row joined to its closure, captured local, and access mode in
/// the same snapshot that owns the expression/path topology.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirCaptureEvaluationRow {
    capture: CaptureId,
    closure: ExprId,
    local: LocalId,
    access: CaptureAccess,
}

impl HirCaptureEvaluationRow {
    pub const fn capture(&self) -> CaptureId {
        self.capture
    }

    pub const fn closure(&self) -> ExprId {
        self.closure
    }

    pub const fn local(&self) -> LocalId {
        self.local
    }

    pub const fn access(&self) -> CaptureAccess {
        self.access
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct HirCaptureEvaluationIndex {
    snapshot: HirSnapshotId,
    rows: Box<[HirCaptureEvaluationRow]>,
    by_capture: BTreeMap<CaptureId, u32>,
    by_closure: BTreeMap<ExprId, Range<u32>>,
}

impl HirCaptureEvaluationIndex {
    pub const fn snapshot(&self) -> HirSnapshotId {
        self.snapshot
    }

    pub fn rows(&self) -> impl ExactSizeIterator<Item = &HirCaptureEvaluationRow> {
        self.rows.iter()
    }

    pub fn capture(&self, capture: CaptureId) -> Option<&HirCaptureEvaluationRow> {
        self.by_capture
            .get(&capture)
            .and_then(|index| self.rows.get(*index as usize))
    }

    pub fn captures_for_closure(&self, closure: ExprId) -> Option<&[HirCaptureEvaluationRow]> {
        self.by_closure
            .get(&closure)
            .map(|range| &self.rows[range.start as usize..range.end as usize])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirExpressionCallableBoundary {
    Call,
    ExplicitClosure,
}

impl HirExpressionCallableBoundary {
    pub const fn cuts(self, kind: HirPlaceholderKind) -> bool {
        matches!(self, Self::ExplicitClosure)
            || (matches!(self, Self::Call)
                && matches!(kind, HirPlaceholderKind::PartialApplication))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirExpressionUseRow {
    expression: ExprId,
    source_ordinal: u32,
    subtree_end_ordinal: u32,
    parent_expression: Option<ExprId>,
    capture_access: CaptureAccess,
    callable_boundary: Option<HirExpressionCallableBoundary>,
    placeholder: Option<HirPlaceholderKind>,
}

impl HirExpressionUseRow {
    pub const fn expression(&self) -> ExprId {
        self.expression
    }

    pub const fn source_ordinal(&self) -> u32 {
        self.source_ordinal
    }

    pub const fn subtree_end_ordinal(&self) -> u32 {
        self.subtree_end_ordinal
    }

    pub const fn parent_expression(&self) -> Option<ExprId> {
        self.parent_expression
    }

    pub const fn capture_access(&self) -> CaptureAccess {
        self.capture_access
    }

    pub const fn callable_boundary(&self) -> Option<HirExpressionCallableBoundary> {
        self.callable_boundary
    }

    pub const fn placeholder(&self) -> Option<HirPlaceholderKind> {
        self.placeholder
    }

    fn cuts_implicit_callable_region(&self, kind: HirPlaceholderKind) -> bool {
        self.callable_boundary
            .is_some_and(|boundary| boundary.cuts(kind))
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct HirExpressionUseIndex {
    snapshot: HirSnapshotId,
    rows: Box<[HirExpressionUseRow]>,
    by_expression: BTreeMap<ExprId, u32>,
}

/// Borrowed, topology-sealed implicit callable region.
///
/// Membership is derived from the canonical expression-use parent chain and
/// typed callable boundaries. No copied expression/local member set is
/// created by this view.
pub struct HirImplicitCallableRegion<'index> {
    index: &'index HirExpressionUseIndex,
    root: ExprId,
    kind: HirPlaceholderKind,
    start_ordinal: u32,
    end_ordinal: u32,
}

impl HirImplicitCallableRegion<'_> {
    pub const fn root(&self) -> ExprId {
        self.root
    }

    pub const fn kind(&self) -> HirPlaceholderKind {
        self.kind
    }

    pub fn contains_expression(&self, expression: ExprId) -> bool {
        let Some(row) = self.index.row(expression) else {
            return false;
        };
        if row.source_ordinal() < self.start_ordinal || row.source_ordinal() >= self.end_ordinal {
            return false;
        }
        self.index.region_contains(self.root, expression, self.kind)
    }

    pub fn contains_binding(&self, binding: &HirLocalBindingOrigin) -> bool {
        binding
            .binding_expression()
            .is_some_and(|expression| self.contains_expression(expression))
    }

    pub fn placeholders(&self) -> impl Iterator<Item = ExprId> + '_ {
        self.index.rows[self.start_ordinal as usize..self.end_ordinal as usize]
            .iter()
            .filter_map(|row| {
                (row.placeholder() == Some(self.kind) && self.contains_expression(row.expression()))
                    .then_some(row.expression())
            })
    }
}

impl HirExpressionUseIndex {
    pub const fn snapshot(&self) -> HirSnapshotId {
        self.snapshot
    }

    pub fn row(&self, expression: ExprId) -> Option<&HirExpressionUseRow> {
        self.by_expression
            .get(&expression)
            .and_then(|index| self.rows.get(*index as usize))
    }

    pub const fn rows(&self) -> &[HirExpressionUseRow] {
        &self.rows
    }

    fn region_contains(&self, root: ExprId, candidate: ExprId, kind: HirPlaceholderKind) -> bool {
        let candidate_root = candidate;
        let mut current = candidate;
        for _ in 0..=self.rows.len() {
            let Some(row) = self.row(current) else {
                return false;
            };
            if current == root {
                return current == candidate_root || !row.cuts_implicit_callable_region(kind);
            }
            if current != candidate_root && row.cuts_implicit_callable_region(kind) {
                return false;
            }
            let Some(parent) = row.parent_expression() else {
                return false;
            };
            current = parent;
        }
        false
    }

    pub fn implicit_callable_region(
        &self,
        root: ExprId,
        kind: HirPlaceholderKind,
    ) -> Result<HirImplicitCallableRegion<'_>, HirSemanticPathError> {
        let row = self
            .row(root)
            .ok_or(HirSemanticPathError::UnresolvedOwner)?;
        let start_ordinal = row.source_ordinal();
        let end_ordinal = row.subtree_end_ordinal();
        if start_ordinal >= end_ordinal
            || usize::try_from(end_ordinal).map_or(true, |end| end > self.rows.len())
        {
            return Err(HirSemanticPathError::InvalidOwnedPath);
        }
        Ok(HirImplicitCallableRegion {
            index: self,
            root,
            kind,
            start_ordinal,
            end_ordinal,
        })
    }
}

impl HirDeclarationContractRoot {
    pub const fn role(&self) -> HirDeclarationContractRootRole {
        self.role
    }

    pub const fn child(&self) -> ExprId {
        self.child
    }
}

impl HirDeclarationBodyRoot {
    pub const fn role(&self) -> HirDeclarationBodyRootRole {
        self.role
    }

    pub const fn projection(&self) -> &HirBodyProjection {
        &self.projection
    }
}

/// Complete, snapshot-bound declaration topology and its derived path rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDeclarationBodyTopology {
    declaration: CallableDeclarationKey,
    source_item: ItemId,
    source_owner: HirCallableSourceOwner,
    parameter_roots: Box<[HirDeclarationParameterRoot]>,
    contract_roots: Box<[HirDeclarationContractRoot]>,
    roots: Box<[HirDeclarationBodyRoot]>,
    paths: HirSemanticPathIndex,
}

/// Borrowed declaration view over one sealed item entry and the module-level
/// local/capture indexes. No declaration-local index is copied here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirDeclarationEvaluationView<'topology> {
    module: &'topology HirModuleEvaluationTopology,
    entry: &'topology HirItemEvaluationEntry,
    body: &'topology HirDeclarationBodyTopology,
}

/// One source-ordered declaration evaluation phase. Signature roots are
/// emitted before contracts and the optional body; no consumer needs to
/// reconstruct that order from three parallel inventories.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirDeclarationEvaluationPhase<'a> {
    Parameter(&'a HirDeclarationParameterRoot),
    Contract(&'a HirDeclarationContractRoot),
    Body(&'a HirDeclarationBodyRoot),
}

/// One typed expression-evaluation edge emitted by the module topology
/// traversal. Nested statement/body edges remain distinct from direct
/// expression edges so selection never rebuilds an arena graph from IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirExpressionEvaluationEdge {
    Expression {
        role: HirExpressionChildRole,
        ownership: crate::expr::HirExpressionChildOwnership,
        child: ExprId,
    },
    ExpressionOwnedBody {
        role: HirExpressionOwnedBodyRole,
        body_role: HirBodyChildRole,
        child: ExprId,
    },
    ExpressionOwnedStatement {
        role: HirExpressionOwnedBodyRole,
        statement_role: HirStatementChildRole,
        child: ExprId,
    },
    Body {
        role: HirBodyChildRole,
        child: ExprId,
    },
    Statement {
        role: HirStatementChildRole,
        child: ExprId,
    },
    ThreadBody {
        role: HirStatementBodyRole,
        child: ExprId,
    },
}

impl HirExpressionEvaluationEdge {
    pub const fn child(&self) -> ExprId {
        match self {
            Self::Expression { child, .. }
            | Self::ExpressionOwnedBody { child, .. }
            | Self::ExpressionOwnedStatement { child, .. }
            | Self::Body { child, .. }
            | Self::Statement { child, .. }
            | Self::ThreadBody { child, .. } => *child,
        }
    }
}

/// One source-ordered item/declaration/member entry in the project
/// evaluation topology. A callable signature and its body are retained in
/// the same entry, so inline member prefix → signature → body order cannot be
/// lost through parallel item/declaration arrays.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirItemEvaluationEntryRole {
    Item,
    InlineMember { member: u16 },
}

/// Accepted top-level item family witness. `Error` is deliberately absent:
/// recovered item payloads cannot enter an executable topology entry.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirAcceptedItemFamily(crate::item::HirItemFamily);

impl HirAcceptedItemFamily {
    pub(crate) const fn from_family(family: crate::item::HirItemFamily) -> Self {
        Self(family)
    }

    pub const fn family(self) -> crate::item::HirItemFamily {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirItemEvaluationEntry {
    item: ItemId,
    entry_ordinal: u32,
    role: HirItemEvaluationEntryRole,
    family: HirAcceptedItemFamily,
    roots: Box<[HirItemEvaluationRoot]>,
    paths: HirSemanticPathIndex,
    body: Option<HirDeclarationBodyTopology>,
}

impl HirItemEvaluationEntry {
    pub const fn item(&self) -> ItemId {
        self.item
    }

    pub const fn entry_ordinal(&self) -> u32 {
        self.entry_ordinal
    }

    pub const fn role(&self) -> HirItemEvaluationEntryRole {
        self.role
    }

    pub const fn family(&self) -> HirAcceptedItemFamily {
        self.family
    }

    pub const fn roots(&self) -> &[HirItemEvaluationRoot] {
        &self.roots
    }

    pub const fn paths(&self) -> &HirSemanticPathIndex {
        &self.paths
    }

    pub const fn body(&self) -> Option<&HirDeclarationBodyTopology> {
        self.body.as_ref()
    }
}

/// One exact module entry in source-ordered project evaluation topology.
#[derive(Debug, Eq, PartialEq)]
pub struct HirModuleEvaluationTopology {
    generation: Arc<super::AcceptedHirModuleGeneration>,
    entries: Box<[HirItemEvaluationEntry]>,
    local_origins: HirLocalBindingOriginIndex,
    captures: HirCaptureEvaluationIndex,
    expression_uses: HirExpressionUseIndex,
    selection_roots: Box<[ExprId]>,
    selection_edges: BTreeMap<ExprId, Box<[HirExpressionEvaluationEdge]>>,
}

impl HirModuleEvaluationTopology {
    pub fn module(&self) -> crate::identity::HirModuleId {
        self.generation.module()
    }

    pub fn snapshot(&self) -> HirSnapshotId {
        self.generation.snapshot()
    }

    pub fn generation(&self) -> &Arc<super::AcceptedHirModuleGeneration> {
        &self.generation
    }

    pub const fn entries(&self) -> &[HirItemEvaluationEntry] {
        &self.entries
    }

    pub const fn local_origins(&self) -> &HirLocalBindingOriginIndex {
        &self.local_origins
    }

    pub const fn captures(&self) -> &HirCaptureEvaluationIndex {
        &self.captures
    }

    pub const fn expression_uses(&self) -> &HirExpressionUseIndex {
        &self.expression_uses
    }

    pub fn expression_owners(&self) -> impl Iterator<Item = ExprId> + '_ {
        self.selection_edges.keys().copied()
    }

    pub const fn selection_roots(&self) -> &[ExprId] {
        &self.selection_roots
    }

    pub fn expression_edges(&self, owner: ExprId) -> &[HirExpressionEvaluationEdge] {
        self.selection_edges.get(&owner).map_or(&[], Box::as_ref)
    }
}

/// Complete deterministic project evaluation topology.
#[derive(Debug, Eq, PartialEq)]
pub struct HirProjectEvaluationTopology {
    generation: Arc<super::AcceptedHirProjectGeneration>,
    modules: Box<[HirModuleEvaluationTopology]>,
}

impl HirProjectEvaluationTopology {
    pub fn package(&self) -> &crate::symbol::CallablePackageId {
        self.generation.package()
    }

    pub fn generation(&self) -> &Arc<super::AcceptedHirProjectGeneration> {
        &self.generation
    }

    pub const fn modules(&self) -> &[HirModuleEvaluationTopology] {
        &self.modules
    }

    pub fn module(
        &self,
        module: crate::identity::HirModuleId,
    ) -> Option<&HirModuleEvaluationTopology> {
        self.modules.iter().find(|value| value.module() == module)
    }

    pub fn expression_owners(&self) -> impl Iterator<Item = ExprId> + '_ {
        self.modules
            .iter()
            .flat_map(HirModuleEvaluationTopology::expression_owners)
    }

    pub fn selection_roots(&self) -> impl Iterator<Item = ExprId> + '_ {
        self.modules
            .iter()
            .flat_map(|module| module.selection_roots().iter().copied())
    }

    pub fn expression_edges(&self, owner: ExprId) -> &[HirExpressionEvaluationEdge] {
        self.modules
            .iter()
            .find(|module| module.module() == owner.module())
            .map_or(&[], |module| module.expression_edges(owner))
    }

    pub fn declaration(
        &self,
        declaration: &CallableDeclarationKey,
    ) -> Result<HirDeclarationEvaluationView<'_>, HirSemanticPathError> {
        let mut found = None;
        for module in &self.modules {
            for entry in module.entries() {
                if let Some(body) = entry.body()
                    && body.declaration() == declaration
                {
                    if found.is_some() {
                        return Err(HirSemanticPathError::DeclarationUnavailable);
                    }
                    found = Some(HirDeclarationEvaluationView {
                        module,
                        entry,
                        body,
                    });
                }
            }
        }
        found.ok_or(HirSemanticPathError::DeclarationUnavailable)
    }

    pub fn declaration_semantic_paths(
        &self,
        declaration: &CallableDeclarationKey,
    ) -> Result<&HirSemanticPathIndex, HirSemanticPathError> {
        self.declaration(declaration).map(|view| view.paths())
    }

    /// Returns the sole accepted structural path for one path-owning HIR
    /// identity. Lookup is constrained to the owner's module, preserves entry
    /// source order, and rejects every second occurrence even when its path is
    /// byte-for-byte identical to the first.
    pub fn semantic_path(
        &self,
        owner: HirSemanticPathOwnerId,
    ) -> Result<Option<HirSemanticPathLocation<'_>>, HirSemanticPathLookupError> {
        let Some(module) = self.module(owner.module()) else {
            return Ok(None);
        };
        let mut found = None;
        for entry in &module.entries {
            record_semantic_path_location(&mut found, owner, &entry.paths)?;
            if let Some(body) = &entry.body {
                record_semantic_path_location(&mut found, owner, &body.paths)?;
            }
        }
        Ok(found)
    }

    /// Returns the sole root-local control-transfer row for one statement.
    /// The returned location borrows the sealed topology and therefore keeps
    /// the exact root, path, and resolved target joined together.
    pub fn control_transfer(
        &self,
        statement: StmtId,
    ) -> Result<HirControlTransferLocation<'_>, HirControlTransferLookupError> {
        let Some(module) = self.module(statement.module()) else {
            return Err(HirControlTransferLookupError::Missing { statement });
        };
        let mut found = None;
        for entry in &module.entries {
            record_control_transfer_location(&mut found, statement, &entry.paths)?;
            if let Some(body) = &entry.body {
                record_control_transfer_location(&mut found, statement, &body.paths)?;
            }
        }
        found.ok_or(HirControlTransferLookupError::Missing { statement })
    }

    /// Returns only the resolved control-transfer row for one statement.
    pub fn control_transfer_row(
        &self,
        statement: StmtId,
    ) -> Result<&HirControlTransferRow, HirControlTransferLookupError> {
        self.control_transfer(statement)
            .map(HirControlTransferLocation::row)
    }

    /// Returns the sole body row named by an exact root-relative locator.
    pub fn semantic_body(
        &self,
        locator: &HirSemanticBodyLocator,
    ) -> Result<Option<HirSemanticBodyLocation<'_>>, HirSemanticBodyLookupError> {
        let root_owner_valid = match locator.root() {
            HirSemanticPathRoot::Declaration(_) => locator.owner().item_role().is_none(),
            HirSemanticPathRoot::Item { .. } => locator.owner().declaration_role().is_none(),
        };
        if !root_owner_valid {
            return Err(HirSemanticBodyLookupError::InvalidLocator {
                locator: Box::new(locator.clone()),
            });
        }
        let mut found = None;
        for module in &self.modules {
            for entry in &module.entries {
                record_semantic_body_location(&mut found, locator, &entry.paths)?;
                if let Some(body) = &entry.body {
                    record_semantic_body_location(&mut found, locator, &body.paths)?;
                }
            }
        }
        Ok(found)
    }
}

fn record_semantic_path_location<'topology>(
    found: &mut Option<HirSemanticPathLocation<'topology>>,
    owner: HirSemanticPathOwnerId,
    index: &'topology HirSemanticPathIndex,
) -> Result<(), HirSemanticPathLookupError> {
    let Some(path) = owner.path_in(index) else {
        return Ok(());
    };
    if owner.module() != index.snapshot().module() {
        return Err(HirSemanticPathLookupError::OwnerModuleMismatch {
            owner,
            snapshot: index.snapshot(),
        });
    }
    if found.is_some() {
        return Err(HirSemanticPathLookupError::DuplicateOwner { owner });
    }
    *found = Some(HirSemanticPathLocation {
        owner,
        snapshot: index.snapshot(),
        root: index.root(),
        path,
    });
    Ok(())
}

fn record_control_transfer_location<'topology>(
    found: &mut Option<HirControlTransferLocation<'topology>>,
    statement: StmtId,
    index: &'topology HirSemanticPathIndex,
) -> Result<(), HirControlTransferLookupError> {
    if statement.module() != index.snapshot().module() {
        return Err(HirControlTransferLookupError::ModuleMismatch {
            statement,
            snapshot: index.snapshot(),
        });
    }
    let Some(row) = index.control_transfer(statement) else {
        return Ok(());
    };
    let Some(path) = index.statement(statement) else {
        return Err(HirControlTransferLookupError::Missing { statement });
    };
    if found.is_some() {
        return Err(HirControlTransferLookupError::Duplicate { statement });
    }
    *found = Some(HirControlTransferLocation {
        snapshot: index.snapshot(),
        root: index.root(),
        path,
        row,
    });
    Ok(())
}

fn record_semantic_body_location<'topology>(
    found: &mut Option<HirSemanticBodyLocation<'topology>>,
    locator: &HirSemanticBodyLocator,
    index: &'topology HirSemanticPathIndex,
) -> Result<(), HirSemanticBodyLookupError> {
    if index.root() != locator.root() {
        return Ok(());
    }
    let Some(row) = index.body_row(locator.owner()) else {
        return Ok(());
    };
    if row
        .owner()
        .expression_owner()
        .is_some_and(|owner| owner.module() != index.snapshot().module())
        || row
            .owner()
            .statement_owner()
            .is_some_and(|owner| owner.module() != index.snapshot().module())
    {
        return Err(HirSemanticBodyLookupError::OwnerModuleMismatch {
            owner: row.owner().clone(),
            snapshot: index.snapshot(),
        });
    }
    if found.is_some() {
        return Err(HirSemanticBodyLookupError::DuplicateBody {
            locator: Box::new(locator.clone()),
        });
    }
    *found = Some(HirSemanticBodyLocation {
        snapshot: index.snapshot(),
        root: index.root(),
        row,
    });
    Ok(())
}

impl AcceptedHirProjectSymbolGeneration<'_, '_> {
    /// Consumes the accepted witness and mints the sole project topology.
    pub fn into_evaluation_topology(
        self,
    ) -> Result<Arc<HirProjectEvaluationTopology>, HirSemanticPathError> {
        HirProjectEvaluationTopologyBuilder::build_project(&self).map(Arc::new)
    }
}

impl HirDeclarationBodyTopology {
    pub const fn declaration(&self) -> &CallableDeclarationKey {
        &self.declaration
    }

    pub const fn snapshot(&self) -> HirSnapshotId {
        self.paths.snapshot()
    }

    pub const fn source_item(&self) -> ItemId {
        self.source_item
    }

    pub const fn source_owner(&self) -> HirCallableSourceOwner {
        self.source_owner
    }

    pub const fn parameter_roots(&self) -> &[HirDeclarationParameterRoot] {
        &self.parameter_roots
    }

    pub const fn roots(&self) -> &[HirDeclarationBodyRoot] {
        &self.roots
    }

    pub const fn contract_roots(&self) -> &[HirDeclarationContractRoot] {
        &self.contract_roots
    }

    pub const fn paths(&self) -> &HirSemanticPathIndex {
        &self.paths
    }

    pub fn phases(&self) -> impl Iterator<Item = HirDeclarationEvaluationPhase<'_>> {
        self.parameter_roots
            .iter()
            .map(HirDeclarationEvaluationPhase::Parameter)
            .chain(
                self.contract_roots
                    .iter()
                    .map(HirDeclarationEvaluationPhase::Contract),
            )
            .chain(self.roots.iter().map(HirDeclarationEvaluationPhase::Body))
    }
}

impl<'topology> HirDeclarationEvaluationView<'topology> {
    pub const fn module(&self) -> &HirModuleEvaluationTopology {
        self.module
    }

    pub const fn entry(&self) -> &HirItemEvaluationEntry {
        self.entry
    }

    pub const fn body(&self) -> &HirDeclarationBodyTopology {
        self.body
    }

    pub const fn paths(&self) -> &'topology HirSemanticPathIndex {
        self.body.paths()
    }

    pub fn local_origin(&self, local: LocalId) -> Option<&HirLocalBindingOrigin> {
        self.body
            .paths()
            .local(local)
            .and_then(|_| self.module.local_origins().binding(local))
    }

    pub fn captures(&self) -> impl Iterator<Item = &HirCaptureEvaluationRow> {
        let paths = self.body.paths();
        self.module
            .captures()
            .rows()
            .filter(move |row| paths.expression(row.closure()).is_some())
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirSemanticPathError {
    #[error("semantic path symbol world does not match the executable project")]
    SymbolWorldMismatch,
    #[error("semantic path declaration is absent or ambiguous")]
    DeclarationUnavailable,
    #[error("semantic path declaration belongs to a foreign HIR snapshot")]
    ForeignSnapshot,
    #[error("semantic path declaration has no executable body")]
    MissingBody,
    #[error("semantic path references an unresolved HIR owner")]
    UnresolvedOwner,
    #[error("HIR owner {owner:?} is reachable through more than one rooted semantic path")]
    DuplicatePath { owner: HirSemanticPathOwnerId },
    #[error("semantic path recursion is cyclic at {owner:?}")]
    CyclicPath { owner: HirSemanticPathOwnerId },
    #[error("expression use row is duplicated for {owner:?}")]
    DuplicateExpressionUse { owner: ExprId },
    #[error("closure capture range is duplicated for {owner:?}")]
    DuplicateClosureCaptureRange { owner: ExprId },
    #[error("capture row is duplicated for {owner:?}")]
    DuplicateCapture { owner: CaptureId },
    #[error("local binding origin is duplicated for {owner:?}")]
    DuplicateLocalOrigin { owner: LocalId },
    #[error("HIR semantic path owner {owner:?} belongs to a module other than {snapshot:?}")]
    OwnerModuleMismatch {
        owner: HirSemanticPathOwnerId,
        snapshot: HirSnapshotId,
    },
    #[error("HIR semantic path owner {owner:?} has an invalid rooted structural path")]
    InvalidOwnerPath { owner: HirSemanticPathOwnerId },
    #[error("HIR semantic path owner {owner:?} has invalid expression-hop evidence")]
    InvalidExpressionHops { owner: HirSemanticPathOwnerId },
    #[error("HIR semantic path owners {first:?} and {second:?} share one structural path")]
    DuplicateStructuralPath {
        first: HirSemanticPathOwnerId,
        second: HirSemanticPathOwnerId,
    },
    #[error("semantic body owner {owner:?} occurs more than once in one path index")]
    DuplicateBodyOwner { owner: HirSemanticBodyOwner },
    #[error("semantic body row for {owner:?} has invalid children or path")]
    InvalidBodyRow { owner: HirSemanticBodyOwner },
    #[error(transparent)]
    BodyProjection(#[from] HirBodyProjectionError),
    #[error(transparent)]
    ExpressionOwnedBodyProjection(#[from] HirExpressionOwnedBodyProjectionError),
    #[error(transparent)]
    StatementBodyProjection(#[from] HirStatementBodyProjectionError),
    #[error("a semantic path child ordinal does not fit u32")]
    OrdinalOverflow,
    #[error("an expression-owned semantic path lacks a structural coordinate")]
    InvalidOwnedPath,
    #[error("a declaration result local has an invalid owner/path join")]
    InvalidResultPath,
    #[error("a declaration result local has an invalid HIR origin")]
    InvalidResultOrigin,
    #[error("control-transfer row for {statement:?} is duplicated")]
    DuplicateControlTransfer { statement: StmtId },
    #[error("control-transfer row for {statement:?} is invalid for its rooted path")]
    InvalidControlTransferRow { statement: StmtId },
    #[error("control-transfer statement {statement:?} belongs to a module other than {snapshot:?}")]
    ControlTransferModuleMismatch {
        statement: StmtId,
        snapshot: HirSnapshotId,
    },
    #[error(transparent)]
    ControlTransfer(#[from] HirControlTransferResolutionError),
}

/// Typed failure while resolving one control-transfer statement during the
/// HIR topology seal.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum HirControlTransferResolutionError {
    #[error("control-transfer statement {statement:?} has an unresolved {kind:?} target")]
    UnresolvedTarget {
        statement: StmtId,
        kind: HirControlTransferKind,
    },
    #[error("break statement {statement:?} with a value requires a loop expression target")]
    BreakValueRequiresLoopExpression { statement: StmtId },
    #[error("control-transfer statement {statement:?} has an invalid lexical scope chain")]
    InvalidScopeChain { statement: StmtId },
    #[error("out statement {statement:?} has an invalid line-plan application target")]
    InvalidOutputApplication { statement: StmtId },
}

impl From<HirExpressionOwnedChildEdgeError> for HirSemanticPathError {
    fn from(error: HirExpressionOwnedChildEdgeError) -> Self {
        match error {
            HirExpressionOwnedChildEdgeError::OrdinalOverflow => Self::OrdinalOverflow,
            HirExpressionOwnedChildEdgeError::EmptyNestedPath => Self::InvalidOwnedPath,
            HirExpressionOwnedChildEdgeError::Body(error) => Self::BodyProjection(error),
        }
    }
}

/// Closed failure vocabulary for topology-wide borrowed path lookup.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirSemanticPathLookupError {
    #[error("HIR semantic path owner {owner:?} has more than one stored rooted path")]
    DuplicateOwner { owner: HirSemanticPathOwnerId },
    #[error("HIR semantic path owner {owner:?} disagrees with stored snapshot {snapshot:?}")]
    OwnerModuleMismatch {
        owner: HirSemanticPathOwnerId,
        snapshot: HirSnapshotId,
    },
}

/// Closed failure vocabulary for topology-wide borrowed control-transfer
/// lookup.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum HirControlTransferLookupError {
    #[error("control-transfer statement {statement:?} has no stored semantic target")]
    Missing { statement: StmtId },
    #[error("control-transfer statement {statement:?} has more than one stored semantic target")]
    Duplicate { statement: StmtId },
    #[error("control-transfer statement {statement:?} disagrees with stored snapshot {snapshot:?}")]
    ModuleMismatch {
        statement: StmtId,
        snapshot: HirSnapshotId,
    },
}

/// Closed failure vocabulary for topology-wide borrowed body lookup.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirSemanticBodyLookupError {
    #[error("semantic body locator has an incompatible root and owner: {locator:?}")]
    InvalidLocator {
        locator: Box<HirSemanticBodyLocator>,
    },
    #[error("semantic body locator resolves to more than one row: {locator:?}")]
    DuplicateBody {
        locator: Box<HirSemanticBodyLocator>,
    },
    #[error("semantic body owner {owner:?} disagrees with stored snapshot {snapshot:?}")]
    OwnerModuleMismatch {
        owner: HirSemanticBodyOwner,
        snapshot: HirSnapshotId,
    },
}

impl HirExecutableProjectView<'_> {
    #[allow(
        clippy::too_many_lines,
        reason = "one project seal preserves authored module, item, member, declaration, and local order"
    )]
    fn build_project_topology(
        self,
        generation: &AcceptedHirProjectSymbolGeneration<'_, '_>,
    ) -> Result<HirProjectEvaluationTopology, HirSemanticPathError> {
        let symbols = generation.symbols();
        let project_generation = Arc::clone(generation.generation());
        if symbols.world().package() != self.package() {
            return Err(HirSemanticPathError::SymbolWorldMismatch);
        }
        let mut modules = Vec::new();
        for (path, module) in self.modules() {
            let module_generation = project_generation
                .module(path)
                .ok_or(HirSemanticPathError::UnresolvedOwner)?;
            project_generation
                .validate_module_lease(module, symbols)
                .map_err(|_| HirSemanticPathError::ForeignSnapshot)?;
            let mut builder = HirProjectEvaluationTopologyBuilder::new_for_module(module);
            let mut entries = Vec::new();
            for item in module.source_ordered_items().iter().copied() {
                let entry_ordinal = checked_ordinal(entries.len())?;
                let item_path_checkpoint = builder.path_checkpoint();
                builder.record_item_member_origins(item)?;
                let roots = item_evaluation_roots(module, item)?;
                for root in &roots {
                    builder.walk_item_root(root)?;
                }
                let item_paths = builder.path_index_since(
                    HirSemanticPathRoot::Item {
                        item,
                        entry_ordinal,
                        role: HirItemEvaluationEntryRole::Item,
                    },
                    &item_path_checkpoint,
                )?;
                let primary_owner = Self::callable_source_owners(module, item)?
                    .into_iter()
                    .find(|owner| {
                        matches!(
                            owner,
                            HirCallableSourceOwner::Item | HirCallableSourceOwner::ViewItem
                        )
                    });
                let item_body = if let Some(owner) = primary_owner {
                    let Some(symbol) =
                        symbols.callable_at_source(module.snapshot_id(), item, owner)
                    else {
                        return Err(HirSemanticPathError::DeclarationUnavailable);
                    };
                    let declaration = symbol.declaration().clone();
                    let parameter_roots = declaration_parameter_roots(module, item, owner)?;
                    let contract_roots = declaration_contract_roots(module, item, owner)?;
                    let body_roots = declaration_body_roots(module, item, owner)?;
                    Some(builder.walk_declaration_topology(
                        declaration,
                        item,
                        owner,
                        &parameter_roots,
                        &contract_roots,
                        &body_roots,
                    )?)
                } else {
                    None
                };
                entries.push(HirItemEvaluationEntry {
                    item,
                    entry_ordinal,
                    role: HirItemEvaluationEntryRole::Item,
                    family: module
                        .resolve_item(item)
                        .map_err(|_| HirSemanticPathError::UnresolvedOwner)?
                        .family()
                        .accepted()
                        .ok_or(HirSemanticPathError::UnresolvedOwner)?,
                    roots: roots.into_boxed_slice(),
                    paths: item_paths,
                    body: item_body,
                });
                for member in Self::inline_member_ordinals(module, item)? {
                    let owner = Self::inline_callable_owner(module, item, member)?;
                    let entry_ordinal = checked_ordinal(entries.len())?;
                    let member_role = HirItemEvaluationEntryRole::InlineMember { member };
                    let member_path_checkpoint = builder.path_checkpoint();
                    let roots = inline_member_roots(module, item, member, owner)?;
                    for root in &roots {
                        builder.walk_item_root(root)?;
                    }
                    let member_paths = builder.path_index_since(
                        HirSemanticPathRoot::Item {
                            item,
                            entry_ordinal,
                            role: member_role,
                        },
                        &member_path_checkpoint,
                    )?;
                    let body = if let Some(owner) = owner {
                        let Some(symbol) =
                            symbols.callable_at_source(module.snapshot_id(), item, owner)
                        else {
                            return Err(HirSemanticPathError::DeclarationUnavailable);
                        };
                        let declaration = symbol.declaration().clone();
                        let parameter_roots = declaration_parameter_roots(module, item, owner)?;
                        let contract_roots = declaration_contract_roots(module, item, owner)?;
                        let body_roots = declaration_body_roots(module, item, owner)?;
                        Some(builder.walk_declaration_topology(
                            declaration,
                            item,
                            owner,
                            &parameter_roots,
                            &contract_roots,
                            &body_roots,
                        )?)
                    } else {
                        None
                    };
                    entries.push(HirItemEvaluationEntry {
                        item,
                        entry_ordinal,
                        role: member_role,
                        family: module
                            .resolve_item(item)
                            .map_err(|_| HirSemanticPathError::UnresolvedOwner)?
                            .family()
                            .accepted()
                            .ok_or(HirSemanticPathError::UnresolvedOwner)?,
                        roots: roots.into_boxed_slice(),
                        paths: member_paths,
                        body,
                    });
                }
            }
            let (local_origins, selection_roots, selection_edges, captures, expression_uses) =
                builder.finish_module()?;
            modules.push(HirModuleEvaluationTopology {
                generation: Arc::clone(module_generation),
                entries: entries.into_boxed_slice(),
                local_origins,
                selection_roots,
                selection_edges,
                captures,
                expression_uses,
            });
        }
        Ok(HirProjectEvaluationTopology {
            generation: project_generation,
            modules: modules.into_boxed_slice(),
        })
    }

    fn inline_member_ordinals(
        module: &HirModule,
        item: ItemId,
    ) -> Result<Vec<u16>, HirSemanticPathError> {
        let item = module
            .resolve_item(item)
            .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
        let length = match item.kind() {
            HirItemKind::Trait(value) => value.members().len(),
            HirItemKind::Impl(value) => value.members().len(),
            HirItemKind::ExternCapability(value) => value.members().len(),
            _ => 0,
        };
        (0..length)
            .map(|member| u16::try_from(member).map_err(|_| HirSemanticPathError::OrdinalOverflow))
            .collect()
    }

    fn inline_callable_owner(
        module: &HirModule,
        item: ItemId,
        member: u16,
    ) -> Result<Option<HirCallableSourceOwner>, HirSemanticPathError> {
        let item = module
            .resolve_item(item)
            .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
        let owner = match item.kind() {
            HirItemKind::Trait(value)
                if matches!(
                    value.members().get(usize::from(member)),
                    Some(crate::item::HirTraitMember::Function(_))
                ) =>
            {
                Some(HirCallableSourceOwner::TraitFunction { member })
            }
            HirItemKind::Impl(value)
                if matches!(
                    value.members().get(usize::from(member)),
                    Some(HirImplMember::Function(_))
                ) =>
            {
                Some(HirCallableSourceOwner::ImplFunction { member })
            }
            HirItemKind::ExternCapability(value)
                if matches!(
                    value.members().get(usize::from(member)),
                    Some(crate::item::HirCapabilityMember::Function(_))
                ) =>
            {
                Some(HirCallableSourceOwner::ExternCapabilityFunction { member })
            }
            HirItemKind::Trait(_) | HirItemKind::Impl(_) | HirItemKind::ExternCapability(_) => None,
            _ => return Err(HirSemanticPathError::InvalidOwnedPath),
        };
        Ok(owner)
    }

    fn callable_source_owners(
        module: &HirModule,
        item: ItemId,
    ) -> Result<Vec<HirCallableSourceOwner>, HirSemanticPathError> {
        let item = module
            .resolve_item(item)
            .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
        let mut owners = Vec::new();
        match item.kind() {
            HirItemKind::Function(_)
            | HirItemKind::Predicate(_)
            | HirItemKind::Proof(_)
            | HirItemKind::Flow(_) => owners.push(HirCallableSourceOwner::Item),
            HirItemKind::View(_) => owners.push(HirCallableSourceOwner::ViewItem),
            HirItemKind::Trait(value) => {
                for (member, value) in value.members().iter().enumerate() {
                    if matches!(value, crate::item::HirTraitMember::Function(_)) {
                        owners.push(HirCallableSourceOwner::TraitFunction {
                            member: u16::try_from(member)
                                .map_err(|_| HirSemanticPathError::OrdinalOverflow)?,
                        });
                    }
                }
            }
            HirItemKind::Impl(value) => {
                for (member, value) in value.members().iter().enumerate() {
                    if matches!(value, HirImplMember::Function(_)) {
                        owners.push(HirCallableSourceOwner::ImplFunction {
                            member: u16::try_from(member)
                                .map_err(|_| HirSemanticPathError::OrdinalOverflow)?,
                        });
                    }
                }
            }
            HirItemKind::ExternCapability(value) => {
                for (member, value) in value.members().iter().enumerate() {
                    if matches!(value, crate::item::HirCapabilityMember::Function(_)) {
                        owners.push(HirCallableSourceOwner::ExternCapabilityFunction {
                            member: u16::try_from(member)
                                .map_err(|_| HirSemanticPathError::OrdinalOverflow)?,
                        });
                    }
                }
            }
            HirItemKind::Module(_)
            | HirItemKind::Use(_)
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
            | HirItemKind::Test(_)
            | HirItemKind::Bench(_)
            | HirItemKind::Style(_)
            | HirItemKind::Error(_) => {}
        }
        Ok(owners)
    }
}

fn declaration_parameter_roots(
    module: &HirModule,
    item: ItemId,
    owner: HirCallableSourceOwner,
) -> Result<Vec<HirDeclarationParameterRoot>, HirSemanticPathError> {
    let item = module
        .resolve_item(item)
        .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
    let mut roots = Vec::new();
    match owner {
        HirCallableSourceOwner::Item => match item.kind() {
            HirItemKind::Function(function) => {
                for (group, parameters) in function.parameter_groups().iter().enumerate() {
                    push_parameters(&mut roots, checked_ordinal(group)?, parameters.parameters())?;
                }
            }
            HirItemKind::Predicate(predicate) => {
                push_parameters(&mut roots, 0, predicate.parameters())?;
            }
            HirItemKind::Proof(proof) => push_parameters(&mut roots, 0, proof.parameters())?,
            HirItemKind::Flow(flow) => push_parameters(&mut roots, 0, flow.parameters())?,
            _ => {}
        },
        HirCallableSourceOwner::ViewItem => {
            let HirItemKind::View(view) = item.kind() else {
                return Err(HirSemanticPathError::MissingBody);
            };
            push_parameters(&mut roots, 0, view.parameters())?;
        }
        HirCallableSourceOwner::TraitFunction { member } => {
            let HirItemKind::Trait(value) = item.kind() else {
                return Err(HirSemanticPathError::MissingBody);
            };
            let Some(crate::item::HirTraitMember::Function(value)) =
                value.members().get(usize::from(member))
            else {
                return Err(HirSemanticPathError::MissingBody);
            };
            push_method_parameters(&mut roots, value.parameter_groups())?;
        }
        HirCallableSourceOwner::ImplFunction { member } => {
            let HirItemKind::Impl(value) = item.kind() else {
                return Err(HirSemanticPathError::MissingBody);
            };
            let Some(HirImplMember::Function(value)) = value.members().get(usize::from(member))
            else {
                return Err(HirSemanticPathError::MissingBody);
            };
            push_method_parameters(&mut roots, value.parameter_groups())?;
        }
        HirCallableSourceOwner::ExternCapabilityFunction { member } => {
            let HirItemKind::ExternCapability(value) = item.kind() else {
                return Err(HirSemanticPathError::MissingBody);
            };
            let Some(crate::item::HirCapabilityMember::Function(value)) =
                value.members().get(usize::from(member))
            else {
                return Err(HirSemanticPathError::MissingBody);
            };
            for (group, parameters) in value.parameter_groups().iter().enumerate() {
                push_parameters(&mut roots, checked_ordinal(group)?, parameters.parameters())?;
            }
        }
    }
    Ok(roots)
}

fn push_parameters(
    roots: &mut Vec<HirDeclarationParameterRoot>,
    group: u32,
    parameters: &[HirParameter],
) -> Result<(), HirSemanticPathError> {
    for (parameter, value) in parameters.iter().enumerate() {
        push_parameter(roots, group, checked_ordinal(parameter)?, value);
    }
    Ok(())
}

fn push_method_parameters(
    roots: &mut Vec<HirDeclarationParameterRoot>,
    groups: &[HirMethodParameterGroup],
) -> Result<(), HirSemanticPathError> {
    for (group, values) in groups.iter().enumerate() {
        let group = checked_ordinal(group)?;
        for (parameter, value) in values.parameters().iter().enumerate() {
            let parameter = checked_ordinal(parameter)?;
            match value {
                HirMethodParameter::Receiver(receiver) => roots.push(HirDeclarationParameterRoot {
                    role: HirDeclarationParameterRootRole::Pattern { group, parameter },
                    child: HirDeclarationParameterRootChild::Pattern(receiver.pattern()),
                }),
                HirMethodParameter::Typed(value) => {
                    push_parameter(roots, group, parameter, value);
                }
            }
        }
    }
    Ok(())
}

fn push_parameter(
    roots: &mut Vec<HirDeclarationParameterRoot>,
    group: u32,
    parameter: u32,
    value: &HirParameter,
) {
    roots.push(HirDeclarationParameterRoot {
        role: HirDeclarationParameterRootRole::Pattern { group, parameter },
        child: HirDeclarationParameterRootChild::Pattern(value.pattern()),
    });
    if let Some(default) = value.default() {
        roots.push(HirDeclarationParameterRoot {
            role: HirDeclarationParameterRootRole::Default { group, parameter },
            child: HirDeclarationParameterRootChild::Expression(default),
        });
    }
}

fn checked_ordinal(value: usize) -> Result<u32, HirSemanticPathError> {
    u32::try_from(value).map_err(|_| HirSemanticPathError::OrdinalOverflow)
}

fn declaration_body_roots(
    module: &HirModule,
    item: ItemId,
    owner: HirCallableSourceOwner,
) -> Result<Vec<HirDeclarationBodyRoot>, HirSemanticPathError> {
    let item = module
        .resolve_item(item)
        .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
    match owner {
        HirCallableSourceOwner::Item => match item.kind() {
            HirItemKind::Function(function) => Ok(vec![declaration_body(
                HirDeclarationBodyRootRole::FunctionBody,
                function.body().try_body_projection()?,
            )]),
            HirItemKind::Predicate(predicate) => Ok(vec![declaration_body(
                HirDeclarationBodyRootRole::PredicateBody,
                predicate.body().try_body_projection()?,
            )]),
            HirItemKind::Proof(proof) => Ok(vec![declaration_body(
                HirDeclarationBodyRootRole::ProofBody,
                proof.body().try_body_projection()?,
            )]),
            HirItemKind::Flow(flow) => Ok(vec![declaration_body(
                HirDeclarationBodyRootRole::FlowBody,
                flow.body().try_body_projection()?,
            )]),
            _ => Err(HirSemanticPathError::MissingBody),
        },
        HirCallableSourceOwner::ImplFunction { member } => {
            let HirItemKind::Impl(implementation) = item.kind() else {
                return Err(HirSemanticPathError::MissingBody);
            };
            let Some(HirImplMember::Function(function)) =
                implementation.members().get(usize::from(member))
            else {
                return Err(HirSemanticPathError::MissingBody);
            };
            function
                .body()
                .map(|body| {
                    body.try_body_projection()
                        .map(|projection| {
                            declaration_body(
                                HirDeclarationBodyRootRole::ImplFunctionBody,
                                projection,
                            )
                        })
                        .map_err(HirSemanticPathError::from)
                })
                .transpose()
                .map(|value| value.into_iter().collect())
        }
        HirCallableSourceOwner::ViewItem => {
            let HirItemKind::View(view) = item.kind() else {
                return Err(HirSemanticPathError::MissingBody);
            };
            view.values()
                .iter()
                .copied()
                .enumerate()
                .map(|(ordinal, expression)| {
                    Ok(HirDeclarationBodyRoot {
                        role: HirDeclarationBodyRootRole::ViewValue {
                            ordinal: checked_ordinal(ordinal)?,
                        },
                        projection: HirBodyProjection::expression(expression),
                    })
                })
                .collect()
        }
        HirCallableSourceOwner::ExternCapabilityFunction { .. }
        | HirCallableSourceOwner::TraitFunction { .. } => Ok(Vec::new()),
    }
}

fn declaration_contract_roots(
    module: &HirModule,
    item: ItemId,
    owner: HirCallableSourceOwner,
) -> Result<Vec<HirDeclarationContractRoot>, HirSemanticPathError> {
    let item = module
        .resolve_item(item)
        .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
    match owner {
        HirCallableSourceOwner::Item => match item.kind() {
            HirItemKind::Function(function) => {
                let mut roots = expression_contract_roots(function.requires(), function.ensures())?;
                roots.extend(effect_contract_roots(function.effect_clauses())?);
                Ok(roots)
            }
            HirItemKind::Predicate(predicate) => {
                expression_contract_roots(predicate.requires(), predicate.ensures())
            }
            HirItemKind::Proof(proof) => {
                expression_contract_roots(proof.requires(), proof.ensures())
            }
            HirItemKind::Flow(flow) => flow_contract_roots(flow.contracts()),
            _ => Ok(Vec::new()),
        },
        HirCallableSourceOwner::ExternCapabilityFunction { member } => {
            let HirItemKind::ExternCapability(value) = item.kind() else {
                return Ok(Vec::new());
            };
            let Some(crate::item::HirCapabilityMember::Function(function)) =
                value.members().get(usize::from(member))
            else {
                return Err(HirSemanticPathError::MissingBody);
            };
            function
                .effects()
                .iter()
                .copied()
                .enumerate()
                .map(|(operand, child)| {
                    Ok(HirDeclarationContractRoot {
                        role: HirDeclarationContractRootRole::EffectOperand {
                            clause: u32::from(member),
                            family: HirFlowContractRootFamily::Effects,
                            operand: checked_ordinal(operand)?,
                        },
                        child,
                    })
                })
                .collect()
        }
        _ => Ok(Vec::new()),
    }
}

fn declaration_body(
    role: HirDeclarationBodyRootRole,
    projection: HirBodyProjection,
) -> HirDeclarationBodyRoot {
    HirDeclarationBodyRoot { role, projection }
}

fn effect_contract_roots(
    clauses: &[crate::item::HirContractOperandList],
) -> Result<Vec<HirDeclarationContractRoot>, HirSemanticPathError> {
    clauses
        .iter()
        .enumerate()
        .try_fold(Vec::new(), |mut roots, (clause, operands)| {
            let clause = checked_ordinal(clause)?;
            for (operand, expression) in operands.operands().iter().copied().enumerate() {
                roots.push(HirDeclarationContractRoot {
                    role: HirDeclarationContractRootRole::EffectOperand {
                        clause,
                        family: HirFlowContractRootFamily::Effects,
                        operand: checked_ordinal(operand)?,
                    },
                    child: expression,
                });
            }
            Ok(roots)
        })
}

fn expression_contract_roots(
    requires: &[ExprId],
    ensures: &[ExprId],
) -> Result<Vec<HirDeclarationContractRoot>, HirSemanticPathError> {
    let mut roots = Vec::with_capacity(requires.len() + ensures.len());
    for (ordinal, expression) in requires.iter().copied().enumerate() {
        roots.push(HirDeclarationContractRoot {
            role: HirDeclarationContractRootRole::Requires {
                ordinal: checked_ordinal(ordinal)?,
            },
            child: expression,
        });
    }
    for (ordinal, expression) in ensures.iter().copied().enumerate() {
        roots.push(HirDeclarationContractRoot {
            role: HirDeclarationContractRootRole::Ensures {
                ordinal: checked_ordinal(ordinal)?,
            },
            child: expression,
        });
    }
    Ok(roots)
}

fn flow_contract_roots(
    clauses: &[crate::item::HirFlowContractClause],
) -> Result<Vec<HirDeclarationContractRoot>, HirSemanticPathError> {
    clauses
        .iter()
        .enumerate()
        .try_fold(Vec::new(), |mut roots, (clause, value)| {
            let clause = checked_ordinal(clause)?;
            match value {
                crate::item::HirFlowContractClause::Effects(operands) => {
                    for (operand, expression) in operands.operands().iter().copied().enumerate() {
                        roots.push(HirDeclarationContractRoot {
                            role: HirDeclarationContractRootRole::EffectOperand {
                                clause,
                                family: HirFlowContractRootFamily::Effects,
                                operand: checked_ordinal(operand)?,
                            },
                            child: expression,
                        });
                    }
                }
                crate::item::HirFlowContractClause::NoEffect { expression } => {
                    roots.push(HirDeclarationContractRoot {
                        role: HirDeclarationContractRootRole::EffectOperand {
                            clause,
                            family: HirFlowContractRootFamily::NoEffect,
                            operand: 0,
                        },
                        child: *expression,
                    });
                }
                crate::item::HirFlowContractClause::Requires(condition) => {
                    roots.push(HirDeclarationContractRoot {
                        role: HirDeclarationContractRootRole::Requires { ordinal: clause },
                        child: condition.expression(),
                    });
                }
                crate::item::HirFlowContractClause::Ensures(condition) => {
                    roots.push(HirDeclarationContractRoot {
                        role: HirDeclarationContractRootRole::Ensures { ordinal: clause },
                        child: condition.expression(),
                    });
                }
                crate::item::HirFlowContractClause::Invariant(condition) => {
                    roots.push(HirDeclarationContractRoot {
                        role: HirDeclarationContractRootRole::Invariant { ordinal: clause },
                        child: condition.expression(),
                    });
                }
                crate::item::HirFlowContractClause::Assume { expression } => {
                    roots.push(HirDeclarationContractRoot {
                        role: HirDeclarationContractRootRole::Assume,
                        child: *expression,
                    });
                }
                crate::item::HirFlowContractClause::Reads(operands) => {
                    for (ordinal, expression) in operands.operands().iter().copied().enumerate() {
                        roots.push(HirDeclarationContractRoot {
                            role: HirDeclarationContractRootRole::Reads {
                                ordinal: checked_ordinal(ordinal)?,
                            },
                            child: expression,
                        });
                    }
                }
                crate::item::HirFlowContractClause::Modifies(operands) => {
                    for (ordinal, expression) in operands.operands().iter().copied().enumerate() {
                        roots.push(HirDeclarationContractRoot {
                            role: HirDeclarationContractRootRole::Modifies {
                                ordinal: checked_ordinal(ordinal)?,
                            },
                            child: expression,
                        });
                    }
                }
                crate::item::HirFlowContractClause::Decreases { expression } => {
                    roots.push(HirDeclarationContractRoot {
                        role: HirDeclarationContractRootRole::Decreases,
                        child: *expression,
                    });
                }
            }
            Ok(roots)
        })
}

#[allow(
    clippy::match_same_arms,
    reason = "the exhaustive item family matrix intentionally retains the no-root rows"
)]
fn item_evaluation_roots(
    module: &HirModule,
    item_id: ItemId,
) -> Result<Vec<HirItemEvaluationRoot>, HirSemanticPathError> {
    let item = module
        .resolve_item(item_id)
        .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
    let mut roots = Vec::new();
    append_item_prefix_roots(&mut roots, item.prefix(), HirItemAttributeOwner::Item)?;
    match item.kind() {
        HirItemKind::Flow(_)
        | HirItemKind::Function(_)
        | HirItemKind::Predicate(_)
        | HirItemKind::Proof(_)
        | HirItemKind::View(_)
        | HirItemKind::Trait(_)
        | HirItemKind::Impl(_)
        | HirItemKind::ExternCapability(_) => {}
        HirItemKind::Activity(activity) => {
            append_item_expression_slice(&mut roots, activity.requires(), |ordinal| {
                HirDeclarationItemRootRole::ActivityRequires { ordinal }
            })?;
            append_item_expression_slice(&mut roots, activity.ensures(), |ordinal| {
                HirDeclarationItemRootRole::ActivityEnsures { ordinal }
            })?;
        }
        HirItemKind::Entry(entry) => {
            for (ordinal, member) in entry.members().iter().enumerate() {
                if let crate::item::HirEntryMember::Option(option) = member
                    && let Some(expression) = option.value().expression()
                {
                    roots.push(item_expression_root(
                        HirDeclarationItemRootRole::EntryOption {
                            member: checked_ordinal(ordinal)?,
                        },
                        expression,
                    ));
                }
            }
        }
        HirItemKind::Style(style) => {
            append_style_roots(&mut roots, style)?;
        }
        HirItemKind::Test(test) => {
            roots.push(item_body_root(
                HirDeclarationItemRootRole::TestBody,
                statement_body_projection(test.scope(), test.body())?,
            ));
        }
        HirItemKind::Bench(bench) => {
            roots.push(item_body_root(
                HirDeclarationItemRootRole::BenchBody,
                statement_body_projection(bench.scope(), bench.body())?,
            ));
        }
        HirItemKind::Resource(resource) => {
            for (field, value) in resource.fields().iter().enumerate() {
                roots.push(item_expression_root(
                    HirDeclarationItemRootRole::ResourceField {
                        field: checked_ordinal(field)?,
                    },
                    value.value(),
                ));
            }
        }
        HirItemKind::Character(_)
        | HirItemKind::Action(_)
        | HirItemKind::Signal(_)
        | HirItemKind::Metric(_)
        | HirItemKind::Layer(_)
        | HirItemKind::Module(_)
        | HirItemKind::Use(_)
        | HirItemKind::Enum(_)
        | HirItemKind::Struct(_)
        | HirItemKind::TypeAlias(_)
        | HirItemKind::Error(_) => {}
    }
    for (member_ordinal, member) in item.members().iter().enumerate() {
        let member = module
            .declaration_members()
            .resolve(*member)
            .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
        append_member_roots(&mut roots, checked_ordinal(member_ordinal)?, member.kind())?;
    }
    Ok(roots)
}

fn inline_member_roots(
    module: &HirModule,
    item_id: ItemId,
    member: u16,
    owner: Option<HirCallableSourceOwner>,
) -> Result<Vec<HirItemEvaluationRoot>, HirSemanticPathError> {
    let item = module
        .resolve_item(item_id)
        .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
    let member_ordinal = u32::from(member);
    let attribute_owner = if matches!(
        owner,
        Some(HirCallableSourceOwner::ExternCapabilityFunction { .. })
    ) {
        HirItemAttributeOwner::CapabilityMember { member }
    } else {
        HirItemAttributeOwner::InlineMember { member }
    };
    let mut roots = Vec::new();
    match item.kind() {
        HirItemKind::Trait(value) => {
            let Some(member) = value.members().get(usize::from(member)) else {
                return Err(HirSemanticPathError::UnresolvedOwner);
            };
            match member {
                crate::item::HirTraitMember::AssociatedType(value) => {
                    append_item_prefix_roots(&mut roots, value.prefix(), attribute_owner)?;
                }
                crate::item::HirTraitMember::Function(_) => {
                    append_trait_member_roots(&mut roots, member_ordinal, member)?;
                }
                crate::item::HirTraitMember::Error => {}
            }
        }
        HirItemKind::Impl(value) => {
            let Some(member) = value.members().get(usize::from(member)) else {
                return Err(HirSemanticPathError::UnresolvedOwner);
            };
            match member {
                HirImplMember::AssociatedType(value) => {
                    append_item_prefix_roots(&mut roots, value.prefix(), attribute_owner)?;
                }
                HirImplMember::Function(_) => {
                    append_impl_member_roots(&mut roots, member_ordinal, member)?;
                }
                HirImplMember::Error => {}
            }
        }
        HirItemKind::ExternCapability(value) => {
            let Some(member) = value.members().get(usize::from(member)) else {
                return Err(HirSemanticPathError::UnresolvedOwner);
            };
            match member {
                crate::item::HirCapabilityMember::AssociatedType(value) => {
                    append_item_prefix_roots(&mut roots, value.prefix(), attribute_owner)?;
                }
                crate::item::HirCapabilityMember::Function(value) => {
                    append_item_prefix_roots(&mut roots, value.prefix(), attribute_owner)?;
                }
                crate::item::HirCapabilityMember::Error => {}
            }
        }
        _ => return Err(HirSemanticPathError::InvalidOwnedPath),
    }
    Ok(roots)
}

fn append_item_prefix_roots(
    roots: &mut Vec<HirItemEvaluationRoot>,
    prefix: &HirItemPrefix,
    owner: HirItemAttributeOwner,
) -> Result<(), HirSemanticPathError> {
    for (attribute, value) in prefix.attributes().iter().enumerate() {
        for (argument_ordinal, argument) in value.arguments().iter().enumerate() {
            let role = if matches!(argument.value_state(), HirCallValue::Missing { .. }) {
                HirDeclarationItemRootRole::Recovery {
                    owner: HirItemRecoveryRootOwner::Attribute {
                        attribute: checked_ordinal(attribute)?,
                        argument: checked_ordinal(argument_ordinal)?,
                    },
                }
            } else {
                HirDeclarationItemRootRole::AttributeArgument {
                    owner,
                    attribute: checked_ordinal(attribute)?,
                    argument: checked_ordinal(argument_ordinal)?,
                }
            };
            roots.push(item_expression_root(role, argument.value()));
        }
    }
    Ok(())
}

fn append_trait_member_roots(
    roots: &mut Vec<HirItemEvaluationRoot>,
    member: u32,
    value: &crate::item::HirTraitMember,
) -> Result<(), HirSemanticPathError> {
    match value {
        crate::item::HirTraitMember::AssociatedType(value) => {
            append_item_prefix_roots(
                roots,
                value.prefix(),
                HirItemAttributeOwner::InlineMember {
                    member: u16::try_from(member)
                        .map_err(|_| HirSemanticPathError::OrdinalOverflow)?,
                },
            )?;
        }
        crate::item::HirTraitMember::Function(value) => {
            append_item_prefix_roots(
                roots,
                value.prefix(),
                HirItemAttributeOwner::InlineMember {
                    member: u16::try_from(member)
                        .map_err(|_| HirSemanticPathError::OrdinalOverflow)?,
                },
            )?;
        }
        crate::item::HirTraitMember::Error => {}
    }
    let _ = member;
    Ok(())
}

fn append_impl_member_roots(
    roots: &mut Vec<HirItemEvaluationRoot>,
    member: u32,
    value: &HirImplMember,
) -> Result<(), HirSemanticPathError> {
    match value {
        HirImplMember::AssociatedType(value) => append_item_prefix_roots(
            roots,
            value.prefix(),
            HirItemAttributeOwner::InlineMember {
                member: u16::try_from(member).map_err(|_| HirSemanticPathError::OrdinalOverflow)?,
            },
        )?,
        HirImplMember::Function(value) => {
            append_item_prefix_roots(
                roots,
                value.prefix(),
                HirItemAttributeOwner::InlineMember {
                    member: u16::try_from(member)
                        .map_err(|_| HirSemanticPathError::OrdinalOverflow)?,
                },
            )?;
        }
        HirImplMember::Error => {}
    }
    let _ = member;
    Ok(())
}

fn append_item_expression_slice(
    roots: &mut Vec<HirItemEvaluationRoot>,
    expressions: &[ExprId],
    mut role: impl FnMut(u32) -> HirDeclarationItemRootRole,
) -> Result<(), HirSemanticPathError> {
    for (ordinal, expression) in expressions.iter().copied().enumerate() {
        roots.push(item_expression_root(
            role(checked_ordinal(ordinal)?),
            expression,
        ));
    }
    Ok(())
}

fn append_style_roots(
    roots: &mut Vec<HirItemEvaluationRoot>,
    style: &crate::item::HirStyleItem,
) -> Result<(), HirSemanticPathError> {
    for (ordinal, token) in style.tokens().iter().enumerate() {
        roots.push(item_expression_root(
            HirDeclarationItemRootRole::Style {
                path: HirStyleRootPath::new(vec![HirStyleRootPathSegment::Token {
                    ordinal: checked_ordinal(ordinal)?,
                }]),
            },
            token.value(),
        ));
    }
    append_style_body_roots(roots, style.body(), &[])
}

fn append_style_body_roots(
    roots: &mut Vec<HirItemEvaluationRoot>,
    body: &[crate::item::HirStyleBodyItem],
    prefix: &[HirStyleRootPathSegment],
) -> Result<(), HirSemanticPathError> {
    for (ordinal, item) in body.iter().enumerate() {
        let ordinal = checked_ordinal(ordinal)?;
        match item {
            crate::item::HirStyleBodyItem::Rule(rule) => {
                let mut rule_path = prefix.to_vec();
                rule_path.push(HirStyleRootPathSegment::Rule { ordinal });
                for (declaration, value) in rule.declarations().iter().enumerate() {
                    let mut path = rule_path.clone();
                    path.push(HirStyleRootPathSegment::Declaration {
                        ordinal: checked_ordinal(declaration)?,
                    });
                    roots.push(item_expression_root(
                        HirDeclarationItemRootRole::Style {
                            path: HirStyleRootPath::new(path),
                        },
                        value.value(),
                    ));
                }
            }
            crate::item::HirStyleBodyItem::Environment(environment) => {
                let mut environment_path = prefix.to_vec();
                environment_path.push(HirStyleRootPathSegment::Environment { ordinal });
                for (clause, value) in environment.clauses().iter().enumerate() {
                    let mut path = environment_path.clone();
                    path.push(HirStyleRootPathSegment::Clause {
                        ordinal: checked_ordinal(clause)?,
                    });
                    roots.push(item_expression_root(
                        HirDeclarationItemRootRole::Style {
                            path: HirStyleRootPath::new(path),
                        },
                        value.value(),
                    ));
                }
                append_style_body_roots(roots, environment.body(), &environment_path)?;
            }
            crate::item::HirStyleBodyItem::Recovered(_) => {}
        }
    }
    Ok(())
}

fn append_member_roots(
    roots: &mut Vec<HirItemEvaluationRoot>,
    member: u32,
    kind: &HirDeclarationMemberKind,
) -> Result<(), HirSemanticPathError> {
    match kind {
        HirDeclarationMemberKind::CharacterDisplayName(value) => {
            if let Some(expression) = value.initializer() {
                let role =
                    if value.assignment() == crate::item::HirCharacterAssignmentState::Missing {
                        HirDeclarationItemRootRole::Recovery {
                            owner: HirItemRecoveryRootOwner::DeclarationMember { member },
                        }
                    } else {
                        HirDeclarationItemRootRole::CharacterDisplayName { member }
                    };
                roots.push(item_expression_root(role, expression));
            }
        }
        HirDeclarationMemberKind::MetricUnit(value) => {
            if let crate::item::HirMetricUnitValue::NonString(expression) = value.value() {
                roots.push(item_expression_root(
                    HirDeclarationItemRootRole::MetricUnit { member },
                    *expression,
                ));
            }
        }
        HirDeclarationMemberKind::MetricBuckets(value) => match value.value() {
            crate::item::HirMetricBucketsValue::Sequence(values) => {
                for (ordinal, expression) in values.iter().copied().enumerate() {
                    roots.push(item_expression_root(
                        HirDeclarationItemRootRole::MetricBuckets {
                            member,
                            ordinal: checked_ordinal(ordinal)?,
                        },
                        expression,
                    ));
                }
            }
            crate::item::HirMetricBucketsValue::NonSequence(expression) => {
                roots.push(item_expression_root(
                    HirDeclarationItemRootRole::MetricBuckets { member, ordinal: 0 },
                    *expression,
                ));
            }
            crate::item::HirMetricBucketsValue::Missing => {}
        },
        HirDeclarationMemberKind::LayerExpression(value) => {
            let field = match value {
                crate::item::HirLayerExpressionMember::Z(_) => HirLayerExpressionRootField::Z,
                crate::item::HirLayerExpressionMember::Visible(_) => {
                    HirLayerExpressionRootField::Visible
                }
                crate::item::HirLayerExpressionMember::Transform(_) => {
                    HirLayerExpressionRootField::Transform
                }
            };
            if let crate::item::HirLayerMemberValue::Present(expression)
            | crate::item::HirLayerMemberValue::Recovered(Some(expression)) =
                value.payload().value()
            {
                let role = if matches!(
                    value.payload().value(),
                    crate::item::HirLayerMemberValue::Recovered(_)
                ) {
                    HirDeclarationItemRootRole::Recovery {
                        owner: HirItemRecoveryRootOwner::DeclarationMember { member },
                    }
                } else {
                    HirDeclarationItemRootRole::LayerField { member, field }
                };
                roots.push(item_expression_root(role, *expression));
            }
        }
        HirDeclarationMemberKind::ViewExport(_)
        | HirDeclarationMemberKind::ActivityInput(_)
        | HirDeclarationMemberKind::ActivityOutput(_)
        | HirDeclarationMemberKind::MetricLabel(_)
        | HirDeclarationMemberKind::CharacterRecovery(_)
        | HirDeclarationMemberKind::LayerReference(_)
        | HirDeclarationMemberKind::LayerPolicy(_) => {}
    }
    let _ = member;
    Ok(())
}

fn item_expression_root(role: HirDeclarationItemRootRole, child: ExprId) -> HirItemEvaluationRoot {
    HirItemEvaluationRoot {
        role,
        projection: HirBodyProjection::expression(child),
    }
}

fn item_body_root(
    role: HirDeclarationItemRootRole,
    child: HirBodyProjection,
) -> HirItemEvaluationRoot {
    HirItemEvaluationRoot {
        role,
        projection: child,
    }
}

fn statement_body_projection(
    scope: crate::identity::ScopeId,
    statements: &[StmtId],
) -> Result<HirBodyProjection, HirSemanticPathError> {
    let body = HirContextualStmtBody::try_ordinary(scope, statements.to_vec().into_boxed_slice())
        .map_err(|_| HirSemanticPathError::InvalidOwnedPath)?;
    body.try_body_projection().map_err(Into::into)
}

fn classify_local_origin(
    module: &HirModule,
    local: LocalId,
    pattern: PatternId,
    input: ExprId,
    local_count: usize,
    direct_statement: bool,
) -> Result<HirLocalValueOrigin, HirSemanticPathError> {
    let local_record = module
        .resolve_local(local)
        .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
    if local_record.kind() != HirLocalKind::LetBinding {
        return Ok(HirLocalValueOrigin::Independent);
    }
    if local_record.is_mutable_binding() || local_record.is_poisoned() {
        return Ok(HirLocalValueOrigin::Composite);
    }
    if !direct_statement || local_count != 1 {
        return Ok(HirLocalValueOrigin::Composite);
    }
    let pattern_value = module
        .resolve_pattern(pattern)
        .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
    let pattern_locals = pattern_local_ids(module, pattern)?;
    let direct = match pattern_value.kind() {
        HirPatternKind::Binding(HirPatternBinding::Bound { local: bound, .. })
        | HirPatternKind::TypedBinding {
            binding: HirPatternBinding::Bound { local: bound, .. },
            ..
        } => *bound == local,
        HirPatternKind::Binding(HirPatternBinding::Recovered { .. })
        | HirPatternKind::TypedBinding {
            binding: HirPatternBinding::Recovered { .. },
            ..
        }
        | HirPatternKind::MutableBinding(_)
        | HirPatternKind::Literal(_)
        | HirPatternKind::EntityReference(_)
        | HirPatternKind::Variant(_)
        | HirPatternKind::Discard
        | HirPatternKind::Tuple { .. }
        | HirPatternKind::Record { .. }
        | HirPatternKind::BracketSequence { .. }
        | HirPatternKind::WholeBinding { .. }
        | HirPatternKind::Or { .. }
        | HirPatternKind::Error(_) => false,
    };
    if direct && pattern_locals.as_slice() == [local] {
        Ok(HirLocalValueOrigin::DirectInitializer(input))
    } else {
        Ok(HirLocalValueOrigin::Composite)
    }
}

fn pattern_local_ids(
    module: &HirModule,
    root: PatternId,
) -> Result<Vec<LocalId>, HirSemanticPathError> {
    let mut locals = Vec::new();
    let mut active = BTreeSet::new();
    let mut visited = BTreeSet::new();
    collect_pattern_local_ids(module, root, &mut active, &mut visited, &mut locals)?;
    Ok(locals)
}

fn collect_pattern_local_ids(
    module: &HirModule,
    pattern: PatternId,
    active: &mut BTreeSet<PatternId>,
    visited: &mut BTreeSet<PatternId>,
    locals: &mut Vec<LocalId>,
) -> Result<(), HirSemanticPathError> {
    if !active.insert(pattern) {
        return Err(HirSemanticPathError::CyclicPath {
            owner: pattern.into(),
        });
    }
    if !visited.insert(pattern) {
        return Err(HirSemanticPathError::DuplicatePath {
            owner: pattern.into(),
        });
    }
    let value = module
        .resolve_pattern(pattern)
        .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
    let edges = value
        .kind()
        .try_child_edges()
        .map_err(|_| HirSemanticPathError::OrdinalOverflow)?;
    for edge in edges {
        match edge.child() {
            HirPatternChild::Local(local) => {
                if locals.contains(&local) {
                    return Err(HirSemanticPathError::DuplicatePath {
                        owner: local.into(),
                    });
                }
                locals.push(local);
            }
            HirPatternChild::Pattern(pattern) => {
                collect_pattern_local_ids(module, pattern, active, visited, locals)?;
            }
            HirPatternChild::Type(_) => {}
        }
    }
    active.remove(&pattern);
    Ok(())
}

fn expression_binding_role(role: &HirExpressionOwnedBodyRole) -> Option<HirExpressionBindingRole> {
    match role {
        HirExpressionOwnedBodyRole::ClosureParameterPattern { parameter } => {
            Some(HirExpressionBindingRole::ClosureParameter {
                parameter: *parameter,
            })
        }
        HirExpressionOwnedBodyRole::IfLetPattern => Some(HirExpressionBindingRole::IfLet),
        HirExpressionOwnedBodyRole::AwaitBranchPattern { branch } => {
            Some(HirExpressionBindingRole::AwaitBranch { branch: *branch })
        }
        HirExpressionOwnedBodyRole::ChoiceForPattern { path } => {
            Some(HirExpressionBindingRole::ChoiceFor { path: path.clone() })
        }
        HirExpressionOwnedBodyRole::ChoiceMatchArmPattern { path, arm } => {
            Some(HirExpressionBindingRole::ChoiceMatchArm {
                path: path.clone(),
                arm: *arm,
            })
        }
        HirExpressionOwnedBodyRole::ChoiceOptionForPattern { path } => {
            Some(HirExpressionBindingRole::ChoiceOptionFor { path: path.clone() })
        }
        HirExpressionOwnedBodyRole::ChoicePlanOnSelectPattern { path } => {
            Some(HirExpressionBindingRole::ChoicePlanOnSelect { path: path.clone() })
        }
        HirExpressionOwnedBodyRole::ChoicePlanCancelTrigger { path } => {
            Some(HirExpressionBindingRole::ChoicePlanCancelTrigger { path: path.clone() })
        }
        HirExpressionOwnedBodyRole::DialogueLinePlanLet { path } => {
            Some(HirExpressionBindingRole::DialogueLinePlanLet { path: path.clone() })
        }
        HirExpressionOwnedBodyRole::AwaitBranchBody { .. }
        | HirExpressionOwnedBodyRole::ChoiceLetStatement { .. }
        | HirExpressionOwnedBodyRole::ChoiceOptionSelectBody { .. }
        | HirExpressionOwnedBodyRole::ChoiceOptionLetStatement { .. }
        | HirExpressionOwnedBodyRole::ChoicePlanTimeoutBody { .. }
        | HirExpressionOwnedBodyRole::ChoicePlanCancelBody { .. }
        | HirExpressionOwnedBodyRole::ChoicePlanOnSelectBody { .. }
        | HirExpressionOwnedBodyRole::DialogueLinePlanStatement { .. } => None,
    }
}

fn trigger_pattern_id(trigger: &crate::stmt::HirTriggerPattern) -> Option<PatternId> {
    match trigger {
        crate::stmt::HirTriggerPattern::Input(pattern)
        | crate::stmt::HirTriggerPattern::Event(pattern)
        | crate::stmt::HirTriggerPattern::Mark(pattern)
        | crate::stmt::HirTriggerPattern::Select(pattern)
        | crate::stmt::HirTriggerPattern::Task(pattern)
        | crate::stmt::HirTriggerPattern::Scope(pattern) => Some(*pattern),
        crate::stmt::HirTriggerPattern::Signal { value, .. } => *value,
        crate::stmt::HirTriggerPattern::Timeout(_) | crate::stmt::HirTriggerPattern::Expr(_) => {
            None
        }
    }
}

/// Single snapshot-bound builder for project, item, and declaration
/// evaluation topology projections.
///
/// The builder owns the maps, binding rows, and cycle/duplicate guards for
/// both declaration projections and the project-level seal.
struct HirProjectEvaluationTopologyBuilder<'module> {
    module: &'module HirModule,
    binding_item: Option<ItemId>,
    binding_owner: Option<HirCallableSourceOwner>,
    expressions: BTreeMap<ExprId, HirSemanticOwnerPath>,
    statements: BTreeMap<StmtId, HirSemanticOwnerPath>,
    patterns: BTreeMap<PatternId, HirSemanticOwnerPath>,
    locals: BTreeMap<LocalId, HirSemanticOwnerPath>,
    body_rows: Vec<HirSemanticBodyRow>,
    control_transfers: BTreeMap<StmtId, HirControlTransferRow>,
    line_plan_applications: Vec<ExprId>,
    local_origins: BTreeMap<LocalId, HirLocalBindingOrigin>,
    expression_uses: BTreeMap<ExprId, HirExpressionUseRow>,
    next_source_ordinal: u32,
    capture_rows: Vec<HirCaptureEvaluationRow>,
    captures_by_capture: BTreeMap<CaptureId, u32>,
    captures_by_closure: BTreeMap<ExprId, Range<u32>>,
    selection_roots: Vec<ExprId>,
    selection_edges: BTreeMap<ExprId, Vec<HirExpressionEvaluationEdge>>,
    active_expressions: BTreeSet<ExprId>,
    active_statements: BTreeSet<StmtId>,
    active_patterns: BTreeSet<PatternId>,
}

type HirFinishedModuleTopology = (
    HirLocalBindingOriginIndex,
    Box<[ExprId]>,
    BTreeMap<ExprId, Box<[HirExpressionEvaluationEdge]>>,
    HirCaptureEvaluationIndex,
    HirExpressionUseIndex,
);

struct HirModuleOwnerSets {
    expressions: BTreeSet<ExprId>,
    statements: BTreeSet<StmtId>,
    patterns: BTreeSet<PatternId>,
    locals: BTreeSet<LocalId>,
}

#[derive(Clone, Debug, Default)]
struct HirPathCheckpoint {
    expressions: BTreeSet<ExprId>,
    statements: BTreeSet<StmtId>,
    patterns: BTreeSet<PatternId>,
    locals: BTreeSet<LocalId>,
    body_rows: usize,
    control_transfers: BTreeSet<StmtId>,
}

#[derive(Clone, Copy)]
struct HirExpressionTraversalProjection<'hir> {
    kind: &'hir HirExprKind,
    body: Option<&'hir HirBodyProjection>,
}

fn path_map_delta<T: Copy + Ord>(
    values: &BTreeMap<T, HirSemanticOwnerPath>,
    checkpoint: &BTreeSet<T>,
) -> BTreeMap<T, HirSemanticOwnerPath> {
    values
        .iter()
        .filter(|(owner, _)| !checkpoint.contains(owner))
        .map(|(owner, path)| (*owner, path.clone()))
        .collect()
}

impl<'module> HirProjectEvaluationTopologyBuilder<'module> {
    /// Seals one complete project topology through the same builder authority
    /// used by declaration projections.
    fn build_project(
        generation: &AcceptedHirProjectSymbolGeneration<'module, '_>,
    ) -> Result<HirProjectEvaluationTopology, HirSemanticPathError> {
        generation.project().build_project_topology(generation)
    }

    fn new_for_module(module: &'module HirModule) -> Self {
        Self {
            module,
            binding_item: None,
            binding_owner: None,
            expressions: BTreeMap::new(),
            statements: BTreeMap::new(),
            patterns: BTreeMap::new(),
            locals: BTreeMap::new(),
            body_rows: Vec::new(),
            control_transfers: BTreeMap::new(),
            line_plan_applications: Vec::new(),
            local_origins: BTreeMap::new(),
            expression_uses: BTreeMap::new(),
            next_source_ordinal: 0,
            capture_rows: Vec::new(),
            captures_by_capture: BTreeMap::new(),
            captures_by_closure: BTreeMap::new(),
            selection_roots: Vec::new(),
            selection_edges: BTreeMap::new(),
            active_expressions: BTreeSet::new(),
            active_statements: BTreeSet::new(),
            active_patterns: BTreeSet::new(),
        }
    }

    fn path_checkpoint(&self) -> HirPathCheckpoint {
        HirPathCheckpoint {
            expressions: self.expressions.keys().copied().collect(),
            statements: self.statements.keys().copied().collect(),
            patterns: self.patterns.keys().copied().collect(),
            locals: self.locals.keys().copied().collect(),
            body_rows: self.body_rows.len(),
            control_transfers: self.control_transfers.keys().copied().collect(),
        }
    }

    fn path_index_since(
        &self,
        root: HirSemanticPathRoot,
        checkpoint: &HirPathCheckpoint,
    ) -> Result<HirSemanticPathIndex, HirSemanticPathError> {
        let paths = HirSemanticPathIndex {
            root,
            snapshot: self.module.snapshot_id(),
            expressions: path_map_delta(&self.expressions, &checkpoint.expressions),
            statements: path_map_delta(&self.statements, &checkpoint.statements),
            patterns: path_map_delta(&self.patterns, &checkpoint.patterns),
            locals: path_map_delta(&self.locals, &checkpoint.locals),
            body_rows: self.body_rows[checkpoint.body_rows..]
                .to_vec()
                .into_boxed_slice(),
            control_transfers: self
                .control_transfers
                .iter()
                .filter(|(statement, _)| !checkpoint.control_transfers.contains(statement))
                .map(|(_, row)| row.clone())
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        };
        paths.validate_root_paths()?;
        Ok(paths)
    }

    fn record_body_row(
        &mut self,
        owner: HirSemanticBodyOwner,
        path: &[HirSemanticPathStep],
        hops: &[HirExpressionSemanticHop],
        projection: HirBodyProjection,
    ) -> Result<(), HirSemanticPathError> {
        let row = HirSemanticBodyRow::try_new(
            owner.clone(),
            HirSemanticOwnerPath::new(path.into(), hops.into()),
            projection,
        )
        .map_err(|_| HirSemanticPathError::InvalidBodyRow { owner })?;
        self.body_rows.push(row);
        Ok(())
    }

    fn record_expression_body_rows(
        &mut self,
        owner: ExprId,
        path: &[HirSemanticPathStep],
        hops: &[HirExpressionSemanticHop],
        kind: &HirExprKind,
    ) -> Result<Option<HirBodyProjection>, HirSemanticPathError> {
        let direct = kind.try_body_projection()?;
        if let Some(projection) = &direct {
            self.record_body_row(
                HirSemanticBodyOwner::direct_expression(owner),
                path,
                hops,
                projection.clone(),
            )?;
        }
        for body in kind.expression_owned_body_projections()? {
            let role = body.role().clone();
            let body_path = pushed(path, HirSemanticPathStep::ExpressionOwned(role.clone()));
            self.record_body_row(
                HirSemanticBodyOwner::try_expression_owned(owner, role)
                    .map_err(|_| HirSemanticPathError::InvalidOwnedPath)?,
                &body_path,
                hops,
                body.projection().clone(),
            )?;
        }
        Ok(direct)
    }

    fn record_statement_body_rows(
        &mut self,
        statement: StmtId,
        parent: &[HirSemanticPathStep],
        hops: &[HirExpressionSemanticHop],
        kind: &HirStmtKind,
    ) -> Result<Vec<crate::stmt::HirStatementBodyProjection>, HirSemanticPathError> {
        let bodies = kind.body_projections()?;
        for body in &bodies {
            let role = *body.role();
            let body_path = pushed(parent, HirSemanticPathStep::StatementBody(role));
            self.record_body_row(
                HirSemanticBodyOwner::statement_body(statement, role),
                &body_path,
                hops,
                body.projection().clone(),
            )?;
        }
        Ok(bodies)
    }

    fn walk_declaration_body(
        &mut self,
        root: &HirDeclarationBodyRoot,
    ) -> Result<(), HirSemanticPathError> {
        let path = [HirSemanticPathStep::DeclarationBody(root.role)];
        self.record_body_row(
            HirSemanticBodyOwner::declaration(root.role),
            &path,
            &[],
            root.projection.clone(),
        )?;
        self.walk_root_body_projection(&root.projection, &path)
    }

    fn walk_item_root(&mut self, root: &HirItemEvaluationRoot) -> Result<(), HirSemanticPathError> {
        let path = [HirSemanticPathStep::DeclarationItem(root.role.clone())];
        self.record_body_row(
            HirSemanticBodyOwner::item(root.role.clone()),
            &path,
            &[],
            root.projection.clone(),
        )?;
        self.walk_root_body_projection(&root.projection, &path)
    }

    fn walk_declaration_topology(
        &mut self,
        declaration: CallableDeclarationKey,
        item: ItemId,
        owner: HirCallableSourceOwner,
        parameter_roots: &[HirDeclarationParameterRoot],
        contract_roots: &[HirDeclarationContractRoot],
        roots: &[HirDeclarationBodyRoot],
    ) -> Result<HirDeclarationBodyTopology, HirSemanticPathError> {
        let expression_keys = self.expressions.keys().copied().collect::<BTreeSet<_>>();
        let statement_keys = self.statements.keys().copied().collect::<BTreeSet<_>>();
        let pattern_keys = self.patterns.keys().copied().collect::<BTreeSet<_>>();
        let local_keys = self.locals.keys().copied().collect::<BTreeSet<_>>();
        let control_transfer_keys = self
            .control_transfers
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        let body_rows_start = self.body_rows.len();
        self.binding_item = Some(item);
        self.binding_owner = Some(owner);
        self.record_declaration_result_local()?;
        for root in parameter_roots {
            self.walk_parameter(root)?;
        }
        for root in contract_roots {
            self.walk_contract(root)?;
        }
        for root in roots {
            self.walk_declaration_body(root)?;
        }
        let paths = HirSemanticPathIndex {
            root: HirSemanticPathRoot::Declaration(declaration.clone()),
            snapshot: self.module.snapshot_id(),
            expressions: self
                .expressions
                .iter()
                .filter(|(id, _)| !expression_keys.contains(id))
                .map(|(id, path)| (*id, path.clone()))
                .collect(),
            statements: self
                .statements
                .iter()
                .filter(|(id, _)| !statement_keys.contains(id))
                .map(|(id, path)| (*id, path.clone()))
                .collect(),
            patterns: self
                .patterns
                .iter()
                .filter(|(id, _)| !pattern_keys.contains(id))
                .map(|(id, path)| (*id, path.clone()))
                .collect(),
            locals: self
                .locals
                .iter()
                .filter(|(id, _)| !local_keys.contains(id))
                .map(|(id, path)| (*id, path.clone()))
                .collect(),
            body_rows: self.body_rows[body_rows_start..]
                .to_vec()
                .into_boxed_slice(),
            control_transfers: self
                .control_transfers
                .iter()
                .filter(|(statement, _)| !control_transfer_keys.contains(statement))
                .map(|(_, row)| row.clone())
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        };
        paths.validate_root_paths()?;
        self.binding_item = None;
        self.binding_owner = None;
        Ok(HirDeclarationBodyTopology {
            declaration,
            source_item: item,
            source_owner: owner,
            parameter_roots: parameter_roots.to_vec().into_boxed_slice(),
            contract_roots: contract_roots.to_vec().into_boxed_slice(),
            roots: roots.to_vec().into_boxed_slice(),
            paths,
        })
    }

    fn record_item_member_origins(&mut self, item: ItemId) -> Result<(), HirSemanticPathError> {
        let value = self
            .module
            .resolve_item(item)
            .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
        for (member_ordinal, member_id) in value.members().iter().copied().enumerate() {
            let member = self
                .module
                .declaration_members()
                .resolve(member_id)
                .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
            let (role, local) = match member.kind() {
                HirDeclarationMemberKind::ActivityInput(value) => {
                    (HirMemberBindingRole::ActivityInput, value.local())
                }
                HirDeclarationMemberKind::ActivityOutput(value) => {
                    (HirMemberBindingRole::ActivityOutput, value.local())
                }
                _ => (HirMemberBindingRole::MethodReceiver, None),
            };
            let Some(local) = local else { continue };
            let path = [HirSemanticPathStep::DeclarationMember {
                member: checked_ordinal(member_ordinal)?,
            }];
            insert_unique(&mut self.locals, local, &path, &[])?;
            self.insert_local_origin(
                local,
                HirBindingSite::Member {
                    item,
                    member: checked_ordinal(member_ordinal)?,
                    role,
                },
                None,
                None,
                HirLocalValueOrigin::Independent,
            )?;
        }
        Ok(())
    }

    fn finish_module(self) -> Result<HirFinishedModuleTopology, HirSemanticPathError> {
        let owner_sets = self.module_owner_sets();
        self.validate_module_owner_sets(&owner_sets)?;
        self.validate_capture_inventory()?;
        for row in self.expression_uses.values() {
            if let Some(parent) = row.parent_expression() {
                let Some(parent_row) = self.expression_uses.get(&parent) else {
                    return Err(HirSemanticPathError::InvalidOwnedPath);
                };
                if parent_row.source_ordinal() >= row.source_ordinal()
                    || row.source_ordinal() >= parent_row.subtree_end_ordinal()
                {
                    return Err(HirSemanticPathError::InvalidOwnedPath);
                }
            }
        }
        let mut expression_use_rows = self.expression_uses.into_values().collect::<Vec<_>>();
        expression_use_rows.sort_by_key(HirExpressionUseRow::source_ordinal);
        let expression_count = checked_ordinal(expression_use_rows.len())?;
        let mut expression_uses_by_id = BTreeMap::new();
        for (index, row) in expression_use_rows.iter().enumerate() {
            let index = checked_ordinal(index)?;
            if row.source_ordinal() != index
                || row.subtree_end_ordinal() <= index
                || row.subtree_end_ordinal() > expression_count
                || expression_uses_by_id
                    .insert(row.expression(), index)
                    .is_some()
            {
                return Err(HirSemanticPathError::InvalidOwnedPath);
            }
        }
        let selection_roots = self.selection_roots.into_boxed_slice();
        let selection_edges = self
            .selection_edges
            .into_iter()
            .map(|(owner, edges)| (owner, edges.into_boxed_slice()))
            .collect();
        Ok((
            HirLocalBindingOriginIndex {
                snapshot: self.module.snapshot_id(),
                origins: self.local_origins,
            },
            selection_roots,
            selection_edges,
            HirCaptureEvaluationIndex {
                snapshot: self.module.snapshot_id(),
                rows: self.capture_rows.into_boxed_slice(),
                by_capture: self.captures_by_capture,
                by_closure: self.captures_by_closure,
            },
            HirExpressionUseIndex {
                snapshot: self.module.snapshot_id(),
                rows: expression_use_rows.into_boxed_slice(),
                by_expression: expression_uses_by_id,
            },
        ))
    }

    fn module_owner_sets(&self) -> HirModuleOwnerSets {
        HirModuleOwnerSets {
            expressions: self.module.expressions().map(|(id, _)| id).collect(),
            statements: self.module.statements().map(|(id, _)| id).collect(),
            patterns: self.module.patterns().map(|(id, _)| id).collect(),
            locals: self.module.locals().map(|(id, _)| id).collect(),
        }
    }

    fn validate_module_owner_sets(
        &self,
        owner_sets: &HirModuleOwnerSets,
    ) -> Result<(), HirSemanticPathError> {
        if self.expressions.keys().copied().collect::<BTreeSet<_>>() != owner_sets.expressions
            || self.statements.keys().copied().collect::<BTreeSet<_>>() != owner_sets.statements
            || self.patterns.keys().copied().collect::<BTreeSet<_>>() != owner_sets.patterns
            || self.locals.keys().copied().collect::<BTreeSet<_>>() != owner_sets.locals
        {
            return Err(HirSemanticPathError::InvalidOwnedPath);
        }
        if self
            .module
            .locals()
            .any(|(local, _)| !self.local_origins.contains_key(&local))
            || self.local_origins.len() != self.module.locals().len()
        {
            return Err(HirSemanticPathError::InvalidOwnedPath);
        }
        if self
            .expression_uses
            .keys()
            .copied()
            .collect::<BTreeSet<_>>()
            != owner_sets.expressions
        {
            return Err(HirSemanticPathError::InvalidOwnedPath);
        }
        if self.selection_edges.iter().any(|(owner, edges)| {
            !self.expressions.contains_key(owner)
                || edges
                    .iter()
                    .any(|edge| !self.expressions.contains_key(&edge.child()))
        }) {
            return Err(HirSemanticPathError::InvalidOwnedPath);
        }
        if self
            .selection_edges
            .keys()
            .copied()
            .collect::<BTreeSet<_>>()
            != owner_sets.expressions
        {
            return Err(HirSemanticPathError::InvalidOwnedPath);
        }
        Ok(())
    }

    fn validate_capture_inventory(&self) -> Result<(), HirSemanticPathError> {
        if self.capture_rows.len() != self.module.captures().len()
            || self.captures_by_capture.len() != self.capture_rows.len()
        {
            return Err(HirSemanticPathError::InvalidOwnedPath);
        }
        let mut closure_ranges = Vec::new();
        for owner in self.expression_uses.keys().copied() {
            let expression = self
                .module
                .resolve_expr(owner)
                .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
            let HirExprKind::Closure(closure) = expression.kind() else {
                continue;
            };
            let Some(range) = self.captures_by_closure.get(&owner).cloned() else {
                return Err(HirSemanticPathError::InvalidOwnedPath);
            };
            let start =
                usize::try_from(range.start).map_err(|_| HirSemanticPathError::OrdinalOverflow)?;
            let end =
                usize::try_from(range.end).map_err(|_| HirSemanticPathError::OrdinalOverflow)?;
            if start > end
                || end > self.capture_rows.len()
                || end - start != closure.captures().len()
            {
                return Err(HirSemanticPathError::InvalidOwnedPath);
            }
            for (offset, capture) in closure.captures().iter().copied().enumerate() {
                let row = &self.capture_rows[start + offset];
                let value = self
                    .module
                    .resolve_capture(capture)
                    .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
                let index = checked_ordinal(start + offset)?;
                if row.capture != capture
                    || row.closure != owner
                    || row.local != value.local()
                    || row.access != value.access()
                    || self.captures_by_capture.get(&capture).copied() != Some(index)
                {
                    return Err(HirSemanticPathError::InvalidOwnedPath);
                }
            }
            closure_ranges.push((range.start, range.end));
        }
        if self.captures_by_closure.len() != closure_ranges.len() {
            return Err(HirSemanticPathError::InvalidOwnedPath);
        }
        closure_ranges.sort_unstable();
        let mut cursor = 0u32;
        for (start, end) in closure_ranges {
            if start != cursor || end < start {
                return Err(HirSemanticPathError::InvalidOwnedPath);
            }
            cursor = end;
        }
        if cursor != checked_ordinal(self.capture_rows.len())? {
            return Err(HirSemanticPathError::InvalidOwnedPath);
        }
        for (capture, index) in &self.captures_by_capture {
            let Some(row) = self.capture_rows.get(*index as usize) else {
                return Err(HirSemanticPathError::InvalidOwnedPath);
            };
            if row.capture != *capture {
                return Err(HirSemanticPathError::InvalidOwnedPath);
            }
        }
        Ok(())
    }

    fn record_declaration_result_local(&mut self) -> Result<(), HirSemanticPathError> {
        let (Some(item), Some(HirCallableSourceOwner::Item)) =
            (self.binding_item, self.binding_owner)
        else {
            return Ok(());
        };
        let value = self
            .module
            .resolve_item(item)
            .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
        let result = match value.kind() {
            HirItemKind::Flow(flow) => flow
                .result_local()
                .map(|value| (value.local(), HirBindingSite::FlowResult { item })),
            HirItemKind::Function(function) => {
                postcondition_result_local(self.module, function.ensures_scope())?.map(|local| {
                    (
                        local,
                        HirBindingSite::PostconditionResult {
                            item,
                            owner: HirCallableSourceOwner::Item,
                        },
                    )
                })
            }
            HirItemKind::Predicate(predicate) => {
                postcondition_result_local(self.module, predicate.ensures_scope())?.map(|local| {
                    (
                        local,
                        HirBindingSite::PostconditionResult {
                            item,
                            owner: HirCallableSourceOwner::Item,
                        },
                    )
                })
            }
            HirItemKind::Proof(proof) => {
                postcondition_result_local(self.module, proof.ensures_scope())?.map(|local| {
                    (
                        local,
                        HirBindingSite::PostconditionResult {
                            item,
                            owner: HirCallableSourceOwner::Item,
                        },
                    )
                })
            }
            _ => None,
        };
        if let Some((local, site)) = result {
            let local_value = self
                .module
                .resolve_local(local)
                .map_err(|_| HirSemanticPathError::InvalidResultOrigin)?;
            if local_value.kind() != crate::scope::HirLocalKind::PostconditionResult
                || local_value.scope().module() != self.module.module_id()
            {
                return Err(HirSemanticPathError::InvalidResultOrigin);
            }
            if self.locals.contains_key(&local) {
                return Err(HirSemanticPathError::InvalidResultPath);
            }
            let path = [HirSemanticPathStep::DeclarationResult];
            insert_unique(&mut self.locals, local, &path, &[])?;
            if !matches!(
                self.locals.get(&local).map(HirSemanticOwnerPath::steps),
                Some([HirSemanticPathStep::DeclarationResult])
            ) {
                return Err(HirSemanticPathError::InvalidResultPath);
            }
            self.insert_local_origin(local, site, None, None, HirLocalValueOrigin::Independent)?;
        }
        Ok(())
    }

    fn walk_contract(
        &mut self,
        root: &HirDeclarationContractRoot,
    ) -> Result<(), HirSemanticPathError> {
        let path = [HirSemanticPathStep::DeclarationContract(root.role)];
        self.record_selection_root(root.child);
        self.walk_expression(root.child, &path, &[], None, CaptureAccess::Read)
    }

    fn walk_body(
        &mut self,
        edge: HirBodyChildEdge,
        parent: &[HirSemanticPathStep],
        hops: &[HirExpressionSemanticHop],
        owning_parent: Option<ExprId>,
        access: CaptureAccess,
    ) -> Result<(), HirSemanticPathError> {
        let path = pushed(parent, HirSemanticPathStep::Body(edge.role()));
        match edge.child() {
            HirBodyChild::Expression(owner) => {
                self.walk_expression(owner, &path, hops, owning_parent, access)
            }
            HirBodyChild::Statement(owner) => {
                self.walk_statement(owner, &path, hops, owning_parent, access)
            }
        }
    }

    fn record_selection_root(&mut self, owner: ExprId) {
        if !self.selection_roots.contains(&owner) {
            self.selection_roots.push(owner);
        }
    }

    fn walk_body_root(
        &mut self,
        edge: HirBodyChildEdge,
        parent: &[HirSemanticPathStep],
    ) -> Result<(), HirSemanticPathError> {
        if let HirBodyChild::Expression(owner) = edge.child() {
            self.record_selection_root(owner);
        } else if let HirBodyChild::Statement(statement) = edge.child() {
            let module = self.module;
            let roots = &mut self.selection_roots;
            record_statement_selection_steps(module, statement, roots)?;
        }
        self.walk_body(edge, parent, &[], None, CaptureAccess::Read)
    }

    fn walk_root_body_projection(
        &mut self,
        projection: &HirBodyProjection,
        path: &[HirSemanticPathStep],
    ) -> Result<(), HirSemanticPathError> {
        if projection.kind() == HirBodyKind::Expression {
            let [edge] = projection.children() else {
                return Err(HirSemanticPathError::InvalidOwnedPath);
            };
            let HirBodyChild::Expression(owner) = edge.child() else {
                return Err(HirSemanticPathError::InvalidOwnedPath);
            };
            self.record_selection_root(owner);
            return self.walk_expression(owner, path, &[], None, CaptureAccess::Read);
        }
        for edge in projection.children().iter().copied() {
            self.walk_body_root(edge, path)?;
        }
        Ok(())
    }

    fn walk_parameter(
        &mut self,
        root: &HirDeclarationParameterRoot,
    ) -> Result<(), HirSemanticPathError> {
        let path = [match root.role {
            HirDeclarationParameterRootRole::Pattern { group, parameter } => {
                HirSemanticPathStep::ParameterPattern { group, parameter }
            }
            HirDeclarationParameterRootRole::Default { group, parameter } => {
                HirSemanticPathStep::ParameterDefault { group, parameter }
            }
        }];
        match root.child {
            HirDeclarationParameterRootChild::Pattern(owner) => {
                let (Some(item), Some(source_owner)) = (self.binding_item, self.binding_owner)
                else {
                    return self.walk_pattern(owner, &path, &[]);
                };
                let HirDeclarationParameterRootRole::Pattern { group, parameter } = root.role
                else {
                    return self.walk_pattern(owner, &path, &[]);
                };
                self.record_pattern_binding(
                    owner,
                    HirBindingSite::DeclarationParameter {
                        item,
                        owner: source_owner,
                        group,
                        parameter,
                    },
                    None,
                    None,
                )?;
                self.walk_pattern(owner, &path, &[])
            }
            HirDeclarationParameterRootChild::Expression(owner) => {
                self.record_selection_root(owner);
                self.walk_expression(owner, &path, &[], None, CaptureAccess::Read)
            }
        }
    }

    fn record_expression_start(
        &mut self,
        owner: ExprId,
        owning_parent: Option<ExprId>,
        access: CaptureAccess,
        kind: &HirExprKind,
    ) -> Result<(), HirSemanticPathError> {
        let source_ordinal = self.next_source_ordinal;
        self.next_source_ordinal = self
            .next_source_ordinal
            .checked_add(1)
            .ok_or(HirSemanticPathError::OrdinalOverflow)?;
        let callable_boundary = match kind {
            HirExprKind::Call(_) => Some(HirExpressionCallableBoundary::Call),
            HirExprKind::Closure(_) => Some(HirExpressionCallableBoundary::ExplicitClosure),
            _ => None,
        };
        let placeholder = match kind {
            HirExprKind::Placeholder(kind) => Some(*kind),
            _ => None,
        };
        if self
            .expression_uses
            .insert(
                owner,
                HirExpressionUseRow {
                    expression: owner,
                    source_ordinal,
                    subtree_end_ordinal: self.next_source_ordinal,
                    parent_expression: owning_parent,
                    capture_access: access,
                    callable_boundary,
                    placeholder,
                },
            )
            .is_some()
        {
            return Err(HirSemanticPathError::DuplicateExpressionUse { owner });
        }
        if let HirExprKind::Closure(closure) = kind {
            let start = checked_ordinal(self.capture_rows.len())?;
            if self.captures_by_closure.contains_key(&owner) {
                return Err(HirSemanticPathError::DuplicateClosureCaptureRange { owner });
            }
            for capture in closure.captures().iter().copied() {
                let value = self
                    .module
                    .resolve_capture(capture)
                    .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
                if value.closure() != owner {
                    return Err(HirSemanticPathError::InvalidOwnedPath);
                }
                let index = checked_ordinal(self.capture_rows.len())?;
                if self.captures_by_capture.insert(capture, index).is_some() {
                    return Err(HirSemanticPathError::DuplicateCapture { owner: capture });
                }
                self.capture_rows.push(HirCaptureEvaluationRow {
                    capture,
                    closure: owner,
                    local: value.local(),
                    access: value.access(),
                });
            }
            let end = checked_ordinal(self.capture_rows.len())?;
            self.captures_by_closure.insert(owner, start..end);
        }
        Ok(())
    }

    fn walk_expression_special_children(
        &mut self,
        owner: ExprId,
        path: &[HirSemanticPathStep],
        hops: &[HirExpressionSemanticHop],
        access: CaptureAccess,
        projection: HirExpressionTraversalProjection<'_>,
    ) -> Result<(), HirSemanticPathError> {
        if let Some(body) = projection
            .body
            .filter(|body| body.kind() == HirBodyKind::Thread)
        {
            for &edge in body.children() {
                self.walk_body(edge, path, hops, Some(owner), access)?;
            }
        }
        if let HirExprKind::Match(matched) = projection.kind {
            for (arm, row) in matched.arms().iter().enumerate() {
                let arm = checked_ordinal(arm)?;
                self.record_pattern_binding(
                    row.pattern(),
                    HirBindingSite::Expression {
                        expression: owner,
                        role: HirExpressionBindingRole::MatchArm { arm },
                    },
                    Some(owner),
                    Some(row.locals()),
                )?;
                self.walk_pattern(
                    row.pattern(),
                    &pushed(path, HirSemanticPathStep::MatchPattern { arm }),
                    hops,
                )?;
            }
        }
        Ok(())
    }

    fn walk_expression_direct_children(
        &mut self,
        owner: ExprId,
        path: &[HirSemanticPathStep],
        hops: &[HirExpressionSemanticHop],
        owning_parent: Option<ExprId>,
        access: CaptureAccess,
        projection: HirExpressionTraversalProjection<'_>,
    ) -> Result<(), HirSemanticPathError> {
        let owned_edges = projection
            .kind
            .expression_owned_child_edges()
            .map_err(HirSemanticPathError::from)?;
        for edge in owned_edges {
            self.walk_expression_owned_edge(owner, &edge, path, hops, access)?;
        }
        if let Some(body) = projection
            .body
            .filter(|body| body.kind() == HirBodyKind::Ordinary)
        {
            for edge in body.children() {
                if let HirBodyChild::Statement(statement) = edge.child() {
                    self.walk_statement(
                        statement,
                        &pushed(path, HirSemanticPathStep::Body(edge.role())),
                        hops,
                        Some(owner),
                        access,
                    )?;
                }
            }
        }
        for edge in projection
            .kind
            .try_child_edges()
            .map_err(|_| HirSemanticPathError::OrdinalOverflow)?
        {
            // Lowered `ForSynthetic::ForInput` is a reference-only edge; the
            // enclosing `for` statement owns the source/iterator/next-value
            // roots. Following it would publish a duplicate coordinate.
            if expression_edge_ownership(
                self.module,
                owner,
                owning_parent,
                path,
                edge.role(),
                edge.child(),
            )? == crate::expr::HirExpressionChildOwnership::ReferenceOnly
            {
                continue;
            }
            let mut child_hops = hops.to_vec();
            child_hops.push(HirExpressionSemanticHop {
                parent: owner,
                child: edge.child(),
                role: edge.role().clone(),
            });
            self.walk_expression(
                edge.child(),
                &pushed(path, HirSemanticPathStep::Expression(edge.role().clone())),
                &child_hops,
                Some(owner),
                access,
            )?;
        }
        // Every owning direct edge must publish exactly one child row. A
        // reference-only lowered edge (currently ForSynthetic::ForInput) is
        // deliberately excluded because its source statement owns the row.
        for edge in projection
            .kind
            .try_child_edges()
            .map_err(|_| HirSemanticPathError::OrdinalOverflow)?
        {
            if expression_edge_ownership(
                self.module,
                owner,
                owning_parent,
                path,
                edge.role(),
                edge.child(),
            )? == crate::expr::HirExpressionChildOwnership::Owning
                && !self.expressions.contains_key(&edge.child())
            {
                return Err(HirSemanticPathError::InvalidOwnedPath);
            }
        }
        Ok(())
    }

    fn record_selection_edges(
        &mut self,
        owner: ExprId,
        path: &[HirSemanticPathStep],
        owning_parent: Option<ExprId>,
        projection: HirExpressionTraversalProjection<'_>,
    ) -> Result<Vec<HirExpressionEvaluationEdge>, HirSemanticPathError> {
        let mut selection_edges = Vec::new();
        if let Some(body) = projection
            .body
            .filter(|body| body.kind() == HirBodyKind::Thread)
        {
            for &edge in body.children() {
                append_selection_thread_body_edge(
                    self.module,
                    edge,
                    &mut selection_edges,
                    HirSelectionStatementContext::Nested,
                )?;
            }
        }
        for edge in projection
            .kind
            .expression_owned_child_edges()
            .map_err(HirSemanticPathError::from)?
        {
            match edge.child() {
                HirExpressionOwnedChild::Pattern(_) => {}
                HirExpressionOwnedChild::Statement(statement) => {
                    append_selection_statement_steps(
                        self.module,
                        statement,
                        &mut selection_edges,
                        HirSelectionStatementContext::Owned(edge.role()),
                    )?;
                }
                HirExpressionOwnedChild::Body(body) => {
                    append_selection_thread_body_edge(
                        self.module,
                        body,
                        &mut selection_edges,
                        HirSelectionStatementContext::Owned(edge.role()),
                    )?;
                }
            }
        }
        if let Some(body) = projection
            .body
            .filter(|body| body.kind() == HirBodyKind::Ordinary)
        {
            for edge in body.children() {
                if let HirBodyChild::Statement(statement) = edge.child() {
                    append_selection_statement_steps(
                        self.module,
                        statement,
                        &mut selection_edges,
                        HirSelectionStatementContext::Body(edge.role()),
                    )?;
                }
            }
        }
        for edge in projection
            .kind
            .try_child_edges()
            .map_err(|_| HirSemanticPathError::OrdinalOverflow)?
        {
            selection_edges.push(HirExpressionEvaluationEdge::Expression {
                role: edge.role().clone(),
                ownership: expression_edge_ownership(
                    self.module,
                    owner,
                    owning_parent,
                    path,
                    edge.role(),
                    edge.child(),
                )?,
                child: edge.child(),
            });
        }
        if matches!(
            projection.kind,
            HirExprKind::Await(_)
                | HirExprKind::Choice(_)
                | HirExprKind::DialogueContentApplication(_)
        ) {
            selection_edges.sort_by_key(expression_selection_edge_order);
        }
        Ok(selection_edges)
    }

    fn walk_expression(
        &mut self,
        owner: ExprId,
        path: &[HirSemanticPathStep],
        hops: &[HirExpressionSemanticHop],
        owning_parent: Option<ExprId>,
        access: CaptureAccess,
    ) -> Result<(), HirSemanticPathError> {
        if self.active_expressions.contains(&owner) {
            return Err(HirSemanticPathError::CyclicPath {
                owner: owner.into(),
            });
        }
        insert_unique(&mut self.expressions, owner, path, hops)?;
        self.active_expressions.insert(owner);
        let expression = self
            .module
            .resolve_expr(owner)
            .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
        let kind = expression.kind();
        self.record_expression_start(owner, owning_parent, access, kind)?;
        let direct_body = self.record_expression_body_rows(owner, path, hops, kind)?;
        let projection = HirExpressionTraversalProjection {
            kind,
            body: direct_body.as_ref(),
        };
        self.walk_expression_special_children(owner, path, hops, access, projection)?;
        self.walk_expression_direct_children(owner, path, hops, owning_parent, access, projection)?;
        let selection_edges =
            self.record_selection_edges(owner, path, owning_parent, projection)?;
        let Some(row) = self.expression_uses.get_mut(&owner) else {
            return Err(HirSemanticPathError::InvalidOwnedPath);
        };
        row.subtree_end_ordinal = self.next_source_ordinal;
        self.selection_edges.insert(owner, selection_edges);
        self.active_expressions.remove(&owner);
        Ok(())
    }

    fn walk_expression_owned_edge(
        &mut self,
        expression: ExprId,
        edge: &crate::expr::HirExpressionOwnedChildEdge,
        parent: &[HirSemanticPathStep],
        hops: &[HirExpressionSemanticHop],
        access: CaptureAccess,
    ) -> Result<(), HirSemanticPathError> {
        let path = pushed(
            parent,
            HirSemanticPathStep::ExpressionOwned(edge.role().clone()),
        );
        match edge.child() {
            HirExpressionOwnedChild::Pattern(owner) => {
                if let Some(role) = expression_binding_role(edge.role()) {
                    self.record_pattern_binding(
                        owner,
                        HirBindingSite::Expression { expression, role },
                        Some(expression),
                        None,
                    )?;
                }
                self.walk_pattern(owner, &path, hops)
            }
            HirExpressionOwnedChild::Statement(owner)
                if matches!(
                    edge.role(),
                    HirExpressionOwnedBodyRole::DialogueLinePlanStatement { .. }
                ) =>
            {
                self.walk_expression_owned_statement(owner, &path, hops, expression, access)
            }
            HirExpressionOwnedChild::Statement(owner) => {
                self.walk_statement(owner, &path, hops, Some(expression), access)
            }
            HirExpressionOwnedChild::Body(edge) => {
                self.walk_body(edge, &path, hops, Some(expression), access)
            }
        }
    }

    fn walk_expression_owned_statement(
        &mut self,
        owner: StmtId,
        path: &[HirSemanticPathStep],
        hops: &[HirExpressionSemanticHop],
        owning_parent: ExprId,
        access: CaptureAccess,
    ) -> Result<(), HirSemanticPathError> {
        let statement = self
            .module
            .resolve_stmt(owner)
            .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
        self.line_plan_applications.push(owning_parent);
        let result = (|| match statement.kind() {
            HirStmtKind::Let {
                pattern,
                initializer,
                locals,
                ..
            } if self.patterns.contains_key(pattern)
                && locals.iter().all(|local| self.locals.contains_key(local)) =>
            {
                insert_unique(&mut self.statements, owner, path, hops)?;
                for local in locals {
                    self.merge_local_origin(
                        *local,
                        HirLocalValueOrigin::DirectInitializer(*initializer),
                    )?;
                }
                Ok(())
            }
            HirStmtKind::Out { .. } => {
                insert_unique(&mut self.statements, owner, path, hops)?;
                self.record_control_transfer(owner, statement.kind())
            }
            _ => self.walk_statement(owner, path, hops, Some(owning_parent), access),
        })();
        self.line_plan_applications.pop();
        result
    }

    fn record_control_transfer(
        &mut self,
        statement: StmtId,
        kind: &HirStmtKind,
    ) -> Result<(), HirSemanticPathError> {
        let (kind, target) = match kind {
            HirStmtKind::Out { label, .. } => {
                if label.is_some() {
                    return Err(HirControlTransferResolutionError::UnresolvedTarget {
                        statement,
                        kind: HirControlTransferKind::Out,
                    }
                    .into());
                }
                let Some(application) = self.line_plan_applications.last().copied() else {
                    return Err(HirControlTransferResolutionError::UnresolvedTarget {
                        statement,
                        kind: HirControlTransferKind::Out,
                    }
                    .into());
                };
                let expression = self
                    .module
                    .resolve_expr(application)
                    .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
                let HirExprKind::DialogueContentApplication(value) = expression.kind() else {
                    return Err(
                        HirControlTransferResolutionError::InvalidOutputApplication { statement }
                            .into(),
                    );
                };
                if value.plan().is_none() {
                    return Err(
                        HirControlTransferResolutionError::InvalidOutputApplication { statement }
                            .into(),
                    );
                }
                (
                    HirControlTransferKind::Out,
                    HirControlTransferTarget::output(application),
                )
            }
            HirStmtKind::Break { label, value } => {
                if label.is_some() {
                    return Err(HirControlTransferResolutionError::UnresolvedTarget {
                        statement,
                        kind: HirControlTransferKind::Break,
                    }
                    .into());
                }
                let (family, body_owner) =
                    self.nearest_loop_target(statement, HirControlTransferKind::Break)?;
                if value.is_some() && family != HirLoopTargetFamily::LoopExpression {
                    return Err(
                        HirControlTransferResolutionError::BreakValueRequiresLoopExpression {
                            statement,
                        }
                        .into(),
                    );
                }
                (
                    HirControlTransferKind::Break,
                    HirControlTransferTarget::loop_target(family, body_owner),
                )
            }
            HirStmtKind::Continue { label } => {
                if label.is_some() {
                    return Err(HirControlTransferResolutionError::UnresolvedTarget {
                        statement,
                        kind: HirControlTransferKind::Continue,
                    }
                    .into());
                }
                let (family, body_owner) =
                    self.nearest_loop_target(statement, HirControlTransferKind::Continue)?;
                (
                    HirControlTransferKind::Continue,
                    HirControlTransferTarget::loop_target(family, body_owner),
                )
            }
            _ => return Ok(()),
        };
        if self
            .control_transfers
            .insert(
                statement,
                HirControlTransferRow::new(statement, kind, target),
            )
            .is_some()
        {
            return Err(HirSemanticPathError::DuplicateControlTransfer { statement });
        }
        Ok(())
    }

    fn nearest_loop_target(
        &self,
        statement_id: StmtId,
        kind: HirControlTransferKind,
    ) -> Result<(HirLoopTargetFamily, HirSemanticBodyOwner), HirSemanticPathError> {
        let statement = self
            .module
            .resolve_stmt(statement_id)
            .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
        let mut scope = Some(statement.scope());
        let mut visited = BTreeSet::new();
        while let Some(scope_id) = scope {
            if !visited.insert(scope_id) {
                return Err(HirControlTransferResolutionError::InvalidScopeChain {
                    statement: statement_id,
                }
                .into());
            }
            let value = self
                .module
                .resolve_scope(scope_id)
                .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
            match value.owner() {
                HirScopeOwner::Expr(owner) => {
                    let expression = self
                        .module
                        .resolve_expr(*owner)
                        .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
                    if matches!(expression.kind(), HirExprKind::Loop(_)) {
                        return Ok((
                            HirLoopTargetFamily::LoopExpression,
                            HirSemanticBodyOwner::direct_expression(*owner),
                        ));
                    }
                }
                HirScopeOwner::Stmt(owner) => {
                    let statement = self
                        .module
                        .resolve_stmt(*owner)
                        .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
                    let target = match statement.kind() {
                        HirStmtKind::While(_) => Some((
                            HirLoopTargetFamily::WhileStatement,
                            HirStatementBodyRole::While,
                        )),
                        HirStmtKind::WhileLet(_) => Some((
                            HirLoopTargetFamily::WhileLetStatement,
                            HirStatementBodyRole::WhileLet,
                        )),
                        HirStmtKind::For(_) => {
                            Some((HirLoopTargetFamily::ForStatement, HirStatementBodyRole::For))
                        }
                        _ => None,
                    };
                    if let Some((family, role)) = target {
                        return Ok((family, HirSemanticBodyOwner::statement_body(*owner, role)));
                    }
                }
                HirScopeOwner::Module(_) | HirScopeOwner::Item(_) => {}
            }
            scope = value.parent();
        }
        Err(HirControlTransferResolutionError::UnresolvedTarget {
            statement: statement_id,
            kind,
        }
        .into())
    }

    fn walk_statement(
        &mut self,
        owner: StmtId,
        path: &[HirSemanticPathStep],
        hops: &[HirExpressionSemanticHop],
        owning_parent: Option<ExprId>,
        access: CaptureAccess,
    ) -> Result<(), HirSemanticPathError> {
        if self.active_statements.contains(&owner) {
            return Err(HirSemanticPathError::CyclicPath {
                owner: owner.into(),
            });
        }
        insert_unique(&mut self.statements, owner, path, hops)?;
        self.active_statements.insert(owner);
        let statement = self
            .module
            .resolve_stmt(owner)
            .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
        self.record_control_transfer(owner, statement.kind())?;
        let body_projections =
            self.record_statement_body_rows(owner, path, hops, statement.kind())?;
        self.record_statement_local_origins(owner, statement.kind(), owning_parent)?;
        for edge in statement
            .kind()
            .try_child_edges()
            .map_err(|_| HirSemanticPathError::OrdinalOverflow)?
        {
            let path = pushed(path, HirSemanticPathStep::Statement(edge.role()));
            let child_access = if matches!(edge.role(), HirStatementChildRole::Target) {
                CaptureAccess::Reassign
            } else {
                access
            };
            match edge.child() {
                HirStatementChild::Expression(owner) => {
                    self.walk_expression(owner, &path, hops, owning_parent, child_access)?;
                }
                HirStatementChild::Statement(owner) => {
                    self.walk_statement(owner, &path, hops, owning_parent, child_access)?;
                }
                HirStatementChild::Pattern(owner) => self.walk_pattern(owner, &path, hops)?,
                HirStatementChild::Type(_) => {}
                HirStatementChild::Local(owner) => {
                    insert_unique(&mut self.locals, owner, &path, hops)?;
                }
            }
        }
        for body in body_projections
            .iter()
            .filter(|body| body.projection().kind() == HirBodyKind::Thread)
        {
            let body_path = pushed(path, HirSemanticPathStep::StatementBody(*body.role()));
            for edge in body.projection().children().iter().copied() {
                self.walk_body(edge, &body_path, hops, owning_parent, access)?;
            }
        }
        self.active_statements.remove(&owner);
        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the statement binding algebra is recorded once in exhaustive source order"
    )]
    fn record_statement_local_origins(
        &mut self,
        statement_id: StmtId,
        kind: &HirStmtKind,
        binding_expression: Option<ExprId>,
    ) -> Result<(), HirSemanticPathError> {
        let direct = match kind {
            HirStmtKind::Let {
                pattern,
                initializer,
                locals,
                ..
            } => Some((
                *pattern,
                *initializer,
                locals,
                true,
                HirLocalBindingStatementRole::Let,
            )),
            HirStmtKind::LetElse {
                pattern,
                initializer,
                locals,
                ..
            } => Some((
                *pattern,
                *initializer,
                locals,
                false,
                HirLocalBindingStatementRole::LetElse,
            )),
            HirStmtKind::LetChoice {
                pattern,
                choice,
                locals,
            } => Some((
                *pattern,
                *choice,
                locals,
                false,
                HirLocalBindingStatementRole::LetChoice,
            )),
            HirStmtKind::LetScope {
                pattern,
                scope_expr,
                locals,
            } => Some((
                *pattern,
                *scope_expr,
                locals,
                false,
                HirLocalBindingStatementRole::LetScope,
            )),
            HirStmtKind::LetActionReceive {
                pattern,
                action,
                locals,
            } => Some((
                *pattern,
                *action,
                locals,
                false,
                HirLocalBindingStatementRole::LetActionReceive,
            )),
            _ => None,
        };
        if let Some((pattern, input, locals, direct_statement, statement_role)) = direct {
            self.record_pattern_binding(
                pattern,
                HirBindingSite::Statement {
                    statement: statement_id,
                    role: statement_role,
                },
                binding_expression,
                Some(locals),
            )?;
            if direct_statement {
                for local in locals.iter().copied() {
                    let origin = self.local_origin_for_binding(
                        local,
                        pattern,
                        input,
                        locals.len(),
                        direct_statement,
                    )?;
                    self.merge_local_origin(local, origin)?;
                }
            }
        }
        match kind {
            HirStmtKind::IfLet(value) => self.record_pattern_binding(
                value.pattern(),
                HirBindingSite::Statement {
                    statement: statement_id,
                    role: HirLocalBindingStatementRole::IfLet,
                },
                binding_expression,
                Some(value.locals()),
            )?,
            HirStmtKind::Match(value) => {
                for (arm, value) in value.arms().iter().enumerate() {
                    self.record_pattern_binding(
                        value.pattern(),
                        HirBindingSite::Statement {
                            statement: statement_id,
                            role: HirLocalBindingStatementRole::MatchArm {
                                arm: checked_ordinal(arm)?,
                            },
                        },
                        binding_expression,
                        Some(value.locals()),
                    )?;
                }
            }
            HirStmtKind::WhileLet(value) => self.record_pattern_binding(
                value.pattern(),
                HirBindingSite::Statement {
                    statement: statement_id,
                    role: HirLocalBindingStatementRole::WhileLet,
                },
                binding_expression,
                Some(value.locals()),
            )?,
            HirStmtKind::For(value) => self.record_pattern_binding(
                value.pattern(),
                HirBindingSite::Statement {
                    statement: statement_id,
                    role: HirLocalBindingStatementRole::For,
                },
                binding_expression,
                Some(value.locals()),
            )?,
            HirStmtKind::Select(HirSelectStmt::Branches { branches, .. }) => {
                for (branch, value) in branches.iter().enumerate() {
                    let branch = checked_ordinal(branch)?;
                    match value.head() {
                        HirSelectBranchHead::Frame { pattern, locals }
                        | HirSelectBranchHead::Event { pattern, locals } => {
                            self.record_pattern_binding(
                                *pattern,
                                HirBindingSite::Statement {
                                    statement: statement_id,
                                    role: HirLocalBindingStatementRole::SelectPattern { branch },
                                },
                                binding_expression,
                                Some(locals),
                            )?;
                        }
                        HirSelectBranchHead::Bind { binding, .. } => {
                            if let Some(local) = binding.resolved() {
                                self.insert_local_origin(
                                    local,
                                    HirBindingSite::Statement {
                                        statement: statement_id,
                                        role: HirLocalBindingStatementRole::SelectBinding {
                                            branch,
                                        },
                                    },
                                    binding_expression,
                                    None,
                                    HirLocalValueOrigin::Independent,
                                )?;
                            }
                        }
                        HirSelectBranchHead::Recovered => {}
                    }
                }
            }
            HirStmtKind::On { trigger, .. } => {
                if let Some(pattern) = trigger_pattern_id(trigger) {
                    self.record_pattern_binding(
                        pattern,
                        HirBindingSite::Statement {
                            statement: statement_id,
                            role: HirLocalBindingStatementRole::OnTrigger,
                        },
                        binding_expression,
                        None,
                    )?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "the site is cloned for each recursively collected local row"
    )]
    fn record_pattern_binding(
        &mut self,
        pattern: PatternId,
        site: HirBindingSite,
        binding_expression: Option<ExprId>,
        expected_locals: Option<&[LocalId]>,
    ) -> Result<(), HirSemanticPathError> {
        let locals = pattern_local_ids(self.module, pattern)?;
        if let Some(expected) = expected_locals
            && locals.as_slice() != expected
        {
            return Err(HirSemanticPathError::InvalidOwnedPath);
        }
        for local in locals {
            let origin = self.local_origin_for_pattern(local)?;
            self.insert_local_origin(
                local,
                site.clone(),
                binding_expression,
                Some(pattern),
                origin,
            )?;
        }
        Ok(())
    }

    fn local_origin_for_pattern(
        &self,
        local: LocalId,
    ) -> Result<HirLocalValueOrigin, HirSemanticPathError> {
        let kind = self
            .module
            .resolve_local(local)
            .map_err(|_| HirSemanticPathError::UnresolvedOwner)?
            .kind();
        Ok(if kind == HirLocalKind::LetBinding {
            HirLocalValueOrigin::Composite
        } else {
            HirLocalValueOrigin::Independent
        })
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "the site is cloned into each sealed row and must remain an owned typed coordinate"
    )]
    fn insert_local_origin(
        &mut self,
        local: LocalId,
        site: HirBindingSite,
        binding_expression: Option<ExprId>,
        pattern: Option<PatternId>,
        origin: HirLocalValueOrigin,
    ) -> Result<(), HirSemanticPathError> {
        match self.local_origins.entry(local) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(HirLocalBindingOrigin {
                    local,
                    site,
                    binding_expression,
                    pattern,
                    origin,
                });
                Ok(())
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let existing = entry.get_mut();
                if existing.site != site
                    || existing.binding_expression != binding_expression
                    || existing.pattern != pattern
                {
                    return Err(HirSemanticPathError::DuplicateLocalOrigin { owner: local });
                }
                if existing.origin != origin {
                    existing.origin = HirLocalValueOrigin::Composite;
                }
                Ok(())
            }
        }
    }

    fn merge_local_origin(
        &mut self,
        local: LocalId,
        origin: HirLocalValueOrigin,
    ) -> Result<(), HirSemanticPathError> {
        let Some(existing) = self.local_origins.get_mut(&local) else {
            return Err(HirSemanticPathError::InvalidOwnedPath);
        };
        if matches!(existing.origin, HirLocalValueOrigin::Composite)
            && matches!(origin, HirLocalValueOrigin::DirectInitializer(_))
        {
            existing.origin = origin;
        } else if existing.origin != origin {
            existing.origin = HirLocalValueOrigin::Composite;
        }
        Ok(())
    }

    fn local_origin_for_binding(
        &self,
        local: LocalId,
        pattern: PatternId,
        input: ExprId,
        local_count: usize,
        direct_statement: bool,
    ) -> Result<HirLocalValueOrigin, HirSemanticPathError> {
        classify_local_origin(
            self.module,
            local,
            pattern,
            input,
            local_count,
            direct_statement,
        )
    }

    fn walk_pattern(
        &mut self,
        owner: PatternId,
        path: &[HirSemanticPathStep],
        hops: &[HirExpressionSemanticHop],
    ) -> Result<(), HirSemanticPathError> {
        if self.active_patterns.contains(&owner) {
            return Err(HirSemanticPathError::CyclicPath {
                owner: owner.into(),
            });
        }
        insert_unique(&mut self.patterns, owner, path, hops)?;
        self.active_patterns.insert(owner);
        let pattern = self
            .module
            .resolve_pattern(owner)
            .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
        for edge in pattern
            .kind()
            .try_child_edges()
            .map_err(|_| HirSemanticPathError::OrdinalOverflow)?
        {
            let path = pushed(path, HirSemanticPathStep::Pattern(edge.role()));
            match edge.child() {
                HirPatternChild::Pattern(owner) => self.walk_pattern(owner, &path, hops)?,
                HirPatternChild::Type(_) => {}
                HirPatternChild::Local(owner) => {
                    insert_unique(&mut self.locals, owner, &path, hops)?;
                }
            }
        }
        self.active_patterns.remove(&owner);
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum HirSelectionStatementContext<'role> {
    Owned(&'role HirExpressionOwnedBodyRole),
    Body(HirBodyChildRole),
    Nested,
}

impl HirSelectionStatementContext<'_> {
    const fn nested(self) -> Self {
        match self {
            Self::Owned(role) => Self::Owned(role),
            Self::Body(_) | Self::Nested => Self::Nested,
        }
    }
}

fn record_statement_selection_steps(
    module: &HirModule,
    owner: StmtId,
    roots: &mut Vec<ExprId>,
) -> Result<(), HirSemanticPathError> {
    let statement = module
        .resolve_stmt(owner)
        .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
    let mut result = Ok(());
    statement
        .kind()
        .evaluation_plan()
        .try_visit_evaluation_steps(|step| {
            if result.is_err() {
                return;
            }
            result = match step {
                HirStmtEvaluationStep::Expression { expression, .. } => {
                    if !roots.contains(&expression) {
                        roots.push(expression);
                    }
                    Ok(())
                }
                HirStmtEvaluationStep::Statement { statement, .. } => {
                    record_statement_selection_steps(module, statement, roots)
                }
                HirStmtEvaluationStep::ThreadBody { edge, .. } => match edge.child() {
                    HirBodyChild::Expression(expression) => {
                        if !roots.contains(&expression) {
                            roots.push(expression);
                        }
                        Ok(())
                    }
                    HirBodyChild::Statement(statement) => {
                        record_statement_selection_steps(module, statement, roots)
                    }
                },
                HirStmtEvaluationStep::Pattern { .. }
                | HirStmtEvaluationStep::Type { .. }
                | HirStmtEvaluationStep::Local { .. }
                | HirStmtEvaluationStep::Publication { .. } => Ok(()),
            };
        })
        .map_err(map_evaluation_step_error)?;
    result
}

fn append_selection_statement_steps(
    module: &HirModule,
    statement: StmtId,
    edges: &mut Vec<HirExpressionEvaluationEdge>,
    context: HirSelectionStatementContext<'_>,
) -> Result<(), HirSemanticPathError> {
    let value = module
        .resolve_stmt(statement)
        .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
    let mut result = Ok(());
    value
        .kind()
        .evaluation_plan()
        .try_visit_evaluation_steps(|step| {
            if result.is_err() {
                return;
            }
            result = match step {
                HirStmtEvaluationStep::Expression { role, expression } => {
                    match context {
                        HirSelectionStatementContext::Owned(owner_role) => {
                            edges.push(HirExpressionEvaluationEdge::ExpressionOwnedStatement {
                                role: owner_role.clone(),
                                statement_role: role,
                                child: expression,
                            });
                        }
                        HirSelectionStatementContext::Body(body_role) => {
                            edges.push(HirExpressionEvaluationEdge::Body {
                                role: body_role,
                                child: expression,
                            });
                        }
                        HirSelectionStatementContext::Nested => {
                            edges.push(HirExpressionEvaluationEdge::Statement {
                                role,
                                child: expression,
                            });
                        }
                    }
                    Ok(())
                }
                HirStmtEvaluationStep::Statement { statement, .. } => {
                    append_selection_statement_steps(module, statement, edges, context.nested())
                }
                HirStmtEvaluationStep::ThreadBody { edge, .. } => {
                    append_selection_thread_body_edge(module, edge, edges, context.nested())
                }
                HirStmtEvaluationStep::Pattern { .. }
                | HirStmtEvaluationStep::Type { .. }
                | HirStmtEvaluationStep::Local { .. }
                | HirStmtEvaluationStep::Publication { .. } => Ok(()),
            };
        })
        .map_err(map_evaluation_step_error)?;
    result
}

fn append_selection_thread_body_edge(
    module: &HirModule,
    edge: HirBodyChildEdge,
    edges: &mut Vec<HirExpressionEvaluationEdge>,
    context: HirSelectionStatementContext<'_>,
) -> Result<(), HirSemanticPathError> {
    match edge.child() {
        HirBodyChild::Expression(expression) => match context {
            HirSelectionStatementContext::Owned(owner_role) => {
                edges.push(HirExpressionEvaluationEdge::ExpressionOwnedBody {
                    role: owner_role.clone(),
                    body_role: edge.role(),
                    child: expression,
                });
            }
            HirSelectionStatementContext::Body(_) | HirSelectionStatementContext::Nested => {
                edges.push(HirExpressionEvaluationEdge::Body {
                    role: edge.role(),
                    child: expression,
                });
            }
        },
        HirBodyChild::Statement(statement) => {
            append_selection_statement_steps(module, statement, edges, context.nested())?;
        }
    }
    Ok(())
}

fn map_evaluation_step_error(error: HirStmtEvaluationStepError) -> HirSemanticPathError {
    match error {
        HirStmtEvaluationStepError::OrdinalOverflow => HirSemanticPathError::OrdinalOverflow,
    }
}

fn expression_edge_ownership(
    module: &HirModule,
    owner: ExprId,
    owning_parent: Option<ExprId>,
    path: &[HirSemanticPathStep],
    role: &HirExpressionChildRole,
    child: ExprId,
) -> Result<crate::expr::HirExpressionChildOwnership, HirSemanticPathError> {
    if matches!(
        path.last(),
        Some(HirSemanticPathStep::Expression(
            HirExpressionChildRole::PostfixDialogueCandidate
        ))
    ) && let HirExpressionChildRole::DialogueCoordinate { ordinal } = role
    {
        let parent = owning_parent.ok_or(HirSemanticPathError::InvalidOwnedPath)?;
        let parent = module
            .resolve_expr(parent)
            .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
        let HirExprKind::PostfixBracket(postfix) = parent.kind() else {
            return Err(HirSemanticPathError::InvalidOwnedPath);
        };
        let crate::dialogue_application::HirPostfixBracketCandidates::Ambiguous {
            dialogue, ..
        } = postfix.candidates()
        else {
            return Err(HirSemanticPathError::InvalidOwnedPath);
        };
        let expression = module
            .resolve_expr(owner)
            .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
        let HirExprKind::DialogueContentApplication(application) = expression.kind() else {
            return Err(HirSemanticPathError::InvalidOwnedPath);
        };
        let coordinate = application
            .coordinates()
            .iter()
            .find(|coordinate| u32::from(coordinate.argument().get()) == *ordinal)
            .ok_or(HirSemanticPathError::InvalidOwnedPath)?;
        if *dialogue != owner
            || postfix.target() != application.target()
            || coordinate.value() != child
        {
            return Err(HirSemanticPathError::InvalidOwnedPath);
        }
        return Ok(crate::expr::HirExpressionChildOwnership::ReferenceOnly);
    }
    if matches!(
        path.last(),
        Some(HirSemanticPathStep::Expression(
            HirExpressionChildRole::PostfixIndexCandidate
                | HirExpressionChildRole::PostfixDialogueCandidate
        ))
    ) && matches!(
        role,
        HirExpressionChildRole::Target | HirExpressionChildRole::DialogueTarget
    ) {
        return Ok(crate::expr::HirExpressionChildOwnership::ReferenceOnly);
    }
    if matches!(
        path.last(),
        Some(HirSemanticPathStep::Expression(
            HirExpressionChildRole::DialogueTarget
        ))
    ) && let HirExpressionChildRole::Argument { ordinal } = role
    {
        let parent = owning_parent.ok_or(HirSemanticPathError::InvalidOwnedPath)?;
        let parent = module
            .resolve_expr(parent)
            .map_err(|_| HirSemanticPathError::UnresolvedOwner)?;
        let HirExprKind::DialogueContentApplication(application) = parent.kind() else {
            return Err(HirSemanticPathError::InvalidOwnedPath);
        };
        if application.target() != owner {
            return Err(HirSemanticPathError::InvalidOwnedPath);
        }
        let coordinate = application
            .coordinates()
            .iter()
            .find(|coordinate| u32::from(coordinate.argument().get()) == *ordinal);
        if let Some(coordinate) = coordinate {
            if coordinate.value() != child {
                return Err(HirSemanticPathError::InvalidOwnedPath);
            }
            return Ok(crate::expr::HirExpressionChildOwnership::ReferenceOnly);
        }
    }
    Ok(role.ownership())
}

fn expression_selection_edge_order(
    edge: &HirExpressionEvaluationEdge,
) -> (Vec<HirNestedExpressionPathSegment>, u8) {
    match edge {
        HirExpressionEvaluationEdge::Expression { role, .. } => {
            (nested_expression_role_path(role), 0)
        }
        HirExpressionEvaluationEdge::ExpressionOwnedBody { role, .. }
        | HirExpressionEvaluationEdge::ExpressionOwnedStatement { role, .. } => {
            (nested_owned_role_path(role), 1)
        }
        _ => (Vec::new(), 2),
    }
}

fn nested_expression_role_path(
    role: &HirExpressionChildRole,
) -> Vec<HirNestedExpressionPathSegment> {
    match role {
        HirExpressionChildRole::LinePlanOptionValue { path }
        | HirExpressionChildRole::LinePlanLetValue { path }
        | HirExpressionChildRole::LinePlanOut { path }
        | HirExpressionChildRole::LinePlanTimelineAssert { path }
        | HirExpressionChildRole::LinePlanExpression { path }
        | HirExpressionChildRole::ChoiceIfCondition { path, .. }
        | HirExpressionChildRole::ChoiceForSource { path }
        | HirExpressionChildRole::ChoiceMatchScrutinee { path }
        | HirExpressionChildRole::ChoiceOptionId { path }
        | HirExpressionChildRole::ChoiceOptionForSource { path }
        | HirExpressionChildRole::ChoiceCompactLabel { path }
        | HirExpressionChildRole::ChoiceCompactCondition { path }
        | HirExpressionChildRole::ChoiceCompactOut { path }
        | HirExpressionChildRole::ChoiceOptionLabel { path, .. }
        | HirExpressionChildRole::ChoiceOptionFieldId { path, .. }
        | HirExpressionChildRole::ChoiceOptionValue { path, .. }
        | HirExpressionChildRole::ChoiceOptionVisible { path, .. }
        | HirExpressionChildRole::ChoiceOptionEnabled { path, .. }
        | HirExpressionChildRole::ChoiceOptionOrder { path, .. }
        | HirExpressionChildRole::ChoiceOptionHotkey { path, .. }
        | HirExpressionChildRole::ChoiceOptionViewKey { path, .. }
        | HirExpressionChildRole::ChoiceOptionViewValue { path, .. }
        | HirExpressionChildRole::ChoiceMatchGuard { path, .. } => path.segments().to_vec(),
        HirExpressionChildRole::ChoicePlanAssignment { item }
        | HirExpressionChildRole::ChoicePlanTimeout { item }
        | HirExpressionChildRole::ChoicePlanCancelSignal { item }
        | HirExpressionChildRole::ChoicePlanCancelTimeout { item }
        | HirExpressionChildRole::ChoicePlanCancelExpr { item } => {
            vec![HirNestedExpressionPathSegment::ChoicePlanItem { ordinal: *item }]
        }
        _ => Vec::new(),
    }
}

fn nested_owned_role_path(
    role: &HirExpressionOwnedBodyRole,
) -> Vec<HirNestedExpressionPathSegment> {
    match role {
        HirExpressionOwnedBodyRole::ChoiceLetStatement { path }
        | HirExpressionOwnedBodyRole::ChoiceForPattern { path }
        | HirExpressionOwnedBodyRole::ChoiceMatchArmPattern { path, .. }
        | HirExpressionOwnedBodyRole::ChoiceOptionForPattern { path }
        | HirExpressionOwnedBodyRole::ChoiceOptionSelectBody { path, .. }
        | HirExpressionOwnedBodyRole::ChoiceOptionLetStatement { path, .. }
        | HirExpressionOwnedBodyRole::ChoicePlanTimeoutBody { path }
        | HirExpressionOwnedBodyRole::ChoicePlanCancelTrigger { path }
        | HirExpressionOwnedBodyRole::ChoicePlanCancelBody { path }
        | HirExpressionOwnedBodyRole::ChoicePlanOnSelectPattern { path }
        | HirExpressionOwnedBodyRole::ChoicePlanOnSelectBody { path }
        | HirExpressionOwnedBodyRole::DialogueLinePlanStatement { path, .. }
        | HirExpressionOwnedBodyRole::DialogueLinePlanLet { path } => path.segments().to_vec(),
        _ => Vec::new(),
    }
}

fn postcondition_result_local(
    module: &HirModule,
    scope: crate::identity::ScopeId,
) -> Result<Option<LocalId>, HirSemanticPathError> {
    let scope = module
        .resolve_scope(scope)
        .map_err(|_| HirSemanticPathError::InvalidResultOrigin)?;
    let mut results = scope.locals().iter().copied().filter(|local| {
        module
            .resolve_local(*local)
            .is_ok_and(|value| value.kind() == HirLocalKind::PostconditionResult)
    });
    let Some(result) = results.next() else {
        return Ok(None);
    };
    if results.next().is_some() {
        return Err(HirSemanticPathError::InvalidResultPath);
    }
    Ok(Some(result))
}

fn pushed(parent: &[HirSemanticPathStep], step: HirSemanticPathStep) -> Vec<HirSemanticPathStep> {
    let mut path = Vec::with_capacity(parent.len() + 1);
    path.extend_from_slice(parent);
    path.push(step);
    path
}

fn insert_unique<K: Into<HirSemanticPathOwnerId> + Ord + Copy>(
    rows: &mut BTreeMap<K, HirSemanticOwnerPath>,
    owner: K,
    path: &[HirSemanticPathStep],
    hops: &[HirExpressionSemanticHop],
) -> Result<(), HirSemanticPathError> {
    if let std::collections::btree_map::Entry::Vacant(entry) = rows.entry(owner) {
        entry.insert(HirSemanticOwnerPath::new(path.into(), hops.into()));
        Ok(())
    } else {
        Err(HirSemanticPathError::DuplicatePath {
            owner: owner.into(),
        })
    }
}

#[cfg(test)]
#[path = "semantic_paths/tests.rs"]
mod tests;
