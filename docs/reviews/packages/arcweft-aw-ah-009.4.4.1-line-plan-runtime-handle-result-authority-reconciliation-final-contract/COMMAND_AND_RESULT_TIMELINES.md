# Command, schedule, and result phase timelines

## 1. Two-phase dialogue activation

The line setup cannot run before the host establishes the dialogue/voice, and
the dialogue cannot become externally ready before setup and result commit
succeed.  Activation is therefore a deterministic two-phase transaction.

| Seq | Runtime phase | Action | Observable output |
|---:|---|---|---|
| 1 | parent flow | evaluate dialogue content value functions and parent captures in source order | value-evaluation trace only |
| 2 | allocate | allocate `DialogueActivationId`, hidden result cell, ledger, activation frame, command sequence | `DialogueActivationAllocated` |
| 3 | host preparing | emit typed dialogue activation request with exact content, Character, voice policy, activation id | `HostRequest::DialogueActivate` |
| 4 | host preparing | validate typed host response; create presentation and `Absent/Lazy/Ready` voice state | `HostOutcome::DialoguePrepared` |
| 5 | activating | execute `LineTaskGroup.activation_ops` in source order; suspending typed requests are allowed | line-operation request/outcome events |
| 6 | activating | `CommitDialogueResult` validates R and transfers affine leaves to hidden result owner | `DialogueResultCommitted`, value digest only |
| 7 | activation microstep | enqueue and run zero-deadline joined callbacks; settle their required host operations | cue/command events |
| 8 | ready commit | atomically expose presentation, enable advance, set elapsed zero | `DialogueReady` |

If any step before 8 fails, the host-prepared presentation is aborted through
typed cleanup and no `DialogueReady` is emitted.  The parent pattern is still
unmodified.

## 2. Primary fixture activation order

```text
DialogueActivate(alice, voice=auto)
DialoguePrepared(voice=Ready(session V))
AcquireActor(site 0) -> StageActorHandle A
Schedule(0.42s, captures=[A], child 0, site 1) -> CueHandle C
VoiceHandle(site 2, session V) -> VoiceHandle V0
CommitResult((V0, C))
DialogueReady
```

The scheduled callback is not executed during setup because its deadline is
0.42s.  The result already owns V0 and C while the dialogue is active; the
parent still cannot observe them until publication.

## 3. Exact `at` timeline

### Evaluation

1. Evaluate delay expression.
2. Convert to `LogicalDuration`.
   - negative: `NegativeCueDelay`;
   - non-finite/invalid representation: `InvalidCueDelay`;
   - conversion overflow: `CueDeadlineOverflow`.
3. Compute `deadline = elapsed.checked_add(delay)`.
4. Evaluate callback captures left-to-right.
5. Validate capture types, ownership, depth, and size.
6. Read and checked-increment the site's issuance counter.
7. Insert schedule live state and cue lease atomically.
8. Return/bind/register the exact `CueHandle`.

### Zero delay

A zero cue is due only after `CommitDialogueResult` completes.  It runs in the
activation microstep before `DialogueReady`.  This prevents callback
re-entrancy into an uncommitted result and lets a zero-time setup callback
affect the first visible frame.  A failing joined zero cue aborts activation.

### Same-tick advance arbitration

For an input advance at logical time T:

1. advance elapsed to T;
2. collect every armed cue with deadline `<= T`;
3. order by `(deadline, handle_site, issuance, child_node)`;
4. run/join all due work;
5. if any due work fails, fail the dialogue and do not apply advance;
6. apply the advance;
7. freeze future cue arming;
8. close/cancel future cues and run cleanup;
9. publish result.

Therefore:

- cue before advance: runs before advance;
- cue exactly at advance: runs before advance;
- cue after advance: is cancelled during close and does not run.

## 4. `actor.look` command timeline

1. Evaluate actor value, look value, then crossfade expression.
2. Validate exact opaque actor type and decode token.
3. Validate producer, generation, activation, owner slot, active lease, and
   exact Character.
4. Validate `CharacterLookId.character == actor.character`.
5. Validate crossfade duration.
6. Allocate a cue token at the look call's handle site and insert an
   `Allocating` cue lease.
