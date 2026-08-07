# Proof-concurrency v6.1.1 full acceptance-matrix closure

- Date: 2026-08-06
- Contract authority:
  `docs/reviews/packages/arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip`
- Matrix scope: base-package `TEST_MATRIX.md` only; later accepted correction
  matrices are not yet rolled into this inventory
- Working copy: protected, uncommitted Proof public-switch integration
- Overall status: `IN_PROGRESS`

This note maps every acceptance row in the accepted package's
`TEST_MATRIX.md` to its legitimate production test owner. It deliberately
does not turn static name presence, a nearby test, or a previously green
revision into current PASS evidence. A row becomes `PASS` only after the
listed behavior has run successfully against the final coherent working copy.
The 157-row inventory below must not be reported as full package-chain closure:
later accepted final-HIR, synthetic-role, tail/generator, Select, Call, and
ordinary-Flow corrections have their own matrices and require a separate
precedence-aware rollup. In particular, the blocked `DesugaredTemporary` and
`ClosureEnvironment` rows are not part of the count below.

## Inventory correction

The first review-aid extractor recognized only table cells shaped exactly as
``| `snake_case_name` |``. That produced 144 unique names/paths, but silently
omitted all nine section-7 cells, whose first cells also contain an em dash and
owner text. Sections 2--6 contain 148 owner-qualified occurrences and 144
unique names because four inclusive-limit names occur once for syntax and once
for HIR. Including section 7 gives the authoritative inventory used here:

- **157 owner-qualified row occurrences**;
- **153 unique acceptance identities**; and
- the four repeated limit names remain separate syntax/HIR rows with separate
  owners and evidence.

Compile-fail fixtures and bullet-list limit rows are acceptance rows too. They
must not disappear merely because they are not ordinary Markdown table cells.

## Package-chain role-specific blockers

Two later correction rows are intentionally outside the base-package count
above and remain design-blocked without blocking unrelated Proof publication
work:

- `DesugaredTemporary` production recipe/generator rows: see
  [`2026-08-06-proof-desugared-temporary-production-recipe-design-gap.md`](2026-08-06-proof-desugared-temporary-production-recipe-design-gap.md).
  The fixture-only Pipe/source-scan reservation has been deleted and provides
  no acceptance evidence.
- `ClosureEnvironment` role/payload/consumer rows: see
  [`2026-08-06-proof-closure-environment-payload-consumer-design-gap.md`](2026-08-06-proof-closure-environment-payload-consumer-design-gap.md).
  The role is absent from production rather than being fabricated as an
  unreferenced child; its accepted tag gap is not completion evidence.

Both requests must return either a complete reachable production authority or
a direct deletion decision. Until then, their role-specific matrix rows remain
`DESIGN_BLOCKED` and must not be collapsed into this ledger's zero-`MISSING`
base-package count.

## Status vocabulary

| Status | Meaning |
|---|---|
| `PASS` | The exact or explicitly mapped equivalent behavior ran successfully on this working copy. |
| `NOT_RUN_EXACT` | The contract-named test/fixture exists, but has not run on the final coherent copy. |
| `NOT_RUN_EQUIVALENT` | A legitimate owner test appears to cover the behavior under another name, but current execution and a final behavior comparison remain pending. |
| `PARTIAL` | Evidence covers only part of the row; the uncovered behavior is an implementation/test target. |
| `MISSING` | No legitimate behavioral evidence was found; this is a genuine closure target. |
| `SUPERSEDED` | A later accepted contract changed the behavior. The later authority and deletion evidence must be named; proximity is insufficient. |

No compatibility alias test will be added. When one existing test is the exact
behavior under a stale name, it may be renamed. When one broader owner test
covers several rows, this note maps those rows to that real test instead of
adding wrapper tests.

## 2. Lossless tree and typed attachment

Current owner suites were green before the current manifest/public-switch
edits (`arcweft-lang-syntax`: 783 unit tests, 13 integration tests, public API
trybuild, and three public-parser-authority tests). That historical run is
useful context only; every row below remains non-PASS until the final syntax
suite is rerun.

