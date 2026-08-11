# Test matrix

All tests assert typed public/crate-owned outcomes. No test reads checked-in
implementation or documentation to search for symbol spellings, snippets,
module paths, or deleted branches.

`old_dispatch_calls == 0` below means a crate-owned injected test counter or
resolver fixture, not source scanning.

## 1. Identity and schema invariants

| Test | Input | Required typed assertion |
|---|---|---|
| `callable_name_accepts_identifier` | valid current identifier | exact `CallableName`, borrowed spelling |
| `callable_name_rejects_empty_control_and_separator` | each invalid class | exact `CallableScalarError` kind/byte |
| `adapter_package_id_uses_manifest_id_only` | same id, differing display/path/Rust docs | equal `AdapterPackageId` |
| `adapter_package_id_rejects_invalid_id` | empty/whitespace/path/control | exact scalar error; no publication |
| `rust_item_path_is_provenance_not_key` | two Rust paths for same callable | same lookup key, distinct provenance |
| `index_exact_max_conversion` | `u16::MAX`/`u32::MAX` | succeeds with exact accessor value |
| `index_one_over_conversion` | one over backing width | `IndexOverflow`, no truncation |
| `callable_path_empty_rejected` | zero segments | `CallablePathError::Empty` |
| `callable_path_exact_limit` | 32 typed segments | succeeds |
| `callable_path_one_over` | 33 segments | typed path/build limit error |
| `method_key_uses_typekind_identity` | equal/different structural `TypeKind` | hash/equality follows `TypeKind`, no label involvement |
| `schema_contiguous_groups_and_parameters` | valid multi-group schema | exact groups, coordinates, current group access |
| `schema_rejects_empty_groups` | zero groups | `EmptyGroups`; zero-arg callable must use one empty initial group |
| `schema_rejects_group_gap` | groups 0,2 | `NonContiguousGroup` |
| `schema_rejects_parameter_gap` | params 0,2 | `NonContiguousParameter` |
| `schema_rejects_duplicate_name` | duplicate named slot | `DuplicateParameterName` |
| `schema_rejects_invalid_rest` | rest not final/two rests | `InvalidRestParameter` |
| `schema_rejects_defaulted_rest` | defaulted rest | `InvalidDefaultedRest` |
| `schema_checks_source_coordinate` | mismatched group/param source | `SourceCoordinateMismatch` |
| `schema_semantic_equality_excludes_docs_and_spans` | same typed shape, different docs/source | `semantic_eq == true` |
| `schema_semantic_equality_includes_effects_and_validator` | effect/validator difference | `semantic_eq == false` |
| `non_empty_catalog_set_rejects_empty` | empty vec | `EmptyCandidateSet` |
| `non_empty_resolved_set_rejects_empty` | empty vec | typed resolver invariant error |
| `resolved_callable_rejects_origin_id_mismatch` | builtin ID/project origin | `InvalidResolvedCallable` |
| `function_value_requires_function_type` | non-function `TypeKind` | validating constructor rejects |
| `curried_id_requires_existing_group` | one-over group | typed identity/group error |
| `data_last_id_requires_final_or_next_sole_param` | middle/rest param | typed identity error |

## 2. Complete closed-family identity tables

These are parameterized table tests over every enum row, not spot checks.

| Test | Table coverage | Required assertion |
|---|---|---|
| `fx_identity_table_is_complete` | all 10 `FxCallableSignatureId` paths | `resolve(path) == Known(id)`, schema family/result exact, reverse uniqueness |
| `fx_unknown_member_is_poisoned_family` | direct unknown and nested path | `UnknownMember`/`InvalidNestedPath`; no ordinary fallback |
| `builtin_identity_table_is_complete` | every `BuiltinCallableId`, math row, vector row, capability row | exact typed path and schema; no duplicate path |
| `std_float_identity_table_is_complete` | both widths × every valid operation | exact arity/input/output; invalid conversions rejected |
| `agent_identity_table_is_complete` | all 30 Agent IDs | exact typed path, schema/result/effect family, no duplicate path |
| `presentation_identity_table_is_complete` | all 11 presentation IDs | exact typed path and result type |
| `dialogue_identity_table_is_complete` | speaker/preset/content identities | exact dialogue ID and schema owner behavior |
| `collection_identity_table_is_complete` | len/map/filter/sum/contains | exact method name and validator |
| `handle_identity_table_is_complete` | five lifecycle + Overlay pop | exact receiver applicability/result |
| `integer_identity_table_is_complete` | min/max/clamp | exact arity and same-receiver types |
| `probe_alias_identity_table_is_complete` | all 11 spellings | distinct typed operation IDs, same `Predicate` result |
| `agent_and_builtin_near_misses_do_not_resolve` | case/dot/extra-segment near misses | `None`; no display normalization |

