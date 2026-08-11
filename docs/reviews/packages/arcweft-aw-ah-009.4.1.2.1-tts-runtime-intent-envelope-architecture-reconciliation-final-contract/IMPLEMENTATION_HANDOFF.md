# Implementation handoff

## 1. Ordered crate/module cuts

### Cut 1 — freeze generic core and bridge contracts

1. Add workspace crate `crates/arcweft-audio-tts-runtime` with direct
   dependencies only on core, audio TTS, and `thiserror`.
2. In `arcweft-core::value`, add checked nominal expression construction,
   canonical decoder, generic binary budget, and inherent Result constructors.
3. In `arcweft-core::task`, add generic intent/request/outcome/registration
   types; change task error carriers to `RuntimePayload` in the same compiling
   cut.
4. Add bridge layouts, wrappers, recursive codecs, and fixed constants.
5. Add direct unit/property tests for all five payloads before connecting
   lowering.

No TTS-specific core variant, extension trait, downcast, or compatibility type
may exist even temporarily in the final cut.

### Cut 2 — AWBC codec 8 and structural verification

1. Replace `AwbcTaskPlan` with the exact codec-8 shape.
2. Add request/outcome/cancellation tags and nominal contracts.
3. Make `MakeRecord` consume its type operand and construct nominal ordinal
   records for public record types.
4. Add canonical runtime-value decode and AWBC truncation/trailing/unknown-tag/
   exact-limit tests.
5. Reject codec 7; do not retain a dual reader.

### Cut 3 — shared callable and runtime-plan lowering

1. Add `TtsCallableId` to the existing shared callable identity owner.
2. Register the three exact accepted ordinary callables and effects.
3. Add type-checking and source-map diagnostics through the existing registry.
4. Lower each callable identity to exactly one bridge-owned intent template,
   TTS class, priority 0, JoinSameKey, source cancellation scope, and exact
   outcome contracts.
5. Prove no source name string reaches runtime-plan dispatch.

### Cut 4 — atomic runtime-driver preparation

1. Add `arcweft-runtime-driver::tts` snapshot/preparer owners.
2. Change `BundleSession::dispatch_requested_tasks` to accept
   `RuntimeRequestedTask`.
3. Prepare every intent before sequence/admission/registry/pin/replay/host
   publication.
4. Add atomic terminal error publication for preparation failure.
5. Construct final key through selected-request inherent key text plus generic
   `TaskKey::try_new`.
6. Convert final selected payload to typed `TtsHostTaskDispatch` with the
   generic registration ID before returning privileged dispatch.

### Cut 5 — sole-scheduler admission, joining, and cancellation

1. Add inherent `RuntimeScheduler::submit_one -> Result<TaskAdmission, TaskAdmissionError>`.
2. Store exact observer records under one execution owner.
3. Publish one execution pin/dispatch for joined work.
4. Clone validated progress/terminal outcomes to observer IDs/sequences.
5. Replace scope-only host cancellation with targeted `cancel_tasks` and detach
   only matching observers.
6. Map TTS cancellation to typed `TtsError::Cancelled` via generic outcome
   contract.

### Cut 6 — typed host registration and outcomes

1. Add one `register_tts_synthesize` builder path and typed token.
2. Add typed `TtsHostAdapterRegistration`, `TtsHostTaskContext`, and the
   `submit_tts`, `drain_tts_updates`, `cancel_tts`, and `pump_tts_main_thread`
   registry methods; reject every other stage and keep the driver type out of
   host-adapter dependencies.
3. Connect runtime-host registration-ID/token matching, call-ID construction,
   provider pump, credential-slot-only provider request, and accepted protocol.
4. Encode/validate progress/result/error through bridge wrappers.
5. Complete the direct generic replacement of `TaskEventKind::Err`,
   `HostTaskOutcome`, every adapter failure producer, and every consumer.

### Cut 7 — Need, save, replay, and reload

1. Add `FlowEvent::AwaitErr`; retain exact outcome contracts in Await state.
2. Resume ordinary await with canonical `Result::Ok`/`Result::Err`.
3. Keep only `HostTasks` and `TaskGenerationPins` save blockers.
4. Correct replay schema 1 in place with typed task request/outcome variants.
5. Record/inject success and failure through the existing vector; fail omitted
   bytes explicitly.
6. Implement queued compatibility/fingerprint migration and active old-pin
   completion atomically.

### Cut 8 — deletion, structural audit, and Tier 2 closure

Delete provisional/legacy surfaces, run the complete matrix, and only then
publish the broad runtime cut.

## 2. Required direct deletions

The final implementation contains none of:

```text
arcweft-core dependency on arcweft-audio-tts
HostTaskRequestTemplate::TtsSynthesis*
HostTaskRequest::TtsSynthesisIntent
HostTaskRequest::TtsSynthesis
TtsRequest { voice, text }
TaskKey::for_tts in core
core hard-coded tts.synthesize / tts.synthesis branch
voice named argument compatibility
TaskEventKind::Err(String)
HostTaskOutcome failure String
RecordedExternalOutcome task failure message String
TTS operation-name registry branch
provisional bridge trait/helper/module
TTS-specific scheduler or replay vector
codec-7 dual reader or TTS V2 alias
```

Where an existing Arcweft enum gains generic behavior, add it to the enum's
own inherent implementation. Do not add local extension traits.

## 3. Migration inventory by current owner

| Current path/owner | Required direct replacement |
|---|---|
| `crates/arcweft-core/src/task.rs` | Remove provisional TTS request types/variants; add generic intent/outcome/registered request and typed error. |
| `crates/arcweft-core/src/engine/suspend.rs` | Remove string TTS branch/alias; evaluate generic intent; validate typed Ready/Err/Progress and construct Result. |
| `crates/arcweft-core/src/value.rs` and nominal owner | Add nominal expression, checked constructor, Result constructors, decoder/binary limit. |
| `crates/arcweft-core/src/awbc/schema.rs`, codec, verifier, VM | Codec 8 typed request/outcome; nominal `MakeRecord`; no dual reader. |
| `crates/arcweft-lang-sema/src/callable/identity.rs` and registry | Add exact TTS callable IDs/signatures/effect. |
| `crates/arcweft-runtime-plan` | Lower selected callable IDs to one nominal intent template. |
| `crates/arcweft-runtime-driver/src/session.rs` | Atomic intent preparation before publication; typed dispatch. |
| `crates/arcweft-runtime-driver/src/task.rs` | Typed failure helper, terminal error publication, selected dispatch form. |
| `crates/arcweft-runtime-scheduler/src/lib.rs` | Admission result, observer map, targeted cancellation, typed cancellation fan-out. |
| `crates/arcweft-host-adapter/src/lib.rs` and new `tts` module | Typed token/context/domain update path; no driver/bridge dependency or operation string. |
| `crates/arcweft-runtime-host/src/native_task.rs` | Route typed TTS dispatch and encode domain outcomes. |
| `crates/arcweft-runtime-driver/src/session/replay/*` | Direct schema-1 typed task outcomes in existing vector. |
| `crates/arcweft-runtime-driver/src/session_save.rs` | Reuse blockers; add no TTS save wire. |
| runtime-driver swap/hot-swap owners | Exact queued tuple/fingerprint migration and active old pin. |

## 4. Structural audit triggers

Implementation is blocked from merge if any audit finds:

- a Cargo path from core to an audio/TTS crate;
- more than one scheduler owner or TTS queue outside the accepted host adapter
  and sole scheduler;
- a TTS operation string in core/runtime-plan/driver/host dispatch;
- `Any`, downcast, JSON/TOML, type-name serialization, extension trait, or local
  duplicate codec;
- host/provider API accepting intent, driver-owned input, or generic TTS payload;
- selected request/provider fields in AWBC intent, save, or replay request
  identity;
- a second external-outcome vector/log;
- a TTS-specific save blocker;
- compatibility/V2/legacy alias or old source spelling;
- text/provider key/credential/audio bytes in diagnostics or debug labels.

Audits must use Cargo metadata, Rust type/visibility tests, structured AWBC
inspection, and runtime behavior. Source-text gates alone are not acceptance
evidence.

## 5. Validation commands

Run from repository root after implementation:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
cargo metadata --format-version 1 --all-features > target/tts-cargo-metadata.json

cargo test -p arcweft-audio-tts-runtime
cargo test -p arcweft-core awbc
cargo test -p arcweft-runtime-plan tts
cargo test -p arcweft-runtime-driver tts
cargo test -p arcweft-runtime-scheduler
cargo test -p arcweft-host-adapter tts
cargo test -p arcweft-runtime-host tts
cargo test -p arcweft-save

cargo test -p arcweft-core --test compile_fail
cargo test -p arcweft-host-adapter --test compile_fail
```

Then run the repository's applicable Tier 2 native, Web, headless, and Agent
checks named by the latest `AGENTS.md`/CI configuration. Do not claim a gate
that is absent; record the exact command and result in implementation evidence.

## 6. Completion evidence package

The implementation handoff must record:

- Git commit and Jujutsu change ID of the implementation checkout;
- `cargo metadata` allowed/forbidden edge assertions;
- exact codec/layout golden hashes from this contract;
- full matrix row results, including exact/one-over cases;
- native/Web/headless/Agent gate logs;
- deletion audit output;
- confirmation that no provisional bridge remains.
