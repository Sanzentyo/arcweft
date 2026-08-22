# Test plan and acceptance matrix

## 1. Test levels

Use existing workspace test conventions and fixtures. Names below are normative row identities; exact Rust module prefixes may follow current source layout.

## 2. Prepare tests

| ID / test name | Fixture/action | Required assertion |
|---|---|---|
| RTR-PREP-001 `prepare_restore_is_observer_silent` | valid snapshot; pause after every prepare substep | lookup/handle/match/runnable metrics remain old epoch |
| RTR-PREP-002 `prepare_restore_round_trips_canonical_snapshot` | canonical valid snapshot | prepared digest/slot count/identities equal golden values |
| RTR-PREP-003 `prepare_rejects_unknown_version_before_allocation` | version > supported | `UnsupportedVersion`; allocation/admission counters unchanged |
| RTR-PREP-004 `prepare_rejects_trailing_bytes` | append one byte | framing corruption; no mutation |
| RTR-PREP-005 `prepare_rejects_snapshot_digest_mismatch` | flip semantic payload bit | digest error; no mutation |
| RTR-PREP-006 `prepare_rejects_plan_seal_mismatch` | alter one semantic child | identifies task; no mutation |
| RTR-PREP-007 `prepare_rejects_duplicate_task_identity` | duplicate `(id,generation)` | deterministic duplicate error |
| RTR-PREP-008 `prepare_rejects_dangling_child_reference` | child target absent | exact owner/target in error |
| RTR-PREP-009 `prepare_rejects_non_isomorphic_handle_batch` | reorder/drop/duplicate slot | `HandleBatchNotIsomorphic` |
| RTR-PREP-010 `prepare_rejects_incomplete_match_transcript` | omit generic match row | complete-transcript error |
| RTR-PREP-011 `prepare_rejects_match_coverage_seal_mismatch` | mutate coverage seal | mismatch before commit |
| RTR-PREP-012 `prepared_restore_is_not_cloneable` | compile-fail UI test | `Clone` unavailable and private fields inaccessible |
| RTR-PREP-013 `dropping_prepared_restore_has_no_durable_or_live_effect` | prepare then drop | no journal, task, handle, queue, or match change |
| RTR-PREP-014 `prepare_enforces_decode_budgets` | oversized count/depth/length | bounded error before dangerous allocation |

## 3. Commit and idempotency tests

| ID / test name | Fixture/action | Required assertion |
|---|---|---|
| RTR-COMMIT-001 `commit_consumes_prepared_once` | by-value commit + compile-fail reuse | cannot double commit same object |
| RTR-COMMIT-002 `commit_publishes_batch_atomically` | readers paused around root swap | readers see only complete old or complete new root |
| RTR-COMMIT-003 `commit_rechecks_epoch` | mutate coordinator after prepare | `StaleCoordinatorEpoch`; no journal commit/publication |
| RTR-COMMIT-004 `commit_rechecks_identity_collision` | admit colliding task after prepare | conflict; no overwrite |
| RTR-COMMIT-005 `same_token_same_digest_returns_same_receipt` | lose first reply, retry | receipt equality, one publication/queue seed |
| RTR-COMMIT-006 `same_token_different_digest_is_corruption` | reuse token | hard mismatch, original state intact |
| RTR-COMMIT-007 `different_restore_tokens_do_not_interleave` | concurrent commits | serialized; loser stale/conflict and no partial effects |
| RTR-COMMIT-008 `committed_record_precedes_public_visibility` | event/fault hook trace | order is COMMITTED sync then root swap |
| RTR-COMMIT-009 `post_commit_publication_path_is_infallible` | capacity/invariant preflight | all fallible work occurs before sync decision |
| RTR-COMMIT-010 `restore_wrapper_delegates_to_two_phases` | instrument calls | exactly one prepare and one commit, same errors |

## 4. Handle/snapshot/match tests

| ID / test name | Required assertion |
|---|---|
| RTR-HND-001 `restored_handle_batch_is_snapshot_isomorphic` | every canonical slot preserves ID, generation, capability, and order |
| RTR-HND-002 `hash_iteration_never_changes_handle_digest` | randomized insertion orders produce identical canonical digest |
| RTR-HND-003 `old_generation_handle_cannot_resolve_new_task` | ABA replacement is rejected |
| RTR-HND-004 `no_public_handle_points_to_detached_cell` | all returned handles resolve through published root epoch |
| RTR-MAT-001 `restore_reproduces_normal_admission_match_seal` | normal admission and restore seals are identical |
| RTR-MAT-002 `task_and_match_lookup_share_one_epoch` | concurrent root swap never mixes roots |
| RTR-PLAN-001 `restore_reuses_canonical_semantic_child_encoder` | byte/digest golden equality with normal encoder |
| RTR-CAR-001 `owned_carrier_enum_restore_behavior_lives_in_owner_impl` | source-level/API test; no extension trait/workaround |

