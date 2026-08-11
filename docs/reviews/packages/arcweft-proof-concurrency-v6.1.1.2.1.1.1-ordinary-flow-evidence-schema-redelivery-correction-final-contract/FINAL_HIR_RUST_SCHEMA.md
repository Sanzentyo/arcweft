# Exact final-HIR Rust schema

Owning crate: `arcweft-lang-hir`

Owning modules:

```text
item
flow
contract
thread_body
scope
local
source
transaction
```

The schema below is the final public HIR shape. It replaces the provisional
`model.rs` Flow/Thread clone tree directly. All semantic records contain typed
IDs or owned semantic values. Source spans, insertions, delimiters, and recovery
sites live only in the revision-bound source index.

## Item payload

The original final item enum gains this exact payload arm:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirItemKind {
    // Other accepted final item variants.
    Flow(HirFlowItem),
}
```

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirFlowItem {
    identity: HirFlowIdentity,
    generics: Box<[HirFlowGenericParameter]>,
    parameters: Box<[HirFlowParameter]>,
    result: HirFlowReturn,
    where_predicates: Box<[HirFlowWherePredicate]>,
    callable_scope: ScopeId,
    requires_scope: ScopeId,
    ensures_scope: ScopeId,
    result_local: Option<HirFlowResultLocal>,
    contracts: Box<[HirFlowContractClause]>,
    body: HirThreadBody,
    poison: HirFlowPoison,
}

impl HirFlowItem {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_new(
        owner: ItemId,
        identity: HirFlowIdentity,
        generics: Box<[HirFlowGenericParameter]>,
        parameters: Box<[HirFlowParameter]>,
        result: HirFlowReturn,
        where_predicates: Box<[HirFlowWherePredicate]>,
        callable_scope: ScopeId,
        requires_scope: ScopeId,
        ensures_scope: ScopeId,
        result_local: Option<HirFlowResultLocal>,
        contracts: Box<[HirFlowContractClause]>,
        body: HirThreadBody,
        poison: HirFlowPoison,
    ) -> Result<Self, HirFlowInvariantError>;

    pub fn identity(&self) -> &HirFlowIdentity;
    pub fn generics(&self) -> &[HirFlowGenericParameter];
    pub fn parameters(&self) -> &[HirFlowParameter];
    pub const fn result(&self) -> &HirFlowReturn;
    pub fn where_predicates(&self) -> &[HirFlowWherePredicate];
    pub const fn callable_scope(&self) -> ScopeId;
    pub const fn requires_scope(&self) -> ScopeId;
    pub const fn ensures_scope(&self) -> ScopeId;
    pub const fn result_local(&self) -> Option<&HirFlowResultLocal>;
    pub fn contracts(&self) -> &[HirFlowContractClause];
    pub const fn body(&self) -> &HirThreadBody;
    pub const fn poison(&self) -> &HirFlowPoison;
}
```

The constructor checks that every contained ID belongs to the owner module,
the four scopes have the exact parents/kinds below, contract and body children
are source-ordered and unique, parameter locals are in the callable scope, and
`result_local` is `Some` exactly when at least one condition-form `Ensures`
clause exists. Its LocalId, scope and type must match that rule.

## Identity and public publication

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFlowIdentity {
    Name {
        name: HirName,
    },
    PublicId {
        public_id: HirIdRef,
    },
    PublicIdAndName {
        public_id: HirIdRef,
        name: HirName,
    },
    Missing,
}
```

The qualified `ItemId` is the internal semantic callable identity for every
recognized Flow, including poisoned Flow items. `HirFlowIdentity` controls
optional public/project publication and presentation:

- `Name`: the accepted project transaction derives the module-scoped Flow
  public identity once from the maintained declaration rule. The HIR does not
  store a fabricated ID.
- `PublicId`: the authored ID is publication identity; there is no presentation
  name.
- `PublicIdAndName`: the ID is publication identity and the name is
  presentation/local lookup. A mismatch poisons the item and publishes no
  callable candidate.
- `Missing`: the item remains typed recovery but publishes no callable
  candidate.

The project layer retains the resulting exact `Arc<CallableRecord>` and
`CheckedCallableId`; no copied Flow symbol or string key is authorized.

## Generics, parameters, return, and `where`

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFlowGenericParameter {
    Lifetime {
        name: HirName,
    },
    Type {
        name: HirName,
        bounds: Box<[TypeId]>,
    },
    Error {
        issue: HirRecoveryIssue,
    },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirFlowParameter {
    pattern: PatternId,
    ty: TypeId,
    locals: Box<[LocalId]>,
}

impl HirFlowParameter {
    pub(crate) fn try_new(
        owner: ItemId,
        callable_scope: ScopeId,
        pattern: PatternId,
        ty: TypeId,
        locals: Box<[LocalId]>,
    ) -> Result<Self, HirFlowInvariantError>;

    pub const fn pattern(&self) -> PatternId;
    pub const fn ty(&self) -> TypeId;
    pub fn locals(&self) -> &[LocalId];
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFlowReturn {
    OmittedUnit,
    Authored(TypeId),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirFlowWherePredicate {
    subject: TypeId,
    bounds: Box<[TypeId]>,
}

impl HirFlowWherePredicate {
    pub(crate) fn try_new(
        owner: ItemId,
        subject: TypeId,
        bounds: Box<[TypeId]>,
    ) -> Result<Self, HirFlowInvariantError>;

    pub const fn subject(&self) -> TypeId;
    pub fn bounds(&self) -> &[TypeId];
}
```

