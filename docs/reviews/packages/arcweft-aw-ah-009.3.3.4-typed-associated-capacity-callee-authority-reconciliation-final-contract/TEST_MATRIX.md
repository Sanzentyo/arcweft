# Test matrix

Every row is required. Test names are normative; implementations may place a row in the named crate's unit or integration test module while preserving the exact assertion. No row is optional and no source-text scan is an acceptance test.

## 1. Lossless parser and call-surface rows

| ID | Test | Fixture | Required assertion |
|---|---|---|---|
| S01 | `arcweft_lang_syntax::associated_string_dot_ranges` | `String.with_capacity(64)` | `PathMember`; receiver `0..6`, dot `6..7`, member `7..20`, callee `0..20`, call `0..24` |
| S02 | `arcweft_lang_syntax::associated_bytes_dot_ranges` | `Bytes.with_capacity(4096)` | exact receiver/dot/member/call ranges and unchanged argument-list punctuation |
| S03 | `arcweft_lang_syntax::associated_bare_vec_retains_typed_path_candidate` | `Vec.with_capacity(8)` | receiver is an authored path with zero generic arguments; no placeholder is inserted |
| S04 | `arcweft_lang_syntax::associated_vec_generic_dot_ranges` | `Vec<I32>.with_capacity(8)` | exact ranges from `TYPE_RECEIVER_MODEL.md` section 3.1 |
| S05 | `arcweft_lang_syntax::associated_vec_generic_parameter_dot_ranges` | `Vec<T>.with_capacity(8)` | `T` is a `GenericArgument(0)` node with exact segment range |
| S06 | `arcweft_lang_syntax::associated_qualified_receiver_lexemes` | `pkg::types::Vec<I32>.with_capacity(8)` | all three segments, two receiver path separators, generic delimiters, terminal dot/member |
| S07 | `arcweft_lang_syntax::associated_alias_receiver_lexemes` | `Alias<I32>.with_capacity(8)` | alias path/generic lexemes retained; no semantic alias decision in syntax |
| S08 | `arcweft_lang_syntax::associated_existing_generic_path_ranges` | `Vec<I32>::with_capacity(8)` | exact ranges from section 3.2; terminal separator is `Path` |
| S09 | `arcweft_lang_syntax::associated_generic_parameter_path_ranges` | `Vec<T>::with_capacity(8)` | exact `T` node and terminal `::` separator |
| S10 | `arcweft_lang_syntax::associated_turbofish_dot_ranges` | `Vec::<I32>.with_capacity(8)` | exact ranges from section 3.3 and one `TurbofishSeparator` |
| S11 | `arcweft_lang_syntax::associated_turbofish_path_ranges` | `Vec::<I32>::with_capacity(8)` | exact ranges from section 3.4 and distinct receiver/member `::` tokens |
| S12 | `arcweft_lang_syntax::associated_nested_generic_lexeme_tree` | `Vec<Option<Result<T,E>>>.with_capacity(8)` | every nested node/delimiter/argument separator has one owner and exact range |
| S13 | `arcweft_lang_syntax::type_source_map_maps_nodes_and_lexemes_together` | accepted generic receiver | `try_map` preserves node/lexeme kinds and structural addresses while replacing every range |
| S14 | `arcweft_lang_syntax::type_source_map_rejects_missing_duplicate_and_out_of_order_lexemes` | constructor-level invalid maps | exact typed `TypeRefSourceMapError`; no partial value |
| S15 | `arcweft_lang_syntax::comparison_lookahead_unchanged_by_associated_receiver` | `a<b`, `a<b+c`, `a < b > (c)` | existing binary AST and ranges; no type candidate |
| S16 | `arcweft_lang_syntax::malformed_static_generic_rolls_back_atomically` | `Vec::<T::>().with_capacity(8)`, `Vec<,T>.with_capacity(8)` | ordinary grammar failure/recovery; no hidden identifier suffix or partial lexeme map |
| S17 | `arcweft_lang_syntax::nongeneric_path_separator_aliases_not_introduced` | `String::with_capacity(64)`, `Bytes::with_capacity(8)`, `Vec::with_capacity(8)` | no `PathMember` associated carrier for a nongeneric terminal `::` form |
| S18 | `arcweft_lang_syntax::ordinary_expression_receiver_remains_ordinary` | `factory().with_capacity(8)` | `ParenthesizedCalleeSyntax::Ordinary`; existing selected call ranges unchanged |
| S19 | `arcweft_lang_syntax::call_argument_surface_unchanged_for_associated_callee` | positional, named, spread, trailing comma, missing close | existing `ArgumentListSyntax` ranges/forms/recovery/active slots byte-for-byte unchanged |
| S20 | `arcweft_lang_syntax::static_generic_current_fixture_parses_without_source_scan` | line from `051_collections_vec_array_methods.arcw` | typed path/member surface; no helper/source-scanning parser |

