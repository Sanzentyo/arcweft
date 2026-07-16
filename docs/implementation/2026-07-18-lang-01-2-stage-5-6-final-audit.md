# Lang-01.2 Stages 5–6 — deletion, tooling, and final audit

## Scope

This audit closes the implementation-ready contract delivered in
`2026-07-16-lang-01.2-implementation-ready-final-contract.zip`, whose SHA-256
is
`F9441CDA8A54F10C5AC594A7C5C3847F047A750107CC1D41DA943B3FFEFC1AA8`.
The package contains 135 normative rows and no open design question.

Stages 1–4 are recorded separately in:

- `2026-07-17-lang-01-2-entry-binding-stage-1.md`;
- `2026-07-17-lang-01-2-binding-validation-stage-2.md`;
- `2026-07-17-lang-01-2-runtime-stage-3.md`; and
- `2026-07-17-lang-01-2-agent-artifact-stage-4.md`.

This document records the breaking deletion, final tooling graph, and the
single evidence ledger used for the complete contract.

## Final source and ownership model

Durable state and Agent controllers now use ordinary declarations:

```arcw
struct GameState {
    score: i32
}

enum GameEvent {
    Increment
}

fn initial_game_state() -> GameState
effects {}
{
    GameState { score = 0 }
}

fn reduce_game(state: &GameState, event: GameEvent)
    -> Result<Reduction<GameState>, ReducerError>
effects {}
{
    match event {
        .Increment => Ok(Reduction.changed(GameState { score = state.score + 1 }))
    }
}

entry game @entry.game.main {
    state = GameState
    initializer = initial_game_state
    event = GameEvent
    reducer = reduce_game
    goto @flow.opening
}
```

The former top-level `state`, `reducer`, and `agent` declarations have no
current CST/AST/HIR family. They recover as ordinary invalid source. The
`.arcw` and `.awfagent` extensions select no grammar dialect. There is no
legacy source switch, compatibility alias, migration reader, dual schema-v1
decoder, or spelling-specific final diagnostic.

The final owners are:

- syntax owns typed entry roles and their exact ranges;
- HIR retains one ordinary nominal/function declaration plus the entry;
- sema owns exact role resolution, contracts, policy, and canonical digests;
- core/runtime owns the selected entry transaction and durable root value;
- save/replay/hot reload compare final entry-bound identities;
- Agent artifacts identify both the selected entry and ordinary controller;
- project index and LSP expose role edges to the original declarations; and
- the launch manifest selects one canonical `entry.*` ID and owns no role.

## Stage 5 result

Repository `.arcw` and `.awfagent` samples, fixtures, manifests, tests, and
stable design examples were migrated to ordinary structs/enums/functions and
typed entries. Public compile-fail tests prove that the removed syntax and HIR
types cannot be named. Parser and compiler tests submit the removed forms
through every supported public entry path and observe only ordinary recovery.

The final Agent and session schema-v1 structures replaced their unpublished
predecessors in place. Strict decoders reject missing required fields, unknown
nested fields, predecessor-only fields, and mixed predecessor/final payloads.
Rejection does not mutate a live session.

## Stage 6 result

Entry-role tooling is built from the accepted checked project and its semantic
index. Definition, references, signature help, hover, completion, outline,
workspace symbols, and rename all retain the ordinary nominal/function owner.
Entry-ID rename joins source entry references and launch-manifest string
tokens, while callable rename updates ordinary calls/import spellings and role
RHS paths without renaming the entry.

Positions use checked UTF-8/UTF-16/UTF-32 conversion. Requests reject invalid
line/character boundaries, stale open-source bytes, and stale secondary
manifest overlays instead of applying a partial workspace edit.

The LSP environment now publishes Rust ABI callables from a selected custom
adapter as its complete accepted publication. A standard adapter augmented
with Rust metadata publishes only the ordered Rust delta beside the fixed
standard publication. This is the same typed publication rule used by the
compiler/CLI path.

## Normative 135-row evidence ledger

