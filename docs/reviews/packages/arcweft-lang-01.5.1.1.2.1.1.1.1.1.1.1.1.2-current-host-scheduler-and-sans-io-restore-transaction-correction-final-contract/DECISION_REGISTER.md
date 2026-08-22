# Decision register

Every row is closed. Reopening a row requires a concrete current-repository or
released/persisted-consumer flaw, not an implementation preference.

| ID | Selected decision | Rejected alternative |
|---|---|---|
| HSRT-001 | type definition in `arcweft-runtime-scheduler`; concrete value owned by native/Web/headless host composition | driver/global coordinator ownership |
| HSRT-002 | `RuntimeTaskScheduler<A: TaskLaunchAdapter>` stores concrete `A` | method-generic adapter, trait object, erased token |
| HSRT-003 | `TaskLaunchAdapter` and crossing batches remain core-owned so scheduler stays core-only | scheduler dependency on host-adapter or host implementation |
| HSRT-004 | core `TaskHost` evolves in place; driver borrows `&mut impl TaskHost` per step | stored adapter-generic driver/session or returned dispatch side channel |
| HSRT-005 | TaskHost has a host error type but no adapter/token associated type | token leakage or string identity bridge |
| HSRT-006 | outer owner reads complete bytes; core decode is pure | async persistence trait passed into scheduler |
| HSRT-007 | decoded snapshot is untrusted/private-row/no-live-value | direct snapshot-to-runtime values/handles |
| HSRT-008 | prepared guard borrows scheduler exclusively and owns concrete prepared token wrappers | reusable prepared DTO, internal global pending slot |
| HSRT-009 | guard Drop reverses all adapter reservations exactly once | leaked reservation or best-effort cleanup |
| HSRT-010 | core `JournalTransaction` is the sole journal/observer/Need staging owner | scheduler journal delta or persistence journal |
| HSRT-011 | one sealed core after-image plus one scheduler after-image per operation | task-by-task apply or copied state table |
| HSRT-012 | adapter prepare/receipt validation finishes before seal/apply | worker launch or receipt validation after apply |
| HSRT-013 | `apply_after_image` is the last `Result` | durable COMMITTED record, later queue failure, adapter commit error |
| HSRT-014 | apply failure reverse-rolls tokens and leaves core/scheduler unchanged | partial mutation or compensating rollback after mutation |
| HSRT-015 | successful core apply → scheduler swap → adapter commit → exposure | adapter commit before journal, receipt exposure before adapter commit |
| HSRT-016 | sealed state retains only construction inputs/storage; core issues handles/values/receipts during successful apply | prepared live objects or scheduler construction after apply |
| HSRT-017 | ensure/restore/rebind/cancel have distinct typed wrappers and one common transcript | generic erased transaction enum at public boundary |
| HSRT-018 | lifecycle transitions are distinct from observer outcomes | `Accepted`/`Running` reused as task event outcomes |
| HSRT-019 | observer events are exactly Progress/Ready/InfrastructureFailure/Cancelled | `Failed(String)`, Accepted, Running, CancellationRequested event variants |
| HSRT-020 | Need cell stores Pending/Progress/Ready/InfrastructureFailure/CancellationRequested/Cancelled | event queue as sole current-state authority |
| HSRT-021 | event order is `(logical_epoch, task_id, sequence)`, generation-prefixed only in retained-generation collections | Browser's current sequence-before-task ordering |
| HSRT-022 | snapshot rejects prepared and active MustBeQuiescent rows; complete Restartable rows restore through adapter prepare | blanket Host rejection or unchecked restart |
| HSRT-023 | outer session after-image is prepared first and swapped infallibly after task apply under exclusive borrows | I/O or driver callback inside scheduler apply |
| HSRT-024 | no durable PREPARED/COMMITTED/APPLIED_ACK state or crash publication replay | filesystem journal as commit authority |
| HSRT-025 | all versions remain 1; ordinary Wire integers use canonical varint | fixed-width private Wire version fields, V2, old reader/migration |
| HSRT-026 | task-plan/View/Match/nominal products enter only through accepted core authorities | scheduler-local stand-in types/digests/catalogs |
| HSRT-027 | driver registry, dispatch DTO, generation-pin side map, immediate submit/cancel, and duplicate queues are deleted in atomic Cut 5 | compatibility overloads or dual paths |
| HSRT-028 | design is ready; implementation waits for the listed typed predecessors | guessing predecessor result shapes or marking an open design question |
