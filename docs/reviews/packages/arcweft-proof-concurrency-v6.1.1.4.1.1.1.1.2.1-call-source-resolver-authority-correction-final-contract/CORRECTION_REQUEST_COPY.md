# Repository correction request copy

Repository path:
`docs/reviews/requests/2026-07-29-seq-proof-01.1.1.4.1.1.1.1.2.1-call-source-resolver-authority-correction.md`

Repository blob SHA:
`a57f0a4bd2419ef49822a2adf6886798d5e2066b`

Audited main:
`004ff3d69f241954eb808985878c348b165a815c`

## Assignment

Prepare one decision-complete standalone replacement for the rejected
Proof-concurrency E12/C01-C03 Call package. Read `AGENTS.md`, the primary E12
request, the rejected-return intake, all referenced predecessor archives and
intakes, the AW-AH-009.3 accounting note, the accepted AW-AH-009.3.3.3.1 and
AW-AH-009.3.3.4 packages, and the current grammar, Call syntax, callable
facts/resolver/limits, signature, and HIR source-query owners.

The rejected return `BC8DE35E8C4D69008344EC44B9CFF1C5C59EE17ECB2CA54006B0ECF6EE923B50` is not an input dependency. Preserve its
usable direction only where repository authority confirms it.

## Preserved useful decisions

- Known Call syntax retains `HirExprKind::Call` with typed poison.
- Every authored slot remains in order and keeps positional, named, or spread
  form through HIR, checking, diagnostics, and signature projection.
- Missing/invalid names never fabricate `HirName`; missing expressions never
  fabricate or sentinel `ExprId`.
- Missing callee/value expressions use real root-owned poison children with
  `RecoveryOperand` ordinal zero and `1 + argument ordinal`.
- Dot syntax keeps same-revision value and nominal evidence until
  value-first/nominal-second classification finishes; explicit `::` is
  nominal-only.
- Explicit call type applications have one ordered qualified final owner and
  remain distinct from generic arguments inside an associated receiver.
- Migration is deletion-driven and leaves no compatibility reader.

## Required correction decisions

1. Define exact Call/callee/issue/root-poison schemas for clean/recovered value
   callees, missing callee, unresolved dot, associated receiver/member/separator
   states, invalid/missing receiver/member, terminal nominal error, bare generic
   arity, every argument form/name/value state, and explicit call type argument.
   Present-invalid typed syntax retains its qualified poisoned ID. Define
   canonical issue order and the exact relationship to singular root poison.
2. Preserve the sole source authority:
   `HirSourceIndex.components: BTreeMap<HirSourceQuery,HirSourceSite>` and
   `HirModule::source_site(expected_source, query) -> HirSourceLookup`.
   `Whole` is slot metadata; `AbsentOptional` is presence; owner poison is
   status; inapplicability is a typed query error. Add Call roles only to this
   map/query. Do not create a Call surface map or second reader.
3. Use current grammar exactly:
   `CallArg := Expr | Ident '=' Expr | Expr '...'`.
   Preserve opening/comma/trailing-comma/close/recovery cursor behavior and the
   accepted AW-AH R04/R05/R08/R09/R13/R14 fixtures.
4. Define one revision-bound central `ExpressionProjection::Call` carrying
   callee evidence, ordered current-grammar argument states and punctuation,
   optional explicit type application, and all recovery components. Delete the
   final detached argument/type/cursor reader in the same switch.
5. Integrate the existing shared resolver and complete `CallTargetFacts`.
   Preserve selected/ambiguous/rejected/non-callable/missing outcomes, result,
   effects, curried groups, function value type, candidates, mappings,
   inferred/expected types, poison, and diagnostics. Keep the 256 candidate
   ceiling. Separate logical argument checking, physical probes, selected
   replay, fact publication, and signature projection.
6. Reuse `HirCallArgumentOrdinal`, central `HirLimit`, `CallableLimits`,
   ordinary 128 arguments, and RichText 32 arguments. Make every exact/one-over
   test reachable. Call recovery ordinals are only `0..=128`; general
   `1023/1024` remains predecessor generator evidence.
7. Provide complete end-to-end and deletion matrices. Tests use public behavior,
   typed queries, compile-fail, and structured dependency evidence, never a
   source gate.

## Constraints

Design only. No production edits, patch, branch, PR, implementation overlay,
aliases, wrappers, extension traits, compatibility shims, dual readers,
source-string reparsing, source gates, CSS/Takumi path, permanent removed-syntax
diagnostic, static Capacity helper, early success dispatcher, detached old HIR,
or old Dialogue repair.

## Required return

Exactly one archive:
`arcweft-proof-concurrency-v6.1.1.4.1.1.1.1.2.1-call-source-resolver-authority-correction-final-contract.zip`

All sidecars are inside. `READY_FOR_IMPLEMENTATION` is permitted only when
`OPEN_QUESTIONS.md` is exactly `none`, every clean/malformed case has one
non-fabricated representation, sole source/resolver/limit authorities are
preserved, and migration is implementable with zero second reader.