The test name is the stable behavioral evidence. Several tests intentionally
cover more than one row where one transaction proves a combined invariant.
The detailed assertion rationale for BIND and runtime families remains in the
Stage 2 and Stage 3 notes linked above.

### Syntax and HIR

| ID | Behavioral evidence |
|---|---|
| `SYN-001` | `stateful_entry_roles_are_typed_and_keep_value_and_member_ranges` |
| `SYN-002` | `editor_test_and_agent_are_direct_entry_kinds` (editor/test cases) |
| `SYN-003` | `editor_test_and_agent_are_direct_entry_kinds` (Agent controller case) |
| `SYN-004` | `stateful_entry_roles_are_typed_and_keep_value_and_member_ranges` parses the complete role set in non-canonical order and preserves each range. |
| `SYN-005` | `entry_kind_and_id_are_both_explicit` |
| `SYN-006` | `duplicate_roles_relate_the_first_and_duplicate_members` |
| `SYN-007` | `removed_role_declarations_are_rejected_by_the_current_grammar` (state) |
| `SYN-008` | `removed_role_declarations_are_rejected_by_the_current_grammar` (reducer) |
| `SYN-009` | `removed_role_declarations_are_rejected_by_the_current_grammar` (Agent) |
| `SYN-010` | `ordinary_document_entrypoints_share_one_parse_result` and `arcw_and_awfagent_documents_share_ast_hir_and_sema_results` |
| `SYN-011` | `typed_view_declaration_preserves_the_view_callable_projection` |
| `SYN-012` | `reserved_role_names_never_fall_through_to_generic_options` |
| `HIR-001` | `ordinary_struct_functions_and_entry_are_the_only_role_owners_in_hir` |
| `HIR-002` | trybuild `tests/ui/removed_role_hir.rs` |

### Binding and identity

| ID | Behavioral evidence |
|---|---|
| `BIND-001` | `bind_001_resolves_stateful_roles_to_original_declarations` |
| `BIND-002` | `bind_002_two_game_entries_keep_independent_role_sets` |
| `BIND-003` | `bind_003_game_editor_and_test_can_share_one_reducer_explicitly` |
| `BIND-004` | `bind_004_each_missing_stateful_role_has_one_stable_diagnostic` |
| `BIND-005` | `bind_005_state_root_type_alias_is_rejected_at_the_rhs` |
| `BIND-006` | `bind_006_generic_state_root_is_rejected_as_open_identity` |
| `BIND-007` | `bind_007_state_rejects_each_non_persistent_transitive_field_with_path` |
| `BIND-008` | `bind_008_event_rejects_non_replay_payload_with_variant_path` |
| `BIND-009` | `bind_009_initializer_with_parameter_is_rejected` |
| `BIND-010` | `bind_010_initializer_wrong_return_reports_expected_and_actual_types` |
| `BIND-011` | `bind_011_initializer_rejects_omitted_open_and_nonempty_effect_contracts` |
| `BIND-012` | `bind_012_reducer_rejects_wrong_parameter_count_and_order` |
| `BIND-013` | `bind_013_reducer_requires_immutable_borrowed_state` |
| `BIND-014` | `bind_014_reducer_requires_owned_event` |
| `BIND-015` | `bind_015_reducer_requires_exact_result_reduction_and_canonical_error` |
| `BIND-016` | `bind_016_reducer_rejects_declared_or_inferred_effects` |
| `BIND-017` | `bind_017_unresolved_and_ambiguous_role_paths_keep_rhs_and_candidates` |
| `BIND-018` | `bind_018_agent_controller_requires_zero_args_and_exact_result` |
| `BIND-019` | `bind_019_agent_effect_outside_declared_policy_is_rejected` |
| `BIND-020` | `bind_020_entry_kind_role_mismatches_have_semantic_backstops` |
| `BIND-021` | `bind_021_duplicate_entry_ids_across_modules_are_rejected` |
| `BIND-022` | `bind_022_removed_controller_role_attributes_are_rejected` and `bind_agent_budget_is_rejected_on_unselected_function` |
| `BIND-023` | `bind_023_initial_flow_requires_one_fixed_owned_selected_state_parameter` |
| `BIND-024` | `bind_024_initializer_accepts_only_ordinary_function_declarations` |
| `BIND-025` | `bind_025_event_role_rejects_alias_and_generic_nominal_roots` |
| `ID-001` | `id_001_rebuilding_identical_project_repeats_every_digest` |
| `ID-002` | `id_002_reversing_module_traversal_order_preserves_binding` |
| `ID-003` | `id_003_state_field_name_order_and_type_each_change_schema_and_binding` |
| `ID-004` | `id_004_event_variant_change_changes_schema_and_binding` |
| `ID-005` | `id_005_reducer_body_only_preserves_binding_but_changes_code_identity` |
| `ID-006` | `id_006_reducer_rename_and_rebind_each_change_binding` plus `id_006_invalid_reducer_signature_and_effect_are_rejected_before_binding` |
| `ID-007` | `id_007_absolute_source_path_does_not_enter_binding` |
| `ID-008` | `id_008_flow_id_or_valid_contract_changes_binding_while_body_only_does_not` |

