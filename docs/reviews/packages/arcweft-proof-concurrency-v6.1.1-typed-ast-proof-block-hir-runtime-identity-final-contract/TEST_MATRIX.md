# Direct test matrix

## 1. Test construction rules

All tests in this matrix are required. Names are exact Rust test function names or exact `trybuild` fixture paths. A test may use a crate-owned `#[cfg(test)]` builder, allocator seed, corruption hook, or immutable state snapshot named in the row. It may not inspect checked-in source, documentation, path spellings, symbol spellings, or snippets to infer implementation shape.

Source fixtures are parsed through public or crate-owned parser APIs. ASCII fixtures with stated ranges assert the numeric half-open `SourceRange` exactly. Multi-case grammar tests keep a literal expected range beside each case rather than deriving the expected answer through production range search. Source-backed HIR tests obtain IDs by typed traversal or `*_for_syntax` APIs, never by integer construction.

Atomicity tests capture a crate-owned immutable `SyntaxDatabaseTestState` or `HirDatabaseTestState` immediately before the failing call and assert full equality afterward. The state includes current snapshot `Arc`, generation/revision, next IDs, live intervals/tombstones, arena page lengths, diagnostic state, cache epochs/facts, and pending invalidations.

## 2. Lossless tree and typed attachment

Owner: `crates/arcweft-lang-syntax/tests/lossless_typed_identity.rs`, except unit-only hooks in `incremental/*`.

| Exact test | APIs called | Input | Required result |
|---|---|---|---|
| `same_line_descendants_receive_distinct_syntax_ids` | `SyntaxDatabase::parse_initial`, `ParsedSource::tree`, typed child iterators, `AstNode::id`, `AstNode::range` | `predicate same((a, b): (Int, Int), c: Int) requires a < c ensures b < c = (a + c) == (b + c)\n` | Every identity-bearing item, fixed parameter group, parameter, tuple/identifier pattern, tuple/name type, clause, body, binary/call/path expression has a distinct ID even when kinds repeat on one physical line; every range equals its literal numeric range; tokens have no ID API. |
| `lossless_root_round_trips_every_utf8_byte` | `parse_initial`, `root_syntax().rowan().text()` | source containing multibyte identifiers, CRLF, blank lines, comments, doc comments, indentation, fence tokens, and malformed UTF-8-safe token text | Rowan root text bytes equal `SourceDocument::bytes()` exactly; `LosslessnessViolation` is not reported. |
| `trivia_only_reparse_preserves_predicate_proof_descendant_ids_and_updates_ranges` | `parse_initial`, `reparse`, typed traversal | predicate/proof fixture; insert spaces, blank comment, and doc-comment indentation without changing semantic tokens | Item, clauses, block, statements, tail, nested expression/type/pattern IDs compare equal with `is_same_reconciled_node`; `SyntaxSnapshotId` generation changes; affected ranges/spans match the new exact document; old handles retain old ranges. |
| `changed_grammar_node_gets_fresh_id_while_unchanged_siblings_survive` | `reparse`, typed traversal | change one literal in the first of two sibling statements | Changed literal and semantic ancestors whose shape changed receive fresh IDs; unchanged sibling subtree retains every ID. |
| `same_parent_unique_reorder_preserves_ids` | `reparse` | swap two distinct proof-call statements in one block | Both statement subtrees preserve IDs; order in typed statement iterator changes to source order. |
| `cross_parent_move_allocates_fresh_ids` | `reparse` | move a pure `let` statement from one block to another | Moved statement and descendants receive fresh IDs because semantic parent role changed; unaffected blocks retain IDs. |
| `copied_subtree_preserves_one_original_and_allocates_fresh_copy` | `reparse` | duplicate an identical assertion condition subtree under the same parent | Deterministic distance then old-ID tie keeps one original; additional copy and descendants receive fresh IDs; no collision. |
| `recovered_equivalent_node_survives_trivia_change` | `reparse` | malformed current expression with the same recovery class/role, plus trivia edit | Recovery node and missing child IDs survive; ranges update; parse remains `Recovered`. |
| `missing_child_identity_is_role_and_anchor_specific` | `parse_initial`, `reparse` | missing proof close brace and missing tail; then trivia before anchors | Missing close and omitted/missing-tail nodes have distinct IDs and zero-width ranges; exact same missing role/parent reconciles; changing expected class or parent allocates fresh ID. |
| `generic_error_nodes_are_distinct_and_deterministic` | `parse_initial`, `reparse` | two identical malformed statements in one block | Both `ErrorStatement` nodes receive different IDs; no-op/trivia reparse maps each by deterministic sequence/distance rules. |
| `typed_to_rowan_and_rowan_to_typed_round_trip` | `AstNode::syntax`, `ParsedSource::bind_rowan`, `SyntaxNodeHandle::cast` | complete predicate/proof fixture | For every typed identity-bearing node: typed ID -> Rowan -> bound handle -> typed cast returns equal handle; no range/text search occurs. |
| `wrong_typed_kind_returns_kind_mismatch` | `ParsedSource::typed_node::<K>` | request `ProofItemKind` for a predicate item ID | `SyntaxLookupError::KindMismatch` with exact ID, expected kind, and actual kind. |
| `stale_generation_current_resolution_is_typed_error` | `SyntaxDatabase::resolve_current` | old typed handle after successful changing reparse that retired its node | `SyntaxLookupError::StaleGeneration` with the exact current and supplied generations; the resolver rejects the stale snapshot before retired-node or slot lookup. |
| `exact_snapshot_operation_rejects_wrong_snapshot` | `ParsedSource::resolve_exact` and `resolve_exact_syntax` with a typed/Rowan handle from another generation | two snapshots in one lineage | `SyntaxLookupError::WrongSnapshot` with exact expected/actual snapshot IDs before slot lookup. Stable-ID lookup by `syntax_node(id)` remains receiver-snapshot lookup and is tested separately. |
| `independent_databases_cannot_resolve_equal_raw_slots` | two `SyntaxDatabase::try_new`, `bind_rowan`, `typed_node` | identical `SourceName`, `SourceDocument`, and bytes in each database | IDs differ by `SyntaxDatabaseId`; cross-use returns `WrongDatabase`, even when internal slot ordinals are both one. |
| `foreign_lineage_in_same_database_is_rejected` | `parse_initial` twice, lookup | two source lineages | `WrongLineage` with exact expected/actual lineage IDs. |
| `syntax_no_op_returns_exact_arc_and_consumes_nothing` | `reparse`, `ParsedSource::is_same_snapshot` | empty edits and byte-identical replacement | `is_same_snapshot` is true; same generation, diagnostics, IDs, stats, and database state. |
| `fatal_event_validation_failure_is_atomic` | crate-owned malformed-event hook, transaction state | unbalanced marker/event sequence | `ParseFailure::Invariant` or `LosslessnessViolation` as seeded; state unchanged; next valid parse receives the same next node IDs as a control database. |
| `fatal_attachment_failure_is_atomic` | crate-owned attachment mismatch hook | event kind intentionally disagrees with typed attachment tag | `ParseFailure::Attachment`; no generation/tree/identity/diagnostic/cache commit; control-ID equality on next valid parse. |
| `syntax_identity_exhaustion_is_atomic` | allocator seed hook | next node slot at maximum, parse requiring one more node | `ParseFailure::NodeIdentityExhausted`; no ID consumed or snapshot published. |

