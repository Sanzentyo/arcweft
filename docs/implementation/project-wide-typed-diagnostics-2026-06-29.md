# Project-wide typed diagnostics - 2026-06-29

## Source package

Applied from:

```text
arcweft-seq07.1-project-wide-typed-diagnostics-2026-06-29.zip
```

## Scope

This cut extends the Seq07 diagnostics renderer foundation from direct-source CLI checks into project-wide compiler and LSP paths.

## Implemented decisions

- `ProjectCompileDiagnostic` now owns `arcweft_source::Diagnostic` directly.
- Module-local project diagnostics preserve a `SourceName` and source text snapshot through `ProjectDiagnosticSource`.
- Source-less linked/global diagnostics are represented explicitly with `source: None`.
- Parser and syntax lint project diagnostics reuse existing shared `Diagnostic` builders.
- HIR lower, sema, and runtime-plan error types expose owned `diagnostic()` conversion methods where dependency direction permits.
- CLI project compile failures render through `DiagnosticEmitter`; string-only loops are removed.
- LSP conversion consumes shared diagnostics and maps labels/notes/suggestions to LSP ranges, related information, and `Diagnostic.data`.

## Intentional source-less diagnostics

The following remain source-less until their owning phases carry source anchors:

- HIR project/link failures.
- name-resolution errors.
- typecheck readiness diagnostics.
- non-effect typecheck errors/warnings.
- line-task lower errors.
- runtime-plan lower errors.
- persistent-query cache I/O/corruption/build-artifact failures.

## Validation

Applied checkout validation:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-source -p arcweft-lang-syntax -p arcweft-compiler -p arcweft-cli -p arcweft-lsp --all-targets
cargo test -p arcweft-compiler diagnostics --all-targets
cargo test -p arcweft-cli app::diagnostics --all-targets
cargo test -p arcweft-cli app::project_commands::tests::release_project_diagnostics_reject_dynamic_goto --all-targets
cargo test -p arcweft-lsp diagnostics --all-targets
cargo clippy -p arcweft-source -p arcweft-lang-syntax -p arcweft-compiler -p arcweft-cli -p arcweft-lsp --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

The structural audit scanned 2006 files, including 1018 Rust files and 480410
Rust physical LOC, with 0 errors and 121 warnings.

The package's broad command
`cargo test -p arcweft-cli diagnostics --all-targets` was also attempted. It
compiled successfully and passed the direct diagnostics/project tests, but the
`diagnostics` substring filter also selected unrelated existing `check.rs`
integration cases. Those failed on pre-existing CLI invocation/resource URI
paths: `spec_rejected_edge_fixtures_fail_with_diagnostics` reported an
unexpected positional argument for `arcw check`, and two
`agent_observe_native::*diagnostics*` tests reported unsupported
`arcweft://session/cli/frame/...rich_text.png` resource URIs. The seq07.1 CLI
paths touched in this cut are covered by the focused commands above.
