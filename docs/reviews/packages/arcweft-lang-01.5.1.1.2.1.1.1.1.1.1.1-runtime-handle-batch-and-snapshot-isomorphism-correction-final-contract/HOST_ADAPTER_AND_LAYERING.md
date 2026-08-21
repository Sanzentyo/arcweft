# Host adapter protocol and Sans-I/O dependency proof

## Dependency direction

```text
arcweft-core
  ↑
arcweft-runtime-scheduler
  ↑
arcweft-host-adapter / arcweft-runtime-host / desktop / web / headless
```

`arcweft-runtime-scheduler/Cargo.toml` currently lists only `arcweft-core`.
That edge is retained. The scheduler must not import, feature-gate, dynamically
load, or test-depend on `arcweft-host-adapter`.

Core owns only typed protocol data and the generic trait. It performs no I/O.
The scheduler owns deterministic planning and state transactions. Adapter/host
crates own route implementations and worker queues.

## Prepare/commit/rollback

All four protocol families—launch, restore, rebind and cancel—obey one timing
contract:

- **prepare:** validate route/capability, check capacity, allocate adapter-local
  memory and reserve an unpublished queue slot;
- **commit:** make that already-reserved slot visible; no allocation, capacity
  check, worker start, I/O or error return;
- **rollback:** release the unpublished reservation; no error return.

A worker observes nothing before commit. Prepare cannot call the current
immediate `submit` implementation internally.

## Typed operation identity

Dedicated built-ins use `BuiltinHostOperationIdV1`. Extensible operations use:

```rust
Catalog {
    catalog_digest: HostOperationCatalogDigest,
    operation: HostOperationId,
}
```

The catalog constructor accepts canonical rows ordered by nonzero numeric ID and
validates capability, request contract, restart policy, cancellation contract
and typed route. The digest covers that exact order and every field. A custom
request's former source string may remain diagnostic metadata only; it does not
select an operation or route.

## Current migration

| Current route | Required replacement |
|---|---|
| `HostAdapter::submit(&TaskSpec)` | `prepare_launch` reservation + scheduler atomic apply + `commit_launch` |
| `HostAdapter::cancel(&TaskId) -> bool` | complete typed cancel batch + prepared token + transaction |
| registry immediate forwarding | route-grouped prepared tokens |
| direct worker spawn/enqueue in submit | unpublished reservation; visibility only in commit |
| boolean cancel success | scheduler-owned typed disposition and eventual infrastructure event |
| string custom operation dispatch | typed operation catalog identity |

A compatibility wrapper that invokes `submit` during prepare violates the
transaction boundary and is not evidence of completion.

## Restart/restore

A Restartable active Host snapshot stores the complete TaskSpec, correlation,
operation identity and launch capability. Restore validates the same current
catalog, calls `prepare_restore`, installs the journal/Need/runtime/observer
state atomically, then calls infallible `commit_restore`.

Rebind follows the same pattern across old/new generation correlations. A
prepared restore or rebind blocks snapshot just like a prepared launch/cancel.
