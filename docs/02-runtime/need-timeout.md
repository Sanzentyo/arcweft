# Deterministic Need timeout

Generic timeout is a standard temporal combinator, not an Await variant or an
Await `with` branch.

```arcw
pub record Timeout {
    limit: Duration
}

pub fn timeout<T>(
    source: Need<T>,
    after: Duration,
) -> Need<Result<T, Timeout>>
```

Await keeps its single rule:

```text
await : Need<T> -> T
```

Therefore:

```text
timeout(source, duration)       : Need<Result<T, Timeout>>
await timeout(source, duration) : Result<T, Timeout>
```

`Timeout` means the caller's wait limit expired. It is not producer failure,
cancellation, or a runtime fault.

## Value layers

Fallible source payloads remain nested deliberately:

```text
source                              : Need<Result<T, E>>
timeout(source, duration)           : Need<Result<Result<T, E>, Timeout>>
await timeout(source, duration)     : Result<Result<T, E>, Timeout>
```

```arcw
match await timeout(load_image(path), 5s) {
    .Ok(.Ok(image)) => show(image)
    .Ok(.Err(load_error)) => show_load_error(load_error)
    .Err(timeout) => show_timeout(timeout.limit)
}
```

Arcweft does not implicitly flatten errors, build an error union, or call
`map_err`. `try await timeout(...)` is ordinary
`Try(Await(Call(std.need.timeout, ...)))` and targets the ordinary nearest
Result boundary.

## No timeout syntax in Await

The language does not add:

```arcw
await source timeout 5s
await? source
tryawait source
```

`with` observes temporal Pending state; `timeout` owns a race; `match` and
`try` consume its Result value. A legacy timeout branch, if recognized for a
migration diagnostic, never remains as an executable compatibility path.

## Logical clock and start point

Generic timeout uses only `RuntimeStepInput.dt`. It never reads an OS clock.
Pause stops countdown; save/load wall time is not charged; replay with the same
`dt` sequence resolves on the same step.

Transport deadlines such as an HTTP backend deadline are producer-specific
capability options. They may stop backend work and return the producer's typed
domain outcome, but they are not the generic Need combinator.

The derived Need starts its timer on its first start demand, not when the source
expression constructs the lazy value. The start transaction is:

1. reject an already-cancelled owning scope;
2. subscribe to the source and snapshot its state;
3. apply the source's checked start policy when needed;
4. resolve immediately if the source is already terminal;
5. otherwise set `remaining = after` and enter Waiting.

Each wrapper has an independent start time and remaining duration, even when
multiple wrappers observe the same source Need.

## Deterministic race order

One runtime step uses this order:

```text
scope cancellation
    > normalized source terminal publication
    > timeout countdown/expiration
    > nonterminal Pending publication
```

Consequences:

- source Ready wins when Ready and expiration occur in the same step;
- source cancellation wins over expiration;
- expiration suppresses a same-step final progress update;
- a late source completion cannot replace an already committed timeout;
- terminal publication is committed once.

For `0s`, Arcweft snapshots the source first. An already Ready source wins;
Pending or NotStarted times out immediately; a cancelled source remains
cancelled.

## Cancellation and Progress

Timeout is wait-local:

```text
derived Need = Ready(Err(Timeout { limit }))
source Need  = unchanged
producer     = not cancelled
```

The wrapper does not gain authority to cancel shared producer work. Cancelling
the wrapper's structured scope removes its timer and subscription and performs
the ordinary non-returning Need cancellation transfer. It does not construct a
Timeout value. A source owned by that same scope may still be cancelled by the
scope's independent ownership authority.

While Waiting, the wrapper forwards the source's latest Pending progress. It
does not synthesize a timeout ratio and source progress does not reset the
total-wait timer. An idle timeout, if later required, is a different
combinator.

## Public and runtime ownership

The public `Timeout` value contains only its requested `limit`. It excludes
Need/Task IDs, OS instants, paths, adapter handles, diagnostic messages, and
resolution ticks. It is deterministic, serializable, and unrestricted.

Runtime Need production has one exhaustive owner:

```text
RuntimeNeedProducer
    HostTask(...)
    Timeout(RuntimeTimeoutNeed)
```

`RuntimeTimeoutNeed` owns output/source Need identities, limit, remaining, and
phase (`NotStarted | Waiting | Resolved`). Timeout does not add
`Need::TimedOut`; it publishes `Ready(Result::Err(Timeout))`.

Sema selects the exact `std.need.timeout` callable identity and publishes one
typed timeout fact containing source expression, duration expression, source
payload, and result type. Lowering never matches the spelling `timeout`.

Structured RuntimePlan represents construction separately from Await:

```text
NeedTimeout { output, source, limit, producer_site }
Await { source = output, ... }
```

The builder validates exact Need payloads, Duration, Timeout nominal identity,
distinct source/output identities, and the issuing builder generation.

AWBC evolves schema version `1` in place with a typed `NeedTimeout` operation.
It does not add `AwaitTimeout`, `TryAwaitTimeout`, or `AwaitWithTimeout`. The
verifier checks byte-identical source/output payload structure and the exact
builtin Timeout identity.

## Save, replay, and reload

A live snapshot stores output/source identities, limit, remaining, and phase.
Waiting resumes from the saved remaining duration. Resolved nodes remain
terminal and do not rerun the race. The first step after load applies the same
cancellation/source/timeout ordering.

Decode rejects contradictory phase/terminal state, unknown or duplicate
identity, and zero identity. Structured and AWBC execution share this snapshot
model.

Hot reload preserves a live timeout only when producer-site identity, source
payload type, output type, and `std.need.timeout` identity still match. A
mismatch performs ordinary reload cancellation; it is not converted to
`Err(Timeout)`.

All Arcweft-owned runtime, AWBC, codec, snapshot, and save markers remain `1`.
No compatibility reader is retained.

## Implementation order

Timeout is implemented only after the final temporal/carrier foundations:

1. Checked Try and Result/Option boundary authority;
2. unary `Need<T>` across every layer;
3. deletion of Await physical-Result, Error/Denied branches, and Await-specific
   Try paths;
4. canonical runtime Need identity and producer ownership;
5. Timeout type, checked intrinsic fact, and NeedTimeout plan operation;
6. logical-time reducer and structured tests;
7. AWBC verifier/codec/VM parity;
8. save/replay/hot-reload tests and obsolete timeout-branch deletion.

## Required behavioral matrix

Tests cover already-Ready and zero-duration sources, same-step Ready/cancel/
progress races, late completion, two wrappers over one source, wrapper-only
cancellation, nested source Result errors, infrastructure faults, save/load on
both sides of expiry, malformed AWBC and snapshots, and structured/AWBC parity.

## See also

- [Await, unary Need, carrier blocks, and `try`](../01-language/await-need-result.md)
- [Nonblocking scheduler and Need](async-scheduler.md)
- [Save / replay / hot reload](save-replay.md)
