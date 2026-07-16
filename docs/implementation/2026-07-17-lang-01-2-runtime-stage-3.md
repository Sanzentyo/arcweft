# Lang-01.2 Stage 3 — root runtime, save/replay, and hot reload

## Outcome

Stage 3 implements the final Lang-01.2 root-state execution boundary on top of
the typed entry catalog from
[Stage 1](2026-07-17-lang-01-2-entry-binding-stage-1.md) and the exact role
validation from
[Stage 2](2026-07-17-lang-01-2-binding-validation-stage-2.md). It closes
`RUN-001` through `RUN-019`, `SAVE-001` through `SAVE-009`, `REP-001` through
`REP-009`, and `HOT-001` through `HOT-010`.

The implementation was validated on Jujutsu change
`mkpplptyovuwzmsyolvltvvuutrpxkmw` over `main`
`9a63ac5512cd75947ba70195681e43ab968f9f12`.

## RuntimePlan and root transaction

`RuntimePlan` now carries only explicit `RuntimeEntrySpec` values. Entry kind,
target, binding, roles, executable callable metadata, flow metadata, nominal
schemas, layouts, contracts, root limits, and command policy are verified
before selection. `Engine::new` leaves flow-bearing plans dormant; only an
exact entry or flow selection can start one. There is no global entry-flow
field or first-flow fallback.

A stateful entry starts atomically:

1. the exact verified entry and stateful contract are selected;
2. its ordinary initializer executes in the pure-callable engine;
3. the value is validated against the selected state schema;
4. candidate durable root state is created at sequence zero;
5. the initial flow receives an owned value-equal copy; and
6. root plus flow are installed together.

Each step preflights its complete unsequenced root-event batch before assigning
internal consecutive `TransitionSequence` values. A reducer invocation buffers
the candidate state and complete command vector. Only a fully valid
`Ok(Reduction)` commits state, sequence, and commands. `Err(ReducerError)`
consumes exactly one sequence without changing state, retains later events,
and allows later runtime phases to run. A trap or invariant failure commits
nothing, does not consume the sequence, abandons later work, and uses the
existing failed-runtime path.

Committed command envelopes are ordered by `(transition, vector_index)`. The
driver dispatches them before host requests produced by later flow work. A
later host failure returns through the normal result/event boundary and never
rolls back the committed root state.

## Save and restore

The existing bundle-session schema ID and version `1` remain unchanged. The
unpublished payload was replaced in place with the final active-entry and
optional-root shape. Required `Option`-valued fields use required-field
deserialization: omission is rejected even when the represented value may be
`null`. Unknown nested fields and predecessor provisional payloads are also
rejected. There is no legacy reader, migration branch, alias, or nested
version.

Snapshot creation checks exact root quiescence and selected entry metadata.
Restore constructs and validates the root, executor, presentation,
virtualization, and view-runtime candidates before aggregate assignment.
Entry ID/kind/binding tampering, state/event identity or layout tampering,
invalid root values, and root-presence mismatches therefore leave the live
session byte-for-byte unchanged.

`RootRuntime` exposes exact blocker counts for an active reducer, retained root
events, and committed commands. The production driver acknowledges committed
root commands in the same step that exposes them, so its durable pending state
is the result-correlation record when the selected route requires a later root
event. Direct core coverage checks the otherwise unobservable committed-command
queue before acknowledgement: two transitions with two commands each produce
`pending_commands = 4`.

## Replay

`RootReplayTraceV1` records exact artifact, selected entry, binding,
state/event identity and layout, initializer digest, sequenced events,
transition outcomes, ordered command digests, traps, and external outcomes.
Replay uses ordinary live ingress while separately verifying the sequence that
core assigns. It stops at the first state, event, outcome, command-index, or
failure divergence. Recorded command outcomes are injected at their recorded
positions; replay never dispatches a real host command.

Replay ownership is split into:

- `session/replay/model.rs` for the final trace and divergence model;
- `session/replay/record.rs` for production recording and canonical
  projections; and
- `session/replay/execute.rs` for preflight and deterministic replay.

The seven-line `session/replay.rs` remains the public responsibility facade.

## Hot reload

`ProgramGeneration` now contains exact per-entry compatibility metadata and a
root layout under `StateId::for_entry_root(entry)`. Compatibility is classified
for the selected entry only. Binding, role family, kind, state/event identity
or layout, initializer/reducer/controller identity or contract, initial-flow
identity or contract, Agent policy, and effective budget changes require a
restart. A change confined to an unselected entry does not create a
Lang-01.2 restart reason.

