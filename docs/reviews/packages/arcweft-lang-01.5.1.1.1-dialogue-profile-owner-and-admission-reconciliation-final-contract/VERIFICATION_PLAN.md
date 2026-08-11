# Verification plan and actual validation ledger

## Baseline guard

In a real Arcweft checkout, begin with:

```sh
git status --short
git rev-parse HEAD
git log -1 --oneline
```

Review against `0c8cb74dd96116a8b987cc419c9a280b6cabe4a4` or a later accepted `main`. If `main` advanced,
reinspect all listed evidence paths rather than assuming this package is still
current.

## Focused behavior and codec validation

Run the owner-local suites first:

```sh
cargo test -p arcweft-dialogue
cargo test -p arcweft-launch
cargo test -p arcweft-compiler --test dialogue_profile_admission
```

Also run the exact runtime-plan, bundle codec, save/replay, CLI, LSP, Agent/MCP,
and backend parity tests that consume `DialogueProfileRevision` and
`LineDisplaySpec` in the current tree.

## Workspace quality gates

```sh
cargo fmt --all -- --check
git diff --check
cargo check --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-workspace
just test-tier2
```

If repository scripts select additional target/platform matrices, run them as
specified by the current scoped `AGENTS.md` and `justfile`.

## Structured dependency evidence

Generate the graph through Cargo, not source spelling:

```sh
cargo metadata --format-version 1 > target/dialogue-profile-cargo-metadata.json
```

Parse package IDs and resolved dependency edges and assert:

- `arcweft-manifest-model` has no View/dialogue/launch/compiler/runtime-driver
  presentation edge;
- project-loader has no runtime-driver edge;
- runtime-plan has no compiler edge;
- compiler imports the lower dialogue revision through the existing direction;
- no second catalog crate/product was added.

The generated JSON may be retained as deterministic test evidence. A `grep` for
crate names is not a replacement.

## Source-map/range validation

Run exact source-map tests with a complete nested fallback manifest. For every
`ManifestTokenPath`/slot, assert exact range and source identity. Run the
single-decode counter test with CLI/LSP/project consumers sharing the same
`SourceBackedManifest`.

## Admission negative matrix

Exercise:

- missing View program;
- missing View;
- wrong-family View at decoder;
- non-dialogue-capable View with definition secondary label;
- missing Style;
- wrong-family Style at decoder;
- missing View/Style provenance;
- View/Style product source revision mismatch;
- resource registry pointer mismatch;
- retained resolved profile mismatch.

Assert stage, code, severity, primary source identity/range, related labels, and
that no `CompiledProject` is returned.

## Atomic publication and parity

Construct previous generation A and candidate B. Mutate one revision component
at a time. Verify rejection leaves A observable through runtime/native/Web/
headless/Agent/MCP and save/replay identity. Then publish valid C and prove all
consumers switch as one generation.

## Deletion closure

Use parser/AST/HIR behavior and public API compile tests to prove source
`dialogue defaults` has no successful typed path. Source search may help review
the deletion, but it is not the acceptance gate.

## Validation recorded by the repository

The maintained implementation note for the completed cut records passing:

```text
cargo fmt --all -- --check
git diff --check
cargo check --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-workspace
just test-tier2
structural audit: 0 errors
```

Those are repository-recorded results at that implementation cut. They were not
rerun by this return.

## Validation performed by this return

```text
current-main source/static evidence inspection: PASS
required archive-member presence: PASS
OPEN_QUESTIONS.md exact bytes `none
`: PASS
per-entry SHA-256 and byte-size manifest verification: PASS
ZIP CRC/integrity test: PASS
extract-and-reverify: PASS
Arcweft Cargo/Clippy/tests/Tier 2 rerun: NOT RUN (no repository checkout)
```

The exact ZIP hash is in the adjacent `.sha256` sidecar. `MANIFEST.txt` covers
all payload files except itself to avoid a self-referential digest.
