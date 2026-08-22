# Dependencies and implementation gates

## 1. Final crate direction

| Crate | Final task responsibility | Permitted task dependencies | Forbidden edge |
|---|---|---|---|
| `arcweft-core` | identities, validation/snapshot protocols, journal, TaskHost, adapter protocol batches/tokens, events/Need/observer/value DTOs | existing lower data/id/Need crates | scheduler, host-adapter, runtime-driver, runtime-host, compiler, bundle, View |
| `arcweft-runtime-scheduler` | `RuntimeTaskScheduler<A>`, runtime after-image, deterministic transaction coordination | `arcweft-core` only | host-adapter, runtime-host, runtime-driver, save, filesystem/network runtime |
| `arcweft-host-adapter` | concrete registry-backed `TaskLaunchAdapter` facade and Host implementations | core and existing adapter context | scheduler, runtime-driver |
| `arcweft-runtime-host` | native/headless concrete scheduler composition | core, scheduler, host-adapter | runtime-driver-owned scheduler state |
| `arcweft-player-web` | Browser concrete scheduler composition and event source | core, scheduler, runtime-driver for session use | driver task DTO as adapter authority |
| `arcweft-runtime-driver` | method-generic borrowed `TaskHost` step; no task owner | core | scheduler, host-adapter, runtime-host |
| CLI/native/Web player application | outer quiescence, complete save I/O, session + concrete scheduler restore orchestration | driver, concrete host, save | persistence callbacks inside scheduler/core apply |

`TaskLaunchAdapter` is core-owned so the scheduler retains its current
core-only dependency. Concrete adapters implement the protocol above the
scheduler. `TaskHost` is also core-owned so driver can borrow a host without an
upward edge.

## 2. Current-to-final composition

| Current source | Current fact | Final action |
|---|---|---|
| `crates/arcweft-runtime-scheduler/src/lib.rs` | Sans-I/O `RuntimeScheduler`, depends only on core | replace with/fold into `RuntimeTaskScheduler<A>`; retain deterministic ordering algorithms |
| `crates/arcweft-runtime-host/src/native_task.rs` | `NativeTaskBridge { registry, scheduler }` | replace fields with one concrete generic scheduler plus only nonauthoritative completion source/stats |
| `crates/arcweft-player-web/src/host.rs` | `BrowserTaskBroker` owns allowed/files/cancel/event side state and imports `HostTaskDispatch` from driver | implement concrete adapter + scheduler composition; delete driver task DTO import and duplicate cancellation/event authority |
| `crates/arcweft-runtime-driver/src/task.rs` | `RuntimeTaskRegistry` owns duplicate lifecycle/event state | delete after `TaskHost` migration |
| `crates/arcweft-runtime-driver/src/session.rs` | returns `requested_tasks`/`cancel_scopes`, pins task generations in a side map | borrow `TaskHost` during step; scheduler/journal own tasks and generation joins |
| `crates/arcweft-runtime-driver/src/session/persistence.rs` | resets driver registry on restore; session save owns current I/O-facing DTO | delete registry reset; accept outer task snapshot orchestration without I/O in scheduler |
| `crates/arcweft-host-adapter/src/lib.rs` | immediate `submit`, completion drain, boolean cancel | replace immediate launch/cancel timing with accepted prepare/commit/rollback facade |

## 3. Typed predecessor graph

This design closes scheduler/transaction choices but does not fabricate upper
semantic products. Production Cut 5 follows:

```text
.1.2 generic Match transcript/path accepted and implemented
  -> .1.4 retained View operation/product accepted and implemented
  -> .1.3.1 task-plan semantic owner finalized and implemented

.1.1.1.1 structural accepted nominal carrier accepted and implemented
  -> complete RuntimeSnapshotAuthority value domain

accepted parent Cut 4 identity/catalog substrate
  + the above products
  -> TaskValidationAuthority + RuntimeSnapshotAuthority land
  -> this RuntimeTaskScheduler<A> / TaskHost / restore transaction implementation
  -> one atomic public Cut 5 switch
```

The `.1.2`, `.1.4`, `.1.3.1`, and `.1.1.1.1` package returns currently are not
assumed. If any remains typed fail-closed, the scheduler preserves that failure
through the existing authority. It does not add a temporary success branch.

## 4. Stable consumption interfaces

The scheduler names only accepted core protocols:

```rust
&TaskValidationAuthority<'_>
&RuntimeSnapshotAuthority<'_>
&dyn ViewTaskPlanAuthority // retained inside the authorities, not a scheduler field
```

Consequently it never needs to name:

- a `.1.2` declaration/body path or checked Match row;
- a compiler-local Match/View key;
- `ViewMatchSiteId` or a provisional View admission digest;
- a `.1.3.1` builder coordinate or task-plan seed;
- an accepted Rust metadata/catalog row; or
- any source/HIR/compiler identity.

Those upper owners construct the authorities before scheduler entry. A missing
or stale product returns its accepted typed error before adapter preparation.

## 5. Outer persistence direction

`arcweft-save` and application filesystem/browser storage remain outside the
task transaction:

```text
filesystem / IndexedDB / caller bytes
  -> application save owner
  -> pure core decode DTO
  -> concrete host scheduler prepare/apply
```

There is no dependency or callback in the reverse direction. The scheduler
accepts owned decoded values, never a reader, path, database handle, async
trait, journal, or persistence service.

## 6. Implementation readiness versus order

`READY_FOR_IMPLEMENTATION` records that this correction has no open semantic
choice. It does not authorize publishing its public types before the typed
predecessors exist. Implementation may prepare private mechanical refactors
only when they do not create a second task model or a public provisional API.

The final public scheduler, `TaskHost`, snapshot typestates, driver migration,
and deletion set publish atomically after all listed predecessors. A compile
failure caused by replacing the old contract is repaired in that same cut; a
compatibility overload is not permitted.
