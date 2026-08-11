# Test matrix

## 1. Test principles

All tests use typed constructors and exact typed APIs. No test discovers implementation behavior by scanning production source text. Dependency-direction evidence comes from Cargo metadata. Display output may be asserted only as presentation evidence after typed identity assertions.

The matrix is normative. Test names may be adjusted to local module naming conventions, but each setup and assertion must remain represented by one direct test or one parameterized row with failure localization.

## 2. Syntax owner tests

| ID | Suggested owner/test | Setup | Required assertions |
|---|---|---|---|
| S-01 | `arcweft-lang-syntax::ast::symbol_path::project_symbol_segment_accepts_external_hyphen` | construct `hero-pack` and path `character.hero-pack` | ordered segments are exactly `character`, `hero-pack`; conversion never classifies `hero-pack` as `ModuleSegment` |
| S-02 | `...::project_symbol_segment_rejects_empty_control_and_separators` | empty, `a.b`, `a:b`, `a/b`, `a\\b`, control character | each fails at `ProjectSymbolSegment::try_new`; no path object exists |
| S-03 | `...::project_symbol_path_rejects_empty_and_invalid_implicit_root` | zero segments; first segment `2d`; later segment `2d` | empty and invalid first fail; later numeric/external segment remains allowed |
| S-04 | retain existing `accepts_external_segments_and_records_exact_ranges` | parse `character.hero-pack.2d` at nonzero base | typed segments and exact ranges remain unchanged |
| S-05 | retain existing `project_paths_convert_to_module_qualified_or_external_root_symbols` | ordinary and external-only qualifier paths | current `SymbolPath` conversion behavior is unchanged |

## 3. Direct-binding owner tests

| ID | Suggested owner/test | Setup | Required assertions |
|---|---|---|---|
| H-01 | `arcweft-lang-hir::symbol::project_direct_binding_retains_exact_typed_path` | construct path `['character','akane']` | `path()` returns the same root/ordered segments; visibility/source/alias flag unchanged |
| H-02 | `...::project_direct_binding_rejects_explicit_root` | valid path with `Crate`, `SelfModule`, and `Super(1)` roots | each returns `ProjectDirectBindingError::ExplicitRoot` with exact root |
| H-03 | `...::external_seed_keeps_canonical_and_binding_identity_distinct` | canonical opaque leaf `character.akane`; qualified/compact/hero paths | seed canonical path remains opaque; three direct paths remain distinct and target one seed |
| H-04 | `...::external_seed_deduplicates_only_exact_direct_bindings` | duplicate qualified binding plus compact binding | duplicate qualified row deduplicates; compact row remains |

## 4. Character producer tests

| ID | Suggested owner/test | Setup | Required assertions |
|---|---|---|---|
| C-01 | `arcweft-project-loader::environment::character_registration_retains_qualified_and_compact_paths` | load owner `character.akane` | direct paths are exactly `['character','akane']` and `['akane']`; canonical path remains opaque `character.akane` |
| C-02 | `arcweft-lang-sema::registration::character_qualified_compact_and_authored_alias_resolve_same_external` | facts include qualified, compact, and direct authored `hero` binding | `ProjectSymbolTable::resolve` for all three paths returns the same `ExternalDeclarationId` |
| C-03 | same test or dedicated typed catalog test | build registered catalog | project map contains all three exact `ProjectCallablePath` keys with equal character non-callable type and same external owner evidence |
| C-04 | `...::character_registration_never_requires_module_identifier_segments` | owner contains valid `hero-pack` component | registration succeeds; segment remains `hero-pack`; no module path is manufactured |
| C-05 | `...::malformed_character_binding_path_preserves_previous_accepted_world` | inject a test-only malformed producer attempt before candidate registration | constructor returns typed error and accepted pointer/generation remain unchanged |

C-02 and C-03 together satisfy the mandatory `character.akane`, compact `akane`, and authored alias `hero` case.

## 5. Adapter producer tests

| ID | Suggested owner/test | Setup | Required assertions |
|---|---|---|---|
| A-01 | `arcweft-adapter-context::symbol::adapter_symbol_path_validates_segments` | `adapter.viewport`, `adapter.hero-pack` | exact ordered typed segments retained; `-` accepted |
| A-02 | `...::adapter_symbol_path_rejects_empty_control_separator_and_invalid_root` | empty path/component, controls, `.`, `:`, `/`, `\\`, first `2d` | owning constructor returns exact `AdapterSymbolPathError`; no `AdapterSymbol` constructed |
| A-03 | `arcweft-adapter-context::codec::adapter_symbol_name_decodes_directly_to_typed_path` | schema-v1 JSON/TOML `name = "adapter.viewport"` | manifest stores two typed segments; no alternate field/version is required |
| A-04 | codec malformed table test | `adapter..viewport`, separator/control/root-invalid values | codec returns `AdapterManifestCodecError::SymbolPath` with typed cause |
| A-05 | `arcweft-adapter-context::manifest::source_backed_facts_publish_typed_qualified_symbol` | typed `AdapterSymbolPath(['adapter','viewport'])` | generated source/canonical ID are `adapter.viewport`; direct binding path has two typed project segments |
| A-06 | `arcweft-lang-sema::callable::adapter_qualified_non_callable_is_catalogued_without_split` | register adapter symbol `adapter.viewport` | catalog contains path segments `adapter`, `viewport`; sema receives no adapter string/path parser input |
| A-07 | `...::adapter_symbol_fact_order_is_deterministic` | reverse symbol insertion | generated registration facts, HIR scope iterator rows, and resulting project binding catalog are equal |

