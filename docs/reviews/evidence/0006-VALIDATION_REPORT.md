# Validation report

Validation date: 2026-08-29. Checkout:
`D:\git\arcweft`, `main` at
`163a3b0da9fdcd5524ffeca8b055d774d53008e2`, equal to `origin/main` at
inspection.

## Design/package validation

Passed:

```text
cargo +nightly -Zscript <design>/tools/validate_design.rs --apply-manifest <design>
PASS ... files=21 statement_rows=35 trigger_tags=9 payload_tags=15 repository=PASS

cargo +nightly -Zscript <design>/tools/validate_design.rs --design-only <design>
PASS ... files=21 statement_rows=35 trigger_tags=9 payload_tags=15 repository=NOT_RUN

cargo +nightly -Zscript <design>/tools/validate_design.rs <design>
PASS ... files=21 statement_rows=35 trigger_tags=9 payload_tags=15 repository=PASS

cargo +nightly -Zscript <design>/tools/negative_self_tests.rs <design>
PASS ... negative_cases=124 mandatory_gates=20 repository=PASS
```

The repository-aware pass checked request bytes/SHA, the complete manifest,
terminal status/open questions, all closed schemas/tags, the exhaustive 35-row
matrix, current five/26 predecessor-correction inventories, exact inspected
HEAD/worktree blobs, Rust AST inventories, Cargo metadata, and dependency
direction. Negative self-tests mutate every mandatory gate, every Trigger,
Select, head, and statement-payload tag, every ingress publication and
scrutinee role, every mark-coordinate component, every 35-row matrix result,
every deletion step/prohibition/transcript exclusion, the source inventory,
and a synthetic reverse dependency.

Also passed:

```text
request/mirror SHA-256 equality and 43,679-byte equality
PowerShell ConvertFrom-Json for all three machine JSON members
rustfmt +nightly --edition 2024 --check on all three Rust validator files
git diff --no-index --check /dev/null <each design member>
```

The last whitespace command is applied member-by-member because the design
directory is new/untracked; it produced zero whitespace-error messages.

## Current repository Cargo validation

The read-only targeted current-worktree command was run separately:

```text
cargo check -p arcweft-lang-sema -p arcweft-compiler \
  -p arcweft-runtime-plan -p arcweft-verify
```

The first attempt **FAILED** in concurrently changing, pre-existing dirty
evaluated-effect/compiler WIP. At then-current compiler blob `097af778...`,
`arcweft-compiler/src/lower.rs` called `world()`, `module()`, and `name()` on
`SemanticTypeDigest` (`E0599`, three errors). Another authorized task then
repaired that user-owned WIP. This design task refreshed the inspected compiler
blob to `4f1e57d...` and reran the exact same command. Final current-snapshot
result: **PASS** (`Finished dev profile` in 39.84s). No WIP file was edited by
this design task; the initial failure remains recorded rather than hidden.

Not run: workspace tests, Clippy, rustdoc, or a full workspace check. This was a
design-only change, the targeted current-worktree Cargo check passed after the
concurrent WIP repair, and no production Rust/Cargo surface was modified. The
cargo-script validators themselves compiled and ran warning-free; their Rust
sources passed rustfmt check.

## Scope and packaging

- Production Rust/Cargo edits: none.
- Existing dirty worktree edits/reset/staging: none.
- New worktree/branch/workspace checkout: none.
- Commit/push: none.
- ZIP: intentionally not created. Checked-in review designs use Git as their
  maintained package authority, and the delegated task explicitly requested no
  duplicate archive unless repository policy required one.
- Open questions: none.
