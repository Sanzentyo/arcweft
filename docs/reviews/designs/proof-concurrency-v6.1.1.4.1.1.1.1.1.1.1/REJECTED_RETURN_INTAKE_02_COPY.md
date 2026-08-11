# Proof 01.1.1.4.1.1.1.1.1.1 Select authority return intake

Date: 2026-07-30

Status: `RETURNED_REJECTED_NOT_READY_FOR_IMPLEMENTATION`

## Archive identity and mechanical validation

The externally returned archive was inspected at:

```text
D:/sanze/Downloads/arcweft-proof-concurrency-v6.1.1.4.1.1.1.1.1.1-select-source-diagnostic-and-producer-authority-correction-final-contract.zip
```

- byte length: `53,285`;
- SHA-256:
  `531396561540A87D63A12861511B334494DB28A813D630C095323292E6CAB141`;
- package baseline: `e02aacd2ab1659bc49bc751b8615f1bb1c1968d3`;
- `26` unique, flat, safe ZIP members;
- `25` intentional non-manifest rows with exact byte lengths and SHA-256
  values;
- continuation request, primary request, and rejected-return intake copies are
  byte-identical to their repository sources;
- all `67` predecessor member-audit rows match the retained predecessor
  archives;
- `FINAL_STATUS.md` is exactly `READY_FOR_IMPLEMENTATION` plus a newline; and
- `OPEN_QUESTIONS.md` is exactly `none`.

The archive is mechanically valid. It is not copied into Git. This path and
digest are its retained identity.

## Decisions retained from the return

The following direction remains authoritative:

- a Select always has an authored target; leading `.member` remains
  `ShortVariant`, so missing Select target and its synthetic child are not
  current-grammar states;
- missing selected member remains the known Select family and is represented
  without fabricating a name;
- final source authority is slot-owned `Whole` plus the exact `Target` and
  `SelectedMember` components; `Recovery` is not applicable to Select;
- a recovery diagnostic remains unique by qualified `SyntheticOwner`, and a
  propagation-only parent never copies a child's terminal diagnostic;
- an independently missing member retains its Select-root diagnostic even when
  the authored target is already poisoned;
- `Expressions`, `TotalSlotsPerModule`, source-component publication,
  diagnostics, rollback, and retry remain separate checked authorities; and
- the eventual public switch is deletion-driven and may not retain a detached
  HIR reader, source reparse, alias, wrapper, or compatibility path.

The returned three-way `Name | Missing | Invalid` member schema is narrowed
below. The extra `Invalid` branch was accepted provisionally by the previous
intake, but the corrected producer audit proves it has no current-language
producer and duplicates no necessary final state.

## Adjudication

The repository rejects the package's self-status. Its parser fixtures,
attached owner, semantic meaning, source-query order, poison issue, and work
accounting cannot all be implemented through the accepted authorities.

### Compact double dots are ranges, not nested Selects

The lossless document lexer longest-matches `..` as one token in
`crates/arcweft-lang-syntax/src/parser/lexer.rs`. The shadow Pratt parser then
routes that token to `RangeExpression` in
`crates/arcweft-lang-syntax/src/parser/expression.rs`.

Consequently, `target..member` and `target..` do not create two Select roots.
The package's producer rows `P-E13-07/08`, detailed rows `T-E13-007/008`, query
rows `T-Q-13-007/008`, diagnostic rows `D-E13-04/05`, nested rollback rows,
three-slot/four-component totals, and compact 128/129-dot limits are
unreachable as written.

The correction must use a tokenization-safe real producer such as
`target. .member` and `target. .`, and must prove its exact CST and ranges
through `ParsedSource`. Compact `..` remains range syntax. No special lexer
exception may be added merely to preserve the rejected fixtures.

### `?.` is postfix Try followed by ordinary Select

The production semantic expression parser reads `target?.member` as:

```text
Select(
  target = Try(target, PostfixQuestion),
  member = member,
)
```

The private shadow lexer/parser currently has a provisional combined `?.`
Select token. The return retained `OptionalDot` only in attachment and then
lowered it to the same two-slot final Select as `target.member`. That discards
result-changing `Try` semantics and is not a valid migration.

The final direction is now fixed: delete `?.` from the combined Select path,
emit the accepted `ExpressionProjection::Try { form: PostfixQuestion }`, then
emit an ordinary dot Select whose target is that Try expression. The Select
payload does not gain an optional-delimiter flag. Every `target?.member` and
`target?.` source, slot, diagnostic, query, limit, and rollback row must include
the inner Try identity.

### Select belongs in the central expression projection

The protected final-HIR integration has one parser-owned
`ExpressionProjection`, one `PendingExpressionProjection`, and one
`AttachedExpressionNode`. The current shadow `emit_select` does not yet set a
projection. The return instead introduced a separate `AttachedSelectExpr` that
walks CST children independently. That path would either fail the central
`MissingExpressionProjection` gate or become a second semantic reader.

The corrected owner is:

```rust
pub enum SyntaxSelectedMember {
    Name(SyntaxName),
    Missing,
}

pub enum ExpressionProjection {
    // existing variants remain unchanged
    Select(SyntaxSelectedMember),
}
```

