# Lang-01.1.1 direct Need authority switch

Date: 2026-08-06

Inspected Git base: `454ca646d37b0e0e1226e181c5a501c9c8e8de15`

Working-tree state: dirty on `codex/proof-public-switch`; the implementation
is complete on the coherent public-switch copy and awaits the shared cut's
commit/push.

Supersedes the `MISSING` classification for direct `Need<T, E>` Ready/Err
materialization in:

- [the direct-style suspension record](2026-07-22-lang-01-1-1-direct-style-suspension-generator.md); and
- [the AWBC direct-suspension kernel record](2026-07-24-lang-01-1-1-awbc-direct-suspension-kernel.md).

## Performed

The Product AWBC await path no longer interprets a verified `NeedHandle` as a
task-plan reader. `AwbcTerminator::Await` now names its register as `handle`,
and VM suspension retains one typed `FiberAwaitTarget`:

- `Task(RuntimeValue)` keeps the existing explicit host-task lifecycle; and
- `Need(NeedId)` reads only producer-owned `RuntimeNeedState` input for the
  current deterministic runtime step.

`RuntimeNeedState` retains the logical epoch, exact `NeedId`, publication
sequence, and `Need<RuntimePayload, RuntimePayload>` state. Publications are
normalized deterministically. The first terminal publication for one identity
wins; a later publication cannot replace it.

The direct Need behavior is:

- `NotStarted` and `Pending` remain suspended without emitting a host task
  request or task-derived flow event;
- `Ready` resumes with the canonical runtime `Result::Ok` value;
- `Err` resumes with the canonical runtime `Result::Err` value and does not
  trap; and
- `Cancelled` performs the existing whole-fiber cancellation unwind.

`FlowFiberStatus::NeedWaiting(NeedId)` preserves this distinct blocked state
through the core facade, CLI server adapter, script runner, Agent runner, and
parity projection. `BundleStepInput` forwards the typed publications to the
runtime step unchanged.

## Deletion evidence

The old semantic conflation is unavailable:

- there is no `RuntimeValue::Need` surrogate;
- there is no Need-to-task-plan conversion in Product suspension;
- there is no `AwbcTerminator::Await { task: ... }` field;
- direct Need resolution does not emit `AwaitStarted`, `AwaitProgress`, or
  `AwaitReady` task events; and
- the already removed Capacity `old_dispatch_calls` counter and string helper
  remain absent.

No compatibility alias, fallback reader, source-string reconstruction, source
gate, new AWBC opcode, or provisional Stream wire shape was introduced.

## Passed validation

- `cargo test -p arcweft-need`: 3 passed;
- `cargo test -p arcweft-core awbc::product_step::tests:: --lib`: 14 passed;
- `cargo test -p arcweft-core --test direct_suspension`: 8 passed;
- `cargo check -p arcweft-core --all-targets --all-features`: passed;
- `cargo check -p arcweft-runtime-driver --all-targets --all-features`:
  passed; and
- `cargo clippy -p arcweft-need --all-targets --all-features -- -D warnings`
  and `cargo clippy -p arcweft-core --all-targets --all-features -- -D
  warnings`: passed.

The Ready/Err same-step tests use `RuntimeStepMode::Drain` with an explicit
64-operation budget. This proves there is no external host handoff when the
caller's deterministic budget permits the resume; it does not bypass the
normal one-operation budget contract.

## Coherent-copy closure validation

The 2026-08-09 final-copy rerun passed:

- `cargo test -p arcweft-need --all-features`: 3/3;
- `cargo test -p arcweft-core --test direct_suspension --all-features`: 8/8;
- `cargo test -p arcweft-core --lib --all-features
  awbc::product_step::tests::`: 20/20;
- `cargo check --workspace --all-targets --all-features`;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- `git diff --check`; and
- `just structure-audit` plus `just structure-audit-gate`, with zero blocking
  violations.

No separate Tier-2 runtime surface is introduced by this direct Need switch;
the relevant Product AWBC deterministic-step owners are the focused tests
above. Commit/push remains coupled to the shared deletion-driven public-switch
cut because its final HIR/sema/runtime files overlap that migration.

## Explicit non-goals

- authored ordinary-function AWBC kind allocation;
- `StreamFactory` runtime/wire/save projection, which remains in the accepted
  Lang-01.3 atomic Stream cohort;
- TTS production work; and
- any restoration of removed authored function roles or old Dialogue/Speaker
  carriers.