## 2. HIR and accepted source identity rows

| ID | Test | Fixture | Required assertion |
|---|---|---|---|
| H01 | `arcweft_lang_hir::associated_callee_survives_module_clone` | every accepted spelling | cloned `Expr::Call` preserves callee enum, receiver tree, nodes, lexemes, separator, member, and ranges |
| H02 | `arcweft_lang_hir::associated_receiver_binds_to_exact_document` | accepted source | every node/lexeme/member/callee span has the same expected `SourceDocumentIdentity` |
| H03 | `arcweft_lang_hir::associated_receiver_rejects_foreign_document` | source map + another document | typed wrong-document failure; no local-string fallback |
| H04 | `arcweft_lang_hir::associated_receiver_rejects_out_of_bounds_and_utf8_split` | invalid mapped range | typed source error; no HIR value publication |
| H05 | `arcweft_lang_hir::associated_receiver_reparse_uses_new_identity` | document replacement | old source-backed receiver is stale/foreign and cannot be reused |
| H06 | `arcweft_lang_hir::associated_call_has_no_parallel_hir_call_enum` | public/crate-owned HIR traversal | ordinary traversal sees one `Expr::Call` and one parenthesized callee carrier only |

## 3. Nominal/type receiver rows

| ID | Test | Fixture | Required assertion |
|---|---|---|---|
| T01 | `arcweft_lang_sema::associated_string_resolves_builtin_type` | `String.with_capacity(64)` with no value `String` | exact `TypeKind::String` product |
| T02 | `arcweft_lang_sema::associated_bytes_resolves_builtin_type` | `Bytes.with_capacity(8)` | exact `TypeKind::Bytes` product |
| T03 | `arcweft_lang_sema::associated_vec_i32_resolves_structurally` | all four `Vec<I32>` spellings | exact `TypeKind::Vec(I32)` and equal normalized receiver |
| T04 | `arcweft_lang_sema::associated_generic_parameter_preserves_id` | generic function `T`, all four `Vec<T>` spellings | receiver child equals the same `GenericTypeParameterId`; no text reconstruction |
| T05 | `arcweft_lang_sema::associated_shadowed_generic_parameters_keep_scope_identity` | nested generic scopes both named `T` | inner call uses inner ID; outer call uses outer ID |
| T06 | `arcweft_lang_sema::associated_qualified_type_preserves_declaration_identity` | two same-name types in different modules | exact selected module/declaration ID |
| T07 | `arcweft_lang_sema::associated_alias_normalizes_target_and_retains_alias_facts` | alias to `Vec<I32>` | capacity receiver/result is target; nominal product retains alias source/declaration facts |
| T08 | `arcweft_lang_sema::associated_bare_vec_is_typed_arity_failure` | `Vec.with_capacity(8)` | no receiver projection/candidate; no `_`; one argument recovery check |
| T09 | `arcweft_lang_sema::associated_unknown_type_is_terminal` | `Missing<I32>.with_capacity(8)` | exact nominal missing diagnostic; no environment/capacity/trait lookup after type failure |
| T10 | `arcweft_lang_sema::associated_ambiguous_type_is_terminal` | two visible same-name types | typed ambiguity; no candidate; args once |
| T11 | `arcweft_lang_sema::associated_wrong_kind_type_is_terminal` | path resolves to non-type symbol | wrong-kind diagnostic; no reinterpretation |
| T12 | `arcweft_lang_sema::associated_unresolved_generic_argument_is_structural_failure` | `Vec<Missing>.with_capacity(8)` | nested argument diagnostic at exact range; no `Named("Missing")` guess |
| T13 | `arcweft_lang_sema::associated_alias_cycle_is_terminal` | cyclic aliases | typed cycle; no capacity candidate |

## 4. Capacity identity/schema/result rows

