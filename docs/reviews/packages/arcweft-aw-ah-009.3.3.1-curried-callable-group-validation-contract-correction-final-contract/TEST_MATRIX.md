\
# TEST MATRIX

## Test rules

- Tests SHALL use typed constructors, public/crate-owned APIs, and resolver outcomes.
- Tests SHALL NOT open source files or assert symbol spellings/file locations.
- Project, standard, and adapter fixtures SHALL use their actual typed candidate IDs.
- One-over SHALL be constructed as `CallableGroupIndex::try_from_usize(schema.groups().len())`.
- Every exact error assertion SHALL include the unwrapped base candidate and exact group.
- No new unchecked public constructor is permitted for corrupt-world tests.

## A. Corrected Cut 1 identity tests

| ID | Exact test name | Fixture/action | Required result |
|---|---|---|---|
| C1-01 | `curried_id_accepts_nonzero_without_schema` | Build a valid non-wrapper base and `group(1)`; call the two-argument constructor without a schema. | `Ok`; accessors return the same base and group. |
| C1-02 | `curried_id_rejects_initial_group` | Valid non-wrapper base plus `group(0)`. | Exact `CallableIdentityError::InvalidCurriedGroup { base, group: group(0) }`. |
| C1-03 | `curried_id_rejects_curried_base` | First build a valid curried ID; use it as the next base. | Exact `InvalidCurriedBase`. |
| C1-04 | `curried_id_rejects_data_last_base` | Build a valid typed `DataLastCallableId`; use `CallableCandidateId::DataLast` as base. | Exact `InvalidCurriedBase`. |
| C1-05 | `curried_id_wrapper_error_precedes_initial_group_error` | Curried or DataLast wrapper plus `group(0)`. | `InvalidCurriedBase`, proving constructor order. |

The obsolete row `curried_id_requires_existing_group` is deleted, not renamed.

## B. Existing resolved-boundary unit tests

| ID | Exact test name | Fixture/action | Required result |
|---|---|---|---|
| RB-01 | `resolved_curried_accepts_exact_multi_group_schema` | Build a project base, a schema with groups 0 and 1, a `CurriedCallableId(base, 1)`, matching Curried instantiation, valid project origin/authority, and call `ResolvedCallable::try_new`. | `Ok`; ID is the Curried wrapper; instantiation base/group match; `schema().group(1)` is the exact group from the supplied `Arc` (typed pointer/reference identity or equivalent direct evidence). |
| RB-02 | `resolved_curried_rejects_project_one_over_group` | Project base; valid schema; structural Curried ID at one-over. | Exact `InvalidCallGroup { candidate: project_base, group: one_over }`. |
| RB-03 | `resolved_curried_rejects_standard_one_over_group` | Standard environment base with matching standard origin/authority. | Exact `InvalidCallGroup { candidate: standard_base, group: one_over }`. |
| RB-04 | `resolved_curried_rejects_adapter_one_over_group` | Adapter environment base with matching adapter origin/authority. | Exact `InvalidCallGroup { candidate: adapter_base, group: one_over }`. |
| RB-05 | `resolved_curried_rejects_base_id_representation` | Existing group, but pass base ID directly with Curried instantiation. | `InvalidResolvedCallable`. |
| RB-06 | `resolved_curried_rejects_mismatched_base` | Curried ID contains base A; instantiation contains base B. | `InvalidResolvedCallable` even if both schemas contain the group. |
| RB-07 | `resolved_curried_rejects_mismatched_group` | Curried ID has group 1; instantiation says group 2. | `InvalidResolvedCallable`, before missing-group classification. |
| RB-08 | `resolved_curried_rejects_non_curried_instantiation` | Curried ID plus `CallableInstantiation::None`. | `InvalidResolvedCallable`. |
| RB-09 | `resolved_curried_rejects_corrupt_world_prebuilt_candidate` | Prebuild nonzero Curried ID before associating it with a one-group record schema. | Exact `InvalidCallGroup`; no repair or fallback. |
| RB-10 | `invalid_call_group_has_stable_diagnostic_code` | Construct the exact resolver error. | `.code() == CallableDiagnosticCode::InvalidCallGroup`. |

