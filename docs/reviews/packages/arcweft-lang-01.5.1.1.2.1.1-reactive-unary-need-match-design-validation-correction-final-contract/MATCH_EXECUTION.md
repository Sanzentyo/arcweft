# Generic Match execution and arm-local bindings

## Checked source

Source uses ordinary `match` over exact `Need<T>`. Sema resolves each case and
nested pattern against T, preserves source arm order, computes ordinary coverage,
checks guards/effects, and assigns existing LocalIds. Need's closed coverage
domain is exactly four cases. Wildcard covers remainder; guarded arms do not
supply unconditional coverage.

Non-exhaustive source, pattern/type/binding/ownership errors fail before product.
There is no Need-specific parser/HIR arm kind.

## Product/runtime

One generic Match selector AWBC function receives
`NeedState { subscription }`. It executes existing pattern tables and guards once
in source order and returns:

- exact source arm ordinal;
- one typed output register for each binding owned by that arm.

The generic Match adapter validates ordinal, output count/type/ownership, and body
span before installing values in the mount's ordinary local table. View runtime
does not re-run pattern/guard logic.

`RuntimeNeedState::project_match_value` creates an ordinary variant:

```text
NotStarted       -> state.NotStarted()
Pending(progress) -> state.Pending(RuntimeValue::Progress(progress))
Ready(payload:T) -> state.Ready(payload.into_runtime_value())
Cancelled        -> state.Cancelled()
```

The projection is ephemeral AWBC input, not a Need handle, ViewRuntimeValue,
presentation payload model, or debug string.

## Nested carriers

```arcw
match load_avatar(user) {
    .not_started => Placeholder()
    .pending(progress) => Progress(progress.ratio)
    .ready(.Ok(image)) => Image(image)
    .ready(.Err(error)) => ErrorMessage(error)
    .cancelled => CancelledBadge()
}

match maybe_lookup(key) {
    .not_started => Empty()
    .pending(progress) => Progress(progress.ratio)
    .ready(.Some(value)) => Value(value)
    .ready(.None) => Missing()
    .cancelled => CancelledBadge()
}
```

Need has no error/denied cases. An outer `Result<Need<T>, AdmissionError>` must
be handled before the Need can be subscribed.

## No-match

Source-valid catalogs are exhaustive. Tampered product/AWBC no-match produces
ordinary `RuntimeMatchError::NoArmMatched`, aborts frame/mount/local/start-intent
publication, and never selects empty, first, last, pending, error, or denied fallback.

## Retained arm state

Materialized state is keyed by `(observer, source arm ordinal, arm contract digest)`.
Changing arms deactivates but retains compatible arm state for the mount lifetime.
Save/replay preserves it. Replacement keeps exact contract matches, drops only
incompatible arm state, and queues one observer invalidation. Unmount drops all
arm state for that occurrence.
