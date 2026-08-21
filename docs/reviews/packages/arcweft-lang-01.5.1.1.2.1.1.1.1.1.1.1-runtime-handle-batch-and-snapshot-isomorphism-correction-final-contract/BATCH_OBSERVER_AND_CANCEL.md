# Batch, observer, and cancellation transactions

## Ensure batch state machine

The aggregate scheduler path is one transaction:

| Phase | Mutation allowed | Failure action |
|---|---|---|
| derive child specs in source-index order | none | return |
| inspect existing Join rows | none | return |
| allocate candidate AlwaysStart ordinals | local plan only | discard |
| allocate candidate observer IDs | local plan only | discard |
| stage journal/runtime/observer/scope/counter after-images | local plan only | discard |
| prepare Host route groups | unpublished adapter reservation only | rollback all prior tokens in reverse order |
| validate after-image cross-references and work limits | none outside plan | rollback all tokens |
| atomic scheduler apply | one infallible after-image replacement | not fallible after preflight |
| adapter commit | expose reserved queue commands; infallible | no rollback path |
| aggregate status publication | one infallible aggregate-row replacement | no partial child visibility |

`EnsureBatchPlan.results` is source-index ordered. Existing Join rows appear in
the result with their existing handle correlation and a newly staged observer.
They do not create task/journal/runtime deltas.

A route grouping does not change semantic order. Each prepared token records its
lowest source index; global preparation and commit order is stable by source
index then typed route ID. Rollback iterates the actual preparation vector in
reverse.

## Atomic apply technique

Each delta holds complete validated after-images for the maps/counters it owns.
Preflight checks:

- all task/Need/observer/scope references resolve in the union of current rows
  and staged rows;
- no existing row is silently replaced except the named aggregate and
  candidate counters;
- staged counters equal the first unused values after the planned rows;
- adapter reservations cover every new Host row exactly once;
- results cover each requested source index exactly once; and
- configured row/byte/work limits hold.

After preflight, map replacement/insertion and counter replacement are
infallible operations. Implementation may use entry APIs or complete map
after-images, but it may not insert one child and then call a fallible operation
for the next.

## Observer allocator

`next_observer_id` is the next candidate, not the last allocated ID. Candidate
planning is pure arithmetic on a local copy. `u64::MAX` is a terminal
unallocatable candidate so the journal can always persist a strictly greater
next value than every issued ID.

Every observer reference uses a typed `TaskObserverId`; retained generations
use `TaskObserverKey`. Removing/detaching an observer changes rows but never
the counter.

## Cancellation state machine

1. resolve and deduplicate requested complete correlations;
2. classify NotFound, AlreadyTerminal and AlreadyRequested without adapter work;
3. validate each active Host row's operation, launch capability and cancellation
   capability;
4. derive one `HostCancelCommandId` per correlation;
5. stage launch, Need, observer, runtime task, scope and pending-event after-images;
6. call adapter `prepare_cancel` for route-stable groups;
7. on refusal, roll back prior prepared tokens in reverse order and discard all
   deltas;
8. validate the complete after-image;
9. atomically install it;
10. call infallible `commit_cancel` for every token; and
11. return per-request dispositions.

The Need transitions to cancellation; no `Result::Err`, `Option::None`,
infrastructure payload, or adapter error is delivered as a domain value.
A worker-side post-commit inability to execute the command is reported later as
`TaskEvent::InfrastructureFailure`.

## Idempotence

- Duplicate correlations inside one request are a caller error and reject the
  entire transaction before prepare.
- A later repeat after committed cancellation returns `AlreadyRequested`.
- The adapter queue and worker deduplicate the typed command ID.
- Rust ownership makes a prepared token single-consumption; commit and rollback
  both consume it.
