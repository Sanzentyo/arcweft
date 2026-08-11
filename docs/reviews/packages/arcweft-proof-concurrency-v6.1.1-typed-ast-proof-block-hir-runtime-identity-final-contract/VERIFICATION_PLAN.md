# Verification plan

## 1. Evidence policy

This file specifies commands for the production implementation cut. None of these commands was run while producing this design-only archive. The implementation note must record the exact Git commit, full Jujutsu change ID available in the implementation checkout, UTC start/end timestamps, toolchain versions, command, exit status, and artifact/report path for every command that is claimed.

Run from the repository root on the final candidate commit with no uncommitted production changes other than the intended cut. The stable feature combination for all focused and workspace Cargo validation is `--all-features`. No default-feature-only result substitutes for it. `CARGO_INCREMENTAL=0` is required for check, Clippy, focused final reruns, compile-fail suites, and the final `just` cut point.

The implementation may run narrower developer loops earlier, but only the commands below satisfy the completion gate.

## 2. Toolchain and identity capture

```bash
rustc -Vv
cargo -V
rustup show active-toolchain
just --version
git rev-parse HEAD
jj log -r @ -T 'commit_id ++ "\n" ++ change_id ++ "\n"'
git status --short
```

The production implementation note must show a 40-lowercase-hex Git commit and the full Jujutsu change ID reported by that checkout. It must also state the source request basis Git `76d39983ad8770a87d6e81745785b6b362a381b4` and whether latest `main` moved during implementation.

## 3. Focused source and syntax validation

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-source --all-targets --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --all-targets --all-features
```

Required named focused reruns:

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --all-features \
  same_line_descendants_receive_distinct_syntax_ids -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --all-features \
  trivia_only_reparse_preserves_predicate_proof_descendant_ids_and_updates_ranges -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --all-features \
  typed_to_rowan_and_rowan_to_typed_round_trip -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --all-features \
  syntax_identity_exhaustion_is_atomic -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --all-features \
  proof_block_exact_shapes_and_ranges -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --all-features \
  removed_forms_use_ordinary_current_grammar_recovery -- --exact
```

Exact production-constant syntax budget cases, including any normally ignored slow constructor cases, run explicitly:

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --all-features \
  --test limits_predicate_proof -- --include-ignored
```

## 4. Focused HIR, sema, verifier, and project validation

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-hir --all-targets --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-sema --all-targets --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-verify --all-targets --all-features
```

Required named reruns:

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-hir --all-features \
  every_source_backed_node_maps_to_exact_hir_kind -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-hir --all-features \
  old_snapshot_resolves_live_interval -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-hir --all-features \
  cross_syntax_database_lowering_is_rejected_atomically -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-hir --all-features \
  closure_capture_order_is_first_use_then_local_id -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-hir --all-features \
  typed_child_beats_disagreeing_display_source -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-hir --all-features \
  one_symbol_table_registers_all_callable_kinds_and_character -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-sema --all-features \
  predicate_and_proof_recursion_sccs_are_rejected -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-verify --all-features \
  verifier_consumes_predicate_proof_arena_records -- --exact
```

Exact production-constant HIR budgets and seeded exhaustion cases:

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-hir --all-features \
  --test limits_atomicity -- --include-ignored
```

## 5. Focused runtime-plan, compiler, runtime, and codec validation

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-core --all-targets --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-runtime-plan --all-targets --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-compiler --all-targets --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-runtime-host --all-targets --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-runtime-driver --all-targets --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-bundle --all-targets --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-save --all-targets --all-features
```

Required named reruns:

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-runtime-plan --all-features \
  check_failure_retains_exact_session_identity -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-runtime-plan --all-features \
  enabled_debug_failure_retains_exact_session_identity -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-runtime-plan --all-features \
  release_plan_omits_debug_evaluation_and_inventory -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-runtime-plan --all-features \
  prove_has_no_runtime_mode_or_guard -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-runtime-plan --all-features \
  runtime_projection_emits_stable_diagnostic_without_message_parsing -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-core --all-features \
  runtime_assertion_core_codec_has_no_session_identity -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-bundle --all-features \
  awbc_bundle_save_checkpoint_cache_round_trip_without_session_ids -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-compiler --all-features \
  compiled_project_contains_no_linked_hir -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-compiler --all-features \
  reloaded_artifact_uses_fresh_inventory_without_old_stmt_equality -- --exact
```

If the combined persisted-boundary test is split among owning crates, each owning exact test must run and the implementation note must map each invocation to the single matrix row; no monolithic test crate may replace owner-level codec tests.

## 6. CLI, LSP, Agent, tooling, and formatter validation

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-cli --all-targets --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-lsp --all-targets --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-tooling --all-targets --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-agent-repl --all-targets --all-features
```

Required named reruns:

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --all-features \
  formatter_preserves_lossless_predicate_proof_nodes -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-lsp --all-features \
  lsp_navigation_uses_typed_syntax_and_module_hir_ids -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-cli --all-features \
  cli_diagnostics_render_exact_revision_spans -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-tooling --all-features \
  agent_runtime_assertion_projection_uses_session_capability -- --exact
```