### Compile-fail syntax ownership tests

Owner: `crates/arcweft-lang-syntax/tests/ui.rs`.

| Fixture | Required compile failure |
|---|---|
| `tests/ui/unbound_fragment_is_not_parsed_source.rs` | `UnboundFragment<ExpressionFragment>` cannot satisfy `LoweringRequest` or any source-file lowering input. |
| `tests/ui/attached_fragment_is_not_source_file.rs` | `AttachedFragment<K>` cannot satisfy `LoweringRequest`. |
| `tests/ui/syntax_node_id_has_no_raw_constructor.rs` | raw fields/constructors are private. |
| `tests/ui/syntax_session_ids_are_not_serde.rs` | syntax database/lineage/snapshot/node IDs do not implement `Serialize` or `Deserialize`. |
| `tests/ui/typed_node_constructor_is_private.rs` | callers cannot forge `AstNode<K>` or change its snapshot/ID/kind. |

## 3. Predicate, proof, and `ProofBlock`

Owner: `crates/arcweft-lang-syntax/tests/predicate_proof.rs`, semantic-context rows in `crates/arcweft-lang-sema/tests/predicate_proof_context.rs`.

| Exact test | APIs/input | Required result |
|---|---|---|
| `predicate_proof_complete_header_grammar_matrix` | `parse_initial` over table cases: no visibility, `pub`, `pub(crate)`, `pub(super)`; generic lifetime/type params; exactly one parameter group; typed destructuring; zero/many where predicates; requires before ensures; expression/block bodies | Each case yields exactly one attached `PredicateItem` or `ProofItem` with the expected wrappers and ranges and no duplicate typing. A second fixed parameter group is ordinary malformed-header recovery and makes the item non-executable. |
| `predicate_has_implicit_bool_and_rejects_authored_arrow` | `predicate p(x: Int) -> Bool = x > 0\n` | attached predicate retains a recovered `ReturnType` node over range `20..27`, reports `syntax.predicate.return_not_allowed`, has semantic return `Bool`, and is non-executable. |
| `proof_omitted_return_is_unit` | `proof p() = ()\n` and `proof p() {}\n` | both signatures expose resolved `Unit`; expression body retains authored Unit expression; empty block has omitted typed tail and later one synthetic Unit HIR tail. |
| `proof_non_unit_expression_body_is_typed_once` | `proof p() -> Int = 1\n` | `ProofBody::form()` is `ProofBodyForm::Expression`; the body-wrapper ID and expression-body ID are distinct; the expression node range is the authored `1`; no string payload or second parse. |
| `proof_non_unit_block_requires_tail` | `proof p() -> Int { let x: Int = 1; }\n` | block has `BlockTail::Omitted` at the exact zero-width anchor before `}`; syntax remains attached, HIR marks the item recovered/non-executable, and semantic diagnostics identify the required non-Unit tail. |
| `requires_must_precede_ensures` | one ensures followed by requires | both clauses remain attached in source order; latter reports `syntax.contract.invalid_clause_order`; item non-executable. |
| `predicate_proof_total_clause_limit_counts_both_kinds` | test limits/default fixture | 64 combined clauses commit; 65 returns `ParseFailure::LimitExceeded(SyntaxLimit::ContractClauses)` with full syntax rollback. |
| `ordinary_names_share_one_namespace_without_overloading` | two modules with function/predicate/proof names, imports and aliases | parser produces ordinary names; `ProjectSymbolTable` test later reports duplicate ordinary names; no signature overload set or authored artifact ID. |
| `predicate_and_proof_recursion_sccs_are_rejected` | sema `type_check_project` over self-recursive and mutually recursive fixtures | deterministic semantic diagnostics on callable names; no runtime/verifier executable facts. Calls remain typed HIR, not parser errors. |
| `expression_body_and_one_expression_block_are_observably_distinct` | `proof a() = 1\n` and `proof b() { 1 }\n` | first has `ProofBodyForm::Expression` plus distinct `ProofBody`/`ExpressionBody` identities; second has `ProofBodyForm::Block` plus distinct body-wrapper, block, open, close, and tail identities. They never compare structurally equal as bodies. |
| `proof_block_exact_shapes_and_ranges` | `proof p() -> Int { let x: Int = 1; assert.prove(x == 1); x }\n` | item `0..60`; block `17..60`; open brace `17..18`; let stmt `19..34`; assertion stmt `35..56`; condition `48..54`; authored tail `57..58`; close brace `59..60`. `ProofStmt` order is Let then Assertion; tail is separate. |
| `predicate_block_exact_shapes_and_ranges` | `predicate p(x: Int) { let y: Int = x; y > 0 }\n` | item `0..45`; block `20..45`; let `22..37`; Boolean tail `38..43`; predicate block accepts no assertion statement. |
| `empty_block_has_distinct_braces_and_omitted_tail` | `proof unit() {}\n` | block `13..15`, open `13..14`, close `14..15`, omitted tail zero-width `14..14`; the body wrapper plus block/open/close/omitted-tail nodes are five distinct IDs. |
| `one_expression_block_retains_authored_tail_identity` | `proof unit() { 1 }\n` | block `13..18`; authored tail expression range `15..16`; no statement; no synthetic syntax tail. |
| `pure_let_initializer_precedes_binding_scope` | proof block `let x: Int = x; x` with an outer `x` | typed `PureLetStmt` owns pattern/type/initializer once; HIR lookup in initializer resolves outer `x`; tail resolves new `x`. |
| `proof_call_statement_uses_existing_call_expression` | proof block with `lemma(x);` | `ProofStmt::ProofCall(ProofCallStmt)` wraps the attached existing `CallExpr`; no proof-call string or line-plan clone. |
| `assert_prove_uses_existing_assertion_authority` | proof block with two conditions | `ProofStmt::Assertion` wraps the existing typed `AssertionStmt`, mode `Prove`, conditions in authored order, exact ranges/IDs. |
| `predicate_assertion_is_context_error_not_reparse` | predicate block with `assert.prove(x)` | syntax attachment succeeds with existing typed assertion node inside recovered predicate statement; sema reports predicate-context error; item non-executable. |
| `proof_runtime_assertions_are_context_errors` | proof block with `assert.check(x)` and `assert.debug(x)` | existing typed assertion modes survive; sema emits proof-context errors; runtime-plan receives no guards. |
| `malformed_header_recovery_keeps_following_declaration` | cases for missing name, malformed generic, missing parameter close, malformed where, missing body | exact diagnostic codes from `API_AND_DIAGNOSTICS.md`; missing nodes have zero-width role anchors; next top-level declaration parses with its own ID. |
| `missing_block_close_uses_zero_width_delimiter_node` | `proof broken() -> Int { let x = ;\nproof next() = ()\n` | broken item ends at synchronization before byte 34; missing close is zero-width at 34; next proof range `34..51`; no tokens from next declaration are absorbed. |
| `malformed_statement_and_tail_are_poisoned_but_queryable` | block with missing initializer and malformed final expression | attached error/missing nodes, ordered diagnostics, `Recovered`; typed/HIR tooling resolves poison; executable view rejects module. |
| `removed_forms_use_ordinary_current_grammar_recovery` | one fixture each for removed ownership block form, entity-style proof header, removed trusted declaration, removed clause keyword, and removed calculation form followed by `proof next() = ()` | only ordinary current diagnostic families (`syntax.item.unexpected_token`, `syntax.statement.unexpected_token`, or current header errors); no historical node/code; following proof parses and can become executable. Test invokes parser APIs only and never searches repository text. |

