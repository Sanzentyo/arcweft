# Validation report

## Scope

Documentation-only design resolution against Git commit
`300e824eea6740eab0ae708508cce00a1bd49435`. No production Rust, Cargo
manifest, test, fixture, branch, worktree, commit, push, or returned ZIP was
created or changed by that original design cut.

The 2026-08-25 C1 coordinate erratum in
`CALL_APPLICATION_AUTHORITY_AMENDMENT.md` corrects the binding capture owner,
removes recursive value-coordinate reconstruction, and fixes checked path/byte
lengths to canonical little-endian `u64`. This report remains the accepted
design evidence, but the erratum's implementation and canonical-byte
validation are now **pending** against the current dirty `main`.

The call-application amendment additionally audited the intentionally dirty C2
implementation at HEAD
`958242ba4d6236fe37475a090bfadbe636de6594`. The documentation cut changed no
production file and preserved all existing implementation WIP.

## Performed and passed

- verified initial clean `main == origin/main` at the full SHA above;
- confirmed the design directory did not exist before authoring;
- inspected the maintained request, accepted parent C1/C2/C3 design, and the
  current source owners listed in `SOURCE_EVIDENCE.md`;
- checked the required member set, `READY_FOR_IMPLEMENTATION`, and exact
  `none` open-question content;
- checked Markdown whitespace with `git diff --check`/new-file checks;
- checked every relative Markdown link in the design and updated request; and
- audited each proposed new domain byte literal for exact repository equality.
- reconciled the nominal projection budget split with existing
  `NominalResolutionLimits` and `NominalAggregationLimits`;
- audited every current prepared/published fact-family owner required by the
  exhaustive projection request visitor; and
- confirmed `env/nominal.rs`, not a TypeCheckEnv map, owns accepted exact
  records, catalog digest, lookup, and instantiation.
- confirmed the amended schemas define cancellation, per-root/project limits,
  arithmetic overflow, identity mismatch, missing-cache failure, retained
  `TypeShape`, exhaustive request inventories, borrowed post-seal lookup, and
  private environment field construction.
- reconciled the implementation dependency so C2.2a extracts only the
  context/expander foundation, C2.2b establishes Record authority, C2.3 creates
  exact final types plus projection-independent rows and consumable seeds, and
  C2.4 alone adds the exhaustive visitor, constructs projection-dependent rows,
  and seals the catalogs; and
- confirmed the temporary C2.2a delegating wrappers are explicitly forbidden
  as accepted completion, parallel authority, intermediate commit, or push and
  are deleted before final C2 publication.
- re-audited the call pipeline with Sol max and a Luna max producer/consumer
  inventory; rejected the local optional/rest witness patch because the same
  authority was split across provisional facts, pending analysis, final
  rebuild, join inference, raw continuation reconstruction, and compiler
  reconstruction;
- selected the schema/source projection composition, candidate-wide MGU,
  frozen cumulative solution, one post-catalog/effect acyclic
  core/continuation/final seal with only the complete application published,
  opaque continuation, and sema-owned execution projection documented in
  `CALL_APPLICATION_AUTHORITY_AMENDMENT.md`; and
- required Option, Result, Agent, Collection, Reduction, and Fx
  inference-bearing schemas to be reconciled without adjacent placeholder
  exceptions; Traverse/Parallel specifically fail closed because no accepted
  carrier exists, while Reduction reuses its accepted nominal generic owner.
- fixed and searched the six new canonical call domains, including the full
  resolved-callable domain
  `arcweft.lang.resolved-callable.v1`,
  `arcweft.lang.call-type-solution.v1`,
  `arcweft.lang.call-candidate-inventory.v1`,
  `arcweft.lang.call-continuation.v1`,
  `arcweft.lang.checked-call-application-core.v1`, and
  `arcweft.lang.checked-call-application.v1`; none existed elsewhere in the
  repository at the amendment baseline.
- re-audited the in-progress C2 implementation with Sol max and rejected
  source-traversal projection because it changes aggregate-budget and
  first-error precedence;
- selected a post-Analyzer disjoint-parts seal so one borrowed context can
  process the complete request inventory in semantic-digest order without an
  `Arc` type-map authority or self-referential Analyzer;
- closed Method ownership by composing one post-finalization join map, using it
  for exact selection enrichment, and moving it into edge facts without a
  second join; and
- closed compiler/runtime-plan field ownership with typed project runtime
  coordinates, move-only record-expression edge rows, typed pattern rows, and
  owner/ordinal environment rejection.