A-05 and A-06 satisfy the mandatory adapter-owned qualified binding case.

## 6. Linker path-retention tests

| ID | Suggested owner/test | Setup | Required assertions |
|---|---|---|---|
| L-01 | `arcweft-lang-hir::symbol::direct_external_paths_survive_linking` | qualified, compact, hero paths in one seed | typed iterator emits all exact paths and same external target |
| L-02 | `...::unaliased_import_uses_typed_destination_leaf` | `use character.akane` | source root retains qualified binding; importer receives exactly `['akane']` |
| L-03 | `...::explicit_alias_uses_typed_alias_segment` | `use character.akane as hero` | importer receives exactly `['hero']`; target unchanged |
| L-04 | `...::grouped_import_preserves_selected_and_alias_segments` | grouped selected names and one alias | destination paths equal typed selected/alias tokens |
| L-05 | `...::glob_preserves_exact_source_path` | glob a scope containing qualified external path | destination receives exact multi-segment path, not one rendered leaf |
| L-06 | `...::reexport_fixed_point_preserves_path_segments` | multi-hop public re-export | every fixed-point hop retains exact ordered segments; visibility/source behavior unchanged |
| L-07 | `...::external_only_qualifier_import_retains_full_typed_binding` | implicit `character.hero-pack.2d` direct binding/import | unaliased destination retains all segments; resolution uses current opaque conversion |
| L-08 | retain/import behavior regression | inaccessible, visibility escalation, ambiguous, unknown imports | exact existing diagnostics/omission behavior and alias/work limits remain unchanged |
| L-09 | `...::scope_binding_coalescing_requires_equal_path` | same target under qualified and compact paths, plus duplicate qualified site | qualified sites coalesce; compact remains separate |
| L-10 | `...::scope_binding_iterator_is_insertion_order_independent` | reverse modules, seeds, direct bindings, and import visitation fixture order where APIs permit | emitted `(module,path,target)` sequence is identical |

L-02 through L-06 satisfy the mandatory import, re-export, glob, and explicit alias preservation case.

## 7. Mixed-scope determinism test

| ID | Suggested owner/test | Setup | Required assertions |
|---|---|---|---|
| M-01 | `arcweft-lang-hir::symbol::qualified_callable_module_and_external_bindings_are_distinct` | one scope exposes: a source callable whose full `ProjectCallablePath` is qualified by its module, a child module binding, and an external qualified local path | iterator rows and catalog keys are distinct; target kinds are `Callable`, `Module`, `External`; repeated/reversed construction gives identical order and map equality |

The phrase “qualified callable” is tested through its existing `ProjectCallablePath { package, module, path }` identity. The correction does not invent a dotted local name for source callables.

## 8. Catalog publication tests

| ID | Suggested owner/test | Setup | Required assertions |
|---|---|---|---|
| P-01 | `arcweft-lang-sema::callable::project_binding_builder_publishes_every_typed_segment_path` | HIR iterator contains one- and multi-segment callable/module/external rows | one project binding is pushed for every iterator row; no row is silently omitted |
| P-02 | `...::project_binding_path_segment_limit_is_typed_failure` | valid HIR path one over test limit | existing `CallableBuildLimitError::PathSegments` returned before catalog publication |
| P-03 | `...::project_binding_work_charges_binding_and_each_segment` | one-, two-, and N-segment rows under test limits | consumed work equals existing row plus segment charges; exact limit succeeds; one-over fails without partial catalog |
| P-04 | `...::missing_non_callable_type_rejects_complete_candidate` | external target absent from owner registry | existing `MissingProjectBindingType`; no catalog returned |
| P-05 | `...::reversed_fact_insertion_produces_identical_project_binding_catalog` | same facts in forward/reverse order | complete `ProjectCallableCatalog` equality, including every qualified/compact/alias key |
| P-06 | `...::unequal_bindings_at_same_typed_path_are_collision` | module/external or two external targets at same path | existing `ProjectBindingCollision` contains exact typed `ProjectCallablePath`; deterministic first/second |
| P-07 | `...::identical_bindings_at_same_typed_path_are_accepted` | duplicate same path and target through two provenance rows | one effective map entry; no collision |

