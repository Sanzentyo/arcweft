# Lang-01.2 Stage 2 — entry binding validation

## Outcome

Stage 2 completes the Lang-01.2 `BIND-001` through `BIND-025` boundary on top
of [Stage 1](2026-07-17-lang-01-2-entry-binding-stage-1.md). It does not start
the Stage 3 runtime, save, replay, hot-reload, or Agent-execution work.

The implementation was validated on Jujutsu change
`mkpplptyovuwzmsyolvltvvuutrpxkmw` over `main`
`9a63ac5512cd75947ba70195681e43ab968f9f12`.

## HIR and nominal ownership

`HirTopLevelDecl::Entry` now owns a `HirEntryDecl`. Syntax
`EntryDeclItem`/`EntryItem` values are converted once in HIR lowering and are
not retained or cloned into HIR. The HIR entry retains its canonical project
module and typed role values. Entry ownership lives in the dedicated
`arcweft-lang-hir::entry` responsibility module rather than expanding
`model.rs`.

Ordinary struct and enum declarations now retain structured generic parameters
through syntax and HIR registration. Entry validation can therefore reject a
generic nominal root instead of accidentally treating `State<T>` or `Event<T>`
as a closed declaration.

State and event roots must be direct, concrete project nominal declarations.
Aliases and generic roots are rejected at the role RHS. Transitive schema
validation records field, enum-variant, payload, option, sequence, and map path
segments so non-persistent or non-replay values report the owning declaration
path instead of an unlocated type failure.

## Exact callable and flow contracts

The checked contracts remain exact:

- initializer: ordinary `fn () -> State`, explicit closed empty effects, and
  empty inferred effects;
- reducer:
  `fn (&State, Event) -> Result<Reduction<State>, ReducerError>`, with an
  immutable shared state borrow, owned event, explicit closed empty effects,
  and empty inferred effects;
- Agent controller: ordinary zero-parameter function returning
  `Result<Unit, AgentError>`, with an explicit closed effect policy that covers
  every inferred effect;
- initial flow: one fixed, owned parameter of the selected state type, with no
  generics, default, rest parameter, or alternate entity family.

Contract diagnostics include expected and actual canonical types. Unresolved
role paths retain the RHS spelling. Ambiguous nominal candidates add their
declaration spans as related evidence. Candidate declarations are not silently
selected.

The semantic pass also rejects removed role attributes `#[agent]`, `#[launch]`,
and `#[bind]`. `#[budget]` is accepted only on a function selected by an Agent
entry. Kind/role checks are repeated as semantic backstops so a parser-recovered
invalid entry cannot become a checked binding.

## BIND-001 through BIND-025 evidence

