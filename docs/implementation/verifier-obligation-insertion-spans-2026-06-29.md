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

The `missing-unsafe-audit.arcw` CLI command exits with code `1` from verifier
policy after seq07.3.1. It is accepted when the CLI output reaches verifier
diagnostics and includes the typed `Generate unsafe lifetime audit metadata`
patch preview.

The package's broad filters `cargo test -p arcweft-verify verifier_obligation`,
`cargo test -p arcweft-verify-lsp verifier_source_edit`, and
`cargo test -p arcweft-lsp code_action` were also run. Some sub-binaries matched
zero tests under those filters; the added exact tests above cover the required
behavior directly.

## Seq07.3.1 closure

`arcw verify fixtures/diagnostics/verifier-actions/missing-unsafe-audit.arcw`
now reaches verifier diagnostics instead of stopping in `sema.typecheck` for
missing unsafe audit metadata. Typecheck still rejects ordinary unrelated body
type errors before verifier repair actions are constructed.

```text
Generate unsafe lifetime audit metadata
reason = _
/// SAFETY: TODO: justify this unsafe lifetime block.
```

The ownership convergence is documented in
`docs/implementation/unsafe-audit-cli-repair-path-convergence-2026-06-30.md`.
Missing unsafe audit metadata is now a verifier-owned policy obligation, and
runtime-producing paths use verifier-owned `has_blocking_runtime_safety_gaps`
rather than string matching rendered diagnostics.

Structural audit result:

```text
files scanned: 2101
Rust files: 1034
Rust physical LOC: 486949
package manifests: 91
violations: 0 error(s), 124 warning(s)
```
