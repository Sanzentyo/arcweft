# Request coverage and closure matrix

## 1. Authority

- Request: `request.md`
- Package subject: **Lang-01.5.1.1.2.1.1.1.1.1.1.1.1 — runtime task coordinator and two-phase restore correction**
- Repository basis: `UNAVAILABLE (authenticated clone was not available in the execution container)` (request-only design; current private source was not locally materialized)
- Design status: **CLOSED**
- `OPEN_QUESTIONS=0`
- This package is design-only. It does not contain or apply a production patch.

The table below maps each machine-detected numbered requirement from the request to an exact design decision, owner, normative section, and test row. The original request is also included verbatim as `99-input-request.md`, so no omitted prose is hidden by extraction.

| Request row | Requirement identity | Exact decision IDs | Owning API/type | Normative location | Acceptance test rows | Status |
|---|---|---|---|---|---|---|
| R-1 | The prose uses `RuntimeTaskScheduler<A: TaskLaunchAdapter>`, while the schema only sketches method-generic `impl RuntimeScheduler` functions and supplies no constructible public ensure/snapshot/restore owner. The adapter trait has associated token types, so an unspecified trait-object bridge is not viable. | D-01/D-02 | `RuntimeTaskCoordinator` / coordinator-owned publication root | 03 §2–§4; 04 §2 | RTC-OWN-001, RTC-PUB-001 | CLOSED |
| R-2 | `RuntimeSnapshotAuthority` is sketched as borrowing a committed journal and directly producing runtime values. The accepted transaction rule instead forbids receipts and accepted-launch Need handles from an uncommitted after-image and requires them to be constructed only by successful apply. No two-phase decoded/prepared/applied restore types are specified. | D-01/D-02 | `RuntimeTaskCoordinator` / coordinator-owned publication root | 03 §2–§4; 04 §2 | RTC-OWN-001, RTC-PUB-001 | CLOSED |
| R-3 | Snapshot event rows use lifecycle names such as `Accepted` and `Running`, while the final live accepted outcome/state algebra uses `Progress`, `Ready`, `InfrastructureFailure`, and `Cancelled`. The mapping and sole persistent authority are not defined. | D-01/D-02 | `RuntimeTaskCoordinator` / coordinator-owned publication root | 03 §2–§4; 04 §2 | RTC-OWN-001, RTC-PUB-001 | CLOSED |
| R-4 | Restore, replacement/rebind, cancellation, observer publication, adapter prepare/commit/rollback, journal mutation, and receipt construction do not share one stated commit point or failure precedence. | D-03–D-09 | `PreparedTaskRestore` consumed by `commit_restore` | 03 §5; 05 §2–§6 | RTR-2PC-001..009 | CLOSED |
| R-1 | Define the final public `RuntimeTaskScheduler<A>` owned by host composition, including exact constructor, ensure, poll/step, cancel, observe, snapshot, prepare-restore, apply-restore, and replacement/rebind signatures. | D-03–D-09 | `PreparedTaskRestore` consumed by `commit_restore` | 03 §5; 05 §2–§6 | RTR-2PC-001..009 | CLOSED |
| R-2 | Define the narrow borrowed `TaskHost` boundary consumed by runtime-driver. State how native, Web, and headless compositions implement it while keeping adapter tokens concrete and host-owned. | D-36 | normative closed decision in coordinator/restore modules | 03–09 | RTR-REQ-xxx | CLOSED |
| R-3 | Define a two-phase restore algebra with untrusted decoded rows, a completely validated prepared after-image, and a sealed apply result. Decoding and preparation must not expose `RuntimeNeedHandle`, `TaskLaunchReceipt`, observer mutation, journal mutation, or adapter-private tokens. | D-03–D-09 | `PreparedTaskRestore` consumed by `commit_restore` | 03 §5; 05 §2–§6 | RTR-2PC-001..009 | CLOSED |
| R-4 | Define the sole commit point and exact ordering for adapter batch commit, scheduler/journal publication, observer state, restored Need handles, receipts, and returned runtime values. A failure before the commit point must leave every owner byte-for-byte unchanged; a post-commit fallible step is forbidden. | D-01/D-02 | `RuntimeTaskCoordinator` / coordinator-owned publication root | 03 §2–§4; 04 §2 | RTC-OWN-001, RTC-PUB-001 | CLOSED |
| R-5 | Reconcile live task state, journal events, snapshot rows, and restored state into one exhaustive typed projection. Distinguish an event (`launch accepted`, `execution started`) from an accepted terminal/current outcome; do not reuse a similarly named enum as both. | D-03–D-09 | `PreparedTaskRestore` consumed by `commit_restore` | 03 §5; 05 §2–§6 | RTR-2PC-001..009 | CLOSED |
| R-6 | Define validation and error precedence for version, canonical wire, duplicate identity, producer/policy/ordinal rederivation, TaskKey/TaskId/NeedId joins, plan and argument digests, View bundle/revision mapping, host catalog routes, adapter prepare, quiescence/restart policy, and apply. | D-03–D-09 | `PreparedTaskRestore` consumed by `commit_restore` | 03 §5; 05 §2–§6 | RTR-2PC-001..009 | CLOSED |
| R-7 | Define replacement/rebind and cancellation through the same batch/after-image authority. NeedId and launch ordinal remain stable only where the accepted replacement rules permit; generation, TaskKey, and TaskId must be rederived. | D-01/D-02 | `RuntimeTaskCoordinator` / coordinator-owned publication root | 03 §2–§4; 04 §2 | RTC-OWN-001, RTC-PUB-001 | CLOSED |
| R-8 | Provide a deletion-driven compile-clean sequence that removes the driver registry, old String identities, old scheduler/receipt/observer/snapshot rows, direct runtime-value restore, and every superseded reader/writer in the same atomic public Cut 5 switch. Every Arcweft-owned version marker remains exactly `1`. Ordinary private Wire integers use the maintained canonical shortest base-128 varint. Hash-transcript little-endian widths are not a second wire format. Removed opcode/flag tombstones remain rejected; numeric allocation is outside this request. | D-03–D-09 | `PreparedTaskRestore` consumed by `commit_restore` | 03 §5; 05 §2–§6 | RTR-2PC-001..009 | CLOSED |