| ID | Behavioral/type evidence |
|---|---|
| `BIND-001` | `bind_001_resolves_stateful_roles_to_original_declarations` checks one valid game binding and its original nominal, callable, and flow declarations. |
| `BIND-002` | `bind_002_two_game_entries_keep_independent_role_sets` checks two distinct role sets and binding identities. |
| `BIND-003` | `bind_003_game_editor_and_test_can_share_one_reducer_explicitly` checks three explicit bindings to the same ordinary reducer declaration. |
| `BIND-004` | `bind_004_each_missing_stateful_role_has_one_stable_diagnostic` checks state, initializer, event, reducer, and goto cardinality independently. |
| `BIND-005` | `bind_005_state_root_type_alias_is_rejected_at_the_rhs` checks the dedicated direct-nominal diagnostic and alias declaration evidence. |
| `BIND-006` | `bind_006_generic_state_root_is_rejected_as_open_identity` checks retained generic parameters and closed-root rejection. |
| `BIND-007` | `bind_007_state_rejects_each_non_persistent_transitive_field_with_path` covers function, reference, `Need`, and thread-handle fields with the field path. |
| `BIND-008` | `bind_008_event_rejects_non_replay_payload_with_variant_path` checks the enum variant payload path. |
| `BIND-009` | `bind_009_initializer_with_parameter_is_rejected` checks zero-parameter exactness. |
| `BIND-010` | `bind_010_initializer_wrong_return_reports_expected_and_actual_types` checks canonical expected/actual type evidence. |
| `BIND-011` | `bind_011_initializer_rejects_omitted_open_and_nonempty_effect_contracts` checks an omitted/unknown row and a non-empty row. The current source grammar has no separately authored effect-tail variable; omission is the open/unknown case. |
| `BIND-012` | `bind_012_reducer_rejects_wrong_parameter_count_and_order` checks the fixed two-parameter contract. |
| `BIND-013` | `bind_013_reducer_requires_immutable_borrowed_state` rejects owned and mutable-borrowed state. |
| `BIND-014` | `bind_014_reducer_requires_owned_event` rejects a borrowed event. |
| `BIND-015` | `bind_015_reducer_requires_exact_result_reduction_and_canonical_error` checks the selected state argument, canonical error, and outer return constructor. |
| `BIND-016` | `bind_016_reducer_rejects_declared_or_inferred_effects` checks both declared and body-inferred effects. |
| `BIND-017` | `bind_017_unresolved_and_ambiguous_role_paths_keep_rhs_and_candidates` checks an unresolved callable RHS and an ambiguous nominal role RHS with both candidate spans. |
| `BIND-018` | `bind_018_agent_controller_requires_zero_args_and_exact_result` checks Agent parameters and result. |
| `BIND-019` | `bind_019_agent_effect_outside_declared_policy_is_rejected` checks declared, inferred, and policy evidence for `fs.read`. |
| `BIND-020` | `bind_020_entry_kind_role_mismatches_have_semantic_backstops` checks stateful roles on server and controller on game after parser recovery. |
| `BIND-021` | `bind_021_duplicate_entry_ids_across_modules_are_rejected` checks canonical project-level duplicate identity. |
| `BIND-022` | `bind_022_removed_controller_role_attributes_are_rejected` checks all three removed role attributes. The additional `bind_agent_budget_is_rejected_on_unselected_function` test closes the budget-selection boundary. |
| `BIND-023` | `bind_023_initial_flow_requires_one_fixed_owned_selected_state_parameter` checks entity family, zero/multiple parameters, borrow, generic, default, variadic, and wrong-state cases. |
| `BIND-024` | `bind_024_initializer_accepts_only_ordinary_function_declarations` rejects task, dialogue, and stream functions; literals, closures, and call/function-value expressions fail before HIR because the role grammar requires a path. Arcweft has no top-level const declaration lane to select. |
| `BIND-025` | `bind_025_event_role_rejects_alias_and_generic_nominal_roots` checks both prohibited event-root forms. |

`hir_entry_is_owned_and_retains_its_project_module` separately checks the HIR
ownership boundary used by all 25 binding cases.

## Structural audit

The canonical audit scanned 3,186 files, including 1,613 Rust files and 741,303
Rust physical LOC. It reported zero errors and 128 warnings. No Stage 2 file is
at an error threshold.

Extracting the entry model reduced `arcweft-lang-hir/src/model.rs` from 1,228
physical LOC to 1,097 and placed the 144-LOC responsibility in
`arcweft-lang-hir/src/entry.rs`. The HIR `lib.rs` remains a 34-LOC facade. The
new BIND unit-test module is a cohesive 610 LOC.

The Stage 2 warning-level production files are existing syntax and orchestration
owners: `ast/items.rs` (1,935 LOC), `parser/items.rs` (1,987),
`checker/module.rs` (2,414), `project_index.rs` (1,201), and runtime-plan
`flow.rs` (1,986). Stage 2 adds focused typed-boundary handling to them; it does
not add a second responsibility or justify an arbitrary split. The dedicated
HIR entry extraction removes the only new warning introduced by this stage.

## Verification

The following commands passed after the HIR entry extraction:

- `cargo fmt --all`;
- `cargo check -p arcweft-lang-hir -p arcweft-lang-sema -p arcweft-runtime-plan`;
- `cargo test -p arcweft-lang-sema entry:: -- --nocapture`: 51 passed;
- `cargo test -p arcweft-lang-sema project_index::tests::checked_project_index -- --nocapture`: 2 passed;
- `cargo test -p arcweft-lang-hir lower::tests:: -- --nocapture`: 6 passed;
- `cargo test -p arcweft-lang-syntax --test entry_roles -- --nocapture`: 7 passed;
- `cargo test -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema`: all unit, integration, compile-fail, and doc tests passed;
- `cargo clippy -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema --all-targets -- -D warnings`;
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`: zero errors, 128 warnings.

## Completion boundary

This closes Stage 2 only. `RUN-*`, `SAVE-*`, `REP-*`, `HOT-*`, and `AGT-*`
execution work remains Stage 3 or later. No runtime transaction model, session
codec, replay executor, hot-reload compatibility rule, or Agent runtime
unification was introduced in this cut.
