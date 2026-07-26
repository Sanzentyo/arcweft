# Proof convergence: LSP inlay semantic fallback deletion

Date: 2026-07-27

Status: implementation and cut-specific validation complete

## Context

The returned Proof-concurrency `v6.1.1.4` archive has SHA-256
`414f95f8ef4c5f3abcce163f0c9b01f124098f0bac856f174af09b5c1e7d564b`.
It is byte-identical to the previously rejected delivery and does not satisfy
the corrective `.1.1.4.1` request. The final HIR leaf payload and atomic public
HIR switch remain pending that corrected contract; this cut does not infer its
schema.

LSP inlay projection nevertheless had an independent duplicate semantic
authority. When the selected profile had no accepted project, or when the open
document was not the exact accepted source, the feature lowered a detached
syntax tree, built a local callable registry, validated it, and ran a second
type checker. That fallback could publish facts from a different source world
than the compiler and accepted LSP project.

## Deleted authority

- the inlay feature's detached `lower_to_hir` path;
- its local callable-registry and HIR-reference validation pass;
- its local type-check-readiness validation and `analyze_types` pass;
- the source-less judgment fallback that accepted semantic facts without an
  exact `SourceDocumentIdentity`.

Inlay hints now require all of the following:

1. an accepted profile environment;
2. a source in that exact accepted `HirProject` selected by the open URI;
3. an editor overlay rebound to the accepted logical source ID whose complete
   revision identity equals the accepted source identity;
4. judgments owned by that accepted source identity.

A missing accepted project, stale editor bytes, or a foreign URI produces an
empty result. No compatibility alias, renamed fallback, dual semantic reader,
source gate, or spelling-specific diagnostic replaces the deleted path.

## Retained boundary

The feature still parses the already-matched bytes to locate presentational
inlay sites. It does not lower or type-check that detached tree, and all type
facts come from the accepted project. Deleting the remaining detached syntax
site reader belongs to the atomic Proof Stage 3/6 authority switch after the
corrected `.1.1.4.1` HIR leaf contract arrives; this preparatory deletion is not
credited as that switch.

## Validation

- the independently audited working change was Jujutsu change
  `yuutkmtwnwwwzryqvpmzmutkmsoptztx` over parent `d851fbd2`;
- `cargo test -p arcweft-lsp --lib inlay_ -- --nocapture`: passed, 4 tests;
- `cargo test -p arcweft-lsp --lib`: passed, 211 tests;
- `cargo check -p arcweft-lsp --all-targets --all-features`: passed;
- `cargo clippy -p arcweft-lsp --all-targets --all-features -- -D warnings`:
  passed;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `just test-workspace`: all preceding workspace and compile-fail suites passed,
  then the final `arcw_fixtures_check_run` suite retained the pre-existing two
  `FsError` failures:
  `spec_should_pass/check/010_capability_fs_read.arcw` and
  `spec_should_pass/run/002_file_read_task.arcw`; no cut-local failure appeared;
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/proof-lsp-inlay-semantic-fallback-deletion-2026-07-27`:
  scanned 3,695 files, 1,932 Rust files, and 902,142 Rust physical LOC; reported
  0 errors and 144 existing warnings. Exact measurements and dependency evidence
  are retained in the
  [structure audit](structure-audits/proof-lsp-inlay-semantic-fallback-deletion-2026-07-27/violations.md).

The changed production file `features/inlay.rs` is 15,171 bytes / 454 physical
LOC. The changed test owner `session/tests.rs` is 70,381 bytes / 2,093 physical
LOC, below the 2,500-LOC integration-test warning threshold. This cut removes
semantic-analysis responsibility from the production module, adds no crate
dependency or facade re-export, and introduces no new structural warning.

Tier 2 is not required for this LSP-only semantic-projection cut: it does not
change runtime, rendering, Agent, MCP, or capture behavior.

## Remaining boundary

The final typed HIR leaf payload, attached syntax/HIR publication, runtime
assertion inventory and codec migration, and save/replay identity remain open.
They must not be implemented from the rejected `.1.1.4` archive.
