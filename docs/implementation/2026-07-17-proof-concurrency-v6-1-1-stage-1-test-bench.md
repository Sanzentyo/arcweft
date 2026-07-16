# Proof-concurrency v6.1.1 Stage 1 test and bench grammar

## Contract and safe-state boundary

This cut continues the ordered implementation from
`arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip`
(SHA-256
`1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef`).
The package requires a complete crate-private full-document grammar before
attachment, the atomic public syntax switch, and the final `ProofBlock`.

The baseline test and audit were captured on proof Stage 1 main `984537ba6dcc`.
While this independent path-scoped work was in progress, the LSP profile
fixture fix `06e502403861` and accepted View runtime-catalog cut
`7a9a070d51f9` became main; Jujutsu rebased this cut onto them without overlap.
`7a9a070d51f9` is therefore the exact editing parent.

Only the crate-private shadow grammar changes here. Public `ParsedSource`, the
current test/bench AST records, HIR, sema, manifests, runtime plans, and
serialized formats retain their sole existing authority. No shadow identity
enters HIR or caches. This is another Stage 1 compiling cut, not completion of
Stage 1 or the package.

## Implemented family

- `test ID KIND { ... }` and `bench ID { ... }` now join the existing grouped
  full-document declaration path instead of remaining one raw logical-line
  wrapper.
- Documentation and outer attributes attach losslessly to the declaration.
  Canonical entity IDs and the test adapter kind remain in the one lexed token
  stream; missing IDs and kinds own zero-width recovery nodes and diagnostics.
- Both declarations own a typed `Block`, brace nodes, and call-based plan
  statements. The outer plan always retains every row as a statement rather
  than converting the last row into a callable return tail.
- `goto` keeps the existing typed statement family. Ordinary plan calls use
  the shared expression grammar. Bench `setup`, `measure`, and `report`
  sections use the shared named-block expression implementation without
  adding those owner names to general expression dispatch.
- Unexpected tokens between the accepted header and body become ordinary
  `ErrorNode` recovery with `syntax.item.unexpected_token`; the body remains
  queryable. A missing or unclosed body owns a missing delimiter/body node and
  synchronizes before the following declaration.
- The Stage 1 inventory fixture now uses the already-canonical public forms
  `test @test.smoke scenario {}` and `bench @bench.speed {}`. No compatibility
  acceptance for the obsolete bare fixture names was added.

The direct tests cover accepted test/bench plans, nested plan sections,
lossless UTF-8/source reconstruction, missing ID/kind/body recovery, exact
header diagnostic ranges, exact missing-close anchors, and recovery before a
following proof.

## Ownership exclusions

This cut does not modify Lang-01.2 state/reducer/Agent/entry binding or any View
file. It also does not redesign live-source authoring, typed resources,
build/profile metadata, trust signing, test execution, bench timing, or the
public raw-body AST. Those boundaries remain with their dedicated contracts
and owners.

## Validation

Baseline evidence:

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --lib --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/proof-concurrency-v6-1-1-stage-1-test-bench-baseline-2026-07-17
```

The baseline had 206 passing syntax tests. Its audit scanned 3,141 files,
including 1,574 Rust files and 720,638 physical Rust LOC across 92 manifests;
it reported zero errors and 128 existing warnings.

Post-change focused evidence:

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --lib parser::test_bench_grammar_tests --all-features -- --nocapture
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-syntax --lib --all-features
CARGO_INCREMENTAL=0 cargo check -p arcweft-lang-syntax --all-targets --all-features
CARGO_INCREMENTAL=0 cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
CARGO_INCREMENTAL=0 cargo check --workspace --all-targets --all-features
CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
git diff --check
```

The four new direct tests and all 210 syntax library tests pass. Focused check
and warning-denying Clippy pass. The workspace-wide check also passes on the
integrated current main.

The additional workspace-wide Clippy attempt reached downstream crates and
then stopped on two independent warnings outside this cut:

- `arcweft-compiler/src/view_part.rs:59` uses `flat_map` over an `Option`
  (`clippy::flat_map_option`) in the accepted View runtime-catalog cut;
- `arcweft-runtime-driver/src/session/hot_swap.rs:111` assigns a cloned source
  label where the active lint requests `clone_into`
  (`clippy::assigning_clones`).

This proof worker does not modify either owner. The syntax-owned Clippy command
above completes with `-D warnings`.

The post-change canonical audit is stored at
`structure-audits/proof-concurrency-v6-1-1-stage-1-test-bench-2026-07-17/`.
It scanned 3,147 files, including 1,577 Rust files and 721,945 physical Rust
LOC across 92 manifests, and reported zero errors and 128 existing warnings.
The new production responsibility module is 8,394 bytes / 252 physical LOC,
contains no embedded tests, and adds no dependency or public API.

## Remaining ordered boundary

The reduced current vocabulary still has unstructured retained declaration
families in the shadow path. Lang-01.2 and View-owned families remain excluded
from this worker. Other retained families must be completed before Stage 1 can
close. Only then may the package proceed to private attachment/reconciliation
(Stage 2), the atomic syntax public switch (Stage 3), and final predicate/proof
typed wrappers and `ProofBlock` (Stage 4). No partial HIR identity migration is
permitted before those gates.
