# Proof-concurrency v6.1.1 full acceptance-matrix closure

- Date: 2026-08-06
- Contract authority:
  `docs/reviews/packages/zips/arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip`
- Matrix scope: base-package `TEST_MATRIX.md` only; later accepted correction
  matrices are not yet rolled into this inventory
- Working copy: protected, uncommitted Proof public-switch integration
- Overall status: `COMPLETE_FOR_IMPLEMENTABLE_SCOPE`

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
| `EXCLUDED_DESIGN_BLOCKED` | A later correction must select the production contract. The row is outside the user-authorized implementable scope and receives no inferred implementation. |

No compatibility alias test will be added. When one existing test is the exact
behavior under a stale name, it may be renamed. When one broader owner test
covers several rows, this note maps those rows to that real test instead of
adding wrapper tests.

## 2. Lossless tree and typed attachment

The current attached-syntax owner ran after the terminal-newline and bounded-
expression-transaction corrections: all 690 library tests, all three public-
parser-authority integration tests, and the public API trybuild harness passed.
Rows remain `PARTIAL` where that run proves only a subset of the package's
required identity/range or malformed-input matrix.

| Acceptance identity | Legitimate exact/equivalent evidence | State |
|---|---|---|
| `same_line_descendants_receive_distinct_syntax_ids` | executed `incremental::database_tests::same_line_descendants_receive_distinct_private_grammar_ids` | `PASS` |
| `lossless_root_round_trips_every_utf8_byte` | executed `grammar::build::tests::lossless_build_retains_utf8_trivia_and_identity_paths`; `parser::document_tests::one_pass_lexer_classifies_current_token_families_losslessly` | `PASS` |
| `trivia_only_reparse_preserves_predicate_proof_descendant_ids_and_updates_ranges` | executed exact test `incremental::database_tests::trivia_only_reparse_preserves_predicate_proof_descendant_ids_and_updates_ranges`; it compares the complete attached Predicate/Proof identity inventory, exact old/new name ranges, changed generation/revision, new spans, and retained old-handle spans | `PASS` |
| `changed_grammar_node_gets_fresh_id_while_unchanged_siblings_survive` | executed `incremental::database_tests::changed_private_grammar_node_is_fresh_while_its_sibling_survives` | `PASS` |
| `same_parent_unique_reorder_preserves_ids` | executed `incremental::database_tests::unique_private_grammar_siblings_retain_ids_when_reordered` | `PASS` |
| `cross_parent_move_allocates_fresh_ids` | executed `incremental::database_tests::moving_a_private_grammar_node_across_block_parents_allocates_a_fresh_id` | `PASS` |
| `copied_subtree_preserves_one_original_and_allocates_fresh_copy` | executed `incremental::database_tests::a_private_grammar_copy_is_fresh_while_the_original_retains_its_id` | `PASS` |
| `recovered_equivalent_node_survives_trivia_change` | executed recovery-role reconciliation proves exact ErrorExpression/MissingName identities, roles, and old/new ranges | `PASS` |
| `missing_child_identity_is_role_and_anchor_specific` | executed exact `incremental::database::tests::missing_child_identity_is_role_and_anchor_specific`; it covers distinct close/tail IDs and zero-width anchors, trivia reconciliation, fresh identity for a different expected delimiter class, and fresh identity for the same missing close role/class under a different Proof-block parent | `PASS` |
| `generic_error_nodes_are_distinct_and_deterministic` | executed recovery-role reconciliation and `parser::recovery::tests::generic_error_preserves_structured_payload_and_shared_projection` | `PASS` |
| `typed_to_rowan_and_rowan_to_typed_round_trip` | `incremental::database::tests::typed_to_rowan_and_rowan_to_typed_round_trip` passed 1/1. The table-driven exact-marker matrix rebounds the complete attached identity inventory without range/text lookup and covers Predicate/Proof items, parameters, clauses, delimiters, statement families, authored/omitted tails, and leaf expression/pattern/type/path families. It also proves that the ID-less `StatementList` remains a structural wrapper while typed block accessors preserve statement order and tail family | `PASS` |
| `wrong_typed_kind_returns_kind_mismatch` | executed `attachment::tests::missing_and_wrong_kind_paths_fail_without_range_or_text_lookup` | `PASS` |
| `stale_generation_current_resolution_is_typed_error` | executed `incremental::database_tests::resolve_current_rejects_an_old_generation_before_node_lookup` | `PASS` |
| `exact_snapshot_operation_rejects_wrong_snapshot` | executed `incremental::database_tests::parsed_expression_lookup_is_exact_and_rejects_stale_or_foreign_ownership` | `PASS` |
| `independent_databases_cannot_resolve_equal_raw_slots` | executed `incremental::database_tests::independent_databases_cannot_resolve_equal_private_raw_slots` | `PASS` |
| `foreign_lineage_in_same_database_is_rejected` | executed `attachment::tests::typed_handle_cannot_cross_an_immutable_snapshot_lineage` and exact fragment lookup | `PASS` |
| `syntax_no_op_returns_exact_arc_and_consumes_nothing` | executed `incremental::database_tests::no_op_replacements_return_the_exact_current_snapshot` | `PASS` |
| `fatal_event_validation_failure_is_atomic` | executed malformed-event transaction coverage proves no partial syntax publication | `PASS` |
| `fatal_attachment_failure_is_atomic` | executed initial and reparse attachment rollback tests | `PASS` |
| `syntax_identity_exhaustion_is_atomic` | executed exhausted-allocation and fragment-attachment rollback tests | `PASS` |
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
| `predicate_proof_complete_header_grammar_matrix` | `parser::predicate_proof_tests::predicate_proof_complete_header_grammar_matrix`, `incremental::database::tests::same_line_descendants_receive_distinct_syntax_ids`, and `attachment::callable::tests::predicate_proof_headers_bind_exact_children_and_malformed_recovery_roles` each passed 1/1. Together they cover all visibility forms, lifetime/type generics, tuple destructuring, return/contracts/body order, multiple source-ordered where predicates, exact typed wrapper/child ranges and distinct IDs, plus the second fixed-parameter-group recovery without a parallel header reader | `PASS` |
| `predicate_has_implicit_bool_and_rejects_authored_arrow` | executed `parser::predicate_proof_tests::predicate_authored_return_is_retained_as_current_typed_recovery` and final Predicate freeze owner in the complete syntax/HIR suites | `PASS` |
| `proof_omitted_return_is_unit` | executed `final_lowering::item_lowering::tests::proof::proof_body_matrix_distinguishes_unit_nonunit_missing_and_expression_owners` and `proof_return::tests::complete_fact_set_retains_exact_unit_classification` | `PASS` |
| `proof_non_unit_expression_body_is_typed_once` | executed proof body matrix above | `PASS` |
| `proof_non_unit_block_requires_tail` | executed proof body matrix and proof-return semantic classifier tests | `PASS` |
| `requires_must_precede_ensures` | executed `parser::predicate_proof_tests::current_header_recovery_retains_missing_nodes_and_order_diagnostics` and final callable freeze tests | `PASS` |
| `predicate_proof_total_clause_limit_counts_both_kinds` | executed shared Predicate/Proof requires-plus-ensures exact/one-over owner tests | `PASS` |
| `ordinary_names_share_one_namespace_without_overloading` | executed mapped owner set: `ordinary_callable_duplicate_names_are_reported_together` rejects Function/Predicate/Proof as one grouped module-name collision, `visibility_import_alias_and_qualification_are_uniform` preserves those same three owners through two-module direct/group/glob/alias/qualified routes and typed ambiguity, and `proof_artifact_id_is_session_only_and_snapshot_bound` proves an authored Proof keeps the ordinary declaration key while its artifact is derived and session-bound | `PASS` |
| `predicate_and_proof_recursion_sccs_are_rejected` | executed exact final-sema owner test rejects every recursive SCC containing Predicate or Proof while retaining every typed call edge | `PASS` |
| `expression_body_and_one_expression_block_are_observably_distinct` | executed parser proof-block/body owners | `PASS` |
| `proof_block_exact_shapes_and_ranges` | executed proof block separation and attached accessor owners, including exact clean/recovered ranges | `PASS` |
| `predicate_block_exact_shapes_and_ranges` | executed predicate block shape/range owners | `PASS` |
| `empty_block_has_distinct_braces_and_omitted_tail` | executed parser proof-block separation owner | `PASS` |
| `one_expression_block_retains_authored_tail_identity` | executed parser proof-block separation and attachment owners | `PASS` |
| `pure_let_initializer_precedes_binding_scope` | executed exact Proof-block owner test `item_lowering::tests::proof::pure_let_initializer_precedes_binding_scope`; it retains one typed pattern/type/initializer payload, proves initializer-first allocation, resolves the initializer to the outer parameter, and resolves the tail to the new binding | `PASS` |
| `proof_call_statement_uses_existing_call_expression` | executed syntax owner proves Proof-call reuses the existing `CallExpression` authority | `PASS` |
| `assert_prove_uses_existing_assertion_authority` | executed assertion syntax owner proves `assert prove` reuses the existing assertion authority | `PASS` |
| `predicate_assertion_is_context_error_not_reparse` | executed `final_analysis::tests::predicate_assertion_reaches_final_sema_with_typed_statement_source_identity` and compiler `proof_and_predicate_assertion_context_errors_are_final_sema_diagnostics` | `PASS` |
| `proof_runtime_assertions_are_context_errors` | executed `final_analysis::tests::assertion_context_is_derived_from_final_hir_function_and_proof_scopes` and compiler context-error test | `PASS` |
| `malformed_header_recovery_keeps_following_declaration` | `parser::predicate_proof_tests::malformed_header_recovery_keeps_following_declaration` and `attachment::callable::tests::predicate_proof_headers_bind_exact_children_and_malformed_recovery_roles` each passed 1/1. Missing name, generic name, parameter close, where colon, body, and extra fixed group retain their exact typed recovery role/range, lossless bytes, and a distinct queryable following Proof identity | `PASS` |
| `missing_block_close_uses_zero_width_delimiter_node` | executed parser owner proves zero-width missing close and a distinct following Proof item | `PASS` |
| `malformed_statement_and_tail_are_poisoned_but_queryable` | the parser owner retains the typed Let, missing initializer, malformed tail, following Proof, and lossless bytes without inventing an error statement or omitted tail. `final_lowering::item_lowering::tests::proof::malformed_proof_body_stays_queryable_while_following_proof_keeps_clean_identity` passed 1/1 and ties that exact fixture to the synthetic initializer insertion, typed poisoned Try tail, source queries, distinct clean following-Proof identity, tooling project view, and executable-view rejection | `PASS` |
| `removed_forms_use_ordinary_current_grammar_recovery` | parser owners cover ownership-block, trusted-declaration, clause-keyword, and calculation-form inputs through ordinary diagnostic families. `final_lowering::item_lowering::tests::removed_form_recovery::ordinary_removed_form_recovery_keeps_following_final_hir_queryable_until_repaired` passed 1/1 over all four forms × following Function/Predicate/Proof: the recovered module stays non-executable, each following declaration remains clean, source-backed, and HIR-queryable, and deleting the malformed prefix retains its `ItemId` and yields an executable module. No removed-spelling recognizer or dedicated diagnostic was added. The base entity-style-Proof subcase is superseded by the maintained explicit-identity grammar in [`proofs-and-unsafe-audits.md`](../01-language/proofs-and-unsafe-audits.md) and its typed `DeclarationPublicId` authority | `PASS` |
| syntax `predicate_parameter_limit_is_inclusive_and_atomic` | executed exact/one-over owner test with atomic rollback | `PASS` |
| syntax `proof_parameter_limit_is_inclusive_and_atomic` | executed exact/one-over owner test with atomic rollback | `PASS` |
| syntax `generic_parameter_limit_is_inclusive_and_atomic` | executed exact/one-over owner test with atomic rollback | `PASS` |
| syntax `where_predicate_limit_is_inclusive_and_atomic` | executed exact/one-over owner test with atomic rollback | `PASS` |
| syntax `contract_clause_limit_is_inclusive_and_atomic` | executed `incremental::database_tests::atomicity::predicate_proof_total_clause_limit_counts_both_kinds` checks the shared requires/ensures bound, exact/one-over rollback, unchanged transaction identities, and clean retry for both declaration kinds | `PASS` |
| syntax `statement_limit_is_inclusive_and_atomic` | executed exact/one-over owner test `incremental::database_tests::atomicity::statement_limit_is_inclusive_and_atomic` | `PASS` |
| syntax `expression_limit_is_inclusive_and_atomic` | executed exact/one-over owner test `incremental::database_tests::atomicity::expression_limit_is_inclusive_and_atomic` | `PASS` |
| syntax `type_limit_is_inclusive_and_atomic` | executed exact/one-over owner test `incremental::database_tests::atomicity::type_limit_is_inclusive_and_atomic` | `PASS` |
| syntax `pattern_limit_is_inclusive_and_atomic` | executed exact/one-over owner test `incremental::database_tests::atomicity::pattern_limit_is_inclusive_and_atomic` | `PASS` |
| syntax `diagnostic_limit_is_inclusive_and_atomic` | executed `incremental::database_tests::diagnostic_limit_is_inclusive_and_one_over_rolls_back` | `PASS` |
| syntax `identity_bearing_node_limit_is_inclusive_and_atomic` | executed exact/one-over owner test `incremental::database_tests::atomicity::identity_bearing_node_limit_is_inclusive_and_atomic` | `PASS` |