| Acceptance identity | Legitimate exact/equivalent evidence | State |
|---|---|---|
| `same_line_descendants_receive_distinct_syntax_ids` | `incremental::database_tests::same_line_descendants_receive_distinct_private_grammar_ids` | `NOT_RUN_EQUIVALENT` |
| `lossless_root_round_trips_every_utf8_byte` | `grammar::build::tests::lossless_build_retains_utf8_trivia_and_identity_paths`; `parser::document_tests::one_pass_lexer_classifies_current_token_families_losslessly` | `NOT_RUN_EQUIVALENT` |
| `trivia_only_reparse_preserves_predicate_proof_descendant_ids_and_updates_ranges` | `incremental::database_tests::trivia_reparse_preserves_private_descendant_ids_and_old_snapshot_ranges` | `NOT_RUN_EQUIVALENT` |
| `changed_grammar_node_gets_fresh_id_while_unchanged_siblings_survive` | `incremental::database_tests::changed_private_grammar_node_is_fresh_while_its_sibling_survives` | `NOT_RUN_EQUIVALENT` |
| `same_parent_unique_reorder_preserves_ids` | `incremental::database_tests::unique_private_grammar_siblings_retain_ids_when_reordered` | `NOT_RUN_EQUIVALENT` |
| `cross_parent_move_allocates_fresh_ids` | `incremental::database_tests::moving_a_private_grammar_node_across_block_parents_allocates_a_fresh_id` | `NOT_RUN_EQUIVALENT` |
| `copied_subtree_preserves_one_original_and_allocates_fresh_copy` | `incremental::database_tests::a_private_grammar_copy_is_fresh_while_the_original_retains_its_id` | `NOT_RUN_EQUIVALENT` |
| `recovered_equivalent_node_survives_trivia_change` | `incremental::database_tests::missing_and_error_nodes_reconcile_by_recovery_role`; trivia-reparse test above | `PARTIAL` |
| `missing_child_identity_is_role_and_anchor_specific` | `incremental::database_tests::missing_and_error_nodes_reconcile_by_recovery_role` | `NOT_RUN_EQUIVALENT` |
| `generic_error_nodes_are_distinct_and_deterministic` | `incremental::database_tests::missing_and_error_nodes_reconcile_by_recovery_role`; `parser::recovery::tests::generic_error_preserves_structured_payload_and_shared_projection` | `PARTIAL` |
| `typed_to_rowan_and_rowan_to_typed_round_trip` | `attachment::tests::typed_and_rowan_handles_round_trip_without_range_search` | `NOT_RUN_EQUIVALENT` |
| `wrong_typed_kind_returns_kind_mismatch` | `attachment::tests::missing_and_wrong_kind_paths_fail_without_range_or_text_lookup` | `NOT_RUN_EQUIVALENT` |
| `stale_generation_current_resolution_is_typed_error` | `incremental::database_tests::resolve_current_rejects_an_old_generation_before_node_lookup` | `NOT_RUN_EQUIVALENT` |
| `exact_snapshot_operation_rejects_wrong_snapshot` | `incremental::database_tests::parsed_expression_lookup_is_exact_and_rejects_stale_or_foreign_ownership` | `NOT_RUN_EQUIVALENT` |
| `independent_databases_cannot_resolve_equal_raw_slots` | `incremental::database_tests::independent_databases_cannot_resolve_equal_private_raw_slots` | `NOT_RUN_EQUIVALENT` |
| `foreign_lineage_in_same_database_is_rejected` | `attachment::tests::typed_handle_cannot_cross_an_immutable_snapshot_lineage`; exact fragment lookup test above | `NOT_RUN_EQUIVALENT` |
| `syntax_no_op_returns_exact_arc_and_consumes_nothing` | `incremental::database_tests::no_op_replacements_return_the_exact_current_snapshot` | `NOT_RUN_EQUIVALENT` |
| `fatal_event_validation_failure_is_atomic` | malformed-event coverage in `grammar::build::tests`; no exact database-state/control-ID row located | `PARTIAL` |
| `fatal_attachment_failure_is_atomic` | `incremental::database_tests::fatal_private_attachment_failure_rolls_back_initial_transaction` and `fatal_private_attachment_failure_rolls_back_reparse_transaction` | `NOT_RUN_EQUIVALENT` |
| `syntax_identity_exhaustion_is_atomic` | `incremental::database_tests::invalid_edits_and_exhausted_allocation_commit_nothing`; `fragment_attachment_failure_consumes_no_lineage_or_node_identity` | `NOT_RUN_EQUIVALENT` |
| `tests/ui/unbound_fragment_is_not_parsed_source.rs` | exact fixture proves an unbound fragment cannot satisfy the parsed-source boundary; syntax public-API trybuild passed | `PASS` |
| `tests/ui/attached_fragment_is_not_source_file.rs` | exact fixture proves an attached fragment cannot satisfy the source-file boundary; syntax public-API trybuild passed | `PASS` |
| `tests/ui/syntax_node_id_has_no_raw_constructor.rs` | exact fixture proves the raw syntax-node constructor/fields remain private; syntax public-API trybuild passed | `PASS` |
| `tests/ui/syntax_session_ids_are_not_serde.rs` | exact fixture proves syntax session identities fail the required Serde bounds; syntax public-API trybuild passed | `PASS` |
| `tests/ui/typed_node_constructor_is_private.rs` | exact fixture proves typed attached-node construction remains private; syntax public-API trybuild passed | `PASS` |

## 3. Predicate, proof, and `ProofBlock`

The parser, attached-syntax, final-HIR, sema, and compiler tests below are
legitimate owner evidence. Most package-era rows are broader cross-layer
acceptance tests than any one current unit test, so the mapping names every
owner needed and leaves the row non-PASS until the combined behavior runs.