The canonical Product AWBC executable identity is represented by a stable code
slot in addition to function slots. It includes constants and runtime tables
referenced indirectly by instructions while excluding source maps and
display-map links, whose dense strings and spans do not affect execution.
Consequently a real executable table change cannot be misclassified as
content-only, while source relocation and display-catalog refreshes do not
masquerade as code changes. A code-compatible commit replaces the executor's
verified program while preserving root value, sequence, flow/executor state,
and empty work queues; it does not rerun the initializer.

Swap commit rejects an active runtime step, retained core or deferred root
events, undispatched committed commands, and pending root-command result
correlations. Rejection does not clear or lose that work.

## RUN evidence

| ID | Behavioral evidence |
|---|---|
| `RUN-001` | `run_001_initializer_state_is_installed_before_initial_flow_execution` checks initializer installation, owned flow argument, and sequence zero before the first flow operation. |
| `RUN-002` | `run_002_invalid_initializer_value_aborts_entry_start` checks invalid persistent initializer output and atomic startup failure. |
| `RUN-003` | `run_003_same_ordered_batch_has_identical_outcomes_and_final_state` executes the same ordered batch twice and compares every outcome and final root snapshot. |
| `RUN-004` | `run_004_committed_commands_preserve_reducer_vector_order` checks two transitions by two commands in exact `(transition,index)` order and the exact pending-command blocker count. |
| `RUN-005` | `committed_root_request_precedes_later_flow_host_request` checks driver dispatch order. |
| `RUN-006` | `run_006_018_rejection_rolls_back_consumes_one_sequence_and_preserves_later_event` checks rejection rollback, command suppression, one consumed sequence, and retained later work. |
| `RUN-007` | `run_007_reducer_trap_has_no_partial_commit_and_skips_later_phases` checks terminal trap semantics. |
| `RUN-008` | `run_008_non_finite_state_and_runtime_handle_trap_without_partial_commit` checks finite-number and non-persistent-handle validation before commit. |
| `RUN-009` | `run_009_011_typed_later_phase_event_is_deferred_to_next_step` checks no same-step recursion. |
| `RUN-010` | `run_010_mutable_initial_flow_state_parameter_is_rejected_by_verification` checks the sole durable-mutation boundary. |
| `RUN-011` | `run_009_011_typed_later_phase_event_is_deferred_to_next_step` checks typed event emission without direct root mutation. |
| `RUN-012` | `run_012_017_transition_sequence_exhaustion_is_atomic_and_not_caller_controlled` checks that live callers cannot inject a sequence; `rep_008_sequence_gap_or_duplicate_is_rejected` checks replay gap/duplicate rejection. |
| `RUN-013` | `run_013_verifier_rejects_missing_callable_schema_and_flow_roles` directly checks each missing/mismatched runtime role boundary. |
| `RUN-014` | `run_014_engine_never_chooses_the_first_flow_without_explicit_selection` checks a two-entry/two-flow plan remains dormant and starts only the exact second entry. |
| `RUN-015` | `initial_flow_owns_a_value_copy_independent_from_durable_root_state` checks that local flow mutation cannot alias the root. |
| `RUN-016` | `run_016_mixed_valid_and_invalid_ingress_batch_is_rejected_atomically` checks whole-batch rollback before sequencing. |
| `RUN-017` | `run_017_queue_limit_is_an_atomic_input_rejection` and `run_012_017_transition_sequence_exhaustion_is_atomic_and_not_caller_controlled` check queue and terminal-sequence boundaries. |
| `RUN-018` | The RUN-006/018 test also proves that later flow phases run after an explicit rejection while later root events remain queued. |
| `RUN-019` | `host_failure_is_observed_without_rolling_back_committed_root_state` checks the later host-result boundary. |

## SAVE, replay, and hot-reload evidence