## 4. HIR arenas, liveness, scopes, locals, and captures

The final-HIR implementation has a much finer-grained payload/source-freeze
matrix than the original package. Those legitimate tests are mapped below;
the package's cross-cut rows stay `PARTIAL` where the currently located tests
do not establish every required transaction-state or liveness assertion.

| Acceptance identity | Legitimate exact/equivalent evidence | State |
|---|---|---|
| `every_source_backed_node_maps_to_exact_hir_kind` | executed exact module-resolution test `every_source_backed_node_maps_to_exact_hir_kind`; every source-backed typed arena family round-trips through its kind-specific resolver, while wrong-kind, sibling-lineage, and foreign-database queries return typed errors | `PASS` |
| `same_line_hir_nodes_do_not_collide` | executed exact equivalent owner test `module::resolution_tests::same_line_hir_nodes_do_not_collide`; one dense physical line allocates distinct source-backed IDs across every typed HIR arena family | `PASS` |
| `trivia_relower_returns_stable_source_ids_with_new_spans` | executed exact owner test `pattern_lowering::tests::attached_matrix::trivia_relower_returns_stable_source_ids_with_new_spans`; source-key-matched Pattern/local/synthetic identities remain stable while both whole and nested-name spans move to the new revision and the old module retains its old spans | `PASS` |
| `changed_source_kind_retires_old_slot_and_allocates_new_kind` | executed exact owner test `expression_lowering::tests::identity::changed_source_kind_retires_old_slot_and_allocates_new_kind`; the old expression remains live only in the old snapshot, the current snapshot reports `Retired`, and the new Pattern owns a distinct slot without reinterpretation | `PASS` |
| `same_parent_reorder_preserves_hir_ids` | executed equivalent owner test `final_lowering::item_lowering::tests::incremental_reorder_preserves_item_ids_but_changes_only_the_source_order_owner` | `PASS` |
| `cross_parent_move_retires_and_reallocates_hir_ids` | executed exact owner test `final_lowering::expression_lowering::tests::identity::cross_parent_move_retires_and_reallocates_hir_ids` | `PASS` |
| `copied_source_node_gets_fresh_hir_ids` | executed exact owner test `final_lowering::expression_lowering::tests::identity::copied_source_node_gets_fresh_hir_ids` | `PASS` |
| `recovered_source_commits_poisoned_hir_for_tooling` | executed compiler recovered-source owner plus exact LSP `recovered_tooling_lease_retains_exact_source_hir_and_navigation_without_semantics`; both retain the exact poisoned HIR/ParsedSource lease while withholding executable semantics | `PASS` |
| `synthetic_roles_are_stable_and_collision_free` | the complete HIR suite executed `identity::tests::synthetic_roles_admit_the_complete_typed_owner_and_ordinal_matrix`, structural-key tests, and every non-blocked lowering row. The role-tag/fingerprint portions affected by `DesugaredTemporary` and `ClosureEnvironment` are explicitly excluded pending their external correction contracts | `EXCLUDED_DESIGN_BLOCKED` |
| `old_snapshot_resolves_live_interval` | executed exact owner test `slot::tests::old_snapshot_resolves_live_interval` checks born/live/retired lookup against the retained old snapshot | `PASS` |
| `wrong_module_is_checked_before_slot` | executed exact owner test `slot::tests::wrong_module_is_checked_before_slot`; it rejects the foreign module before consulting a coincident slot | `PASS` |
| `wrong_kind_corruption_hook_never_panics` | executed exact owner test `slot::tests::wrong_kind_corruption_hook_never_panics`; the corruption hook returns the typed wrong-kind error without panicking | `PASS` |
| `cross_syntax_database_lowering_is_rejected_atomically` | executed equivalent owner test `final_lowering::item_lowering::tests::foreign_and_stale_attached_roots_poison_the_transaction_without_publication` | `PASS` |
| `stale_syntax_snapshot_lowering_is_rejected_atomically` | executed equivalent foreign/stale attached-root owner test; proof-return stale lease tests supply the wider project boundary | `PASS` |
| `hir_no_op_returns_exact_arc_and_no_invalidation` | executed exact owner test `final_lowering::tests::identical_project_request_retains_exact_module_without_advancing_database_state`; the database selects the current clean module before revision/arena staging and preserves its exact `Arc`, snapshot, invalidation epoch, slot ledger, and complete database state | `PASS` |
| `root_and_nested_scope_kinds_are_allocated_exactly` | executed `scope::tests::closed_scope_kinds_admit_only_their_semantic_owner_families`, attached-header root-scope, and expression-control scope tests in the complete HIR suite | `PASS` |
| `let_initializer_uses_pre_binding_scope` | executed exact test in `final_lowering::expression_lowering::tests::control` | `PASS` |
| `destructuring_binds_left_to_right_after_initializer` | executed exact final-lowering test `expression_lowering::tests::control::destructuring_binds_left_to_right_after_initializer`; the initializer uses the pre-binding scope and the committed locals follow pattern preorder | `PASS` |
| `duplicate_pattern_names_poison_all_duplicate_bindings` | executed exact final-lowering test `expression_lowering::tests::control::duplicate_pattern_names_poison_all_duplicate_bindings`; the first local remains the lookup winner and every later duplicate receives its own poisoned non-winning local | `PASS` |
| `underscore_allocates_no_local` | executed exact pattern-lowering test `pattern_lowering::tests::underscore_allocates_no_local` | `PASS` |
| `poisoned_pattern_does_not_leak_names` | executed exact final-lowering test `expression_lowering::tests::control::poisoned_pattern_does_not_leak_names` | `PASS` |
| `sequential_shadowing_increments_local_generation` | executed exact final-lowering test `expression_lowering::tests::control::sequential_shadowing_increments_local_generation` together with the closed scope-generation owner tests | `PASS` |
| `mutable_binding_and_mutable_reference_remain_distinct` | executed exact test in final expression-control lowering | `PASS` |
| `closure_capture_order_is_first_use_then_local_id` | executed exact test in final expression-control lowering | `PASS` |
| `closure_parameter_and_inner_shadow_prevent_capture` | executed exact test in final expression-control lowering | `PASS` |
| `if_let_match_while_let_for_scopes_match_contract` | executed statement contract test plus expression `e31_if_let...`/`e32_match...`, Thread braced-arm, missing-tail, and reverse-arm-order owner tests; Match has no container-wide `Block`, and each ordinary arm is an outer-parented `MatchArm` scope directly typed-owned by the source-backed Match `ExprId` or `StmtId` | `PASS` |
| `postcondition_result_is_ensures_only` | executed exact Flow contract test `item_lowering::tests::flow::postcondition_result_is_ensures_only`; synthetic `result` resolves in `ensures` and remains absent from `requires` and the body | `PASS` |
| `typed_child_beats_disagreeing_display_source` | executed source-freeze substitution/tamper tests across final lowering | `PASS` |
| `recovered_module_is_excluded_from_executable_caches` | executed compiler `recovered_module_never_enters_runtime_plan_or_compile_cache` and LSP post-HIR publication owner; recovered HIR remains tooling-visible while both executable admission and new/old signature caches are empty | `PASS` |
| HIR `module_limit_is_inclusive_and_atomic` | executed `database::tests::module_limit_and_identity_exhaustion_are_atomic` and final-project limit test in the complete HIR suite | `PASS` |
| HIR `item_limit_is_inclusive_and_atomic` | executed `arena::tests::every_typed_arena_enforces_its_exact_and_one_over_limit_atomically` | `PASS` |
| HIR `scope_limit_is_inclusive_and_atomic` | executed the same every-typed-arena production limit test | `PASS` |
| HIR `statement_limit_is_inclusive_and_atomic` | executed the same every-typed-arena production limit test | `PASS` |
| HIR `expression_limit_is_inclusive_and_atomic` | executed the same every-typed-arena production limit test; the wider Select total-slot Tier 2 row remains separate below | `PASS` |
| HIR `type_limit_is_inclusive_and_atomic` | executed the same every-typed-arena production limit test | `PASS` |
| HIR `pattern_limit_is_inclusive_and_atomic` | executed the same every-typed-arena production limit test | `PASS` |
| `local_module_limit_is_inclusive_and_atomic` | executed exact/one-over owner test `arena::tests::local_module_limit_is_inclusive_and_atomic` at 65,536/65,537 locals | `PASS` |
| `local_scope_limit_is_inclusive_and_atomic` | executed exact/one-over owner test `final_lowering::item_lowering::tests::extern_capability::local_scope_limit_is_inclusive_and_atomic` at 4,096/4,097 locals | `PASS` |
| `capture_limit_is_inclusive_and_atomic` | executed exact final expression-control lowering test; the backing-arena exact/one-over test remains part of the wider HIR rerun | `PASS` |
| `hir_diagnostic_limit_is_inclusive_and_atomic` | executed exact/one-over owner test `module::tests::hir_diagnostic_limit_is_inclusive_and_atomic` at 1,024/1,025 diagnostics | `PASS` |
| `synthetic_descendant_limit_is_inclusive_and_atomic` | executed exact final expression-control lowering test | `PASS` |
| `total_slot_limit_is_inclusive_and_atomic` | both production Tier-2 owners executed individually: the 786,432-slot exact case committed and the 786,433rd direct Select rolled back atomically | `PASS` |
| `module_identity_exhaustion_is_atomic` | executed `database::tests::module_limit_and_identity_exhaustion_are_atomic` | `PASS` |
| `revision_exhaustion_is_atomic` | executed `database::tests::revision_exhaustion_keeps_the_exact_current_lease` | `PASS` |
| `slot_identity_exhaustion_is_atomic` | executed exact generic owner test `slot::tests::slot_identity_exhaustion_is_atomic` for every typed HIR ID family | `PASS` |
| `local_generation_exhaustion_is_atomic` | executed exact final-lowering transaction test `pattern_lowering::tests::local_generation_exhaustion_is_atomic`; failure leaves the database state unchanged | `PASS` |
| `cache_epoch_exhaustion_is_atomic` | executed `database::tests::invalidation_epoch_exhaustion_keeps_the_exact_current_lease` | `PASS` |

