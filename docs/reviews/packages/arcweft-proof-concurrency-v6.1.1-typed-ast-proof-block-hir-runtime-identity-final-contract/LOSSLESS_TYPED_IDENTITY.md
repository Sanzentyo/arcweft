# Lossless grammar tree, typed attachment, and syntax identity

## 1. Owning modules

The final syntax responsibility layout is:

```text
crates/arcweft-lang-syntax/src/
  grammar.rs                 public SyntaxKind and SyntaxRole vocabulary
  grammar/
    event.rs                 crate-private parser event stream and markers
    build.rs                 event validation and Rowan GreenNode construction
    kinds.rs                 inherent SyntaxKind classification
  attachment.rs              sealed AstKind, AstNode, typed attachment index
  attachment/
    error.rs                 SyntaxLookupError and AttachmentFailure
    snapshot.rs              immutable SyntaxSnapshotData and node handles
  incremental.rs             facade only: SyntaxDatabase and ParsedSource exports
  incremental/
    database.rs              lineage ownership and public transactions
    transaction.rs           private syntax transaction staging/commit
    reconcile.rs             grammar-node reconciliation
    shape.rs                 role-aware semantic shapes
    limits.rs                inclusive syntax budgets
  parser.rs                  document/fragment facade only
  parser/
    document.rs              one full-source parser cursor
    item.rs                  item grammar dispatch
    predicate_proof.rs       predicate/proof surface and recovery
    statement.rs             statement grammar
    expression.rs            Pratt/expression grammar over the shared cursor
    pattern.rs               pattern grammar over the shared cursor
    type_ref.rs              type grammar over the shared cursor
    recovery.rs              synchronization and missing-node emission
```

No `mod.rs` files are introduced. `grammar.rs`, `attachment.rs`, `incremental.rs`, and `parser.rs` are re-export facades and remain below 250 physical lines after the split.

## 2. Exact `SyntaxKind` contract

`SyntaxKind` remains `pub`, `#[repr(u16)]`, `Copy`, `Eq`, `Ord`, and `Hash`, because formatter, LSP, and syntax tests inspect grammar families. Raw Rowan conversion is crate-private. The enum has the following final families and variants.

### 2.1 Root, non-identity containers, and layout sugar

```rust
pub enum SyntaxKind {
    SourceFile,
    ItemList,
    StatementList,
    ExpressionList,
    ParameterList,
    GenericParameterList,
    WherePredicateList,
    AttributeList,
    FieldList,
    ArgumentList,
    MatchArmList,
    LogicalLine,
    IndentedSuite,
    FenceBody,
    DelimitedGroup,
    // ... remaining variants below
}
```

`SourceFile` is identity-bearing. Every other variant in this subsection is a lossless structural wrapper and deliberately receives no `SyntaxNodeId`.

`LogicalLine` groups the exact tokens ending at a depth-zero physical newline or EOF. `IndentedSuite` and `FenceBody` retain indentation/fence tokens and recovered ownership but are not a second semantic-parent authority. Grammar nodes inside them own semantic identity.

### 2.2 Items

```rust
ModuleDeclaration,
UseDeclaration,
FlowItem,
FunctionItem,
PredicateItem,
ProofItem,
AgentItem,
CallableItem,
StateItem,
TraitItem,
ImplItem,
EnumItem,
StructItem,
TypeAliasItem,
EntityDeclarationItem,
EntryDeclarationItem,
ExternCapabilityItem,
ExternModuleItem,
HookItem,
DialogueDefaultsItem,
MemoFunctionItem,
TestItem,
BenchItem,
ParserItem,
SourceItem,
StyleItem,
TopLevelFlowItem,
ErrorItem,
```

There is no `TrustedAxiomItem`, `BorrowBlock`, `CalcItem`, or historical removed-syntax kind. `ErrorItem` is ordinary current-grammar recovery and is non-executable.

### 2.3 Declaration components, attributes, contracts, and bodies