## 3. Project publication

| Test | Fixture | Required typed assertion |
|---|---|---|
| `project_function_publication_preserves_identity` | one source function | candidate is exact `CallableDeclarationId`; exact package/module/path |
| `project_curried_groups_preserved` | three parameter groups | groups 0/1/2 and current/next group exact |
| `project_parameter_kinds_preserved` | positional, named-only, rest, defaulted | exact passing/presence rows |
| `project_docs_preserved` | summary/details/parameter docs | `ProjectSource` provenance and exact text coordinates |
| `project_missing_docs_is_typed_missing` | no docs | `DocumentationProvenance::Missing`, no empty fabricated text |
| `project_parameter_spans_preserved` | exact declaration/name/type/default spans | exact source identity and containment |
| `project_effects_preserved` | declared project effects and checked invocation | catalog retains `CallableEffectSchema::Project` with exact declaration/declared row; committed target facts retain the instantiated checker `EffectRow` |
| `project_module_without_callables_published` | empty module | module/source row present, zero declarations |
| `project_non_callable_binding_published` | project value with env same path | `ProjectNameBinding::NonCallable` exact `TypeKind` |
| `project_non_callable_shadows_environment` | call non-callable project path | `NonCallable` target; env candidate not considered |
| `project_callable_shadows_environment` | same path project/env | exact project candidate selected |
| `duplicate_project_declaration_rejected` | duplicate typed ID | `DuplicateTypedId`; no world publication |
| `duplicate_project_binding_rejected` | two typed bindings same resolution path | `ProjectBindingCollision` |
| `project_source_identity_mismatch_rejected` | parameter span from another source | typed source/build error |
| `source_impl_is_not_synthesized` | source impl method visible only in trait catalog | no project method record; existing trait resolution still works |

## 4. Standard and adapter publication

| Test | Fixture | Required typed assertion |
|---|---|---|
| `standard_manifest_ids_have_typed_owners` | six standard IDs | matching `StandardEnvironmentId`, never Adapter |
| `core_standard_owner_is_distinct` | core built publication | `Standard(Core)` |
| `adapter_callable_path_requires_typed_segments` | multi-segment typed path and dotted string near-miss | typed segments succeed; no dotted-string constructor exists |
| `adapter_signature_rejects_group_or_parameter_gap` | malformed adapter groups | exact `AdapterCallableModelError`; no sema publication |
| `adapter_signature_rejects_invalid_rest_or_name` | defaulted/duplicate/non-final rest and nameless named param | exact typed model error |
| `adapter_tooling_subject_duplicate_or_missing_target` | duplicate and dangling typed subjects | deterministic manifest/publication error |
| `adapter_only_callable_published` | selected non-standard manifest | exact `AdapterPackageId`, ID/key/schema/docs/effects |
| `adapter_tooling_docs_preserved` | typed tooling subject + summary/details/parameter docs | `AdapterTooling` provenance, exact candidate and coordinates; no prose lookup |
| `rust_provenance_preserved` | typed Rust metadata | adapter ID, Rust package name/version/hash, exact `RustItemPath`, exported typed path, purity, and fixed effect row |
| `rust_function_requires_provenance` | RustFunction without metadata | `MissingRustProvenance` |
| `adapter_missing_docs_is_typed_missing` | absent tooling/Rust docs | `Missing` provenance |
| `adapter_defaults_rest_named_preserved` | typed multi-group manifest signature | exact group, name, passing, default/rest, and curried schema rows |
| `adapter_effects_preserved` | typed manifest/Rust effects | exact `CallableEffectSchema::Fixed(EffectRow)` |
| `duplicate_typed_environment_id_rejected` | same ID twice | `DuplicateTypedId`, even if schema equal |
| `same_rank_standard_collision_rejected` | two standard providers/key | `SameRankCollision` with both providers |
| `same_rank_adapter_collision_rejected` | two adapters/key | same typed error |
| `same_provider_overloads_allowed` | one provider, contiguous indices | ordered non-empty set |
| `overload_gap_rejected` | indices 0,2 | `NonContiguousOverloads` |
| `duplicate_provider_overload_rejected` | duplicate index | `DuplicateProviderOverload` |
| `standard_adapter_exact_duplicate_coalesces` | same typed shape, different IDs/docs | one primary Standard, Adapter equivalent retained |
| `standard_adapter_different_signatures_overload` | same key, different parameter types | two candidates in deterministic order |
| `standard_wins_equal_viability_tie` | non-equal shapes with equal score | Standard selected |
| `more_specific_adapter_can_win` | Adapter exact typed match, Standard unchecked/wider | Adapter selected by specificity |
| `reversed_publication_order_is_identical` | forward/reversed input | equal candidate ID slices, selected ID, diagnostics |
| `manifest_display_and_rust_path_do_not_change_identity` | mutate non-ID fields | same owner/key; provenance/docs differ only |
| `launch_profile_and_manifest_path_do_not_change_owner` | same accepted manifest reached through explicit/default/previous/lexical profile selection and different path | owner remains manifest `id`; canonical candidate order unchanged |
| `callable_publication_failure_preserves_previous_world` | every publication/build error | prior world/catalog/characters/sources pointer-identical |