| Acceptance identity | Legitimate exact/equivalent evidence | State |
|---|---|---|
| `predicate_proof_complete_header_grammar_matrix` | `parser::predicate_proof_tests::complete_headers_emit_distinct_typed_descendant_families_losslessly`; final predicate/proof lowering matrices | `NOT_RUN_EQUIVALENT` |
| `predicate_has_implicit_bool_and_rejects_authored_arrow` | `parser::predicate_proof_tests::predicate_authored_return_is_retained_as_current_typed_recovery`; `final_lowering::item_lowering::tests::predicate::canonical_predicate_freezes_signature_contracts_assertion_body_and_synthetic_owners` | `NOT_RUN_EQUIVALENT` |
| `proof_omitted_return_is_unit` | `final_lowering::item_lowering::tests::proof::proof_body_matrix_distinguishes_unit_nonunit_missing_and_expression_owners`; `proof_return::tests::complete_fact_set_retains_exact_unit_classification` | `NOT_RUN_EQUIVALENT` |
| `proof_non_unit_expression_body_is_typed_once` | proof body matrix above | `NOT_RUN_EQUIVALENT` |
| `proof_non_unit_block_requires_tail` | proof body matrix above; proof-return semantic classifier tests | `NOT_RUN_EQUIVALENT` |
| `requires_must_precede_ensures` | `parser::predicate_proof_tests::current_header_recovery_retains_missing_nodes_and_order_diagnostics`; final callable freeze tests | `NOT_RUN_EQUIVALENT` |
| `predicate_proof_total_clause_limit_counts_both_kinds` | `parser::predicate_proof_tests::generic_where_and_contract_limits_are_per_declaration_and_inclusive` | `PARTIAL` |
| `ordinary_names_share_one_namespace_without_overloading` | final project symbol/callable catalog duplicate-name tests; no single contract-shaped test located | `PARTIAL` |
| `predicate_and_proof_recursion_sccs_are_rejected` | executed exact final-sema owner test rejects every recursive SCC containing Predicate or Proof while retaining every typed call edge | `PASS` |
| `expression_body_and_one_expression_block_are_observably_distinct` | parser proof-block/body tests; final proof body matrix | `NOT_RUN_EQUIVALENT` |
| `proof_block_exact_shapes_and_ranges` | `parser::predicate_proof_tests::proof_block_separates_statements_tail_braces_and_omitted_tail`; `attachment::tests::proof_block_accessors_preserve_statement_pattern_type_and_tail_identity` | `PARTIAL` |
| `predicate_block_exact_shapes_and_ranges` | parser complete-header/block tests; final predicate freeze tests | `PARTIAL` |
| `empty_block_has_distinct_braces_and_omitted_tail` | parser proof-block separation test; proof body matrix | `NOT_RUN_EQUIVALENT` |
| `one_expression_block_retains_authored_tail_identity` | parser proof-block separation test; proof body matrix | `NOT_RUN_EQUIVALENT` |
| `pure_let_initializer_precedes_binding_scope` | final pattern/expression lowering and scope tests; exact cross-layer name absent | `PARTIAL` |
| `proof_call_statement_uses_existing_call_expression` | `final_lowering::item_lowering::tests::proof::canonical_proof_freezes_signature_contracts_proof_call_assertion_and_tail` | `NOT_RUN_EQUIVALENT` |
| `assert_prove_uses_existing_assertion_authority` | same canonical proof freeze test; syntax assertion ownership tests | `NOT_RUN_EQUIVALENT` |
| `predicate_assertion_is_context_error_not_reparse` | `final_analysis::tests::predicate_assertion_reaches_final_sema_with_typed_statement_source_identity`; compiler `proof_and_predicate_assertion_context_errors_are_final_sema_diagnostics` | `NOT_RUN_EQUIVALENT` |
| `proof_runtime_assertions_are_context_errors` | `final_analysis::tests::assertion_context_is_derived_from_final_hir_function_and_proof_scopes`; compiler context-error test | `NOT_RUN_EQUIVALENT` |
| `malformed_header_recovery_keeps_following_declaration` | parser `missing_body_does_not_consume_following_clean_declaration`, `missing_parameter_close_synchronizes_before_the_following_declaration`, and current-header recovery tests | `NOT_RUN_EQUIVALENT` |
| `missing_block_close_uses_zero_width_delimiter_node` | parser `missing_block_close_synchronizes_before_the_following_declaration` and prefix-preservation test | `PARTIAL` |
| `malformed_statement_and_tail_are_poisoned_but_queryable` | parser `malformed_statement_is_typed_without_consuming_following_sibling`; final proof body/freeze recovery tests | `NOT_RUN_EQUIVALENT` |
| `removed_forms_use_ordinary_current_grammar_recovery` | parser current generic-recovery tests and removed-form public compile-fail suite | `PARTIAL` |
| syntax `predicate_parameter_limit_is_inclusive_and_atomic` | parser combined parameter-limit test; no full database-state/control-ID atomic row located | `PARTIAL` |
| syntax `proof_parameter_limit_is_inclusive_and_atomic` | parser combined parameter-limit test; no full database-state/control-ID atomic row located | `PARTIAL` |
| syntax `generic_parameter_limit_is_inclusive_and_atomic` | parser combined generic/where/contract-limit test; no full transaction-state row located | `PARTIAL` |
| syntax `where_predicate_limit_is_inclusive_and_atomic` | parser combined generic/where/contract-limit test; no full transaction-state row located | `PARTIAL` |
| syntax `contract_clause_limit_is_inclusive_and_atomic` | executed `incremental::database_tests::atomicity::predicate_proof_total_clause_limit_counts_both_kinds` checks the shared requires/ensures bound, exact/one-over rollback, unchanged transaction identities, and clean retry for both declaration kinds | `PASS` |
| syntax `statement_limit_is_inclusive_and_atomic` | executed exact/one-over owner test `incremental::database_tests::atomicity::statement_limit_is_inclusive_and_atomic` | `PASS` |
| syntax `expression_limit_is_inclusive_and_atomic` | executed exact/one-over owner test `incremental::database_tests::atomicity::expression_limit_is_inclusive_and_atomic` | `PASS` |
| syntax `type_limit_is_inclusive_and_atomic` | executed exact/one-over owner test `incremental::database_tests::atomicity::type_limit_is_inclusive_and_atomic` | `PASS` |
| syntax `pattern_limit_is_inclusive_and_atomic` | executed exact/one-over owner test `incremental::database_tests::atomicity::pattern_limit_is_inclusive_and_atomic` | `PASS` |
| syntax `diagnostic_limit_is_inclusive_and_atomic` | `incremental::database_tests::diagnostic_limit_is_inclusive_and_one_over_rolls_back` | `NOT_RUN_EQUIVALENT` |
| syntax `identity_bearing_node_limit_is_inclusive_and_atomic` | executed exact/one-over owner test `incremental::database_tests::atomicity::identity_bearing_node_limit_is_inclusive_and_atomic` | `PASS` |

## 4. HIR arenas, liveness, scopes, locals, and captures

The final-HIR implementation has a much finer-grained payload/source-freeze
matrix than the original package. Those legitimate tests are mapped below;
the package's cross-cut rows stay `PARTIAL` where the currently located tests
do not establish every required transaction-state or liveness assertion.