```rust
InnerAttribute,
OuterAttribute,
DocBlock,
Visibility,
NameDefinition,
NameReference,
Path,
PathSegment,
GenericParameterGroup,
GenericParameter,
LifetimeParameter,
TypeParameter,
FixedParameterGroup,
Parameter,
WhereClause,
WherePredicate,
ReturnType,
RequiresClause,
EnsuresClause,
ExpressionBody,
PredicateBody,
ProofBody,
FunctionBody,
FlowBody,
Block,
PredicateBlock,
ProofBlock,
OpenBraceNode,
CloseBraceNode,
OpenParenNode,
CloseParenNode,
OpenBracketNode,
CloseBracketNode,
OpenAngleNode,
CloseAngleNode,
```

All variants in this subsection are identity-bearing except `PathSegment`. A full `Path` is the semantic typed node and carries identity; `PathSegment` remains an ID-less child because resolution and HIR key the complete authored path. `Open*Node` and `Close*Node` wrap one real delimiter token or one zero-width `MissingToken` and therefore give delimiter roles distinct node identities without assigning IDs to tokens.

`DocBlock` owns consecutive documentation tokens attached to one declaration. Each `OuterAttribute` owns one attribute; `AttributeList` is ID-less. A documentation or attribute node that changes owner receives fresh identity because its semantic parent role changes.

### 2.4 Statements

```rust
AssertionStatement,
LetStatement,
AssignmentStatement,
LetElseStatement,
LetChoiceStatement,
LetScopeStatement,
LetLoopStatement,
LetAwaitStatement,
LetActionReceiveStatement,
ReturnStatement,
OutStatement,
GotoStatement,
ThreadStatement,
DeferBlockStatement,
DeferStatement,
YieldStatement,
SignalStatement,
LifetimeSetStatement,
WaitStatement,
OnStatement,
UnsafeLifetimeStatement,
IfStatement,
LoopStatement,
WhileStatement,
WhileLetStatement,
ForStatement,
MatchStatement,
CloseStatement,
SelectStatement,
BreakStatement,
ContinueStatement,
ExpressionStatement,
ProofCallStatement,
ErrorStatement,
```

Every statement node is identity-bearing. `ProofCallStatement` is the proof-body surface boundary for a call-shaped expression statement. Semantic resolution must still prove that its callee is a proof.

### 2.5 Expressions

```rust
LiteralExpression,
EntityReferenceExpression,
LifetimePathExpression,
PathExpression,
ShortVariantExpression,
PlaceholderExpression,
TupleExpression,
BracketSequenceExpression,
NumericBracketSequenceExpression,
ArrayRepeatExpression,
CallExpression,
SelectExpression,
DialogueCallExpression,
IndexExpression,
PipeExpression,
TryExpression,
AwaitExpression,
ThreadExpression,
RangeExpression,
RecordExpression,
RecordLiteralExpression,
BinaryExpression,
BorrowExpression,
DereferenceExpression,
ClosureExpression,
UnaryExpression,
BlockExpression,
ComputationBlockExpression,
MemoBlockExpression,
NamedBlockExpression,
IfExpression,
IfLetExpression,
MatchExpression,
MatchArm,
CallArgument,
RecordField,
ClosureParameter,
OmittedBlockTail,
MissingExpression,
ErrorExpression,
```

Every variant in this subsection is identity-bearing except `ExpressionList`, `ArgumentList`, and `MatchArmList` from subsection 2.1. Each call argument, record field, closure parameter, and match arm receives a distinct ID because it is independently source-backed and may own types, patterns, names, or diagnostics.

`OmittedBlockTail` is a zero-width identity-bearing syntax node at the tail insertion point. It is not itself a HIR expression. A Unit-valued implicit tail receives a synthetic HIR `ExprId`; a required missing value receives a poisoned synthetic HIR expression.

### 2.6 Patterns

```rust
WildcardPattern,
BindingPattern,
MutableBindingPattern,
LiteralPattern,
EntityReferencePattern,
TuplePattern,
RecordPattern,
RecordPatternField,
VariantPattern,
SequencePattern,
RestPattern,
WholeBindingPattern,
OrPattern,
MissingPattern,
ErrorPattern,
```