7. Allocate monotonically increasing `RuntimeStageCommandId` from the dialogue
   command sequence.
8. Enqueue:

```rust
RuntimeStageCommand::SetCharacterLook {
    command,
    activation,
    cue,
    actor,
    character,
    look,
    crossfade,
}
```

9. Suspend the action fiber until typed outcome when the host contract is
   acknowledging; a host capable of immediate deterministic acceptance may
   return in the same engine step but still emits request then outcome.
10. On `Accepted`, mark cue pending/running and return the handle.  On
    `Rejected`, mark failed and propagate callback failure.

An unbound result remains owned by the current child/line scope.  `Some(Discard)`
is the only immediate-drop spelling.

## 5. Voice-handle timeline

| Voice state | Operation |
|---|---|
| `Ready(session)` | issue one affine lease token, increment session lease count, return handle |
| `Lazy(ticket)` | emit typed `StartDialogueVoice { activation, ticket }`, suspend activation, validate response, set `Ready`, issue lease |
| `Absent` | fail activation with `MissingActiveVoice`; do not return a fake/no-op handle |
| `Failed(error)` | fail with `VoiceStartRejected(error)` |
| `Completed(session)` | issue a completed affine lease to the exact completed session; the handle supports identity/status/drop only and cannot restart playback |

The primary `voice=auto` fixture must reach `Ready` or a successful lazy start.

## 6. Result lifecycle

### Producer commit

`CommitDialogueResult` is legal only in an admitted result-producing function
for the current activation.  It:

1. evaluates the expression;
2. validates exact `R` recursively;
3. enumerates affine leaves in canonical `RuntimeValuePath` order;
4. validates every ledger owner and transfer;
5. atomically transfers them to `DialogueResult(path)`;
6. stores `Committed { ty, value }`;
7. terminates that completing path.

No parent local changes here.

### Consumer publication

On successful joined close:

1. enter `Publishing`;
2. validate the stored R again against the pinned accepted type;
3. simulate the sole target pattern in a temporary binding transaction;
4. simulate all handle transfers/discards;
5. if all succeed, commit parent locals and ledger transitions;
6. enqueue typed drop commands for explicit discards in path order;
7. mark result `Published`;
8. resume the parent at the dialogue continuation.

This is the only parent binding boundary.  In the primary fixture, VoiceHandle
is explicitly discarded and CueHandle moves to `outer_cue`.

## 7. Cancellation and nonlocal control

| Event | Before producer commit | After producer commit, before ready | Ready/active | Closing/publishing |
|---|---|---|---|---|
| ordinary cancellation | abort activation; no result | abandon result; cleanup | run cancellation rule and close policy | idempotent close request; no second rule |
| completing cancellation rule | must commit R on its admitted path | may replace only if cell still uncommitted; otherwise duplicate error | commit then close/publish | cannot commit after publication begins |
| callback failure | abort/fail | fail and abandon | fail, cancel siblings, failed cleanup | failure wins until publish transaction commits |
| parent return/goto | close/abandon before control transfer | close/abandon | close/abandon | waits for current atomic publish; after commit, normal nonlocal semantics apply to parent |
| host disconnect | typed host failure | typed host failure | typed host failure | cleanup failure is secondary diagnostic; primary host failure retained |

Primary failure wins over cleanup failure.  Cleanup diagnostics are attached as
ordered secondary causes and never replace the original result/host/callback
failure.

## 8. Host ordering key

Every host-visible request uses:

```text
(activation, command_sequence)
```

The normalized global ordering is:

```text
(engine_logical_step, phase_rank, activation, command_sequence)
```

Phase ranks are fixed:

```text
0 activate request
1 activate outcome
2 activation line operation request
3 activation line operation outcome
4 result commit
5 zero-cue request/outcome
6 ready
7 active cue request/outcome
8 advance
9 cancellation
10 cleanup request/outcome
11 result publish/bind
12 resume/status
```

Native, Web, and headless hosts must not reorder commands with the same
activation.  Cross-activation ordering follows the engine's accepted fiber
scheduler order.