### Match semantic ownership versus lexical scope

The absence of a container-wide Match `Block` does not make Match ownerless.
The source-backed Match `ExprId` or `StmtId` remains the semantic and
transaction owner of the scrutinee and every arm. Scrutinee name lookup uses
the inherited outer lexical scope, but its retained once-evaluation extent is
Match-owned through arm selection and the Match join; lexical lookup does not
select its drop lifetime. Each ordinary arm allocates one distinct
`HirScopeKind::MatchArm` whose lexical parent is that same outer scope and whose
typed owner is `HirScopeOwner::Expr(match_id)` or
`HirScopeOwner::Stmt(match_id)`. Sibling arms therefore share the Match
semantic owner without sharing bindings or forming a fictitious nested Block.
The ordered `HirMatchArm`/`HirStmtMatchArm` records retain each distinct
`ScopeId`; the owner relation is intentionally one-to-many, so no consumer may
assume that one `HirScopeOwner` identifies exactly one scope. Pattern bindings,
the guard, and the selected value/body use the arm scope, which supplies the
binding-visibility and cleanup boundary that a fabricated Match-level Block
would not improve.

An authored expression `Block` below an arm retains its own nested Block scope.
In Thread statement context, a braced arm instead uses one Block as both the arm
scope and the nested `HirThreadBody` owner; it does not allocate a parallel
`MatchArm`. Consumers that need semantic containment must follow the typed
scope owner, while lexical lookup follows `ScopeId::parent`. Treating the scope
tree alone as the semantic ownership graph would be the actual inconsistency.
No `DesugaredTemporary` or `ClosureEnvironment` decision may change these Match
scope invariants. Current structured Flow runtime and AWBC agree on once-
evaluation and arm binding isolation, but Flow AWBC stores the scrutinee in a
containing-frame temp without a Match-join `Drop`/`Clear`; the structured helper-
local value ends when selection returns. Expression AWBC Match instead exits a
synthetic control function through `Return` or `Trap`, so it needs frame-exit
release evidence rather than the same join-time fix. The dedicated
`MatchScrutinee` branch of the correction request must close both context-
specific lifetime obligations, either with a typed Match-owned child or a
codegen-local release recipe on every relevant exit. It must not obtain lifetime
by fabricating a container Block.

