# Decision register

| ID | Decision | Exact resolution | State |
|---|---|---|---|
| `D-01` | Retain NeedProducerInstanceKey transcript | family, contract, plan, producer site, payload type and sole RuntimeValueDigest; no extra serializer | `FROZEN` |
| `D-02` | Retain Join/AlwaysStart ordinals | Join=0; AlwaysStart journal counter begins at 1 and consumes only successful accepted launches | `FROZEN` |
| `D-03` | Retain TaskKey/TaskId split | TaskKey excludes ordinal; TaskId includes it exactly once | `FROZEN` |
| `D-04` | Retain handle policy | reusable pre-launch handles are Join-only; AlwaysStart handle is launch output | `FROZEN` |
| `D-05` | Move GenerationId to core | arcweft-core owns the sole type; scheduler derives complete correlation; driver-local owner is deleted | `CORRECTED` |
| `D-06` | Retain zero policy | fixed producer/Need/task identities reject all-zero; semantic digests accept every hash and Option represents absence | `FROZEN` |
| `D-07` | Retain product separation | generic Match, Need producer admission and View admission remain separate products | `FROZEN` |
| `D-08` | Retain View roles | ViewProgramId is identity; AcceptedViewProgramRevision([u8;32]) is accepted revision and is not producer identity | `FROZEN` |
| `D-09` | Retain outcome categories | domain errors remain payload; infrastructure failure is typed; cancellation is nonreturning Need cancellation | `FROZEN` |
| `D-10` | Retain opaque evidence | value class and persistence are mandatory catalog evidence | `FROZEN` |
| `D-11` | Do not reopen AWBC allocation | semantic opcode range, function kinds/flags, u32 varint, encoder/reader and no tombstones stay final | `FROZEN` |
| `D-12` | One canonical visitor | the original exhaustive RuntimeValue visitor is sink-parametric; byte and BLAKE3 sinks share it | `CORRECTED` |
| `D-13` | SnapshotOnly has value identity | Plain+SnapshotOnly uses existing opaque transcript; no producer-specific digest | `CORRECTED` |
| `D-14` | Constant publication is a separate fence | all constant publishers explicitly reject SnapshotOnly/affine/NeedHandle/runtime-local values | `CORRECTED` |
| `D-15` | Affine diagnostic digest rejected | a non-producer RuntimeValueDigest cannot encode affine handles; typed snapshots remain separate | `CLOSED` |
| `D-16` | TaskSpec has one execution field | TaskExecution Host|Runtime; no unconditional or parallel optional request fields | `CORRECTED` |
| `D-17` | Runtime request variants closed | AwaitManyAggregate and Timeout only | `CLOSED` |
| `D-18` | AwaitMany request owns complete child specs | ordered source items and complete child TaskSpec rows avoid an unowned runtime factory or debug inference | `CLOSED` |
| `D-19` | RuntimeTaskScheduler<A> is sole owner | journal, counters, adapter, runtime state, events, persistence and replacement are co-owned | `CLOSED` |
| `D-20` | Driver is consumer | old RuntimeTaskRegistry and generation/counter/journal authority are deleted | `CLOSED` |
| `D-21` | Commit is infallible | prepare reserves every fallible resource; commit/rollback return unit; AdapterCommit removed | `CORRECTED` |
| `D-22` | NeedHandle semantic key is NeedId | manual Eq/Hash/Ord; complete structure validated separately | `CORRECTED` |
| `D-23` | Stale generation is a use error | value equality remains true; Await/timeout reject before mutation | `CLOSED` |
| `D-24` | Replacement is the sole generation rebind | preserve NeedId/ordinal/producer and rederive TaskKey/TaskId | `CLOSED` |
| `D-25` | Purpose-built strict v1 snapshot codec | no generic Serde authority, compatibility reader or translation path | `CLOSED` |
| `D-26` | Prepared adapter tokens are not persistent | snapshot rejects in-flight adapter transactions; MustBeQuiescent host tasks block mid-flight save | `CLOSED` |
| `D-27` | Current HirSnapshotId is Match lookup generation | no AcceptedSemanticGeneration invented | `CORRECTED` |
| `D-28` | Stable coordinates are declaration-rooted role paths | raw IDs/spans/spelling are lookup-only | `CLOSED` |
| `D-29` | Compiler-local and bundle rows split | CheckedMatchRef remains local; persistent row contains projections only | `CORRECTED` |
| `D-30` | Predicate is a TypeKind leaf | no child recursion in classifier | `CORRECTED` |
| `D-31` | Shared is rejected | MissingRuntimeSnapshotOwner; no new Shared carrier in this correction | `CORRECTED` |
| `D-32` | Successful SnapshotClone requires four owners | runtime projection, live carrier, canonical identity and snapshot codec | `CLOSED` |
| `D-33` | Five cuts are exact | public RuntimeValue variant appears only in atomic Cut 5; Cut 3 cannot depend on Cut 4 | `CORRECTED` |
| `D-34` | READY gate | all choices above are closed; production implementation/testing remains future work, not an open design question | `CLOSED` |

## Rejected alternatives

- A second producer-argument serializer or digest: rejected because it would split value identity.
- Treating constant admission as the canonical identity policy: rejected because Plain+SnapshotOnly must identify producer arguments and snapshots.
- Encoding affine handles in a diagnostic RuntimeValueDigest: rejected because no stable value identity exists.
- `TaskSpec { host: Option<_>, runtime: Option<_> }`: rejected because it creates invalid zero/two-owner states.
- Inferring runtime ownership from debug labels, host operation names or request spelling: rejected.
- Keeping a journal in the driver and an adapter in the scheduler: rejected because ordinal/prepare/publication cannot be atomic.
- Fallible adapter commit and `AdapterCommit`: rejected because every fallible reservation belongs in prepare.
- Structural derived equality for `RuntimeNeedHandle`: rejected because the canonical value key is NeedId only.
- Adding generation/spec bytes to RuntimeValue NeedHandle identity: rejected by the retained tag-20 contract.
- Inventing `AcceptedSemanticGeneration`: rejected because current `FinalSemanticAnalysis` already owns exact `HirSnapshotId` mappings.
- Embedding `CheckedMatchRef` or copied compiler rows in a bundle: rejected because session-local IDs cannot persist.
- Generic Serde semantic transcripts or snapshot format: rejected due unstable field/tag/order authority.
- Treating planned Shared work as evidence: rejected; Shared remains `MissingRuntimeSnapshotOwner`.
- Claiming a Rust enum variant is private during a staggered public switch: rejected; all exhaustive consumers update in Cut 5.
