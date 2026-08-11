# Mandatory direct test matrix

Every name below is normative. Tests use public or crate-owned typed behavior. None is a source-spelling/file-placement gate.

| # | Exact test name | Owner/layer | Required assertion |
|---:|---|---|---|
| 1 | `dialogue_application_bracket_and_colon_share_semantic_kind_with_distinct_surfaces` | syntax + HIR | Bracket and colon lower to the same dialogue-application HIR kind while source roles remain form-exact. |
| 2 | `colon_inline_lf_ranges_are_exact` | syntax | Inline LF content, separator, line ending, head, colon, and full ranges are exact. |
| 3 | `colon_inline_crlf_ranges_are_exact` | syntax | CRLF is one two-byte line-ending object and is excluded from semantic content. |
| 4 | `colon_indented_blank_comment_unicode_ranges_are_exact` | syntax | Raw body retains blank/comment/Unicode bytes and semantic content bounds the first/last content token. |
| 5 | `colon_tabs_count_as_one_source_byte_unit` | syntax | Each tab contributes one indentation byte unit with no tab-stop expansion. |
| 6 | `colon_mixed_space_tab_indentation_is_byte_counted` | syntax | Mixed prefixes are valid, byte-counted, and preserved exactly. |
| 7 | `colon_misaligned_line_is_retained_and_poisoned` | syntax + HIR | A head<n<base line remains content, records an issue, and poisons the application. |
| 8 | `colon_leading_blank_and_comment_lines_do_not_select_base` | syntax | Only the first meaningful wider line establishes base. |
| 9 | `colon_trailing_trivia_is_retained_without_semantic_text` | syntax | Trailing blank/comment trivia remains in body but not semantic content. |
| 10 | `nested_target_missing_paren_inserts_at_open_bracket` | syntax | Existing call recovery inserts `)` at `[` and keeps the complete CallExpr target. |
| 11 | `postfix_missing_close_inserts_at_owner_boundary` | syntax | Missing `]` records the exact typed boundary and generic diagnostic. |
| 12 | `empty_bracket_retains_poisoned_dialogue_application` | syntax + HIR | Empty payload selects recovered missing dialogue content when index is not viable. |
| 13 | `colon_missing_body_retains_poisoned_application` | syntax + HIR | No colon body produces Missing indentation/content and tooling HIR only. |
| 14 | `with_colon_and_with_block_attach_equivalently` | syntax + HIR | Both plan spellings attach to the same semantic plan shape with distinct source ranges. |
| 15 | `following_statement_with_after_blank_line_is_not_attached` | syntax | An intervening blank line leaves `with` to the following statement owner. |
| 16 | `misaligned_with_is_not_attached` | syntax | A `with` prefix unequal to the head prefix is not stolen. |
| 17 | `items_numeric_payload_retains_index_candidate` | syntax | `items[0]` is a clean index candidate and no dialogue atom is fabricated. |
| 18 | `expression_start_bracket_sequence_is_not_postfix` | syntax | Prefix `[` keeps the existing collection CST/AST. |
| 19 | `dialogue_controls_produce_dialogue_candidate` | syntax | Existing control syntax yields a dialogue candidate without name lookup. |
| 20 | `rich_text_produces_dialogue_candidate` | syntax | Existing RichText syntax yields a dialogue candidate. |
| 21 | `interpolation_produces_dialogue_candidate` | syntax | Existing interpolation yields a dialogue candidate and typed child expression. |
| 22 | `exactly_ambiguous_postfix_is_retained_without_name_lookup` | syntax | A payload accepted by both grammars yields exactly two candidates regardless of callee spelling. |
| 23 | `nested_call_target_is_complete_call_expr` | syntax | `foo()[content]` owns the full existing call target. |
| 24 | `record_literal_call_argument_is_not_postfix_reclassified` | syntax | A record literal inside a call argument remains ordinary expression syntax. |
| 25 | `bare_block_without_with_is_not_plan` | syntax | A following bare block never attaches. |
| 26 | `comments_and_trivia_round_trip_from_source_document` | syntax | All lossless trivia remains accessible from the source document/CST. |
| 27 | `malformed_with_colon_attaches_recovered_plan` | syntax + HIR | A valid `with:` prefix attaches a recovered plan and poisons the app. |
| 28 | `malformed_with_brace_attaches_recovered_plan` | syntax + HIR | A valid `with {` prefix attaches a recovered plan and poisons the app. |
| 29 | `candidate_classification_ignores_callee_spelling` | syntax | Renaming the target without changing tokens/roles does not change candidate classification. |
| 30 | `colon_surface_has_no_bracket_roles` | syntax + HIR | Open/close bracket roles are absent and colon role is present. |
| 31 | `postfix_payload_tokens_have_one_cst_owner` | syntax | Every interior token/trivia has exactly one payload CST owner. |
| 32 | `postfix_bracket_reconciles_by_root_and_child_roles` | syntax | Incremental identity follows accepted root/target/payload roles, not ranges. |
| 33 | `dialogue_missing_content_uses_insertion_not_fake_range` | syntax + HIR | Missing content maps to insertion offset/site only. |
| 34 | `non_ascii_whitespace_is_content_not_indentation` | syntax | Only ASCII space/tab enter indentation prefixes. |
| 35 | `blank_comment_only_indented_body_is_missing_content` | syntax + HIR | Trivia-only body becomes Missing, retained, and poisoned. |
| 36 | `plan_keyword_inside_interpolation_is_not_attached` | syntax | Nested interpolation `with` remains content. |
| 37 | `plan_keyword_inside_rich_text_is_not_attached` | syntax | RichText-contained `with` remains content. |
| 38 | `dialogue_target_call_retains_argument_list_syntax` | syntax | Target call exposes the exact existing ArgumentListSyntax. |
| 39 | `ordinary_call_argument_shape_contract_is_unchanged` | syntax | Named/positional/spread/nested/trailing/recovered arguments retain AW-AH-009.3.1 behavior. |
| 40 | `call_argument_absolute_id_ref_is_typed` | syntax + HIR | Absolute coordinate value remains `IdRef::Absolute` / `HirIdRef::Absolute`. |
| 41 | `call_argument_relative_id_ref_is_typed` | syntax + HIR | Unqualified relative coordinate remains `Relative`. |
| 42 | `call_argument_family_relative_id_ref_is_typed` | syntax + HIR | Family-relative coordinate remains `FamilyRelative`. |
| 43 | `runtime_expression_coordinate_remains_runtime_expression` | HIR + sema | A non-IdRef value remains its ExprId and is classified Runtime without source access. |
| 44 | `id_ref_binary_operand_does_not_fabricate_coordinate_id` | HIR + sema | An expression containing an IdRef but rooted at binary expression is Runtime. |
| 45 | `duplicate_id_coordinates_preserve_ordinal_order` | HIR + sema | Every duplicate id coordinate survives in authored ordinal order. |
| 46 | `duplicate_text_key_coordinates_preserve_ordinal_order` | HIR + sema | Every duplicate text_key coordinate survives in authored ordinal order. |
| 47 | `malformed_coordinate_retains_value_site_and_poison` | syntax + HIR | Recovered named value retains exact value site/error ExprId and poison. |
| 48 | `nested_target_call_coordinates_are_not_recursively_collected` | HIR | Only immediate outer CallExpr named arguments contribute. |
| 49 | `positional_id_ref_is_not_a_coordinate` | HIR | A positional IdRef remains an ordinary argument. |
| 50 | `callback_block_call_surface_is_unchanged` | syntax | Existing callback-block surface and ranges are unchanged. |
| 51 | `recovered_call_argument_terminator_is_unchanged` | syntax | Existing missing-`)` terminator kind/insertion remains exact. |
| 52 | `bracket_application_allocates_source_backed_expr_id` | HIR | Root ExprId key uses bracket SyntaxNodeId and HirIdKind::Expr. |
| 53 | `colon_application_allocates_source_backed_expr_id` | HIR | Root ExprId key uses colon application SyntaxNodeId. |
| 54 | `target_and_nested_call_use_expr_id_links` | HIR | Payloads link target/call children by ExprId, not syntax clones. |
| 55 | `dialogue_application_in_flow_uses_flow_scope` | HIR | Flow occurrence uses current Flow/body scope. |
| 56 | `dialogue_application_in_function_uses_callable_scope` | HIR | Function occurrence uses current Callable/body scope. |
| 57 | `dialogue_application_in_closure_uses_closure_scope` | HIR | Closure occurrence uses Closure scope and normal capture rules. |
| 58 | `dialogue_application_in_branch_uses_conditional_scope` | HIR | Branch occurrence uses existing Conditional/MatchArm scope. |
| 59 | `dialogue_application_in_block_uses_block_scope` | HIR | Block occurrence uses existing Block scope. |
| 60 | `dialogue_application_in_expression_statement_uses_owner_scope` | HIR | Expression statement does not create a special dialogue scope. |
| 61 | `unresolved_postfix_retains_two_candidates` | HIR | Ambiguous HIR stores exactly index/dialogue candidate ExprIds sharing target. |
| 62 | `invalid_postfix_retains_two_failure_summaries` | HIR | Invalid HIR stores exactly two typed failures and no fabricated candidate IDs. |
| 63 | `ambiguous_candidate_only_expr_ids_are_deterministic_synthetic_children` | HIR | Candidate IDs use root owner, interpretation role, deterministic preorder. |
| 64 | `source_lookup_returns_application_target_delimiters_content_plan` | HIR | All required present source roles return exact revision-bound sites. |
| 65 | `source_lookup_missing_close_returns_insertion_site` | HIR | CloseBracket returns typed insertion for recovered missing close. |
| 66 | `source_lookup_colon_roles_exclude_brackets` | HIR | Colon returns colon/content roles and absent bracket roles. |
| 67 | `source_lookup_coordinate_parts_by_ordinal` | HIR | Whole/name/value sites resolve by checked argument ordinal. |
| 68 | `recovered_application_commits_tooling_snapshot_not_executable` | HIR + tooling | Recovered module is queryable but rejected by executable paths. |
| 69 | `ambiguous_application_requires_resolution_before_execution` | sema | Checked executable view rejects missing postfix resolution. |
| 70 | `source_revision_and_document_identity_are_authoritative` | HIR | Every source site matches root document/revision; display strings are irrelevant. |
| 71 | `dialogue_hir_contains_no_authored_expr_clone` | HIR compile/API | Public payload construction cannot store syntax AuthoredExpr/Expr. |
| 72 | `lowering_uses_no_source_reparse` | HIR behavior | Typed lowering succeeds from attachments after source text display is unavailable as authority. |
| 73 | `callable_and_flow_share_one_expression_arena` | HIR | Both owners return ExprIds from the same module arena. |
| 74 | `interpolation_expr_ids_use_current_scope` | HIR | Dialogue interpolation IDs retain current lexical scope. |
| 75 | `plan_expr_ids_use_same_module_arenas` | HIR | Plan expressions/statements/patterns use existing module arenas. |
| 76 | `stale_expr_id_lookup_returns_typed_error` | HIR | Revision/liveness mismatch returns existing typed ID error. |
| 77 | `root_source_key_uses_syntax_node_id_not_range` | HIR | Moving range with retained syntax ID retains root ID under accepted reconciliation. |
| 78 | `semantic_resolution_does_not_reallocate_candidate_ids` | HIR + sema | Sema resolution records a fact and leaves immutable IDs unchanged. |
| 79 | `immediate_target_coordinates_are_preserved_in_hir` | HIR | Coordinates reflect only immediate target call and keep duplicates/order. |
| 80 | `hir_entity_reference_retains_three_idref_forms` | HIR | All three `HirIdRef` variants round-trip typed syntax identity. |
| 81 | `postfix_bracket_runs_exactly_two_candidate_parsers` | syntax limits | Instrumentation observes two attempts and no third interpretation. |
| 82 | `postfix_candidate_diagnostics_charge_global_limit` | syntax limits | Candidate diagnostics consume existing global cap. |
| 83 | `syntax_limit_one_over_discards_document_transaction` | syntax atomicity | Exact budget succeeds; one-over publishes no partial document. |
| 84 | `hir_expression_limit_one_over_discards_lowering_transaction` | HIR atomicity | Exact arena limit succeeds; one-over publishes no snapshot/IDs. |
| 85 | `checked_range_projection_overflow_is_internal_failure` | syntax + HIR | Overflow returns typed internal failure, not user diagnostic/recovery. |
| 86 | `failed_hir_lowering_publishes_no_component_source_roles` | HIR atomicity | Rollback leaves source component map unchanged. |
| 87 | `postfix_candidate_storage_is_bounded_to_two` | syntax + HIR | Public constructors reject any shape other than the exact two-result enum. |
| 88 | `indentation_issue_diagnostics_charge_existing_budget` | syntax limits | Misalignment issues cannot exceed existing diagnostic budget. |
| 89 | `exact_limit_succeeds_and_one_over_fails` | syntax + HIR limits | All relevant existing caps obey inclusive exact/one-over behavior. |
| 90 | `malformed_constructor_relations_are_internal_not_user_diagnostics` | syntax + HIR API | Checked constructors return invariant failures without publishing diagnostics. |
| 91 | `sema_consumes_typed_dialogue_application_without_flow_sidecar` | sema | Checker accepts the same HIR expression in Flow and callable bodies. |
| 92 | `verifier_rejects_unresolved_or_recovered_application` | verify | Verifier requires checked clean resolution. |
| 93 | `runtime_plan_rejects_nonexecutable_hir_snapshot` | runtime-plan | Runtime-plan cannot consume recovered/unresolved project state. |
| 94 | `lsp_reuses_accepted_project_snapshot_for_dialogue_source_roles` | LSP | LSP queries the accepted Arc<HirProject> and never reparses. |
| 95 | `tooling_reads_component_source_map_without_source_search` | tooling | Tooling obtains every site through typed role lookup. |
| 96 | `compile_fail_removed_dialogue_ast_and_hir_types_are_absent` | compile-fail | Removed public old types/variants/constructors fail to compile. |
| 97 | `public_parser_paths_have_no_removed_syntax_mode` | public API behavior | No parser mode/feature API accepts old `.say`/speaker syntax; normal generic recovery remains. |
| 98 | `ordinary_call_regression_suite_remains_green` | regression | AW-AH-009.3.1 call suite passes unchanged. |
| 99 | `character_dialogue_cut1_domain_suite_remains_green` | regression | Cut 1 domain/runtime suite passes unchanged. |
| 100 | `workspace_quality_gates_pass_after_direct_replacement` | workspace | Format, targeted tests, workspace check, clippy -D warnings, and just verify pass. |

## Execution grouping

- 1–37: syntax, ranges, recovery, CST ownership, indentation, and plan attachment.
- 38–51: ordinary-call composition and typed ID coordinates.
- 52–80: HIR identity, arena ownership, scopes, source map, poison, and typed ID forms.
- 81–90: limits, checked arithmetic, construction, and atomicity.
- 91–100: downstream consumers, deletion/public API, regressions, and workspace quality.

The implementation may place tests in the repository-owned modules that expose the behavior, but it may not rename, merge, weaken, or replace them with source scans.