### Thread statement authority-switch evidence

The later ordinary-Flow correction is outside the base package's 157-row
count, but its Thread statement authority has now executed at all three direct
owners. Parser-owned `SyntaxKind::ThreadStatement`, its typed statement marker,
and its dedicated production remain deleted. The sole production chain is
`ExpressionStatement -> ThreadExpression -> ExprId -> HirExprKind::Thread`;
the HIR statement is ordinary `HirStmtKind::Expression`, and runtime-plan reads
that expression payload rather than a statement-specific Thread carrier.

| Direct owner | Executed exact evidence | Focused result |
|---|---|---|
| syntax | `parser::predicate_proof_tests::control_statements_and_thread_expression_own_typed_children` retains `ThreadExpression` under the ordinary statement owner and no `ThreadStatement` family; selected test passed 1/1 | `PASS` |
| final HIR | `final_lowering::item_lowering::tests::flow_transaction::empty_thread_expression_is_an_ordinary_flow_statement_with_an_expr_owned_body_scope` proves `HirStmtKind::Expression -> HirExprKind::Thread` and `HirScopeOwner::Expr`; selected test passed 1/1 | `PASS` |
| runtime-plan | `final_flow::tests::thread_expression_statement_lowers_through_the_sole_expression_owner` projects the typed Thread expression to one `FlowOp::Thread`; selected test passed 1/1 | `PASS` |

