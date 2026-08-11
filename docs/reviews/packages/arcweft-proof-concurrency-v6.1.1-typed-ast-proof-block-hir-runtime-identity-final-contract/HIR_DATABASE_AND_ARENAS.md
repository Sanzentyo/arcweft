# `HirDatabase`, immutable snapshots, typed arenas, and lowering transaction

## 1. Owning modules

```text
crates/arcweft-lang-hir/src/
  database.rs                public HirDatabase facade and module states
  module.rs                  immutable HirModule/HirModuleStatus
  identity.rs                database/module/snapshot/typed ID vocabulary
  slot.rs                    slot origin, liveness, metadata, limits
  arena.rs                   private paged typed arenas
  resolve.rs                 public immutable typed resolution
  item.rs                    HirItem and per-item records
  expr.rs                    HirExpr/HirExprKind
  stmt.rs                    HirStmt/HirStmtKind
  type_ref.rs                HirType/HirTypeKind
  pattern.rs                 HirPattern/HirPatternKind
  scope.rs                   HirScope/HirLocal/HirCapture data
  lower.rs                   lowering facade below 250 LOC
  lower/
    transaction.rs           staging, failure construction, commit
    items.rs                 item dispatch/direct syntax lowering
    expressions.rs           expression lowering
    statements.rs            statement lowering
    types.rs                 type lowering
    patterns.rs              pattern lowering
    scopes.rs                lexical scope stack
    locals.rs                binding/local generations
    captures.rs              closure capture inventory
    diagnostics.rs           ordering and exact deduplication
```

`model.rs`, clone-oriented `lower.rs`, and append/linked helpers are deleted after callers migrate. No new arena/database code is appended to them.