| Acceptance identity | Legitimate exact/equivalent evidence | State |
|---|---|---|
| `every_source_backed_node_maps_to_exact_hir_kind` | final item/expression/type/pattern/statement lowering suites plus `source_index::tests::exact_query_key_uses_typed_owner_and_role_not_source_position` | `PARTIAL` |
| `same_line_hir_nodes_do_not_collide` | typed arena/identity tests and same-line final lowering fixtures; no one exact all-kind row located | `PARTIAL` |
| `trivia_relower_returns_stable_source_ids_with_new_spans` | final incremental item/function/predicate tests and revision-bound source-index tests | `PARTIAL` |
| `changed_source_kind_retires_old_slot_and_allocates_new_kind` | incremental function/metric/layer retirement tests | `PARTIAL` |
| `same_parent_reorder_preserves_hir_ids` | executed equivalent owner test `final_lowering::item_lowering::tests::incremental_reorder_preserves_item_ids_but_changes_only_the_source_order_owner` | `PASS` |
| `cross_parent_move_retires_and_reallocates_hir_ids` | executed exact owner test `final_lowering::expression_lowering::tests::identity::cross_parent_move_retires_and_reallocates_hir_ids` | `PASS` |
| `copied_source_node_gets_fresh_hir_ids` | executed exact owner test `final_lowering::expression_lowering::tests::identity::copied_source_node_gets_fresh_hir_ids` | `PASS` |
| `recovered_source_commits_poisoned_hir_for_tooling` | executed compiler recovered-source owner plus exact LSP `recovered_tooling_lease_retains_exact_source_hir_and_navigation_without_semantics`; both retain the exact poisoned HIR/ParsedSource lease while withholding executable semantics | `PASS` |
| `synthetic_roles_are_stable_and_collision_free` | `identity::tests::synthetic_roles_admit_the_complete_typed_owner_and_ordinal_matrix`, synthetic-key structural/fingerprint tests, and synthetic lowering rows | `NOT_RUN_EQUIVALENT` |
| `old_snapshot_resolves_live_interval` | executed exact owner test `slot::tests::old_snapshot_resolves_live_interval` checks born/live/retired lookup against the retained old snapshot | `PASS` |
| `wrong_module_is_checked_before_slot` | `identity::tests::id_resolve_error_variants_preserve_exact_payload_shapes`; module lookup tests | `PARTIAL` |
| `wrong_kind_corruption_hook_never_panics` | typed resolver/freeze corruption tests exist, but the exact wrong-kind hook row was not located | `PARTIAL` |
| `cross_syntax_database_lowering_is_rejected_atomically` | executed equivalent owner test `final_lowering::item_lowering::tests::foreign_and_stale_attached_roots_poison_the_transaction_without_publication` | `PASS` |
| `stale_syntax_snapshot_lowering_is_rejected_atomically` | executed equivalent foreign/stale attached-root owner test; proof-return stale lease tests supply the wider project boundary | `PASS` |
| `hir_no_op_returns_exact_arc_and_no_invalidation` | executed exact owner test `final_lowering::tests::identical_project_request_retains_exact_module_without_advancing_database_state`; the database selects the current clean module before revision/arena staging and preserves its exact `Arc`, snapshot, invalidation epoch, slot ledger, and complete database state | `PASS` |
| `root_and_nested_scope_kinds_are_allocated_exactly` | `scope::tests::closed_scope_kinds_admit_only_their_semantic_owner_families`; attached-header root-scope and expression-control scope tests | `NOT_RUN_EQUIVALENT` |
| `let_initializer_uses_pre_binding_scope` | executed exact test in `final_lowering::expression_lowering::tests::control` | `PASS` |
| `destructuring_binds_left_to_right_after_initializer` | pattern-lowering binding/local order tests; no exact row located | `PARTIAL` |
| `duplicate_pattern_names_poison_all_duplicate_bindings` | final pattern lowering poison tests; exact duplicate-binding acceptance row not located | `PARTIAL` |
| `underscore_allocates_no_local` | final pattern-lowering binding tests; exact row not located | `PARTIAL` |
| `poisoned_pattern_does_not_leak_names` | predicate/proof parameter and let poison tests | `PARTIAL` |
| `sequential_shadowing_increments_local_generation` | `scope::tests::local_generations_are_nonzero_monotonic_and_nonwrapping`; final block/local tests | `PARTIAL` |
| `mutable_binding_and_mutable_reference_remain_distinct` | executed exact test in final expression-control lowering | `PASS` |
| `closure_capture_order_is_first_use_then_local_id` | executed exact test in final expression-control lowering | `PASS` |
| `closure_parameter_and_inner_shadow_prevent_capture` | executed exact test in final expression-control lowering | `PASS` |
| `if_let_match_while_let_for_scopes_match_contract` | executed statement contract test plus expression `e31_if_let...`/`e32_match...`, Thread braced-arm, missing-tail, and reverse-arm-order owner tests | `PASS` |
| `postcondition_result_is_ensures_only` | final callable contract/synthetic-result tests; exact scope-use row not located | `PARTIAL` |
| `typed_child_beats_disagreeing_display_source` | executed source-freeze substitution/tamper tests across final lowering | `PASS` |
| `recovered_module_is_excluded_from_executable_caches` | executed compiler `recovered_module_never_enters_runtime_plan_or_compile_cache` and LSP post-HIR publication owner; recovered HIR remains tooling-visible while both executable admission and new/old signature caches are empty | `PASS` |
| HIR `module_limit_is_inclusive_and_atomic` | `database::tests::module_limit_and_identity_exhaustion_are_atomic`; final-project limit test | `NOT_RUN_EQUIVALENT` |
| HIR `item_limit_is_inclusive_and_atomic` | `arena::tests::every_typed_arena_enforces_its_exact_and_one_over_limit_atomically` | `NOT_RUN_EQUIVALENT` |
| HIR `scope_limit_is_inclusive_and_atomic` | same every-typed-arena production limit test | `NOT_RUN_EQUIVALENT` |
| HIR `statement_limit_is_inclusive_and_atomic` | same every-typed-arena production limit test | `NOT_RUN_EQUIVALENT` |
| HIR `expression_limit_is_inclusive_and_atomic` | same every-typed-arena test plus select Tier-2 production expression-limit matrix | `NOT_RUN_EQUIVALENT` |
| HIR `type_limit_is_inclusive_and_atomic` | same every-typed-arena production limit test | `NOT_RUN_EQUIVALENT` |
| HIR `pattern_limit_is_inclusive_and_atomic` | same every-typed-arena production limit test | `NOT_RUN_EQUIVALENT` |
| `local_module_limit_is_inclusive_and_atomic` | executed exact/one-over owner test `arena::tests::local_module_limit_is_inclusive_and_atomic` at 65,536/65,537 locals | `PASS` |
| `local_scope_limit_is_inclusive_and_atomic` | executed exact/one-over owner test `final_lowering::item_lowering::tests::extern_capability::local_scope_limit_is_inclusive_and_atomic` at 4,096/4,097 locals | `PASS` |
| `capture_limit_is_inclusive_and_atomic` | executed exact final expression-control lowering test; the backing-arena exact/one-over test remains part of the wider HIR rerun | `PASS` |
| `hir_diagnostic_limit_is_inclusive_and_atomic` | executed exact/one-over owner test `module::tests::hir_diagnostic_limit_is_inclusive_and_atomic` at 1,024/1,025 diagnostics | `PASS` |
| `synthetic_descendant_limit_is_inclusive_and_atomic` | executed exact final expression-control lowering test | `PASS` |
| `total_slot_limit_is_inclusive_and_atomic` | select Tier-2 total-slot exact/one-over tests | `NOT_RUN_EQUIVALENT` |
| `module_identity_exhaustion_is_atomic` | `database::tests::module_limit_and_identity_exhaustion_are_atomic` | `NOT_RUN_EQUIVALENT` |
| `revision_exhaustion_is_atomic` | `database::tests::revision_exhaustion_keeps_the_exact_current_lease` | `NOT_RUN_EQUIVALENT` |
| `slot_identity_exhaustion_is_atomic` | typed arena exact/one-over limits; a dedicated identity-exhaustion row was not located | `PARTIAL` |
| `local_generation_exhaustion_is_atomic` | `scope::tests::local_generations_are_nonzero_monotonic_and_nonwrapping`; no full transaction rollback row located | `PARTIAL` |
| `cache_epoch_exhaustion_is_atomic` | `database::tests::invalidation_epoch_exhaustion_keeps_the_exact_current_lease` | `NOT_RUN_EQUIVALENT` |