### Inclusive syntax limit tests

Owner: `crates/arcweft-lang-syntax/tests/limits_predicate_proof.rs`. Every test captures `SyntaxDatabaseTestState`, accepts the exact maximum, fails one over, asserts the precise `SyntaxLimit`, asserts unchanged state, then compares next-valid IDs with a control database.

- `predicate_parameter_limit_is_inclusive_and_atomic` — 64/65.
- `proof_parameter_limit_is_inclusive_and_atomic` — 64/65.
- `generic_parameter_limit_is_inclusive_and_atomic` — 256/257.
- `where_predicate_limit_is_inclusive_and_atomic` — 256/257.
- `contract_clause_limit_is_inclusive_and_atomic` — 64/65 total.
- `statement_limit_is_inclusive_and_atomic` — 65,536/65,537, using lowered test limit 1/2 in the fast unit variant and one production-constant construction test.
- `expression_limit_is_inclusive_and_atomic` — 262,144/262,145, same fast/full split.
- `type_limit_is_inclusive_and_atomic` — 131,072/131,073.
- `pattern_limit_is_inclusive_and_atomic` — 131,072/131,073.
- `diagnostic_limit_is_inclusive_and_atomic` — 1,024/1,025 after exact deduplication.
- `identity_bearing_node_limit_is_inclusive_and_atomic` — 1,048,576/1,048,577.

