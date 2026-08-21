# Structured/AWBC and host behavioral parity

## 1. Shared authority

The structured executor and AWBC VM share:

- accepted RuntimePlan type/opaque owner inventory;
- `RuntimeLineHandleLedger` and exact token validator;
- `LineTaskLiveState` reducer transitions;
- typed stage/voice/dialogue command envelopes and outcomes;
- result commit and publication transaction implementation;
- logical time and same-tick ordering;
- normalized observation encoder;
- limit constants and work accounting.

They differ only in executable body representation:

| Role | Structured | AWBC |
|---|---|---|
| activation body | `FlowOp` slice | `LineActivation` function |
| child/cancel/cleanup body | `FlowOp` slice | `LineTask` function |
| expression/register state | structured fiber env | verified AWBC frame |
| suspension coordinate | `FlowCursor` | `AwbcResumePointId` |

No executor has an alternate handle registry, schedule reducer, result slot, or
host command materializer.

## 2. Normalized observation grammar

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeLineObservationV1 {
    pub version: u32, // exactly 1
    pub sequence: u64,
    pub logical_step: u64,
    pub activation: DialogueActivationId,
    pub event: RuntimeLineObservationKindV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuntimeLineObservationKindV1 {
    ActivationAllocated,
    HostRequest { command_sequence: u64, command: RuntimeCommandDigest },
    HostOutcome { command_sequence: u64, outcome: RuntimeOutcomeDigest },
    HandleIssued { token: RuntimeLineHandleToken, kind: RuntimeHandleKind },
    HandleTransferred { token: RuntimeLineHandleToken, destination: RuntimeOwnerDigest },
    HandleDropped { token: RuntimeLineHandleToken, final_state: RuntimeHandleLeaseState },
    CueArmed { token: RuntimeLineHandleToken, deadline: LogicalDuration },
    CueStarted { token: RuntimeLineHandleToken },
    CueFinished { token: RuntimeLineHandleToken, status: RuntimeChildStatus },
    ResultCommitted { ty: RuntimeTypeDigest, value: RuntimeValueDigest },
    DialogueReady,
    DialogueAdvance,
    DialogueClosing { exit: ScopeExit },
    CleanupStarted { exit: ScopeExit },
    CleanupFinished { exit: ScopeExit },
    ResultPublished { pattern: RuntimePatternDigest },
    ParentResumed,
    Failed { diagnostic: RuntimeDiagnosticDigest },
}
```

Digests are computed from canonical typed values/ids.  They never include
pointer addresses, executor-local register numbers, native object ids, thread
ids, wall-clock time, or display/debug labels.

## 3. Exact successful trace order

For the primary fixture with an advance after 0.42s:

```text
ActivationAllocated
HostRequest(DialogueActivate)
HostOutcome(DialoguePrepared)
HandleIssued(StageActor site0/0)
HostRequest(AcquireActor)
HostOutcome(Acquired)
HandleIssued(Cue schedule site1/0)
CueArmed(deadline=0.42s)
HandleIssued(Voice site2/0)
ResultCommitted((Voice site2/0, Cue site1/0))
DialogueReady
CueStarted(site1/0)
HandleIssued(Cue look site3/0)
HostRequest(SetCharacterLook)
HostOutcome(Accepted)
CueFinished(site1/0, completed)
DialogueAdvance
DialogueClosing(completed)
CleanupStarted(completed)
HandleDropped(unexported actor/look handles in canonical order)
CleanupFinished(completed)
HandleDropped(result voice due outer `_`)
HandleTransferred(result cue -> parent local)
ResultPublished(pattern digest)
ParentResumed
```

If native host reports look completion as a later outcome, that outcome and
cue final state appear at the same logical point in both executors.

## 4. Differential comparison

A differential test compares:

1. canonical host request bytes;
2. canonical host outcome consumption bytes;
3. normalized observation bytes;
4. final parent environment canonical bytes;
5. dialogue/child/handle status vector;
6. diagnostics including primary/secondary ordering;
7. queued commands and next command sequence;
8. save snapshot canonical bytes at selected safe points.

Any difference is failure.  Tests do not normalize away handle tokens, result
values, status, ordering, or diagnostics.

## 5. Native, Web, and headless behavior

| Host | Required behavior |
|---|---|
| Native | map exact typed Character/look/resource ids to renderer/audio objects; preserve command order; return typed outcomes |
| Web | encode/decode the same tagged command/outcome DTO; no JavaScript string parsing of callable/handle labels; preserve u64/bytes identities losslessly |
| Headless | validate catalog/ownership, record the same request, return deterministic success/rejection, and maintain logical resource states without rendering |

Host-local capability denial uses the same rejection code and failure ordering.
A renderer cannot reinterpret or repair a malformed producer, activation, or
Character proof; core rejects it before host dispatch.

## 6. Ordering parity matrix

| Boundary | Required order |
|---|---|
| activation | allocate → request → outcome → setup ops → result commit → zero cues → ready |
| schedule | delay eval → captures → issue → arm → return/bind |
| cue vs advance | every deadline `<= advance time` completes/fails before advance |
| cancellation | cancellation selected → child cancellation/join → cleanup → result publish/abandon |
| host rejection | request → rejection → primary failure → joined unwind → cleanup secondary diagnostics |
| result | commit hidden cell → line active/close → publish pattern → parent resume |
| explicit `_` | full pattern validation → kept transfers and discard drops in canonical path order → binding commit |

## 7. Agent and CLI observation

Agent observation and CLI output consume the normalized typed observations.
They may render human-readable labels after validation, but labels are output
only.  Neither can feed a rendered handle/result string back into execution.

CLI success for RUN-037 requires:

- check succeeds;
- structured run exits successfully and returns `"done"`;
- AWBC run produces the same result and trace;
- CLI stdout/stderr/exit classification is identical for both modes;
- no fixture allowlist or edge-fixture skip remains.
