# Producer outcome authority — 2026-08-21

## Inspected state

- Base Git revision: `61704d3b11315ea3331cf6dfbd65011fe109bde1`
  (`Replace binary Need with unary temporal payloads`).
- Branch: clean `main`, matching `origin/main` before this cut.
- Working tree while this record was written: dirty with the coherent producer
  outcome implementation described below.
- Sequencing authority:
  [`Post-Try convergence implementation order`](2026-08-18-post-try-convergence-order.md),
  section 4.

## Implemented result

Host tasks and Product AWBC now retain one checked temporal payload coordinate:

1. `TaskOutcomeContract` owns only `payload: RuntimeCheckedType`.
   `AwbcTaskPlan` owns only `payload_type`. Their version-1 codecs evolved in
   place; no binary outcome reader or compatibility DTO remains.
2. `TaskEventKind::Error` and `HostTaskCompletion::Error` were deleted.
   `Ready` carries the complete admitted payload. A fallible producer publishes
   `Ready(Result::Ok(value))` or `Ready(Result::Err(error))`; `Failed` remains an
   infrastructure/runtime fault and `Cancelled` remains a control outcome.
3. Structured and Product AWBC Await validate each host publication against
   the checked payload contract and resume with that value unchanged. They no
   longer synthesize a Result from separate task Ready/Error coordinates.
4. Native file, system-info, desktop, and Agent controller producers construct
   their full Result payload at the adapter boundary from the accepted
   contract. Exact opaque error owners remain responsible for wrapping their
   nominal payloads.
5. RuntimePlan can lower an infallible `Need<T>` host call directly; it no
   longer requires every Await payload to be a Result.
6. Task progress publication is narrowed to the canonical
   `arcweft_need::Progress` owner. Its validation-preserving serde codec now
   lives with `Progress` and is reused by RuntimeValue and TaskEvent.

## Validation performed

### Passed

- `cargo fmt --all`
- `git diff --check`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features`: exit status 0.
  Existing advisory warnings remain; the one newly enlarged Await-many owner
  was subsequently decomposed through a shared payload-validation helper.
- `just test-doc`: passed.
- `just structure-audit-gate`: passed with 0 blocking violations.
- `cargo test -p arcweft-agent-runner --lib`: 47 passed.
- `cargo test -p arcweft-core --lib`: 215 passed after replacing the obsolete
  synthetic-Result expectation.
- `cargo test -p arcweft-need --lib`: 4 passed, including validated Progress
  codec round-trip and invalid-ratio rejection.
- `cargo test -p arcweft-runtime-scheduler --lib`: 8 passed.
- `cargo test -p arcweft-runtime-plan --lib`: 47 passed, plus its API compile,
  assertion identity, Product parity, and iterator integration suites.
- `arcweft-runtime-driver`: 58 library tests, 21 Product-session tests, 4
  dialogue-view-store tests, and 29 View runtime tests passed.
- `arcweft-adapter-desktop`: 6 library tests and 1 native integration test
  passed.
- `arcweft-runtime-host`: 34 of 35 tests passed on the first full run; the sole
  stale system-info test used a default Unit outcome. After giving it the
  checked Result contract, its exact rerun passed.
- `spec_should_pass/check/056_await_infallible_need_payload.arcw` compiled to a
  RuntimePlan and passed Product AWBC verification with zero diagnostics.
- `spec_should_pass/run/002_file_read_task.arcw` executed under both structured
  bytecode VM and Product AWBC. The missing-file domain error arrived as
  `AwaitReady(Result::Err(...))`, matched as a value, and returned `"missing"`.

### Environment maintenance

- A prior test build exhausted `D:` while linking generated Cargo artifacts.
  After verifying the exact workspace target, `cargo clean` removed 283.7 GiB
  from `D:\git\arcweft\target`. Source and user data were not removed.

## Structural review

The touched owners remain cohesive: `arcweft-core::task` owns the checked task
contract and event algebra; AWBC schema/codec/verifier/mapping owns its sole
wire projection; RuntimePlan owns typed producer lowering; host adapters own
materialization of their admitted values; scheduler and runtime-driver consume
the closed event algebra. `arcweft-need::Progress` now owns its own validated
serialization instead of relying on a core-only copied helper. No dependency
direction was reversed and no source spelling is reinterpreted.

## Remaining work and non-goals

- This cut does not yet introduce the canonical `RuntimeNeedProducer` sum type
  for host tasks and the later Timeout producer. It removes the binary outcome
  dependency that blocked that owner.
- Pending observer body execution, publication latching, re-wait, AWBC observer
  routing, and reactive View observation remain the next atomic cut.
- Stream error coordinates are unaffected; Stream is not a temporal Need.
- No timeout combinator is introduced here.