The production-constant constructions are marked ignored only in local focused runs and are executed by the final cut-point suite; lowered-limit unit variants always run.

## 4. HIR arenas, liveness, scopes, locals, and captures

Owner: `crates/arcweft-lang-hir/tests/arena_lowering.rs`, scope-specific rows in `tests/scopes_locals_captures.rs`, corruption/exhaustion unit hooks beside allocators.

| Exact test | APIs/input | Required result |
|---|---|---|
| `every_source_backed_node_maps_to_exact_hir_kind` | `HirDatabase::lower`, `item_for_syntax`, `expr_for_syntax`, `stmt_for_syntax`, `type_for_syntax`, `pattern_for_syntax`, resolvers | Every attached source node that lowers has exactly one expected typed HIR ID; requesting another kind returns `HirSourceLookupError::KindMismatch`; no cloned syntax value is stored. |
| `same_line_hir_nodes_do_not_collide` | same-line predicate fixture | all same-kind expressions/types/patterns/statements resolve to distinct typed IDs and metadata source keys. |
| `trivia_relower_returns_stable_source_ids_with_new_spans` | syntax trivia reparse then HIR lower | source-key-matched slots remain the same typed IDs; new snapshot metadata returns new revision-bound spans; old snapshot returns old spans. |
| `changed_source_kind_retires_old_slot_and_allocates_new_kind` | change expression node into pattern/type role through valid grammar edit | old typed ID resolves in old snapshot, current returns `Retired`; new kind gets fresh ID; no raw-slot reinterpretation. |
| `same_parent_reorder_preserves_hir_ids` | reordered statements whose syntax IDs survive | HIR IDs survive and module statement order changes. |
| `cross_parent_move_retires_and_reallocates_hir_ids` | move source subtree to another block | source syntax IDs and HIR IDs are fresh; old IDs remain live only in old snapshot. |
| `copied_source_node_gets_fresh_hir_ids` | duplicate source subtree | one reconciled original ID/HIR slot; copy gets fresh syntax and HIR IDs. |
| `recovered_source_commits_poisoned_hir_for_tooling` | recoverable malformed proof | `HirModuleStatus::Recovered`; all recoverable IDs resolve with poison metadata/diagnostics; `is_executable` false. |
| `synthetic_roles_are_stable_and_collision_free` | omitted Unit tail, predicate Bool return, omitted proof Unit return, contract scopes, closure capture, elided region, recovery operand, postcondition result, desugared temporary | IDs use exact `(owner, role, ordinal)` keys; ordinals stable under unrelated sibling edits; distinct roles/ordinals never collide; anchors are prescribed zero-width source spans. |
| `old_snapshot_resolves_live_interval` | three revisions: born, live, retired | old snapshots in interval resolve; pre-birth returns `NotYetLive`; retirement/current returns `Retired` with exact revisions. |
| `wrong_module_is_checked_before_slot` | ID from another module/database | `IdResolveError::WrongModule`; no page index or kind access. |
| `wrong_kind_corruption_hook_never_panics` | crate-owned slot-kind corruption hook | typed resolver returns `KindMismatch { expected, actual }`; process does not panic. |
| `cross_syntax_database_lowering_is_rejected_atomically` | `LoweringRequest` from foreign syntax database | `HirLowerFailure::WrongSyntaxDatabase`; `HirDatabaseTestState` unchanged. |
| `stale_syntax_snapshot_lowering_is_rejected_atomically` | lower current then submit older parsed snapshot | `HirLowerFailure::StaleSource`; no revision/tombstone/slot/cache mutation. |
| `hir_no_op_returns_exact_arc_and_no_invalidation` | lower exact same request/schema twice, inspect `HirLowerOutput` | `Arc::ptr_eq(first.module(), second.module())`; no revision increment; `second.invalidations().is_empty()`; all database state equal. |
| `root_and_nested_scope_kinds_are_allocated_exactly` | function/flow/predicate/proof/block/match/loop/conditional/closure/contracts fixture | one root plus exact child `HirScopeKind`/owner/parent/children relationships in source traversal order. |
| `let_initializer_uses_pre_binding_scope` | outer `x`; inner `let x = x` | initializer path resolves outer `LocalId`; following expression resolves new generation. |
| `destructuring_binds_left_to_right_after_initializer` | nested tuple/struct pattern | locals allocated depth-first left-to-right after initializer; all share statement binding point and deterministic generations. |
| `duplicate_pattern_names_poison_all_duplicate_bindings` | `(x, x)` | one duplicate-name diagnostic; both occurrences retain pattern IDs; the first local remains the single lookup winner and each later duplicate receives its own poisoned, non-winning local; no panic. |
| `underscore_allocates_no_local` | patterns containing `_` | pattern node exists; scope local inventory has no underscore local/capture. |
| `poisoned_pattern_does_not_leak_names` | malformed destructuring | poisoned pattern queryable; no partial name becomes visible outside its committed valid subpattern rules. |
| `sequential_shadowing_increments_local_generation` | three sequential `let x` statements | same scope/name generations 1,2,3; each use resolves nearest preceding binding. |
| `mutable_binding_and_mutable_reference_remain_distinct` | `let mut x`, `let r: &mut Int`, assignments/dereference | local mutability flag only on `x` binding; mutable-reference behavior uses existing `BorrowKind`/type semantics, not local mutability. |
| `closure_capture_order_is_first_use_then_local_id` | closure repeats outer names in mixed order | one capture per local; order by first source use then `LocalId`; repeated uses reuse capture; access escalates deterministically. |
| `closure_parameter_and_inner_shadow_prevent_capture` | closure parameter shadows outer, then inner let shadows parameter | no capture for shadowed name; uses resolve correct local generations. |
| `if_let_match_while_let_for_scopes_match_contract` | one fixture for each control form | binding visibility begins only in specified body/arm/guard region; no leakage to sibling arms, else, loop condition predecessor, or after loop. |
| `postcondition_result_is_ensures_only` | non-Unit callable with requires/body/ensures | synthetic result local visible in ensures contract scope only; absent from requires/body and Unit returns. |
| `typed_child_beats_disagreeing_display_source` | crate-owned typed builder whose non-authoritative display/source text says a different literal/operator | direct lowering follows typed child IDs/kinds/values; result proves no string reparse. Builder is crate-owned and does not scan source. |
| `recovered_module_is_excluded_from_executable_caches` | recovered HIR module into sema/verifier/runtime-plan/codegen cache APIs | tooling query succeeds; executable cache insertion returns typed recovered-module error; cache state unchanged. |