## 5. Module-preserving project and unified symbols

The final-project owner now exposes module-qualified borrowed projections.
That is legitimate evidence for the project rows; it is not permission to
recreate a linked module or flattened side table. Symbol rows remain partial
where the current tests do not yet exercise every callable family, visibility
mode, or revision invalidation edge required by the package.

| Acceptance identity | Legitimate exact/equivalent evidence | State |
|---|---|---|
| `ordered_project_iteration_preserves_module_ids` | exact owner test `final_project::tests::ordered_project_iteration_preserves_module_ids`; executed in the seven-row HIR project owner suite | `PASS` |
| `project_module_rejects_package_mismatch` | executed `final_project::tests::module_binding_rejects_identity_mismatch_and_stale_arc`; the later accepted AW-AH-009.3.2 binding owner and 2026-08-06 session decision supersede the base package's single-error owner/name | `PASS` |
| `project_module_rejects_path_mismatch` | the same executed module-binding owner test; the final database-bound constructor rejects the mismatch before publication without rebinding the supplied `Arc` | `PASS` |
| `project_module_rejects_source_mismatch` | the same executed module-binding owner test; the final owner retains and compares the full `SourceDocumentIdentity`, which is stronger than the base package's document-ID-only payload | `PASS` |
| `project_requires_canonical_root_module` | executed `final_project::tests::project_rejects_duplicates_limit_and_mixed_database` checks `MissingRootModule` with no project publication | `PASS` |
| `project_rejects_duplicate_path_and_source` | the same executed project-rejection test; later accepted binding/project admission separates the final error owners and enriches the duplicate-source payload without changing rejection | `PASS` |
| `project_view_allows_recovered_but_executable_view_rejects_first_canonical` | exact owner test executed in the seven-row HIR project owner suite; the 2026-08-06 session decision explicitly selects a tooling project view and a separate clean executable-view admission | `PASS` |
| `exported_parts_iterate_without_flattening` | exact owner test executed in the seven-row HIR project owner suite compares module paths, original item/member IDs, and borrowed record identity | `PASS` |
| `styles_iterate_without_flattening` | exact owner test executed in the seven-row HIR project owner suite compares module paths, original item IDs, and borrowed record identity | `PASS` |
| `one_symbol_table_registers_all_callable_kinds_and_character` | exact owner test `symbol::tests::symbol_projection::one_symbol_table_registers_all_callable_kinds_and_character` covers function, predicate, proof, and Character in one revision-bound table | `NOT_RUN_EXACT` |
| `ordinary_callable_duplicate_names_are_reported_together` | exact owner test plus project-symbol `DuplicateDeclaration { sites }` coalescing now retain one source-ordered group before diagnostic sorting/truncation; final execution is pending | `NOT_RUN_EXACT` |
| `visibility_import_alias_and_qualification_are_uniform` | existing direct/group/glob/alias and visibility tests establish the individual mechanisms; the smallest missing exact row is the table-driven symbol-owner test specified below | `PARTIAL` |
| `symbol_table_revision_invalidates_exact_changed_modules` | module-local HIR invalidation and compiler dependency-fingerprint propagation exist; the smallest missing exact three-unit cache row is specified below | `PARTIAL` |
| `proof_artifact_id_is_session_only_and_snapshot_bound` | exact owner test derives artifacts from registered proofs in two changed snapshots and checks declaration/snapshot/item identity; the HIR session-Serde compile-fail row below supplies the separate codec boundary | `NOT_RUN_EXACT` |
| `compiled_project_contains_no_linked_hir` | `compiler/tests/project_cache_transaction.rs::compiled_project_exposes_the_exact_shared_hir_project` and the exact compiler compile-fail fixture below; the complete public-product field matrix has not run as one row | `PARTIAL` |
| `crates/arcweft-lang-hir/tests/ui/no_linked_module.rs` | exact fixture calls the deleted `HirProject::linked_module` signature; HIR public-API trybuild passed | `PASS` |
| `crates/arcweft-lang-hir/tests/ui/no_append_module_body.rs` | exact fixture calls the deleted `HirModule::append_module_body` signature; HIR public-API trybuild passed | `PASS` |
| `crates/arcweft-compiler/tests/ui/no_linked_hir.rs` | exact fixture is registered by the compiler trybuild suite | `NOT_RUN_EXACT` |
| `crates/arcweft-lang-hir/tests/ui/no_provisional_proof_types.rs` | exact fixture names the removed Proof item/clause/trusted-axiom/ID-reference paths; HIR public-API trybuild passed | `PASS` |
| `crates/arcweft-lang-hir/tests/ui/hir_ids_have_no_raw_constructors.rs` | exact fixture covers database/module/revision/snapshot, all eight arena IDs, and generation constructors; HIR public-API trybuild passed | `PASS` |
| `crates/arcweft-lang-hir/tests/ui/hir_session_ids_are_not_serde.rs` | exact fixture checks Serialize and Deserialize exclusion for the session/source/synthetic/proof-artifact identity families; HIR public-API trybuild passed | `PASS` |

