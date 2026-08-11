# Consolidated API, constructor, error, range, and diagnostic contract

## 1. Visibility and constructor policy

| Type family | Public type | Production constructor | Raw constructor | Serde |
|---|---:|---|---|---:|
| source document/provenance | existing | existing checked source APIs | existing private/checked | existing policy |
| syntax database/lineage/snapshot/node IDs | yes | database/transaction only | private | no |
| typed AST handles | yes | `attachment.rs` only, `pub(crate)` | none | no |
| unbound fragment | yes | standalone fragment parser | no source-backed ID | no |
| attached fragment | yes | `SyntaxDatabase::attach_fragment` only | none | no |
| HIR database/module/snapshot/typed IDs | yes | `HirDatabase` transaction only | private | no |
| HIR arena records | yes, immutable | lowering transaction only | none | no |
| project module/project/view | yes | checked `try_new` | none | no session ID codec |
| `ProofArtifactId` | yes | registered proof only, `pub(crate)` | none | no |
| core assertion guard/fingerprint | yes | checked fixed-byte decode or runtime-plan derivation | unchecked private | yes |
| runtime assertion site/inventory/fault | yes | runtime-plan/session only | none | no |

No compatibility aliases, deprecated constructors, extension traits, or public integer conversion APIs survive.

## 2. Syntax transaction APIs

```rust
impl SyntaxDatabase {
    pub fn try_new() -> Result<Self, SyntaxDatabaseCreateError>;

    pub fn parse_initial(
        &mut self,
        source_snapshot: SourceSnapshotId,
        document: Arc<SourceDocument>,
        options: ParseOptions,
    ) -> Result<ParsedSource, ParseFailure>;

    pub fn reparse(
        &mut self,
        current: &ParsedSource,
        edits: &[SourceEdit],
        options: ParseOptions,
    ) -> Result<ParsedSource, ParseFailure>;

    pub fn current(
        &self,
        lineage: SyntaxLineageId,
    ) -> Result<ParsedSource, SyntaxLookupError>;

    pub fn resolve_current<K: AstKind>(
        &self,
        node: &AstNode<K>,
    ) -> Result<AstNode<K>, SyntaxLookupError>;

    pub fn database_id(&self) -> SyntaxDatabaseId;
}

pub fn parse_source(
    database: &mut SyntaxDatabase,
    source_snapshot: SourceSnapshotId,
    document: Arc<SourceDocument>,
    options: ParseOptions,
) -> Result<ParsedSource, ParseFailure>;
```

```rust
pub enum ParseFailure {
    SourceMismatch,
    StaleSnapshot {
        current: SyntaxSnapshotId,
        supplied: SyntaxSnapshotId,
    },
    InvalidEdits(InvalidEditSet),
    LimitExceeded(SyntaxLimit),
    SourceGenerationExhausted,
    DatabaseIdentityExhausted,
    LineageIdentityExhausted,
    NodeIdentityExhausted,
    LosslessnessViolation,
    Attachment(AttachmentFailure),
    Invariant(SyntaxInvariantFailure),
}
```

Fatal failures commit no document revision, generation, tree, identity, typed attachment, diagnostic vector, statistic, or cache entry.

## 3. Typed syntax APIs

```rust
impl ParsedSource {
    pub fn snapshot_id(&self) -> &SyntaxSnapshotId;
    pub fn is_same_snapshot(&self, other: &ParsedSource) -> bool;
    pub fn source_snapshot_id(&self) -> &SourceSnapshotId;
    pub fn document(&self) -> &SourceDocument;
    pub fn tree(&self) -> TypedSyntaxTree;
    pub fn root_syntax(&self) -> SyntaxNodeHandle;
    pub fn syntax_node(&self, id: SyntaxNodeId)
        -> Result<SyntaxNodeHandle, SyntaxLookupError>;
    pub fn typed_node<K: AstKind>(&self, id: SyntaxNodeId)
        -> Result<AstNode<K>, SyntaxLookupError>;
    pub fn bind_rowan(&self, node: &SyntaxNode)
        -> Result<SyntaxNodeHandle, SyntaxLookupError>;
    pub fn resolve_exact<K: AstKind>(
        &self,
        node: &AstNode<K>,
    ) -> Result<AstNode<K>, SyntaxLookupError>;
    pub fn resolve_exact_syntax(
        &self,
        node: &SyntaxNodeHandle,
    ) -> Result<SyntaxNodeHandle, SyntaxLookupError>;
    pub fn diagnostics(&self) -> &[SyntaxDiagnostic];
    pub fn status(&self) -> ParseStatus;
}

impl SyntaxNodeHandle {
    pub fn id(&self) -> SyntaxNodeId;
    pub fn kind(&self) -> SyntaxKind;
    pub fn rowan(&self) -> &SyntaxNode;
    pub fn range(&self) -> SourceRange;
    pub fn source_span(&self) -> SourceSpan;
    pub fn cast<K: AstKind>(&self) -> Result<AstNode<K>, SyntaxLookupError>;
}

impl<K: AstKind> AstNode<K> {
    pub fn id(&self) -> SyntaxNodeId;
    pub fn snapshot_id(&self) -> &SyntaxSnapshotId;
    pub fn syntax(&self) -> SyntaxNodeHandle;
    pub fn range(&self) -> SourceRange;
    pub fn source_span(&self) -> SourceSpan;
    pub fn is_same_reconciled_node(&self, other: &Self) -> bool;
}
```

