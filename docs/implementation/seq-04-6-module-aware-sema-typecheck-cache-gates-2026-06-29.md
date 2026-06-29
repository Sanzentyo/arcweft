# Seq04.6 module-aware sema/typecheck cache gates implementation

Implemented as a conservative typecheck gate.

- Added `TypecheckGateObject` as an exact-compiler `.awbo` family.
- Enabled typed read/write-through for `QueryKind::TypeCheck` evidence.
- Gate hits are valid cache evidence but force linked typecheck rebuild via
  `CacheRecordStatus::HitThenRebuilt`.
- Actual module-aware typecheck reuse remains a follow-up because current project
  compilation still typechecks `HirProject::linked_module()`.

Validation commands and any failures should be recorded in this document when
the overlay is applied to the repository.

## Applied changes

The package patch file was not directly applicable in this checkout because
`git apply --check` rejected it as malformed patch text. The package-provided
Rust script was used instead:

```bash
cargo +nightly -Zscript .tmp/seq04_6/arcweft-seq04.6-module-aware-sema-typecheck-cache-gates-2026-06-29/overlay/tools/apply-seq04-6-typecheck-gate.rs --root .
cargo +nightly -Zscript .tmp/seq04_6/arcweft-seq04.6-module-aware-sema-typecheck-cache-gates-2026-06-29/overlay/tools/apply-seq04-6-typecheck-gate.rs --root . --apply
```

The overlay did not cover `crates/arcweft-cli/src/app/project_commands.rs`.
That call site now writes `CompilerObjectKind::TypecheckGate` through the same
persistent query path as the other compiler object kinds by constructing sibling
interface summary and HIR body facts, then building the conservative gate
payload.

## Validation

Passed:

```bash
cargo check -p arcweft-project -p arcweft-project-loader -p arcweft-compiler -p arcweft-cli --all-targets --all-features
cargo test -p arcweft-project persistent_object --all-features -- --nocapture
cargo test -p arcweft-project-loader persistent_query --all-features -- --nocapture
cargo test -p arcweft-compiler persistent_query --all-features -- --nocapture
cargo test -p arcweft-cli cache --all-features
cargo fmt --all -- --check
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The structure audit scanned 1,972 files, 1,007 Rust files, and 476,376 Rust
physical LOC, reporting 0 errors and 119 warnings.

Blocked by pre-existing warnings outside this change:

```bash
cargo clippy -p arcweft-project -p arcweft-project-loader -p arcweft-compiler -p arcweft-cli --all-targets --all-features -- -D warnings
```

This command reaches `arcweft-player-native` through `arcweft-cli
--all-features` and fails on existing dead-code warnings in
`native_audio.rs`, `window_driver.rs`, and `windowed.rs`. The seq04.6-specific
`clippy::doc_markdown` finding in `arcweft-compiler/src/persistent.rs` was
fixed during application.

## Follow-up boundary

Actual module-aware sema/typecheck reuse is still intentionally out of scope for
seq04.6. The follow-up request is
`docs/reviews/requests/2026-06-29-seq-04.6.1-module-aware-sema-typecheck-reuse-boundary.md`.
