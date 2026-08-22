# Limits and deterministic first-error precedence

## 1. Exact default limits

```rust
RuntimeTaskPlanSealLimits {
    max_task_plan_rows:       65_536,
    max_executable_rows:   1_048_576,
    max_children_per_row:     65_536,
    max_function_roles:       65_536,
    max_request_roles:        65_536,
    max_control_effect_rows:  65_536,
    max_view_bindings:        65_536,
    max_transcript_bytes: 67_108_864,
    max_semantic_work:      4_194_304,
}

ViewTaskPlanBindingLimits {
    rows:                    65_536,
    semantic_work:         4_194_304,
}
```

These limits are validation policy and are not hashed. Existing complete View
product source/range limits continue to apply independently; passing one limit
set does not bypass another.

## 2. Preflight count accounting

Preflight is performed before semantic row traversal in this field order:

1. task-plan rows;
2. total executable rows across fixed tables `0..14`;
3. maximum children on any executable row, evaluated in fixed table/source
   order;
4. maximum roles on any producer function;
5. maximum roles on any request template;
6. total control/effect rows reachable from task plans;
7. View marker count / validated binding count; and
8. statically known transcript byte lower bound.

For each count, checked conversion/addition runs first. Arithmetic overflow is
the error; otherwise `actual > limit` is the limit error. `actual == limit`
passes. The first field in this order that is over limit wins.

## 3. Dynamic semantic work meter

After preflight, the encoder maintains two checked `u64` counters.

### `semantic_work`

Charge exactly one unit for each:

- fixed table header visited;
- executable row visited;
- scalar tag/integer/boolean/option emitted;
- digest field emitted, including a memoized child digest reuse;
- list element or child edge entered;
- UTF-8 string entered, plus one unit for every 32-byte chunk rounded up;
- producer function parameter, capture, endpoint, or child role;
- request argument role, request field, or child-role path step;
- control/effect row, input type, or child contract reference;
- task-plan base row and final task-plan binding row;
- View authority freshness check, coordinate lookup, and binding comparison; and
- expected-key comparison and uniqueness-map insertion.

The charge is checked before the semantic action. If the next charge would make
`semantic_work > max_semantic_work`, the action is not performed and
`SemanticWork` is returned.

### `transcript_bytes`

Before every hasher update, checked-add the exact byte count that update would
write. If the result exceeds `max_transcript_bytes`, no bytes from that update
are written and `TranscriptBytes` is returned. Domain bytes and length/tag
bytes are included. Intermediate owner row transcripts and final transcripts
share the same per-seal meter; memoized digests do not charge their original
row bytes again, only the 32 bytes written at the reuse site.

`actual == limit` passes and one additional charged unit/byte rejects.

## 4. Builder first-error precedence

The builder has no envelope or expected-key errors. Its exact order is:

1. checked arithmetic and preflight limits;
2. existing core structural verification in fixed table/source order;
3. task coordinate owner/order and family/binding compatibility;
4. producer function resolution for the first task row;
5. request-template resolution/encoding for that row;
6. control/effect-contract resolution/encoding for that row;
7. remaining non-task executable rows in fixed table/source order;
8. executable digest finalization;
9. each task row in source order:
   - missing View authority;
   - stale authority;
   - missing View binding;
   - coordinate/base/program/site/admission mismatch;
   - binding transcript work/byte limit;
10. global duplicate check at the second source-order occurrence;
11. final table/cross-reference verification; and
12. publication.

Within structural verification, the current `RuntimePlan::verify` owner keeps
its own accepted deterministic variant order. This contract does not add a
second structural verifier.

## 5. Decode first-error precedence

1. outer envelope magic/version/section bounds;
2. duplicate/unknown section, noncanonical section order, trailing bytes;
3. private RuntimePlan image tag/count/canonical coordinate errors;
4. private expected-key cardinality and raw length errors;
5. private View image canonical decode;
6. arithmetic and preflight limits;
7. core structural/family/binding errors;
8. complete View product and binding table errors:
   - stale current program/revision/source-set stamp;
   - noncanonical coordinate order;
   - duplicate coordinate;
   - missing expected View coordinate;
   - extra non-View coordinate;
   - program mismatch;
   - site mismatch;
   - admission mismatch;
9. child and executable semantic errors as in builder;
10. per-View authority errors as in builder;
11. first source-order expected-key mismatch;
12. first global duplicate at its second row;
13. final cross-reference validation; and
14. atomic publication.

Expected-key mismatch precedes duplicate because each sealed row is compared as
it is collected; the uniqueness index is built only after all expected keys
match.

## 6. View stale/missing/mismatch behavior

- **Stale authority:** the validated resource no longer matches the outer
  current program, accepted revision, or source-set stamp. This is checked once
  per authority call before coordinate lookup and outranks missing binding.
- **Missing binding:** the authority is current but has no row for the exact
  owner-bound coordinate.
- **Mismatched binding:** a row exists but its coordinate owner, View marker,
  family, program, site, or admission does not equal the validated joined
  products. No digest is returned.
- **Extra binding:** complete product construction rejects it before core
  sealing; the authority map is exact coverage, not a permissive registry.

## 7. Duplicate semantics

The key space is one global map per structured RuntimePlan. The second row is
an error even when its binding/family differs from the first. The error records
both source-order coordinates and both binding kinds.

Normal encoders include family and binding, so a cross-family duplicate would
require a hash collision or malformed test fixture. A private `cfg(test)` typed
digest constructor feeds the uniqueness collector directly to prove that the
collector has no family-specific partition. That constructor is not compiled
into production or exposed by the library.

## 8. Failure atomicity

Every error above occurs while state is private. There is no error variant for
“published partial table,” “rollback public plan,” or “repair expected key.”
Failure drops the candidate. The caller must correct/rebuild the complete input.
