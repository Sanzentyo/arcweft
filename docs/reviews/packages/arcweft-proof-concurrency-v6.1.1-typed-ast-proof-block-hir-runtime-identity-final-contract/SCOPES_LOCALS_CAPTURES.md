# Scopes, locals, captures, and direct typed lowering

## 1. Exact records

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirScopeKind {
    Module,
    Callable,
    Flow,
    Predicate,
    Proof,
    Block,
    MatchArm,
    Loop,
    Conditional,
    Closure,
    ContractRequires,
    ContractEnsures,
}

pub struct HirScope {
    kind: HirScopeKind,
    parent: Option<ScopeId>,
    owner: HirScopeOwner,
    children: Box<[ScopeId]>,
    locals: Box<[LocalId]>,
}

pub enum HirScopeOwner {
    Module(HirModuleId),
    Item(ItemId),
    Expr(ExprId),
    Stmt(StmtId),
    Syntax(SyntaxNodeId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLocalKind {
    Parameter,
    LetBinding,
    PatternBinding,
    ClosureParameter,
    LoopBinding,
    MatchBinding,
    PostconditionResult,
}

pub struct HirLocal {
    scope: ScopeId,
    kind: HirLocalKind,
    name: HirName,
    generation: LocalGeneration,
    pattern: Option<PatternId>,
    annotation: Option<TypeId>,
    mutable_binding: bool,
    poisoned: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CaptureAccess {
    Read,
    Reassign,
}

pub struct HirCapture {
    closure: ExprId,
    local: LocalId,
    access: CaptureAccess,
    first_use: SourceSpan,
}
```

All fields are private. Constructors are private to lowering. Accessors return immutable IDs/references. `CaptureAccess` intentionally does not encode Copy/Move/Borrow dataflow; those decisions belong to later cuts.

## 2. Scope allocation keys

Source-backed scopes use the identity-bearing owner syntax node plus scope kind:

```rust
SourceKey {
    syntax: owner_syntax_id,
    kind: HirIdKind::Scope,
}
```

When one syntax owner creates more than one scope, additional scopes use a synthetic key with an exact role:

| Construct | Scope allocation | `HirScopeOwner` |
|---|---|---|
| module | source-backed `SourceFile` root | `Module(module_id)` |
| function/ordinary callable | source-backed item for `Callable` scope | `Item(item_id)` |
| flow | source-backed flow item for `Flow` scope | `Item(item_id)` |
| predicate | source-backed `PredicateBody` node for `Predicate` scope | `Item(item_id)` |
| proof | source-backed `ProofBody` node for `Proof` scope | `Item(item_id)` |
| ordinary expression/statement block | source-backed block node for `Block` scope | `Syntax(block_syntax_id)` |
| each match arm | source-backed match-arm node | `Syntax(match_arm_syntax_id)` |
| loop body | source-backed loop/for/while body block | `Stmt(loop_stmt_id)` |
| conditional then/else body | source-backed branch block/expression | `Syntax(branch_syntax_id)` |
| closure | source-backed closure expression | `Expr(closure_expr_id)` |
| requires | `SyntheticKey { owner: Item(item_id), role: ContractRequiresScope, ordinal: 0 }` | `Item(item_id)` |
| ensures | `SyntheticKey { owner: Item(item_id), role: ContractEnsuresScope, ordinal: 0 }` | `Item(item_id)` |

`ContractRequiresScope` and `ContractEnsuresScope` are variants of the one repository-owned `SyntheticRole` enum in `identity.rs`. No private parallel scope-role enum, string role, free helper, or extension trait exists. Using the distinct body node for predicate/proof scopes prevents two `ScopeId` allocations from sharing the same `(SyntaxNodeId, HirIdKind::Scope)` source key.

The callable parent chain is:

```text
Module -> Callable -> Predicate/Proof/Flow -> body/contract/branch scopes
```

A predicate/proof item uses a `Callable` scope for generic and fixed parameters, then one child `Predicate`/`Proof` body scope. Requires and ensures are sibling contract scopes whose parent is the callable scope.

## 3. Name lookup

```rust
pub enum LocalLookup {
    Found(LocalId),
    NotFound,
    AmbiguousPoisoned(Box<[LocalId]>),
}

impl HirModule {
    pub fn lookup_local(
        &self,
        scope: ScopeId,
        name: &HirName,
        before: SourceSpan,
    ) -> Result<LocalLookup, IdResolveError>;
}
```

Lookup walks the current scope then parents. Within one scope it chooses the highest live `LocalGeneration` whose binding point begins before the use span. A poisoned local is never returned as `Found`; when only poisoned candidates exist, tooling receives `AmbiguousPoisoned` and sema emits the existing unresolved/poison diagnostic.

Source order, not raw arena order, determines visibility.

## 4. Local allocation keys and generations

Every authored binding name obtains one `LocalId`. The outer pattern has a source-backed `PatternId`; individual bindings use:

```text
SyntheticKey {
    owner: Pattern(pattern_id),
    role: DestructuredBinding,
    ordinal: preorder_binding_index,
}
```

A simple binding pattern has ordinal zero. A wildcard allocates no local. A poisoned named binding still allocates a poisoned local so tooling can render it, but lookup never resolves through it.

`LocalGeneration` is scoped by `(ScopeId, normalized HirName)`:

- first successful binding: generation 1;
- each later sequential shadow in the same scope: previous plus one;
- nested-scope shadowing starts generation 1 in the nested scope because `ScopeId` is part of the key;
- duplicate names inside one destructuring pattern do not advance the shadow generation for later statements;
- a failed HIR transaction consumes no generation;
- overflow is fatal `HirLowerFailure::LocalGenerationExhausted` and commits nothing.

Slots never reuse an old `LocalId`, even after retirement.

## 5. Parameter binding

Parameters are lowered in authored order. For each parameter:

1. lower its annotation type in the callable scope;
2. lower its full pattern tree;
3. validate irrefutability;
4. enumerate binding names in deterministic preorder, left-to-right;
5. reject duplicate names inside the same pattern;
6. allocate locals with `HirLocalKind::Parameter` in the callable scope;
7. publish all non-poisoned names simultaneously after the full parameter pattern succeeds.

Later parameter annotations/patterns can resolve earlier generic parameters and earlier parameter types where current Arcweft rules allow, but parameter default expressions do not exist. The body and contracts see all fixed parameters.

A parameter named `result` is poisoned and reports `sema.binding.result_reserved`.

## 6. `let` pre-binding and destructuring

For every `let` form:

1. lower and resolve the initializer in the pre-binding scope;
2. lower the optional annotation;
3. lower the pattern without publishing names;
4. validate context-specific irrefutability and duplicate names;
5. allocate local records in pattern preorder;
6. append the successful bindings to the scope's source-ordered binding timeline;
7. lower following statements/tail with the new names visible.

This guarantees that `let x = x` reads an outer `x`, never the new binding.

Destructuring order is depth-first preorder, visiting tuple/sequence elements left-to-right and record fields in authored order. Whole binding visits the whole-name binding before its nested pattern. Or-pattern alternatives must bind exactly the same name set; the first alternative determines local ordinals and later alternatives point to those locals after semantic validation.

## 7. Irrefutability, duplicates, `_`, and poison

- callable parameters, ordinary `let`, proof/predicate pure `let`, and `for` bindings require an irrefutable pattern unless the construct explicitly has a failure branch such as `let else`;
- `if let`, `while let`, and match arms allow refutable patterns;
- duplicate name in one pattern: first occurrence owns the resolvable local, every later occurrence receives its own poisoned `LocalId`, and `sema.binding.duplicate_pattern_name` labels both occurrences;
- `_` has a `PatternId` but no `LocalId` and never shadows;
- `..`/rest has a `PatternId`; when it binds no name it allocates no local;
- an error/missing pattern allocates no unnamed phantom local; any recoverable explicit names inside it allocate poisoned locals for tooling;
- poison propagates to the owning statement/item's executable status but does not prevent immutable arena commit unless a fatal invariant/limit fails.

## 8. Mutability and references

`let mut x` or a mutable binding pattern sets `HirLocal::mutable_binding = true` and permits reassignment to that binding under existing semantic rules. It does not imply that the value is a mutable reference.

Mutable referent access is represented only by the existing `BorrowKind::Mutable` in `HirTypeKind::Reference` and `HirExprKind::Borrow`. Assignment through a dereference is checked as referent access; assignment to a local is checked as binding reassignment. No combined boolean or cloned mutability enum is introduced.

Predicate and proof parameters/lets must be immutable. A parsed mutable binding is retained and poisoned with the context diagnostic; it is never silently normalized.

## 9. Closure parameters and captures

### 9.1 Closure scope

A closure expression allocates one source-backed `Closure` scope. Closure parameters are lowered and published before the body. They shadow outer names and are never captures.

### 9.2 Capture discovery

While lowering the closure body, a local reference that resolves outside the closure scope chain creates or reuses a capture:

```rust
capture_key = (closure_expr_id, outer_local_id)
```

The first use in source order assigns the next capture ordinal and records its exact `SourceSpan`. Later uses reuse the same `CaptureId`. Hash-map iteration never determines capture order.

Nested closures capture the nearest visible local. An inner local that shadows an outer name prevents uses after its binding point from capturing the outer local. A use in an initializer before the inner binding may still capture the outer local under the pre-binding rule.

### 9.3 Access classification

- ordinary read, borrow, call argument, condition, and projection: `CaptureAccess::Read`;
- direct reassignment to a captured mutable binding: `CaptureAccess::Reassign`;
- repeated use upgrades Read to Reassign when any use reassigns;
- no Copy/Move/borrow-mode capture inference is performed in cut 01.1.1.

Capture IDs use `SyntheticKey { owner: Expr(closure_expr_id), role: ClosureCapture, ordinal: first_use_ordinal }`. The ordinal is stable first-use order. A failed transaction consumes no capture ID or ordinal.

## 10. Control-flow scope rules

### 10.1 `if let`

The scrutinee lowers in the parent scope. The pattern and pattern locals belong to the then-branch `Conditional` scope. They are visible in the optional guard and then branch, not in the scrutinee or else branch. The else branch has a separate sibling scope.

### 10.2 Match

The scrutinee lowers in the parent scope and receives a deterministic synthetic `MatchScrutinee` child if desugaring needs storage. Each arm has its own `MatchArm` scope. Arm pattern locals are visible in that arm's guard and body only. Arms cannot see one another's locals.

### 10.3 `while let`

The scrutinee expression lowers in the parent scope on each semantic iteration. Pattern locals belong to the loop body's `Loop` scope and are visible in the guard/body, not after the loop.

### 10.4 `for`

The iterator source lowers in the parent scope. Synthetic iterator/next-value expressions use roles `ForIterator` and `ForNextValue`. The pattern is bound in the loop body scope after the source is lowered. Pattern locals are visible only in the body and require the language's existing irrefutability rule.

### 10.5 Ordinary loops and conditionals

Each loop body and each then/else body receives its own scope. Conditions lower in the parent scope unless the construct explicitly binds a pattern. Block expressions always create a nested `Block` scope, including a one-expression block.

## 11. Postcondition result

Each function, flow, predicate, or proof with at least one `ensures` clause allocates exactly one synthetic local:

```text
SyntheticKey {
    owner: Scope(ensures_scope),
    role: PostconditionResult,
    ordinal: 0,
}
```

The local is named the reserved `result`, has generation 1, is immutable, and has `HirLocalKind::PostconditionResult`.

- predicate result type: synthetic `Bool` type from `PredicateBoolReturn`;
- proof result type: the resolved proof return type, including `Unit`;
- function/flow result type: the callable's resolved return type under the existing callable contract rules;
- visible in every ensures expression;
- not visible in requires, body, parameter patterns, or nested declarations;
- never serializable or authored;
- no ensures clauses: no result local is allocated, but the return type remains present.

The source span is zero-width at the first ensures expression start. When all ensures clauses are recovered/missing, it anchors at the resolved return type end or fixed parameter close.

## 12. Direct lowering by node family

### 12.1 Expressions

Each identity-bearing `ExprNode` allocates/resolves one source-backed `ExprId`. Child expression IDs are lowered directly from typed child handles. Display/source labels are optional diagnostics only.

### 12.2 Statements

Each statement node allocates one `StmtId`. `ProofCallStmt` becomes `HirStmtKind::ProofCall`; common assertions carry existing `AssertionMode` and condition `ExprId`s. Source strings such as current `expr_source` fields are deleted.

### 12.3 Types and patterns

Every typed type/pattern node receives its own source-backed typed ID. Nested same-kind nodes on one physical line remain distinct because their grammar `SyntaxNodeId`s differ. Reference and prefix forms copy the existing `BorrowKind` enum value directly.

### 12.4 Predicate/proof

Lowering order is:

1. predeclare item ID and callable symbol identity inputs;
2. allocate callable/body/contract scopes;
3. lower generics and fixed parameter types/patterns/locals;
4. lower `PredicateBoolReturn` for a predicate or `ProofUnitReturn` for an omitted proof return;
5. lower `where` predicates;
6. lower requires in requires scope;
7. allocate postcondition result and lower ensures in ensures scope;
8. lower expression body or block statements/tail in body scope;
9. stage semantic/context diagnostics and poison;
10. finalize ordered child ID arrays.

No step clones an AST enum or parses a string.

## 13. Resolver accessors

```rust
impl HirScope {
    pub fn kind(&self) -> HirScopeKind;
    pub fn parent(&self) -> Option<ScopeId>;
    pub fn owner(&self) -> &HirScopeOwner;
    pub fn children(&self) -> &[ScopeId];
    pub fn locals(&self) -> &[LocalId];
}

impl HirLocal {
    pub fn scope(&self) -> ScopeId;
    pub fn kind(&self) -> HirLocalKind;
    pub fn name(&self) -> &HirName;
    pub fn generation(&self) -> LocalGeneration;
    pub fn pattern(&self) -> Option<PatternId>;
    pub fn annotation(&self) -> Option<TypeId>;
    pub fn is_mutable_binding(&self) -> bool;
    pub fn is_poisoned(&self) -> bool;
}

impl HirCapture {
    pub fn closure(&self) -> ExprId;
    pub fn local(&self) -> LocalId;
    pub fn access(&self) -> CaptureAccess;
    pub fn first_use(&self) -> &SourceSpan;
}
```

All owner/child IDs are validated before commit. Resolvers never panic on stale, wrong-module, or corrupted test IDs.

## 14. Limits and atomic evidence

- maximum locals per module: 65,536;
- maximum locals per scope: 4,096;
- maximum captures per module: 65,536;
- maximum synthetic descendants per owner: 1,024;
- local generation domain: full nonzero generation type; no wrap.

Exact maxima commit. One over or seeded exhaustion is fatal and leaves all arena lengths, slot counters, local-generation maps, capture inventories, tombstones, current snapshot, and cache epochs exactly unchanged.
