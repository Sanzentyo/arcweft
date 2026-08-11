# Generic postfix-bracket CST and typed syntax AST

## 1. Lossless CST contract

### 1.1 New and removed syntax kinds

Add the following variants to the repository-owned `SyntaxKind` and
`SyntaxRole` enums and their inherent implementations:

```rust
SyntaxKind::PostfixBracketExpression
SyntaxKind::PostfixBracketPayload
SyntaxKind::DialogueContentApplicationExpression

SyntaxRole::Target
SyntaxRole::Payload
SyntaxRole::OpenDelimiter
SyntaxRole::CloseDelimiter
SyntaxRole::Plan
```

Reuse existing delimiter, missing-token, dialogue-content, and line-plan child
kinds where they already express the role. Delete
`SyntaxKind::DialogueCallExpression`. Postfix `IndexExpression` is no longer a
CST emission owner; index is a typed AST/HIR interpretation of the generic
postfix node. An expression-start collection keeps its existing CST kind.

### 1.2 Exact child order

A postfix bracket CST has exactly this logical order:

```text
PostfixBracketExpression
  Target: completed expression child
  OpenDelimiter: `[` child
  Payload: PostfixBracketPayload child
  CloseDelimiter: `]` child or one missing-token child
  Plan: optional existing LinePlan child
```

The target child retains its own `SyntaxNodeId`. The root, payload, and plan
also have normal accepted syntax identities. Every token and trivia item inside
`[` and `]` belongs exactly once to the payload child. Candidate overlays do
not create overlapping Rowan/token ownership.

A colon application has this logical order:

```text
DialogueContentApplicationExpression
  Target: completed expression child
  Colon delimiter child
  existing DialogueContent child or missing-content attachment
  optional existing LinePlan child
```

It contains no open/close bracket child.

## 2. Surface types

