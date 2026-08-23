# 06. Runtime task, Need, handle-batch, and AWBC integration

## Data flow

```text
checked type + accepted match domain
        │
        ├─ emit nominal/structural stable keys
        ├─ emit optional structural projection witness
        ▼
sealed AcceptedRuntimeCarrier ──► immutable task-plan input
        │                               │
        │                               ├─ runtime match executor
        │                               ├─ transcript/coverage digest check
        │                               └─ snapshot encoder
        ▼
staged restore carrier/value batch ──► coordinator atomic publish ──► Need/task wakeup
```

## Task-plan rules

- A task plan references a carrier-table entry by the plan's existing canonical child/reference mechanism.
- Sealing the plan verifies that every carrier constraint and projection witness is reachable and digest-consistent.
- Semantic child encoding includes stable carrier/witness keys in deterministic order.
- Carrier metadata is not lazily synthesized by the worker that first executes a match.

## Runtime handle/batch rules

- Live handles remain process-local and are allocated only after stable references resolve.
- Batch order does not affect semantic IDs or snapshot bytes.
- A handle batch contains separate staged tables for payload values, carrier metadata, match plans/witnesses, and tasks, with an explicit dependency order.
- The publish barrier installs roots only when all tables pass validation.

## Need rules

- `Need<T>` identity denotes temporal production of `T`; it is not a type identity for `T`.
- A `Need` that yields a nominal value carries that value's sealed nominal carrier when the value becomes available.
- Cancellation/failure of the producer cannot mutate shared carrier metadata.
- Restoring a waiting Need resolves its value/carrier references before registering waiters or wakeups.

## AWBC/allocation rules

- Use the existing AWBC/value arena for payload ownership and current canonical metadata interner for small immutable carrier facts.
- The match hot path performs no allocation.
- Snapshot decode applies explicit count/byte limits before allocating boxed slices.
- Generic arguments and projection steps are allocated once as boxed slices/interned entries, not once per arm attempt.
- Do not box the whole carrier enum solely to suppress enum-size lint; box only genuinely variable/large collections according to measured layout and existing repository policy.

## Concurrency invariants

1. Published carriers are immutable.
2. A payload handle cannot be observed without its validated carrier in the same published generation.
3. Restored plan/witness and carrier catalogs are generation-consistent.
4. Task wakeup happens after the publication release barrier; readers acquire before dereference.
5. Transcript ordering follows the task/match execution authority already established by the coordinator, not hash-map iteration.
