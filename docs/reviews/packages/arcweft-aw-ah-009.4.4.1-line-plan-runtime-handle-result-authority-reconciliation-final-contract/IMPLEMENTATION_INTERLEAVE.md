# Compile-clean implementation interleave

This is an implementation order, not a production overlay.  Each numbered
interleave is one coherent compile gate.  No gate may add a compatibility
alias, dual reader, fixture switch, source recognizer, or generic dynamic
fallback.

## I0 — freeze evidence

- record `origin/main` full SHA;
- re-read root/crate/local `AGENTS.md`;
- run repository searches for all symbols in `DELETION_MATRIX.md`;
- capture the current failing RUN-037 CLI diagnostic;
- ensure working tree/user changes are preserved.

Gate: evidence note only; no production behavior change.

## I1 — direct semantic types and callable owners

Change together:

- add direct `TypeKind` capability/handle variants;
- keep `StageMethodId` and add its runtime operation mapping method;
- add `LineContextMethodId` and `LineScheduleCallableId`;
- remove `LineContext.voice_handle` from `CapacityMethodId`;
- replace temporary `Named` return/receiver tests and exhaustive matches;
- keep Character look exact ownership.

Gate:

```text
cargo check -p arcweft-lang-sema -p arcweft-compiler -p arcweft-runtime-plan --all-targets --all-features
cargo test -p arcweft-lang-sema <exact new callable/type tests> --all-features
```

There is no temporary alias back to `Named("CueHandle")`.

## I2 — opaque affine authority

Change together:

- extend original `RuntimeOpaqueTypeOwner` and `RuntimeOpaqueValue`;
- extend original `RuntimeValue::ownership`, save bytes, validators, paths,
  constants, and AWBC type conversion exhaustively;
- add exact line handle producer declarations to the existing type inventory;
- reject SnapshotOnly constants.

Gate: core value/pattern/ownership/codec tests, compile of every exhaustive
RuntimeValue/Opaque consumer, Clippy without new allow attributes.

## I3 — RuntimePlan owner and admission

Change together:

- extend original `FlowOp`;
- extend original `LineTaskGroup`, child trigger/capture/live state;
- add direct builder/admission APIs and limits;
- lower HIR source order into activation ops, handle sites, child graph, commit;
- add result target to `FlowOp::Dialogue`;
- remove runtime semantic projection exclusion for Stage/line/at calls.

Gate: HIR/sema existing tests remain green; runtime-plan focused positive and
negative admission tests compile and pass.  RUN-037 must now reach structured
execution rather than a projection fallback.

## I4 — structured activation/result/ledger

Change together:

- extend `DialogueState` and parent suspension coordinate;
- add dialogue activation transaction;
- add original ledger implementation and ownership transfers;
- execute activation ops in their own owner frame;
- implement hidden result commit and atomic publish/abandon;
- update return/goto/cancellation/failure unwind.

Gate: core structured tests for tuple result, destructuring, `_`, duplicate and
missing result, normal/cancel/fail/nonlocal exit.

## I5 — typed schedule, actor, voice commands

Change together:

- implement `Schedule` evaluation/capture/issuance/deadline;
- replace authored Delay trigger with Scheduled site;
- add typed stage command owner and native test host adapter;
- implement exact actor/look proof and cue lifecycle;
- implement voice Ready/Lazy/Absent/Failure paths;
- implement same-tick cue/advance ordering and joined cleanup.

Gate: structured runtime timing/failure/identity tests and headless command
trace tests.  No pure `at`, wait lowering, or no-op stage call exists.

## I6 — delete string handle/result route atomically

In the same compile gate:

- delete `LineOutRequest`;
- delete `LineEffectRequest::{RegisterHandle,DropHandle,Out}`;
- delete observation, core, runtime-host, CLI, bundle, and test match arms;
- make handle drop use the original affine drop implementation;
- make line result use only dialogue result state;
- add compile-fail/API absence tests.

Gate: workspace compile proves no old constructor remains; repository search
matches only source request/history/deletion evidence and negative compile
fixtures.

## I7 — AWBC schema/codec/verifier atomic cut

Change in one gate:

- schema tables, opaque fields, function kind, line operations, handle sites,
  scheduled trigger, group, result target, opcodes;
- codec writer/reader/tag tables and canonical digest traversal;
- remove old effect kinds and old Dialogue/Delay readers;
- verifier structure/code/limit/result dataflow;
- runtime-plan AWBC lowering.

Gate: schema/codec roundtrip, explicit tag tests, malformed old payload
rejection, type/topology/result tamper tests.  ABI and codec versions assert `1`.

## I8 — AWBC VM/fiber/product-step/snapshot

- execute typed line operations;
- run activation frame and result cell;
- route reducer commands to exact AWBC child functions with issued captures;
- persist suspension/ledger/schedule/result;
- update product step and common reducer mapping;
- delete old effect VM/product mappings.

Gate: AWBC focused execution plus structured/AWBC differential traces for every
operation/exit class.

## I9 — all host adapters

- native renderer/audio mapping;
- Web DTO and correlation;
- headless deterministic model;
- runtime driver/bundle runner;
- host rejection and cleanup secondary-error ordering.

Gate: the same typed command corpus against all adapters; no adapter parses a
callable/handle label.

## I10 — bundle/save/replay/hot replacement

- bundle schema/digest/admission;
- save version-1 replacement and restore transaction;
- replay typed event log;
- pinned-generation retention and active-replacement rejection;
- transactional rollback tests.

Gate: save at every phase, replay identity equality, tamper precedence, hot
replacement before/while/after active dialogue.

## I11 — CLI, Agent, fixtures, docs

- normalized Agent/CLI observation rendering;
- remove edge fixture skip/allowlist;
- run unchanged RUN-037 through check, structured, AWBC, CLI;
- run simpler mark fixture;
- update maintained chapters/contracts;
- record implementation evidence.

Final gates:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p arcweft-cli --all-features
# plus repository-required tiered/differential/tamper suites
```

## Atomicity constraints

- I1 removes `Named` spellings in the same gate that adds direct variants.
- I3 removes projection exclusions in the same gate that adds exact lowering.
- I6 removes all string routes in one gate; no producer can select them after
  I3 and no consumer remains after I6.
- I7 changes schema, codec, verifier, lowering, and tags together; there is one
  version-1 reader.
- I8 changes AWBC live/snapshot shapes together; no defaulting old fields.