## 5. FX behavior parity

| Test | Input | Required assertion |
|---|---|---|
| `fx_stack_parity` | valid and malformed stack calls | same `Fx` result and exact current diagnostics; selected `Fx::Stack` |
| `fx_conditional_parity` | missing/unknown/positional/typed fields | same errors and expected types; selected ID stable |
| `fx_shader_parity` | leading resource plus properties and invalid extra positional | same diagnostics/result |
| `fx_property_expectations_parity` | sample/color/tint/outline_color | exact expected types in argument facts |
| `fx_other_constructor_named_only_parity` | table over remaining constructors | same named-only behavior/result |
| `unknown_fx_cannot_be_shadowed` | project/env function named same path | poisoned FX target, not project/env |
| `fx_definition_validation_attaches_without_second_target` | user FX definition invocation | one primary ordinary candidate plus validation diagnostics; resolver count 1 |

## 6. Enum, Result, and Option parity

| Test | Input | Required assertion |
|---|---|---|
| `project_enum_expected_owner_disambiguates` | same variant spelling in two enums | expected owner candidate only |
| `project_enum_without_expected_type_does_not_guess` | short variant call, no expected enum | no fabricated enum target |
| `result_ok_expected_type` | expected `Result<A,E>` | payload expected `A`, result exact expected, ID `Ok` |
| `result_err_expected_type` | expected `Result<A,E>` | payload expected `E`, result exact expected, ID `Err` |
| `result_without_expected_type_preserves_placeholder` | no expected | inferred payload side, other placeholder/poison exactly current |
| `result_payload_shape_diagnostics` | missing/multiple/named/spread | exact existing diagnostics and one check per expression |
| `option_some_expected_type` | expected `Option<A>` | payload expected `A`, exact result |
| `constructor_checker_signature_candidate_equal` | each constructor | same primary candidate ID in checker facts and signature |

## 7. Builtin behavior parity

| Test | Coverage | Required assertion |
|---|---|---|
| `builtin_never_family_parity` | panic/fail/bail | same Never result; all args checked once |
| `builtin_assert_family_parity` | ensure/assert/debug_assert | same missing condition/type behavior and Unit |
| `builtin_color_float_vector_parity` | rgb/sin/cos/vec2/3/4 | exact arity, positional diagnostics, types/results |
| `builtin_math_table_parity` | all six math IDs | exact two operands and result |
| `builtin_std_float_table_parity` | every valid width/operation | exact arity/types/result and named/spread diagnostics |
| `capability_event_emit_parity` | valid/malformed emit | unchecked prefix and current result/effects preserved |
| `builtin_reserved_precedence` | same-spelling lexical/project/env callable | builtin candidate selected |

