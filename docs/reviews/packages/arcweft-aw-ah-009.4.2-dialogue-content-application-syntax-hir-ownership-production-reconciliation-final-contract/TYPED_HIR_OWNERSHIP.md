# Typed HIR expression ownership and source mapping

## 1. Arena authority

The final implementation uses the accepted proof-concurrency v6.1.1
`HirDatabase`, immutable `HirModule`, source-backed/synthetic allocation keys,
`HirSlotMetadata`, typed arenas, lexical scopes, and lowering transaction.
Dialogue does not own another expression arena and Flow does not retain a
sidecar.

The relevant original enum is changed directly:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirExprKind {
    // existing typed variants
    EntityReference(HirIdRef),
    Call(HirCallExpr),
    Index { target: ExprId, index: ExprId },
    DialogueContentApplication(HirDialogueContentApplication),
    PostfixBracket(HirPostfixBracket),
    // remaining existing typed variants
}
```

`HirExprKind::DialogueCall` is deleted. Existing enum behavior is implemented
in its inherent match sites; no extension trait or free string conversion is
introduced.

## 2. Dialogue application payload

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDialogueContentApplication {
    target: ExprId,
    content: HirDialogueContent,
    plan: Option<HirLinePlan>,
    coordinates: Box<[HirDialogueCoordinate]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDialogueCoordinate {
    kind: HirDialogueCoordinateKind,
    argument: HirCallArgumentOrdinal,
    value: ExprId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDialogueCoordinateKind {
    Id,
    TextKey,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirCallArgumentOrdinal(u16);
```

Fields are private; construction is `pub(crate)` and checked; accessors are
read-only. `HirCallArgumentOrdinal::try_new` rejects values outside the existing
128-argument ordinary-call limit. Coordinates are strictly increasing by
ordinal and may repeat kinds.

`HirDialogueContent` is the direct typed projection of the retained existing
`DialogueContent` variant set. Text/RichText/control/line-break semantics are
unchanged. Every expression-valued interpolation or control field is an
`ExprId` in the same module arena. It stores no syntax AST, raw content string,
`TextRange`, or source display authority.

## 3. Typed line plan inside the payload

