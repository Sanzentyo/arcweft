# Validation plan

This package defines implementation validation; no command below is claimed as
already run against a full checkout by this design return.

## 1. Focused crate tests

Run the owning suites after each compiling cut, including at minimum:

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-project --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-launch --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-character --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-sema --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-project-loader --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-bundle --all-features
CARGO_INCREMENTAL=0 cargo test -p arcweft-lsp --all-features
```

Use the repository's exact current package name if an LSP package was renamed;
the owning module and test rows do not change.

## 2. Compile-fail/public API evidence

Add UI/compile-fail cases proving:

- no Source root-family/target variant exists;
- no public `ProjectSemanticIndex` can be constructed without accepted content;
- binary bytes cannot be passed to the text overlay type;
- old source `content`/Source AST/HIR/public types cannot be named;
- no old wire tag or serde alias is accepted.

## 3. Workspace and lint gates

```bash
cargo fmt --all -- --check
CARGO_INCREMENTAL=0 cargo check --workspace --all-targets --all-features
CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check -- .
git diff --cached --check -- .
test -z "$(git diff --name-only --diff-filter=U)"
```

No `#[allow(...)]`, unstable feature, unsafe code, new macro, compatibility
alias, or ad hoc extension trait is justified by this contract.

## 4. Repository suites

```bash
CARGO_INCREMENTAL=0 just verify
CARGO_INCREMENTAL=0 just verify-full
```

`verify-full` is mandatory because this cut changes project loading,
project-wide semantic facts, bundle inputs, watch behavior, and LSP atomic
publication.

## 5. Structural audit

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The resulting implementation note SHALL report zero structural errors and
resolve warnings in changed in-scope files. Cargo metadata SHALL prove:

- `arcweft-project` has no path to sema, loader, compiler, bundle, or LSP;
- Sans-I/O owners have no filesystem adapter dependency;
- there is one final content-fact owner;
- project-loader remains the cross-layer coordinator.

## 6. Determinism and artifact gates

- Run revision golden vectors twice from permuted input insertion order.
- Build bundle fixtures twice and compare exact bytes/hashes.
- Run disk/overlay parity with identical effective bytes.
- Run all concurrent stale-publication tests repeatedly under the repository's
  deterministic scheduler/testing support.

## 7. Final implementation evidence

The implementation note SHALL distinguish passed, failed, blocked, and not-run
commands. It SHALL list the final Git commit, changed files, focused logs,
workspace logs, Tier 2 log, structural report, and all test matrix IDs.
