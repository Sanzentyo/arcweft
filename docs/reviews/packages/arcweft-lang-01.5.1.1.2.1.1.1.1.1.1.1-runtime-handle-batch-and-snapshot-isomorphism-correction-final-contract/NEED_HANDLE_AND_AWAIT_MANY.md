# Need handle and AwaitMany construction

## Reusable constructor algorithm

`RuntimeNeedHandle::try_reusable_join` executes in this order:

1. validate `producer` and `outcome` themselves;
2. require `spec.producer == producer` and `spec.outcome == outcome`;
3. require `spec.policy == JoinSameKey`;
4. call the existing inherent `TaskExecution::validate_for` using the exact
   producer family and policy;
5. validate Host operation/request/catalog or Runtime request invariants;
6. recompute the producer instance key from the retained version-1 transcript;
7. derive the complete correlation at the active generation and ordinal zero;
8. validate every redundant correlation component (`NeedId`, `TaskKey`,
   `TaskId`) through `TaskCorrelation::validate`; and
9. construct `ReusableJoin { spec: Box::new(spec) }`.

No scheduler map, ordinal counter, observer counter or adapter method is
touched. A mismatch returns the first typed error and constructs nothing.

## Accepted constructor algorithm

`try_from_accepted_launch` consumes a sealed borrow over a journal row plus its
Need row. It verifies:

- both rows are already in the same active generation;
- the complete correlations are equal;
- producer and outcome equal the journal's complete TaskSpec;
- Join has ordinal zero;
- AlwaysStart has a positive committed ordinal; and
- the launch state is at least Accepted, including a committed adapter token
  for Host execution.

The returned state is `AcceptedLaunch`. The spec remains owned by the journal,
not duplicated in the handle.

## Await behavior

### ReusableJoin

```text
validate handle structure
→ validate active generation
→ scheduler.ensure_task(stored complete spec)
→ require returned complete correlation == handle correlation
→ stage observer ID and observer/Need cross-reference
→ atomically register observer
```

An existing Join row is observed without task mutation. A new Join row uses the
normal single-task transaction. Host prepare occurs only here, not when the
handle was built.

### AcceptedLaunch

```text
validate handle structure
→ validate active generation
→ resolve exact journal and Need rows by complete correlation
→ do not derive a correlation
→ do not call ensure_task
→ stage and atomically register observer
```

## AwaitMany exact transcript

For `captured = [c0, …]`, source item `x_i`, and `i: u32`:

```text
child_argument(i) =
  RuntimeValue::Tuple([
    RuntimeValue::Tuple([c0, …]),
    RuntimeValue::UInt(RuntimeUInt::U32(i)),
    x_i,
  ])
```

The template consumes that typed value, derives its canonical argument digest,
and builds all `TaskSpec` fields. The request constructor validates that the
template family is `AwaitManyChild`, that the child policy/execution pair is
permitted, and that `source_items.len() <= u32::MAX` and configured work
limits.

The aggregate producer's base argument remains exactly:

```text
RuntimeValue::Tuple(source_items in source order)
```

Captured values belong to the child template transcript and are not silently
added to or removed from the aggregate base transcript.

## Restore/tamper rule

The snapshot stores captured values, source items, the complete typed template,
limit and only the derived rows required to resume active child state. Restore
recomputes every child argument, producer digest, complete spec and correlation.
Any persisted derived row that differs returns
`AwaitManySnapshotError::DerivedChildMismatch { source_index }` before adapter
prepare or scheduler state publication.