`emit_select` must use the same projected-start/event owner as every other
expression, attach exact `Target` and `SelectedMember` components, and make the
common `AttachedExpressionNode` the sole attached reader. The lossless CST and
the Select `Whole` span retain the dot token; no delimiter source scan or
parallel `AttachedSelectExpr::try_from_node` is added.

The current grammar creates a `NameReference` or a zero-width `MissingName`
only. It does not create an invalid Select member `ErrorNode`. Parser-admitted
names use the same identifier predicate as `HirName`. Therefore the final
payload is narrowed directly to:

```rust
pub enum HirSelectedMember {
    Name(HirName),
    Missing,
}

pub struct HirSelectExpr {
    target: ExprId,
    member: HirSelectedMember,
}
```

`AttachedSelectedMember::Invalid`, `HirSelectedMember::Invalid`, their
diagnostic/source/NameBytes rows, and direct-constructor evidence are deleted.
An impossible name conversion is an attachment/lowering invariant failure and
rolls back the transaction; it is not publishable recovery state. This is the
deletion-driven resolution of the previous provisional schema, not a
compatibility change.

### MissingName owns its actual insertion after trivia

The shadow parser consumes trivia after the dot before it creates
`MissingName`. Its insertion is therefore the attached `MissingName` range at
the current cursor, not necessarily the dot's end. The return's delimiter-end
invariant is false for whitespace/comment-separated producers.

The source authority is the zero-width attached `MissingName` range. Tests must
include ordinary, whitespace, and comment trivia and must not reconstruct the
insertion with range arithmetic.

### MissingName does not emit a parser diagnostic

The current `emit_select` creates `MissingName` but emits neither
`SyntaxEvent::MissingToken` nor `SyntaxEvent::Diagnostic`. The accepted
`SyntaxParseStats` has no parser-recovery-record field. Its diagnostic counter
counts only normalized `SyntaxEvent::Diagnostic` identities, and
`SyntaxLimit::Diagnostics` is `1,024`.

The previous continuation request incorrectly asserted that each E13
MissingName increments a parser recovery record and a syntax diagnostic and
that diagnostics reject at 129. That assertion mixed the obsolete detached
expression parser's `128` diagnostic limit with the final attached grammar.
This repository intake explicitly corrects that request error.

The final E13 rule is:

```text
MissingName syntax diagnostic delta       0
separate parser recovery-record delta     not an existing counter
missing-member HIR diagnostic delta       1 per Select owner
```

Thus a direct missing Select contributes one final diagnostic; a poisoned
target plus clean member contributes only the descendant's one diagnostic;
and a poisoned target plus missing member contributes two diagnostics owned by
two different Select roots. General syntax-diagnostic 1,024/1,025 evidence
must use a real syntax-diagnostic producer. Repeated Select evidence must be
bounded by the actual expression/identity/source budgets, not the deleted
128-row reader.

### Authored poisoned target is `RecoveredChild`, not `MissingOperand`

An authored target exists as an `ExprId`. If that child is poisoned, the
Select root uses the accepted roleful propagation issue:

```rust
HirRecoveryIssue::InvalidExpression(
    HirExpressionRecoveryIssue::RecoveredChild {
        role: HirExprSourceRole::Target,
    },
)
```

`MissingOperand { role: Target }` is reserved for a missing synthetic child.
Using it here would conflate an authored poisoned target with an absent target,
which has no Select producer.

Target propagation has precedence for the singular root poison. Diagnostic
obligations remain payload-based: `Name` adds no parent diagnostic; `Missing`
requires the Select-root `SelectedMember` diagnostic even when the target's
distinct owner already has a terminal diagnostic.

### Source-query precedence must not be reordered

The accepted query order is:

```text
owner module/kind/liveness
-> role applicability and ordinal
-> expected document ID
-> source revision
-> retained source length
-> committed presence and owner status
```

The return moved document/revision/length before role applicability. Its
negative and combined-failure rows must be rewritten to the accepted order.
No E13-specific map, fallback, raw range reader, or stored query outcome is
introduced.

### The return invents a new name-construction owner

The return changes `HirName::try_new(Box<str>) -> Result<HirName,
HirNameInvariantError>` into a constructor that takes an undeclared
`HirWorkBudget` and returns a new `HirNameConstructionError`. The accepted
owner instead has the lowering transaction preflight `HirLimit::NameBytes`
before calling the existing `HirName` constructor.

The E13 correction must not redesign every name consumer. Valid authored name
bytes retain exact/one-over transaction evidence; Missing charges zero; the
deleted unreachable Invalid rows provide no implementation evidence.

## Corrected implementation boundary

E13 remains design-blocked until a standalone replacement closes the corrected
producer and matrix. The next request is
[Proof 01.1.1.4.1.1.1.1.1.1.1 Select central projection and accounting correction](../reviews/requests/2026-07-30-seq-proof-01.1.1.4.1.1.1.1.1.1.1-select-central-projection-and-accounting-correction.md).

The correction is not asked to choose the architecture again. It must encode
the fixed decisions above, recompute only the affected E13/Try rows from real
producers, and return one standalone replacement package. Other decision-ready
final-HIR families may continue independently. No obsolete Select reader,
`OptionalDot`, invalid-member branch, source fallback, or compatibility path is
repaired while waiting.
