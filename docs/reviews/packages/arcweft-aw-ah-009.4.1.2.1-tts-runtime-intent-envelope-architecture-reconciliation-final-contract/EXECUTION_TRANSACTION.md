# Execution transaction

## 1. Source-to-host sequence

```text
Arcweft source
  | ordinary call resolved by shared callable registry
  v
HIR call + ResolvedCallable(Builtin::Tts(TtsCallableId))
  | checker verifies exact signature/effect/Need<TtsAudioAsset,TtsError>
  v
arcweft-runtime-plan
  | constructs selector/options expressions by accepted field ordinals
  | TtsIntentPayload::template(...)
  v
RuntimeTaskRequestTemplate::Intent + exact TaskOutcomeContract
  | structured RuntimePlan or AWBC codec 8
  v
arcweft-core evaluator
  | evaluates one nominal record expression
  v
RuntimeRequestedTask::Intent(TaskIntentSpec)
  | BundleSession::dispatch_requested_tasks
  v
arcweft-runtime-driver::tts (atomic preparation)
  | decode intent -> accepted catalog + one availability snapshot
  | selected TtsSynthesisRequest -> fingerprint -> final TaskKey
  | selected nominal payload + typed registration -> TaskSpec
  v
sole RuntimeScheduler
  | Scheduled or Joined by final key
  v
runtime-driver HostTaskDispatch::Tts(TtsHostTaskDispatch)
  | generic numeric registration ID; no operation string; no intent
  v
runtime-host
  | match retained typed registration; derive call ID; build TtsHostTaskContext
  v
HostAdapterRegistry::submit_tts(context, selected request)
  | host policy, credential lease, attempt allocation
  v
TtsProviderSynthesisRequest (credential slot only)
  | provider executor / AWTP 1
  v
TtsProgress | TtsAudioAsset | TtsError
  | bridge encode -> typed HostTaskOutcome -> scheduler normalization
  v
TaskEventKind::{Progress, Ready, Err}(RuntimePayload)
  | core AwaitState exact contract validation
  v
Need pending / Result::Ok(TtsAudioAsset) / Result::Err(TtsError)
```

## 2. Atomic preparation success

For each `TaskIntentSpec`, runtime-driver performs a local transaction:

```text
P0 borrow immutable Arc<TtsPreparationSnapshot>
P1 verify class/policy/priority/outcome contracts
P2 decode exact TtsSynthesisIntent payload
P3 validate text/options limits
P4 select profile/speaker/provider/binding/format against snapshot
P5 seal TtsSelectionEvidence and fingerprint
P6 build key tts.v1.<64 lowercase hex>
P7 encode selected payload and build registered TaskSpec
P8 validate the selected payload and all outcome/cancellation contracts,
   build the debug label, and reserve every publication slot needed by P9–P12
--- commit boundary under exclusive &mut BundleSession ---
P9 allocate logical sequence
P10 submit to sole scheduler
P11 publish observer registry state
P12 if Scheduled, publish the execution generation pin
P13 if dispatched, construct typed HostTaskDispatch
P14 replay substitution or privileged host dispatch
```

P0–P8 do not mutate scheduler, registry, sequence counter, generation pins,
replay recorder, host registry, or provider state. P9–P12 are one infallible
in-memory commit under the same exclusive session borrow: after P8 they perform
no validation, allocation, host call, replay call, or externally observable
intermediate publication. A panic is not an accepted failure path; every
validation/selection/capacity failure is typed before the commit.

## 3. Preparation failure

```text
same TaskId / NeedId / cancel scope
  -> exact TtsError from catalog/availability, or request-stage
     ProtocolFailure/InvalidPayload for malformed executable data
  -> encode and validate TtsErrorPayload locally
  -> prevalidate sequence capacity and absence of a terminal record for TaskId
  -> allocate one logical sequence and
     RuntimeTaskRegistry::publish_terminal_error as one infallible commit
  -> queue TaskEventKind::Err(error_payload)
  -> core observer resumes with Result::Err(TtsError)
```

The negative postconditions are exact:

```text
scheduler submissions delta = 0
scheduler in-flight delta = 0
active registry records delta = 0
host dispatches delta = 0
generation pins delta = 0
replay external outcomes delta = 0
provider submissions delta = 0
credential leases delta = 0
```

The terminal failed record is visible only when completed records are explicitly
requested; it is created atomically with the event and never appears Pending or
Running. Duplicate-task, contract, payload, and sequence-capacity checks occur
before the mutation; the commit itself has no fallible step.

## 4. Task identity and joining

The selected request fingerprint and exact key remain the accepted lower
contract. The key includes all selected semantic coordinates and excludes task
observer coordinates.

```text
included:
  selected profile/Character presence+IDs; logical speaker; provider; binding;
  provider-key digest; exact text; locale; style; rate; pitch; format;
  extension IDs+values; profile/provider/availability/capability/config digests;
  artifact identity; ABI; timeout; retry

excluded:
  TaskId; NeedId; cancellation scope; priority; queue position; attempt;
  wall clock; credential locator/value; provider trace; progress; result bytes;
  accepted_generation as an independent field
```

`TaskAdmission::Scheduled` creates the execution owner. `Joined` creates only an
observer relation. Same-key join requires exact class, policy, and outcome/
cancellation-contract equality; priority differences join and retain the first
admitted execution priority. The scheduler owns one execution-to-observers map:

```rust
struct TaskObserver {
    task_id: TaskId,
    sequence: TaskSequence,
    cancel_scope: CancelScopeId,
    outcome: TaskOutcomeContract,
}
```