Every pattern and record-pattern field is identity-bearing. `_` has a pattern ID but later allocates no local. The outer node of a destructuring pattern and every nested pattern have independent identity.

### 2.7 Types

```rust
PrimitiveType,
PathType,
GenericApplicationType,
TupleType,
ReferenceType,
SliceType,
ArrayType,
FunctionType,
SumType,
InferType,
LifetimeType,
ElidedRegionType,
TypeArgument,
MissingType,
ErrorType,
```

Every type and type argument is identity-bearing. `ReferenceType` carries the existing `BorrowKind` in its typed projection. An elided region is source-backed only when an explicit omission wrapper exists; the semantic region itself is a synthetic HIR child keyed by `SyntheticRole::ElidedRegion`.

### 2.8 Recovery and missing grammar nodes

```rust
MissingName,
MissingBody,
MissingTokenNode,
ErrorNode,
```

These nodes are identity-bearing. `MissingTokenNode` is the grammatical role wrapper around the zero-width `MissingToken` token. Specialized `MissingExpression`, `MissingPattern`, and `MissingType` are used where the expected semantic family is known; generic `MissingTokenNode` is used for punctuation and keywords.

### 2.9 Tokens

```rust
WhitespaceToken,
NewlineToken,
CommentToken,
DocCommentToken,
IdentifierToken,
LifetimeToken,
NumberToken,
StringToken,
RawStringToken,
CharacterToken,
EntityReferenceToken,
KeywordToken,
PunctuationToken,
TextToken,
ErrorToken,
MissingToken,
EofToken,
```

Tokens never receive `SyntaxNodeId`. Their exact UTF-8 text is retained, including invalid/recovered token text. `MissingToken` has empty text. Its expected token class and insertion range are stored in immutable snapshot metadata keyed by the enclosing identity-bearing missing/delimiter node, not encoded in an ad hoc token string.

## 3. Identity-bearing classification and parent roles

`SyntaxKind` owns the classification as inherent methods:

```rust
impl SyntaxKind {
    pub const fn is_token(self) -> bool;
    pub const fn identity_class(self) -> IdentityClass;
    pub const fn ast_tag(self) -> Option<AstTag>;
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum IdentityClass {
    IdentityBearing,
    StructuralWrapper,
    Token,
}
```

No extension trait or free classification helper survives.

Every identity-bearing child is reconciled under an exact `SyntaxRole` determined by its parent kind and field:

```rust
pub(crate) enum SyntaxRole {
    Root,
    Attribute(u16),
    Documentation,
    Visibility,
    Name,
    GenericGroup,
    GenericParameter(u16),
    ParameterGroup,
    Parameter(u16),
    ParameterPattern,
    ParameterType,
    WhereClause,
    WherePredicate(u16),
    ReturnType,
    RequiresClause(u16),
    EnsuresClause(u16),
    Body,
    OpenDelimiter,
    CloseDelimiter,
    Statement(u32),
    Tail,
    Condition,
    Callee,
    Argument(u16),
    Target,
    Operand,
    LeftOperand,
    RightOperand,
    Pattern,
    Type,
    Initializer,
    Scrutinee,
    Guard,
    ThenBranch,
    ElseBranch,
    MatchArm(u16),
    Field(u16),
    Element(u32),
    Recovery(u32),
}
```

The ordinal is a deterministic sibling discriminator only after semantic matching. It is not an identity and is never exposed as one.

## 4. Single parse/event/tree pipeline

### 4.1 Lexer

The lexer runs once over `SourceDocument::text()` and emits `LexToken { kind, range }`. Token text is always sliced from the exact document; token values do not own copied source strings.

### 4.2 Parser events

All document and nested grammar parsers share one `DocumentParser` with one token cursor and one event vector:

