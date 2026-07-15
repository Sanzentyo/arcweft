# Proof/concurrency v6: nonterminal scheduler progress

- Date: 2026-07-15
- Basis: `ec20509c910f45e8299f70c2f87cae5b568fb375`
- Package: `arcweft-proof-concurrency-v6-final.zip`
- Status: focused production cut

## Outcome

`TaskEventKind::Progress` is nonterminal in the runtime scheduler. Receiving
progress no longer removes the owner task from the in-flight and same-key join
inventories, increments terminal completion counters, or consumes joined
waiters. Existing joined waiters receive the progress event, later waiters can
still join the same work, and the eventual terminal event is delivered to all
of them exactly once through the current scheduler contract.

This aligns the scheduler with the existing core await and runtime-driver task
state, both of which already treat progress as nonterminal.

## Scope boundary

This cut deliberately does not introduce the package's future `WorkRecord` and
`SubscriptionRecord` model. It also does not choose the unresolved canonical
event order or scheduler/driver lifecycle authority, change cancellation into
a request-and-ack protocol, add a persistent inbox, redesign `AwaitMany`,
replace structured fibers or line tasks, extend checkpoints, or change AWBC
ownership semantics. Those items require their final shared typed boundaries
before production implementation.

## Behavioral evidence

The focused scheduler test covers all of the following in one lifecycle:

- progress is delivered to an existing joined waiter;
- the owner remains in flight and terminal counters remain unchanged;
- another waiter can join after progress; and
- the later terminal result completes the owner and both waiters.

No source-spelling gate is used.

## Validation

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo test -p arcweft-runtime-scheduler --all-features` | Pass: 8 tests |
| `cargo clippy -p arcweft-runtime-scheduler --all-targets --all-features -- -D warnings` | Pass |
| `cargo test -p arcweft-runtime-host native_task --all-features` | Pass: 5 focused tests |
| `cargo +nightly -Zscript tools/structure-audit.rs --root .` | Pass: 0 errors, 127 warnings |

The first focused scheduler build exposed a test-only payload mismatch because
`Progress` carries `RuntimePayload`, not `String`; the test was corrected to use
the owned typed boundary before the passing run. The first runtime-host command
hit its 120-second build timeout without a test result; the same command was
rerun with a longer timeout and passed.
