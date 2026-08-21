# Acceptance commands

Run from repository root after implementation. Adjust only package names if Cargo workspace naming differs; do not reduce coverage.

## Formatting and static checks

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

## Targeted crates

```sh
cargo test -p arcweft-lang-hir
cargo test -p arcweft-lang-sema
cargo test -p arcweft-compiler
cargo test -p arcweft-runtime-plan
cargo test -p arcweft-core
cargo test -p arcweft-runtime-driver
cargo test -p arcweft-cli
cargo test -p arcweft-lsp
```

Run the repository's native/AWBC parity package(s) and fixture recipes identified by current `just` files. The implementation status must record the exact command names used.

## Fixture and new focused tests

```sh
# Repository fixture command that includes current_pass/check/013.
just <current-check-fixture-recipe>

# Focused test filters to be added by implementation.
cargo test -p arcweft-compiler runtime_reachability
cargo test -p arcweft-compiler task_fn_await_shape
cargo test -p arcweft-runtime-plan reachability
cargo test -p arcweft-core opaque_nominal_layout
cargo test -p arcweft-runtime-driver nominal_opaque_save
```

Do not invent a local fixture-only script in place of the maintained harness.

## Deletion and prohibition audits

```sh
rg -n 'HirRuntimeSemanticOwnerInventory|runtime_semantic_owner_inventory' crates tests
rg -n '013_task_fn_await_shape|OpeningAssets|load_opening_assets' crates
rg -n 'RuntimeTransient.*Layout|TransientNominalLayout|Opaque.*RuntimeTypeSchema' crates
rg -n 'AWBC_.*VERSION|SAVE_SCHEMA_VERSION|SCHEMA_VERSION|CODEC_VERSION' crates
```

Expected:

- first command: no production hits;
- second command: no production hits;
- third command: no newly introduced alternate layout/schema path;
- fourth command: every Arcweft-owned marker relevant to this cut remains `1`.

Audit runtime projection source for any new `TypeKind::Named` success arm, dummy `Bytes`, empty record, `Dynamic`, producer schema copy, display-name hash, or semantic-identity-as-layout behavior.

## Determinism

Run the focused reachability/plan/AWBC tests repeatedly and compare produced digests/bytes. Randomized insertion-order tests must use deterministic seeds in failure output.

## Workspace closure

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The final implementation status must record exact command output summaries and the full Git SHA used.