## 8. Agent behavior parity

Use one parameterized test over all 30 IDs for candidate/result/effects, plus
focused special-shape tests:

| Test | Coverage | Required assertion |
|---|---|---|
| `agent_all_intrinsics_candidate_result_effect_parity` | all IDs | exact candidate ID, result `TypeKind`, committed effects, old dispatch count 0 |
| `agent_assert_message_shape_parity` | expect/deny | current positional/named/missing/extra/spread diagnostics |
| `agent_text_and_attach_parity` | checkpoint/note/attach | exact expected types/effects |
| `agent_capture_parity` | target/name/format/kind | exact result/effects/diagnostics |
| `agent_path_and_probe_parity` | state/observation paths, signal/metric | exact entity/path types and probe results |
| `agent_predicate_combinator_parity` | exists/action_enabled/all/any/not | exact predicate typing |
| `agent_viewport_and_pointer_parity` | viewport/point/click | exact coordinate/point types/effects |
| `agent_invoke_and_rag_parity` | invoke/rag.query | exact current mapping/effects/results |
| `agent_reserved_precedence` | same-spelling project/env | Agent candidate selected |

## 9. Presentation structural typing and behavior

| Test | Fixture | Required typed assertion |
|---|---|---|
| `presentation_all_ids_result_parity` | all 11 calls | exact candidate/result/current diagnostics/effects |
| `show_look_canonical_owner` | canonical owner spelling | exact `CharacterId`, `Look` expectation, clean |
| `show_look_compact_owner` | compact spelling | same candidate/owner/expectation |
| `show_look_qualified_owner` | qualified spelling | same candidate/owner/expectation |
| `show_look_alias_owner` | authored alias | same structural owner; alias only in display |
| `show_look_second_positional` | second positional look | maps group0/param1 |
| `show_look_named` | named look | same coordinate |
| `show_look_duplicate_positional_named` | both forms | one `DuplicateArgument`, one expression check each |
| `show_owner_missing` | omitted character | selected Show, typed owner diagnostic, look unchecked/recovered |
| `show_owner_non_character` | non-character entity/value | exact type-mismatch diagnostic, no guessed owner |
| `show_owner_unknown_external` | unresolved external owner | exact unknown-external diagnostic |
| `show_owner_unknown_part` | typed stale/unknown part fact | exact unknown-part diagnostic |
| `same_look_spelling_different_characters_rejected` | owner A, value from B | structural type mismatch |
| `same_spelling_look_part_variant_rejected` | same local text across families | only `Look { A }` accepted |
| `presentation_open_named_behavior_preserved` | view/menu/overlay/show/ref/hide unknown named | checked once, no new unknown-name error where open |
| `presentation_closed_named_behavior_preserved` | bg/image/player_viewport unknown named | exact existing diagnostic |
| `presentation_state_commits_once` | clear.bg/hide plus competing overload fixture | state changes only for selected commit |
| `presentation_rejected_candidate_does_not_mutate_state` | failed candidate | state pointer/value unchanged |

## 10. Dialogue structural typing and behavior

| Test | Fixture | Required typed assertion |
|---|---|---|
| `dialogue_look_canonical_owner` | speaker line canonical | exact `Look { character }` expectation |
| `dialogue_look_compact_owner` | compact | same structural owner |
| `dialogue_look_qualified_owner` | qualified | same structural owner |
| `dialogue_look_alias_owner` | alias | same structural owner; alias display only |
| `dialogue_speaker_preset_owner` | preset value | same character identity |
| `dialogue_owner_missing` | content call without owner | dialogue target retained; typed diagnostic; look unchecked |
| `dialogue_owner_non_character` | typed non-character callee | exact mismatch diagnostic |
| `dialogue_owner_unknown_external` | missing external fact | exact unknown-external diagnostic |
| `dialogue_owner_unknown_part` | unknown part fixture | exact unknown-part diagnostic |
| `dialogue_same_local_spelling_is_scoped` | cross-character/look/part/variant | only exact structural owner accepted |
| `dialogue_reserved_option_order` | all `LineOptions` | exact parameter coordinates and active mapping |
| `dialogue_open_line_args_preserve_order` | custom names | open named rows after reserved fields |
| `dialogue_reserved_custom_duplicate` | custom name equals reserved | one duplicate diagnostic |
| `dialogue_content_validation_unchanged` | tags/marks/FX/wait/speed/rich text | same content diagnostics/line-plan; callable candidate unchanged |
| `dialogue_checker_signature_candidate_equal` | focused signature query | same dialogue candidate ID and arg facts |