The ignored Tier 2 test
`thread_flow_items_exact_and_one_over_are_atomic_for_attached_thread_expression`
at the real 65,536/65,537 boundary was not executed and remains
`NOT_RUN_EXACT`. These three focused passes do not promote `T-LIMIT-26`, the
aggregate ordinary-Flow matrix, or any heavy boundary row to `PASS`.

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
| `one_symbol_table_registers_all_callable_kinds_and_character` | exact owner test executed: function, predicate, proof, and Character share one revision-bound table | `PASS` |
| `ordinary_callable_duplicate_names_are_reported_together` | exact owner test executed: project-symbol `DuplicateDeclaration { sites }` retains one source-ordered group before diagnostic sorting/truncation | `PASS` |
| `visibility_import_alias_and_qualification_are_uniform` | exact table-driven owner test executed: direct/group/glob/alias/qualified routes retain Function/Predicate/Proof owner identity; inaccessible and escalation diagnostics retain the exact consumer/facade revision; ambiguity retains all three typed owners | `PASS` |
| `symbol_table_revision_invalidates_exact_changed_modules` | exact three-unit compiler test executed with a body-stable `public` to `pub(crate)` declaration change: changed/dependent units miss, unrelated unit hits and retains its exact accepted HIR `Arc` | `PASS` |
| `proof_artifact_id_is_session_only_and_snapshot_bound` | exact owner test executed: a body-only reparse in one syntax/HIR session preserves declaration and source-backed `ItemId` while changing snapshot/artifact; a separate database yields a distinct artifact | `PASS` |
| `compiled_project_contains_no_linked_hir` | exact compiler test executed and compares every tested `CompiledProjectModule::hir()` with the matching accepted `HirProject` module by `Arc::ptr_eq`; the compile-fail fixture remains a separate public-API row below | `PASS` |
| `crates/arcweft-lang-hir/tests/ui/no_linked_module.rs` | exact fixture calls the deleted `HirProject::linked_module` signature; HIR public-API trybuild passed | `PASS` |
| `crates/arcweft-lang-hir/tests/ui/no_append_module_body.rs` | exact fixture calls the deleted `HirModule::append_module_body` signature; HIR public-API trybuild passed | `PASS` |
| `crates/arcweft-compiler/tests/ui/no_linked_hir.rs` | exact fixture passed as one of the eight registered `removed_compiler_apis_are_unavailable` trybuild cases | `PASS` |
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
avoids that prohibited presentation repair. The exact grouped-owner test passed
on the current coherent working copy.

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

