# Test matrix

All rows are normative. Tests use typed/public or crate-owned APIs, negative and
transactional behavior, exact SourceSpan identity, compile-fail contracts, Cargo
metadata, and the canonical structural audit. No row authorizes a repository
source scan, file-placement assertion, CSS/Takumi route, compatibility alias, or
removed-spelling recognizer.

| ID | Exact test name | Primary owner | Required assertion |
|---|---|---|---|
| TM-001 | `dialogue_line_id_accepts_exact_say_family` | arcweft-id unit | `say.opening.greeting` constructs and round-trips exactly. |
| TM-002 | `dialogue_line_id_rejects_line_alias_family` | arcweft-id unit | `line.opening.greeting` is rejected; only source family `say` is valid. |
| TM-003 | `dialogue_text_key_accepts_exact_text_family` | arcweft-id unit | `text.opening.greeting` constructs and round-trips exactly. |
| TM-004 | `dialogue_line_id_accepts_exact_256_utf8_bytes` | arcweft-id unit | A valid `say.*` value of exactly 256 UTF-8 bytes succeeds. |
| TM-005 | `dialogue_line_id_rejects_257_utf8_bytes` | arcweft-id unit | The same value at 257 bytes fails with the typed byte-limit error. |
| TM-006 | `character_dialogue_limit_uses_lower_dialogue_identity_constant` | cross-crate integration | Cut 1 `max_line_id_bytes` equals 256 from the lower identity owner without changing its public field. |
| TM-007 | `dialogue_identity_raw_fields_and_serde_are_unavailable` | arcweft-id trybuild | Tuple/raw construction and Serde derive/use fail to compile; checked codecs remain required. |
| TM-008 | `runtime_line_id_conversion_discards_only_the_say_family` | runtime-plan unit | Checked conversion produces the path tail and has no Character/callee input. |
| TM-009 | `flow_owner_frozen_example_generates_second_id` | HIR candidate unit | `flow.game.intro` + `scene/greeting` + generated #2 equals `say.flow.game.intro.scene.greeting.002`. |
| TM-010 | `callable_owner_frozen_example_generates_first_id` | HIR candidate unit | Package `game`, module `game.dialogue`, Function `phone_line`, scope `retry` equals the frozen callable ID. |
| TM-011 | `callable_owner_prefix_retains_every_typed_owner_path_segment` | HIR candidate unit | Nested owner path segments appear in authored typed order between owner family and callable name. |
| TM-012 | `callable_root_module_prefix_omits_crate_display_word` | HIR candidate unit | A crate-root callable includes package/owner/name but no literal `crate` segment. |
| TM-013 | `callable_owner_package_module_mismatch_is_fatal` | HIR lowering unit | A `CallableDeclarationId` not matching `HirModuleKey` aborts the module transaction with AW-CD-026 projection. |
| TM-014 | `named_scopes_contribute_outermost_to_innermost` | HIR candidate unit | Typed named scope segments append in lexical order. |
| TM-015 | `unnamed_scopes_do_not_contribute_identity_segments` | HIR candidate unit | Additional unnamed ScopeIds leave the generated prefix unchanged. |
| TM-016 | `generated_ordinal_one_formats_001` | arcweft-lang-hir unit | Ordinal 1 formats `001`. |
| TM-017 | `generated_ordinal_two_formats_002` | arcweft-lang-hir unit | Ordinal 2 formats `002`. |
| TM-018 | `generated_ordinal_999_formats_999` | arcweft-lang-hir unit | Ordinal 999 formats `999`. |
| TM-019 | `generated_ordinal_1000_formats_without_truncation` | arcweft-lang-hir unit | Ordinal 1,000 formats `1000`. |
| TM-020 | `generated_ordinal_production_maximum_formats_exactly` | arcweft-lang-hir unit | Ordinal 262,144 formats `262144` and remains within policy. |
| TM-021 | `generated_counters_are_independent_per_exact_prefix` | HIR candidate unit | Two distinct owner/scope prefixes each begin at `001`. |
| TM-022 | `explicit_id_does_not_consume_generated_ordinal` | HIR candidate unit | Explicit line between generated sites leaves generated outputs `001`, `002`. |
| TM-023 | `failed_candidate_does_not_consume_generated_ordinal` | HIR candidate unit | A wrong-family/dynamic/oversized candidate between generated sites leaves later ordinal unchanged. |
| TM-024 | `recovered_application_does_not_consume_generated_ordinal` | HIR candidate unit | Poisoned application produces no candidate and no counter mutation. |
| TM-025 | `character_and_display_renames_do_not_change_generated_id` | HIR candidate integration | Changing CharacterId alias/display metadata while owner/scopes stay fixed yields equal candidate IDs. |
| TM-026 | `callee_local_alias_and_source_spelling_do_not_change_generated_id` | HIR candidate integration | Equivalent typed owner/application under different local/callee/source spellings yields equal ID. |
| TM-027 | `relative_and_say_family_relative_resolve_equally` | HIR candidate unit | `@.greeting` and `@say:.greeting` resolve to the same final line ID. |
| TM-028 | `relative_origins_remain_distinct_provenance` | HIR candidate unit | The equal IDs retain `ExplicitRelative` versus `ExplicitFamilyRelative` origin. |
| TM-029 | `super_removes_exactly_one_named_scope` | HIR candidate unit | Parent depth one removes only the innermost named scope. |
| TM-030 | `repeated_super_removes_requested_named_scopes` | HIR candidate unit | Parent depth N removes N named scopes and preserves owner components. |
| TM-031 | `relative_traversal_cannot_escape_source_owner` | HIR candidate unit | Depth above available scopes yields AW-CD-022 at the authored ID span. |
| TM-032 | `relative_traversal_from_owner_root_is_rejected` | HIR candidate unit | Any parent request with zero named scopes yields AW-CD-022. |
| TM-033 | `absolute_say_id_is_preserved_exactly` | HIR candidate unit | A valid absolute `@say.opening.greeting` retains exact case and bytes. |
| TM-034 | `wrong_absolute_line_family_reports_aw_cd_013` | HIR diagnostic unit | An absolute non-`say` family reports AW-CD-013 at the exact value span. |
| TM-035 | `wrong_family_relative_line_id_reports_aw_cd_013` | HIR diagnostic unit | `@text:.greeting` in `id` reports AW-CD-013 at the exact value span. |
| TM-036 | `ownerless_generated_application_reports_aw_cd_021` | HIR candidate unit | No `id` outside flow/callable reports missing owner and produces no candidate. |
| TM-037 | `ownerless_relative_application_reports_aw_cd_021` | HIR candidate unit | Ownerless relative/family-relative line ID is rejected. |
| TM-038 | `ownerless_absolute_say_id_succeeds` | HIR candidate unit | Ownerless application with valid absolute `@say.*` produces a candidate. |
| TM-039 | `dynamic_id_expression_is_not_reparsed` | HIR candidate unit | A clean non-HirIdRef coordinate reports AW-CD-023 and no source read occurs. |
| TM-040 | `duplicate_id_coordinates_report_aw_cd_027` | HIR diagnostic unit | Both coordinate spans are retained; duplicate is primary and first is secondary. |
| TM-041 | `explicit_id_source_evidence_is_exact_coordinate_span` | HIR source-map unit | Candidate source returns the exact immediate ID value SourceSpan. |
| TM-042 | `duplicate_explicit_ids_report_aw_cd_020_with_two_spans` | project acceptance unit | Two explicit identical IDs reject project with later primary/first secondary. |
| TM-043 | `explicit_then_generated_collision_does_not_probe` | project acceptance unit | Generated later site collides with earlier explicit and is not renumbered. |
| TM-044 | `generated_then_explicit_collision_does_not_rewrite_either` | project acceptance unit | Explicit later site collides with earlier generated candidate. |
| TM-045 | `two_generated_sites_with_same_candidate_collide_instead_of_skip` | project acceptance unit | Counters from separate modules/prefix state can collide and produce AW-CD-020. |
| TM-046 | `cross_module_collision_uses_package_qualified_sources` | project acceptance unit | Collision records both package/module keys. |
| TM-047 | `cross_document_collision_retains_both_source_identities` | project acceptance unit | Both revision-bound SourceSpans survive. |
| TM-048 | `third_duplicate_relates_to_original_first_site` | project acceptance unit | Each later duplicate uses the original canonical first site as secondary. |
| TM-049 | `module_input_permutations_produce_equal_accepted_inventory` | project acceptance property | All permutations of the same module set yield structurally equal records/indexes. |
| TM-050 | `module_input_permutations_produce_equal_collision_diagnostics` | project acceptance property | All permutations yield identical ordered AW-CD-020 diagnostics. |
| TM-051 | `failed_project_build_preserves_previous_accepted_project_arc` | accepted lifecycle integration | Rejected collision candidate leaves prior project Arc/generation untouched. |
| TM-052 | `valid_build_after_failure_matches_fresh_valid_build` | accepted lifecycle integration | No scratch reservation/counter leaks into the next build. |
| TM-053 | `collision_failure_reserves_no_line_id` | project acceptance unit | A subsequent transaction may accept the formerly colliding ID when only one site remains. |
| TM-054 | `project_diagnostic_limit_one_over_is_atomic` | project acceptance unit | Diagnostic 1,025 returns fatal AW-CD-025 and no partial project/complete set. |
| TM-055 | `project_work_limit_one_over_is_atomic` | project acceptance unit | Work 786,433 returns fatal limit and no project. |
| TM-056 | `project_candidate_limit_one_over_is_atomic` | project acceptance unit | Candidate 262,145 rejects before publication. |
| TM-057 | `recovered_module_publishes_no_executable_candidates` | HIR/project integration | Tooling HIR can exist; executable project rejects recovered status and inventory is empty. |
| TM-058 | `declaration_free_executable_module_contributes_empty_inventory` | project acceptance unit | Empty valid module participates in source/module validation and succeeds. |
| TM-059 | `dependency_module_candidates_share_the_project_namespace` | project acceptance integration | Root/dependency duplicate IDs collide in one inventory. |
| TM-060 | `line_collision_publishes_no_text_key_fact` | project acceptance unit | Neither derived nor explicit key appears from a rejected project. |
| TM-061 | `absent_text_key_derives_complete_line_body` | HIR candidate unit | `say.a.b.c` derives exactly `text.a.b.c`. |
| TM-062 | `explicit_absolute_text_key_is_preserved` | HIR candidate unit | Valid `@text.shared.greeting` retains exact bytes and Explicit provenance. |
| TM-063 | `wrong_text_key_family_reports_aw_cd_024` | HIR diagnostic unit | Absolute non-text family is rejected at exact key span. |
| TM-064 | `relative_text_key_reports_aw_cd_024` | HIR diagnostic unit | Relative/family-relative key is rejected without speaker/owner synthesis. |
| TM-065 | `oversized_explicit_text_key_reports_aw_cd_025` | HIR diagnostic unit | 257-byte explicit key produces no candidate. |
| TM-066 | `derived_text_key_one_byte_growth_is_checked` | HIR candidate unit | A 256-byte line whose derived key is 257 bytes fails without ordinal commit. |
| TM-067 | `shared_explicit_text_key_is_accepted_for_distinct_lines` | project acceptance unit | Two unique lines may intentionally share one explicit key. |
| TM-068 | `explicit_line_rename_updates_derived_text_key` | rename integration | Renaming line changes derived key and typed line references transactionally. |
| TM-069 | `explicit_line_rename_preserves_explicit_text_key` | rename integration | Explicit localization key remains unchanged. |
| TM-070 | `character_rename_changes_neither_line_nor_text_key` | rename integration | Character registration rename leaves both accepted identities equal. |
| TM-071 | `generated_line_rename_materializes_immediate_absolute_id` | rename/code-action integration | Uses AW-AH-009.4.2 insertion facts and changes origin to ExplicitAbsolute. |
| TM-072 | `explicit_line_rename_edits_exact_id_and_reference_spans` | rename integration | No source search; declaration and accepted typed references receive edits. |
| TM-073 | `generated_line_rename_is_unavailable_for_poisoned_source` | rename integration | No clean insertion site means no rename edit. |
| TM-074 | `line_id_consumers_cannot_recover_character_id` | compile/API integration | No conversion or API exposes CharacterId from line/text IDs. |
| TM-075 | `localization_and_runtime_plan_consume_same_accepted_line_record` | cross-layer integration | Both borrow the same generation-owned project fact. |
| TM-076 | `package_module_document_identity_comes_from_hir_module_key` | HIR lowering unit | No display string participates; exact typed values are equal to request key. |
| TM-077 | `source_revision_mismatch_fails_before_candidate_work` | HIR lowering unit | AW-CD-026/fatal mismatch performs zero candidate work and publishes no snapshot. |
| TM-078 | `noop_hir_rebuild_reuses_snapshot_and_candidate_arc` | HIR database integration | Identical request returns exact HirSnapshotId and candidate Arc. |
| TM-079 | `noop_project_rebuild_reuses_accepted_project_arc` | accepted lifecycle integration | Identical ordered module key/snapshot tuple reuses Arc<HirProject>. |
| TM-080 | `changed_source_invalidates_affected_module_and_project_inventory` | incremental integration | Changed module recomputes; project collision result changes generation. |
| TM-081 | `unchanged_module_candidate_arcs_are_reused_after_neighbor_edit` | incremental integration | Only affected module candidates rebuild before project transaction. |
| TM-082 | `stale_expr_id_cannot_lookup_accepted_line` | HIR identity unit | Retired/wrong-module ExprId yields existing IdResolveError. |
| TM-083 | `all_consumers_observe_one_accepted_project_generation` | cross-layer/Tier2 | HIR, sema, LSP, Agent/tooling, runtime-plan leases expose equal generation/project Arc. |
| TM-084 | `aw_cd_020_projects_primary_and_secondary_source_labels` | diagnostic unit | `arcweft_source::Diagnostic` has later primary and first secondary. |
| TM-085 | `aw_cd_020_cross_document_secondary_projects_related_information` | LSP diagnostic integration | Primary document diagnostic contains related information for first document. |
| TM-086 | `aw_cd_013_projects_exact_authored_id_span` | diagnostic unit | Top-level span/primary label equals ID value, not application. |
| TM-087 | `dialogue_line_diagnostic_order_is_deterministic` | diagnostic property | Permuted construction produces the fixed source/code/subject order. |
| TM-088 | `dialogue_line_diagnostic_dedup_uses_complete_typed_identity` | diagnostic unit | Exact duplicates collapse; different later sites remain. |
| TM-089 | `line_diagnostics_do_not_use_single_range_hir_lower_error` | type/API integration | Structured diagnostic retains code/subjects/related span and cannot convert through string-only error. |
| TM-090 | `fatal_limit_projects_aw_cd_025_without_partial_success` | diagnostic/project unit | Fatal typed limit maps to AW-CD-025 and no accepted facts. |
| TM-091 | `accepted_inventory_id_and_expr_indexes_correlate` | project acceptance unit | Every record is found exactly once by ID and source ExprId. |
| TM-092 | `accepted_inventory_source_order_index_is_canonical` | project acceptance unit | Source-order iterator matches package/module/span/source-order key. |
| TM-093 | `accepted_inventory_cache_fingerprint_is_permutation_stable` | project acceptance property | Canonical length-prefixed fingerprint matches for module input permutations. |
| TM-094 | `session_source_and_hir_identities_are_non_serde` | trybuild | HirModuleKey source session IDs, ExprId, ScopeId, SourceSpan cannot auto-serialize. |
| TM-095 | `hir_dependency_graph_does_not_reach_arcweft_dialogue` | cargo-metadata integration | HIR uses arcweft-id constant/types and has no upward dialogue dependency. |
| TM-096 | `removed_dialogue_speaker_slug_api_does_not_compile` | arcweft-lang-hir trybuild | Old speaker slug import/construction fails. |
| TM-097 | `removed_line_identity_helpers_and_counter_context_do_not_compile` | arcweft-lang-hir trybuild | Old normalize/build helpers and line counter field are unavailable. |
| TM-098 | `say_spelling_is_rejected_by_ordinary_current_grammar_and_resolution` | syntax/sema integration | No dedicated removed-spelling diagnostic or executable node survives. |
| TM-099 | `structural_audit_reports_no_parallel_line_or_project_owner` | canonical structural audit | Dependency/type/responsibility analysis finds one project inventory owner; no source spelling checks. |
| TM-100 | `tier2_agent_mcp_runtime_observation_uses_accepted_line_generation` | Tier2 integration | Agent/MCP/runtime observation line IDs equal the accepted project facts without reconstruction. |

## Required command grouping

- TM-001–TM-008: lower identity/runtime conversion focused tests.
- TM-009–TM-026: owner and generation tests.
- TM-027–TM-041: explicit/relative/ownerless coordinate tests.
- TM-042–TM-060: project transaction and lifecycle tests.
- TM-061–TM-075: text-key, rename, and consumer tests.
- TM-076–TM-093: lifecycle, source, diagnostic, and cache tests.
- TM-094–TM-100: API/dependency/deletion/structural/Tier 2 gates.
