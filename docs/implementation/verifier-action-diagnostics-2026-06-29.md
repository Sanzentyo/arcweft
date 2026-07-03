# Verifier Action Diagnostics - 2026-06-29

## Source package

Applied from:

```text
arcweft-seq07.2-verifier-action-diagnostics-2026-06-29.zip
```

## Applied scope

- Added `arcweft_source::DiagnosticCommand` as the typed host-action companion to edit-bearing `DiagnosticSuggestion`.
- Extended verifier `ToolAction` with optional source-edit and host-command payloads.
- Added typed conversions from verifier diagnostics/reports into `arcweft_source::Diagnostic`.
- Preserved semantic-owned verifier actions during semantic diagnostic merge.
- Routed CLI verifier/check/build/compile diagnostic output through `DiagnosticEmitter`.
- Converted verifier source edits and host commands into LSP code actions from the same `ToolAction` data.
- Stored verifier reports in LSP document analysis so code actions do not parse rendered diagnostic text.

## Decisions

`GenerateProofStub` and `GenerateUnsafeAudit` are edit-capable only when they carry exact spans. They default to `HasPlaceholders` because proof bodies and safety justifications are human-authored.

`ShowObligation`, `NavigateToProof`, and `NavigateToUnsafeAudit` are host commands. They are not represented as empty `SourceEdit`s.

`TrustedAssumption`, `RawSyntax`, runtime conflicts, and solver raw/model output remain evidence-only in this cut.

## Validation

Executed from the repository root after applying the package:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-source -p arcweft-verify -p arcweft-cli -p arcweft-lsp --all-targets
cargo check -p arcweft-cli --all-targets
cargo test -p arcweft-source source_span_preserves_range_positions_and_diagnostics --all-targets
cargo test -p arcweft-verify diagnostics --all-targets
cargo test -p arcweft-verify verifier_action_source_edit_becomes_diagnostic_suggestion --all-targets
cargo test -p arcweft-verify verifier_host_action_becomes_diagnostic_command --all-targets
cargo test -p arcweft-verify-lsp verifier_source_edit_action_becomes_workspace_edit --all-targets
cargo test -p arcweft-verify-lsp verifier_host_action_becomes_command_action --all-targets
cargo test -p arcweft-lsp verifier_report_actions_are_included_in_code_actions --all-targets
cargo test -p arcweft-cli plain_renderer_includes_code_label_and_patch --all-targets
cargo run -p arcweft-cli -- verify fixtures/diagnostics/verifier-actions/missing-proof.arcw
cargo clippy -p arcweft-source -p arcweft-verify -p arcweft-cli -p arcweft-lsp --all-targets --all-features -- -D warnings
cargo clippy -p arcweft-cli --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

The fixture `cargo run` command is expected to exit with code `1` because the
fixture intentionally contains a missing proof. It was accepted when the CLI
printed typed verifier actions and returned that failure code.

`cargo test -p arcweft-cli verifier --all-targets` and
`cargo test -p arcweft-verify-lsp code_action --all-targets` were also run as
package-aligned filters, but matched zero tests in the current checkout. The
specific new tests above cover those paths directly.

Structural audit result: `2047` files scanned, `1028` Rust files, `483517`
Rust physical LOC, `0 error(s), 121 warning(s)`.

## Follow-up

Thread exact source spans into verifier obligations where the AST/HIR can identify safe proof/audit insertion sites. Until then, command actions provide the stable repair-plan path.