### Executed exact Section 5 closure tests

The former partial Section 5 rows required no additional symbol authority,
cache, or compatibility reader. They were closed by these executable
cross-owner tests:

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
  the symbol-owner boundary. The executed test additionally checks exact
  diagnostic source revisions and all three ambiguity candidate owners.
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
  or source-name gate would be a prohibited parallel model. The executed change
  is visibility-only, so a body fingerprint cannot accidentally satisfy the
  symbol invalidation row.

Both exact tests passed. The neighboring exact symbol-table, duplicate-name,
Proof-artifact, and shared-HIR-lease tests also passed in the same validation
session. The compiler public-API compile-fail fixture remains independently
tracked above and is not inferred from these runtime tests.

### Package-chain canonical Flow identity consumer evidence

The following evidence belongs to the later ordinary-Flow correction and is
outside the base package's 157-row count. It is mapped here because the public
switch now carries one checked Flow identity through runtime publication and
persistence. A `PASS` in this table applies only to the named focused test; it
does not promote the aggregate `T-MIG-*` row or the 306-row Flow matrix to
package-wide PASS.

| Later contract mapping | Concrete evidence and exact behavior | Current focused result |
|---|---|---|
| `T-ID-09`, `T-PROJ-07`, and the identity input to `T-MIG-04` | `arcweft-core/tests/runtime_id_boundaries.rs::{checked_flow_identity_is_one_way_and_keeps_public_label_separate,dynamic_flow_target_selects_one_accepted_identity_or_reports_label_ambiguity}` prove that equal public labels do not collapse distinct digest-derived identities and that dynamic public-label lookup reports ambiguity; the complete focused integration binary passed 11/11 | `PASS` |
| `T-MIG-04` verifier/runtime parity and canonical AWBC persistence | `arcweft-core::awbc::tests::{canonical_codec_round_trips_checked_flow_identity_and_public_label,canonical_flow_bindings_preserve_same_label_declarations_and_reject_ambiguous_targets}` cover the canonical/public split, exact Flow-to-function bijection, verifier admission, codec round trip, ambiguous public lookup, missing lookup, and rejection of a canonical label as authored input; the complete `awbc::tests` focused set passed 31/31 | `PASS` |
| `T-MIG-04` live product-state identity | `arcweft-core::awbc::product_step::tests::snapshot_restore_and_hot_swap_require_exact_semantic_flow_identity` rejects same-label replacement during hot swap and snapshot restore without mutating the accepted executor | `PASS` |
| `T-MIG-04` active-choice target identity | `arcweft-core::awbc::product_step::tests::snapshot_restore_rejects_same_label_choice_target_substitution` rejects an active choice whose target was replaced by a distinct same-label Flow and retains the original exact target | `PASS` |
| `T-MIG-04` plus the save/restore portion of `T-MIG-06` | `arcweft-runtime-driver/tests/awbc_product_session.rs::awbc_product_session_save_preserves_exact_same_label_flow_identity` round-trips the exact live binding through AWFB session save/import, produces identical re-export bytes, rejects same-label substitution, and leaves the session unchanged; a standalone replay-trace identity row is not proved by this test | `PASS` |
| `T-MIG-04` bundle-patch consumer | `arcweft-bundle::patch::tests::{awbc_patch_fingerprints_keep_same_label_checked_flows_distinct,awbc_patch_classifies_each_same_label_flow_independently}` key fingerprints by canonical Flow identity and independently classify body, interface, and removal changes | `PASS` |
| `T-MIG-04` runtime-driver generation consumer | `arcweft-runtime-driver::swap::tests::{generation_from_bundle_uses_product_awbc_function_identity,product_awbc_code_slots_keep_same_label_flow_declarations_distinct}` derive code slots from exact AWBC Flow/function bindings and retain two canonical slots for two same-label declarations | `PASS` |
| `T-MIG-06` and consumer inventory `U050..U055` Agent graph projection | `arcweft-agent-runner::tests::{controller_bytecode_resumes_project_graph_neighborhood_fields,project_graph_neighborhood_uses_exact_symbol_identity_when_public_labels_match}` preserve graph fields and select the exact structural symbol while refusing a public-label-shaped surrogate key | `PASS` |
| `T-MIG-05` LSP typed projection | `arcweft-lsp::features::entry_roles::tests::lsp_navigation_uses_typed_syntax_and_module_hir_ids` is the exact typed ParsedSource/module-HIR/project-lease navigation owner; the complete `entry_roles` focused set passed 10/10 after the public-switch and executable-symbol publication correction. It does not by itself promote every Flow-specific consumer row | `PASS` |

