# Unsafe audit CLI repair path convergence (seq07.3.1)

Date: 2026-06-30

## Decision

Missing unsafe audit metadata is a verifier-owned policy obligation, not a
typecheck hard error.

The typechecker remains responsible for ordinary type correctness: it checks a
present `reason` expression and all statements inside the `unsafe lifetime` body.
It does not reject an absent `reason` or absent `/// SAFETY:` doc comment. That
absence is represented by the existing semantic `UnsafeLifetimeAudit` obligation,
which already carries `has_reason`, `has_safety_doc`, and parser-owned
`UnsafeAuditInsertion` span data into `arcweft-verify`.

## Alternatives evaluated

1. Structured typecheck suggestions were rejected because they would duplicate a
   repair path already modeled by verifier `ToolActionSourceEdit` and would make
   typecheck own a policy decision rather than type shape.
2. A verifier-specific loader that skips only audit-metadata type errors was
   rejected as a transitional allow-list. It would need to classify typecheck
   errors that should not exist in the final ownership model.
3. Moving missing metadata fully into verifier obligations is the smallest final
   model. It reuses the existing syntax -> HIR -> sema -> verifier flow and keeps
   CLI/LSP on their existing typed diagnostic/suggestion carriers.

## Safety gates

`arcw verify` uses the selected `VerificationMode`; the default test policy emits
missing unsafe audit metadata as an error with a typed `GenerateUnsafeAudit`
repair suggestion.

Runtime-producing paths also consult verifier-owned safety state. A dev verifier
policy may keep ordinary proof obligations advisory, but `VerificationReport` now
exposes `has_missing_unsafe_audit_metadata` and
`has_blocking_runtime_safety_gaps`, so compile, bundle, project build, and run
routes can reject unaudited unsafe lifetime blocks without string matching.

Release mode remains strictly blocked through the existing verifier error policy.

## Diagnostic edit carrier

No typecheck-specific source-edit carrier is added. Verifier actions continue to
lower through `ToolAction::diagnostic_suggestion` into
`arcweft_source::DiagnosticSuggestion`, and LSP continues to convert the same
`ToolActionSourceEdit` into a workspace edit.

## Tests

The implementation updates or adds tests for:

- sema deferral of missing audit metadata while still rejecting unrelated body
  type errors before verifier;
- verifier `GenerateUnsafeAudit` source edits and dev-mode runtime safety gaps;
- LSP conversion of an unsafe-audit replacement edit into a workspace edit;
- CLI plain renderer patch preview for unsafe-audit metadata;
- fixture expectations for `fixtures/diagnostics/verifier-actions/missing-unsafe-audit.arcw`.

## Validation performed

```bash
cargo fmt --all
cargo test -p arcweft-lang-sema unsafe_lifetime --all-targets --all-features
cargo test -p arcweft-verify unsafe_audit_insertion --all-targets --all-features
cargo test -p arcweft-verify missing_unsafe_audit_metadata_is_runtime_safety_gap_in_dev --all-targets --all-features
cargo test -p arcweft-verify-lsp verifier_empty_insertion_action_becomes_workspace_edit --all-targets --all-features
cargo test -p arcweft-verify-lsp verifier_unsafe_audit_replacement_action_becomes_workspace_edit --all-targets --all-features
cargo test -p arcweft-cli plain_renderer_includes_verifier_proof_stub_patch_preview --all-targets --all-features
cargo test -p arcweft-cli plain_renderer_includes_verifier_unsafe_audit_patch_preview --all-targets --all-features
cargo test -p arcweft-cli spec_rejected_edge_fixtures_fail_with_diagnostics --all-targets --all-features
cargo run -p arcweft-cli -- verify fixtures/diagnostics/verifier-actions/missing-unsafe-audit.arcw
cargo check -p arcweft-lang-sema -p arcweft-verify -p arcweft-verify-lsp -p arcweft-cli -p arcweft-lsp --all-targets --all-features
cargo clippy -p arcweft-lang-sema -p arcweft-verify -p arcweft-verify-lsp -p arcweft-cli -p arcweft-lsp --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
cargo fmt --all -- --check
git diff --check
just test-fast
```

Results:

- sema unsafe-lifetime focused tests: 3 passed, including metadata deferral and
  unrelated body type-error rejection;
- verify unsafe-audit insertion test: 1 passed;
- verify dev runtime-safety gap test: 1 passed;
- verify-lsp workspace-edit tests: 2 passed;
- CLI plain renderer proof/unsafe-audit preview tests: 2 passed;
- CLI spec rejected-edge fixture test: 1 passed after moving its single-source
  diagnostic invocation from the obsolete `arcw check <file>` form to
  `arcw verify <file>`;
- `cargo run ... missing-unsafe-audit.arcw` exited with code `1` as expected
  from verifier policy and emitted the typed `Generate unsafe lifetime audit
  metadata` patch preview;
- package check and clippy with `-D warnings`: passed;
- structural audit: `files scanned: 2111`, `Rust files: 1041`,
  `Rust physical LOC: 492368`, `violations: 0 error(s), 125 warning(s)`;
- `just test-fast`: 151 + 31 + 71 + 8 + 129 tests passed.