## 11. Lexical, project, environment, virtual, and function-value parity

| Test | Fixture | Required assertion |
|---|---|---|
| `local_function_selected_before_project_environment` | same name at all levels | exact `LocalCallableId` |
| `local_non_callable_stops_fallback` | local value plus project/env callable | non-callable target |
| `project_callable_selected_before_environment` | same path | project ID |
| `project_non_callable_stops_environment` | non-callable project binding | non-callable facts |
| `standard_selected_before_adapter_equal_tie` | equal viability | standard environment ID |
| `adapter_only_free_call` | adapter-only record | exact adapter ID/result/effects |
| `virtual_path_valid_parity` | current accepted virtual path | same result/effects |
| `virtual_path_rejected_parity` | OS absolute/prohibited path | same diagnostic/poison; no fallback |
| `speaker_callable_parity` | speaker and preset | exact speaker candidate/result |
| `function_value_fixed_call` | exact function type | exact argument/result facts |
| `function_value_named_rejected` | named arg | same diagnostic/poison |
| `function_value_fixed_literal_spread` | fixed literal spread | same mapping/result |
| `function_value_dynamic_spread_rejected` | dynamic spread | same diagnostic |
| `curried_groups_progress_exactly` | three groups | base ID/current/next group sequence exact |
| `partial_call_result_parity` | allowed and disallowed contexts | same function result/diagnostics |
| `higher_order_effect_commits_selected_only` | two overloads with effects | only selected effect edge committed |
| `rejected_candidate_target_facts_absent` | overloaded call | rejected candidate only in considered list, no committed facts/effects |

## 12. Selected method family parity

| Test | Coverage | Required assertion |
|---|---|---|
| `drop_method_parity` | drop name with args | `Drop` candidate, Unit, current recovery |
| `traverse_parity` | valid/invalid receiver/task/effects | exact candidate/result/diagnostics/effects |
| `parallel_parity` | valid/invalid named limit/receiver | exact candidate/result/diagnostics |
| `environment_method_typed_parity` | typed standard/adapter method | exact signature mapping/result/effects |
| `environment_method_untyped_parity` | normalized legacy row | args checked once, same result |
| `collection_all_methods_parity` | len/map/filter/sum/contains valid+invalid | exact candidate/result/diagnostics/effects |
| `presentation_handle_all_methods_parity` | lifecycle + Overlay pop | exact candidate/no-arg/Unit |
| `integer_all_methods_parity` | min/max/clamp over every integer type | same-type args/result and diagnostics |
| `domain_all_methods_parity` | every row in inventory | exact candidate/result/current diagnostic/effect |
| `capacity_table_parity` | every current capacity table row | exact typed ID/arity/result, untyped args |
| `trait_inherent_parity` | inherent catalog outcome | exact trait ID/projection/result |
| `trait_unique_parity` | one visible trait | exact trait ID/projection/result |
| `trait_ambiguous_is_terminal` | two visible traits + data-last | ambiguity, args checked once, no fallback |
| `data_last_direct_parameter_parity` | receiver final current parameter | exact injected coordinate/result/effects |
| `data_last_curried_parameter_parity` | receiver sole next group parameter | exact group progression/result/effects |
| `data_last_ambiguity` | two same-rank candidates | typed ambiguity, no selected effects |
| `unknown_method_parity` | no family | same unknown diagnostic and one arg check each |

## 13. Selected method precedence