A line plan remains inline semantic data owned by the application; it is not a
new expression arena. Every expression, statement, and pattern child is an ID
in the existing typed arenas.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirLinePlan {
    root_scope: ScopeId,
    label: Option<HirName>,
    items: Box<[HirLinePlanItem]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirLinePlanItem {
    Init(Box<[StmtId]>),
    Thread(StmtId),
    On(StmtId),
    Option { name: HirName, value: ExprId },
    Let { pattern: PatternId, value: ExprId },
    Statement(StmtId),
    Out(ExprId),
    CancelRule(StmtId),
    TimedCue { anchor: ExprId, body: ExprId },
    StartGroup(Box<[HirLinePlanItem]>),
    TogetherGroup(Box<[HirLinePlanItem]>),
    TimelineAssert {
        policy: TimelineAssertPolicy,
        condition: ExprId,
    },
    Expression(ExprId),
    Error(StmtId),
}
```

This enum is an isomorphic typed projection of the current `LinePlanItem` set:
`Raw` becomes a poisoned existing `HirStmtKind::Error`; Thread, On, and
CancelRule lower through their original `HirStmtKind` implementations; group
ordering is source order. `BlockStyle` is syntax display data and is omitted
from semantic HIR. The existing label behavior is retained.

The line plan allocates one child existing `Block` scope. Nested constructs use
their existing scope kinds. A malformed plan can still allocate a poisoned
`HirLinePlan` for tooling, but no executable product consumes it.

## 4. Unresolved postfix payload

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirPostfixBracket {
    target: ExprId,
    candidates: HirPostfixBracketCandidates,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirPostfixBracketCandidates {
    Ambiguous {
        index: ExprId,
        dialogue: ExprId,
    },
    Invalid {
        index: HirPostfixCandidateFailure,
        dialogue: HirPostfixCandidateFailure,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirPostfixCandidateFailure {
    kind: HirPostfixCandidateFailureKind,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPostfixCandidateFailureKind {
    EmptyPayload,
    UnexpectedToken,
    MissingOperand,
    TrailingToken,
    InvalidDialogueAtom,
}
```

For `Ambiguous`, `index` must resolve to an `HirExprKind::Index` whose target is
exactly `HirPostfixBracket::target`; `dialogue` must resolve to an
`HirExprKind::DialogueContentApplication` with that same target. Constructors
reject any mismatch. Candidate roots are ordinary `ExprId` values and carry
normal slot metadata/poison state.

For `Invalid`, no candidate expression IDs are fabricated. The two failure
summaries are bounded typed facts for tooling/diagnostics and contain no source
range; their exact sites remain in the component source map/diagnostic record.

## 5. Identity allocation

### 5.1 Source-backed roots

The application or generic postfix root uses the accepted source allocation
key:

```text
(SyntaxNodeId of the application root, HirIdKind::Expr)
```

Bracket and colon roots therefore retain independent source identities. The
target and every ordinary nested call child keep their normal source-backed
`ExprId` keys. No key contains a range, line number, item ordinal, source
string, or callee spelling.

### 5.2 Candidate-only IDs

Add these variants directly to the accepted repository-owned `SyntheticRole`
enum and its inherent implementations:

```rust
PostfixIndexCandidateExpression,
DialogueContentCandidateExpression,
```

For an ambiguous bracket, all candidate-only expression IDs use:

```text
owner = Expr(source-backed postfix root)
role = interpretation-specific role
ordinal = deterministic zero-based preorder within that interpretation
```

Ordinal zero is the candidate root. The shared target is excluded from both
inventories. Candidate-only statement/pattern children use the same owner and
interpretation role with their own `HirIdKind` and deterministic per-kind
preorder. A failed lowering transaction consumes no ID or ordinal.

A trivia-only source reconciliation retains these IDs when the source root and
typed candidate structure retain identity. Sema resolution does not reallocate
or rewrite them.

## 6. Lexical containment

The application root uses the current scope passed to ordinary expression
lowering:

- Flow expression statement: current Flow/body scope;
- ordinary function body: current Callable/body scope;
- closure body: current Closure scope;
- branch: the existing Conditional/MatchArm scope;
- block/tail: the existing Block scope;
- expression statement: the statement owner's current scope.

The application creates no scope. The target lowers in the same current scope.
Dialogue interpolations lower in that scope. The optional line plan allocates
one child Block scope as stated above. Candidate-only expressions use the same
scope they would have after selection; they do not receive a fake Flow scope.

## 7. Source component map

The whole expression span remains in `HirSlotMetadata`. Add the following roles
and site types to the existing HIR source-map owner:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirExprSourceRole {
    Whole,
    Target,
    OpenBracket,
    CloseBracket,
    Colon,
    Content,
    ContentBody,
    Plan,
    ConfigurationArgument {
        argument: HirCallArgumentOrdinal,
        part: HirCallArgumentSourcePart,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallArgumentSourcePart {
    Whole,
    Name,
    Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirSourceSite {
    Span(SourceSpan),
    Insertion(HirInsertionPoint),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirInsertionPoint {
    source: SourceDocumentIdentity,
    offset: usize,
}
```

`HirInsertionPoint::try_new` is crate-private and verifies document identity,
revision, `offset <= document.len()`, and UTF-8 boundary. It implements no
Serde.

Add the inherent API to `HirModule`:

```rust
impl HirModule {
    pub fn expr_source_site(
        &self,
        id: ExprId,
        role: &HirExprSourceRole,
    ) -> Result<Option<&HirSourceSite>, IdResolveError>;
}
```

`Whole` is projected from slot metadata; it is not redundantly inserted into
the component map. Every other present site is staged in the lowering
transaction and published atomically.

Role behavior is exact:

- `Target`: exact target expression span;
- `OpenBracket`: exact `[` span for bracket forms, absent for colon;
- `CloseBracket`: exact `]` span or insertion site, absent for colon;
- `Colon`: exact colon span for colon forms, absent for bracket;
- `Content`: semantic content bounding span or missing-content insertion;
- `ContentBody`: full bracket payload range or raw indented colon body; for
  inline colon it is the semantic content span plus retained in-line trivia
  only when those bytes are owned by the application;
- `Plan`: exact attached plan span or absent;
- `ConfigurationArgument`: exact existing AW-AH-009.3.1 whole/name/value range
  selected by ordinal.

All component sites must carry the same source document and revision as the
root. Source display strings have no authority.

## 8. Poison, recovery, and executable gating

- missing `)`, missing `]`, missing content, malformed content, indentation
  misalignment, malformed attached plan, malformed coordinate value, and
  invalid/no-candidate postfix poison the affected slots and make the HIR
  module `Recovered` under the accepted substrate;
- a clean two-candidate postfix is not a syntax recovery and may live in a
  clean HIR snapshot;
- sema records exactly one `PostfixBracketResolution::{Index, Dialogue}` keyed
  by the root `ExprId`, or emits an ambiguity/no-match typed diagnostic;
- the HIR snapshot is immutable; resolution does not mutate `HirExprKind`;
- an executable checked-project view requires a clean HIR module and a complete
  resolution for every reachable `HirExprKind::PostfixBracket`;
- verifier, runtime-plan, codegen, and executable caches consume only that
  checked view;
- LSP/tooling query the single accepted recovered or clean `Arc<HirProject>`
  and never invoke an on-demand parser/lowerer.

## 9. Clone/equality/serialization policy

Small payloads derive `Clone`, `Debug`, `Eq`, and `PartialEq`; ID/value enums
also derive hash/ordering as shown. The immutable module remains `Arc`-backed
under the accepted HIR contract. No syntax clone, source string, or Serde
implementation is added to `HirExprKind`, dialogue payloads, postfix payloads,
coordinates, source sites, or insertion points.