### Duplicate-declaration owner adjudication

No accepted Proof-concurrency v6.1.1 successor package currently stored under
`docs/reviews/packages/` changes the
`ordinary_callable_duplicate_names_are_reported_together` row or replaces its
diagnostic contract. The working-copy correction therefore lives in the
project symbol publication owner:

- collect pairwise collision evidence under the typed `(module, name)` key and
  coalesce it before diagnostic ordering and truncation;
- produce exactly one `DuplicateDeclaration` value for a colliding group, with
  a source-ordered `Box<[SourceSpan]>` containing every declaration site;
- retain the first authored declaration only as the deterministic lookup
  winner, never as an overload set, while the project publication remains
  rejected atomically; and
- make the diagnostic projector consume that grouped source list directly.

Grouping pairwise errors later in CLI/LSP would leave the semantic owner
ambiguous and duplicate the ordering rule; the current owner-level coalescing
avoids that prohibited presentation repair. It is not PASS until its exact test
and affected symbol suite run.

The base Proof package's single `HirProjectError` schema is not the final error
ownership authority. The later accepted AW-AH-009.3.2 archive (SHA-256
`8701FF3AE6024CD62C33C4B36ABDFA358BFA30AA93209655870C475EEA1DD40D`)
introduces `HirProjectModuleError` and direct checked
`HirProjectModule::try_new`; the package-chain intake accepts that correction
with no open question. The repository-visible
[2026-08-06 session-ownership decision](2026-08-06-proof-public-switch-session-ownership-decision.md)
then selects the final database-bound module constructor, complete project
admission, and clean executable-view admission as distinct typed owners.
Current `Wrong*`, duplicate, stale-lease, and recovered-view payloads are
result-preserving enrichments of those owners. No result-changing conflict
remains for rows 2--7 beyond executing their exact/equivalent tests. Recreating
the base single-error enum, aliases, or `From` shims would instead restore a
superseded boundary.

### Smallest remaining exact Section 5 tests

The two partial Section 5 rows do not require another symbol authority, cache,
or compatibility reader. They require one executable cross-owner test each.
The smallest result-complete tests are:

- `symbol::tests::import_linking::visibility_import_alias_and_qualification_are_uniform`
  in `arcweft-lang-hir`: use one table-driven test with three project fixtures.
  The success fixture declares Function, Predicate, and Proof callables at
  public, crate, super, and private visibility in root and nested modules, then
  resolves direct, group, glob, qualified, and aliased imports through the same
  symbol table. It must compare the retained `CallableDeclarationOwner` and
  source revision, not display strings. The two rejection fixtures establish
  typed inaccessible/visibility-escalation failures and deterministic ambiguity
  for two distinct targets under one alias. This is enough to prove uniformity;
  separate tests per spelling would repeat parser setup without strengthening
  the symbol-owner boundary.
- `project_cache_transaction::symbol_table_revision_invalidates_exact_changed_modules`
  in `arcweft-compiler`: compile a three-unit project consisting of a changed
  dependency, a root/dependent unit importing it, and an unrelated unit. After
  populating the compile cache, change only the dependency declaration while
  retaining document identities. The exact row must show that the project
  symbol revision advances, dependency and dependent fingerprints change and
  miss, the unrelated fingerprint stays equal and hits, and no replacement
  project is published before the complete rebuild succeeds. Existing
  dependency-fingerprint propagation and `ProjectCompileUnitSummary` cache
  status are the intended evidence owners; adding a second invalidation table
  or source-name gate would be a prohibited parallel model.

Until those exact tests execute, both rows remain `PARTIAL`. If either test
exposes different production behavior, repair the final symbol/cache owner and
delete the obsolete path; do not weaken the row to distributed static evidence.

## 6. Runtime assertion fault and serialization

The complete owner analysis, package-precedence decision, and distributed
persisted-boundary mapping are recorded in
[`2026-08-06-proof-concurrency-v6-1-1-runtime-assertion-closure.md`](2026-08-06-proof-concurrency-v6-1-1-runtime-assertion-closure.md).
The combined persisted-boundary row is intentionally mapped to the real AWBC,
AWFB, save, cache, checkpoint, and compile-fail owners. A monolithic alias test
would conceal ownership rather than strengthen the evidence.

