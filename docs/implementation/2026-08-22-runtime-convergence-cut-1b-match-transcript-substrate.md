# Runtime convergence Cut 1b — Match transcript substrate

Date: 2026-08-22
Base commit: `423bc649a1755669c45dedce04cdd9706f710e4f`

## Result

- Cut: `1b — typed statement/pattern/body paths and fail-closed Match transcript substrate`
- Result: `PASS WITH EXPLICIT FAIL-CLOSED DEVIATIONS`
- Full generic Match completion: not claimed
- Production commit/push at the time of this note: not yet performed

## Implemented

- Added exhaustive typed HIR statement and pattern child-edge authorities.
  Pattern locals are owned by pattern edges rather than copied from statements.
- Added HIR body edges and declaration-rooted semantic path indexing for
  function/predicate/proof/Flow/Impl bodies, parameters, nested expressions,
  statements, patterns, Thread bodies, and synthetic `for` roots.
- Migrated runtime semantic reachability to statement/pattern typed edges. The
  checked `for` source, iterator witness, next value, pattern local, and body are
  reachable without restoring an all-trait-method fallback.
- Added opaque accepted declaration, expression, pattern, Match, coverage-domain,
  stable value, and stable pattern identities.
- Added exact literal transcripts, checked callable-contract participation,
  source-order arm/binding/guard/body transcripts, transcript byte/depth/work
  limits, false-guard precedence, unreachable evidence, and hard
  non-exhaustiveness rejection.
- Added one canonical sema project-runtime nominal schema/layout projection and
  used its core `RuntimeTypeSchema::try_layout_hash` result in nominal/variant
  and coverage-domain transcripts.
- Added exact bounded success for Bool, Unit, Option, Result, and closed project
  enums whose payload patterns are total. Dynamic guards do not contribute to
  coverage but are still tested for usefulness against prior rows.

## Fail-closed deviations

The implementation publishes no incomplete transcript for checked authorities
whose stable payload is not yet owned by current facts. It returns
`UnsupportedIdentity` or `UnsupportedCoverage` before a `CheckedMatch` or digest
is exposed for:

- project item/entry values; method, DialogueView, and Agent-field selects;
- StageLook, Await, Choice, Try, implicit callable/parameter, Pipe, View, Style,
  dialogue, and postfix-bracket resolution families;
- Entity patterns, Character/Builtin closed variant owners, and record fields
  lacking a checked accepted-field row; and
- tuple/record/array/Vec/Slice/Seq product coverage, non-Boolean literal/open
  residual coverage, and non-total variant payload decomposition.

These are missing completion products, not accepted non-goals. The sibling
correction request defines the remaining design closure. No source-name lookup,
raw HIR coordinate, wildcard success branch, or copied layout algorithm was
used to hide the gap.

## Validation

- `cargo test -p arcweft-lang-sema --lib --all-features`: 235 passed, 0 failed.
- `cargo test -p arcweft-lang-hir --lib --all-features`: 840 passed, 0 failed,
  8 ignored.
- `cargo check -p arcweft-lang-hir -p arcweft-lang-sema --all-targets
  --all-features`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- Structure audit and fail-on-blocking write gate: 0 blocking violations.
- Focused checked-Match tests cover statement roots, Thread/For closure, stable
  Option binding rows, guard usefulness, non-exhaustiveness, work limits,
  source-arm order, checked callable contract, project-enum layout evidence,
  and fail-closed tuple coverage.
- The first parallel HIR test attempt failed before running because D: had only
  166 MB free. `cargo clean` removed 259.1 GiB of generated main-checkout
  artifacts; the exact HIR command then passed. No source/WIP was removed.
- Strict changed-crate Clippy with `-D warnings` was attempted and stopped in
  unchanged dependencies before reaching Cut 1b code. Existing diagnostics are
  redundant-closure, doc-markdown, large-enum-variant, too-many-lines, and
  too-many-arguments findings in `arcweft-lang-syntax` and `arcweft-core`.

## Structural review

`final_analysis/semantic_transcript.rs` crosses the 1,200 LOC review trigger.
It currently owns one atomic construction transaction: checked path enrichment,
bounded transcript accounting, coverage, and final digest publication share the
same private counters and failure precedence. Splitting it before the remaining
coverage/identity correction would create an artificial private protocol that
must immediately change with that return. The follow-up implementation should
extract the final bounded coverage owner and closed payload encoders once their
complete types are fixed; this cut retains cohesion and the structural gate has
zero blocking findings.

## Remaining work

- Close and implement the linked complete transcript/coverage correction.
- Cut 2 ownership digests, Cut 3 compiler-local View admission, Cut 4 identity
  substrate, and Cut 5 atomic runtime switch remain separate reviewable cuts.