| ID | Test | Fixture | Required assertion |
|---|---|---|---|
| C01 | `arcweft_lang_sema::associated_string_capacity_identity` | `String.with_capacity(64)` | ID receiver String, member `with_capacity`, arity 1, family/origin capacity, result String, `TypeReceiver(String)` |
| C02 | `arcweft_lang_sema::associated_bytes_capacity_identity` | `Bytes.with_capacity(4096)` | exact Bytes facts |
| C03 | `arcweft_lang_sema::associated_vec_capacity_identity` | `Vec<I32>.with_capacity(8)` | exact `Vec<I32>` ID/result/instantiation |
| C04 | `arcweft_lang_sema::associated_generic_vec_capacity_identity` | `Vec<T>.with_capacity(8)` | exact generic parameter ID in ID/result/instantiation |
| C05 | `arcweft_lang_sema::associated_alias_capacity_uses_normalized_receiver` | alias receiver | ID/result equals target; no alias label in identity |
| C06 | `arcweft_lang_sema::associated_spelling_forms_have_equal_candidate` | dot/path/turbofish variants with same receiver/arity | candidate ID, family, origin, schema, result, and instantiation equal |
| C07 | `arcweft_lang_sema::capacity_schema_is_variadic_unchecked_without_placeholder` | any capacity ID | one rest-unchecked/open-unchecked schema as parent contract; no exact `_` parameter/result |
| C08 | `arcweft_lang_sema::capacity_arity_identity_zero` | `String.with_capacity()` | ID arity 0; accepted unchecked schema; result String |
| C09 | `arcweft_lang_sema::capacity_arity_identity_one` | one positional | ID arity 1 |
| C10 | `arcweft_lang_sema::capacity_arity_identity_multiple` | three positional | ID arity 3 |
| C11 | `arcweft_lang_sema::capacity_arity_identity_named` | `capacity = n` | ID counts one authored entry; schema accepts; value checked once |
| C12 | `arcweft_lang_sema::capacity_arity_identity_spread` | `values...` | ID counts one authored entry; schema accepts spread; value checked once |
| C13 | `arcweft_lang_sema::capacity_arity_identity_mixed` | positional + named + spread | ID equals three authored entries; each value once |
| C14 | `arcweft_lang_sema::capacity_near_miss_member_not_selected` | `String.with_capacitx(8)` | no capacity ID; proceed to trait/unknown |
| C15 | `arcweft_lang_sema::nonreservable_type_not_capacity` | `Map<I32,I32>.with_capacity(8)` | no capacity candidate unless typed environment/trait owns member |
| C16 | `arcweft_lang_sema::value_with_capacity_never_static_capacity` | lexical `value: String` | `CallCallee::Selected`; no `TypeReceiver`, no static capacity candidate |
| C17 | `arcweft_lang_sema::bare_vec_never_constructs_placeholder_receiver` | bare Vec | no `CapacityMethodId`; no `TypeKind::Named("_")` in facts/diagnostics/schema |

## 5. Registered/non-registered and public parity rows

| ID | Test | Fixture | Required assertion |
|---|---|---|---|
| P01 | `arcweft_lang_sema::associated_capacity_registered_detached_candidate_parity` | equal worlds for every supported receiver | candidate/family/origin/receiver/arity/schema/result/instantiation equal |
| P02 | `arcweft_lang_sema::associated_capacity_registered_detached_argument_parity` | zero/multiple/named/spread | equal mapping/poison/diagnostics and one check per entry |
| P03 | `arcweft_lang_sema::associated_capacity_checker_signature_primary_equal` | public checker + native signature query | exact primary candidate equality |
| P04 | `arcweft_lang_sema::associated_capacity_checker_signature_schema_equal` | same query | groups/policy/result/effects/origin/poison equal |
| P05 | `arcweft_lsp::associated_capacity_native_lsp_projection_parity` | cursor in every argument slot | LSP displays native semantic help; no LSP resolution or string parsing |
| P06 | `arcweft_lang_sema::associated_capacity_source_identity_does_not_change_candidate` | accepted vs detached same semantics | only source evidence differs; candidate identity equal |
| P07 | `arcweft_lang_sema::associated_capacity_all_spelling_forms_public_parity` | canonical/current/turbofish spellings | checker and signature primary equal for each and across equivalent forms |

## 6. Collision precedence rows

