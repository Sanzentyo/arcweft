# Proof-concurrency v6.1.1 E13 Select central projection and accounting correction

Status: `READY_FOR_IMPLEMENTATION`

Audited GitHub `main`: `004ff3d69f241954eb808985878c348b165a815c` (`Adjudicate corrected Proof Select return`).

This archive is the complete standalone replacement required by the 2026-07-30
E13 continuation request. It does not depend on either rejected Select return
and does not preserve their matrices under new names.

## Closed result

- The final payload is exactly `HirSelectedMember::Name(HirName) | Missing` in
  the original `HirSelectExpr` owner. `Invalid` is deleted because no current
  Select producer consumes a non-name token as a member.
- Select extends the single central `ExpressionProjection` and is read only
  through the common `AttachedExpressionNode`; no standalone attached Select
  reader, projection database, extension trait, or CST fallback exists.
- `target?.member` and `target?.` allocate an inner postfix-`Try` identity and
  an ordinary outer dot-Select identity. The provisional combined `?.` /
  `OptionalDot` path is deleted.
- Missing Select targets are unreachable because leading `.member` is
  `ShortVariant`. E13 allocates no `RecoveryOperand` child and charges zero
  synthetic descendants.
- Missing members own the parser-produced zero-width `MissingName` insertion,
  add zero syntax diagnostics, and add exactly one owner-keyed HIR recovery
  diagnostic.
- Authored poisoned targets propagate through
  `InvalidExpression(RecoveredChild { role: Target })`; the descendant keeps
  its terminal diagnostic. A simultaneously missing outer member adds one
  independent Select-root diagnostic, ordered descendant before ancestor.
- The sole public source authority remains
  `HirModule::source_site(expected_source, HirSourceQuery)`. Select admits only
  slot `Whole` plus `Target` and `SelectedMember`; `Recovery` and all unrelated
  roles are inapplicable.
- Syntax, source bytes, name bytes, final diagnostics, HIR expressions, global
  module slots, source components, rollback, retry, and deduplication are
  closed by independently executable rows.
- Migration is one deletion-driven compiling switch. No alias, wrapper,
  compatibility reader, dual map, source reparse, source gate, CSS/Takumi
  branch, or removed-syntax-specific diagnostic is authorized.

## Package organization

The archive contains exact copies of the new request, primary E13 request,
previous continuation request, and both rejection intakes; direct predecessor
member/hash evidence; complete affected schemas; producer/CST/projection
geometry; Try+Select identity; source, poison, diagnostics, limits, rollback,
consumer deletion, focused test, and traceability matrices; and a non-self
manifest.

Every sidecar is inside this ZIP. No adjacent status, summary, checksum, or
manifest file is required.