### Selection

| ID | Behavioral evidence |
|---|---|
| `SEL-001` | `explicit_profile_selection_is_exact` and successful selected-entry compiler lowering |
| `SEL-002` | `profile_entry_is_required_and_fully_qualified` (missing entry) |
| `SEL-003` | `profile_entry_is_required_and_fully_qualified` (short and `@entry` spellings) |
| `SEL-004` | `profile_cannot_duplicate_source_entry_role_bindings` and `source_roles_are_rejected_at_the_key_span` |
| `SEL-005` | `sel_005_checks_selected_entry_identity_and_kind_before_runtime_lowering` |
| `SEL-006` | `direct_source_entry_is_required_and_canonical` |
| `SEL-007` | `bind_002_two_game_entries_keep_independent_role_sets` plus `cli_test_and_bench_profiles_use_profile_sources` |
| `SEL-008` | `editor_test_and_agent_are_direct_entry_kinds`, `launch_kind_owns_editor_and_agent_variants`, `launch_kind_maps_to_the_compiler_entry_selection_without_string_dispatch`, Agent runner entry verification, and AWBC enum round trips |

### Runtime

| ID | Behavioral evidence |
|---|---|
| `RUN-001` | `run_001_initializer_state_is_installed_before_initial_flow_execution` |
| `RUN-002` | `run_002_invalid_initializer_value_aborts_entry_start` |
| `RUN-003` | `run_003_same_ordered_batch_has_identical_outcomes_and_final_state` |
| `RUN-004` | `run_004_committed_commands_preserve_reducer_vector_order` |
| `RUN-005` | `committed_root_request_precedes_later_flow_host_request` |
| `RUN-006` | `run_006_018_rejection_rolls_back_consumes_one_sequence_and_preserves_later_event` |
| `RUN-007` | `run_007_reducer_trap_has_no_partial_commit_and_skips_later_phases` |
| `RUN-008` | `run_008_non_finite_state_and_runtime_handle_trap_without_partial_commit` |
| `RUN-009` | `run_009_011_typed_later_phase_event_is_deferred_to_next_step` |
| `RUN-010` | `run_010_mutable_initial_flow_state_parameter_is_rejected_by_verification` |
| `RUN-011` | `run_009_011_typed_later_phase_event_is_deferred_to_next_step` |
| `RUN-012` | `run_012_017_transition_sequence_exhaustion_is_atomic_and_not_caller_controlled` and `rep_008_sequence_gap_or_duplicate_is_rejected` |
| `RUN-013` | `run_013_verifier_rejects_missing_callable_schema_and_flow_roles` |
| `RUN-014` | `run_014_engine_never_chooses_the_first_flow_without_explicit_selection` |
| `RUN-015` | `initial_flow_owns_a_value_copy_independent_from_durable_root_state` |
| `RUN-016` | `run_016_mixed_valid_and_invalid_ingress_batch_is_rejected_atomically` |
| `RUN-017` | `run_017_queue_limit_is_an_atomic_input_rejection` and the transition-sequence exhaustion case |
| `RUN-018` | `run_006_018_rejection_rolls_back_consumes_one_sequence_and_preserves_later_event` |
| `RUN-019` | `host_failure_is_observed_without_rolling_back_committed_root_state` |