### Inclusive HIR limit and exhaustion tests

Owner: `crates/arcweft-lang-hir/tests/limits_atomicity.rs`. Each test uses production constants plus fast seeded limits, compares `HirDatabaseTestState`, and verifies the next valid transaction receives control IDs.

- `module_limit_is_inclusive_and_atomic` — 65,536/65,537.
- `item_limit_is_inclusive_and_atomic` — 16,384/16,385.
- `scope_limit_is_inclusive_and_atomic` — 16,384/16,385.
- `statement_limit_is_inclusive_and_atomic` — 65,536/65,537.
- `expression_limit_is_inclusive_and_atomic` — 262,144/262,145.
- `type_limit_is_inclusive_and_atomic` — 131,072/131,073.
- `pattern_limit_is_inclusive_and_atomic` — 131,072/131,073.
- `local_module_limit_is_inclusive_and_atomic` — 65,536/65,537.
- `local_scope_limit_is_inclusive_and_atomic` — 4,096/4,097.
- `capture_limit_is_inclusive_and_atomic` — 65,536/65,537.
- `hir_diagnostic_limit_is_inclusive_and_atomic` — 1,024/1,025 after ordering/dedup.
- `synthetic_descendant_limit_is_inclusive_and_atomic` — 1,024/1,025 for one owner.
- `total_slot_limit_is_inclusive_and_atomic` — 786,432/786,433.
- `module_identity_exhaustion_is_atomic` — seeded last module slot.
- `revision_exhaustion_is_atomic` — seeded `HirRevision::MAX` current module.
- `slot_identity_exhaustion_is_atomic` — seeded last slot for each `HirIdKind` through table cases.
- `local_generation_exhaustion_is_atomic` — seeded last generation for one scope/name.
- `cache_epoch_exhaustion_is_atomic` — seeded last invalidation epoch.

