# Ordered compile-clean implementation plan

## Stage 0 — intake

- record exact current `main` SHA, branch, dirty state, worktrees, target usage;
- re-read root/scoped AGENTS and Rust Skill;
- verify both parent ZIPs and this correction archive;
- map every owner named in `CONSUMER_AND_DELETION_INVENTORY.md`;
- add an implementation evidence note; no source gate.

## Stage 1 — additive generic ownership APIs

- land `RuntimeValueOwnership`, slot revisions, transaction-owned prepared drop, checked copy/move, closed payload, snapshot DTO corrections;
- add allocator cursor type and activation-domain owner;
- keep Stream handle unconstructible;
- focused tests compile without removing old Clone surfaces yet.

## Stage 2 — migrate structured runtime and View consumers

- replace environment/local/capture/aggregate clone paths;
- add View static ownership facts and transfer intent;
- migrate existing View evaluator, parameter/default/state/repeat/nested-call inputs to checked copies;
- migrate handler input to move;
- reject may-be-affine retained View types;
- migrate View save to whole-execution snapshot values.

At exit, no View path requires live `RuntimeValue` Serde or unchecked clone.

## Stage 3 — ownership-complete ABI 1

- add `CopyValue = 0x2a` under ABI 1;
- correct existing Move/Drop semantics and inherent operand-use metadata;
- migrate verifier, VM, fiber, compiled-region and runtime-plan lowering;
- update all ABI expectations from 2 to 1;
- regenerate all AWBC fixtures/caches/artifacts; no old reader.

## Stage 4 — snapshot activation and allocator

- serialize exact allocator cursor in schema 2;
- move restore activation to `RuntimeExecutionDomain`;
- add domain-wide two-driver tests;
- make replacement lease transfer atomic;
- validate first post-restore mint;
- remove direct per-driver activation entrypoints.

## Stage 5 — View static requirement wire

- add `static_requirements` to the direct-replacement transcript;
- include requirement and input-transfer facts in semantic/revision digests;
- update certificate schema/digest joins;
- implement strict span containment and outermost dispatch;
- update tamper/hot-replacement/save parity tests.

## Stage 6 — remove unconditional authority leaks

After every workspace consumer uses checked APIs:

- remove `Clone`/Serde from live executable owners;
- remove `bindings_snapshot()` and equivalent facade copies;
- delete old prepared-drop APIs;
- delete ABI-2 names/constants/tests;
- delete View legacy save/input/static fallback paths.

This is one coherent public switch; no compatibility alias remains.

## Stage 7 — Stream parent publication

Only after Stage 6:

- publish `ExternalStreamPartial` and `StreamHandle` on the generic owner;
- run parent P4+C1 through P8+C6 in retained order;
- ensure View retained boundaries reject Stream handles and affine aggregates.

## Stage 8 — full gates

- focused unit/codec/compile-fail/reference/parity tests;
- `cargo fmt --all -- --check`;
- `cargo check --workspace --all-targets --all-features`;
- strict workspace Clippy;
- workspace tests and relevant Tier 2;
- Cargo metadata/dependency graph;
- canonical structure audit/gate;
- deterministic generated artifact comparison;
- commit/push coherent cuts and record skipped/blocked rows honestly.