| Acceptance identity | Legitimate exact/equivalent evidence | State |
|---|---|---|
| `check_failure_retains_exact_session_identity` | exact integration test in `arcweft-runtime-plan/tests/assertion_identity.rs` | `PASS` |
| `enabled_debug_failure_retains_exact_session_identity` | exact integration test in `arcweft-runtime-plan/tests/assertion_identity.rs` | `PASS` |
| `condition_indices_follow_authored_zero_based_order` | exact integration test in `arcweft-runtime-plan/tests/assertion_identity.rs` | `PASS` |
| `condition_index_validation_rejects_invalid_count_and_bounds` | exact integration test in `arcweft-runtime-plan/tests/assertion_identity.rs` | `PASS` |
| `prove_has_no_runtime_mode_or_guard` | exact integration test in `arcweft-runtime-plan/tests/assertion_identity.rs` | `PASS` |
| `release_plan_omits_debug_evaluation_and_inventory` | exact integration test in `arcweft-runtime-plan/tests/assertion_identity.rs` | `PASS` |
| `guard_derivation_uses_typed_seed_and_is_deterministic` | exact integration test in `arcweft-runtime-plan/tests/assertion_identity.rs` | `PASS` |
| `invalid_guard_and_fingerprint_zero_values_are_rejected` | exact integration test in `arcweft-core/tests/runtime_assertion_identity.rs` | `PASS` |
| `runtime_fault_invalid_guard_is_typed_error` | exact integration test in `arcweft-runtime-plan/tests/assertion_identity.rs` | `PASS` |
| `runtime_fault_artifact_mismatch_is_typed_error` | exact integration test in `arcweft-runtime-plan/tests/assertion_identity.rs` | `PASS` |
| `runtime_assertion_core_codec_has_no_session_identity` | exact integration test in `arcweft-core/tests/runtime_assertion_identity.rs` | `PASS` |
| `awbc_bundle_save_checkpoint_cache_round_trip_without_session_ids` | executed distributed actual-owner mapping listed in the runtime closure note: canonical AWBC, AWFB, typed save envelope, verified compile-cache hit, generic fiber checkpoint, and both session-ID exclusion compile-fail suites | `PASS` |
| `core_dependency_graph_excludes_compiler_layers` | executed exact parsed-`cargo metadata` graph test in `arcweft-runtime-host/tests/dependency_direction.rs` | `PASS` |
| `runtime_host_normal_graph_excludes_hir_and_runtime_plan` | executed exact parsed-`cargo metadata` graph test in `arcweft-runtime-host/tests/dependency_direction.rs` | `PASS` |
| `runtime_projection_emits_stable_diagnostic_without_message_parsing` | exact shared projection test in `arcweft-tooling/tests/runtime_assertion_diagnostic.rs`; direct CLI, serve, and native Agent adapters also use the typed projector but their focused tests remain pending | `PASS` |
| `reloaded_artifact_uses_fresh_inventory_without_old_stmt_equality` | executed exact compiler integration test in `arcweft-compiler/tests/assertions.rs` | `PASS` |
| `reloaded_artifact_without_exact_source_association_stays_unassociated` | exact tooling integration test in `arcweft-tooling/tests/runtime_assertion_diagnostic.rs` | `PASS` |
| `crates/arcweft-runtime-plan/tests/ui/runtime_fault_has_no_public_constructor.rs` | exact fixture is registered by the runtime-plan trybuild suite | `PASS` |
| `crates/arcweft-runtime-plan/tests/ui/runtime_session_identity_is_not_serde.rs` | exact fixture is registered by the runtime-plan trybuild suite | `PASS` |
| `crates/arcweft-core/tests/ui/core_cannot_name_hir_ids.rs` | exact fixture is registered by the core trybuild suite | `PASS` |
| `crates/arcweft-core/tests/ui/prove_is_not_runtime_assertion_mode.rs` | exact fixture is registered by the core trybuild suite | `PASS` |

Confirmed execution evidence for these rows, with `CARGO_INCREMENTAL=0`:

- `cargo test -p arcweft-runtime-plan --test assertion_identity --all-features`
  passed 10/10 tests: the nine exact runtime-plan acceptance tests above plus
  the supporting `runtime_fault_profile_mismatch_is_typed_error` test.
- `cargo test -p arcweft-core --test runtime_assertion_identity --all-features
  runtime_assertion_core_codec_has_no_session_identity -- --exact` passed 1/1
  selected test, with one test filtered out.
- `cargo test -p arcweft-core --test runtime_assertion_identity --all-features
  invalid_guard_and_fingerprint_zero_values_are_rejected -- --exact` passed 1/1
  selected test, with one test filtered out.
- `cargo test -p arcweft-runtime-plan --test api_compile --all-features
  removed_runtime_plan_apis_are_unavailable -- --exact` passed 1/1 harness test;
  all three registered fixtures passed, including the two Section 6 fixtures.
- `cargo test -p arcweft-core --test api_compile --all-features
  runtime_assertion_identity_boundaries_are_compile_time_closed -- --exact`
  passed 1/1 harness test and both registered fixtures.
- `cargo test -p arcweft-tooling --test runtime_assertion_diagnostic
  --all-features` passed 2/2 tests, exactly the two tooling rows above.
- `cargo test -p arcweft-compiler --test assertions --all-features
  reloaded_artifact_uses_fresh_inventory_without_old_stmt_equality -- --exact`
  passed 1/1 selected test. The exact compiler artifact-digest unit test also
  passed 1/1 and copied the canonical `ArtifactKey` digest without an assertion-
  specific parallel fingerprint owner.
- `cargo test -p arcweft-runtime-host --test dependency_direction
  --all-features` passed 3/3 tests, including both exact dependency-graph rows.
- The distributed persisted-boundary row passed at each actual owner: canonical
  AWBC assertion payload, AWFB typed assertion payload, typed save envelope,
  verified persistent compile-cache hit, generic fiber checkpoint round trip,
  and the runtime-plan/core compile-fail suites above. No monolithic alias codec
  or session-identity serializer was added.

## 7. Tooling, formatter, recovery, and deletion

These rows deliberately stay non-PASS where current evidence is distributed
across owners. In particular, a stable persisted-only assertion diagnostic is
the accepted fallback when no exact compiler-session association exists; no
adapter may fabricate an old or fresh HIR identity merely to satisfy the
package-era Agent wording.

| Acceptance identity | Legitimate exact/equivalent evidence | State |
|---|---|---|
| `formatter_preserves_lossless_predicate_proof_nodes` — syntax formatter tests | executed exact tooling owner test preserves bytes/trivia, reparses through the attached `SyntaxDatabase`, and resolves distinct final typed Predicate/Proof nodes | `PASS` |
| `lsp_navigation_uses_typed_syntax_and_module_hir_ids` — `arcweft-lsp` | executed exact LSP owner resolves function, predicate, and proof navigation through the compiler-retained ParsedSource, module-qualified HIR IDs, and one accepted `Arc<HirProject>` across two imported modules | `PASS` |
| `cli_diagnostics_render_exact_revision_spans` — `arcweft-cli` | `app::diagnostics::tests::adapter_parity_omits_or_rejects_stale_diagnostic_and_edit_spans`, exact revision rendering tests, and the typed runtime-assertion adapter projection cover parts of the row | `PARTIAL` |
| `agent_runtime_assertion_projection_uses_session_capability` — `arcweft-agent-repl`/tooling | exact-session tooling projection and the section-6 persisted-only fallback are both typed and neither parses the message; the native Agent path currently has only the persisted failure and therefore cannot claim a session capability | `PARTIAL` |
| `verifier_consumes_predicate_proof_arena_records` — `arcweft-verify` | executed exact compiler integration test resolves final Predicate/Proof expression IDs through final analysis and binds the verifier's typed proof artifact/obligation to the originating Proof item | `PASS` |
| `runtime_plan_consumes_project_view_without_flattening` — runtime-plan/compiler | final flow lowering consumes `HirProjectExecutableView`; semantic-fact generation tests and compiler shared-project test cover identity preservation, while `no_linked_hir.rs` proves one removed public path | `PARTIAL` |
| `malformed_removed_form_does_not_hide_following_current_declarations` — syntax integration | ordinary generic-recovery and following-declaration parser tests exist without a removed-spelling recognizer, but the complete malformed-fixture matrix has not been mapped to one executable row | `PARTIAL` |
| `recovered_module_never_enters_runtime_plan_or_compile_cache` — compiler/tooling | executed exact compiler transaction test rejects runtime-plan/cache publication; exact LSP publication test publishes a new tooling-only generation, clears old/new request caches, and retains no executable payload | `PASS` |
| `public_api_surface_contains_only_final_nodes` — compile-fail suites | exact syntax and HIR ownership/identity suites are green, while runtime-plan/core/compiler removed/raw/session/linked fixtures remain mapped but not yet run together on the final copy | `PARTIAL` |

