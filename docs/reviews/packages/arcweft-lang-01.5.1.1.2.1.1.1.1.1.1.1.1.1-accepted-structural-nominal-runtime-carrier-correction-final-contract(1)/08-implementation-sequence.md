# 08. Implementation sequence and admission gates

This is a design-only package. The sequence below is the concrete future production change plan and intentionally contains no patch.

## Phase 0 — owner lock and baseline

1. Checkout `UNAVAILABLE` (or rebase the design against a newer `origin/main` and record the replacement SHA).
2. Re-read every applicable `AGENTS.md` for files to be edited.
3. Confirm the owner anchors in `01-evidence-basis.md` by opening the full files, not only grep snippets.
4. Record baseline `fmt`, `check`, focused tests, workspace tests, and clippy. Do not attribute pre-existing failures to the change.

**Gate G0:** one named owner for carrier, checked plan, codec, and coordinator; no duplicate enum/API family.

## Phase 1 — canonical IDs and owner enum

1. Reuse or extend existing canonical structural-shape and nominal-instance interning.
2. Add the `Structural`/`Nominal` representation to the original runtime carrier/value enum.
3. Add constructors, accessors, invariant validation, equality/hash semantics, and errors to its inherent `impl`.
4. Migrate construction sites; keep any temporary compatibility constructor `pub(crate)` and delete it in Phase 5.

**Gate G1:** T1–T5, T15–T16 pass; no side table, extension trait, or debug-name identity.

## Phase 2 — checked constraint and projection witness

1. Normalize aliases and generic args in checked typing.
2. Emit `AcceptedCarrierConstraint` and, only when legal, `StructuralProjectionWitness`.
3. Include both in semantic child encoding/sealing and the coverage digest.
4. Make invalid/opaque projections a checked diagnostic rather than a runtime guess.

**Gate G2:** T6–T19 pass in focused compiler/runtime test suites.

## Phase 3 — runtime execution and transcript

1. Replace any shape-only/nominal-recovery fallback with `AcceptedRuntimeCarrier::admit`.
2. Execute precompiled projection steps.
3. Emit stable transcript rows from the same constraint table.
4. Measure hot path to ensure no allocation/catalog traversal/full-shape hashing.

**Gate G3:** complete admission matrix; execution and coverage digest remain isomorphic.

## Phase 4 — snapshot codec and two-phase restore

1. Add unresolved wire records and canonical encode/decode.
2. Add resolver validation in dependency order.
3. Stage carriers with payloads/plans/tasks; publish through the coordinator barrier.
4. Add golden vectors, corruption tests, round-trip and deterministic-order property tests.

**Gate G4:** T20–T31 pass, including atomic failure and byte-for-byte re-encoding.

## Phase 5 — closure and cleanup

1. Remove legacy constructors, fallbacks, compatibility aliases, and independently generated coverage/runtime domains.
2. Run `rg` proof searches for forbidden side tables, shape-to-nominal recovery, raw ID serialization, and duplicate carrier enums.
3. Run all command gates and update relevant design/implementation docs with exact SHA and logs.

**Gate G5:** T32 plus workspace format/check/test/clippy; no production TODO/placeholder/Open Question.

## File-level edit map

| Order | Current/proposed owner | Changes |
|---:|---|---|
| 1 | `crates/<language-owner>/src/checked/type.rs` | canonical IDs, normalized accepted constraint, projection witness emission |
| 2 | `crates/<runtime-owner>/src/value.rs (new module only if no owning enum exists)` | carrier variants and inherent construction/admission behavior |
| 3 | `crates/<runtime-owner>/src/match_exec.rs` | sole use of checked constraints; transcript and coverage digest tie |
| 4 | `crates/<runtime-owner>/src/snapshot.rs` | stable wire record, canonical codec, unresolved resolver |
| 5 | `crates/<runtime-owner>/src/task.rs` | staged dependency graph and atomic publication/wakeup |
| 6 | adjacent test modules/fixtures | T1–T32 and golden vectors |

## Migration rule

At no point may old and new carrier authorities both be public. Introduce new internals, migrate all producers/consumers in one gated series, then remove the old path before admission. Compatibility at a persistence boundary must be a versioned decoder path, not a silent semantic fallback.