```rust
pub enum SyntaxLookupError {
    WrongDatabase { expected: SyntaxDatabaseId, actual: SyntaxDatabaseId },
    WrongLineage { expected: SyntaxLineageId, actual: SyntaxLineageId },
    WrongSnapshot { expected: SyntaxSnapshotId, actual: SyntaxSnapshotId },
    ForeignRowanRoot { expected: SyntaxSnapshotId },
    StaleGeneration { current: SourceGeneration, supplied: SourceGeneration },
    KindMismatch { id: SyntaxNodeId, expected: SyntaxKind, actual: SyntaxKind },
    NotIdentityBearing { kind: SyntaxKind },
    MissingNode { id: SyntaxNodeId },
    UnboundFragment,
}
```

`WrongDatabase` and `WrongLineage` are checked before node slot lookup. `WrongSnapshot` applies to `resolve_exact`/`resolve_exact_syntax`; stable-ID lookup through `syntax_node`/`typed_node` resolves in the receiver snapshot. `ForeignRowanRoot` applies only to an unbound raw Rowan node whose exact root allocation is foreign. `StaleGeneration` applies to current-lineage resolution.

## 4. Fragment APIs

```rust
pub fn parse_expression_fragment(
    text: &str,
    options: ParseOptions,
) -> UnboundFragment<ExpressionFragment>;

pub fn parse_type_fragment(
    text: &str,
    options: ParseOptions,
) -> UnboundFragment<TypeFragment>;

pub fn parse_pattern_fragment(
    text: &str,
    options: ParseOptions,
) -> UnboundFragment<PatternFragment>;

pub fn parse_statement_fragment(
    text: &str,
    options: ParseOptions,
) -> UnboundFragment<StatementFragment>;

impl<K: FragmentKind> UnboundFragment<K> {
    pub fn text(&self) -> &str;
    pub fn diagnostics(&self) -> &[FragmentDiagnostic];
    pub fn completion(&self) -> &ParseCompletion;
}

impl<K: FragmentKind> AttachedFragment<K> {
    pub fn snapshot_id(&self) -> &SyntaxSnapshotId;
    pub fn root(&self) -> AstNode<K::AstKind>;
}

impl SyntaxDatabase {
    pub fn attach_fragment<K: FragmentKind>(
        &mut self,
        source_snapshot: SourceSnapshotId,
        document: Arc<SourceDocument>,
        span: SourceSpan,
        fragment: UnboundFragment<K>,
    ) -> Result<AttachedFragment<K>, ParseFailure>;
}
```

Only a bound `ParsedSource` rooted at `SourceFile` can construct `LoweringRequest`.

## 5. HIR APIs

```rust
impl HirDatabase {
    pub fn try_new() -> Result<Self, HirDatabaseCreateError>;
    pub fn lower(
        &mut self,
        request: LoweringRequest<'_>,
    ) -> Result<HirLowerOutput, HirLowerFailure>;
    pub fn current(&self, key: &HirModuleKey) -> Option<Arc<HirModule>>;
    pub fn snapshot(
        &self,
        id: HirSnapshotId,
    ) -> Result<Arc<HirModule>, HirSnapshotLookupError>;
}

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

`HirLowerFailure` is constructed only by `lower::transaction`. It is fatal and never inserted as a diagnostic into a committed module. Successful lowering returns invalidations only inside `HirLowerOutput`; a no-op returns the exact current module plus an empty invalidation set.

Creation and lookup support types are exact and non-Serde:

```rust
pub enum SyntaxDatabaseCreateError {
    IdentityExhausted,
}