## 5. Module-preserving project and unified symbols

Owner: `crates/arcweft-lang-hir/tests/project_symbols.rs`, compiler integration in `crates/arcweft-compiler/tests/module_preserving_project.rs`.

| Exact test | APIs/input | Required result |
|---|---|---|
| `ordered_project_iteration_preserves_module_ids` | `HirProject::try_new`, `view().modules()`, `items()` | canonical path order; each `HirSnapshotId`, `ItemId`, child ID equals the originating module value; no rebasing or clone. |
| `project_module_rejects_package_mismatch` | `HirProjectModule::try_new` | exact `HirProjectError::PackageMismatch`; input `Arc<HirModule>` unchanged. |
| `project_module_rejects_path_mismatch` | same | `ModulePathMismatch`. |
| `project_module_rejects_source_mismatch` | same | `SourceDocumentMismatch`. |
| `project_requires_canonical_root_module` | `HirProject::try_new` without crate root | `HirProjectError::MissingRootModule`; no partial project exists. |
| `project_rejects_duplicate_path_and_source` | `HirProject::try_new` | `DuplicateModulePath` or `DuplicateSourceDocument` in canonical input order; no partial project. |
| `project_view_allows_recovered_but_executable_view_rejects_first_canonical` | clean and two recovered modules | `view` includes all; `executable_view` returns first canonical `RecoveredModule`. |
| `exported_parts_iterate_without_flattening` | project with exported parts in multiple modules | iterator yields `(module, original ItemId, borrowed record)` in canonical module/source order. |
| `styles_iterate_without_flattening` | style records in multiple modules | original module/style IDs preserved; no linked module. |
| `one_symbol_table_registers_all_callable_kinds_and_character` | `ProjectSymbolTable::link` | function, predicate, proof, and Character declarations appear in one revision-bound table; `CallableDeclarationOwner` and external Character symbol ownership are exact. |
| `ordinary_callable_duplicate_names_are_reported_together` | same module with function/predicate/proof same name | one deterministic duplicate-name diagnostic containing all source sites in span order; no overload set. |
| `visibility_import_alias_and_qualification_are_uniform` | public/crate/super/private callables with path, glob, group, aliases | same existing visibility/import rules for all callable kinds; inaccessible/ambiguous/escalation errors are typed and revision-bound. |
| `symbol_table_revision_invalidates_exact_changed_modules` | change one module declaration/import | table revision advances; changed and dependent modules invalidated; unrelated module cache facts preserved. |
| `proof_artifact_id_is_session_only_and_snapshot_bound` | registered proof in two HIR snapshots | `ProofArtifactId` derives from existing `CallableDeclarationId`, `HirSnapshotId`, `ItemId`; IDs differ across changed snapshot and have no textual/Serde codec. |
| `compiled_project_contains_no_linked_hir` | full compiler public product | only `HirProject`, per-module sema/runtime products, symbol table, and session assertion capability; no flattened module accessor. |

