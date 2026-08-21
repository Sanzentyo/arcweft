# Lang-01.5.1.1.2.1.1 — reactive unary-Need match design-validation correction

## Sequence, inputs, and precedence

This is the mandatory redelivery correction for
[Lang-01.5.1.1.2.1 reactive unary-Need match reconciliation](2026-08-21-lang-01.5.1.1.2.1-reactive-unary-need-match-reconciliation.md).
It does not replace or narrow that primary request. It requires one complete,
independently usable design answer after the first returned archive contained
only the request and failed validation evidence.

Inspected production baseline:
`0fa8a3b845b2dc966f181f450a1ca1f36e49d966`.

Required retained inputs are:

- the primary request linked above;
- the accepted parent
  [`Lang-01.5.1.1.2 final-HIR View execution package`](../packages/arcweft-lang-01.5.1.1.2-final-hir-view-execution-catalog-and-static-certification-reconciliation-final-contract/README.md);
- the failed returned archive retained at
  [`arcweft-lang-01.5.1.1.2.1-reactive-unary-need-match-reconciliation-final-contract.zip`](../packages/zips/arcweft-lang-01.5.1.1.2.1-reactive-unary-need-match-reconciliation-final-contract.zip),
  SHA-256
  `C5857AFCFCDDC88D2F642C4B4ACB0E61A68BBC4AC0BE42755BA9C2593B20E732`;
- its failed
  [`design-validation.json`](../packages/arcweft-lang-01.5.1.1.2.1-reactive-unary-need-match-reconciliation-final-contract/evidence/design-validation.json);
  and
- current production and maintained language/runtime documentation at the
  inspected baseline.

Current production and maintained documentation take precedence over package
sketches. Preserve the accepted parent catalog, generic
`ViewInstruction::Match`, ordinary RuntimeValue/AWBC execution, ownership,
resources, transactional publication, static certification, work limits,
save/replay, and hot-replacement decisions unless a concrete current-repository
contradiction is demonstrated.

## Split reason

The first returned archive is byte-valid but explicitly reports
`pass: false`. It omits `README.md`, the concrete reconciliation design,
requirements traceability, source evidence, the test matrix, the implementation
sequence, and verification. It also reports insufficient Rust line evidence
and undersized traceability and test matrices. Therefore it closes none of the
primary request's result-changing decisions and cannot authorize production
work.

These failures form one atomic correction. Separate answers for subscriptions,
wire/save identity, or deletion would risk incompatible contracts, so use one
design-only assignee and return one complete archive.

## Required decisions

Close every decision in the primary request, including all of the following:

1. Define the sole checked owner for a View-context ordinary `match` over
   unary `Need<T>`, retaining final-HIR expression/pattern identities, accepted
   generation, exact `T`, source-ordered arms, bindings, exhaustiveness,
   effects, source roles, and ownership disposition.
2. Define one retained subscription identity that maps checked Need-producing
   expressions to producer/Need identity without source strings, copied
   endpoint tables, or a RuntimeValue handle surrogate.
3. Define deterministic publication selection for `NotStarted`,
   `Pending(Progress)`, `Ready(T)`, and `Cancelled`, including epoch/sequence,
   first frame, stale/duplicate/coalesced publications, invalidation, remounts,
   and multiple observers/mounts.
4. Define ordinary pattern and arm-local binding execution through the
   parent's generic Match and RuntimeValue/AWBC owner. Do not introduce a View
   VM, `ViewRuntimeValue`, or presentation-only payload fallback.
5. Define nested `Need<Result<T, E>>` and `Need<Option<T>>` behavior. Need has
   no error or denied branch; domain failure remains inside `Ready(Result::Err)`
   and admission denial remains outside Need.
6. Select producer-start ownership for observing `NotStarted`, the sole
   Sans-I/O start request owner, deduplication identity, cancellation owner,
   and failure behavior.
7. Define version-1 bundle, runtime, save, replay, and hot-replacement identity
   for subscriptions, cursors, mount occurrences, retained arm state, queued
   invalidation, producer generation, and restore/replacement transactions.
8. Define strict in-place deletion of `ViewProgramInstruction::Await`,
   `ViewAwait`, `ViewAwaitBranchSpan`, the four-way evaluator,
   `InvalidAwaitState`, codec rows, merge/fingerprint branches, tests, and stale
   direct-await parent rows, with no compatibility reader or alias.
9. Reconcile static certification so any live Need subscription is dynamic and
   an authored static assertion fails through the parent's ordinary typed proof
   path at the exact first contaminant.
10. Define failure precedence and atomicity across sema, compiler publication,
    strict decode, runtime publication, pattern/exhaustiveness, ownership,
    stale generation, restore, and replacement.
11. Define exact bounded accounting and limits for subscriptions,
    publications, pattern/arm work, payload depth, mount fanout, invalidation
    queues, restore, replacement, and diagnostic accumulation.
12. Provide a deletion-driven compile-clean interleave that lands any missing
    parent generic-Match substrate first, atomically switches all consumers,
    and then deletes the old Await surface.

