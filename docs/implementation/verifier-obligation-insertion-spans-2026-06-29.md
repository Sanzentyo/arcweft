# Verifier obligation insertion spans (seq07.3)

Date: 2026-06-29

This document records the seq07.3 implementation of exact proof/audit insertion spans for verifier obligations.

## Summary

Seq07.2 introduced typed verifier actions with optional source edit payloads, but proof/audit edit actions remained blocked on exact spans. Seq07.3 adds typed insertion targets to verifier obligations and only lowers them into `ToolActionSourceEdit` when the source boundary is provably safe.

## Implemented contracts

- `ProofObligation::insertion_target` carries an optional exact `VerifierInsertionTarget`.
- Proof stubs use an empty top-level insertion range derived from HIR top-level ranges.
- Unsafe audit metadata uses a replacement range for the exact opening `{` of braced `unsafe lifetime` blocks.
- `GenerateProofStub` and `GenerateUnsafeAudit` become source-edit actions only when their target policy matches the obligation kind.
- Otherwise, existing host-command fallback is preserved.

## Ownership boundaries

- Syntax/parser owns exact unsafe block opening-brace ranges.
- HIR owns top-level insertion inventory.
- Sema carries unsafe audit insertion summaries without depending on verifier types.
- Verify lowers typed insertion targets into typed source edits.
- CLI/LSP consume existing diagnostic/action carriers.

## Validation

Executed in `D:\git\arcweft` after applying the package:

```bash
cargo fmt --all
cargo check -p arcweft-source -p arcweft-lang-hir -p arcweft-verify -p arcweft-verify-lsp -p arcweft-cli -p arcweft-lsp --all-targets --all-features
cargo test -p arcweft-verify proof_insertion --all-targets --all-features
cargo test -p arcweft-verify unsafe_audit_insertion --all-targets --all-features
cargo test -p arcweft-verify verifier_action_source_edit_becomes_diagnostic_suggestion --all-targets --all-features
cargo test -p arcweft-verify-lsp verifier_empty_insertion_action_becomes_workspace_edit --all-targets --all-features
cargo test -p arcweft-cli plain_renderer_includes_verifier_proof_stub_patch_preview --all-targets --all-features
cargo test -p arcweft-verify verifier_obligation --all-targets --all-features
cargo test -p arcweft-verify-lsp verifier_source_edit --all-targets --all-features
cargo test -p arcweft-lsp code_action --all-targets --all-features
cargo test -p arcweft-cli verifier --all-targets --all-features
cargo test -p arcweft-lang-sema unsafe_lifetime --all-targets --all-features
cargo run -p arcweft-cli -- verify fixtures/diagnostics/verifier-actions/missing-proof.arcw
cargo run -p arcweft-cli -- verify fixtures/diagnostics/verifier-actions/missing-unsafe-audit.arcw
cargo clippy -p arcweft-source -p arcweft-lang-hir -p arcweft-verify -p arcweft-verify-lsp -p arcweft-cli -p arcweft-lsp --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

The `missing-proof.arcw` CLI command is expected to exit with code `1` because
the fixture intentionally contains a verifier error. It is accepted when the
CLI output includes the typed `Generate proof stub` patch preview.

The `missing-unsafe-audit.arcw` CLI command is currently expected to exit with
code `1` at the typecheck phase. This documents the gap below rather than a
completed CLI repair path.

The package's broad filters `cargo test -p arcweft-verify verifier_obligation`,
`cargo test -p arcweft-verify-lsp verifier_source_edit`, and
`cargo test -p arcweft-lsp code_action` were also run. Some sub-binaries matched
zero tests under those filters; the added exact tests above cover the required
behavior directly.

## Known gap

`arcw verify fixtures/diagnostics/verifier-actions/missing-unsafe-audit.arcw`
still stops in `sema.typecheck` before verifier diagnostics are emitted:

```text
unsafe lifetime block requires a reason
unsafe lifetime block requires a SAFETY doc comment
```

The verifier and LSP layers can produce and transport the unsafe-audit
source-edit action, but the full CLI verify path cannot display it until audit
metadata ownership is converged between typecheck and verifier. This is split
into
`docs/reviews/requests/2026-06-30-seq-07.3.1-unsafe-audit-cli-repair-path-convergence.md`.

Structural audit result:

```text
files scanned: 2101
Rust files: 1034
Rust physical LOC: 486949
package manifests: 91
violations: 0 error(s), 124 warning(s)
```