### Save and replay

| ID | Behavioral evidence |
|---|---|
| `SAVE-001` | `save_001_stateful_root_round_trip_preserves_value_sequence_and_entry_contract` |
| `SAVE-002` | `save_002_active_reducer_reports_exact_blocker` |
| `SAVE-003` | `save_003_retained_root_event_reports_exact_non_quiescent_count` and RUN-004 pending-command count |
| `SAVE-004` | `save_004_active_entry_tampering_is_rejected_without_mutation` |
| `SAVE-005` | `save_005_state_and_event_role_tampering_is_rejected_without_mutation` |
| `SAVE-006` | `save_006_invalid_root_value_or_presence_is_rejected_without_mutation` |
| `SAVE-007` | `save_007_predecessor_v1_missing_runtime_generation_pin_is_rejected` and `save_007_unknown_nested_session_field_is_rejected` |
| `SAVE-008` | `arcweft-save` envelope checksum/schema/strict typed-JSON tests |
| `SAVE-009` | stateful missing-root case in SAVE-006 and `save_009_non_stateful_entry_rejects_injected_root_without_mutation` |
| `REP-001` | `rep_001_production_recording_replays_initializer_state_outcome_and_commands` |
| `REP-002` | `rep_002_recorded_rejection_replays_without_changing_state` |
| `REP-003` | `rep_003_artifact_and_binding_mismatch_fail_during_preflight` |
| `REP-004` | `rep_004_initializer_digest_divergence_fails_before_transition` |
| `REP-005` | `rep_005_post_state_digest_divergence_reports_first_transition` |
| `REP-006` | `rep_006_command_divergence_reports_first_command_index` |
| `REP-007` | `rep_007_recorded_external_outcome_is_injected_without_dispatch` |
| `REP-008` | `rep_008_sequence_gap_or_duplicate_is_rejected` |
| `REP-009` | `rep_009_recorded_trap_is_terminal_and_has_no_command_dispatch` |

### Hot reload

| ID | Behavioral evidence |
|---|---|
| `HOT-001` | body-only product identity checks and `hot_009_code_compatible_swap_preserves_root_state_and_sequence_without_reinitializing` |
| `HOT-002` | `unselected_entry_contract_changes_do_not_restart_the_active_entry` |
| `HOT-003` | `state_layout_change_requires_restart` |
| `HOT-004` | event-contract mutation in `every_active_stateful_entry_contract_field_is_hot_swap_critical` |
| `HOT-005` | callable/policy mutation in the stateful and Agent hot-swap critical-field tests |
| `HOT-006` | selected ID/kind/flow mutation in `active_entry_role_family_change_requires_restart` and stateful critical-field tests |
| `HOT-007` | `hot_007_verified_executable_generation_populates_the_selected_root_layout` |
| `HOT-008` | active-step swap rejection and the three `hot_008_*` pending-root-work tests |
| `HOT-009` | `hot_009_code_compatible_swap_preserves_root_state_and_sequence_without_reinitializing` |
| `HOT-010` | `missing_active_entry_or_selected_root_layout_requires_restart` |

### Agent unification

