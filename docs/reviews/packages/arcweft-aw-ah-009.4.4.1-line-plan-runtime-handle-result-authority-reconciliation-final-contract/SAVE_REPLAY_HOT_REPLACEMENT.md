# Bundle, save, replay, restore, and hot-replacement contract

## 1. Persistence boundary

Line handles are `SnapshotOnly` opaque values.  They may occur in a live save
only when the save also contains the owning dialogue activation, ledger, and
pinned artifact generation.  They are forbidden in authored constants,
standalone bundle constants, cache keys detached from a live activation, or
host-private renderer state.

All Arcweft-owned schema/version fields in this cut are exactly `1`.

## 2. Save schema

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DialogueActivationSnapshotV1 {
    pub version: u32, // exactly 1
    pub activation: DialogueActivationId,
    pub generation: RuntimeArtifactFingerprint,
    pub content: RuntimeDialogueContentPlanId,
    pub phase: DialogueRuntimePhase,
    pub parent_resume: RuntimeFlowResumeSnapshotV1,
    pub parent_result_target: RuntimeDialogueResultTargetSnapshotV1,
    pub activation_frame: Option<RuntimeActivationFrameSnapshotV1>,
    pub result: DialogueResultSnapshotV1,
    pub issuance_by_site: Vec<RuntimeLineSiteCounterSnapshotV1>,
    pub handles: Vec<RuntimeLineHandleSnapshotV1>,
    pub schedules: Vec<RuntimeScheduledLineTaskSnapshotV1>,
    pub reducer: LineTaskLiveSnapshotV1,
    pub command_sequence: u64,
    pub pending_commands: Vec<RuntimePendingCommandSnapshotV1>,
    pub elapsed: LogicalDuration,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DialogueResultSnapshotV1 {
    Uncommitted {
        ty: RuntimePlanTypeId,
    },
    Committed {
        ty: RuntimePlanTypeId,
        value: RuntimeValue,
    },
    Published,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeLineHandleSnapshotV1 {
    pub value_type: RuntimePlanTypeId,
    pub token: RuntimeLineHandleToken,
    pub owner: RuntimeHandleOwnerSlotSnapshotV1,
    pub state: RuntimeHandleLeaseState,
    pub resource: RuntimeHandleResourceSnapshotV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeScheduledLineTaskSnapshotV1 {
    pub token: RuntimeLineHandleToken,
    pub node: RuntimeLineTaskNodeId,
    pub deadline: LogicalDuration,
    pub captures: Vec<RuntimeTypedValueSnapshotV1>,
    pub state: RuntimeScheduledState,
}
```

`version` is checked for equality to `1`; there is no migration reader.

## 3. Resource persistence

A save stores logical resource facts only:

- exact Character and Character look semantic ids;
- voice source/session logical identity and lease count;
- stage actor token, Character, and lease state;
- cue kind, scheduled node/deadline or stage command id/state;
- pending typed command envelopes and deterministic command sequence.

Native window/renderer/audio object ids, Web object references, pointers,
channels, file descriptors, and task handles are not serialized.  Restore
reconstructs host resources through typed reconciliation commands after the
entire candidate validates.

## 4. Save points

A save is admitted at any engine safe point where:

- no affine transfer transaction is half-applied;
- no result pattern transaction is half-applied;
- every pending host request has a persisted typed envelope and command id;
- activation/AWBC frames are at a verified resume point;
- reducer state is represented by its canonical live snapshot;
- bounded value traversal completes.

Saving during `Activating`, `Ready`, `Closing`, and before publication is
supported.  Saving during the atomic commit or publication mutation is delayed
to the immediately following safe point; it is never serialized as partial
state.

## 5. Restore validation transaction

The decoder builds an isolated candidate.  The engine, active plan set,
renderer/audio host, observation stream, command queue, and parent fibers are
not mutated until all validation succeeds.

Failure precedence is exact:

1. **container/schema/limit** — magic, section length, version exactly `1`,
   duplicate fields, value depth/size, count limits;
2. **checked type** — every local, capture, result, owner slot, and handle value
   has the expected accepted type;
3. **opaque producer** — producer id, semantic identity, value class,
   persistence, and payload grammar;
4. **generation** — snapshot activation's artifact fingerprint is available and
   equals the pinned plan generation;
5. **activation** — token activation equals the enclosing activation, owner
   fiber/content/occurrence are consistent, and no duplicate activation exists;
6. **site/topology** — site ids, kinds, Character owners, issuance counters,
   scheduled child pairing, reducer node state, and owner paths;
7. **result** — state/type/value/pattern compatibility and affine paths;
8. **command** — command ids/sequences and pending request/lease correlation;
9. **host reconciliation preflight** — host declares it can reconstruct or
   deterministically reject every required logical resource.

The first category determines the primary error.  All checks within a category
are reported in canonical path order up to the diagnostic limit.

## 6. Commit and host reconciliation

After validation:

1. reserve engine activation/fiber identities in a private restore transaction;
2. send typed host preflight/restore requests in saved command order;
3. collect typed outcomes without exposing the activation to ordinary input;
4. if any outcome rejects, issue typed rollback for already reconstructed
   resources, discard the candidate, and leave pre-restore engine state intact;
5. atomically install parent fiber, dialogue state, ledger, schedule queue,
   reducer, frames, result, and observation sequence;
6. emit one `RestoreCommitted` observation;
7. resume deterministic execution.

Host-specific rollback failure is a secondary diagnostic; the restore remains
failed and the candidate is never installed.

## 7. Uncommitted and committed results

| Saved state | Restore behavior |
|---|---|
| `Uncommitted` in `HostPreparing/Activating` | resume activation function at exact frame/resume point; duplicate commit still rejected |
| `Committed` in `Activating` | restore hidden typed cell and result-owned handle paths; complete zero-cue/ready transaction |
| `Committed` in `Ready` | restore active line; parent remains unbound |
| `Committed` in `Closing` | restore close/reducer/cleanup state; publish only after joined terminal state |
| `Published` | legal only if parent continuation state already contains the committed bindings and dialogue is no longer active; otherwise tamper error |

A committed result is never reconstructed from a label or re-evaluated from
source.  Its exact RuntimeValue is persisted.

## 8. Deterministic replay

Replay records and consumes:

```rust
pub struct RuntimeLineReplayEventV1 {
    pub version: u32, // exactly 1
    pub logical_step: u64,
    pub activation: DialogueActivationId,
    pub command_sequence: u64,
    pub event: RuntimeLineReplayEventKindV1,
}
```

Recorded kinds include dialogue prepared outcome, voice start outcome, stage
command outcome, advance input, cancellation input, logical-time advancement,
and host failure.  Replay does not call a real host.  It verifies that the next
runtime request exactly matches the recorded activation, command sequence,
operation kind, producer, Character/look, and arguments before applying the
recorded outcome.

Because occurrence counters, issuance counters, deadlines, and scheduler order
are deterministic and persisted, replay produces the same handle tokens and
normalized trace.

## 9. Bundle behavior

Bundles contain accepted RuntimePlan/AWBC type and producer declarations,
line-operation tables, handle-site declarations, result patterns/types, and
host ABI digests.  They do not contain live line handles.

Bundle admission rejects:

- a line producer missing from the accepted runtime-type graph;
- a copied line-specific producer table;
- a `SnapshotOnly` opaque constant;
- an old string handle/out effect kind;
- an old `Dialogue` AWBC payload;
- any Arcweft-owned version marker other than `1`;
- a line operation unsupported by the declared host ABI digest.

## 10. Hot replacement

### 10.1 Active activation policy

An active dialogue, its parent suspended continuation, handle ledger,
scheduled children, result cell, and pending commands remain pinned to the
artifact generation that created them.  Installing a new artifact:

- affects new entry/flow/dialogue activations only;
- does not rewrite an active `DialogueActivationId`;
- does not reinterpret old handle sites or result types through the new plan;
- retains the old generation until all pinned fibers/activations complete or
  are explicitly cancelled and cleaned up.

An API that requests in-place active-dialogue replacement returns the
structured error `ActiveDialogueGenerationPinned` before mutation.

### 10.2 Handle use after replacement

- a handle used inside its still-pinned old activation is valid;
- a handle passed to a new-generation activation fails generation then
  activation validation;
- a stale host outcome from an old activation cannot complete a new command,
  even if source site and display text match;
- a save containing the old activation restores only when the exact old
  generation remains available.

### 10.3 Committed result

A committed result is interpreted under the pinned old result type and parent
pattern.  The old parent continuation publishes it.  New code does not receive
or reinterpret that value unless ordinary source-level control later crosses a
typed public boundary.

## 11. Transactional mismatch errors

```rust
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeLineRestoreError {
    #[error("line snapshot schema or limit mismatch")]
    SchemaOrLimit,
    #[error("line snapshot checked type mismatch at {path}")]
    Type { path: RuntimeValuePath },
    #[error("line snapshot opaque producer mismatch at {path}")]
    Producer { path: RuntimeValuePath },
    #[error("line snapshot generation mismatch")]
    Generation,
    #[error("line snapshot activation mismatch")]
    Activation,
    #[error("line snapshot schedule/ledger topology mismatch")]
    Topology,
    #[error("line snapshot result mismatch")]
    Result,
    #[error("line snapshot command correlation mismatch")]
    Command,
    #[error("host rejected resource restoration")]
    HostReconciliation,
    #[error("active dialogue is pinned to its original generation")]
    ActiveDialogueGenerationPinned,
}
```
