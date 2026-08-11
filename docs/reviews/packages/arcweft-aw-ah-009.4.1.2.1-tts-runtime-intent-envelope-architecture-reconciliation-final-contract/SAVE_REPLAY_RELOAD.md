# Save, replay, and reload

## 1. Executable program data versus runtime-only data

| Executable RuntimePlan/AWBC/bundle data | Runtime-only selected/execution data |
|---|---|
| `TtsCallableId` | selected provider and binding |
| selector variant and authored IDs/expressions | raw provider speaker key and digest |
| exact text expression | accepted profile/provider/availability digests |
| locale/options expressions | selected/defaulted locale/style/rate/pitch/format |
| intent nominal ID/layout/field ordinals | request fingerprint and final TaskKey |
| TTS class, JoinSameKey, priority 0, cancel scope | registration ID and host dispatch |
| ready/error/progress/cancellation contracts | logical sequence and generation pin |
| accepted profile/provider catalogs in AWFB sections 22/23 | attempt, call ID, credential slot/lease |
| callable/effect/Need types | progress, buffers/spool, result bytes before completion |

No unprepared intent is serialized into scheduler, host, save, or replay state.
The only serialized intent is executable program structure in RuntimePlan/AWBC.

## 2. Save contract

Active or queued TTS work is not resumable. The existing blockers remain the
only blockers:

```text
BundleSessionPendingBlocker::HostTasks { active, queued_events }
BundleSessionPendingBlocker::TaskGenerationPins { active }
```

The correction does not add a TTS save variant, task snapshot, selected-request
wire, pending intent, progress snapshot, partial spool, credential, provider
catalog, provider key, adapter call, or retry state.

A completed `TtsAudioAsset` retained in ordinary durable nominal state is saved
through the existing nominal runtime-value path. Per asset binary bytes are
bounded to 32 MiB; aggregate TTS asset bytes per save remain 256 MiB. Restore
validates nominal identity, layout, exact ordinals, content digest, and limits
before publication.

Joined observers count as active host tasks for `HostTasks`. Only the execution
owner has a `TaskGenerationPins` entry. A preparation failure has neither
blocker after its terminal event is consumed.

## 3. Replay schema 1 direct correction

`ROOT_REPLAY_SCHEMA_VERSION` remains exactly `1`; engine identity remains
`arcweft.root-replay.v1`. The existing `external_outcomes` vector is the sole
external log.

```rust
pub struct RecordedExternalTaskRequestV1 {
    pub task_id: TaskId,
    pub key: TaskKey,
    pub class: TaskClass,
    pub logical_epoch: LogicalEpoch,
    pub sequence: TaskSequence,
}

pub enum RecordedExternalRequestV1 {
    HostCall(RuntimeHostCallId),
    Task(RecordedExternalTaskRequestV1),
}

pub struct RecordedExternalOutcome {
    pub position: RecordedExternalOutcomePositionV1,
    pub request: RecordedExternalRequestV1,
    pub outcome: RecordedExternalOutcomeResultV1,
    pub root_event_sequence: Option<TransitionSequence>,
}

pub enum RecordedExternalFailureV1 {
    HostCall {
        kind: RecordedHostCallErrorKindV1,
        message: String,
    },
    Task(RuntimePayload),
}

pub enum RecordedExternalOutcomeResultV1 {
    Success(RuntimePayload),
    SuccessOmitted {
        type_id: RuntimeNominalTypeId,
        layout: TypeLayoutHash,
        value_digest: RuntimeValueDigest,
        omitted_bytes: u64,
    },
    Failure(RecordedExternalFailureV1),
}
```

This directly corrects schema 1 in place. Generic host-call failures retain the
existing typed kind plus bounded message; task failures are always
`RecordedExternalFailureV1::Task(RuntimePayload)`. There is no task-error
message fallback and no TTS-specific outcome vector. Task identity uses the
already selected TaskKey and deterministic epoch/sequence; no intent or provider
operation string is recorded. The replay verifier rejects a HostCall failure for
a Task request and a Task failure for a HostCall request.

## 4. Recording transaction

1. preparation and scheduler admission complete;
2. one execution owner reaches typed `HostTaskDispatch::Tts`;
3. recorder appends the exact `RecordedExternalRequestV1::Task` identity;
4. live host execution proceeds;
5. after bridge encoding and driver contract validation, recorder stores the
   exact terminal success asset or error payload;
6. joined observers are not separately recorded; scheduler fan-out is
   deterministic internal behavior;
7. preparation failures are not external outcomes because no dispatch exists.