| ID | Test | Fixture | Required assertion |
|---|---|---|---|
| X01 | `arcweft_lang_sema::associated_dot_lexical_value_beats_builtin_type` | lexical value named `String` | value-selected; no type resolution retry |
| X02 | `arcweft_lang_sema::associated_dot_project_value_beats_imported_type` | project value/type collision | project value-selected; no type retry |
| X03 | `arcweft_lang_sema::associated_dot_environment_value_beats_type` | environment value/type collision | environment value-selected; no type retry |
| X04 | `arcweft_lang_sema::associated_dot_value_ambiguity_is_terminal` | ambiguous values plus valid type | typed value ambiguity; no type retry |
| X05 | `arcweft_lang_sema::associated_dot_value_access_error_is_terminal` | inaccessible value plus public type | value error; no type retry |
| X06 | `arcweft_lang_sema::associated_path_generic_bypasses_value_namespace` | value named Vec plus `Vec<T>::with_capacity` | type-associated; value lookup counter zero |
| X07 | `arcweft_lang_sema::associated_typed_environment_beats_capacity` | typed env `with_capacity` on String | environment candidate; capacity selector not selected |
| X08 | `arcweft_lang_sema::associated_typed_environment_beats_trait` | env and trait same member | environment candidate |
| X09 | `arcweft_lang_sema::associated_capacity_beats_trait` | String capacity plus trait member | capacity candidate |
| X10 | `arcweft_lang_sema::associated_unique_trait_after_capacity_miss` | noncapacity member with one trait | trait candidate |
| X11 | `arcweft_lang_sema::associated_trait_ambiguity_is_terminal` | two visible traits | typed ambiguity; no data-last/untyped fallback |
| X12 | `arcweft_lang_sema::associated_data_last_is_inapplicable` | viable same-name data-last callable | data-last candidate count zero; no shadow warning |
| X13 | `arcweft_lang_sema::associated_untyped_method_fallback_is_ineligible` | map-only legacy row | no fallback candidate |
| X14 | `arcweft_lang_sema::associated_near_miss_trait_can_resolve` | `with_capacitx` supplied by one trait | trait candidate, never capacity |
| X15 | `arcweft_lang_sema::associated_near_miss_without_trait_is_unknown` | no owner | `UnknownCallKind::AssociatedType` |

## 7. Failure/recovery and exactly-once rows

| ID | Test | Fixture | Required assertion |
|---|---|---|---|
| R01 | `arcweft_lang_sema::associated_malformed_receiver_checks_retained_arguments_once` | recovered malformed generic + valid args | no associated seed; every retained arg count 1 |
| R02 | `arcweft_lang_sema::associated_missing_member_checks_arguments_once` | recovered missing member | unknown recovery; each retained arg once |
| R03 | `arcweft_lang_sema::associated_unknown_type_checks_arguments_once` | missing type + three args | each arg once, resolver capacity count zero |
| R04 | `arcweft_lang_sema::associated_ambiguous_type_checks_arguments_once` | ambiguous type + named/spread | each value once |
| R05 | `arcweft_lang_sema::associated_invalid_member_checks_arguments_once` | valid type + no env/capacity/trait | unknown associated + each arg once |
| R06 | `arcweft_lang_sema::associated_trait_ambiguity_checks_arguments_once` | ambiguous traits | one check each; no fallback replay |
| R07 | `arcweft_lang_sema::associated_recovered_argument_has_one_slot` | recovered nonempty value | one recovered fact/slot/check; no phantom argument |
| R08 | `arcweft_lang_sema::associated_cancellation_before_commit_is_atomic` | cancel at each resolver/checker step | no target facts/candidate/cache publication; no duplicated committed checks |
| R09 | `arcweft_lang_sema::associated_work_exhaustion_is_atomic` | fail at each work boundary | typed resource error; no partial candidate/facts |
| R10 | `arcweft_lang_sema::associated_stale_source_is_noncacheable` | stale accepted document | typed source error; no result cache entry |

## 8. Exact counters

| ID | Test | Required exact counters |
|---|---|---|
| W01 | `arcweft_lang_sema::associated_capacity_success_exact_counters` | call registrations 1; nominal resolutions 1; shared resolver 1; typed env lookup 1; capacity selector 1; capacity materialization 1; old dispatch 0; selected replay 1; fact publication 1; argument checks = authored entries |
| W02 | `arcweft_lang_sema::associated_environment_override_exact_counters` | resolver 1; env lookup 1; capacity materialization 0; old dispatch 0; args once |
| W03 | `arcweft_lang_sema::associated_type_failure_exact_counters` | nominal 1; shared resolver 0; old dispatch 0; args once in recovery |
| W04 | `arcweft_lang_sema::associated_trait_fallback_exact_counters` | resolver 1; env lookup 1; capacity selector 1 with miss; trait resolution 1; args once |
| W05 | `arcweft_lang_sema::associated_signature_query_exact_counters` | surface traversal once; resolver once; candidate materialization once; selected replay once; old dispatch zero; each arg once |
| W06 | `arcweft_lang_sema::associated_registered_detached_counter_parity` | same counters for semantically equal registered/detached calls |