## 5. Persistence/golden tests

| ID / test name | Required assertion |
|---|---|
| RTR-PER-001 `restore_prepared_v1_golden_bytes` | exact canonical bytes and digest |
| RTR-PER-002 `restore_committed_v1_golden_bytes` | exact canonical bytes and digest |
| RTR-PER-003 `journal_rejects_torn_tail` | reducer stops at last valid record and reports/truncates per store policy |
| RTR-PER-004 `journal_rejects_unknown_kind` | fail closed |
| RTR-PER-005 `journal_rejects_nonzero_reserved_bits` | fail closed |
| RTR-PER-006 `journal_rejects_duplicate_child_labels` | fail closed |
| RTR-PER-007 `journal_rejects_same_token_different_digest` | corruption |
| RTR-PER-008 `optional_applied_ack_does_not_change_decision` | removing ACK yields same committed state |
| RTR-PER-009 `supported_old_snapshot_normalizes_deterministically` | same current semantic digest |
| RTR-PER-010 `unknown_snapshot_version_mutates_nothing` | zero journal/runtime effects |

## 6. Crash/fault-injection tests

Create one deterministic test per row `RTR-CRASH-000` through `RTR-CRASH-011`, matching CP-00..CP-11 in `05-state-machine-and-persistence.md`. Each test must:

1. run with a fault hook that terminates/reopens persistence at the exact boundary;
2. create a fresh coordinator process fixture;
3. run startup recovery before scheduler admission;
4. assert journal reduction, published epoch, task/handle/match cardinality, queue seed count, and receipt idempotency;
5. assert no task body ran more than allowed.

The especially load-bearing rows are:

- `RTR-CRASH-008 committed_before_publish_is_mandatorily_published_on_restart`;
- `RTR-CRASH-009 publication_before_reply_does_not_duplicate_admission`;
- `RTR-CRASH-010 lost_success_reply_returns_stable_receipt`.

## 7. Concurrency model tests

| ID / test name | Interleavings/assertion |
|---|---|
| RTR-CON-001 `lookup_racing_publication_never_observes_mixed_epoch` | exhaustive/modelled root reads |
| RTR-CON-002 `two_same_token_commits_publish_once` | one receipt, one root swap |
| RTR-CON-003 `two_different_token_commits_recheck_epoch` | deterministic winner; loser no effects |
| RTR-CON-004 `cancel_before_commit_has_no_effect` | drop only |
| RTR-CON-005 `cancel_after_durable_commit_cannot_rollback` | publish completes/replays |
| RTR-CON-006 `shutdown_before_commit_rejects_cleanly` | no journal/publication |
| RTR-CON-007 `shutdown_after_commit_preserves_replay_obligation` | committed state survives |
| RTR-CON-008 `task_cancel_racing_publish_targets_one_epoch` | no detached-cell mutation |
| RTR-CON-009 `journal_io_never_holds_published_root_lock` | instrument lock ownership |
| RTR-CON-010 `restore_never_polls_task_under_restore_gate` | poll counter remains zero until publication |
| RTR-CON-011 `old_root_reader_survives_swap` | safe lifetime and correct old data |
| RTR-CON-012 `pending_publication_has_single_owner` | no leak/double drop under panic injection |

Use `loom` only if already accepted by the workspace; otherwise use the repository's existing deterministic scheduler/model checker. Do not add a dependency solely to satisfy the name.

## 8. Property/fuzz tests

- arbitrary valid task DAG/snapshot → prepare → commit → resnapshot yields equal semantic digest;
- arbitrary corruption at every byte → either strict error/no mutation or valid same semantic form; never panic/over-allocate;
- arbitrary task/handle insertion order → same canonical slot ordering and digest;
- arbitrary journal record sequences → reducer accepts only grammar-valid monotonic states;
- arbitrary duplicate IDs/generations → exact collision policy;
- arbitrary match transcript truncation/permutation → closure verification fails unless canonical equivalent.

## 9. Workspace acceptance gates

Run the exact commands required by the latest applicable `AGENTS.md`. At minimum, subject to those instructions:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Add narrower package tests and feature matrices dictated by the workspace. These commands are implementation acceptance requirements; this design-only package does not claim they were run against a patch.