| IDs | Behavioral evidence |
|---|---|
| `SAVE-001` | `save_001_stateful_root_round_trip_preserves_value_sequence_and_entry_contract`. |
| `SAVE-002` | `save_002_active_reducer_reports_exact_blocker`. |
| `SAVE-003` | `save_003_retained_root_event_reports_exact_non_quiescent_count` plus the exact four-command core blocker assertion in RUN-004. |
| `SAVE-004` | `save_004_active_entry_tampering_is_rejected_without_mutation`. |
| `SAVE-005` | `save_005_state_and_event_role_tampering_is_rejected_without_mutation`. |
| `SAVE-006` | `save_006_invalid_root_value_or_presence_is_rejected_without_mutation`. |
| `SAVE-007` | `save_007_predecessor_v1_missing_runtime_generation_pin_is_rejected` and `save_007_unknown_nested_session_field_is_rejected`. |
| `SAVE-008` | Existing `arcweft-save` schema/checksum/strict typed-JSON regression tests. |
| `SAVE-009` | The stateful missing-root case in SAVE-006 and `save_009_non_stateful_entry_rejects_injected_root_without_mutation`. |
| `REP-001`–`REP-009` | One directly numbered production integration test per row in `root_command_dispatch.rs`, covering deterministic commit, rejection, preflight identity, initializer/state/command divergence, external-outcome injection without dispatch, sequence gaps/duplicates, and terminal traps. |
| `HOT-001` | Product AWBC code identity plus `hot_009_code_compatible_swap_preserves_root_state_and_sequence_without_reinitializing`. |
| `HOT-002` | `unselected_entry_contract_changes_do_not_restart_the_active_entry`. |
| `HOT-003`–`HOT-006` | `state_layout_change_requires_restart`, `every_active_stateful_entry_contract_field_is_hot_swap_critical`, `every_active_agent_execution_contract_field_is_hot_swap_critical`, and `active_entry_role_family_change_requires_restart`. |
| `HOT-007` | `hot_007_verified_executable_generation_populates_the_selected_root_layout`. |
| `HOT-008` | Swap-session active-step rejection plus three production pending-root-work integration tests. |
| `HOT-009` | The directly numbered production compatible-commit test preserves root value/sequence and proves the replacement initializer is not run. |
| `HOT-010` | `missing_active_entry_or_selected_root_layout_requires_restart`. |

## Structural audit

The canonical audit scanned 3,204 files, including 1,630 Rust files and
752,434 Rust physical LOC. It reported zero errors and 131 warnings.

The Stage 3 runtime owners were split at their actual responsibility
boundaries:

| Path | Owning crate | Bytes | Physical LOC | Responsibility |
|---|---:|---:|---:|---|
| `src/entry.rs` | `arcweft-core` | 318 | 12 | intentional entry facade |
| `src/entry/identity.rs` | `arcweft-core` | 3,694 | 133 | runtime entry identities and hashes |
| `src/entry/schema.rs` | `arcweft-core` | 32,970 | 961 | verifiable runtime type schemas |
| `src/entry/roles.rs` | `arcweft-core` | 7,705 | 229 | entry role and command contracts |
| `src/root.rs` | `arcweft-core` | 41,290 | 1,159 | isolated root transaction owner |
| `src/awbc/product_step.rs` | `arcweft-core` | 43,308 | 1,152 | Product AWBC step orchestration facade |
| `src/awbc/product_step/execution.rs` | `arcweft-core` | 9,158 | 231 | compact VM host and policy adaptation |
| `src/awbc/product_step/suspension.rs` | `arcweft-core` | 23,204 | 573 | await/host/effect suspension transitions |
| `src/awbc/product_step/lifecycle.rs` | `arcweft-core` | 24,533 | 621 | source, failure, status, and facade lifecycle |
| `src/session/replay.rs` | `arcweft-runtime-driver` | 130 | 7 | replay facade |
| `src/session/replay/model.rs` | `arcweft-runtime-driver` | 8,130 | 213 | trace and divergence data model |
| `src/session/replay/record.rs` | `arcweft-runtime-driver` | 13,097 | 324 | production recorder |
| `src/session/replay/execute.rs` | `arcweft-runtime-driver` | 31,200 | 834 | deterministic replay executor |

`arcweft-core` remains Sans I/O. Entry/schema/transaction ownership adds no
project, manifest, filesystem, network, or tooling dependency. The audit's
remaining warnings are pre-existing review-level workspace hotspots or
architecture advisories; Stage 3 introduces no error-level file.

## Verification

The final Stage 3 checkout passed:

- `cargo fmt --all`;
- `cargo clippy -p arcweft-core --all-targets --all-features -- -D warnings`;
- `cargo clippy -p arcweft-runtime-driver -p arcweft-save --all-targets --all-features -- -D warnings`;
- `cargo test -p arcweft-core root_state --lib --no-fail-fast` — 17 passed;
- `cargo test -p arcweft-core active_reducer --lib --no-fail-fast` — 1 passed;
- `cargo test -p arcweft-core product_step --lib --no-fail-fast` — 8 passed;
- `cargo test -p arcweft-runtime-driver --test root_command_dispatch` — 28 passed;
- `cargo test -p arcweft-runtime-driver --test awbc_product_session save_` — 13 passed;
- `cargo test -p arcweft-runtime-driver swap::tests --lib` — 16 passed;
- `cargo test -p arcweft-save --all-targets` — 10 passed; and
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` — zero errors,
  131 review warnings.

## Completion boundary

This closes Stage 3. Agent ordinary-function compilation and runtime
integration are Stage 4. Old `state`, `reducer`, and `agent` syntax/HIR family
deletion remains Stage 5, and final LSP/tooling migration remains Stage 6.
