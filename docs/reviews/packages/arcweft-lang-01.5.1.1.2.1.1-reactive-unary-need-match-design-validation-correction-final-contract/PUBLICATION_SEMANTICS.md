# Deterministic publication semantics

## Selection table

| Incoming condition | Disposition | Mutation | Invalidation |
|---|---|---|---|
| no journal row | synthetic NotStarted | observer only | initial frame |
| current generation, first cursor | accept | install state/digest/cursor | one per changed observer |
| lower cursor | stale | none | none |
| equal cursor, equal digest | duplicate | none | none |
| equal cursor, different digest | hard conflict | reject whole batch | none |
| greater cursor, nonterminal current | accept | replace state | one per changed observer |
| greater cursor after Ready/Cancelled | stale-after-terminal | none | none |
| retired generation | stale-generation | none | none |
| unknown future generation | hard error | reject whole batch | none |
| invalid identity/type/ownership/depth | hard error | reject whole batch | none |

Cursor order is `(LogicalEpoch, TaskSequence)`. Equality across different Need
journal keys has no meaning.

## Batch algorithm

1. Check batch count before allocation.
2. Validate one declared generation and sort by
   `(NeedId, epoch, sequence, canonical state digest)` in scratch.
3. Group by Need. Resolve retired generation before payload projection.
4. For current generation, compute bounded state digest and apply the table.
5. Stop selection at first Ready/Cancelled; later rows are stale-after-terminal.
6. Compare old/new effective state for live observers.
7. Allocate at most one invalidation per changed observer.
8. Check fanout/queue limits.
9. Swap journal and append invalidations atomically.

A hard error in any group rejects every group in the call. Stale/duplicate rows
remain typed outcome counters.

## First frame

Binding evaluates the producer AWBC function once to a verified NeedHandle/NeedId.
An existing journal row is selected immediately. Otherwise NotStarted is selected
and the frame candidate contains one start intent. The intent reaches task-registry
dedup only after the complete frame commits. A failed frame commits no dispatch.

## Coalescing

Across separate committed batches each greater Pending state is observable.
Within one batch, only the highest Pending before terminal becomes effective.
Pending-to-Ready and Pending-to-Cancelled produce one invalidation and expose the
terminal state in the next frame; no intermediate progress frame is synthesized.

## Observers/mounts/remount

Fanout is canonical observer-key order. Two mounts have distinct monotonic
ViewMountIds and independent local/arm state. Two subscriptions in one mount are
independent observers. All may share one journal/producer. Unmount mutates only
the observer table. Remount never reuses the retired mount key and immediately
sees current journal state.

## Telemetry

Outcome counts: accepted, duplicate, stale-cursor, stale-generation,
stale-after-terminal, invalidated observers. They are observations, not alternate
semantic results. Payloads are never logged/stringified as fallback.
