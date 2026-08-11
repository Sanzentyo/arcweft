# Production reconciliation

## 1. Re-audited basis

The contract was reconciled against private repository `Sanzentyo/arcweft` at current `main` commit:

```text
8984661d5679efccf7a16255f921530cd0b7cacc
```

The current head is the repository audit commit immediately after the request's re-audit basis `328e362f811896ebf866002c458fe0b970976654`. Jujutsu metadata was not available through the repository connector, so the inspected Jujutsu change is recorded as `unavailable`.

The parent AW-AH-009.3 package identity is:

```text
arcweft-aw-ah-009.3-character-nominal-signature-help-final-contract.zip
cdd1d7b764da238a6e4e8f3e774a3384017c8da5ffaea1969f2af279102a7cd5
```

The governing reconciliation request reproduces the result-changing parent clause and freezes the parent policies that this request must not redesign.

## 2. Current contradiction

Current source syntax has one source-less semantic shape:

```rust
Expr::Call {
    callee: Box<Expr>,
    args: Vec<CallArg>,
}
```

Its producers do not share one authored delimiter surface:

| Current producer | Authored syntax | Exact range facts currently available | Why the parent shape fails |
|---|---|---|---|
| Pratt `(` postfix | `callee(args)` | Lexer has parens, commas, and token spans, but AST discards them | Can satisfy `ArgumentListSyntax` only after parser retention is implemented |
| Selected parenthesized call | `target.member(args)` | Same as ordinary postfix | Same loss as ordinary call |
| Postfix callback block | `target.member { params => body }` | Lexer has braces, parameters, arrow, and body spans | Has no parentheses or call-argument commas; an `ArgumentListSyntax` would be false |
| Static-generic rescue helper | generic callee followed by `(...)` | Source-scanning helper reconstructs text after the Pratt parser | Violates parser-owned ranges and no after-the-fact search |
| Dialogue/content call head | `alice.say(args)[...]` | Parser retains raw argument text/base, then splits it | Needs the same exact list carrier without manufacturing an ordinary call node |
| Colon speaker head | `alice(args): ...` | Surface retains only the trimmed interior range | Loses the authored parens and separators required for cursor behavior |
| Public constructors and direct tests | `Expr::call`, `Expr::selected_call`, struct literals | No source document or token ranges exist | Cannot provide exact authored ranges without fabrication |

The proof-concurrency shadow parser does not repair this semantic source-AST ownership. It remains a separate substrate and is not selected as a dependency.

## 3. Selected reconciliation

The parent contract's uniform `CallExpressionSyntax { argument_list: ArgumentListSyntax, ... }` assumption is replaced by:

```rust
Expr::Call(CallExpr)

CallExpr {
    callee,
    args,
    syntax: CallSurfaceSyntax,
}

CallSurfaceSyntax::Parenthesized(ParenthesizedCallSyntax)
CallSurfaceSyntax::CallbackBlock(CallbackBlockCallSyntax)
```

The parenthesized branch owns the exact `ArgumentListSyntax`. The callback branch owns exact callback syntax. This is one semantic call model and one exhaustive authored-surface model, not a dual AST.

The solution is intentionally not:

- two semantic `Expr` variants with duplicated call checking/lowering;
- one ordinary call with `Option<ArgumentListSyntax>`;
- one generalized delimiter string or delimiter-kind field;
- a callback encoded as a one-element parenthesized list;
- a generated/fake source surface;
- a post-parse source search.

## 4. Producer-by-producer resolution

### 4.1 Pratt parenthesized calls

The Pratt parser carries each parsed expression with its exact source range. When it consumes an authored `(`, it records token spans while parsing arguments and constructs `ArgumentListSyntax` before creating `CallExpr`.

For a complete call, the list stores the authored `)`. For a recoverable missing `)`, the list stores a typed missing terminator with an exact insertion point and the owner boundary that stopped parsing. The outer owner token is not consumed.

### 4.2 Postfix callback blocks

The callback parser constructs the existing semantic `Expr::Closure`, then constructs `CallbackBlockSyntax` from the same lexer tokens. `CallExpr::try_callback_block` creates exactly one positional closure argument and validates the syntax/closure correspondence.

The outer callback application is not a signature-help surface. Calls nested in its body retain their own parenthesized surfaces.

### 4.3 Static generic calls

`parse_static_generic_call` is deleted. The existing path/generic Pratt grammar is extended to accept the current valid static generic call spellings directly, retaining token spans through ordinary call construction. Invalid spellings fail through the normal expression grammar. No source spelling recognizer remains.

### 4.4 Dialogue and speaker surfaces

The shared token-level parenthesized-list parser is reused by:

- ordinary expression calls;
- `alice(args): ...` speaker heads;
- `alice.say(args)[...]` content-call heads;
- existing View/dialogue/line-plan argument positions that currently call source-lossy expression helpers.

Speaker and content nodes own `ArgumentListSyntax` directly on their special-form surfaces. They are not converted to synthetic ordinary calls.

### 4.5 Direct and generated construction

Public source-AST call constructors are deleted. Tests that exercise source syntax parse text. Current executable generators use `RuntimeExpr::Call`, which is the selected non-authored semantic representation and does not claim source syntax.

## 5. Recovery ownership

The parser's owner supplies the expression extent and stopping boundary. A missing parenthesized close stores one of:

- `EndOfExpression` at the exact owner-supplied end; or
- a typed authored token boundary whose range begins at the insertion position.

This prevents a call from consuming a speaker colon, callback arrow, outer comma, semicolon, `)`, `]`, or `}`. The owner resumes from that token.

An isolated malformed argument becomes one recovered argument slot only when a nonempty token segment exists. Empty slots and missing separators remain errors. No recovery range is invented.

## 6. HIR and semantic reconciliation

Current HIR stores syntax `Expr` directly. The new immutable types clone through that boundary without a parallel HIR representation.

Every semantic consumer that only needs call meaning uses `CallExpr::callee()` and `CallExpr::args()`. The one AW-AH-009.3 signature resolver additionally requires a parenthesized projection. Callback applications terminate dispatch as `NotApplicable` before candidate resolution or cache lookup.

This preserves one checker, one callable resolver, one native/Rust-adapter precedence policy, and one cache policy.

## 7. Parent clauses retained unchanged

This reconciliation does not change:

- `CharacterNominalType` identity, display, aliases, or parameter classification;
- `RegisteredSemanticWorld`, accepted profile publication, generation identity, or failure preservation;
- `SourceDocumentIdentity` versus `SourceSnapshotId` separation;
- checked LSP UTF-16/UTF-8 position conversion;
- native versus Rust-adapter precedence;
- callable candidate, overload, named/reordered/duplicate/spread, partial-call, or active-parameter semantic binding;
- stale checks, cache identity, cache invalidation, cancellation, work limits, diagnostic limits, or error ordering;
- the AW-AH-009.3.2 accepted-HIR/request-lifecycle design space;
- proof-concurrency typed-node identity;
- completion, hover, definition, rename, CSS/Takumi, or removed borrow-block behavior.

The only parent clause replaced is the assertion that every semantic call owns a non-optional parenthesized argument list.

## 8. No compatibility interval

The old struct-like variant and public constructors are removed in the same unmerged direct-replacement series that migrates all consumers. Main never contains both old and final public call models. No alias, deprecated function, optional fallback, or source-less parser constructor is committed.

## 9. Readiness conclusion

The contradiction has one selected owner model, one parenthesized carrier, one callback carrier, one explicit callback signature-help outcome, one parser recovery rule, one generated-expression representation, and one direct migration. There are no result-changing open decisions and no fabricated source range.
