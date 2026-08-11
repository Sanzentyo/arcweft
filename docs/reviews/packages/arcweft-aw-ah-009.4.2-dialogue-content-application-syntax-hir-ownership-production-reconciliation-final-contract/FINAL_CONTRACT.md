# Final contract and precedence

## 1. Authority

This package is the final AW-AH-009.4.2 production-reconciliation contract. It
supersedes only the contradictory dialogue-specific source/HIR clauses
identified by the request. It does not supersede validated general substrate.

Precedence is fixed as follows:

1. this package for dialogue-content application CST, AST, recovery, typed HIR,
   source component mapping, and direct migration;
2. AW-AH-009.3.1 for ordinary `CallExpr`, callback-block calls,
   `CallSurfaceSyntax`, `ArgumentListSyntax`, and missing-`)` recovery;
3. proof-concurrency v6.1.1 for source-backed identity, typed HIR arenas,
   lexical scopes, transactional lowering, immutable snapshots, and project
   structure;
4. AW-AH-009.3.2 for the single accepted `Arc<HirProject>` request lifecycle;
5. AW-AH-009.4 Cut 1 for `RuntimeNominalRecordValue`, `CharacterDialogue`,
   `CharacterDialogueConfig`, patch behavior, role newtypes, limits, and
   runtime schema;
6. AW-AH-009.4.3 for final line-ID materialization, project collision policy,
   text-key acceptance, and diagnostics owned by that cut.

No compatibility interval changes this order.

## 2. Machine result

```text
STATUS=READY_FOR_IMPLEMENTATION
OPEN_RESULT_CHANGING_DECISIONS=0
PRODUCTION_CHANGES_INCLUDED=0
BASELINE_GIT=e6e8cce33d4c09a9f9efa9ba2169fc5c6b0b7139
```

## 3. Required-decision ledger

### D-001 — one lossless postfix CST

Every `[` after a completed expression emits one
`PostfixBracketExpression`. It owns one target child, one open-delimiter child,
one payload child owning every interior token once, one present or missing
close-delimiter child, and at most one attached line-plan child. The CST does
not select index or dialogue meaning.

### D-002 — expression-start collection remains separate

A `[` parsed in prefix/expression-start position continues through the existing
collection/sequence grammar. It is never wrapped in a postfix node.

### D-003 — bounded typed classification

Typed syntax performs exactly two candidate attempts over the retained payload:
one full ordinary expression and one full existing `DialogueContent`. There is
no candidate list and no third interpretation.

### D-004 — classification has no spelling or type authority

CST construction and candidate classification may inspect grammar position,
token kinds, delimiters, indentation, and parser-owned syntax roles. They may
not inspect identifier text, `.say`, Character identity, callable identity,
resolved type, alias, or source substring.

### D-005 — exact AST outcomes

One viable candidate becomes `Expr::Index` or
`Expr::DialogueContentApplication`. Two viable candidates become
`Expr::PostfixBracket` with exactly two typed candidates. No viable candidate
becomes `Expr::PostfixBracket` with exactly two failure summaries. A missing
close delimiter changes recovery/poison state, not candidate meaning.

### D-006 — colon is dialogue-only source syntax

At an expression-statement or tail-expression owner boundary, a completed
expression followed by a colon may form a colon dialogue-content application.
It never fabricates bracket tokens and never creates an index candidate.
Record, type, label, named-argument, and other colon owners retain precedence in
their existing grammar positions.

### D-007 — one semantic application meaning

Clean bracket and colon dialogue forms both lower to
`HirExprKind::DialogueContentApplication`. Their exact source surfaces remain
distinct in the source component map.

### D-008 — byte indentation

Colon indentation is measured as the count of leading ASCII space and tab
bytes. A tab contributes one byte unit; no tab-stop or display-column expansion
exists. Mixed space/tab indentation is valid and compared byte-for-byte by
width.

### D-009 — blank/comment line policy

Blank and comment-only lines are retained in the raw body but neither establish
an indented body base nor cause dedent. The first meaningful line wider than
the head establishes the full base prefix.

### D-010 — misalignment policy

After a base exists, a meaningful line wider than the head but narrower than
the base remains in the body, records one typed misalignment issue, and poisons
the application. It is not silently normalized.

### D-011 — exact plan attachment

Only the keyword token `with`, at the application owner's top level and
followed by `:` or `{`, may attach a plan. It must be on the same line after
horizontal trivia or on the immediately following physical line at exactly the
head indentation, with no blank/comment-only line between. For indented colon
content it may be the first dedented line at head indentation. A bare block
never attaches.

### D-012 — ordinary call substrate is unchanged

A target such as `alice(look=smile)[content]` owns exactly one existing
`CallExpr`; its argument list remains parser-owned AW-AH-009.3.1 syntax. No
dialogue-only argument parser, second call AST, source-less constructor, or
adapter surface is introduced.

### D-013 — existing `IdRef` becomes the expression carrier

`Expr::EntityRef` directly carries the repository-owned `IdRef`. The obsolete
expression-only `EntityRefSyntax` carrier is deleted after callers migrate.
Absolute, relative, and family-relative source values remain distinct typed
variants.

