# Design request: Proof-concurrency 01.1.1.4.1.1 source-owner and semantic consistency correction

- Date: 2026-07-27
- Sequence: Proof-concurrency v6.1.1.4.1.1, narrowly correcting the
  v6.1.1.4.1 READY-claim package before its affected public HIR switch
- Kind: independently throwable, GitHub-only, design-only correction
- Production changes: prohibited
- Required archive name:
  `arcweft-proof-concurrency-v6.1.1.4.1.1-source-owner-and-semantic-consistency-correction-final-contract.zip`

## Assignment

Prepare one decision-complete correction for the retained Proof
v6.1.1.4.1 package in the latest GitHub `main` of:

`https://github.com/Sanzentyo/arcweft`

Read repository `AGENTS.md`, this request, the full retained v6.1.1.4.1 ZIP,
its primary/correction request copies, and the current intake note:

- `docs/reviews/designs/proof-concurrency-v6.1.1.4.1/arcweft-proof-concurrency-v6.1.1.4.1-final-hir-semantic-leaf-expression-payload-correction-final-contract.zip`
- `docs/implementation/2026-07-27-proof-01-1-1-4-1-ready-claim-redelivery-intake.md`

This is not permission to redesign the accepted 35-expression inventory,
qualified arenas, AW-AH-009.4.2 outer Dialogue/ID shapes, shared resolver,
Thread body, RichText ownership, or deletion order. Correct only the internal
contradictions below and restate every superseded schema/matrix row in full so
the result is usable without inference.

## Required decisions

### 1. Exact source owner, role, storage key, and query authority

The retained package exposes only
`expr_source_site(ExprId, &HirExprSourceRole)` but requires component queries
for PatternId and TypeId owners.

Select and specify one exact typed design that covers:

- every ExprId role already accepted by AW-AH-009.4.2;
- every PatternId role, including pattern fields and path roots/segments;
- every TypeId role, including named/elided regions and type paths;
- component storage keys and deterministic ordinals;
- source-backed, poisoned, absent optional, synthetic/insertion, stale,
  foreign-module, and rolled-back outcomes; and
- public read-only query signatures and their owning crate/module.

State explicitly whether the accepted `expr_source_site` method remains
unchanged or is superseded, and how. Do not leave an orphan part enum, a
generic untyped owner, an overload that cannot be expressed in Rust, or a
fallback based on vector position. Do not introduce a compatibility wrapper
or dual reader.

### 2. Lossless pathless variant-pattern payload

Define the exact final HIR representation for current syntax families with no
authored path: `.Foo`, `Some`, `None`, `Ok`, and `Err`. Reconcile it with
root-preserving `HirPath`, expected-type resolution, payload PatternId
ownership, poison behavior, source roles, and scopes. No fabricated empty
path, early string resolution, or generic `Error` downgrade is allowed.

### 3. Duration structural traits versus value comparison

Select one exact contract for `HirDurationLiteral::Value` and
`authored_unit`:

- which Rust structural traits include the authored unit;
- the exact semantic equality/order API used by checker/cache/fingerprint;
- Hash/Eq/Ord consistency;
- whether authored unit is semantic payload or revision-bound component
  metadata; and
- tests showing that equivalent spellings such as `1s` and `1000ms` have the
  required structural, semantic-value, diagnostic, and fingerprint outcomes.

Do not leave both the schema-wide derive rule and unit-insensitive equality as
unnamed competing authorities.

### 4. Checker-only overflow owners

Define exact sema/checker result and error records for float width overflow and
Duration runtime-range overflow. State whether the corresponding HIR issue
variants are removed, retained but unconstructible for valid HIR, or assigned
a different lowering meaning. Give direct lowering/checker phase tests and
prevent default/truncating values.

### 5. Elided-region synthetic owner

Reconcile `HirElidedRegion { key: SyntheticKey }` with the accepted
`SyntheticOwner` inventory, which currently has no TypeId owner. Define the
exact owner-enum extension (or an explicit replacement), constructor and
accessors, role/ordinal validation, equality/ordering/fingerprint identity,
source insertion ownership, stale/foreign behavior, and migration from the
current private raw-owner substrate. Do not use an untyped raw ID or infer the
owner kind by probing arena slots.

### 6. Exact byte/segment limits and accounting phase

Close every matrix charge that currently lacks a numeric owner, including
decoded string bytes and path/registry segment/source-byte budgets. For each,
state:

- exact inclusive production maximum or explicit absence of a separate limit;
- syntax/HIR/checker/decoder charging phase;
- observed/limit integer types;
- exact-boundary commit and one-over atomic outcome; and
- interaction with SourceDocument, `HirLimits`, callable, and RichText limits.

Do not invent a source gate or scan checked-in source text.

### 7. Repair matrix/test traceability

Replace every affected P01-P12, C08-C13, Duration/float/limit,
`T-SOURCE-01`, and migration row with the corrected exact contract. Resolve
the 164 referenced but absent `T-Q-*`/`T-RB-*` family identifiers by either
providing the rows or declaring and validating them as named subtests under an
existing row. Clarify the observable negative test for fieldless valid
variants without asking an impossible constructor-invariant mutation.

## Constraints

- Design only: do not edit production code, create a patch, branch, PR, or
  implementation overlay.
- Preserve all uncontradicted v6.1.1.4.1 decisions and matrices.
- No alias, wrapper, extension trait, compatibility shim, dual reader,
  source-string reparse, source gate, CSS/Takumi path, or permanent
  removed-syntax-specific diagnostic.
- Do not repair the old Speaker/ContentCall/HirDialogue or detached syntax
  readers; the final migration remains deletion-driven.
- Copy this complete request and the relevant retained-package schemas/rows
  into the return. Do not return a prose-only answer or a delta requiring the
  implementer to compare archives manually.

## Required return

Return exactly one ZIP with every sidecar inside and no required adjacent
files:

```text
arcweft-proof-concurrency-v6.1.1.4.1.1-source-owner-and-semantic-consistency-correction-final-contract.zip
```

Include README, complete request copy, FINAL_STATUS, OPEN_QUESTIONS, exact Rust
schemas, corrected source-role/query contract, corrected numeric/Duration
contract, corrected lowering/test rows, traceability, repository evidence,
validation report, and manifest.

Use `READY_FOR_IMPLEMENTATION` only when `OPEN_QUESTIONS.md` is exactly `none`
and every required source owner, trait, failure phase, limit, and test reference
is closed. Otherwise return the same correctly named ZIP with `NOT_READY` and
explicit unresolved decisions.