All fields are private. Types are public read-only syntax products; checked
constructors and invariant errors are crate-private. No type implements Serde.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedLinePlanSurface {
    syntax: SyntaxNodeId,
    range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostfixBracketSurface {
    syntax: SyntaxNodeId,
    target_syntax: SyntaxNodeId,
    payload_syntax: SyntaxNodeId,
    target_range: TextRange,
    open_bracket: TextRange,
    payload_range: TextRange,
    terminator: BracketTerminatorSyntax,
    plan: Option<AttachedLinePlanSurface>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BracketTerminatorSyntax {
    Closed { close_bracket: TextRange },
    RecoveredMissing {
        insertion: usize,
        boundary: PostfixBracketRecoveryBoundarySyntax,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PostfixBracketRecoveryBoundarySyntax {
    EndOfExpression { anchor: usize },
    LineEnding { range: TextRange },
    OwnerEnd { anchor: usize },
    Token {
        token: PostfixBracketBoundaryToken,
        range: TextRange,
    },
    PlanKeyword { range: TextRange },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PostfixBracketBoundaryToken {
    Comma,
    Semicolon,
    CloseParen,
    CloseBracket,
    CloseBrace,
    FatArrow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogueContentSite {
    Present { range: TextRange },
    Missing {
        insertion: usize,
        boundary: DialogueContentRecoveryBoundarySyntax,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogueContentRecoveryBoundarySyntax {
    CloseBracket { range: TextRange },
    MissingBracketClose { insertion: usize },
    Inline(DialogueInlineBoundary),
    Indented(DialogueDedentBoundary),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogueContentApplicationSurface {
    Bracket(BracketDialogueApplicationSurface),
    Colon(ColonDialogueApplicationSurface),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BracketDialogueApplicationSurface {
    bracket: PostfixBracketSurface,
    content: DialogueContentSite,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColonDialogueApplicationSurface {
    syntax: SyntaxNodeId,
    target_syntax: SyntaxNodeId,
    head_range: TextRange,
    colon: TextRange,
    content: DialogueContentSite,
    indentation: DialogueIndentation,
    plan: Option<AttachedLinePlanSurface>,
}
```

`PostfixBracketSurface::try_new` checks document membership, UTF-8 boundaries,
identity/child roles, target-before-open ordering, one-byte `[` range, payload
containment, exact present/missing close relation, plan ordering, and complete
root range derivability. `ColonDialogueApplicationSurface::try_new` checks a
one-byte colon, target/head agreement, content/indentation agreement, plan
agreement, and source ordering. Impossible relations are internal parser
failures.

The full source range is derived, never redundantly stored:

- start is `target_range.start()` or `head_range.start()`;
- end is the attached plan end when present;
- otherwise bracket end is the present close end or missing insertion;
- otherwise colon end is the inline content/boundary or indented body/boundary;
- every add/subtract is checked.

## 3. Typed AST variants

The repository-owned `Expr` enum is changed directly:

```rust
pub enum Expr {
    // existing variants retained unless directly superseded
    EntityRef(IdRef),
    Call(CallExpr),
    Index(IndexExpr),
    DialogueContentApplication(DialogueContentApplicationExpr),
    PostfixBracket(PostfixBracketExpr),
    // remaining existing variants
}
```

`Expr::DialogueCall` is deleted; it is not aliased or deprecated.

Exact new payloads:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexExpr {
    target: Box<Expr>,
    index: Box<Expr>,
    status: ApplicationRecoveryStatus,
    surface: PostfixBracketSurface,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueContentApplicationExpr {
    target: Box<Expr>,
    content: DialogueContent,
    plan: Option<LinePlan>,
    status: ApplicationRecoveryStatus,
    surface: DialogueContentApplicationSurface,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostfixBracketExpr {
    target: Box<Expr>,
    candidates: PostfixBracketCandidates,
    surface: PostfixBracketSurface,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PostfixBracketCandidates {
    Ambiguous {
        index: PostfixIndexCandidate,
        dialogue: PostfixDialogueCandidate,
    },
    Invalid {
        index: PostfixCandidateFailure,
        dialogue: PostfixCandidateFailure,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostfixIndexCandidate {
    index: Box<Expr>,
    status: ApplicationRecoveryStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostfixDialogueCandidate {
    content: DialogueContent,
    plan: Option<LinePlan>,
    status: ApplicationRecoveryStatus,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ApplicationRecoveryStatus {
    Clean,
    Recovered,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostfixCandidateFailure {
    kind: PostfixCandidateFailureKind,
    site: PostfixCandidateFailureSite,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PostfixCandidateFailureKind {
    EmptyPayload,
    UnexpectedToken,
    MissingOperand,
    TrailingToken,
    InvalidDialogueAtom,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PostfixCandidateFailureSite {
    Span(TextRange),
    Insertion(usize),
}
```

Every field has a read-only accessor. `try_new` constructors are crate-private,
validate exact child/surface agreement, and reject source-less construction.
There is no public constructor taking strings, raw ranges, or missing source
identity.

The target is stored once in `PostfixBracketExpr`. Candidate records hold only
the interpretation-specific payload. This is a bounded ambiguity carrier, not
a second AST or a clone of the target.

## 4. Candidate attempts and quality

The typed attachment builder opens two transaction-local cursor forks over the
same payload token interval:

1. the ordinary expression candidate succeeds only when one existing ordinary
   expression consumes every nontrivia payload token;
2. the dialogue candidate succeeds only when the existing `DialogueContent`
   grammar consumes every payload token and yields at least one dialogue atom,
   or yields the specified recovered missing-content result for an empty
   payload.

A dialogue atom is an existing dialogue text/RichText/control/interpolation/
line-break grammar product. Candidate classification does not turn an
arbitrary number token or punctuation sequence into raw text merely to create a
second candidate.

Each attempt returns `Clean`, `Recovered`, or one failure summary. Viability is
`Clean` or `Recovered`. Classification is exact:

| Index attempt | Dialogue attempt | AST result |
|---|---|---|
| viable | failure | `Expr::Index` |
| failure | viable | `Expr::DialogueContentApplication` |
| viable | viable | `Expr::PostfixBracket::Ambiguous` |
| failure | failure | `Expr::PostfixBracket::Invalid` |

A clean and a recovered success are still two viable interpretations and are
retained. Candidate-specific recovery diagnostics are staged and published
only with the resulting retained node; no losing failure emits a speculative
user diagnostic. A missing outer `]` independently marks the retained result
recovered.

`items[0]` has a clean ordinary number expression and no dialogue atom, so it
is an index candidate. A payload with dialogue controls, RichText,
interpolation, or a dialogue line break is a dialogue candidate. A token stream
accepted by both grammars remains ambiguous without callee/name lookup.

## 5. Incremental identity

The generic postfix root, target child, and payload child each retain accepted
`SyntaxNodeId` identity. Reconciliation keys use syntax structure and child
roles, never source range or display spelling. A trivia-only edit retains the
root when the accepted syntax reconciler retains its node; component ranges
update with the new source snapshot. Reclassification between index,
dialogue, and ambiguous AST outcomes does not create a new CST root.

Colon application identity is its own source node and never pretends to be the
bracket root. Both forms may later have the same semantic HIR kind while
retaining distinct source keys.

## 6. Plan attachment

The parser recognizes a plan suffix only after a dialogue interpretation is
viable. In an ambiguous bracket it belongs only to the dialogue candidate.

Attachment requires all of the following:

1. exact keyword token `with` at top level outside strings, RichText,
   interpolation, dialogue controls, and nested delimiters;
2. the application is the root of an expression statement or tail-expression
   owner;
3. `with` is followed after horizontal trivia by `:` or `{`;
4. it is either on the same physical line after horizontal trivia, or on the
   immediately following physical line with no intervening blank/comment-only
   line and with a prefix byte-for-byte equal to the application head prefix;
5. for indented colon content, it may be the first dedented line at the exact
   head prefix.

A blank line, comment-only intervening line, different prefix, deeper prefix,
shallower owner boundary, semicolon-terminated following statement, or missing
`:`/`{` prevents attachment. A bare `{ ... }` without `with` never attaches.
Malformed content after a valid `with:` or `with {` prefix is retained as the
existing recovered `LinePlan` and poisons the application.

For inline colon content, a same-line `with` suffix must be preceded by at
least one horizontal-trivia token and satisfy the exact keyword/delimiter rule.
The content candidate must consume the prefix before that token. This is a
grammar boundary, not source-string search.

## 7. Diagnostics

The old `syntax.expression.missing_dialogue_close` and the index-specific
postfix close ownership are directly replaced by one generic
`syntax.expression.missing_postfix_bracket_close`, emitted through the existing
missing-delimiter mechanism. Ordinary missing-`)` remains
`syntax.expression.missing_call_close`.

An ambiguous clean postfix emits no syntax diagnostic. Sema owns inability to
resolve it. An invalid postfix retains the generic node and emits at most one
final summary for each failed interpretation, plus already-budgeted nested
ordinary grammar diagnostics. No `.say` or removed-syntax diagnostic exists.