pub enum AttachmentFailure {
    UnbalancedEvents,
    TokenCoverageMismatch,
    MissingTokenOutOfBounds,
    AstTagMismatch { id: SyntaxNodeId, expected: AstTag, actual: SyntaxKind },
    DuplicateAttachment { id: SyntaxNodeId },
    MissingAttachment { id: SyntaxNodeId },
    FragmentNotComplete { completion: ParseCompletion },
    FragmentTextMismatch,
    Invariant,
}

pub enum SyntaxInvariantFailure {
    AllocatorRegression,
    IdentityMapMismatch,
    SnapshotOwnershipMismatch,
}

pub enum HirDatabaseCreateError {
    IdentityExhausted,
}

pub enum HirSnapshotLookupError {
    WrongDatabase { expected: HirDatabaseId, actual: HirDatabaseId },
    UnknownModule { module: HirModuleId },
    UnknownRevision { module: HirModuleId, revision: HirRevision },
}

pub enum HirSourceLookupError {
    WrongSyntaxDatabase { expected: SyntaxDatabaseId, actual: SyntaxDatabaseId },
    WrongSyntaxLineage { expected: SyntaxLineageId, actual: SyntaxLineageId },
    NotLowered { syntax: SyntaxNodeId, expected: HirIdKind },
    KindMismatch { syntax: SyntaxNodeId, expected: HirIdKind, actual: HirIdKind },
}

pub enum HirInvariantFailure {
    ArenaKindMismatch,
    DuplicateSourceKey,
    DuplicateSyntheticKey,
    InvalidLiveInterval,
    InvalidScopeParent,
    InvalidLocalTimeline,
    InvalidCaptureOwner,
    InvalidSourceSpan,
}
```

Every enum derives `Clone`, `Debug`, `Eq`, `thiserror::Error`, and `PartialEq` where its fields permit; the detailed display strings live with the owning module and do not alter variant selection.

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
    pub fn item_for_syntax(&self, id: SyntaxNodeId) -> Result<ItemId, HirSourceLookupError>;
    pub fn scope_for_syntax(&self, id: SyntaxNodeId) -> Result<ScopeId, HirSourceLookupError>;
    pub fn local_for_syntax(&self, id: SyntaxNodeId) -> Result<LocalId, HirSourceLookupError>;
    pub fn expr_for_syntax(&self, id: SyntaxNodeId) -> Result<ExprId, HirSourceLookupError>;
    pub fn stmt_for_syntax(&self, id: SyntaxNodeId) -> Result<StmtId, HirSourceLookupError>;
    pub fn type_for_syntax(&self, id: SyntaxNodeId) -> Result<TypeId, HirSourceLookupError>;
    pub fn pattern_for_syntax(&self, id: SyntaxNodeId) -> Result<PatternId, HirSourceLookupError>;
}
```

```rust
pub enum IdResolveError {
    WrongModule { expected: HirModuleId, actual: HirModuleId },
    NotYetLive { id: RawHirIdView, snapshot: HirSnapshotId, born: HirRevision },
    Retired { id: RawHirIdView, snapshot: HirSnapshotId, retired_at: HirRevision },
    KindMismatch { id: RawHirIdView, expected: HirIdKind, actual: HirIdKind },
}
```

## 6. Project APIs

```rust
impl HirProjectModule {
    pub fn try_new(
        package: CallablePackageId,
        path: CanonicalModulePath,
        source_document: SourceDocumentId,
        module: Arc<HirModule>,
    ) -> Result<Self, HirProjectError>;
}

impl HirProject {
    pub fn try_new(
        package: CallablePackageId,
        modules: impl IntoIterator<Item = HirProjectModule>,
    ) -> Result<Self, HirProjectError>;
    pub fn view(&self) -> HirProjectView<'_>;
    pub fn executable_view(&self) -> Result<HirProjectView<'_>, HirProjectError>;
}
```

`HirProjectError` variants are exactly those in `PROJECT_AND_SYMBOLS.md`. No method mutates package/path/source ownership of a module.

## 7. Runtime assertion APIs