P-01 and P-05 satisfy the complete-shadow-map and reversed-insertion mandatory cases.

## 9. Resolver shadow tests

| ID | Suggested owner/test | Setup | Required assertions |
|---|---|---|---|
| R-01 | `arcweft-lang-sema::callable::qualified_character_non_callable_terminates_environment_fallback` | project binds `['character','akane']`; environment publishes callable same path | resolver returns project non-callable result; environment candidate is not selected |
| R-02 | `...::authored_alias_non_callable_terminates_environment_fallback` | project binds `['hero']`; environment publishes callable `hero` | same terminal non-callable result |
| R-03 | `...::compact_character_binding_remains_independent` | project binds `akane`; environment has `character.akane` only | exact segmented keys do not cross-shadow |
| R-04 | existing precedence regression suite | builtins/reserved/project/environment cases | no resolver order outside the newly complete project map changes |

R-01 and R-02 satisfy the mandatory same-spelled environment callable fallback termination case.

## 10. Accepted-world atomicity tests

| ID | Suggested owner/test | Setup | Required assertions |
|---|---|---|---|
| T-01 | sema registrar candidate test | establish previous world, then attempt colliding typed project bindings | registration returns callable catalog collision; no candidate world returned |
| T-02 | `arcweft-lsp::profiles::state::typed_project_binding_collision_preserves_accepted_world_pointer` | accept world A; update facts to colliding world B | error is published for B; `Arc::ptr_eq(before, state.accepted_world())`; generation and accepted catalog/symbol pointers unchanged |
| T-03 | same state harness for malformed adapter/character typed path | accept A; decode/construct invalid B | failure occurs before registration publication; accepted pointer/generation unchanged |
| T-04 | retry determinism | after failed B, submit valid C | C publishes normally from A; no residue from failed B |

T-02 is the mandatory collision/pointer test. T-03 covers malformed typed paths under the same transaction contract.

## 11. Public API and dependency evidence

| ID | Evidence mechanism | Required assertions |
|---|---|---|
| D-01 | compile-only integration use of public syntax/HIR APIs | external crate code constructs `ProjectSymbolPath` and `ProjectDirectBinding`, reads `path()`, consumes typed `scope_bindings`; old `name()` API is absent |
| D-02 | compile-only adapter-context default-feature use | `AdapterSymbolPath` and `AdapterSymbol` work without enabling `sema` |
| D-03 | compile-only adapter-context `sema` feature use | typed adapter path converts to project facts through public APIs |
| D-04 | `cargo metadata --format-version 1 --no-deps` JSON evidence | `arcweft-lang-hir` has no sema or adapter-context dependency; sema has no adapter-context dependency; adapter-context's syntax/HIR/sema dependencies remain feature-gated as intended |
| D-05 | canonical structural audit | no dependency cycle or structural error; no source-text gate is added |

D-01 through D-05 satisfy the mandatory typed API/Cargo metadata requirement. No production source scan is an acceptance test.

## 12. Deletion regression tests

Deletion is primarily compile-enforced, not source-scan-enforced:

| ID | Mechanism | Required result |
|---|---|---|
| X-01 | compile all migrated call sites after deleting old constructor | no caller can pass `String`/`&str` to `ProjectDirectBinding::try_new` |
| X-02 | compile catalog builder against typed iterator | no destructuring form `(module, spelling, target)` remains |
| X-03 | compile adapter consumers | no `with_symbol(name, ty)` caller remains |
| X-04 | direct qualified binding tests | a qualified binding cannot disappear through an invalid-name branch |
| X-05 | public API compile test | old `ProjectDirectBinding::name()` and `ProjectSymbolBindingCollision::spelling()` are unavailable |

## 13. Focused command matrix

Commands are run after implementation in this order; package/test filters may be narrowed during development but the listed final gates are required:

```text
cargo fmt --all -- --check
cargo test -p arcweft-lang-syntax symbol_path
cargo test -p arcweft-lang-hir symbol
cargo test -p arcweft-character id
cargo test -p arcweft-adapter-context
cargo test -p arcweft-adapter-context --features sema
cargo test -p arcweft-project-loader environment
cargo test -p arcweft-lang-sema callable
cargo test -p arcweft-lang-sema registration
cargo test -p arcweft-lang-sema --test character_manifest_types
cargo test -p arcweft-compiler registration
cargo test -p arcweft-lsp profiles
cargo metadata --format-version 1 --no-deps
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

If an exact test filter does not match the final local module name, run the owning package's complete tests; a filter mismatch is not a waiver.

## 14. Acceptance summary

The implementation may be declared complete only when:

- every mandatory request scenario maps to a passing row above;
- the qualified binding omission is impossible by type/API construction;
- reversed insertion equality and accepted-pointer atomicity pass;
- dependency direction is evidenced through Cargo metadata and compilation;
- the complete workspace and canonical structural gates pass;
- no compatibility or second-path behavior is retained.