## Validation and remaining closure

### Evidence already obtained on this working copy

- A read-only table inventory found exactly 157 mapped row occurrences and
  153 unique identities. The only duplicates are the four intentional
  syntax/HIR limit rows (`statement`, `expression`, `type`, and `pattern`). An
  exact multiset comparison with the accepted ZIP's `TEST_MATRIX.md` found
  zero missing, added, or multiplicity-mismatched identities.
- The syntax deletion migration reached `cargo check` clean and 649/649
  library tests passed. Subsequent compile-fail fixture additions and the wider
  still-changing public switch mean those results remain focused evidence, not
  final-copy matrix PASS.
- `cargo test -p arcweft-lang-hir --test public_api` passed 1/1 after a
  trybuild overwrite/normal cycle. All five exact section-5 HIR fixtures
  reported `ok` with the intended absent-method, absent-type, private-constructor,
  and missing-Serde-bound failures; those five rows are current PASS evidence.
- `cargo test -p arcweft-lang-syntax --test public_api` passed 1/1. All five
  exact section-2 syntax ownership/identity fixtures reported `ok`; those five
  rows are current PASS evidence.
- The seven-test incremental syntax atomicity owner suite passed 7/7. It
  directly closed the shared predicate/proof contract-clause limit and the
  statement, expression, type, pattern, and identity-bearing-node exact/one-over
  transactions without consuming rejected identities.
- Six focused HIR identity/limit owners passed 6/6: cross-parent move, copied
  source identity, module-local and scope-local exact/one-over counts, HIR
  diagnostic exact/one-over publication, and retained old-snapshot liveness.
- `cargo test -p arcweft-lang-hir --lib --all-features 'final_lowering::'`
  passed 445/445 executed tests with 6 ignored. This run closes the exact
  pre-binding, mutable-binding/reference, closure-capture ordering and
  shadowing, capture-limit, synthetic-descendant-limit, Match scope, attached
  foreign/stale rejection, same-parent reorder, and typed-child source-freeze
  rows mapped above. The later removal of a fixture-only
  `DesugaredTemporary` reservation still requires its renamed produced-role
  test to run again; no blocked recipe row receives credit from this run.
- `cargo test -p arcweft-lang-hir --lib --all-features
  final_lowering::tests::identical_project_request_retains_exact_module_without_advancing_database_state
  -- --exact --nocapture` passed 1/1. The equal-input republish success path is
  gone: exact accepted syntax/source selects the existing clean module before
  revision or arena staging and leaves the complete database state unchanged.
- `cargo test -p arcweft-lang-hir --lib --all-features project::tests:: --
  --nocapture` passed 7/7. The module-preserving iteration, recovered tooling
  view/executable rejection, exported-part projection, and style projection
  exact rows all ran and reported `ok`.
- `cargo check -p arcweft-lang-sema --all-targets --all-features` passed after
  the fact-state, type-rule, and callable-effect owner corrections. The focused
  final-analysis suite passed 46/46, including the exact recursion-SCC,
  callable-body-result, and registered-environment recheck rows; the complete
  sema library suite then passed 145/145.
- `cargo test -p arcweft-tooling --lib --all-features
  tests::formatter_preserves_lossless_predicate_proof_nodes -- --exact`
  passed 1/1 and reparsed the byte-identical output through the attached syntax
  owner.
- `cargo test -p arcweft-compiler --test assertions --all-features
  verifier_consumes_predicate_proof_arena_records -- --exact` passed 1/1 after
  deleting the stale `Bool` fixture spelling in favor of canonical `bool`; no
  nominal compatibility alias was restored.
- Focused core, runtime-host, and runtime-driver assertion materialization and
  transport tests passed before the later CLI/Agent publication edits. They
  remain useful behavioral evidence, but are not substituted for the exact
  package rows or for final-copy validation.
- `rustfmt` completed for the direct CLI, serve, native Agent, and controller
  assertion-publication files.
- `git diff --check` passed for those publication files.

### Not run on the final coherent copy

- the exact/equivalent rows still marked `NOT_RUN_EXACT` or
  `NOT_RUN_EQUIVALENT` above;
- focused CLI, serve, native Agent, and Agent-controller assertion tests;
- changed-crate checks and Clippy;
- workspace check, strict workspace Clippy, and `just test-workspace`;
- applicable Tier 2 suites; and
- the final structural/dependency audit.

Cargo validation is intentionally not being multiplied while the shared
workspace target-space constraint and another coordinated Cargo process are
active. This is an execution constraint, not a PASS and not an architecture
exception.

### Genuine implementation or evidence gaps

- section 3: remaining exact syntax transactions not already marked `PASS`;
- section 4: the remaining scope/local/capture/liveness/rollback rows not
  already marked `PASS`;
- section 5: all-callable visibility/import parity and exact
  changed/dependent/unrelated symbol invalidation;
- section 6: final execution of every exact row and every legitimate owner of
  the distributed persistence row; and
- section 7: exact LSP navigation, recovered-cache, Agent session-capability,
  and cross-owner public-surface acceptance rows.

These gaps must be implemented at their typed owner or retained explicitly as
open evidence. They must not be closed with compatibility aliases, wrapper
tests, source greps, removed-spelling recognizers, or duplicate readers.
