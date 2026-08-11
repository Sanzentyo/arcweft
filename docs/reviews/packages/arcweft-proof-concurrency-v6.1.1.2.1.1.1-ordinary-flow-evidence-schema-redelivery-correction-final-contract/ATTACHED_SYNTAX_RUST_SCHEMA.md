# Exact attached-syntax Rust schema

Owning crate: `arcweft-lang-syntax`

Owning modules:

```text
grammar::kinds
grammar::roles
attachment::node
attachment::flow
attachment::thread_body
```

All fields are private. Constructors are `pub(crate)` and checked. Public
syntax consumers receive read-only handles and accessors only. None of these
types implements Serde. No source text, copied range, detached `Flow`,
detached `FlowItem`, or detached `ContractClause` is retained.

## In-place grammar vocabulary changes

The following variants are added to the original `SyntaxKind` enum. They are
not defined in a Flow-local enum.

```rust
// crates/arcweft-lang-syntax/src/grammar/kinds.rs
pub(crate) enum SyntaxKind {
    // Existing variants remain, except the deletion below.

    InvariantClause,
    AssumeClause,
    ReadsClause,
    EffectsClause,
    NoEffectClause,
    ModifiesClause,
    DecreasesClause,

    ChoiceStatement,
    SourceLocaleStatement,
    ScopeStatement,
    IncludeStatement,
    AwaitWithStatement,

    DialogueContentApplicationExpression,
}
```

`RequiresClause` and `EnsuresClause` remain and join the same contract family.
`DialogueCallExpression` is deleted in the public replacement cut and replaced
directly by `DialogueContentApplicationExpression`. No alias or conversion
variant survives.

The original classifiers gain exhaustive arms:

```rust
impl SyntaxKind {
    pub(crate) const fn is_contract_clause(self) -> bool {
        matches!(
            self,
            Self::RequiresClause
                | Self::EnsuresClause
                | Self::InvariantClause
                | Self::AssumeClause
                | Self::ReadsClause
                | Self::EffectsClause
                | Self::NoEffectClause
                | Self::ModifiesClause
                | Self::DecreasesClause
        )
    }

    pub(crate) const fn is_thread_flow_item(self) -> bool {
        self.is_statement()
            || matches!(self, Self::DialogueContentApplicationExpression)
    }
}
```

`is_statement()` includes the five new statement kinds. The original
`AstTag`, identity-class, marker, family, and budget tables are extended in
place. All clause nodes, all five new statement nodes, and the dialogue
application expression are identity-bearing.

## In-place role changes

`SyntaxRole::{RequiresClause(u16), EnsuresClause(u16)}` and their role-class
variants are deleted when all callers use the heterogeneous role below.

```rust
// crates/arcweft-lang-syntax/src/grammar/roles.rs
pub(crate) enum SyntaxRole {
    // Existing variants.
    ContractClause(u16),
    ContractMode,
    ContractOperand(u16),
    ThreadFlowItem(u32),
    TrailingRecovery(u32),
}

pub(crate) enum SyntaxRoleClass {
    // Existing variants.
    ContractClause,
    ContractMode,
    ContractOperand,
    ThreadFlowItem,
    TrailingRecovery,
}
```

`SyntaxRole::class()` and `ordinal()` are extended on the original enum.
`ContractClause`, `ContractOperand`, `ThreadFlowItem`, and
`TrailingRecovery` return their carried ordinal. `ContractMode` returns no
ordinal. Contract and body source order is therefore independent of family and
arena allocation order.

## Shared attached primitives