A success can be omitted only when the configured generic replay byte budget
requires it. The marker stores type/layout/value digest and exact omitted byte
count; it is not replayable success.

## 5. Playback transaction

1. load the same artifact/catalog identity and schema-1 trace;
2. lower/evaluate the ordinary call to a typed intent;
3. prepare the selected request against the replay construction snapshot;
4. verify selected key/class and admit through the sole scheduler;
5. at the owner dispatch seam, match the next recorded task request exactly;
6. suppress host/provider dispatch;
7. validate and inject recorded success/error through the ordinary scheduler
   completion path;
8. scheduler fans out to joined observers; core Need observes typed values.

Replay never admits an unprepared intent and never calls a provider to repair a
recording. A missing, duplicate, out-of-order, wrong-key, wrong-class,
wrong-sequence, wrong nominal/layout, digest-mismatched, truncated, trailing, or
over-limit outcome is a replay error.

`SuccessOmitted` always fails with `tts.replay.result-bytes-omitted`, including
expected type/layout, digest, and byte count as structured non-sensitive fields.
No partial asset reaches Need.

## 6. Typed replay failures

The existing replay error owner adds exact variants:

```rust
pub enum RecordedTaskPayloadRole {
    Success,
    Failure,
}

pub enum RootReplayError {
    // existing variants
    ExternalTaskMismatch {
        external_index: u32,
        expected: RecordedExternalTaskRequestV1,
        actual: RecordedExternalTaskRequestV1,
    },
    InvalidExternalTaskPayload {
        external_index: u32,
        role: RecordedTaskPayloadRole,
        source: RuntimePayloadContractError,
    },
    OmittedExternalTaskBytes {
        external_index: u32,
        type_id: RuntimeNominalTypeId,
        layout: TypeLayoutHash,
        value_digest: RuntimeValueDigest,
        omitted_bytes: u64,
    },
}
```

These are structured replay-control errors, not task error strings. When a
recorded `Failure(RecordedExternalFailureV1::Task(error))` is valid, it is
injected as `TaskEventKind::Err` and Need receives the exact `TtsError`.

## 7. Queued reload

Only a fully selected scheduler task can be queued. The driver retains its
selected request and accepted execution evidence; no intent is retained for
host/replay.

A queued request migrates only when the following accepted tuple is byte-equal
between old and candidate generations:

```text
profile semantic digest
selected binding ID
provider ID
provider-key digest
capability digest
public-config digest
artifact identity
ABI hash
credential-ref canonical text
protocol ID
```

The candidate-generation transaction is exact:

```text
R0 build and validate candidate catalog/availability/typed registration off to side
R1 decode the already selected queued TtsSynthesisRequest
R2 call TtsAcceptedCatalog::rebind_queued_request(previous, availability,
   candidate_generation); this validates the exact compatibility tuple and
   reconstructs selection evidence without a TtsSynthesisIntent
R3 require identical fingerprint and TaskKey; build the replacement registered
   TaskSpec with the candidate numeric registration ID
R4 acquire exclusive &mut BundleSession, verify the pending slot and old pin,
   and prevalidate `RuntimeScheduler::replace_pending_in_place(execution,
   expected_key, replacement)`
R5 perform the in-memory pending-task replacement and generation-pin swap as one
   infallible commit under that exclusive borrow
R6 retain original TaskId, observer set, logical epoch, sequence, priority,
   cancellation scopes, and scheduler position
```

R0–R4 are fallible and publish nothing. R5 has no validation or allocation and
cannot fail. Any tuple, availability, selected-evidence, fingerprint, TaskKey,
registration, or slot mismatch terminalizes every live observer with exact
`TtsError::CatalogChanged`, removes the queued execution, emits no host dispatch,
and releases the old pin. No preparation error from a candidate generation is
substituted for `CatalogChanged` during queued migration.

The queued selected request is the sole migration input. The suspended core
evaluator remains waiting on its TaskId/NeedId and is never consulted for source
arguments. Save remains blocked while the selected execution is queued.

## 8. Active reload

An execution that has crossed typed host dispatch remains bound to the old
selected request, adapter registration, credential reference, and generation
pin. It completes, fails, times out, or cancels under that generation. The new
generation receives no duplicate dispatch and cannot replace its provider,
binding, key, options, or output contract. The old pin is released only after
terminal host cleanup and event publication.

## 9. No second persistence path

The implementation must not add a TTS replay file, TTS task journal, selected
request save record, intent save record, V2 compatibility reader, omitted-byte
provider fallback, or alternate reload queue. Structural audits in the test
matrix enforce one scheduler, one replay external-outcome vector, and the two
existing save blockers.