## C. Shared-resolver integration tests

These rows land when the one shared resolver reaches the corresponding provider family. They are mandatory before old-resolver deletion.

| ID | Exact test name | Fixture/action | Required result |
|---|---|---|---|
| SR-01 | `shared_resolver_rejects_project_curried_one_over` | Accepted request/world; project catalog record; request one-over continuation. | `ResolveCallOutcome::Rejected(InvalidCallGroup { candidate: project_base, group })`. |
| SR-02 | `shared_resolver_rejects_standard_curried_one_over` | Accepted request/world; standard record; one-over continuation. | Same typed rejection with standard base. |
| SR-03 | `shared_resolver_rejects_adapter_curried_one_over` | Accepted request/world; adapter record; one-over continuation. | Same typed rejection with adapter base. |
| SR-04 | `shared_resolver_publishes_exact_curried_schema_group` | Accepted request/world; multi-group project record; request group 1. | One resolved candidate using Curried ID, matching instantiation, full record schema, exact group 1. |
| SR-05 | `shared_resolver_rejects_initial_curried_group` | Accepted request/world; raw continuation request at group 0. | `Rejected(InvalidCallGroup { candidate: base, group: 0 })`. |
| SR-06 | `shared_resolver_corrupt_world_has_no_fallback` | Crate-private accepted-world fixture associates prebuilt Curried group with schema lacking it while another old/alternate route could otherwise resolve the base. | One `Rejected(InvalidCallGroup)` outcome; no resolved candidate and no legacy/provider retry. |
| SR-07 | `shared_resolver_curried_candidate_matches_checker_target_fact` | Resolve and check a valid multi-group call. | Checker target fact and resolver/query fact contain the same `CallableCandidateId::Curried`. |
| SR-08 | `shared_resolver_curried_result_is_insertion_order_invariant` | Build equivalent accepted catalogs in reversed insertion order. | Identical typed Curried candidate, schema group, origin, authority, and outcome. |

## D. Regression/deletion tests

| ID | Exact test name | Required evidence |
|---|---|---|
| DR-01 | `curried_group_error_does_not_resolve_base_candidate` | A missing group never returns a base-ID candidate. |
| DR-02 | `curried_group_error_does_not_retry_old_resolver` | Instrumented typed resolver fixture records one shared attempt and zero legacy attempts; no source inspection. |
| DR-03 | `curried_group_validation_preserves_accepted_world_guard` | World mismatch is rejected before candidate publication; no curried success is observable. |
| DR-04 | `curried_group_validation_preserves_cancellation` | Cancellation remains the resolver result before publication when cancellation wins under existing policy. |
| DR-05 | `curried_group_validation_charges_existing_work_budget` | Existing work accounting is unchanged; no duplicate schema lookup charge from a second path. |

## Required fixture details

### Multi-group schema

- group 0: `CallableGroupKind::Initial`;
- group 1: `CallableGroupKind::Curried`;
- both indices and parameter indices built by typed constructors;
- no holes, because public schema construction already rejects them.

### One-over

```rust
let one_over = CallableGroupIndex::try_from_usize(schema.groups().len())
    .expect("test group index fits");
assert!(schema.group(one_over).is_none());
```

### Positive exact-group proof

Retain an `Arc` clone before construction and directly prove the resolved product uses the same full schema allocation/group, for example:

```rust
let expected = schema.group(group(1)).expect("group 1");
let resolved = ResolvedCallable::try_new(
    // matching typed inputs
    Arc::clone(&schema),
    // ...
).expect("valid curried product");
let actual = resolved.schema().group(group(1)).expect("published group 1");
assert!(std::ptr::eq(expected, actual));
```

This is direct typed runtime evidence, not a source gate.

## Completion gate

The correction is implementation-complete only when:

- C1-01 through C1-05 pass;
- RB-01 through RB-10 pass;
- SR-01 through SR-06 pass before old-resolver deletion;
- SR-07/SR-08 and DR rows required by the enclosing shared-resolver cut pass;
- focused sema check/clippy/tests and structural audit pass;
- no compatibility path, second resolver, source gate, CSS path, or Takumi path exists.