## 2. Database, module, and snapshot identity

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDatabaseId(NonZeroU64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirModuleId {
    database: HirDatabaseId,
    slot: NonZeroU32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRevision(NonZeroU32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirSnapshotId {
    module: HirModuleId,
    revision: HirRevision,
}
```

Fields and raw constructors are private. No session identity implements Serde. `HirDatabaseId` uses a process-local nonzero atomic allocator and never wraps. `HirModuleId` is staged from the next module slot and consumed only by a successful first commit for a module key. Revisions begin at one and advance exactly once per successful byte-changing/source-changing lowering commit. Failed or no-op lowering consumes no revision.

The database component in `HirModuleId` makes cross-database IDs unequal and causes typed resolution to fail before inspecting a raw slot.

## 3. Module key and lowering request

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirModuleKey {
    package: CallablePackageId,
    path: CanonicalModulePath,
    document: SourceDocumentId,
}

impl HirModuleKey {
    pub fn new(
        package: CallablePackageId,
        path: CanonicalModulePath,
        document: SourceDocumentId,
    ) -> Self;

    pub fn package(&self) -> &CallablePackageId;
    pub fn path(&self) -> &CanonicalModulePath;
    pub fn document(&self) -> &SourceDocumentId;
}

pub struct LoweringRequest<'a> {
    key: HirModuleKey,
    source: &'a ParsedSource,
}

impl<'a> LoweringRequest<'a> {
    pub fn try_new(
        key: HirModuleKey,
        source: &'a ParsedSource,
    ) -> Result<Self, HirLowerFailure>;
    pub fn key(&self) -> &HirModuleKey;
    pub fn source(&self) -> &'a ParsedSource;
}
```

The constructor verifies that the exact `SourceDocumentId` in the parsed source equals the module key, that the source has a bound `SourceFile` root, and that the syntax snapshot belongs to the accepted source document. Package/path checking against project ownership occurs again when the module enters a project.

A detached fragment cannot satisfy the `ParsedSource` parameter at compile time.

## 4. Public database API

```rust
pub struct HirDatabase {
    id: HirDatabaseId,
    modules: BTreeMap<HirModuleKey, ModuleState>,
    next_module_slot: NonZeroU32,
    limits: HirLimits,
}

impl HirDatabase {
    pub fn try_new() -> Result<Self, HirDatabaseCreateError>;

    pub fn lower(
        &mut self,
        request: LoweringRequest<'_>,
    ) -> Result<HirLowerOutput, HirLowerFailure>;

    pub fn current(
        &self,
        key: &HirModuleKey,
    ) -> Option<Arc<HirModule>>;

    pub fn snapshot(
        &self,
        id: HirSnapshotId,
    ) -> Result<Arc<HirModule>, HirSnapshotLookupError>;

    pub fn database_id(&self) -> HirDatabaseId;
}

#[derive(Clone)]
pub struct HirLowerOutput {
    module: Arc<HirModule>,
    invalidations: HirInvalidationSet,
}

impl HirLowerOutput {
    pub fn module(&self) -> &Arc<HirModule>;
    pub fn into_module(self) -> Arc<HirModule>;
    pub fn invalidations(&self) -> &HirInvalidationSet;
    pub fn into_parts(self) -> (Arc<HirModule>, HirInvalidationSet);
}
```

`with_test_limits`, allocator seeding, and corruption hooks are `#[cfg(test)] pub(crate)` only. No public API accepts raw module/revision/slot values. Returning `HirLowerOutput` is the only invalidation publication path; the database has no hidden drain queue or callback side channel.

## 5. Immutable module snapshot

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirModuleStatus {
    Clean,
    Recovered,
}

pub struct HirModule {
    snapshot: HirSnapshotId,
    syntax_snapshot: SyntaxSnapshotId,
    source_snapshot: SourceSnapshotId,
    source_identity: SourceDocumentIdentity,
    document: Arc<SourceDocument>,
    key: HirModuleKey,
    status: HirModuleStatus,
    diagnostics: Arc<[HirDiagnostic]>,
    slots: Arc<SlotSnapshot>,
    items: ArenaSnapshot<HirItem>,
    scopes: ArenaSnapshot<HirScope>,
    locals: ArenaSnapshot<HirLocal>,
    expressions: ArenaSnapshot<HirExpr>,
    statements: ArenaSnapshot<HirStmt>,
    types: ArenaSnapshot<HirType>,
    patterns: ArenaSnapshot<HirPattern>,
    captures: ArenaSnapshot<HirCapture>,
    source_index: Arc<SourceAllocationIndex>,
    synthetic_index: Arc<SyntheticAllocationIndex>,
    invalidation_epoch: NonZeroU64,
}
```

`HirModule` itself is not an inner `Arc` wrapper. `HirDatabase`, project views, and snapshots share it as one `Arc<HirModule>`, avoiding double reference-counting while its arena pages and indexes remain structurally shared.

Public accessors:

```rust
impl HirModule {
    pub fn snapshot_id(&self) -> HirSnapshotId;
    pub fn syntax_snapshot_id(&self) -> &SyntaxSnapshotId;
    pub fn source_snapshot_id(&self) -> &SourceSnapshotId;
    pub fn source_identity(&self) -> &SourceDocumentIdentity;
    pub fn document(&self) -> &SourceDocument;
    pub fn key(&self) -> &HirModuleKey;
    pub fn status(&self) -> HirModuleStatus;
    pub fn is_executable(&self) -> bool;
    pub fn is_cache_eligible(&self) -> bool;
    pub fn diagnostics(&self) -> &[HirDiagnostic];
    pub fn invalidation_epoch(&self) -> NonZeroU64;
}
```

`is_executable` and `is_cache_eligible` are true only for `Clean`. A recovered module remains fully queryable and may enter tooling/project navigation, but executable sema/codegen/runtime entrypoints reject it before cache lookup or insertion.

The exact `SourceDocument` is retained so every `SourceSpan` remains revision-bound. `SourceDocumentIdentity` is not reconstructed from display names or hashes.

## 6. Typed IDs and raw representation

The existing public typed vocabulary remains and gains no Serde implementation:

```rust
pub struct ItemId(RawHirId);
pub struct ScopeId(RawHirId);
pub struct LocalId(RawHirId);
pub struct ExprId(RawHirId);
pub struct StmtId(RawHirId);
pub struct TypeId(RawHirId);
pub struct PatternId(RawHirId);
pub struct CaptureId(RawHirId);
```

```rust
struct RawHirId {
    module: HirModuleId,
    slot: NonZeroU32,
    kind: HirIdKind,
}
```

`RawHirId` and every typed constructor stay private to `identity.rs`. Public access is limited to `module()`, `kind()`, and `Debug`; the numeric slot is crate-private. `TryFrom<u32>`, `From<NonZeroU32>`, `Serialize`, `Deserialize`, string parsing, and textual codecs do not exist.

`HirIdKind` owns inherent typed conversion and arena selection behavior. No local trait or free helper is added to compensate for missing variants.

## 7. Allocation origins and slot metadata

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceKey {
    syntax: SyntaxNodeId,
    kind: HirIdKind,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirOrigin {
    Source(SourceKey),
    Synthetic(SyntheticKey),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntheticKey {
    owner: SyntheticOwner,
    role: SyntheticRole,
    ordinal: u32,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntheticOwner {
    Item(ItemId),
    Scope(ScopeId),
    Local(LocalId),
    Expr(ExprId),
    Stmt(StmtId),
    Type(TypeId),
    Pattern(PatternId),
    Syntax(SyntaxNodeId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntheticRole {
    ImplicitUnitTail,
    PredicateBoolReturn,
    ProofUnitReturn,
    ElidedRegion,
    RecoveryOperand,
    PostconditionResult,
    DesugaredTemporary,
    MissingRequiredTail,
    DestructuredBinding,
    ClosureEnvironment,
    ClosureCapture,
    ContractRequiresScope,
    ContractEnsuresScope,
    ForIterator,
    ForNextValue,
    IfLetScrutinee,
    WhileLetScrutinee,
    MatchScrutinee,
    PatternRest,
}
```

These variants are added to the existing repository-owned enum and its inherent implementations. No string role tags survive.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSlotMetadata {
    kind: HirIdKind,
    born: HirRevision,
    retired_at: Option<HirRevision>,
    origin: HirOrigin,
    poisoned: bool,
    span: SourceSpan,
}
```

Every slot has a revision-bound span, including synthetic slots. Snapshot-specific metadata is immutable. Retained IDs may have a different span and poison state in a later snapshot while preserving their global born/retired interval.

The mutable database ledger stores kind, birth, optional retirement, and allocation key. Each immutable snapshot stores the span/poison view and a high-water mark. Old snapshots retain their own values and spans through shared arena pages.

Public immutable inspection is exact and does not expose raw constructors:

```rust
impl SourceKey {
    pub fn syntax(&self) -> SyntaxNodeId;
    pub fn kind(&self) -> HirIdKind;
}

impl SyntheticKey {
    pub fn owner(&self) -> &SyntheticOwner;
    pub fn role(&self) -> SyntheticRole;
    pub fn ordinal(&self) -> u32;
}

impl HirSlotMetadata {
    pub fn kind(&self) -> HirIdKind;
    pub fn born(&self) -> HirRevision;
    pub fn retired_at(&self) -> Option<HirRevision>;
    pub fn origin(&self) -> &HirOrigin;
    pub fn is_poisoned(&self) -> bool;
    pub fn span(&self) -> &SourceSpan;
}
```

## 8. Source-backed identity behavior

The allocation key for every source-backed HIR value is exactly `(SyntaxNodeId, HirIdKind)` within one `HirModuleId`.

- same reconciled syntax ID and same HIR kind: retain HIR ID;
- same syntax ID but changed HIR kind: retire old slot at the new revision and allocate a fresh slot;
- fresh syntax ID: fresh HIR slot;
- removed syntax: retire its source-backed slots at the new revision;
- trivia-only reparse: retain IDs and values where structurally equal, update snapshot spans;
- same-parent source reorder: follow retained syntax IDs;
- cross-parent move: fresh syntax IDs and therefore fresh source-backed HIR IDs;
- copy: one matched source node may retain; each additional copy receives fresh IDs;
- recovered source node: allocate/retain the correct typed kind with `poisoned = true` where a typed family is known;
- generic recovery node: lower only to the corresponding `Error` item/stmt/expr/type/pattern kind; never guess executable behavior.

A source-backed HIR value is never keyed by a source range, source string, line number, item ordinal, or display spelling.

## 9. Synthetic identity and anchors

Synthetic ordinals are zero-based and stable within an owner/role inventory. They are assigned by semantic source order, never hash-map iteration.

| Role | Owner | Ordinal | Zero-width anchor |
|---|---|---:|---|
| `ImplicitUnitTail` | proof block syntax node | 0 | omitted-tail insertion point |
| `PredicateBoolReturn` | predicate item syntax node | 0 | end of fixed parameter group |
| `ProofUnitReturn` | proof item syntax node | 0 | end of fixed parameter group |
| `ElidedRegion` | reference type ID | 0 | `&` token end |
| `RecoveryOperand` | poisoned parent expr/stmt | child role ordinal | missing-child insertion point |
| `PostconditionResult` | ensures contract scope | 0 | first `ensures` expression start; return-type end or fixed-parameter close only when ensures exist but all expressions are missing/recovered |
| `DesugaredTemporary` | lowering owner | deterministic lowering ordinal | source operator/token end that caused desugaring |
| `MissingRequiredTail` | predicate/proof block syntax node | 0 | omitted-tail insertion point |
| `DestructuredBinding` | source pattern ID | preorder binding ordinal | binding-name start |
| `ClosureEnvironment` | closure expression ID | 0 | closure introducer end |
| `ClosureCapture` | closure expression ID | first-use ordinal | first captured use start |
| `ContractRequiresScope` | callable item ID | 0 | first `requires` keyword start; end of fixed parameter group when every requires clause is missing/recovered |
| `ContractEnsuresScope` | callable item ID | 0 | first `ensures` keyword start; return-type end or fixed parameter close when every ensures clause is missing/recovered |
| `ForIterator` | for statement ID | 0 | `in` token end |
| `ForNextValue` | for statement ID | 0 | body opening brace start |
| `IfLetScrutinee` | if-let expr/stmt ID | 0 | `=` token end |
| `WhileLetScrutinee` | while-let stmt ID | 0 | `=` token end |
| `MatchScrutinee` | match expr/stmt ID | 0 | `match` keyword end |
| `PatternRest` | rest pattern ID | 0 | rest token end |

A synthetic key whose owner retires also retires in the same revision unless the identical source-backed owner and role survive. Synthetic values are never serialized.

## 10. Private paged arenas

```rust
struct ArenaSnapshot<T> {
    pages: Arc<[Arc<[ArenaEntry<T>]>]>,
    len: u32,
}

struct ArenaEntry<T> {
    slot: NonZeroU32,
    value: T,
}
```

Pages contain at most 256 entries and are copy-on-write at transaction commit. Arena storage and constructors are private. Public iteration yields typed IDs and immutable references:

```rust
impl HirModule {
    pub fn items(&self) -> impl ExactSizeIterator<Item = (ItemId, &HirItem)>;
    pub fn scopes(&self) -> impl ExactSizeIterator<Item = (ScopeId, &HirScope)>;
    pub fn locals(&self) -> impl ExactSizeIterator<Item = (LocalId, &HirLocal)>;
    pub fn expressions(&self) -> impl ExactSizeIterator<Item = (ExprId, &HirExpr)>;
    pub fn statements(&self) -> impl ExactSizeIterator<Item = (StmtId, &HirStmt)>;
    pub fn types(&self) -> impl ExactSizeIterator<Item = (TypeId, &HirType)>;
    pub fn patterns(&self) -> impl ExactSizeIterator<Item = (PatternId, &HirPattern)>;
    pub fn captures(&self) -> impl ExactSizeIterator<Item = (CaptureId, &HirCapture)>;
}
```

Iterators expose only values live in that immutable snapshot, in raw slot order. Tooling that needs source order sorts/uses explicit item/statement child arrays, not arena order.

## 11. Exact core HIR records

### 11.1 Items

```rust
pub struct HirItem {
    scope: ScopeId,
    kind: HirItemKind,
}

pub enum HirItemKind {
    ModuleDeclaration(HirModuleDeclaration),
    UseDeclaration(HirUseDeclaration),
    Flow(HirFlowItem),
    Function(HirFunctionItem),
    Predicate(HirPredicate),
    Proof(HirProof),
    Agent(HirAgentItem),
    Callable(HirCallableItem),
    State(HirStateItem),
    Trait(HirTraitItem),
    Impl(HirImplItem),
    Enum(HirEnumItem),
    Struct(HirStructItem),
    TypeAlias(HirTypeAliasItem),
    EntityDeclaration(HirEntityDeclarationItem),
    EntryDeclaration(HirEntryDeclarationItem),
    ExternCapability(HirExternCapabilityItem),
    ExternModule(HirExternModuleItem),
    Hook(HirHookItem),
    DialogueDefaults(HirDialogueDefaultsItem),
    MemoFunction(HirMemoFunctionItem),
    Test(HirTestItem),
    Bench(HirBenchItem),
    Parser(HirParserItem),
    Source(HirSourceItem),
    Style(HirStyleItem),
    TopLevelFlow(HirTopLevelFlowItem),
    Error(HirErrorItem),
}
```

There is no trusted-axiom item. Existing unrelated item payloads keep their language semantics but replace nested syntax clones/strings with arena IDs and HIR-owned names/literals.

```rust
pub struct HirPredicate {
    name: HirName,
    visibility: Option<Visibility>,
    generic_parameters: Box<[HirGenericParameter]>,
    parameters: Box<[HirParameter]>,
    where_predicates: Box<[HirWherePredicate]>,
    requires: Box<[ExprId]>,
    ensures: Box<[ExprId]>,
    return_type: TypeId,
    body: HirPredicateBody,
    callable_scope: ScopeId,
    requires_scope: ScopeId,
    ensures_scope: ScopeId,
}

pub enum HirPredicateBody {
    Expression(ExprId),
    Block {
        scope: ScopeId,
        statements: Box<[StmtId]>,
        tail: ExprId,
    },
    Error(ExprId),
}

pub struct HirProof {
    name: HirName,
    visibility: Option<Visibility>,
    generic_parameters: Box<[HirGenericParameter]>,
    parameters: Box<[HirParameter]>,
    where_predicates: Box<[HirWherePredicate]>,
    requires: Box<[ExprId]>,
    ensures: Box<[ExprId]>,
    return_type: TypeId,
    body: HirProofBody,
    callable_scope: ScopeId,
    requires_scope: ScopeId,
    ensures_scope: ScopeId,
}

pub enum HirProofBody {
    Expression(ExprId),
    Block {
        scope: ScopeId,
        statements: Box<[StmtId]>,
        tail: ExprId,
    },
    Error(ExprId),
}
```

### 11.2 Expressions

```rust
pub struct HirExpr {
    scope: ScopeId,
    kind: HirExprKind,
}

pub enum HirExprKind {
    Unit,
    Literal(HirLiteral),
    EntityReference(HirEntityReference),
    LifetimePath { key: HirLifetimeKey, optional: bool },
    Path(HirPath),
    ShortVariant(HirName),
    Placeholder,
    Tuple(Box<[ExprId]>),
    BracketSequence(Box<[ExprId]>),
    NumericBracketSequence(HirNumericSequence),
    ArrayRepeat { value: ExprId, length: ExprId },
    Call { callee: ExprId, arguments: Box<[HirCallArgument]> },
    Select { target: ExprId, member: HirName },
    DialogueCall { callee: ExprId, content: HirDialogueContent, plan: Option<ItemId> },
    Index { target: ExprId, index: ExprId },
    Pipe { left: ExprId, right: ExprId },
    Try { expression: ExprId },
    Await { expression: ExprId, applies_try: bool },
    Thread { scope: ScopeId, body: Box<[StmtId]> },
    Range { start: Option<ExprId>, end: Option<ExprId>, inclusive: bool },
    Record { path: HirPath, fields: Box<[HirRecordField]> },
    RecordLiteral(Box<[HirRecordField]>),
    Binary { left: ExprId, operator: BinaryOp, right: ExprId },
    Borrow { kind: BorrowKind, expression: ExprId },
    Dereference { expression: ExprId },
    Closure {
        scope: ScopeId,
        parameters: Box<[HirClosureParameter]>,
        return_type: Option<TypeId>,
        body: ExprId,
        captures: Box<[CaptureId]>,
    },
    Unary { operator: UnaryOp, expression: ExprId },
    Block { scope: ScopeId, statements: Box<[StmtId]>, tail: ExprId },
    ComputationBlock { kind: ComputationBlockKind, scope: ScopeId, statements: Box<[StmtId]>, tail: ExprId },
    MemoBlock { options: Box<[(HirName, ExprId)]>, scope: ScopeId, statements: Box<[StmtId]>, tail: ExprId },
    NamedBlock { name: HirName, scope: ScopeId, statements: Box<[StmtId]>, tail: ExprId },
    If { condition: ExprId, then_branch: ExprId, else_branch: Option<ExprId> },
    IfLet {
        scrutinee: ExprId,
        pattern: PatternId,
        guard: Option<ExprId>,
        then_branch: ExprId,
        else_branch: Option<ExprId>,
        scope: ScopeId,
    },
    Match { scrutinee: ExprId, arms: Box<[HirMatchArm]> },
    Error,
}
```

### 11.3 Statements

```rust
pub struct HirStmt {
    scope: ScopeId,
    kind: HirStmtKind,
}

pub enum HirStmtKind {
    Assertion { mode: AssertionMode, conditions: Box<[ExprId]> },
    Let { pattern: PatternId, annotation: Option<TypeId>, initializer: ExprId, locals: Box<[LocalId]> },
    Assign { target: ExprId, value: ExprId },
    LetElse { pattern: PatternId, annotation: Option<TypeId>, initializer: ExprId, else_scope: ScopeId, else_body: Box<[StmtId]>, locals: Box<[LocalId]> },
    LetChoice { pattern: PatternId, choice: ExprId, locals: Box<[LocalId]> },
    LetScope { pattern: PatternId, scope_expr: ExprId, locals: Box<[LocalId]> },
    LetLoop { pattern: PatternId, loop_expr: ExprId, locals: Box<[LocalId]> },
    LetAwait { pattern: PatternId, await_expr: ExprId, locals: Box<[LocalId]> },
    LetActionReceive { pattern: PatternId, action: ExprId, locals: Box<[LocalId]> },
    Return { value: ExprId },
    Out { label: Option<HirName>, value: ExprId },
    Goto { target: ExprId },
    Thread { scope: ScopeId, body: Box<[StmtId]> },
    DeferBlock { outcome: DeferOutcome, scope: ScopeId, body: Box<[StmtId]> },
    Defer { outcome: DeferOutcome, expression: ExprId },
    Yield { expression: ExprId },
    Signal { target: ExprId, value: ExprId },
    LifetimeSet { target: ExprId, value: ExprId },
    Wait { target: ExprId },
    On { trigger: HirTriggerPattern, scope: ScopeId, body: Box<[StmtId]> },
    UnsafeLifetime { audit: HirUnsafeAudit, scope: ScopeId, body: Box<[StmtId]> },
    If { condition: ExprId, then_scope: ScopeId, then_body: Box<[StmtId]>, else_scope: Option<ScopeId>, else_body: Box<[StmtId]> },
    Loop { scope: ScopeId, body: Box<[StmtId]> },
    While { condition: ExprId, scope: ScopeId, body: Box<[StmtId]> },
    WhileLet { scrutinee: ExprId, pattern: PatternId, guard: Option<ExprId>, scope: ScopeId, body: Box<[StmtId]>, locals: Box<[LocalId]> },
    For { source: ExprId, pattern: PatternId, scope: ScopeId, body: Box<[StmtId]>, locals: Box<[LocalId]> },
    Match { scrutinee: ExprId, arms: Box<[HirStmtMatchArm]> },
    Close { target: ExprId },
    Select { expression: ExprId },
    Break { label: Option<HirName>, value: Option<ExprId> },
    Continue { label: Option<HirName> },
    Expression { expression: ExprId },
    ProofCall { call: ExprId },
    Error,
}
```

Existing statement semantics remain. Source/display strings are deleted; typed child IDs are authoritative.

### 11.4 Types

```rust
pub struct HirType {
    kind: HirTypeKind,
}

pub enum HirTypeKind {
    Primitive(PrimitiveType),
    Path(HirPath),
    Apply { constructor: TypeId, arguments: Box<[TypeId]> },
    Tuple(Box<[TypeId]>),
    Reference { kind: BorrowKind, region: TypeId, referent: TypeId },
    Slice(TypeId),
    Array { element: TypeId, length: ExprId },
    Function { parameters: Box<[TypeId]>, result: TypeId },
    Sum(Box<[TypeId]>),
    Lifetime(HirLifetime),
    Infer,
    Error,
}
```

### 11.5 Patterns

```rust
pub struct HirPattern {
    kind: HirPatternKind,
}

pub enum HirPatternKind {
    Wildcard,
    Binding { name: HirName, mutable: bool, local: Option<LocalId> },
    Literal(HirLiteral),
    EntityReference(HirEntityReference),
    Tuple(Box<[PatternId]>),
    Record { path: Option<HirPath>, fields: Box<[HirPatternField]> },
    Variant { path: HirPath, fields: Box<[PatternId]> },
    Sequence(Box<[PatternId]>),
    Rest,
    WholeBinding { binding: PatternId, pattern: PatternId },
    Or(Box<[PatternId]>),
    Error,
}
```

### 11.6 Public immutable record access

All arena record fields remain private. The exact read API is:

```rust
impl HirItem {
    pub fn scope(&self) -> ScopeId;
    pub fn kind(&self) -> &HirItemKind;
}

impl HirPredicate {
    pub fn name(&self) -> &HirName;
    pub fn visibility(&self) -> Option<Visibility>;
    pub fn generic_parameters(&self) -> &[HirGenericParameter];
    pub fn parameters(&self) -> &[HirParameter];
    pub fn where_predicates(&self) -> &[HirWherePredicate];
    pub fn requires(&self) -> &[ExprId];
    pub fn ensures(&self) -> &[ExprId];
    pub fn return_type(&self) -> TypeId;
    pub fn body(&self) -> &HirPredicateBody;
    pub fn callable_scope(&self) -> ScopeId;
    pub fn requires_scope(&self) -> ScopeId;
    pub fn ensures_scope(&self) -> ScopeId;
}

impl HirProof {
    pub fn name(&self) -> &HirName;
    pub fn visibility(&self) -> Option<Visibility>;
    pub fn generic_parameters(&self) -> &[HirGenericParameter];
    pub fn parameters(&self) -> &[HirParameter];
    pub fn where_predicates(&self) -> &[HirWherePredicate];
    pub fn requires(&self) -> &[ExprId];
    pub fn ensures(&self) -> &[ExprId];
    pub fn return_type(&self) -> TypeId;
    pub fn body(&self) -> &HirProofBody;
    pub fn callable_scope(&self) -> ScopeId;
    pub fn requires_scope(&self) -> ScopeId;
    pub fn ensures_scope(&self) -> ScopeId;
}

impl HirExpr {
    pub fn scope(&self) -> ScopeId;
    pub fn kind(&self) -> &HirExprKind;
}

impl HirStmt {
    pub fn scope(&self) -> ScopeId;
    pub fn kind(&self) -> &HirStmtKind;
}

impl HirType {
    pub fn kind(&self) -> &HirTypeKind;
}

impl HirPattern {
    pub fn kind(&self) -> &HirPatternKind;
}
```

Constructors remain private to `lower::transaction`; downstream crates pattern-match the borrowed public enums returned by these accessors.

## 12. Resolution

```rust
impl HirModule {
    pub fn resolve_item(&self, id: ItemId) -> Result<&HirItem, IdResolveError>;
    pub fn resolve_scope(&self, id: ScopeId) -> Result<&HirScope, IdResolveError>;
    pub fn resolve_local(&self, id: LocalId) -> Result<&HirLocal, IdResolveError>;
    pub fn resolve_expr(&self, id: ExprId) -> Result<&HirExpr, IdResolveError>;
    pub fn resolve_stmt(&self, id: StmtId) -> Result<&HirStmt, IdResolveError>;
    pub fn resolve_type(&self, id: TypeId) -> Result<&HirType, IdResolveError>;
    pub fn resolve_pattern(&self, id: PatternId) -> Result<&HirPattern, IdResolveError>;
    pub fn resolve_capture(&self, id: CaptureId) -> Result<&HirCapture, IdResolveError>;
    pub fn metadata<I: HirTypedId>(&self, id: I) -> Result<&HirSlotMetadata, IdResolveError>;
    pub fn item_for_syntax(&self, id: SyntaxNodeId) -> Result<ItemId, HirSourceLookupError>;
    pub fn scope_for_syntax(&self, id: SyntaxNodeId) -> Result<ScopeId, HirSourceLookupError>;
    pub fn local_for_syntax(&self, id: SyntaxNodeId) -> Result<LocalId, HirSourceLookupError>;
    pub fn expr_for_syntax(&self, id: SyntaxNodeId) -> Result<ExprId, HirSourceLookupError>;
    pub fn stmt_for_syntax(&self, id: SyntaxNodeId) -> Result<StmtId, HirSourceLookupError>;
    pub fn type_for_syntax(&self, id: SyntaxNodeId) -> Result<TypeId, HirSourceLookupError>;
    pub fn pattern_for_syntax(&self, id: SyntaxNodeId) -> Result<PatternId, HirSourceLookupError>;
}
```

The sealed `HirTypedId` trait is implemented only inside `identity.rs` to share resolver plumbing. It is not an extension point.

`IdResolveError` retains the exact public variants:

```rust
pub enum IdResolveError {
    WrongModule { expected: HirModuleId, actual: HirModuleId },
    NotYetLive { id: RawHirIdView, snapshot: HirSnapshotId, born: HirRevision },
    Retired { id: RawHirIdView, snapshot: HirSnapshotId, retired_at: HirRevision },
    KindMismatch { id: RawHirIdView, expected: HirIdKind, actual: HirIdKind },
}
```

Resolution order is WrongModule, NotYetLive, Retired, KindMismatch, arena entry. The final entry lookup is checked and returns `KindMismatch`/an invariant error through the test corruption hook; it never indexes unchecked and never panics.

Old snapshots resolve an ID when `born <= snapshot.revision < retired_at`. A current snapshot returns `Retired` when `retired_at <= current.revision`. A later-born ID queried in an old snapshot returns `NotYetLive` even if its raw slot is within a shared page.

Poison is metadata, not a resolver error. Tooling can resolve poisoned values and inspect diagnostics.

Auxiliary lookup/create errors are closed as follows:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RawHirIdView {
    module: HirModuleId,
    kind: HirIdKind,
    slot: NonZeroU32,
}

impl RawHirIdView {
    pub fn module(&self) -> HirModuleId;
    pub fn kind(&self) -> HirIdKind;
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirDatabaseCreateError {
    #[error("HIR database identity allocation is exhausted")]
    IdentityExhausted,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirSnapshotLookupError {
    #[error("HIR snapshot belongs to another database")]
    WrongDatabase { expected: HirDatabaseId, actual: HirDatabaseId },
    #[error("HIR module is not present in this database")]
    UnknownModule { module: HirModuleId },
    #[error("HIR revision is not retained for this module")]
    UnknownRevision { module: HirModuleId, revision: HirRevision },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirSourceLookupError {
    #[error("syntax ID belongs to another syntax database")]
    WrongSyntaxDatabase { expected: SyntaxDatabaseId, actual: SyntaxDatabaseId },
    #[error("syntax ID belongs to another source lineage")]
    WrongSyntaxLineage { expected: SyntaxLineageId, actual: SyntaxLineageId },
    #[error("syntax node was not lowered as the requested HIR kind")]
    NotLowered { syntax: SyntaxNodeId, expected: HirIdKind },
    #[error("syntax node lowered as another HIR kind")]
    KindMismatch { syntax: SyntaxNodeId, expected: HirIdKind, actual: HirIdKind },
}
```

`RawHirIdView` is constructed only by resolver errors. Its numeric slot has no public accessor, parser, conversion, or Serde implementation.

## 13. Limits and exhaustion

```rust
pub struct HirLimits {
    pub modules_per_database: u32,          // 65,536
    pub items_per_module: u32,              // 16,384
    pub scopes_per_module: u32,             // 16,384
    pub statements_per_module: u32,         // 65,536
    pub expressions_per_module: u32,        // 262,144
    pub types_per_module: u32,              // 131,072
    pub patterns_per_module: u32,           // 131,072
    pub locals_per_module: u32,             // 65,536
    pub locals_per_scope: u32,              // 4,096
    pub captures_per_module: u32,           // 65,536
    pub diagnostics_per_module: u32,         // 1,024
    pub synthetic_descendants_per_owner: u32, // 1,024
    pub total_slots_per_module: u32,         // 786,432
}
```

Values shown are the production defaults and inclusive maxima. `HirRevision` and raw slot domains retain their full nonzero `u32` ranges; database identity uses nonzero `u64`.

Fatal exhaustion variants are:

```rust
pub enum HirLowerFailure {
    WrongSyntaxDatabase { expected: SyntaxDatabaseId, actual: SyntaxDatabaseId },
    WrongSyntaxLineage { expected: SyntaxLineageId, actual: SyntaxLineageId },
    StaleSource { current: SyntaxSnapshotId, supplied: SyntaxSnapshotId },
    SourceIdentityMismatch {
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
    SourceDocumentMismatch {
        expected: SourceDocumentId,
        actual: SourceDocumentId,
    },
    LimitExceeded { limit: HirLimit, maximum: u32, observed: u64 },
    ModuleIdentityExhausted,
    RevisionExhausted { module: HirModuleId },
    SlotIdentityExhausted { module: HirModuleId, kind: HirIdKind },
    LocalGenerationExhausted { scope: ScopeId, name: HirName },
    CacheEpochExhausted { module: HirModuleId },
    Invariant(HirInvariantFailure),
}
```

Only `lower::transaction` constructs these variants. Fatal failures are not converted into committed diagnostics.

## 14. No-op, stale, recovered, and cache behavior

- same `HirModuleKey`, exact same `ParsedSource` snapshot, and same lowering schema version: return `HirLowerOutput` containing the exact current `Arc<HirModule>` and `HirInvalidationSet::empty(current)`;
- same source identity but older syntax generation: `StaleSource`, no mutation;
- source from another syntax database/lineage: typed mismatch, no mutation;
- same module key with changed accepted source snapshot: stage next HIR revision;
- clean parsed source with recoverable HIR semantic/lowering diagnostics: commit `Recovered` snapshot;
- fatal source/limit/identity/invariant failure: no snapshot and no mutation;
- recovered snapshot: included in tooling project views, excluded from executable semantic, verifier-result, runtime-plan, codegen, and persistent compile caches;
- cache invalidations are returned only by successful commit and are keyed by module/snapshot, not source display name.

## 15. Private lowering transaction

```rust
struct HirLoweringTransaction<'db, 'src> {
    database: &'db mut HirDatabase,
    request: LoweringRequest<'src>,
    module_plan: StagedModuleIdentity,
    revision_plan: HirRevision,
    slots: StagedSlotLedger,
    item_arena: StagedArena<HirItem>,
    scope_arena: StagedArena<HirScope>,
    local_arena: StagedArena<HirLocal>,
    expr_arena: StagedArena<HirExpr>,
    stmt_arena: StagedArena<HirStmt>,
    type_arena: StagedArena<HirType>,
    pattern_arena: StagedArena<HirPattern>,
    capture_arena: StagedArena<HirCapture>,
    source_allocations: StagedSourceIndex,
    synthetic_allocations: StagedSyntheticIndex,
    local_generations: StagedLocalGenerations,
    capture_inventories: StagedCaptureInventories,
    diagnostics: Vec<HirDiagnostic>,
    status: HirModuleStatus,
    retirements: Vec<RawHirId>,
    invalidations: HirInvalidationSet,
}
```

The transaction clones only affected 256-entry arena pages and allocation-map shards. It never mutates `ModuleState` during lowering.

### 15.1 Phases

1. validate database, lineage, source generation, exact document identity, module key, and no-op condition;
2. stage module ID/revision without consuming counters;
3. predeclare item/source keys and root scopes;
4. lower signatures, types, patterns, clauses, bodies, statements, expressions, locals, and captures directly from typed handles;
5. determine live source/synthetic keys, stage retirements, and preserve matched IDs;
6. validate every typed child reference, scope parent, local generation, capture owner, arena kind, span, count, and liveness interval;
7. collect and order diagnostics; derive `Clean`/`Recovered`;
8. construct immutable arena/slot snapshots and invalidation set;
9. commit module state, allocator next values, revision, tombstones, current snapshot, old snapshot retention, and cache epoch in one mutation block;
10. return `HirLowerOutput { module, invalidations }`; downstream caches receive invalidations only from this value.

### 15.2 Atomicity

Dropping the transaction before commit discards all proposed entries, source/synthetic maps, counters, generations, captures, diagnostics, retirements, tombstones, revisions, and invalidations. A failed transaction cannot make a previously live ID retired and cannot reserve an ID that the next valid transaction would otherwise receive.

### 15.3 Diagnostics

Diagnostics are assembled as:

1. attached syntax diagnostics referenced by the lowered subtree, in syntax source order;
2. HIR diagnostics sorted by `(primary start, primary end, code, HirIdKind ordinal, message)`;
3. exact deduplication by `(code, primary SourceSpan, structured kind-specific key)`.

Deduplication occurs before enforcing the HIR diagnostic limit. Distinct conditions, bindings, IDs, or secondary labels remain distinct even when message text matches.

## 16. Invalidation contract

```rust
pub struct HirInvalidationSet {
    module: HirModuleId,
    previous: Option<HirSnapshotId>,
    current: HirSnapshotId,
    changed_items: Box<[ItemId]>,
    retired_items: Box<[ItemId]>,
    symbol_revision_changed: bool,
    executable_status_changed: bool,
}

impl HirInvalidationSet {
    pub(crate) fn empty(current: HirSnapshotId) -> Self;
    pub fn module(&self) -> HirModuleId;
    pub fn previous(&self) -> Option<HirSnapshotId>;
    pub fn current(&self) -> HirSnapshotId;
    pub fn changed_items(&self) -> &[ItemId];
    pub fn retired_items(&self) -> &[ItemId];
    pub fn symbol_revision_changed(&self) -> bool;
    pub fn executable_status_changed(&self) -> bool;
    pub fn is_empty(&self) -> bool;
}
```

The set is session-only and non-Serde. It is created inside the transaction and becomes visible only in the successful `HirLowerOutput`. Sema, verifier, runtime-plan, compiler, LSP, and tooling cache owners consume that returned value through their cache boundaries. A no-op returns an empty set with `module = current.module()`, `previous = Some(current)`, `current = current`, empty item lists, and both flags false. A failed transaction returns no set.

## 17. Direct lowering invariant

Lowering reads only typed accessors and IDs from `ParsedSource`. Optional source/display strings may be retained by tooling as labels but can never control behavior. A crate-owned test builder must be able to provide a display label that disagrees with the typed child and prove HIR follows the typed child.

`BorrowKind`, reference types, borrow/dereference expressions, `AssertionMode`, and assertion conditions are moved into the corresponding HIR records by their existing typed enum values and child IDs. No parallel borrow or assertion enum is introduced.