| Test | Competing families | Required selected ID |
|---|---|---|
| `traverse_beats_environment` | domain traverse + env same receiver/name | `Domain::Traverse` |
| `parallel_beats_environment` | domain parallel + env | `Domain::Parallel` |
| `environment_beats_collection` | env `len` + iterable | Environment |
| `environment_beats_handle` | env lifecycle + handle | Environment |
| `environment_beats_integer` | env `min` + integer | Environment |
| `environment_beats_domain` | env `get` + map | Environment |
| `environment_beats_capacity` | env + capacity row | Environment |
| `environment_beats_trait` | env + visible trait | Environment |
| `environment_beats_data_last` | env + fallback | Environment and one shadow warning |
| `collection_beats_trait_and_data_last` | collection + both | Collection and one shadow warning when fallback viable |
| `handle_beats_trait_and_data_last` | handle + both | Handle and one shadow warning |
| `integer_beats_trait_and_data_last` | integer + both | Integer and one shadow warning |
| `domain_beats_trait_and_data_last` | domain + both | Domain and one shadow warning |
| `capacity_beats_trait_and_data_last` | capacity + both | Capacity and one shadow warning |
| `trait_unique_beats_data_last` | unique trait + fallback | Trait and one shadow warning |
| `trait_ambiguity_blocks_data_last` | ambiguous traits + fallback | ambiguity, no fallback |
| `closed_collection_name_invalid_receiver_does_not_fallback` | invalid receiver + data-last same name | collection rejection/recovery, no fallback |

## 14. Shared argument mapping and diagnostics

| Test | Input | Required assertion |
|---|---|---|
| `positional_mapping` | positional args | exact coordinates/inferred/expected facts |
| `named_reordered_mapping` | reversed named args | exact named coordinates independent of order |
| `duplicate_named_mapping` | duplicate name | one stable diagnostic, both expressions checked once |
| `positional_named_same_slot_duplicate` | both forms | duplicate coordinate diagnostic |
| `missing_required_parameter` | omission | stable missing diagnostic, default/optional unaffected |
| `defaulted_and_optional_omission` | omission | viable; omission score exact |
| `rest_positional_mapping` | extra args | all map to rest coordinate/order |
| `rest_named_mapping` | open/rest named args | exact rest mapping |
| `unsupported_spread` | spread under Reject | stable diagnostic and one expression check |
| `fixed_literal_spread` | fixed literal | deterministic expanded mapping plus one committed slot fact per typed element |
| `open_checked_named` | unknown open value | validator runs once and fact retains authored name |
| `open_unchecked_named` | unknown open value | one unchecked expression check, no fabricated parameter ID |
| `candidate_checkpoint_rolls_back_diagnostics` | first overload rejects, second wins | only selected diagnostics remain |
| `candidate_checkpoint_rolls_back_presentation_state` | rejected presentation candidate | no state leak |
| `candidate_checkpoint_rolls_back_effects` | rejected effectful candidate | no effect leak |
| `candidate_checkpoint_rolls_back_borrow_facts` | rejected candidate | no borrow/evidence leak |
| `equal_viability_same_provider_is_ambiguous` | two overloads | deterministic candidate list and typed ambiguity |

## 15. Target facts and public results

| Test | Input | Required assertion |
|---|---|---|
| `focused_mode_records_exactly_one` | focused call expression | one fact with exact expression/document/source |
| `focused_mode_missing_expression_rejected` | absent ID | typed focused-fact error |
| `focused_mode_duplicate_expression_rejected` | corrupt duplicate fixture | typed duplicate error |
| `disabled_mode_allocates_no_facts` | ordinary check | no fact map/vector allocation via test instrumentation |
| `all_mode_keys_by_typed_expression_id` | nested calls | exact distinct facts |
| `checker_and_signature_primary_id_equal_all_families` | parameterized over every family enum/table | equality of primary candidate IDs |
| `equivalent_ids_preserved_in_signature` | coalesced standard/adapter | exact ordered equivalent IDs |
| `argument_inferred_expected_types_preserved` | ordinary, fixed-spread, mismatch, recovered args | exact authored-argument and per-slot expression/inferred/expected/mapped facts and poison |
| `function_value_type_and_group_preserved` | curried/higher-order call | exact function type/current/next group |
| `semantic_signature_rejects_duplicate_candidate` | duplicate help entries | `DuplicateCandidate` |
| `semantic_help_rejects_empty` | empty signatures | `EmptySignatures` |
| `semantic_help_active_signature_bounds` | exact max and one-over index | exact success/failure |
| `semantic_help_active_parameter_bounds` | valid/invalid coordinate | exact success/failure |
| `semantic_help_source_identity_validation` | mixed source spans | `SourceIdentityMismatch` |
| `ambiguous_help_is_deterministic` | ambiguous overload | ordered signatures + ambiguity diagnostic, no checker selection claim |
| `labels_are_not_identity` | alter display alias/format | candidate/type/source unchanged |