### Compile-fail project/HIR tests

Owner: HIR and compiler `tests/ui.rs`.

- `crates/arcweft-lang-hir/tests/ui/no_linked_module.rs` — `HirProject::linked_module` absent.
- `crates/arcweft-lang-hir/tests/ui/no_append_module_body.rs` — `HirModule::append_module_body` absent.
- `crates/arcweft-compiler/tests/ui/no_linked_hir.rs` — `CompiledProject::linked_hir` absent.
- `crates/arcweft-lang-hir/tests/ui/no_provisional_proof_types.rs` — old `ProofClause`, trusted declaration, and authored proof-ID types unavailable from final public modules.
- `crates/arcweft-lang-hir/tests/ui/hir_ids_have_no_raw_constructors.rs` — every typed HIR ID raw constructor/field private.
- `crates/arcweft-lang-hir/tests/ui/hir_session_ids_are_not_serde.rs` — HIR IDs, snapshots, proof artifacts fail Serde bounds.

## 6. Runtime assertion fault and serialization

Owners: `crates/arcweft-runtime-plan/tests/assertion_identity.rs`, core/codec tests in their owning crates, presentation integration in compiler/CLI/LSP/Agent tests.

| Exact test | APIs/input | Required result |
|---|---|---|
| `check_failure_retains_exact_session_identity` | lower executable `assert.check(a, b)`; project first failure | fault identity contains exact assertion `StmtId`, index 0, `RuntimeAssertionMode::Check`, exact first condition `SourceSpan`; presentation has statement span/label separately. |
| `enabled_debug_failure_retains_exact_session_identity` | debug-profile lowering of `assert.debug(a, b)` | same for index 1 and `Debug`; inventory and guard exist only in enabled profile. |
| `condition_indices_follow_authored_zero_based_order` | 64 typed conditions | site indices are exactly 0..63 in authored order; guards unique; no sort by expression ID/text. |
| `condition_index_validation_rejects_invalid_count_and_bounds` | `try_new(0,0)`, `(0,65)`, `(64,64)`, `(63,64)` | first two `InvalidConditionCount`, third `OutOfBounds`, last succeeds with `get()==63`. |
| `prove_has_no_runtime_mode_or_guard` | `RuntimeAssertionMode::try_from_assertion_mode(Prove)` and proof HIR | `ProveHasNoRuntimeRepresentation`; runtime plan/inventory emit zero entries; no public fault/site constructor accepts prove. |
| `release_plan_omits_debug_evaluation_and_inventory` | build debug and release plan from same HIR | release has no Debug condition instruction, core assertion, guard, site, or cache fact; Check entries retain stable authored ordering. |
| `guard_derivation_uses_typed_seed_and_is_deterministic` | same canonical package/module/callable/ordinal/index/profile in two sessions with the same runtime-plan `ArtifactKey` | equal 16-byte guard despite different `StmtId`; changing any seed field changes guard; output nonzero. Test invokes derivation API, not message/source parsing. |
| `invalid_guard_and_fingerprint_zero_values_are_rejected` | checked byte constructors | `RuntimeIdentityDecodeError` for all-zero arrays; valid fixed arrays round trip. |
| `runtime_fault_invalid_guard_is_typed_error` | core failure guard absent from inventory | `RuntimeAssertionProjectionError::UnknownGuard`; no guessed source/message identity. |
| `runtime_fault_artifact_mismatch_is_typed_error` | session capability fingerprint differs | `ArtifactMismatch`; no old/fresh identity association. |
| `runtime_assertion_core_codec_has_no_session_identity` | JSON/CBOR/MsgPack/binary core codec round trips | guard, condition, message, profile persist; the runtime artifact fingerprint equals the existing runtime-plan `ArtifactKey` digest bytes; encoded schema contains no `StmtId`, `HirSnapshotId`, syntax ID, module slot, or session fault type. Assert decoded typed fields, not serialized text search. |
| `awbc_bundle_save_checkpoint_cache_round_trip_without_session_ids` | owning typed round-trip APIs for AWBC, bundle, save/checkpoint, compile cache | persisted values round trip and expose no session-ID typed field; fresh session inventory is supplied separately. Compile-time trait tests prove session IDs cannot enter codec generics. |
| `core_dependency_graph_excludes_compiler_layers` | parsed `cargo metadata` graph | no normal/dev/target path from `arcweft-core` to syntax, HIR, sema, runtime-plan, compiler, CLI, or LSP; test reads metadata graph objects, not manifests as text. |
| `runtime_host_normal_graph_excludes_hir_and_runtime_plan` | metadata graph | no normal edge/path from runtime-host to syntax/HIR/compiler/runtime-plan; existing development test edge may remain. |
| `runtime_projection_emits_stable_diagnostic_without_message_parsing` | `RuntimeAssertionInventory::project_failure`, CLI/LSP/Agent/debug renderers | code exactly `runtime.assertion_failed`; primary label exact condition span; secondary statement span; observed message displayed but never used for lookup. |
| `reloaded_artifact_uses_fresh_inventory_without_old_stmt_equality` | persist/load runtime artifact, rebuild compiler session from matching exact sources | same guard/fingerprint can resolve to a fresh `StmtId`; test asserts old and fresh IDs are not claimed equal and only fresh fault identity is returned. |
| `reloaded_artifact_without_exact_source_association_stays_unassociated` | load artifact without matching compiler session or with source identity mismatch | stable runtime diagnostic from persisted source map/data only; no fabricated HIR/source-session identity. |

