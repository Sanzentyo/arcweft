# Lang 01.1.1.3.2 runtime reachability implementation cut

Date: 2026-08-21
Implementation base: `b9b2b6551945150a47766c1b1df647db0a28c202`

## Scope established

This cut replaces the old whole-project runtime semantic owner inventory with
one generation-bound `HirRuntimeSemanticReachability` authority. `CheckAll`
roots accepted Flows and checked Entries; `SelectedEntry` roots only the exact
selected checked Entry. The HIR owner computes deterministic breadth-first
closure, canonical shortest paths, a version-1 SHA-256 identity transcript,
and the reached local/expression/statement/type/pattern/capture sets.

The compiler now projects exact checked edges for project calls, trait
dispatch, Choice Flow transfer, and Entry bindings, then proves the exact
reached edge-row sets after closure. Trait dispatch is source-site independent:
ordinary method calls and implicit `for` witnesses share the same
`CheckedTraitDispatch` kind. `CheckedTraitConformance` owns the exact
`ImplMethodDeclarationId`; compiler reconstruction from `(ItemId, member)` was
deleted. Choice Flow transfer likewise accepts the owning expression site, so
SelectedEntry compilation reaches only the exact goto target instead of
implicitly rooting every Flow.

Reachable ordinary functions are classified before runtime type/layout
projection. Suspending, effectful direct-frame, and stream-factory functions
receive stable typed diagnostics. Unreachable ordinary functions retain all
final semantic facts for tooling but publish no runtime facts. Opaque project
nominal schema leaves retain their typed path, producer, and semantic identity
and receive `compiler.runtime_nominal.opaque_leaf_has_no_schema_layout` only
when reached after callable preflight.

`RuntimePlanSemanticFacts` is bound to the reachability identity and admitted
executable set. Final Flow/Entry lowering consumes only that set. The deleted
`HirRuntimeSemanticOwnerInventory` has no remaining source or test consumer.

## Physical API corrections

- `ImplMethodDeclarationId` is not `Copy`; executable owners and edge rows are
  cloned only at generation-bound projection points.
- Large typed error payloads are boxed without converting identities or schema
  paths to strings.
- `RuntimePlanSemanticFactInput` remains a staging value; atomic admission
  receives the reachability owner and publishes its identity plus admitted
  executable set. This avoids a second caller-populated membership authority.
- General `CheckedTraitDispatch` and `CheckedFlowTransfer` sites replace
  call-only/statement-only sketches because accepted implicit `for` witnesses
  and compact Choice goto facts are executable transfers too.
- HIR validation was split into the cohesive
  `runtime_semantic_owners/validation.rs` owner. The main reachability file is
  1,094 physical lines and the validation owner is 186 lines.

## Validation

Passed:

- `cargo check --workspace --all-targets --all-features`
- `cargo test -p arcweft-compiler --lib` (55 passed)
- compiler `choice_static`, `iterator_witness_source`, and
  `project_cache_transaction` integration suites
- tooling and Agent REPL runtime assertion diagnostic integration tests
- `cargo test -p arcweft-compiler reached_suspending_function_fails_before_opaque_project_nominal_layout -- --nocapture`
- `cargo run -q -p arcweft-cli -- check tests/fixtures/arcw/current_pass/check/013_task_fn_await_shape.arcw`
  (1 Flow, 0 warnings, 0 obligations)
- changed HIR, runtime-plan, and compiler library Clippy checks with
  `--no-deps -- -D warnings`
- `just structure-audit` and `just structure-audit-gate` (0 blocking
  violations)
- `git diff --check`

Known repository-wide gate failures, reproduced without changing their source
files:

- full four-crate test run: 834 HIR tests passed, 8 ignored, and
  `final_lowering::expression_lowering::tests::choice::missing_choice_body_keeps_choice_payload_and_poisoned_outer_owner`
  failed both in the full run and alone because a `PathExpression` syntax node
  was returned where the unchanged test expects `ChoiceExpression`;
- workspace/all-target Clippy stops on existing warnings in lower dependencies
  and unchanged tests, including syntax/core size and line-count warnings,
  sema pre-existing line-count/style warnings, and compiler `try_pipe.rs`
  raw-string hashes.

## Remaining package work

- project the non-authoritative tooling `RuntimeEmissionDisposition` display
  fact from this reachability owner;
- add explicit RuntimePlan/AWBC/save byte-parity and stale/mismatched
  reachability negative coverage requested by the package matrix;
- continue the independently reviewable line-plan and unary Need/match packages
  received after this cut.