### Package-chain Call accounting and verifier-witness evidence

These rows belong to the later repository-adjudicated Call correction and are
outside the base package's 157-row inventory. The precedence-aware row state is
owned by the [package-chain rollup](2026-08-07-proof-concurrency-v6-1-1-package-chain-matrix-rollup.md).
The focused passes below do not promote the other active Call rows or the full
rollback/retry transaction matrix.

| Later contract mapping | Executed exact behavior | Current focused result |
|---|---|---|
| `A-011`, `T-RB-12-004`, `T-LIM-12-008` | `t_lim_12_008_and_t_rb_12_004_candidate_one_over_rolls_back_publication` proves candidate 257 enters the resolver once, performs zero probes/replays/publications, returns `CandidateLimit`, and publishes no report, facts, result, retained accounting carrier, or physical trace | `PASS` |
| `A-012`, `T-LIM-12-009` | final sema retains the complete two-candidate set with two probes and one selected replay; verifier projection retains 2 witnesses with 0 omitted | `PASS` |
| `A-013`, `T-LIM-12-010` | `call_adj_a_013_three_candidate_semantic_facts_remain_complete` retains all three semantic candidates with three probes and one selected replay; verifier projection retains 2 witnesses with 1 omitted | `PASS` |
| `A-006`, `T-RES-12-004`, complete-considered-set normalization | `ambiguous_call_retains_complete_considered_set_beyond_the_tied_subset` preserves two tied candidates separately from all three considered candidates, probes all three, and performs no replay | `PASS` |
| Proof-witness portion of `T-RB-12-007` plus normalized conflict precedence | verifier projection is identical on retry; after the primary, the first distinct ambiguity conflict precedes an earlier rejected-but-considered candidate. This does not prove the broader rollback transaction row | `PASS` |

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
| `lsp_navigation_uses_typed_syntax_and_module_hir_ids` — `arcweft-lsp` | exact LSP owner resolves function, predicate, and proof navigation through the compiler-retained ParsedSource, module-qualified HIR IDs, and one accepted `Arc<HirProject>` across two imported modules; the complete `entry_roles` focused set passed 10/10 after the current public-switch changes | `PASS` |
| `cli_diagnostics_render_exact_revision_spans` — `arcweft-cli` | `cargo test -p arcweft-cli --lib --all-features app::diagnostics::tests::cli_diagnostics_render_exact_revision_spans -- --exact --nocapture` passed 1/1 and exercises the CLI, LSP, and Agent projections plus explicit stale-revision rejection from one accepted typed diagnostic | `PASS` |
| `agent_runtime_assertion_projection_uses_session_capability` — `arcweft-cli`/tooling | `cargo test -p arcweft-cli --lib --all-features app::agent::native::player_observation::tests::native_agent_runtime_assertion_projection_uses_compiled_session_capability -- --exact --nocapture` passed 1/1 after compiling and running an authored false assertion. The native Agent projects the typed failure through the exact compiled session inventory; the observed typed message is retained without using it to rediscover the guard/site, and the persisted-only fallback remains separate | `PASS` |
| `verifier_consumes_predicate_proof_arena_records` — `arcweft-verify` | executed exact compiler integration test resolves final Predicate/Proof expression IDs through final analysis and binds the verifier's typed proof artifact/obligation to the originating Proof item | `PASS` |
| `runtime_plan_consumes_project_view_without_flattening` — runtime-plan/compiler | exact compiler integration test passed and compares three module paths, module-qualified item/Flow identities, and runtime-plan inventory through `HirProjectExecutableView`; `compiled_project_contains_no_linked_hir` retains the accepted module `Arc`, while the eight-fixture compiler trybuild harness proves the flattened reader remains deleted | `PASS` |
| `malformed_removed_form_does_not_hide_following_current_declarations` — syntax/HIR integration | `final_lowering::item_lowering::tests::removed_form_recovery::ordinary_removed_form_recovery_keeps_following_final_hir_queryable_until_repaired` passed 1/1 and executes the complete four malformed forms × three following current callable families. It proves ordinary recovery, clean source-backed following HIR, non-executable recovered publication, stable repaired `ItemId`, and executable repaired publication without a removed-spelling recognizer | `PASS` |
| `recovered_module_never_enters_runtime_plan_or_compile_cache` — compiler/tooling | executed exact compiler transaction test rejects runtime-plan/cache publication; exact LSP publication test publishes a new tooling-only generation, clears old/new request caches, and retains no executable payload | `PASS` |
| `public_api_surface_contains_only_final_nodes` — compile-fail suites | the complete accepted compile-fail inventory executed at its actual owners: five syntax fixtures reject detached fragments, raw construction, and session Serde; five HIR fixtures reject linked/append/provisional Proof APIs, raw IDs, and session Serde; the compiler fixture rejects `CompiledProject::linked_hir`; and the four runtime-plan/core fixtures reject public fault construction, session Serde, runtime `Prove`, and a core-to-HIR type edge | `PASS` |

## Validation and remaining closure

### Evidence already obtained on this working copy

- A read-only table inventory found exactly 157 mapped row occurrences and
  153 unique identities. The only duplicates are the four intentional
  syntax/HIR limit rows (`statement`, `expression`, `type`, and `pattern`). An
  exact multiset comparison with the accepted ZIP's `TEST_MATRIX.md` found
  zero missing, added, or multiplicity-mismatched identities.
- The syntax deletion migration reached 690/690 library tests on the current
  working copy after the terminal-newline and bounded-expression-transaction
  corrections. The three public-parser-authority integration tests and the
  public API trybuild harness also passed. Rows above remain `PARTIAL` where
  those suites do not contain the complete contract-shaped matrix.