## 2. Cross-cutting closure decisions

| Decision | Closed choice | Rejected alternative |
|---|---|---|
| D-01 | `RuntimeTaskCoordinator` is the sole runtime owner for installed task identity, restore serialization, pending publication, published root, and runnable admission. | Persistence store, individual handles, or task-local cells acting as a second coordinator. |
| D-02 | Handles are non-authoritative views/capabilities. Their liveness is derived from the coordinator-published generation. | Restoring each handle independently and then attempting to reconcile a task map. |
| D-03 | Restore has exactly two public semantic phases: **prepare** and **commit**. Internal journal/install/publish substeps do not create a third public phase. | A monolithic `restore()` with partial mutations, or a public prepare/install/publish three-phase API. |
| D-04 | Prepare is observer-silent and mutation-free with respect to the live coordinator. It may allocate detached objects only. | Registering tasks or waking consumers while decode is still in progress. |
| D-05 | Commit consumes a non-`Clone` `PreparedTaskRestore` and is serialized by the target coordinator. | Reusable prepared values or independent per-task commits. |
| D-06 | The entire task table, handle batch, match substrate, and runnable seed set are published through one atomic root replacement. | Incremental task-by-task insertion. |
| D-07 | The durable `COMMITTED` decision precedes external visibility; startup replays committed-but-not-published work. | Visibility before durable decision or treating a lost success reply as an unknown outcome. |
| D-08 | Same `RestoreId` + same batch digest is idempotent and returns the same receipt. Same ID + different digest is corruption. | “Last writer wins” restore token reuse. |
| D-09 | Once the durable commit decision exists, cancellation cannot convert the operation to abort; completion becomes cleanup/replay work. | Returning cancellation while leaving the caller unable to know whether tasks are authoritative. |
| D-10 | Restore journal records use the repository's canonical encoder/seal infrastructure; no ad-hoc serde/bincode/native-endian format. | A new opaque persistence codec with unstable bytes. |
| D-11 | Persisted records contain semantic identity and digests, never process pointers, `Arc` addresses, wakers, mutex state, or executor-local tokens. | Serializing live runtime objects. |
| D-12 | Snapshot and handle batch retain canonical one-to-one slot ordering and generation identity. | Reallocating handles in hash iteration order. |
| D-13 | Prepared restore validates all plan seals, semantic child encodings, task references, generic-match transcript, and coverage closure before commit. | Deferring validation until a restored task first runs. |
| D-14 | Unknown schema versions/features fail closed before mutation; older supported versions normalize to the current prepared form. | Best-effort parsing of unknown fields. |
| D-15 | The coordinator epoch and the persisted base epoch are checked at prepare and rechecked under the commit gate. | Trusting a stale prepare result indefinitely. |
| D-16 | Task identity collision is permitted only for the exact idempotent replay of the same restore token/digest. | Silently replacing a live task with the same ID. |
| D-17 | No I/O, user callback, task poll, or await occurs while the published-root write lock is held. | Holding broad runtime locks across fsync or decode. |
| D-18 | Lock order is restore serial gate → journal append lock → short coordinator install/publish lock → task-local lock; reverse acquisition is forbidden. | Opportunistic nested locking. |
| D-19 | Existing arcweft-owned enums gain required variants/behavior in their original definition/`impl`; no local extension trait or stringly helper. | Ad-hoc compatibility wrappers around project-owned enums. |
| D-20 | Startup recovery treats the journal as decision authority and reconstructs runtime memory; memory is never used as proof of durable completion. | Inferring durable state from a prior process's in-memory state. |
| D-21 | `OPEN_QUESTIONS=0`; implementation discovery items are expressed as source-verification gates, not unresolved design choices. | Deferring core ownership/ordering/error decisions to implementation. |