## 7. Compile-fail public API suites

The exact UI fixture paths are listed in `TEST_MATRIX.md`. Run every owning suite:

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --test ui --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-hir --test ui --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-compiler --test ui --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-runtime-plan --test ui --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-core --test ui --all-features
```

The accepted `.stderr` files must assert compiler diagnostics caused by private/absent types, methods, constructors, and trait implementations. They may not assert implementation by searching Rust source or documentation.

## 8. Metadata and dependency-layer evidence

Create a machine-readable metadata snapshot:

```bash
mkdir -p target/cut-01-1-1-evidence
cargo metadata --format-version 1 --all-features \
  > target/cut-01-1-1-evidence/cargo-metadata.json
```

Run the checked-in typed metadata validator added by this cut:

```bash
cargo +nightly -Zscript tools/verify-cut-01-1-1-layering.rs \
  --metadata target/cut-01-1-1-evidence/cargo-metadata.json \
  --report target/cut-01-1-1-evidence/layering-report.md
```

The script deserializes Cargo metadata and verifies:

- `arcweft-lang-syntax` has no path to HIR, sema, runtime-plan, compiler, CLI, or LSP;
- `arcweft-lang-hir` depends on syntax/source but not core/runtime host;
- `arcweft-core` has no path to syntax, HIR, sema, runtime-plan, compiler, CLI, or LSP;
- `arcweft-runtime-host` has no normal path to runtime-plan, HIR, syntax, or compiler;
- no session-ID type is available through a persisted/data crate dependency;
- the final normal-edge fan-in/fan-out numbers match the structural report.

Also capture human-readable trees:

```bash
cargo tree -p arcweft-lang-syntax -e normal \
  > target/cut-01-1-1-evidence/syntax-tree.txt
cargo tree -p arcweft-lang-hir -e normal \
  > target/cut-01-1-1-evidence/hir-tree.txt
cargo tree -p arcweft-core -e normal \
  > target/cut-01-1-1-evidence/core-tree.txt
cargo tree -p arcweft-runtime-host -e normal \
  > target/cut-01-1-1-evidence/runtime-host-tree.txt
cargo tree -i arcweft-lang-syntax -e normal \
  > target/cut-01-1-1-evidence/syntax-reverse-tree.txt
cargo tree -i arcweft-lang-hir -e normal \
  > target/cut-01-1-1-evidence/hir-reverse-tree.txt
```

## 9. Formatting, workspace check, and Clippy

Run exactly:

```bash
cargo fmt --all -- --check
CARGO_INCREMENTAL=0 cargo check --workspace --all-targets --all-features
CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets --all-features -- -D warnings
```

A warning-free Clippy command is mandatory even if a checked-in `just` recipe currently invokes Clippy without `-D warnings`.

## 10. Checked-in fast cut-point suite

The final pre-push normal workspace path is:

```bash
CARGO_INCREMENTAL=0 just verify
```

It must run after all focused fixes and after the final formatting/workspace commands. A pass from an earlier intermediate commit does not satisfy the gate.

Tier 2 is not automatically required for this cut because proof discharge, scheduler behavior, rendering, GPU/native UI, and checkpoint semantics are non-goals. If implementation changes any Tier-2 risk owner or behavior beyond dependency-only/test-only edits, run the checked-in full suite instead:

```bash
CARGO_INCREMENTAL=0 just verify-full
```

The implementation note must name the triggering files and record whether `verify-full` ran. It must not claim Tier-2 validation when only the fast suite ran.

## 11. Diff, conflict, and repository-state checks

```bash
git diff --check -- .
git diff --cached --check -- .
test -z "$(git diff --name-only --diff-filter=U)"
git status --short
```

`git diff --check` is the conflict-marker/whitespace gate for changed content. The unmerged-file query confirms no unresolved index conflict. No command greps checked-in source or documentation for removed spellings, type names, paths, or implementation snippets.

## 12. Structural audit

Run the required canonical command exactly:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Check in the resulting cut-specific report under `docs/implementation/structure-audits/`. The report must satisfy `STRUCTURE_PLAN.md`, including exact bytes, physical LOC, code LOC, numeric embedded-test LOC, generated status, responsibilities, and crate fan-in/fan-out for changed/largest files. The report must show zero structural errors. Any warning involving an in-scope changed file must be resolved or explicitly justified in the implementation note under the size gates of this contract.

After checking in the audit, rerun:

```bash
cargo fmt --all -- --check
git diff --check -- .
CARGO_INCREMENTAL=0 just verify
```

## 13. Final evidence index

The implementation note must include a compact table with one row per section above and links/paths to captured logs. It must distinguish:

- `PASS` — command executed on the final candidate commit and exited zero;
- `NOT_APPLICABLE` — Tier 2 only, with risk-owner reasoning;
- `NOT_RUN` — never acceptable for sections 2-12 other than conditional Tier 2.

Do not fabricate logs, infer a pass from earlier checked-in documentation, or use this design archive's creation checks as production validation.