- The final coherent-copy HIR library suite reached 841 passed, zero failed,
  and eight explicitly ignored Tier-2 rows in 268.13 seconds. All eight ignored
  production-boundary owners were executed separately and passed; in
  particular the Flow 65,536/65,537 transaction owner passed in 85.23 seconds
  on the final copy.
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
- `cargo test -p arcweft-compiler --test api_compile --all-features
  removed_compiler_apis_are_unavailable -- --exact --nocapture` passed its one
  harness and all eight registered fixtures, including `no_linked_hir.rs`.
  `compiled_project_contains_no_linked_hir` and
  `runtime_plan_consumes_project_view_without_flattening` then each passed 1/1
  in `project_cache_transaction`, proving exact accepted-module leases and the
  module-preserving runtime-plan view without a flattened compatibility owner.
- `cargo test -p arcweft-core --test runtime_id_boundaries` passed 11/11;
  `cargo test -p arcweft-core awbc::tests --lib` passed 31/31; and
  `cargo test -p arcweft-core awbc::product_step --lib` passed the current
  19/19 suite. These runs include the exact same-label identity, canonical
  codec/verifier, hot-swap, snapshot, and active-choice substitution rows.
- `cargo test -p arcweft-runtime-driver --test awbc_product_session` passed
  21/21, including exact save/import/substitution. The `same_label_flow`
  filtered library run passed 3/3, including canonical-slot preservation and
  rejected live binding exchange without generation or state mutation.
- Focused `arcweft-bundle` execution passed both exact patch tests; the product
  AWBC-only, catalog-resource codec, and style cross-section integration
  binaries also passed 4/4, 5/5, and 7/7 respectively with canonical Flow
  bindings.
- Focused `arcweft-agent-runner` execution passed
  `project_graph_neighborhood_uses_exact_symbol_identity_when_public_labels_match`
  and `controller_bytecode_resumes_project_graph_neighborhood_fields` (2/2).
- The final-copy `arcweft-lsp` `entry_roles` focused set passed 10/10 after the
  accepted symbol lease retained non-executable callable identities until final
  executable publication.
- Match ownership evidence passed at each current owner: the HIR `match_`
  filtered set passed 25/25, core runtime scrutinee-once/rejected-binding
  cleanup passed 1/1, and AWBC product parity for once-evaluation and arm
  scoping passed 1/1. This does not yet promote the aggregate ordinary-Flow F06
  row whose remaining container/source assertions are tracked below.
- `cargo test -p arcweft-lang-sema --lib final_analysis::tests:: --
  --nocapture` passed 62/62, including the exact normalized Call A-012/A-013,
  complete ambiguous considered-set, and candidate-257 rollback owners.
- `cargo check -p arcweft-verify --all-features` passed with 71 existing sema
  `dead_code` warnings. `cargo test -p arcweft-verify --lib
  call_witness::tests:: -- --nocapture` then passed 4/4, covering witness
  2-to-2/0, 3-to-2/1, deterministic retry projection, and ambiguity-conflict
  precedence. These are later Call-correction results, not additional base-row
  PASS claims.
- Focused core, runtime-host, and runtime-driver assertion materialization and
  transport tests passed before the later CLI/Agent publication edits. They
  remain useful behavioral evidence, but are not substituted for the exact
  package rows or for final-copy validation.
- `rustfmt` completed for the direct CLI, serve, native Agent, and controller
  assertion-publication files.
- `git diff --check` passed for those publication files.

### Final coherent-copy validation

The final implementable-scope validation ran on 2026-08-09 from Git HEAD
`80331c81e338d20e968a10947d5e848c39610384` with the public-switch working
copy dirty:

- `cargo test -p arcweft-lang-syntax --lib --all-features` passed 690/690;
- `cargo test -p arcweft-lang-syntax --test public_api --all-features`
  passed its harness and all 23 registered compile-fail fixtures;
- `cargo test -p arcweft-lang-hir --lib --all-features` passed 841/841
  executed tests with eight Tier-2 tests ignored by the ordinary harness;
- all eight applicable real production-boundary Tier-2 owners passed when
  executed individually, including syntax/HIR expression, source bytes,
  diagnostics, Flow, Thread, total slots, and rollback boundaries;
- `cargo test -p arcweft-lang-hir --test public_api --all-features` passed its
  harness and all 37 registered compile-fail fixtures;
- the final sema library suite passed 163/163;
- LSP `entry_roles` passed 10/10 and the current AW-AH-009.3 cache/state/
  request/position matrix passed 7/7, 12/12, 42/42, 50/50, and 8/8;
- runtime-plan assertion identity passed 10/10; core assertion identity passed
  2/2; both runtime-plan/core compile-fail harnesses passed; tooling assertion
  projection passed 2/2; and compiler artifact reload passed 1/1;
- runtime-driver `awbc_product_session` passed 21/21, including exact
  save/import/substitution identity;
- `cargo check --workspace --all-targets --all-features` passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  passed; and
- `just structure-audit` and `just structure-audit-gate` passed with zero
  blocking violations.

The eight Character-definition failures in the complete LSP suite are not
Proof failures: their valid `show(...)` fixtures reach the final compiler and
fail closed because the typed Presentation command ABI is not yet published.
That ABI is owned by the unreturned
[`AW-AH-011/013` request](../reviews/requests/2026-07-14-aw-ah-011-and-013-typed-presentation-command-abi.md),
not by the returned CharacterDialogue package. The fixtures are not weakened
and no string runtime path or Presentation shim is introduced here.

### Explicit external-design exclusions

The `DesugaredTemporary` production recipe, `ClosureEnvironment`
payload/consumer, and typed Presentation command ABI remain excluded because
their external contracts have not returned. No role, tag, fingerprint vector,
source gate, compatibility alias, dual reader, string runtime path, or shim is
inferred while those external designs are pending.
