# AW-AH-009.4.4.1 line-plan opaque affine handle cut

Date: 2026-08-21
Implementation base: `22df44b808f8b2ae238357b2171b2de6ebd14266`
Working tree before implementation: clean; `main` matched `origin/main`

## Scope established

This cut extends the original checked opaque runtime owner instead of adding a
parallel line-handle value family. `RuntimeOpaqueValue` and
`RuntimeOpaqueTypeOwner` now retain two closed admission facts:

- `RuntimeOpaqueValueClass::{Plain, AffineHandle(RuntimeHandleKind)}`; and
- `RuntimeOpaquePersistence::{ConstantAndSnapshot, SnapshotOnly}`.

`RuntimeHandleKind::{StageActor, Cue, Voice}` owns the exact standard producer
identity for each line handle:

- `std.line.stage_actor_handle`;
- `std.line.cue_handle`; and
- `std.line.voice_handle`.

Compiler projection maps the direct semantic `StageActorHandle`, `CueHandle`,
and `VoiceHandle` families to this one opaque owner. An exact stage actor uses
`ExactIdentity`; the erased `StageActorHandle(Any)` storage type uses
`ProducerWide`. All three handle families are affine and snapshot-only.
`StageApi` and `LineContext` remain non-value capabilities and continue to fail
closed at runtime projection.

The existing RuntimePlan checked-type projection, plan type table, AWBC type
row and codec, verifier, VM materializer, save snapshot, and bundle type
consumer now preserve the same class and persistence facts. No schema, codec,
wire, save, or digest-domain version was bumped; every Arcweft-owned marker
remains `1`.

## Constant, replay, and snapshot boundaries

Snapshot-only opaque handles are rejected from:

- RuntimePlan literals;
- AWBC opaque constants and VM constant materialization;
- replay-safe root values; and
- canonical entry/schema constant bytes.

Live fiber/save snapshots retain and validate the complete opaque owner and
accept an exact snapshot-only affine handle. Tampering with class,
persistence, producer, semantic identity, admission, or type arguments fails
closed. Recursive values use one `contains_nonconstant_opaque` traversal, so a
handle cannot enter a constant or replay boundary by nesting inside a sequence,
tuple, record, variant, choice, or opaque payload.

Plain opaque values retain their former unrestricted ownership and
constant-and-snapshot behavior. There is no legacy reader, defaulted decode,
side table, string fallback, or second RuntimeValue variant.

## Structural review

The primary owner remains `arcweft-core`: opaque values and checked owners own
admission, ownership, persistence, value acceptance, and canonical tags;
RuntimePlan and AWBC carry exhaustive projections of those facts. The compiler
only maps checked language handle types into that lower-layer authority.
`arcweft-bundle` consumes the AWBC type family without becoming an owner.
Dependency direction and Sans-I/O boundaries are unchanged, and no crate
dependency or public facade was added.

The repeated manual projection trigger was reviewed across core,
runtime-plan, compiler, and bundle. Every projection carries the same closed
class and persistence enums; there is no copied registry or independently
mutable mapping. The larger touched files keep their existing cohesive owners:
AWBC schema/verification/materialization, RuntimePlan construction, runtime
value traversal, semantic-fact publication, and compiler lowering. This cut
adds fields to those existing exhaustive boundaries and does not add a new
responsibility cluster or test-only public API. The retained generated audit is
under
`docs/implementation/structure-audits/aw-ah-009-4-4-1-line-plan-opaque-handles-2026-08-21/`.
It measured 2,169 files, 2,041 Rust files, 1,014,124 Rust physical LOC, 95
workspace packages, 196 review triggers, and zero blocking violations.

## Validation

Passed:

- `cargo fmt --all`
- `cargo check -p arcweft-core -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features`
- `cargo check --workspace --all-targets --all-features`
- `cargo test -p arcweft-core --lib` (219 passed)
- `cargo test -p arcweft-runtime-plan --lib` (49 passed)
- `cargo test -p arcweft-compiler --lib` (55 passed)
- `cargo test -p arcweft-bundle --lib` (132 passed)
- focused opaque owner, affine ownership, snapshot round-trip, tamper rejection,
  constant rejection, AWBC codec/verifier, and RuntimePlan projection tests
- `just structure-audit`
- `just structure-audit-gate` (zero blocking violations)
- Tier 2 MCP stdio (4 passed)
- Tier 2 Select production boundaries (5 passed)
- Tier 2 Flow production boundaries (2 passed)
- checked-in visual artifact well-formed validation (1 passed)
- `git diff --check`

The strict core Clippy command was performed. It reports only four existing
findings outside this cut: the large `AwbcProductExecutorStatus` variant, long
functions in `engine/eval.rs` and `pure.rs`, and the argument count of
`TaskState::new_with_outcome`. All findings introduced while developing this
cut were decomposed before this record.

The first `just test-workspace` attempt was blocked by a full D: drive. Cargo
clean removed 166,993 workspace-local, reproducible build-artifact files
(295.2 GiB); no source, documentation, fixture, or external artifact was
removed. The fresh run then found and this cut corrected one stale bundle
encoding assertion for the already-owned `Progress = 122` value family.

The final `just test-workspace` run reached the HIR library suite and stopped on
the existing
`missing_choice_body_keeps_choice_payload_and_poisoned_outer_owner` failure:
834 passed, 1 failed, and 8 ignored in that suite. The failure is a syntax-node
kind mismatch in Choice recovery and does not traverse the opaque handle
boundary.

The exhaustive Tier 2 command was performed and its stopped recipes were
continued individually. Existing failures remain outside this cut:

- Agent observe capture finds two prepared-text owners for one rich-text
  object;
- native auxiliary and renderer golden fixtures retain removed Character body
  members, or reach the separately unfinished RichText runtime projection;
- one of six Select boundaries fails at `retired prefill revision stays
  current`.

These failures were not hidden or repaired through compatibility paths.

The unchanged RUN-037 command
`target/debug/arcw.exe run tests/fixtures/arcw/spec_should_pass/run/011_dialogue_line_value_and_handle_discard.arcw --json --steps 5`
was performed and intentionally failed at
`compiler.runtime_semantic_projection`: the selected `StageMethod` still has
no typed runtime intrinsic. It passes the new handle-type projection and stops
at the next package boundary.

## Remaining package work

- add typed RuntimePlan line operations, result targets, handle sites, and
  operation-specific admission limits;
- lower Stage/LineContext/schedule semantic callables into those operations;
- converge structured execution, AWBC instructions, host commands/results,
  persistence, and cleanup behavior; and
- delete the old string handle/result path atomically once every final consumer
  uses the typed authority.

RUN-037 remains stopped at that StageMethod typed runtime-intrinsic boundary
until the next operation cut.
