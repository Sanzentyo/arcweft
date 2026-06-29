# Diagnostics Renderer Foundation - 2026-06-29

## Source Package

Applied from:

```text
D:/sanze/Downloads/arcweft-diagnostics-impl.zip
```

The package introduced the first shared structured diagnostic substrate and CLI
renderer. It intentionally left project-wide compiler diagnostics and verifier
action output as follow-up work.

## Applied Scope

- Added `annotate-snippets` as a workspace dependency and kept the renderer
  dependency in `arcweft-cli`.
- Extended `arcweft-source::Diagnostic` with stable codes, primary/secondary
  labels, notes, suggestions, source edits, and applicability.
- Extended parser recovery suggestions with structured edits and applicability.
- Extended syntax lints so `style::explicit_decl_id` and
  `style::deep_dot_run_relative_id` can carry machine-applicable edits.
- Added `crates/arcweft-cli/src/app/diagnostics.rs` as the process-facing
  renderer over `annotate-snippets`.
- Routed direct-source parse diagnostics, syntax lints, HIR lower errors,
  semantic errors, typecheck errors, and line-task lowering errors through the
  CLI diagnostic renderer in `load_and_check_with_env`.

## Integration Notes

The package patch at `patches/callsite-integration.patch` was malformed in this
checkout (`git apply --check` reported a corrupt patch), so the overlay files
were applied directly and the call-site edits were ported manually. The manual
edits preserve the package intent while adjusting to the current parser surfaces.

The packaged deep-dot lint test used a plain `goto @...ending` sample, but the
current lint surface reaches the deep-dot rule through parsed relative IDs such
as choice option IDs. The test now uses a choice option ID so it exercises the
implemented lint rule instead of relying on an unimplemented parser projection.

## Non-Goals

- Project-wide `ProjectCompileDiagnostic` and persistent compiler diagnostic
  paths still use older message-bearing structures in places. This is split to
  seq-07.1.
- Verifier `ToolAction` and repair/action output are not yet rendered as CLI/LSP
  diagnostic suggestions. This is split to seq-07.2.
- `annotate-snippets` is not added below CLI. Lower-level crates continue to
  expose typed diagnostic data only.

## Validation

Passed:

```bash
cargo search annotate-snippets --limit 1
cargo fmt --all -- --check
git diff --check
cargo check -p arcweft-source -p arcweft-lang-syntax -p arcweft-cli --all-targets
cargo test -p arcweft-source -p arcweft-lang-syntax --all-targets
cargo test -p arcweft-cli plain_renderer_includes_code_label_and_patch --all-targets -- --nocapture
cargo clippy -p arcweft-source -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
cargo clippy -p arcweft-cli --all-targets --no-default-features --features agent-repl,native-capture -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

`cargo search` confirmed `annotate-snippets = "0.12.16"` as the current
crates.io version. The structural audit scanned 1,984 files and 1,009 Rust
files, reporting 0 errors and 119 warnings.

Workspace clippy was attempted:

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

It is still blocked by pre-existing `arcweft-player-native` dead-code warnings in
`native_audio.rs`, `window_driver.rs`, and `windowed.rs`; the failures are not
introduced by this diagnostics cut.