Every result-changing alternative must be selected. `OPEN_QUESTIONS.md` must
be exactly `none`, not an explanation that questions remain.

## Required repository evidence and consumer inventory

Inspect current production rather than relying on filenames or the failed
archive. The returned source evidence and consumer matrix must cover at least:

- maintained View, Need, Progress, Result/Option, and ordinary match chapters;
- syntax/HIR match expressions, patterns, source roles, and View contexts;
- final semantic View catalog plus checked unary Need/Progress/Result owners;
- compiler View product lowering and RuntimePlan/AWBC dynamic programs;
- `arcweft-view` instructions, dependency graph, mount identity, local state,
  static proof, and old Await consumers;
- bundle model, codec, validation, merge, digest, source maps, and strict old
  payload rejection;
- runtime-driver evaluation, Need publication input, save/replay, replacement,
  native/Web/headless/Agent observation, and generated artifacts; and
- all current Need/Await/View tests plus every directly superseded parent
  matrix row.

For each result-changing claim, cite exact current Rust paths and line ranges,
the owning type/API, and the consumer that proves the selected dependency
direction. Include enough Rust-shaped detail to implement without guessing.

## Required artifacts

The archive must contain, at minimum:

- `README.md` with final readiness and reading order;
- `OPEN_QUESTIONS.md` containing exactly `none`;
- a concrete design contract with exact Rust-shaped owners and APIs;
- source/repository evidence with current full Git SHA and line citations;
- requirements traceability mapping every primary and correction requirement;
- producer/consumer and deletion matrices;
- wire, codec, save, replay, replacement, and version-1 allocation tables;
- diagnostic precedence, atomicity, and bounded-work contracts;
- a compile-clean implementation sequence;
- a full positive/negative/tamper/Tier-2 test matrix; and
- a verification report plus an internal SHA-256 manifest covering every
  payload.

The validator must fail the package for a missing required artifact, an
unresolved decision, an unverified manifest row, inadequate line evidence, or
an undersized traceability/test matrix. Include its machine-readable result and
human-readable status inside the ZIP.

## Required tests and verification design

The test matrix must specify exact owners, inputs, and expected results for:

- all four Need states and multiple Pending progress publications;
- first, duplicate, stale, out-of-order, coalesced, and same-step
  progress-to-ready publication;
- nested Result/Option patterns, arm bindings, source order, exhaustiveness,
  and ordinary no-match behavior;
- two mounts and two observers, remount, producer dedup/start/cancel, stale
  producer generation, save/restore, replay, and hot replacement;
- affine payload/capture rejection and static-certification contamination;
- malformed type/pattern/subscription identity, cursor corruption, codec
  tamper, strict rejection of old Await bytes, and no-partial-publication
  transactions;
- exact-limit and one-over work accounting; and
- API-absence proof for every old Await type, variant, discriminant, evaluator,
  and `AwaitView` spelling.

List focused Cargo gates, workspace check/test/Clippy, documentation,
structure, save/replay, differential runtime, native/Web/headless, Agent, and
generated-artifact gates. Commands are design evidence, not claims that
Arcweft production already passed them.

## Implementation order

1. Reconcile the primary request against current `main` and freeze all owners,
   identities, wire/save allocations, limits, and precedence.
2. Publish complete checked subscription/match facts while compiler/runtime
   product construction still fails closed.
3. Add the parent generic-Match/runtime-value substrate that is genuinely
   absent, without adding a Need-specific value VM.
4. Switch compiler, bundle, runtime, save/replay, replacement, tooling, and
   generated consumers atomically to the typed subscription model.
5. Delete the complete old Await instruction/model/codec/evaluator/test route
   in the same version-1 boundary cut.
6. Run focused, tamper, differential, Tier-2, workspace, Clippy, documentation,
   generated, and structural gates before reporting implementation readiness.

## Constraints and non-goals

- This is design-only. Do not edit production code, tests, fixtures, manifests,
  branches, patches, PRs, or implementation overlays.
- Do not redesign accepted parent catalog, generic Match, RuntimeValue,
  ownership, resource, transactional publication, or static-proof decisions
  without a concrete repository-evidenced flaw.
- Do not restore direct View Await, `AwaitView`, Need-owned error/denied arms, a
  View VM, a parallel value model, source reconstruction, strings as identity,
  copied endpoint catalogs, compatibility aliases, shims, dual readers, source
  gates, or removed-syntax diagnostics.
- Do not implement Need timeout, Stream/Watch observation, broader mount
  syntax, Dialogue/Ruby, Choice, CSS, or Takumi in this correction.
- Keep lower crates Sans I/O, dependency direction intact, behavior
  deterministic, and every Arcweft-owned version marker fixed at `1`.

## Expected output

Return one independently usable archive named
`arcweft-lang-01.5.1.1.2.1.1-reactive-unary-need-match-design-validation-correction-final-contract.zip`.
It must include the complete primary design answer as corrected by this
request, not merely a packaging patch, validation transcript, delta, or pointer
to the failed archive. Do not include a production code overlay.
