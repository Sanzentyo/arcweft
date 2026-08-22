# Runtime convergence Cut 4 — core identity and catalog substrate

Date: 2026-08-22

Originally inspected base: `423bc649a1755669c45dedce04cdd9706f710e4f`.
The reviewable commit was rebased without conflict onto
`d428cac5b9550dcc6ba9976f5bf9b1e025ec55d7` before final integration
validation.

Worktree: `D:\git\arcweft-cut4`, branch `codex/cut4-core-identity-catalog`.
The worktree was clean at intake. This note records the implementation
working state before the reviewable local commit; generated `target/` output
was removed once after disk exhaustion and is not a source change.

## Performed

- Moved `GenerationId` ownership to `arcweft_core::task` with an explicit
  zero-valid constructor/getter and removed the runtime-driver-owned type.
- Added `TaskLaunchOrdinal::JOIN` and the version-1 semantic digest owners
  used by later task/Need cuts.
- Added the typed `NeedProducerFamily`/`NeedProducerSpec` substrate. The sole
  instance identity authority is `NeedProducerSpec::instance_key`; the
  canonical domain is `arcweft.need.producer-instance.v1\0`.
- Added typed Host route/operation identities, request contracts, canonical
  `HostOperationCatalog`, and a core-owned `ViewTaskPlanAuthority` protocol.
  Catalog construction now consumes a typed Builtin/custom operation input,
  computes the catalog digest without a self-reference, and privately seals
  every retained row with its full `HostOperationIdentity`. Both Builtin and
  catalog-bound custom identities resolve through the same final row table.
- Added `RuntimeCheckedType::semantic_identity_digest`, refactored the existing
  exhaustive RuntimeValue visitor to one private sink-parametric traversal with
  byte and direct-BLAKE3 sinks.
- Updated runtime-driver/player-native consumers to use the core-owned
  `GenerationId` API. No `NeedId`, `TaskKey`, `TaskId`, or
  `TaskCorrelation` migration was attempted; those are Cut 5 authority.
- Reconciled the rebased pattern semantic digest with Cut 2's final
  `RuntimeOpaqueValueClass::semantic_tag` and
  `RuntimeOpaquePersistence::semantic_tag` owners. The removed
  `canonical_tag` methods were not restored as compatibility aliases.

## Passed

- `cargo fmt --all -- --check`
- `cargo test -p arcweft-core -p arcweft-runtime-driver --lib
  --all-features` — core 224 and runtime-driver 58 passed after rebase.
- `cargo check -p arcweft-core -p arcweft-runtime-driver --all-targets
  --all-features` — passed after rebase.
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` — blocking
  violations: 0.
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  target/structure-audit --fail-on-blocking` — blocking violations: 0.
- `git diff --check`

Focused identity/catalog tests cover generation zero, Join ordinal zero,
producer-spec field sensitivity, Builtin and catalog-bound custom Host catalog
lookup, and custom RuntimeValue sink equivalence.

## Failed or blocked

- `cargo clippy -p arcweft-core --lib --all-features -- -D warnings` remains
  blocked by pre-existing warnings in AWBC/pure evaluation plus the existing
  `TaskSpec::new_with_outcome` arity warning. New Cut 4 code has no remaining
  clippy diagnostics after the documented lint fixes.
- `cargo check -p arcweft-player-native --lib` is blocked by the existing
  missing asset `web/assets/noto-sans-jp-vf.ttf` included by
  `arcweft-player-scene/src/fonts.rs`.
- The original pre-rebase runtime-driver test attempt was blocked by the host
  disk reaching zero free bytes while compiling. The final post-rebase test
  passed in the normal worktree target after generated artifacts were cleaned.

## Not run

Workspace-wide tests and Tier-2 runtime targets were not run. They require the
player asset blocker and additional disk headroom to be resolved and are
review-cut validation, not identity-substrate evidence.

## Non-goals and deviations

This cut does not publish `RuntimeTaskPlan`/`RuntimeTaskPlanTable`, TaskSpec/Need
handles, journal/observer mutation, AWBC changes, scheduler adapter batches,
snapshot migration, or persistent View rows. Sol max re-audit found that a
caller-supplied task-plan digest is self-certifying, while canonical structured
plan recomputation depends on the Cut 3 child semantic encoders. The row and
table are therefore deferred to Cut 5 rather than publishing a partial or
parallel authority in Cut 4. `TaskPlanSemanticDigest` remains only as the typed
completed-digest reference in `NeedProducerSpec`. No compatibility reader,
alias, string-based identity bridge, or raw fixed-identity constructor was
added.