## 16. Limits and cancellation

For every inclusive limit, run exact and one-over tests.

| Limit | Exact assertion | One-over assertion |
|---|---|---|
| 32 path segments | succeeds | typed build/path failure |
| 16 groups | succeeds | schema/build failure |
| 128 parameters | succeeds | schema/build failure |
| 32 overloads/key | succeeds | catalog build failure |
| 256 candidates/call | query succeeds/ambiguity as applicable | query limit, no facts/cache |
| 32 nested calls | succeeds | query limit before entering one-over |
| 256 recovery nodes | succeeds | query limit before staging one-over |
| 128 diagnostics | succeeds | query limit before staging one-over |
| 8,388,608 source bytes | input/result succeeds | source-byte query rejection |
| 4,096 project modules | catalog succeeds | candidate world rejected |
| 262,144 catalog records | catalog succeeds | candidate world rejected |
| 1,048,576 build work | catalog succeeds | candidate world rejected, prior world preserved |
| 4,096 query work | query succeeds | typed exhaustion, no partial facts/help/cache |

Additional tests:

| Test | Required assertion |
|---|---|
| `build_work_checked_add_overflow` | `WorkOverflow`, counter unchanged |
| `query_work_checked_add_overflow` | `ArithmeticOverflow`, no partial result |
| `cancellation_before_family_probe` | Cancelled, no candidate/argument check |
| `cancellation_between_candidates` | Cancelled, all checkpoints discarded |
| `cancellation_before_commit` | Cancelled, no effects/state/facts/cache |
| `corrupt_empty_candidate_set_fails_closed` | `CorruptCatalog::EmptySet` |
| `corrupt_key_mismatch_fails_closed` | typed key mismatch |
| `corrupt_duplicate_or_unsorted_ids_fail_closed` | typed reason, no guessed target |
| `corrupt_missing_by_id_record_fails_closed` | typed reason |

## 17. Atomicity and insertion determinism

| Test | Required assertion |
|---|---|
| `every_build_error_preserves_prior_arc` | previous registered world, env, catalog, character facts, source registry, cache pointer-identical |
| `reversed_project_module_input_is_canonicalized_by_hir_order` | same module rows/records |
| `reversed_environment_publications_same_catalog` | same ordered IDs and lookup results |
| `hashmap_seed_does_not_change_result` | repeated randomized hash seeds yield same typed slices/selection |
| `failed_rebuild_does_not_pair_new_manifest_with_old_world` | old generation/key remains exact; new query not served |

## 18. Dependency and visibility evidence

| Test/evidence | Required assertion |
|---|---|
| public API compile test | intended IDs/schemas/results/accessors construct/read only through valid APIs |
| negative visibility compile test | catalog builder, mutable lexical insertion, raw ID construction, corrupt constructors inaccessible externally |
| HIR public API test | callable source rows accessible without sema dependency |
| adapter publication public API test | manifest normalizes to sema publication; no direct env callable mutation API |
| Cargo metadata assertion | HIR has no sema dependency |
| Cargo metadata assertion | sema has no adapter-context or LSP dependency |
| Cargo metadata assertion | adapter-context-to-sema edge is acyclic and intended |
| Cargo metadata assertion | core/runtime-host gain no syntax/HIR/sema normal dependency |
| trait/Serde compile evidence | callable catalog/facts/function-value IDs do not implement persisted serialization |
| canonical structural audit | zero structural errors under repository policy |

## 19. Final migration evidence

The final integration test runs one representative accepted and rejected call
from every inventory family through the public checker and signature query,
asserting:

```text
shared_resolver_invocations == number_of_call_expressions
old_dispatch_calls == 0
checker.primary_candidate == signature.primary_candidate
argument_expression_checks == exactly_once_per_committed_or_recovery_argument
```

The counters are test-only typed instrumentation at the dispatcher boundary.
They do not scan source and do not ship in production.
