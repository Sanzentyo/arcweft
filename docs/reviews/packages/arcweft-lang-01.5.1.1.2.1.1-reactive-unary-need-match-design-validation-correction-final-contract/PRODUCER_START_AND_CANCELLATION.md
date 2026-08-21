# Producer start, deduplication, cancellation, failure

## Policy

V1 has one start policy: `ObserveStartsNotStarted`. Selecting NotStarted emits a
Sans-I/O intent; it never calls an adapter directly.

The exact AWBC producer function/task-plan owns construction. Product-step verifies
it, and runtime task registry turns a committed intent into HostTaskDispatch.
Host/native/Web adapters execute dispatches. View, bundle, semantic, and product
DTO layers stay Sans I/O.

## Dedup key

```text
(GenerationId,
 verified NeedId,
 verified TaskKey,
 TaskPolicy::JoinSameKey,
 ViewNeedProducerContractDigest)
```

Same verified producer joins. Same spelling with different binding/generation does
not. Running/completed exact task yields join/no-new-dispatch. Rejected frame,
product, restore, or replacement candidate commits no intent.

## Cancellation

V1 is ProducerOwned:

- arm changes do not cancel;
- observer removal and last View observer removal do not cancel;
- remount does not cancel/restart an exact live producer;
- explicit existing task/scope APIs cancel;
- successful replacement may retire/cancel a generation only when no exact producer survives.

Cancelled is terminal temporal state, not Result Err, denied, or infrastructure
failure.

## Failure classification

Synchronous admission failure occurs before subscription and is an outer Result.
Asynchronous domain failure is a Ready Result payload. Host dispatch failure,
malformed publication, verifier failure, or invariant failure is a typed runtime
error and never fabricates Ready/Cancelled/error/denied.