The attachment layer owns these primitives. They are handles into one immutable
syntax snapshot, not copied source coordinates.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AttachedTokenSite {
    token: SyntaxTokenHandle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AttachedInsertionSite {
    snapshot: SyntaxSnapshotId,
    offset: SyntaxOffset,
    boundary: AttachedRecoveryBoundary,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum AttachedRecoveryBoundary {
    NextContractClause,
    BodyOpen,
    BodyClose,
    NextTopLevelItem,
    EndOfDocument,
    Token(SyntaxKind),
}

impl AttachedTokenSite {
    pub(crate) fn try_new(
        token: SyntaxTokenHandle,
        expected: SyntaxKind,
    ) -> Result<Self, SyntaxAccessError>;

    pub(crate) fn token(&self) -> SyntaxTokenHandle;
    pub(crate) fn kind(&self) -> SyntaxKind;
    pub(crate) fn snapshot_id(&self) -> &SyntaxSnapshotId;
    pub(crate) fn range(&self) -> SourceRange;
}

impl AttachedInsertionSite {
    pub(crate) fn try_new(
        snapshot: SyntaxSnapshotId,
        offset: SyntaxOffset,
        boundary: AttachedRecoveryBoundary,
    ) -> Result<Self, SyntaxAccessError>;

    pub(crate) fn snapshot_id(&self) -> &SyntaxSnapshotId;
    pub(crate) fn offset(&self) -> SyntaxOffset;
    pub(crate) fn boundary(&self) -> AttachedRecoveryBoundary;
}
```

`SyntaxOffset` is the existing checked UTF-8-boundary offset owner. It is not a
raw `usize` public constructor.

## Flow declaration owner

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FlowItemNode {
    syntax: AstNode<FlowItemKind>,
    prefix: AttachedItemPrefix,
    flow_keyword: AttachedTokenSite,
    identity: AttachedFlowIdentity,
    signature: AttachedFlowSignature,
    contracts: Box<[AttachedFlowContractClause]>,
    body: AttachedRequiredFlowBody,
    trailing_recovery: Box<[RecoveryNode]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AttachedItemPrefix {
    documentation: Option<AstNode<DocBlockKind>>,
    attributes: Box<[AstNode<OuterAttributeKind>]>,
    visibility: Option<AstNode<VisibilityKind>>,
}

impl FlowItemNode {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_new(
        syntax: AstNode<FlowItemKind>,
        prefix: AttachedItemPrefix,
        flow_keyword: AttachedTokenSite,
        identity: AttachedFlowIdentity,
        signature: AttachedFlowSignature,
        contracts: Box<[AttachedFlowContractClause]>,
        body: AttachedRequiredFlowBody,
        trailing_recovery: Box<[RecoveryNode]>,
    ) -> Result<Self, FlowAttachmentError>;

    pub(crate) fn syntax(&self) -> &AstNode<FlowItemKind>;
    pub(crate) fn prefix(&self) -> &AttachedItemPrefix;
    pub(crate) fn flow_keyword(&self) -> &AttachedTokenSite;
    pub(crate) fn identity(&self) -> &AttachedFlowIdentity;
    pub(crate) fn signature(&self) -> &AttachedFlowSignature;
    pub(crate) fn contracts(&self) -> &[AttachedFlowContractClause];
    pub(crate) fn body(&self) -> &AttachedRequiredFlowBody;
    pub(crate) fn trailing_recovery(&self) -> &[RecoveryNode];
}
```

The constructor validates one snapshot, exact child roles, role continuity,
source containment, source order, identity state, at most one admitted
parameter group, heterogeneous clause order, body ownership, and disjoint
trailing recovery.

## Four identity states

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AttachedFlowIdentity {
    Name {
        name: AstNode<NameDefinitionKind>,
    },
    PublicId {
        public_id: AttachedFlowPublicId,
    },
    PublicIdAndName {
        public_id: AttachedFlowPublicId,
        name: AstNode<NameDefinitionKind>,
    },
    Missing {
        missing: AstNode<MissingNameKind>,
        insertion: AttachedInsertionSite,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AttachedFlowPublicId {
    syntax: AstNode<DeclarationPublicIdKind>,
    value: AttachedFlowIdSyntax,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AttachedFlowIdSyntax {
    Authored(IdRef),
    DerivedFromEmptyMarker {
        marker_family: Option<DeclarationFamilyName>,
    },
}

impl AttachedFlowPublicId {
    pub(crate) fn try_new(
        syntax: AstNode<DeclarationPublicIdKind>,
        value: AttachedFlowIdSyntax,
    ) -> Result<Self, FlowAttachmentError>;

    pub(crate) fn syntax(&self) -> &AstNode<DeclarationPublicIdKind>;
    pub(crate) fn value(&self) -> &AttachedFlowIdSyntax;
}

impl AttachedFlowIdentity {
    pub(crate) fn name(&self) -> Option<&AstNode<NameDefinitionKind>>;
    pub(crate) fn public_id(&self) -> Option<&AttachedFlowPublicId>;
    pub(crate) fn is_missing(&self) -> bool;
}
```

`DerivedFromEmptyMarker` is admitted only in
`PublicIdAndName`; `flow @. name` and `flow @flow:. name` use the following
name as the grammar-defined family-local suffix. The marker node remains the
public-ID source component. No later source read reconstructs the suffix.

Semantic identity rules:

- the qualified final `ItemId` is the internal callable identity in all four
  states;
- an authored or marker-derived public ID is the optional project public
  identity;
- `Name` is presentation/local lookup;
- name-only project publication performs the maintained module-scoped Flow-ID
  derivation inside the accepted project transaction;
- `Missing` publishes no project/callable candidate;
- an authored ID/name mismatch retains both children and poisons publication;
  neither spelling is silently rewritten.

## Signature owner

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AttachedFlowSignature {
    generics: Option<AttachedFlowGenericGroup>,
    parameters: Option<AttachedFlowParameterGroup>,
    result: AttachedFlowReturnSyntax,
    where_clause: Option<AttachedFlowWhereClause>,
    recovery: Box<[AttachedFlowSignatureRecovery]>,
    end: AttachedInsertionSite,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AttachedFlowGenericGroup {
    syntax: AstNode<GenericParameterGroupKind>,
    open: AstNode<OpenAngleKind>,
    parameters: Box<[AttachedFlowGenericParameter]>,
    close: AttachedDelimiterState<CloseAngleKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AttachedFlowGenericParameter {
    Lifetime {
        syntax: AstNode<LifetimeParameterKind>,
        name: AstNode<NameDefinitionKind>,
    },
    Type {
        syntax: AstNode<TypeParameterKind>,
        name: AstNode<NameDefinitionKind>,
        bounds: Box<[TypeNode]>,
    },
    Error {
        syntax: RecoveryNode,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AttachedFlowParameterGroup {
    syntax: AstNode<FixedParameterGroupKind>,
    open: AstNode<OpenParenKind>,
    parameters: Box<[AttachedFlowParameter]>,
    close: AttachedDelimiterState<CloseParenKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AttachedFlowParameter {
    syntax: AstNode<ParameterKind>,
    pattern: PatternNode,
    colon: AttachedRequiredToken,
    ty: TypeNode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AttachedFlowReturnSyntax {
    Omitted,
    Authored {
        syntax: AstNode<ReturnTypeKind>,
        arrow: AttachedTokenSite,
        ty: TypeNode,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AttachedFlowWhereClause {
    syntax: AstNode<WhereClauseKind>,
    keyword: AttachedTokenSite,
    predicates: Box<[AttachedFlowWherePredicate]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AttachedFlowWherePredicate {
    syntax: AstNode<WherePredicateKind>,
    subject: TypeNode,
    colon: AttachedRequiredToken,
    bounds: Box<[TypeNode]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AttachedFlowSignatureRecovery {
    SecondParameterGroup {
        syntax: AstNode<ErrorNodeKind>,
        group: AstNode<FixedParameterGroupKind>,
    },
    UnexpectedHeaderNode {
        syntax: RecoveryNode,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AttachedDelimiterState<K: ExactAstKind> {
    Present(AstNode<K>),
    Missing {
        node: AstNode<MissingTokenNodeKind>,
        insertion: AttachedInsertionSite,
    },
    InvalidPresent {
        node: RecoveryNode,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AttachedRequiredToken {
    Present(AttachedTokenSite),
    Missing {
        node: AstNode<MissingTokenNodeKind>,
        insertion: AttachedInsertionSite,
    },
    InvalidPresent {
        node: RecoveryNode,
    },
}
```

Required accessor signatures:

```rust
impl AttachedFlowSignature {
    pub(crate) fn try_new(/* exact fields above */)
        -> Result<Self, FlowAttachmentError>;
    pub(crate) fn generics(&self) -> Option<&AttachedFlowGenericGroup>;
    pub(crate) fn parameters(&self) -> Option<&AttachedFlowParameterGroup>;
    pub(crate) fn result(&self) -> &AttachedFlowReturnSyntax;
    pub(crate) fn where_clause(&self) -> Option<&AttachedFlowWhereClause>;
    pub(crate) fn recovery(&self) -> &[AttachedFlowSignatureRecovery];
    pub(crate) fn end(&self) -> &AttachedInsertionSite;
}
```

Rules:

- zero or one fixed parameter group is admitted;
- a second group is retained only in `AttachedFlowSignatureRecovery` and
  creates no parameter locals;
- parameter defaults have no Flow grammar and recover as unexpected header
  nodes;
- every admitted parameter has a pattern and type node; a missing type is the
  real attached `MissingType` child;
- `Omitted` return has no type node;
- `->` with no valid type has `Authored { ty: MissingType }`;
- malformed `where` retains typed/error predicate children;
- `end` is the checked zero-width insertion immediately before the first
  contract clause or body opener after the final signature/where component.
  It is the source origin used by the synthetic postcondition result local.

## Contract owner

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum AttachedContractMode {
    Default,
    Prove,
    Check,
    Debug,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AttachedContractCondition {
    mode: AttachedContractMode,
    mode_site: Option<AttachedTokenSite>,
    expression: ExprNode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AttachedContractList {
    open: Option<AstNode<OpenBraceKind>>,
    operands: Box<[ExprNode]>,
    close: Option<AttachedDelimiterState<CloseBraceKind>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AttachedFlowContractClause {
    Requires {
        syntax: AstNode<RequiresClauseKind>,
        keyword: AttachedTokenSite,
        condition: AttachedContractCondition,
    },
    Ensures {
        syntax: AstNode<EnsuresClauseKind>,
        keyword: AttachedTokenSite,
        condition: AttachedContractCondition,
    },
    Invariant {
        syntax: AstNode<InvariantClauseKind>,
        keyword: AttachedTokenSite,
        condition: AttachedContractCondition,
    },
    Assume {
        syntax: AstNode<AssumeClauseKind>,
        keyword: AttachedTokenSite,
        expression: ExprNode,
    },
    Reads {
        syntax: AstNode<ReadsClauseKind>,
        keyword: AttachedTokenSite,
        operands: AttachedContractList,
    },
    Effects {
        syntax: AstNode<EffectsClauseKind>,
        keyword: AttachedTokenSite,
        operands: AttachedContractList,
    },
    NoEffect {
        syntax: AstNode<NoEffectClauseKind>,
        keyword: AttachedTokenSite,
        expression: ExprNode,
    },
    Modifies {
        syntax: AstNode<ModifiesClauseKind>,
        keyword: AttachedTokenSite,
        operands: AttachedContractList,
    },
    Decreases {
        syntax: AstNode<DecreasesClauseKind>,
        keyword: AttachedTokenSite,
        expression: ExprNode,
    },
}
```

Each variant has a crate-private `try_new` constructor and the enum exposes:

```rust
impl AttachedFlowContractClause {
    pub(crate) fn syntax(&self) -> SyntaxNodeHandle;
    pub(crate) fn kind(&self) -> SyntaxKind;
    pub(crate) fn keyword(&self) -> &AttachedTokenSite;
    pub(crate) fn mode(&self) -> Option<AttachedContractMode>;
    pub(crate) fn operands(&self) -> AttachedContractOperands<'_>;
}

pub(crate) enum AttachedContractOperands<'a> {
    One(&'a ExprNode),
    Many(&'a [ExprNode]),
}
```

A missing scalar payload is an attached `MissingExpression` `ExprNode`.
An explicitly empty braced list has zero operands and is not missing. A
missing/unclosed list delimiter is represented by its delimiter state.
Interleaving order is the parent role `ContractClause(ordinal)` and is never
reconstructed by concatenating family-specific vectors.

## Statement-only Flow and Thread bodies

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AttachedRequiredFlowBody {
    Present(AttachedFlowStatementBody),
    Missing {
        node: AstNode<MissingBodyKind>,
        insertion: AttachedInsertionSite,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AttachedFlowStatementBody {
    syntax: AstNode<FlowBodyKind>,
    open: AstNode<OpenBraceKind>,
    items: Box<[AttachedThreadFlowItem]>,
    close: AttachedDelimiterState<CloseBraceKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AttachedThreadExpressionBody {
    owner: AstNode<ThreadExpressionKind>,
    body: AstNode<BlockKind>,
    open: AstNode<OpenBraceKind>,
    items: Box<[AttachedThreadFlowItem]>,
    close: AttachedDelimiterState<CloseBraceKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AttachedThreadFlowItem {
    Statement(StatementNode),
    DialogueApplication(AstNode<DialogueContentApplicationExpressionKind>),
    Choice(AstNode<ChoiceStatementKind>),
    If(AstNode<IfStatementKind>),
    IfLet(AstNode<IfStatementKind>),
    Match(AstNode<MatchStatementKind>),
    Loop(AstNode<LoopStatementKind>),
    While(AstNode<WhileStatementKind>),
    WhileLet(AstNode<WhileLetStatementKind>),
    For(AstNode<ForStatementKind>),
    Select(AstNode<SelectStatementKind>),
    SourceLocale(AstNode<SourceLocaleStatementKind>),
    Scope(AstNode<ScopeStatementKind>),
    Include(AstNode<IncludeStatementKind>),
    AwaitWith(AstNode<AwaitWithStatementKind>),
    Error(AstNode<ErrorStatementKind>),
}
```

`IfLet` is distinguished from ordinary `If` by the existing typed
`IfStatementHeadNode`, not by text. The checked constructor verifies the head
kind before selecting the enum variant.

Both body records expose exactly:

```rust
pub(crate) fn open(&self) -> &AstNode<OpenBraceKind>;
pub(crate) fn items(&self) -> &[AttachedThreadFlowItem];
pub(crate) fn close(&self) -> &AttachedDelimiterState<CloseBraceKind>;
```

There is deliberately no `tail()`, `value()`, or `Option<ExprNode>` accessor.
A terminal ordinary expression is an `ExpressionStatement` in
`AttachedThreadFlowItem::Statement`.

`AttachedThreadFlowItem` exposes:

```rust
pub(crate) fn syntax(&self) -> SyntaxNodeHandle;
pub(crate) fn kind(&self) -> SyntaxKind;
pub(crate) fn family(&self) -> AttachedThreadFlowItemFamily;
```

where the family enum has the same sixteen discriminants. Both Flow and
ThreadExpression body constructors admit all sixteen rows. Context-specific
grammar validation is represented by the body constructor type, not a string
flag.

## Attachment error and validation order

```rust
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub(crate) enum FlowAttachmentError {
    #[error(transparent)]
    Access(#[from] SyntaxAccessError),
    #[error("Flow child belongs to a different immutable syntax snapshot")]
    ForeignSnapshot,
    #[error("Flow child role is not admitted for its parent")]
    WrongRole,
    #[error("Flow child ordinal is discontinuous")]
    OrdinalGap,
    #[error("Flow children are not in source order")]
    SourceOrder,
    #[error("Flow child lies outside its parent")]
    Containment,
    #[error("Flow identity children form no admitted state")]
    InvalidIdentityState,
    #[error("Flow has more than one admitted parameter group")]
    MultipleParameterGroups,
    #[error("Flow body exposes an ordinary value tail")]
    ValueTail,
    #[error("the same syntax identity is used for two Flow children")]
    DuplicateSyntaxIdentity,
}
```

Validation order is:

1. snapshot identity;
2. exact parent kind;
3. role class and ordinal continuity;
4. source containment/order;
5. duplicate identity;
6. identity state;
7. signature cardinality;
8. clause shape;
9. statement-only body shape;
10. trailing recovery separation.

User-authored malformed syntax is represented by attached recovery nodes and
diagnostics. `FlowAttachmentError` represents an impossible parser/attachment
invariant and aborts the syntax transaction; it is not converted into a user
diagnostic.