### Compile-fail runtime identity tests

- `crates/arcweft-runtime-plan/tests/ui/runtime_fault_has_no_public_constructor.rs`.
- `crates/arcweft-runtime-plan/tests/ui/runtime_session_identity_is_not_serde.rs`.
- `crates/arcweft-core/tests/ui/core_cannot_name_hir_ids.rs`.
- `crates/arcweft-core/tests/ui/prove_is_not_runtime_assertion_mode.rs`.

## 7. Tooling, formatter, recovery, and deletion

| Exact test and owner | Required result |
|---|---|
| `formatter_preserves_lossless_predicate_proof_nodes` — syntax formatter tests | formatter consumes grammar/typed APIs; formatting/reparse remains lossless and yields valid final predicate/proof nodes with no raw-body parsing. |
| `lsp_navigation_uses_typed_syntax_and_module_hir_ids` — `arcweft-lsp` | definition/hover/rename/source labels round-trip attached syntax and per-module HIR IDs; recovered nodes remain queryable. |
| `cli_diagnostics_render_exact_revision_spans` — `arcweft-cli` | syntax/HIR/runtime diagnostics point at exact current `SourceSpan`; stale handles return typed errors. |
| `agent_runtime_assertion_projection_uses_session_capability` — `arcweft-agent-repl`/tooling | Agent output uses `runtime.assertion_failed` and inventory; no message parse. |
| `verifier_consumes_predicate_proof_arena_records` — `arcweft-verify` | verifier sees final typed clauses/body IDs; no provisional string/old clause input. Proof discharge remains outside this cut. |
| `runtime_plan_consumes_project_view_without_flattening` — runtime-plan/compiler | per-module lowering preserves IDs; no linked module path exists. |
| `malformed_removed_form_does_not_hide_following_current_declarations` — syntax integration | each ordinary malformed fixture followed by valid function/predicate/proof parses the following declarations; no removed-spelling recognizer or historical diagnostic. |
| `recovered_module_never_enters_runtime_plan_or_compile_cache` — compiler/tooling | tooling reads it; runtime/codegen/cache APIs reject it atomically. |
| `public_api_surface_contains_only_final_nodes` — compile-fail suites | old public proof/trusted/raw-session/linked APIs fail to compile. |

No audit test greps source, docs, file paths, or symbol spellings. Final absence is proved through public API compile failures, parser behavior, dependency graph objects, and direct runtime/codec behavior.