### D-014 — coordinates are immediate and ordered

Only named arguments `id` and `text_key` on the immediate outer target
`CallExpr` become dialogue coordinates. They are retained in authored argument
order by checked ordinal, including duplicates. Nested target calls, positional
arguments, and identically named nested record fields are not coordinates.

### D-015 — coordinate value authority is typed HIR

Every coordinate retains its value `ExprId`. A compile-time ID value is present
only when that typed expression is `HirExprKind::EntityReference(HirIdRef)`
after following only the existing transparent grouping edge. Any other clean
expression is a runtime expression. An error expression remains an error. No
source read, text comparison, or fabricated `IdRef` occurs.

### D-016 — one expression arena

Dialogue applications, unresolved postfix brackets, their targets, ordinary
calls, interpolations, plan expressions, and candidate-only expressions all
use the accepted `HirModule` expression arena and `ExprId`. Callable and flow
bodies do not have separate dialogue storage.

### D-017 — exact HIR variants

The final enum adds or retains only these relevant variants in the original
`HirExprKind` implementation:

```rust
EntityReference(HirIdRef),
Index { target: ExprId, index: ExprId },
DialogueContentApplication(HirDialogueContentApplication),
PostfixBracket(HirPostfixBracket),
```

`HirExprKind::DialogueCall` is deleted. There is no parallel dialogue arena or
flow sidecar.

### D-018 — source-backed root, shared target, synthetic candidates

A bracket or colon application root is source-backed by its exact
`SyntaxNodeId`. Its target subtree retains normal source-backed IDs. When a
bracket is ambiguous, the index and dialogue candidate roots and candidate-only
children receive deterministic synthetic `ExprId` values owned by the source
root, while sharing the same target `ExprId`.

### D-019 — no ranges in semantic payloads

The HIR slot metadata owns the whole expression `SourceSpan`. A transactionally
published component map owns target, delimiters, colon, content, raw body,
plan, and coordinate-part sites. HIR semantic payloads do not retain
`TextRange`, syntax AST clones, source strings, or display spelling.

### D-020 — insertion sites are typed

Missing close delimiters and missing content map to a checked
`HirInsertionPoint`, not a fabricated zero-length `SourceSpan`.

### D-021 — scope behavior

The application expression and all dialogue interpolations use the current
lexical scope. The application creates no scope. An attached line plan creates
one child existing `Block` scope; nested thread, handler, branch, block, and
closure constructs follow their existing scope rules.

### D-022 — poison and executable gating

Recovered syntax publishes a recovered/poisoned immutable HIR snapshot for
tooling but cannot enter executable sema, verifier, runtime-plan, codegen, or
cache paths. A clean ambiguous postfix remains a valid HIR/tooling node, but an
executable checked product requires a sema resolution fact keyed by its root
`ExprId`.

### D-023 — exact bounded work

Each postfix bracket performs one CST emission pass and at most two candidate
passes. It retains no more than two candidate results and no more than one final
failure summary per failed interpretation. Existing syntax/HIR limits are
charged; no client-configurable dialogue limit is added.

### D-024 — fatal transaction boundary

Budget exhaustion, checked-arithmetic failure, invalid UTF-8 boundary,
invalid node identity, impossible range containment, or arena/source-map
invariant failure aborts the owning syntax or HIR transaction and publishes
nothing. These are internal failures, not user diagnostics.

### D-025 — direct deletion

`SpeakerLine`, `SpeakerLineSurface`, string `ContentCall`,
`Expr::DialogueCall`, `DialogueCallExpression`, `HirDialogue`,
`HirExprKind::DialogueCall`, post-parse bracket/call source search, and
speaker-derived HIR fields are removed through one public direct replacement.
No alias, wrapper, deprecated variant, dual reader, extension trait, source
fallback, or removed-spelling recognizer remains.

### D-026 — no unrelated route

This cut introduces no CSS route, Takumi route, top-level hook/memo/parser/
reducer/state syntax, borrow-block syntax, `.say` syntax, line-ID policy,
runtime wire change, Character runtime redesign, or text-rendering change.

## 4. Final grammar ownership

The normative ownership sketch is:

```text
prefix `[`                 -> existing collection expression
completed expression `[`  -> one generic postfix-bracket CST
completed expression `:`  -> colon dialogue application only at owner boundary
postfix one candidate      -> direct typed AST variant
postfix two candidates     -> unresolved generic typed AST/HIR variant
postfix zero candidates    -> invalid generic typed AST/HIR variant
```

Candidate selection from source grammar is not semantic resolution. Sema may
resolve an ambiguous clean bracket only from typed target/payload judgments and
records the result without mutating or reallocating HIR.

## 5. Final readiness statement

All result-changing choices requested by AW-AH-009.4.2 are fixed by this
package. The implementation assignee has no authority to choose an alternate
indentation metric, candidate count, AST shape, HIR owner, source-map owner,
coordinate carrier, recovery boundary, migration strategy, or test outcome.