- iterated the amended call design through Sol max blocker audits covering
  acyclic core/continuation/final identity, generation-local ID exclusion,
  compiler callee/receiver source constructibility, lower-layer coordinate
  ownership, recursive cancellation/accounting, exact language candidate
  encoding, schema-derived argument actions, open-named admission, and every
  base-instantiation payload;
- moved the C1 path/value-coordinate algebra and its sole encoder to the
  selected sema-root owner in the design, required issuer-only prepared
  candidates to be consumed only after the private coordinate-index seal, and
  removed the common callee and copied dispatch/action authorities;
- fixed OpenSupply mapping, candidate canonicalization, mutable lower
  constraint context/report handling, exact application-core domain/wire, and
  raw-HIR allocation-invariance/tamper gates; and
- received final Sol max `APPROVE`: no remaining result-changing
  contradiction, constructibility failure, phase/layer cycle, duplicate
  authority, or encoding ambiguity was found.

The proposed domains are:

```text
arcweft.lang.accepted-project-item.v1\0
arcweft.lang.accepted-variant-case.v1\0
arcweft.lang.accepted-record-field.v1\0
arcweft.lang.accepted-environment-field.v1\0
arcweft.lang.accepted-character-look.v1\0
arcweft.lang.effect-semantic.v1\0
arcweft.view.specified-value-semantic.v1\0
arcweft.lang.resolved-callable.v1\0
arcweft.lang.call-type-solution.v1\0
arcweft.lang.call-candidate-inventory.v1\0
arcweft.lang.call-continuation.v1\0
arcweft.lang.checked-call-application-core.v1\0
arcweft.lang.checked-call-application.v1\0
```

The first five already occur only in the accepted parent design as proposed
domains and do not occur as a production digest domain. The remaining eight are
new and have no exact existing repository match. The removed View-modifier
domain is not implemented or registered.

## Failed

- The current full sema library tier has `410 passed, 40 failed` tests. The
  failures remain in the call/type-constraint, candidate probe/replay,
  rooted-evaluator, and pending nominal-projection families listed in the
  implementation amendment below.

## Blocked

- Strict implementation completion remains **BLOCKED**. The accepted-root
  catalog and path plumbing compile, but the lower call/application graph and
  C2.4 nominal/item-root closure are not complete.
- Item-root negative-path coverage and the remaining strict audit corpus are
  pending; this report must not be read as an implementation PASS.

## Not run

- Cargo format/check/test/Clippy, workspace tests, Tier 2, and structural gates
  were not run because this cut changes documentation only.
- The request's returned-archive validator and negative corpus were not run;
  no external returned archive is being accepted by this repository-local
  design task.
- Production implementation tests listed in
  `CUTS_TESTS_AND_DELETION.md` remain implementation acceptance criteria, not
  design validation evidence.
- No additional Cargo or production test command was run for this
  documentation-only phase-boundary amendment. Focused callable-join tests run
  against the concurrent implementation are recorded with that implementation
  cut, not as design validation.
- C1 coordinate owner tests for full binding paths, allocation-order/raw-ID
  exclusion, and canonical `u64` length bytes are pending after the erratum.

## 2026-08-25 accepted-root implementation amendment

The active dirty-main implementation now carries the version-1
`AcceptedSemanticRoot` tag byte in canonical checked paths and uses one
Analyzer-owned `AcceptedSemanticRootCatalog` lease over the already sealed
HIR project topology. Item roots use the typed source-order entry/inline-member
role and direct exhaustive family tags; Match and producer coordinate APIs no
longer rebuild declaration/item path indexes or accept a declaration argument.
`cargo check -p arcweft-lang-sema`, focused coordinate/Match/producer tests,
and the full sema library tier were run during this cut. After the lower exact
regression was added, the full tier inventory is 450 tests: `410 passed, 40
failed`. Failures remain in the call/type constraint, candidate-probe/replay,
rooted-evaluator, dialogue-topology, and nominal-projection families
(including `CallConstraintFailure`, `CheckedCallableJoin(ResultMismatch)`,
zero-vs-expected candidate counters, `ExpressionTypeUnavailable`,
`InvalidOwner`, and the existing `InvalidNominalOwner` Match fixture); no
catalog/path compile failure occurred. HIR full tests were `876 passed, 0
failed, 8 ignored`; HIR clippy exited successfully with warnings. Sema
clippy also exited successfully with warnings after the `never_loop` shape in
`types/constraints/transaction.rs` was removed. This is implementation
evidence, not an implementation PASS; strict completion and item-root negative
tests remain pending.