```rust
impl RuntimeAssertion {
    pub fn new(
        guard: RuntimeAssertionGuardId,
        condition: String,
        message: String,
        profile: RuntimeAssertionProfile,
    ) -> Self;
    pub fn guard(&self) -> RuntimeAssertionGuardId;
    pub fn condition(&self) -> &str;
    pub fn message(&self) -> &str;
    pub fn profile(&self) -> RuntimeAssertionProfile;
}

impl RuntimeAssertionFailure {
    pub fn new(assertion: RuntimeAssertion) -> Self;
    pub fn assertion(&self) -> &RuntimeAssertion;
    pub fn into_assertion(self) -> RuntimeAssertion;
}

impl AssertionMode {
    pub const fn is_runtime_capable(self) -> bool;
}

impl RuntimeAssertionGuardId {
    pub fn try_from_bytes(
        bytes: [u8; 16],
    ) -> Result<Self, RuntimeIdentityDecodeError>;
    pub const fn as_bytes(&self) -> &[u8; 16];
}

impl RuntimeArtifactFingerprint {
    pub fn try_from_bytes(
        bytes: [u8; 32],
    ) -> Result<Self, RuntimeIdentityDecodeError>;
    pub const fn as_bytes(&self) -> &[u8; 32];
}

impl AssertionConditionIndex {
    pub fn try_new(
        index: usize,
        condition_count: usize,
    ) -> Result<Self, AssertionConditionIndexError>;
    pub const fn get(self) -> u8;
}

impl RuntimeAssertionMode {
    pub fn try_from_assertion_mode(
        mode: AssertionMode,
    ) -> Result<Self, RuntimeAssertionModeError>;
}

impl RuntimeAssertionInventory {
    pub fn project_failure(
        &self,
        artifact: RuntimeArtifactFingerprint,
        failure: RuntimeAssertionFailure,
    ) -> Result<RuntimeAssertionFault, RuntimeAssertionProjectionError>;
}
```

`RuntimeAssertionMode` has only `Check` and `Debug`. No runtime fault constructor accepts `AssertionMode::Prove`. `RuntimeIdentityDecodeError`, site/fault accessors, `ExecutionDiagnosticContext`, projection errors, and the exact runtime-plan `ArtifactKey` fingerprint authority are frozen in `RUNTIME_ASSERTION_FAULT.md`.

## 8. Range rules

| Construct | Primary range rule |
|---|---|
| real token-backed node | first owned real token start through last owned real token end |
| source item | first outer doc/attribute token when attached, otherwise declaration keyword, through body end |
| declaration name | exact identifier; missing name zero-width at expected insertion |
| generic/parameter group | open delimiter through close delimiter; missing close ends at synchronization anchor |
| clause | keyword start through terminator; excludes following newline trivia |
| expression body | `=` start through expression/optional semicolon; excludes following newline |
| block | opening brace start through closing brace end; missing close ends at recovery anchor/EOF |
| statement | first non-trivia token through semicolon or last token before logical newline |
| authored tail | exact expression |
| omitted tail | zero-width at close-brace start/recovery anchor |
| synthetic HIR child | zero-width source span at the role-specific anchor in `HIR_DATABASE_AND_ARENAS.md` |
| runtime fault condition | exact condition expression `SourceSpan` |
| runtime fault statement label | exact assertion statement `SourceSpan` |

Every span is obtained through the exact retained `SourceDocument`.

## 9. Syntax diagnostics

All codes are stable lowercase dotted strings. Removed spellings have no dedicated code.

| Code | Owner | Trigger | Primary range | Recovery/executability |
|---|---|---|---|---|
| `syntax.item.unexpected_token` | syntax parser | token does not begin/continue current item grammar | offending token | ordinary `ErrorItem`, non-executable |
| `syntax.statement.unexpected_token` | syntax parser | malformed current statement | offending balanced fragment | `ErrorStatement`, non-executable owner |
| `syntax.predicate.missing_name` | predicate parser | missing ordinary identifier with valid later parameter synchronization | zero-width before fixed parameter group | `MissingName`, predicate non-executable |
| `syntax.proof.missing_name` | proof parser | same for proof | zero-width before fixed parameter group | `MissingName`, proof non-executable |
| `syntax.predicate.return_not_allowed` | predicate parser | authored `-> Type` | arrow through recovered type | typed recovery, predicate non-executable |
| `syntax.predicate.missing_parameters` | predicate parser | no fixed parameter group | zero-width before next header/body boundary | missing group, non-executable |
| `syntax.proof.missing_parameters` | proof parser | no fixed parameter group | same | missing group, non-executable |
| `syntax.predicate.missing_parameter_close` | predicate parser | missing `)` | zero-width synchronization anchor | missing delimiter, non-executable |
| `syntax.proof.missing_parameter_close` | proof parser | same | same | same |
| `syntax.predicate.malformed_header` | predicate parser | malformed generic/where/header sequence not covered above | offending recovered fragment | typed error child, non-executable |
| `syntax.proof.malformed_header` | proof parser | same | same | same |
| `syntax.contract.invalid_clause_order` | shared contract parser | `requires` after any `ensures` | full misplaced clause | clause retained, owner non-executable |
| `syntax.contract.missing_expression` | shared contract parser | clause terminates without expression | zero-width before terminator | `MissingExpression`, owner non-executable |
| `syntax.predicate.missing_body` | predicate parser | no `=` expression or block | zero-width end of accepted header | `MissingBody`, non-executable |
| `syntax.proof.missing_body` | proof parser | same | same | same |
| `syntax.predicate.missing_block_close` | predicate parser | block reaches top-level sync/EOF | zero-width recovery anchor | missing close, non-executable |
| `syntax.proof.missing_block_close` | proof parser | same | same | same |
| `syntax.block.malformed_tail` | block parser | tokens after apparent tail or malformed unterminated final expression | recovered fragment | error stmt + omitted tail, non-executable |
| `syntax.type.missing` | type parser | required type omitted | zero-width insertion | `MissingType`, non-executable owner |
| `syntax.pattern.missing` | pattern parser | required pattern omitted | zero-width insertion | `MissingPattern`, non-executable owner |
| `syntax.expression.missing` | expression parser | required expression omitted | zero-width insertion | `MissingExpression`, non-executable owner |