This is scheduler state, not a second scheduler or TTS log. One owner pin covers
the execution. Joined observers carry no pin.

## 5. Dispatch and host transaction

At scheduler dispatch, runtime-driver consumes the final `TaskSpec`:

1. require `TaskClass::TtsSynthesis`, `JoinSameKey`, key prefix and exact 64-hex
   suffix;
2. require `HostTaskRequest::Registered` with the expected typed registration;
3. decode and revalidate `TtsSelectedRequestPayload`;
4. require selected request fingerprint equals key suffix;
5. construct `TtsHostTaskDispatch` and remove the generic payload from the
   privileged-facing value;
6. allow replay substitution or hand the selected dispatch to runtime-host;
7. runtime-host matches its retained `TtsHostAdapterRegistration` by numeric ID,
   derives the accepted 16-byte call ID as generation u64 little-endian followed
   by sequence u64 little-endian, constructs `TtsHostTaskContext`, and calls the
   one `HostAdapterRegistry::submit_tts` path.

Any failure before step 7 produces no provider I/O. Registration mismatch is a
host-contract error mapped to typed `ProtocolFailure/InvalidPayload` and stable
`tts.runtime.registration-missing` or selected-payload diagnostic.

The TTS adapter then constructs `TtsProviderSynthesisRequest` from the selected
request plus host-only call ID, attempt, credential slot, and timeout clock. It
does not reselect provider/binding/format and does not read source intent.

## 6. Progress transaction

```text
provider event
  -> TtsHostAdapter state-machine validation
  -> HostAdapterRegistry::drain_tts_updates with typed registration
  -> monotonic phase and values; adjacent same-phase coalescing
  -> maximum 128 published events
  -> TtsProgress domain value
  -> TtsHostTaskUpdate::Progress(TtsProgress)
  -> runtime-host bridge encodes exact TtsProgressPayload
  -> HostTaskUpdate::Progress(RuntimePayload)
  -> driver contract validation
  -> scheduler clones event to all live observers
  -> each core AwaitState emits AwaitProgress for its Need and stays pending
```

Progress carries no text, provider key, credential, raw provider message, trace
payload, or audio bytes. A regression, wrong nominal/layout, event after
terminal, or 129th non-coalescible event is a typed protocol failure.

## 7. Success transaction

Success is published only after the accepted provider protocol, byte-count and
digest match, complete format probe/decode, nonempty duration, supported sample
rate/channels, and spool finalization.

```text
TtsAudioAsset domain value
  -> TtsHostTaskUpdate::Completed { result: Ok(TtsAudioAsset), metrics }
  -> runtime-host bridge encodes exact asset nominal payload, including bytes
  -> HostTaskOutcome { result: Ok(RuntimePayload), metrics }
  -> driver validates nominal/layout/digest/32 MiB cap
  -> replay recorder captures the same typed outcome when recording
  -> scheduler completes owner and clones Ready to live joined observers
  -> registry marks all observers completed
  -> owner generation pin released after host cleanup and event publication
  -> AwaitReady / Result::Ok(TtsAudioAsset)
```

The asset is not automatically played, installed in an `AudioGraph`, assigned
an `AudioVoiceId`, or projected to Agent output.

## 8. Error transaction

All provider/catalog/runtime failures use exact nominal `std.audio.TtsError`.

```text
TtsError domain value
  -> TtsHostTaskUpdate::Completed { result: Err(TtsError), metrics }
  -> runtime-host bridge encodes exact TtsErrorPayload
  -> HostTaskOutcome { result: Err(RuntimePayload), metrics }
  -> driver validates exact nominal/layout/variant payload
  -> replay records same typed failure when external
  -> scheduler clones Err to every live observer
  -> AwaitErr / Result::Err(TtsError)
```

A wrong generic error payload is not displayed or wrapped as a string. It is
diagnosed and replaced with exact
`ProtocolFailure { stage: Completion, code: InvalidPayload }`.

## 9. Observer cancellation and execution cancellation

### One observer of a joined execution

```text
cancel scope A
  -> detach only observers in A
  -> each detached TTS observer receives Err(TtsError::Cancelled)
  -> execution continues if any observer remains
  -> no host cancel and no owner-pin release
```

### Final observer

```text
final detach
  -> publish typed Cancelled error to that observer
  -> enqueue one targeted execution TaskId in cancel_tasks
  -> runtime-host calls HostAdapterRegistry::cancel_tts with the typed token
  -> adapter sends one provider cancel / cleanup
  -> discard later progress/results
  -> release execution generation pin after cleanup terminalization
```

### Queued owner before host dispatch

The scheduler removes the execution, terminalizes all live observers with typed
Cancelled, emits no `HostTaskDispatch`, and releases the owner pin. Cancellation
is never retried or converted to provider failure.

## 10. Deterministic ordering

Logical epoch is the emitting core tick. Sequence is allocated in request order
only after successful preparation or immediately before atomic terminal failure
publication. Scheduler and registry normalize by
`(logical_epoch, sequence, task_id)`. Joined copies preserve each observer's own
sequence. Host completion order never changes core observation order.

## 11. Debug and diagnostic transaction

The ordinary final debug label remains exactly:

```text
tts.synthesize text_bytes=<n> locale=<locale> format=<format>
```

No preselection debug label is published. Diagnostic structured fields are
bounded and redacted; the diagnostic channel is separate from typed Need error
payloads and cannot replace them.