## 9. Limits and atomic boundaries

| ID | Test | Boundary | Required assertion |
|---|---|---|---|
| L01 | `arcweft_lang_syntax::associated_call_exact_argument_limit` | `MAX_CALL_ARGUMENTS` | typed call/callee and all entries published |
| L02 | `arcweft_lang_syntax::associated_call_one_over_argument_limit` | `MAX_CALL_ARGUMENTS + 1` | existing call-argument limit error before syntax value publication |
| L03 | `arcweft_lang_syntax::associated_receiver_exact_generic_argument_limit` | existing max generic arguments | complete type tree/lexeme map |
| L04 | `arcweft_lang_syntax::associated_receiver_one_over_generic_argument_limit` | one over | typed parser limit; no partial receiver |
| L05 | `arcweft_lang_syntax::associated_receiver_exact_type_node_limit` | existing max type nodes | complete map |
| L06 | `arcweft_lang_syntax::associated_receiver_one_over_type_node_limit` | one over | typed limit; no partial map |
| L07 | `arcweft_lang_sema::associated_capacity_exact_resolver_work_limit` | exact configured work | success with exact report |
| L08 | `arcweft_lang_sema::associated_capacity_one_over_resolver_work_limit` | one over | atomic resource error and non-cacheable outcome |
| L09 | `arcweft_lang_sema::associated_capacity_arity_conversion_boundary` | maximum callable arity accepted by `CapacityMethodId` | exact ID succeeds; one-over fails before candidate publication |
| L10 | `arcweft_lang_sema::associated_capacity_candidate_limit_does_not_partially_publish` | environment overloads at/over limit | exact succeeds; one-over rejects atomically |

## 10. Deletion and structural rows

| ID | Test/evidence | Required assertion |
|---|---|---|
| D01 | `arcweft_lang_sema::associated_capacity_typed_authority_compiles` | crate-owned compile test constructs `ParenthesizedCalleeSyntax`, nominal projection, `CallCallee::AssociatedType`, accepted/detached authorities, and exhaustive `CallableInstantiation::TypeReceiver`; no string seed is part of any API |
| D02 | `arcweft_lang_sema::associated_capacity_old_dispatch_counter_is_zero` | public checker and signature rows report zero old-dispatch calls for all spellings/modes |
| D03 | `arcweft_lang_sema::associated_capacity_family_inventory_remains_23` | no `BuiltinCallableId`, no 24th family, exact existing capacity family ID |
| D04 | `arcweft_lsp::associated_capacity_dependency_direction` | syntax <- HIR <- sema <- LSP direction; no syntax/HIR dependency on sema/LSP |
| D05 | `arcweft_lang_sema::associated_capacity_no_runtime_receiver_injection` | exhaustive instantiation handling rejects `TypeReceiver` from value/data-last injection paths at compile time |
| D06 | `arcweft_lang_sema::associated_capacity_schema_has_no_placeholder` | typed schema traversal finds no `Named("_")`; this is typed behavior, not a source scan |
| D07 | `cargo check --workspace --all-targets --all-features` | clean |
| D08 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | clean without new allow/unsafe/unstable/macro |
| D09 | `just test-workspace` | clean |
| D10 | Tier 2 `spec_should_pass/check/051_collections_vec_array_methods.arcw` | current `Vec<i32>::with_capacity(4usize)` remains accepted through typed authority |
| D11 | focused Tier 2 canonical/turbofish fixtures | canonical dot, explicit generic path, and turbofish forms produce equal semantic candidate |
| D12 | repository structural audit | direct switch contains no compatibility alias, deprecated carrier, dual reader, fallback, source gate, signature-only resolver, or parallel HIR call owner; audit is human/module-graph evidence, not a helper-name acceptance test |

## 11. Completion gate

The slice is complete only when every row S01–S20, H01–H06, T01–T13, C01–C17, P01–P07, X01–X15, R01–R10, W01–W06, L01–L10, and D01–D12 passes and the implementation note records the exact commands and results. There are no deferred rows.