Fatal limits produce `ParseFailure::LimitExceeded` and commit no diagnostic snapshot. They do not use ordinary diagnostic codes.

## 10. Semantic/context diagnostics

| Code | Trigger | Primary range | Secondary evidence |
|---|---|---|---|
| `sema.predicate.missing_boolean_tail` | block predicate has omitted tail | omitted-tail zero-width span | predicate return contract |
| `sema.predicate.tail_must_be_bool` | predicate tail type is not Bool | tail expression | resolved actual/expected types |
| `sema.predicate.assertion_not_allowed` | any assertion in predicate body | assertion statement | predicate declaration |
| `sema.proof.runtime_assertion_not_allowed` | `assert.check`/`assert.debug` in proof | mode/statement span | proof declaration |
| `sema.proof.missing_value_tail` | non-Unit proof omits block tail | omitted-tail zero-width span | return type |
| `sema.proof.tail_type_mismatch` | authored proof tail does not match return type | tail expression | return type |
| `sema.proof.expression_statement_not_proof_call` | call-shaped proof statement resolves to non-proof or fails callable-kind check | call expression | resolved declaration when present |
| `sema.proof.impure_let` | proof/predicate let initializer is impure | initializer | impure operation/callee |
| `sema.predicate.mutable_binding_not_allowed` | mutable parameter/let in predicate | binding token/name | declaration |
| `sema.proof.mutable_binding_not_allowed` | mutable parameter/let in proof | binding token/name | declaration |
| `sema.binding.duplicate_pattern_name` | same normalized name occurs twice in one pattern | later name | first name |
| `sema.binding.result_reserved` | authored parameter/local binds `result` | binding name | ensures-only synthetic result rule |
| `sema.pattern.irrefutable_required` | parameter/ordinary let/for pattern is refutable | pattern | construct keyword |
| `sema.callable.recursive_contract` | SCC contains predicate/proof | call edge | declaration(s) in SCC |
| `sema.callable.duplicate_name` | function/predicate/proof ordinary name collision | later name | first declaration name |

Existing general resolution, type, purity, visibility, import, assertion-condition, reference, and borrow diagnostics remain owned by their current modules.

## 11. Runtime diagnostic

The stable presentation code is exactly:

```text
runtime.assertion_failed
```

With a matching fresh-session inventory, the primary label is the exact condition span and the statement is a secondary label. Without inventory, presentation may use persisted source-map evidence but must not fabricate a `StmtId`, mode, or revision-bound HIR span.

## 12. Diagnostic ordering and deduplication

Syntax diagnostics are emitted in parser/event source order. HIR/semantic diagnostics are sorted by:

```text
(primary start, primary end, code, HIR kind ordinal, message)
```

Exact duplicates are removed by structured key `(code, primary SourceSpan, kind-specific identity)`. Message text alone is never a deduplication or identity key. Syntax diagnostics precede HIR diagnostics at equal range.

## 13. Poison and execution gates

A diagnostic does not automatically imply fatal transaction failure. Recoverable syntax/HIR commits a complete immutable `Recovered` snapshot. That snapshot:

- resolves typed IDs;
- preserves error/missing nodes and exact ranges;
- supports formatter/LSP/tooling queries;
- is rejected by executable project view, sema readiness, codegen, runtime-plan, verifier-result, and persistent compile cache entrypoints.

Only fatal mismatch, stale input, limit, identity exhaustion, or invariant failures commit nothing.