`OmittedUnit` is the semantic Unit result. It allocates no `TypeId`, no
synthetic type, no source row, and no placeholder type name. `->` followed by
a missing or malformed type is `Authored(error_type_id)` and remains distinct.

A second parameter group is attached recovery only. It creates no
`HirFlowParameter`, pattern, type, or local. Parameter defaults are not part of
the Flow grammar.

## Scope and local schema

The original scope-kind enum is extended in place:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirScopeKind {
    // Existing kinds.
    FlowCallable,
    ContractRequires,
    ContractEnsures,
    FlowBody,
    ThreadBody,
}
```

A recognized Flow stages exactly four scopes:

```text
module root
└── FlowCallable             source-backed by the Flow item
    ├── ContractRequires     SyntheticRole::ContractRequiresScope, ordinal 0
    ├── ContractEnsures      SyntheticRole::ContractEnsuresScope, ordinal 0
    └── FlowBody             source-backed by the FlowBody node
```

The requires, ensures, and body scopes are siblings. Contract scopes do not
nest, so a binding introduced while checking one clause cannot leak to another
phase or the body.

The original local record's type vocabulary is extended in its owning module:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLocalType {
    Authored(TypeId),
    SemanticUnit,
    Inferred,
    Poisoned,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLocalOrigin {
    Parameter,
    PatternBinding,
    PostconditionResult,
    BodyBinding,
    Recovery,
}
```

Parameter patterns allocate their locals in pattern preorder in
`FlowCallable`. The local allocator uses the existing module-wide
`LocalGeneration` for each normalized spelling. Allocation order is:

1. generic semantic bindings owned by the callable record;
2. parameter locals, parameter source order then pattern preorder;
3. the synthetic postcondition result local, if required;
4. body and nested-scope locals in body source order.

`result` is reserved in Flow parameters. A parameter binding with that spelling
is poisoned before local publication.

The result local is:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirFlowResultLocal {
    local: LocalId,
    ty: HirLocalType,
}

impl HirFlowResultLocal {
    pub(crate) fn try_new(
        owner: ItemId,
        ensures_scope: ScopeId,
        local: LocalId,
        result: &HirFlowReturn,
    ) -> Result<Self, HirFlowInvariantError>;

    pub const fn local(&self) -> LocalId;
    pub const fn ty(&self) -> &HirLocalType;
}
```

It exists if and only if at least one `HirFlowContractClause::Ensures` exists.
It uses:

```text
SyntheticOwner::Scope(ensures_scope)
SyntheticRole::PostconditionResult
ordinal 0
```

The local's generation is the next accepted module-wide generation for the
reserved spelling `result`, allocated in the deterministic order above. Its
source origin is the Flow signature-end insertion site. Its type is
`SemanticUnit` for `OmittedUnit`, `Authored(id)` for an authored return, and
retains poison through an error `TypeId`. It is visible only while checking
`Ensures`; it is not visible to `Requires`, `Invariant`, `Assume`, `Reads`,
`Effects`, `NoEffect`, `Modifies`, `Decreases`, or the body.

## Contract schema

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirContractMode {
    Default,
    Prove,
    CheckRuntime,
    DebugCheck,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirContractCondition {
    mode: HirContractMode,
    expression: ExprId,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirContractOperandList {
    operands: Box<[ExprId]>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFlowContractClause {
    Requires(HirContractCondition),
    Ensures(HirContractCondition),
    Invariant(HirContractCondition),
    Assume {
        expression: ExprId,
    },
    Reads(HirContractOperandList),
    Effects(HirContractOperandList),
    NoEffect {
        expression: ExprId,
    },
    Modifies(HirContractOperandList),
    Decreases {
        expression: ExprId,
    },
}
```

Every clause record is stored once in the one heterogeneous source-ordered
slice. A missing scalar operand is a real `ExprId` whose expression kind is
`MissingExpression`; an explicitly empty braced list has zero operands.

Scope assignment:

- `Ensures` expressions use `ContractEnsures`.
- Every other family uses `ContractRequires`.
- `Invariant` is stored once in the requires-phase scope. The checker applies
  it at both entry and successful exit; the HIR does not duplicate it.
- `NoEffect` is an effect prohibition, not an `Ensures` condition and cannot
  see `result`.

Semantic authorities:

- condition, assume, place, and decreases expressions use the final expression
  arena and accepted nominal/callable resolver;
- `Reads` and `Modifies` retain expression IDs; sema validates and canonicalizes
  places in the existing place/effect fact owner;
- `Effects` and `NoEffect` retain selector expression IDs; sema resolves them
  through the sole `EffectCatalog` to accepted `EffectId` values;
- proof/invariant references inside expressions resolve through the accepted
  project and checked callable/proof authorities;
- no Flow-only resolver, effect set, proof catalog, or copied semantic row is
  created.

Duplicate behavior is defined in `CONTRACT_CLAUSE_MATRIX.tsv`.

## Shared statement-only body

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirThreadBody {
    scope: ScopeId,
    items: Box<[HirThreadFlowItem]>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirThreadFlowItem {
    Statement(StmtId),
    DialogueApplication(ExprId),
    Choice(StmtId),
    If(StmtId),
    IfLet(StmtId),
    Match(StmtId),
    Loop(StmtId),
    While(StmtId),
    WhileLet(StmtId),
    For(StmtId),
    Select(StmtId),
    SourceLocale(StmtId),
    Scope(StmtId),
    Include(StmtId),
    AwaitWith(StmtId),
    Error(StmtId),
}

impl HirThreadBody {
    pub(crate) fn try_new(
        owner: HirThreadBodyOwner,
        scope: ScopeId,
        items: Box<[HirThreadFlowItem]>,
        source_manifest: &[HirThreadFlowItemSource],
    ) -> Result<Self, HirFlowInvariantError>;

    pub const fn scope(&self) -> ScopeId;
    pub fn items(&self) -> &[HirThreadFlowItem];
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirThreadBodyOwner {
    Flow(ItemId),
    ThreadExpression(ExprId),
    NestedScope(ScopeId),
}
```

There is no tail field. Empty Flow and Thread bodies have an empty item slice
and complete semantically as Unit. Unit is a checker/runtime result, not an
extra HIR expression or type node.

All sixteen rows are admitted under the two top-level owners. Nested bodies
inside control statements use `NestedScope(scope)`, where the contained
`HirThreadBody::scope` is exactly that unique source-ordered branch/body scope.
This prevents then/else, match-arm, Select-branch, and AwaitWith-branch bodies
from colliding under one statement ID without inventing a body ID.

`LetChoice`, `LetScope`,
`LetLoop`, `LetAwait`, and a nested `thread` statement remain
`Statement(StmtId)` because the outer authored family is a statement. They are
not recategorized as standalone body variants.

The exact statement payloads are the final `HirStmtKind` owners:
`Choice`, `If`, `IfLet`, `Match`, `Loop`, `While`, `WhileLet`, `For`, `Select`,
`SourceLocale`, `Scope`, `Include`, and `AwaitWith`. Nested bodies use new `HirThreadBody` records with
`HirThreadBodyOwner::NestedScope(the_body_scope)`. The scope parent is the
owning statement or expression scope defined in `FLOW_THREAD_ITEM_MATRIX.tsv`.

## Source manifest and sole query

The original query enum and role enums are extended in place. There is no
Flow-specific source map or convenience reader.

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirItemSourceRole {
    Whole,
    Keyword,
    Visibility,
    PublicId,
    Name,
    GenericGroup,
    GenericParameter { ordinal: u16 },
    ParameterGroup,
    Parameter {
        ordinal: u16,
        part: HirFlowParameterSourcePart,
    },
    Return {
        part: HirFlowReturnSourcePart,
    },
    WhereClause,
    WherePredicate { ordinal: u16 },
    ContractClause {
        ordinal: u16,
        part: HirFlowContractSourcePart,
    },
    Body,
    BodyOpen,
    BodyClose,
    TrailingRecovery { ordinal: u32 },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFlowParameterSourcePart {
    Whole,
    Pattern,
    Colon,
    Type,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFlowReturnSourcePart {
    Whole,
    Arrow,
    Type,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFlowContractSourcePart {
    Whole,
    Keyword,
    Mode,
    Operand { ordinal: u16 },
    OpenDelimiter,
    CloseDelimiter,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirScopeSourceRole {
    Whole,
    OpenDelimiter,
    CloseDelimiter,
    SyntheticOrigin,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLocalSourceRole {
    Whole,
    Name,
    Type,
    Pattern,
    SyntheticOrigin,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirThreadBodySourceRole {
    Whole,
    OpenDelimiter,
    CloseDelimiter,
    Item { ordinal: u32, part: HirThreadFlowItemSourcePart },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirThreadFlowItemSourcePart {
    Whole,
    ChildWhole,
}
```

Child-specific components remain owned by the child `StmtId` or `ExprId` and
its existing source-role enum. `ChildWhole` is a relation/freeze check to that
child's whole site; it is not a copied range.

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourceQuery {
    Item {
        id: ItemId,
        role: HirItemSourceRole,
    },
    Scope {
        id: ScopeId,
        role: HirScopeSourceRole,
    },
    Local {
        id: LocalId,
        role: HirLocalSourceRole,
    },
    ThreadBody {
        owner: HirThreadBodyOwner,
        role: HirThreadBodySourceRole,
    },
    Stmt {
        id: StmtId,
        role: HirStmtSourceRole,
    },
    Expr {
        id: ExprId,
        role: HirExprSourceRole,
    },
    Pattern {
        id: PatternId,
        role: HirPatternSourceRole,
    },
    Type {
        id: TypeId,
        role: HirTypeSourceRole,
    },
}
```

The sole public API remains:

```rust
impl HirModule {
    pub fn source_site(
        &self,
        expected_source: &SourceDocumentIdentity,
        query: HirSourceQuery,
    ) -> Result<HirSourceSite, HirSourceQueryError>;
}
```

No `flow_source_site`, `thread_body_range`, raw range accessor, source text
fallback, or overload is authorized.

Query validation order is exact:

1. typed owner module/database and wrapper kind;
2. `WrongModule`;
3. `NotYetLive`;
4. `Retired`;
5. `KindMismatch`;
6. role applicability;
7. role ordinal bounds;
8. expected source document identity;
9. expected source revision;
10. committed source-site presence.

Thus a bad ordinal precedes a wrong expected document. A rolled-back
reservation has no public ID and no query surface.

## Flow poison

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirFlowPoison {
    primary: Option<HirFlowIssue>,
    related: Box<[HirFlowIssue]>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirFlowIssue {
    class: HirFlowIssueClass,
    owner: HirFlowIssueOwner,
    source: HirSourceQuery,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFlowIssueClass {
    Prefix,
    Identity,
    Signature,
    Contract,
    MissingBody,
    BodyChild,
    UnclosedBody,
    TrailingRecovery,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFlowIssueOwner {
    Item(ItemId),
    Scope(ScopeId),
    Local(LocalId),
    Stmt(StmtId),
    Expr(ExprId),
    Pattern(PatternId),
    Type(TypeId),
}
```

`POISON_PRECEDENCE.md` defines canonical selection. Child terminal diagnostics
remain on child owners; the Flow stores a roleful recovered-child issue and
does not copy the child diagnostic.

## Constructor failure

```rust
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum HirFlowInvariantError {
    #[error("Flow child belongs to HIR module {actual:?}, expected {expected:?}")]
    WrongModule {
        expected: HirModuleId,
        actual: HirModuleId,
    },
    #[error("Flow scope has the wrong owner, kind, or parent")]
    WrongScopeGraph,
    #[error("Flow parameter local is outside the callable scope")]
    WrongParameterLocal,
    #[error("Flow result-local presence or type disagrees with its ensures clauses")]
    WrongResultLocal,
    #[error("Flow contract children are not in attached source order")]
    ContractOrder,
    #[error("Flow body children are not in attached source order")]
    BodyOrder,
    #[error("Flow body child identity is duplicated")]
    DuplicateBodyChild,
    #[error("Flow body child kind does not match its enum discriminant")]
    BodyChildKind,
    #[error("Flow source manifest disagrees with attached syntax")]
    SourceFreeze,
    #[error("Flow body contains an ordinary value tail")]
    ValueTail,
}
```

These are internal transaction failures. User syntax recovery is committed as
typed HIR plus `HirFlowPoison`; it is not converted to an invariant error.

## Deterministic allocation and transaction

The module lowering transaction performs a read-only preflight over the
attached snapshot, then reserves in this order:

1. Flow `ItemId`;
2. callable, requires, ensures, and body `ScopeId`s;
3. generic/type/pattern children in attached source order;
4. parameter locals in parameter order and pattern preorder;
5. result local when required;
6. contract expressions in heterogeneous clause/operand source order;
7. body statement/expression roots in body source order;
8. nested children according to each accepted lowering contract;
9. source rows in query-key order derived from attached roles;
10. diagnostics, project candidate, checked/cached facts, and invalidation fact.

Raw arena slot order is not exposed as source order. The transaction publishes
all staged maps and facts in one commit. Failure, cancellation, panic,
stale/foreign input, liveness failure, one-over limit, or source-freeze failure
drops the staging state and publishes nothing.