| ID | Behavioral evidence |
|---|---|
| `AGT-001` | `selected_agent_entry_lowers_only_its_exact_ordinary_controller` and `ordinary_agent_entry_round_trips_and_runs_with_exact_artifact_binding` |
| `AGT-002` | `selected_agent_entry_lowers_only_its_exact_ordinary_controller` |
| `AGT-003` | `shared_controller_keeps_one_callable_identity_and_distinct_entry_artifacts` |
| `AGT-004` | budget assertions in `shared_controller_keeps_one_callable_identity_and_distinct_entry_artifacts` |
| `AGT-005` | `agent_artifact_requires_an_exact_agent_entry_and_matching_project_index` and non-Agent runner rejection |
| `AGT-006` | `unbound_agent_effect_function_is_not_discovered_as_a_controller` |
| `AGT-007` | existing Agent observe/wait/capture/action runtime-plan and runner suites |
| `AGT-008` | `controller_bundle_runs_through_bytecode_host_boundary` and exact entry-bound manifest validation |
| `AGT-009` | removed-source parser/compiler tests plus `predecessor_agent_item_schema_v1_is_rejected` and `mixed_predecessor_and_final_schema_v1_is_rejected` |
| `AGT-010` | `arcw_and_awfagent_documents_share_ast_hir_and_sema_results` |
| `AGT-011` | `shared_controller_keeps_one_callable_identity_and_distinct_entry_artifacts` |

### Tooling

| ID | Behavioral evidence |
|---|---|
| `TOOL-001` | `checked_project_index_records_exact_entry_roles_to_original_declarations` |
| `TOOL-002` | `role_rhs_uses_the_ordinary_callable_for_definition_signature_hover_and_rename` |
| `TOOL-003` | `state_and_event_roles_define_their_ordinary_nominal_declarations` |
| `TOOL-004` | callable references in the role-RHS test and `direct_and_aliased_import_calls_share_typed_references_but_rename_only_authored_name` |
| `TOOL-005` | declaration/call/import/role edits in the callable rename tests |
| `TOOL-006` | role-initiated rename in `role_rhs_uses_the_ordinary_callable_for_definition_signature_hover_and_rename` |
| `TOOL-007` | `manifest_entry_token_defines_and_renames_the_source_entry` |
| `TOOL-008` | role/declaration/call signature assertions in the role-RHS test |
| `TOOL-009` | exact declaration and manifest ranges plus `entry_reference_ranges_follow_utf8_utf16_and_utf32_encodings` |
| `TOOL-010` | outline/hover/completion/workspace-symbol assertions in the role-RHS test and `workspace_symbols_union_distinct_worlds_deduplicate_and_ignore_profile_order` |

### Migration and architecture

| ID | Behavioral evidence |
|---|---|
| `MIG-001` | migrated stateful compiler/CLI fixtures, including the checked game/test samples |
| `MIG-002` | RUN-003 through RUN-006 state and command-order golden behavior |
| `MIG-003` | migrated Agent sample/check inventory and ordinary Agent entry end-to-end execution |
| `MIG-004` | syntax and HIR trybuild fixtures `removed_role_items.rs` and `removed_role_hir.rs` |
| `MIG-005` | `removed_role_declarations_are_rejected_by_the_current_grammar`, `source_compiler_entrypoints_reject_removed_role_declarations_at_parse`, and `project_compiler_entrypoints_reject_removed_role_declarations_at_parse` |
| `MIG-006` | final session/Agent schema-v1 round trips and predecessor/mixed/unknown-field rejection tests |
| `MIG-007` | Cargo metadata assertions in `tests/dependency_direction.rs` |
| `MIG-008` | complete ordinary nominal/function callable catalog regressions in sema/compiler/tooling |
| `MIG-009` | syntax and HIR View callable regression tests |
| `MIG-010` | final fmt, check, clippy, and `just test-workspace` gates below |
| `MIG-011` | `lang_entry_binding_layers_remain_sans_project_and_host_io` over structured Cargo metadata |
| `MIG-012` | canonical nightly structure audit below |

## Focused verification

The final focused pass completed:

```text
cargo test -p arcweft-lang-syntax --all-targets
cargo test -p arcweft-lang-hir --all-targets
cargo test -p arcweft-lang-sema entry::
cargo test -p arcweft-lang-sema project_index
cargo test -p arcweft-launch
cargo test -p arcweft-compiler --lib project::entry_tests
cargo test -p arcweft-compiler --lib removed_role
cargo test -p arcweft-compiler --lib arcw_and_awfagent_documents_share_ast_hir_and_sema_results
cargo test -p arcweft-core root_state
cargo test -p arcweft-runtime-driver --test root_command_dispatch
cargo test -p arcweft-runtime-driver --test awbc_product_session save_007
cargo test -p arcweft-runtime-driver hot_
cargo test -p arcweft-agent-protocol artifact::tests
cargo test -p arcweft-agent-runner controller
cargo test -p arcweft-lsp --lib
cargo test -p arcweft-cli --lib direct_source_entry_is_required_and_canonical
cargo test -p arcweft-cli --test check cli_test_and_bench_profiles_use_profile_sources
cargo test -p arcweft-project-loader --test dependency_direction lang_entry_binding_layers_remain_sans_project_and_host_io
```

All completed successfully. The CLI profile test initially exposed a root
module declaration that named its filename-derived child rather than the
profile-selected root module; the fixture now uses the actual `crate` root and
passes through all three profile commands.

## Workspace-regression closure

The workspace pass exposed several callers that had still depended on the
deleted implicit-flow startup behavior. Runtime-accelerator, runtime-plan,
AWBC parity, iterator-witness, native Agent observe, and responsive-stage
placement tests now provide an explicit entry or use the deliberately named
flow-only test constructor. No production path chooses the first flow.

Hot-swap tests also exposed that source/debug metadata was participating in a
content-only executable comparison. `AwbcProgram::executable_identity()` now
owns one canonical executable digest used by both bundle and runtime-driver
boundaries. It excludes source/display metadata while retaining executable
tables, constants, instructions, and canonical string references. This is a
shared AWBC domain operation rather than two field-by-field projections.

The checked-in Web demo bundle was regenerated from its migrated source through
the repository fixture command. Native/Web parity then completed 7/7. One
Windows linker failure was caused by a corrupt generated compiler PDB; a
package-scoped `cargo clean -p arcweft-compiler` removed the generated artifact
and the exact compiler style regression completed 4/4 without a source change.

## Final workspace gates

The following commands are the completion gates for the same checkout:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
CARGO_BUILD_JOBS=2 just test-workspace
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

All five completion gates passed on Jujutsu change `mkpplpty`. Workspace check
completed in 1 minute 45 seconds and strict workspace Clippy completed in 1
minute 53 seconds. The low-parallelism workspace test route completed every
non-CLI workspace test, the 196-test CLI library/binary suite, and all focused
CLI integration suites with zero test failures.

The first unconstrained `just test-workspace` attempt stopped before running
the complete suite because Windows rejected concurrent `rustc` mappings with
OS error 1455 (`the paging file is too small`). At that point Cargo had started
approximately twenty compiler processes. This was an execution-environment
failure, not a compiler diagnostic or test assertion. Re-running the identical
repository test route with `CARGO_BUILD_JOBS=2` kept at most two compiler
processes active and completed successfully without a source change.

The canonical structure audit measured:

```text
files scanned: 3226
Rust files: 1646
Rust physical LOC: 754850
package manifests: 92
violations: 0 error(s), 129 warning(s)
```

The warning inventory is `SIZE001=97`, `SIZE002=6`, `TEST001=23`,
`ARCH002=1`, and `ARCH003=2`. There is no error-level structural exception.
The final review also removed three avoidable ownership hotspots:

- `arcweft-core/src/plan.rs` is now a 660-line facade over the 645-line
  `plan/entry_inventory.rs` responsibility module;
- `arcweft-compiler/src/project.rs` is 889 production lines, with 347 test
  lines in `project/tests.rs`; and
- `arcweft-runtime-driver/src/swap.rs` is 835 production lines, with 648 test
  lines in `swap/tests.rs`.

The runtime Agent budget now has one owner,
`arcweft_core::entry::AgentBudget`. The protocol duplicate and manual budget
projections/comparisons were deleted. Compiler-only adaptation that requires
checked semantic state is owned by `EntryRuntimeProjection`.

Finally, the large selected-call fact variant now uniquely boxes its immutable
`SignatureOrigin`. This preserves clone/equality behavior and the public fact
shape while removing the workspace Clippy failure without a lint suppression,
compatibility wrapper, or second owner.
