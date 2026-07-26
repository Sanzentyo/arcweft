# Proof unsafe-audit raw-range authority deletion

Date: 2026-07-26

Status: `VALIDATED_COHERENT_SLICE`

This deletion-driven Proof-concurrency slice removes the provisional
unsafe-audit edit authority that copied an opening-brace `TextRange` from the
detached parser AST through sema into verifier and LSP output. The final owner
is the revision-bound HIR source component selected in
[`2026-07-26-proof-hir-local-schema-decisions.md`](2026-07-26-proof-hir-local-schema-decisions.md):
`UnsafeAuditInsertion`, keyed by its qualified audit `StmtId` in the accepted
HIR project generation and source snapshot.

## Deleted authority

- `arcweft-lang-syntax` no longer defines or fabricates the raw
  `UnsafeAuditInsertion` AST carrier.
- `arcweft-lang-sema` no longer copies an audit insertion span into
  `SemanticUnsafeAuditSummary`.
- `arcweft-verify` no longer owns `UnsafeAuditMetadata`, reconstructs an edit
  from a raw range, or publishes that range as an obligation insertion target.
- `arcweft-verify-lsp` no longer treats a parser-derived audit range as a
  workspace edit.

The `GenerateUnsafeAudit` host command remains available, but it publishes no
source edit until the public Proof HIR path can query the exact typed source
component from the accepted project/snapshot pair. Missing, stale, foreign, or
rolled-back components must continue to produce no edit. No alias, dual
reader, source-string fallback, source gate, or compatibility shim was added.

## Verification

- `cargo fmt --all`
- `git diff --check`
- `cargo check` for `arcweft-lang-syntax`, `arcweft-lang-sema`,
  `arcweft-verify`, and `arcweft-verify-lsp`, with all targets and all features
- strict Clippy for the same four crates, with all targets and all features
- all-target/all-feature tests for the same four crates

The focused all-target test run passed, including 1,117 sema unit tests, 39
verifier tests, 16 verifier-LSP tests, 473 syntax unit tests, and the syntax
integration/compile-fail suites.

The reviewable-cut gates also produced the following evidence:

- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- workspace library/integration/compile-fail tests excluding only
  `arcweft-rust-abi-macros`: passed;
- `arcweft-rust-abi-macros` compile-fail exact rerun: passed;
- all `test-workspace` CLI targets other than
  `arcw_fixtures_check_run`: passed; and
- canonical structure audit: 3,684 files, 1,936 Rust files, 906,205 Rust
  physical LOC, 94 manifests, zero errors, and 146 warnings.

The unmodified `just test-workspace` recipe remains red for two pre-existing,
unrelated reasons. Its workspace command consistently races while copying the
`arcweft-rust-abi-macros` Windows compile-fail fixture; the exact test passes
in isolation. Its `arcw_fixtures_check_run` target passes three of five rows
and fails the two filesystem-capability fixtures with
`sema.nominal.unknown_type` for unpublished `FsError`. Both failures predate
and do not traverse this unsafe-audit edit path. No production compatibility
path or stale fixture expectation was added to hide them.

This cut does not meet the Tier 2 trigger: it changes syntax/sema/verifier/LSP
edit ownership but does not change runtime, render, Agent, MCP, or capture
behavior. The repository ZIP audit found 29 archives under `docs/reviews/`,
all with current SHA-256 values recorded under `docs/implementation/` and no
unclassified archive.

## Remaining boundary

Proof `01.1.1.4` still blocks the public final-HIR lowering switch. This slice
does not restore `hir::lower_source_document` and does not adapt attached
syntax back into `TypedSyntaxTree`. Once final semantic leaf/expression
payloads are returned, the typed `UnsafeAuditInsertion` component is lowered,
validated before project publication, and projected to an edit only through
the accepted revision-bound HIR authority.