```rust
pub(crate) enum SyntaxEvent {
    StartNode {
        kind: SyntaxKind,
        role: SyntaxRole,
    },
    Token {
        kind: SyntaxKind,
        range: SourceRange,
    },
    MissingToken {
        expected: ExpectedToken,
        at: usize,
    },
    Diagnostic(PendingSyntaxDiagnostic),
    FinishNode,
}

pub(crate) fn parse_document_events(
    document: &SourceDocument,
    options: ParseOptions,
    limits: &SyntaxTransactionLimits,
) -> Result<EventParse, ParseFailure>;
```

Expression, type, pattern, statement, callback, and body functions receive `&mut DocumentParser`; none accept an authoritative `&str` fragment. A parser may ask the document for a diagnostic-only source slice after it has already produced typed events, but that slice is never parsed again and never controls lowering.

### 4.3 Validated Rowan construction

`grammar::build` validates balanced start/finish events, token order, exact byte coverage, and missing-token zero-width placement. It then produces one Rowan `GreenNode` and an `UnattachedGrammarIndex` whose entries use stable event paths, not source ranges, to point to identity-bearing nodes in the newly built red tree.

```rust
pub(crate) fn build_lossless_tree(
    document: &SourceDocument,
    events: EventParse,
) -> Result<UnattachedSyntax, AttachmentFailure>;
```

The concatenated real token text must equal the exact source bytes. Failure is fatal and commits nothing.

### 4.4 Typed attachment is a projection, not a parse

`attachment::attach` walks the completed grammar tree once in event order, validates each identity-bearing kind against its `AstTag`, and builds typed child tables from node roles. It never searches by range or text.

```rust
pub(crate) fn attach_typed_tree(
    syntax: UnattachedSyntax,
    identities: SyntaxIdentityMap,
    snapshot: SyntaxSnapshotId,
    document: Arc<SourceDocument>,
) -> Result<Arc<SyntaxSnapshotData>, AttachmentFailure>;
```

The attachment table is immutable and published only with the committed `ParsedSource`.

## 5. Session, lineage, snapshot, and node identity

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxDatabaseId(NonZeroU64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxLineageId {
    database: SyntaxDatabaseId,
    ordinal: NonZeroU64,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxSnapshotId {
    lineage: SyntaxLineageId,
    source: SourceSnapshotId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxNodeId {
    lineage: SyntaxLineageId,
    slot: NonZeroU64,
}
```

All fields and raw constructors are private. These types implement no Serde traits.

`SyntaxDatabaseId` is allocated by a process-local nonzero atomic allocator when a `SyntaxDatabase` is created. Exhaustion returns `SyntaxDatabaseCreateError::IdentityExhausted`; it never wraps. A lineage ordinal is staged on the first successful initial parse for one accepted `SourceName`/`SourceDocumentId` pair and consumed only at commit. A node slot is monotonically allocated within one lineage and never reused.

Including the lineage in `SyntaxNodeId` makes equality safe across independent databases even when their local slot counters both begin at one.

`SourceSnapshotId` remains visible inside `SyntaxSnapshotId` and remains the sole source generation authority. `SyntaxDatabaseId` and `SyntaxLineageId` are only session-local resolution authorities.

## 6. Immutable parsed snapshot and typed handles

### 6.1 Public ownership

```rust
#[derive(Clone)]
pub struct ParsedSource(Arc<ParsedSourceData>);

struct ParsedSourceData {
    syntax: Arc<SyntaxSnapshotData>,
    document: Arc<SourceDocument>,
    diagnostics: Arc<[SyntaxDiagnostic]>,
    status: ParseStatus,
    stats: SyntaxParseStats,
}

#[derive(Clone)]
pub struct TypedSyntaxTree(AstNode<SourceFileKind>);

#[derive(Clone)]
pub struct AstNode<K: AstKind> {
    snapshot: Arc<SyntaxSnapshotData>,
    id: SyntaxNodeId,
    marker: PhantomData<fn() -> K>,
}

#[derive(Clone)]
pub struct SyntaxNodeHandle {
    snapshot: Arc<SyntaxSnapshotData>,
    id: SyntaxNodeId,
    node: SyntaxNode,
}
```

`AstKind` is a public sealed trait implemented only by syntax-owned zero-sized marker types. `AstNode` constructors are crate-private in `attachment.rs`. Moving or cloning a typed value retains the same immutable snapshot `Arc`; there is no detached form and no forgeable identity.

### 6.2 Equality

`AstNode<K>::Eq` compares exact `SyntaxSnapshotId`, `SyntaxNodeId`, and marker kind. It does not recursively compare content.

Cross-revision identity comparison is explicit:

```rust
impl<K: AstKind> AstNode<K> {
    pub fn is_same_reconciled_node(&self, other: &Self) -> bool;
}
```

It returns true only when lineage and `SyntaxNodeId` match. Structural comparison is explicit and snapshot-owned:

```rust
impl ParsedSource {
    pub fn subtree_equivalent<A: AstKind, B: AstKind>(
        &self,
        left: &AstNode<A>,
        right: &AstNode<B>,
    ) -> Result<bool, SyntaxLookupError>;
}
```

It compares non-trivia grammar shape and never changes identity semantics.

### 6.3 Direct lookup and round trip

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
}
```

`bind_rowan` accepts only a red node whose root green allocation is the exact root allocation retained by the snapshot and whose event path is present in the snapshot index. Structurally equal nodes from another parse are rejected as `ForeignRowanRoot`. Exact handle-to-snapshot validation uses `resolve_exact`/`resolve_exact_syntax`, which can report both expected and actual snapshot IDs. Callers normally traverse `SyntaxNodeHandle`, which keeps the snapshot authority attached; arbitrary Rowan construction is not an attachment API.

No API accepts a source range, source text, integer slot, traversal index, or local extension trait to recover typed identity.

## 7. Lookup errors

```rust
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SyntaxLookupError {
    #[error("syntax handle belongs to another syntax database")]
    WrongDatabase {
        expected: SyntaxDatabaseId,
        actual: SyntaxDatabaseId,
    },
    #[error("syntax handle belongs to another source lineage")]
    WrongLineage {
        expected: SyntaxLineageId,
        actual: SyntaxLineageId,
    },
    #[error("syntax handle belongs to another immutable snapshot")]
    WrongSnapshot {
        expected: SyntaxSnapshotId,
        actual: SyntaxSnapshotId,
    },
    #[error("rowan node is not rooted in this immutable syntax snapshot")]
    ForeignRowanRoot {
        expected: SyntaxSnapshotId,
    },
    #[error("syntax generation is stale")]
    StaleGeneration {
        current: SourceGeneration,
        supplied: SourceGeneration,
    },
    #[error("syntax node kind mismatch")]
    KindMismatch {
        id: SyntaxNodeId,
        expected: SyntaxKind,
        actual: SyntaxKind,
    },
    #[error("syntax node is not identity-bearing")]
    NotIdentityBearing { kind: SyntaxKind },
    #[error("syntax node does not exist in this snapshot")]
    MissingNode { id: SyntaxNodeId },
    #[error("unbound fragment has no source-backed syntax identity")]
    UnboundFragment,
}
```

`ParsedSource::resolve_exact` and `resolve_exact_syntax` return `WrongSnapshot` for a valid handle from another immutable snapshot. `syntax_node(id)` and `typed_node(id)` intentionally resolve the stable ID in the receiver snapshot and therefore do not infer a supplied generation from an ID that carries no snapshot. `bind_rowan` returns `ForeignRowanRoot` when a raw Rowan node is not rooted in the receiver's exact green allocation. `SyntaxDatabase::resolve_current` returns `StaleGeneration` for an older generation in the same lineage. Cross-database and cross-lineage checks run before raw slot or kind lookup.

## 8. Grammar-node reconciliation

### 8.1 Shape

For each identity-bearing node, `incremental::shape` computes:

```rust
struct NodeShape {
    kind: SyntaxKind,
    role: SyntaxRoleClass,
    own_non_trivia_digest: [u8; 16],
    ordered_child_role_digest: [u8; 16],
    recovery_class: RecoveryClass,
}
```

The own digest includes semantic token kind and exact non-trivia token bytes directly owned by the node. It excludes whitespace and ordinary comments. Documentation tokens are semantic because documentation ownership is observable. Child digests include identity-bearing descendants by role and kind, not their numeric IDs.

### 8.2 Parent authority

The parent is the nearest identity-bearing grammar node and its exact field role. `LogicalLine`, `IndentedSuite`, `FenceBody`, and list wrappers are skipped. There is no parallel brace/indent parent map after the migration.

### 8.3 Deterministic matching order

Within each old/new parent-role bucket:

1. retain the root ID when both roots exist in the same lineage;
2. match unique exact full-subtree shapes, independent of sibling order;
3. match remaining equal own-shapes by stable longest common subsequence;
4. match remaining equal shapes by minimum absolute old/new ordinal distance;
5. break equal distance by lower old `SyntaxNodeId` slot;
6. allocate fresh IDs for every unmatched new node;
7. retire every unmatched old node from the current identity map; slots remain permanently unavailable.

A changed node receives a fresh ID. Its unchanged identity-bearing children may still retain IDs only through a unique replacement bridge: old and new parent must have the same `SyntaxRoleClass`, be the sole unmatched parent candidates under the same identity-bearing grandparent role, and their child-role partitions must be unambiguous. This preserves unaffected nested children without treating text ranges as identity.

### 8.4 Observable cases

- trivia-only edit: all semantic grammar IDs retained; ranges/spans update in the new snapshot;
- unique same-parent reorder: IDs follow nodes;
- repeated identical same-parent reorder: stable LCS, then distance, then old-ID tie;
- cross-parent move: moved node and all source-keyed descendants whose semantic parent changes receive fresh IDs;
- copy: the best matched original retains its ID; every additional copy receives fresh IDs;
- changed kind: old identity retires and new kind receives fresh identity;
- equivalent recovery: retained only when recovery class, expected token, parent role, and deterministic match agree;
- missing child that remains missing: may retain ID under the same expected role;
- filling a missing child: missing node retires and real node receives fresh ID;
- generic `ErrorNode`: stable only under equal recovery class and same role; source spelling alone never matches it across parents.

## 9. Parse transactions and APIs

### 9.1 Canonical bound document APIs

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
}
```

`parse_source` becomes the exact thin facade below and cannot create a fake lineage:

```rust
pub fn parse_source(
    database: &mut SyntaxDatabase,
    source_snapshot: SourceSnapshotId,
    document: Arc<SourceDocument>,
    options: ParseOptions,
) -> Result<ParsedSource, ParseFailure>;
```

It delegates to `SyntaxDatabase::parse_initial`. `parse_document` and the detached source-backed `ParsedSource` constructor are deleted.

A byte-identical reparse or empty edit list returns a `ParsedSource` wrapper sharing the exact current snapshot `Arc`, does not advance generation, and does not rerun attachment.

### 9.2 Standalone fragments

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnboundFragment<K: FragmentKind> {
    text: Arc<str>,
    tree: FragmentTree,
    diagnostics: Arc<[FragmentDiagnostic]>,
    completion: ParseCompletion,
    marker: PhantomData<fn() -> K>,
}

#[derive(Clone)]
pub struct AttachedFragment<K: FragmentKind> {
    snapshot: Arc<SyntaxSnapshotData>,
    root: AstNode<K::AstKind>,
}

pub fn parse_expression_fragment(text: &str, options: ParseOptions)
    -> UnboundFragment<ExpressionFragment>;
pub fn parse_type_fragment(text: &str, options: ParseOptions)
    -> UnboundFragment<TypeFragment>;
pub fn parse_pattern_fragment(text: &str, options: ParseOptions)
    -> UnboundFragment<PatternFragment>;
pub fn parse_statement_fragment(text: &str, options: ParseOptions)
    -> UnboundFragment<StatementFragment>;

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

Attachment requires `completion() == ParseCompletion::Complete`, `span.source()` equal to the exact document identity, and the span bytes exactly equal the retained fragment bytes. Incomplete/invalid fragments return `AttachmentFailure::FragmentNotComplete`; a byte mismatch returns `AttachmentFailure::FragmentTextMismatch`. The attached fragment is its own syntax lineage and cannot be passed as a whole source file. `LoweringRequest` accepts only `&ParsedSource` whose root marker is `SourceFileKind`, making accidental fragment/document interchange a compile error.

Compiler callbacks and body parsers must either parse as part of the owning document cursor or accept an explicitly attached fragment. They may not retain a raw body string for later parsing.

## 10. Limits and atomicity

The syntax transaction applies these inclusive limits:

| Limit | Maximum |
|---|---:|
| prefix depth | 64 |
| assertion conditions | 64 |
| predicate parameters | 64 |
| proof parameters | 64 |
| total requires plus ensures clauses per item | 64 |
| generic parameters per item | 256 |
| where predicates per item | 256 |
| top-level items | 16,384 |
| statements | 65,536 |
| expressions | 262,144 |
| type nodes | 131,072 |
| pattern nodes | 131,072 |
| identity-bearing grammar nodes | 1,048,576 |
| diagnostics | 1,024 |

Malformed/missing identity-bearing children count toward the corresponding node and identity budgets. The sixty-fourth parameter/clause/condition and the exact global maxima succeed. One over is a fatal `ParseFailure::LimitExceeded` and commits nothing.

A private `SyntaxTransaction` stages the new document, source generation, event stream, tree, diagnostics, reconciled map, allocator next value, attachment table, statistics, and cache entries. Only `commit` mutates `SyntaxDatabase`. Fatal lexing, event validation, losslessness failure, attachment mismatch, source mismatch, stale input, limit overflow, generation exhaustion, database/lineage/node identity exhaustion, or invariant failure leaves the current snapshot and every staged counter untouched.

## 11. Exact end-to-end call sequence

```rust
let document = Arc::new(SourceDocument::try_new(id, name.clone(), bytes)?);
let source_snapshot = SourceSnapshotId::initial(name);
let mut syntax_db = SyntaxDatabase::try_new()?;

let parsed = syntax_db.parse_initial(
    source_snapshot,
    Arc::clone(&document),
    ParseOptions::default(),
)?;

let predicate = parsed
    .tree()
    .items()
    .find_map(ItemNode::into_predicate)
    .expect("fixture predicate");

let mut hir_db = HirDatabase::try_new()?;
let lowered = hir_db.lower(LoweringRequest::try_new(module_key, &parsed)?)?;
let hir = lowered.module();
let item_id = hir.item_for_syntax(predicate.id())?;
let item = hir.resolve_item(item_id)?;
let invalidations = lowered.invalidations();
```

Internal transaction sequence:

```text
SourceDocument bytes
  -> validate SourceSnapshotId/document name
  -> lex once
  -> emit one grammar event stream
  -> validate/build one lossless Rowan tree
  -> compute grammar roles/shapes
  -> reconcile/allocate SyntaxNodeId values
  -> attach sealed typed handles
  -> enforce all limits and invariants
  -> commit ParsedSource atomically
  -> validate LoweringRequest database/lineage/generation/module key
  -> stage HIR arenas/scopes/locals/captures/liveness
  -> commit immutable HirModule atomically
  -> resolve typed HIR ID
```

No fatal parse or attachment failure publishes a source generation, typed tree, node ID, diagnostic vector, or cache entry.

## 12. Required migration boundary

The grammar tree and attachment path are implemented privately while all source-backed HIR continues to use the old public path. The public switch is one workspace-compiling change that simultaneously:

- changes `parse_source`/compiler facades to return bound `ParsedSource`;
- migrates all typed wrappers to `AstNode` handles;
- migrates expressions, statements, patterns, types, predicate/proof bodies, formatter, LSP, and test builders;
- changes HIR source keys to grammar `SyntaxNodeId`; and
- deletes `CstLineEvents`, detached `TypedSyntaxTree` construction, line semantic-parent identity, and source-backed substring parsing.

No completed or reviewable cut may lower some HIR nodes from line IDs and others from grammar IDs.
