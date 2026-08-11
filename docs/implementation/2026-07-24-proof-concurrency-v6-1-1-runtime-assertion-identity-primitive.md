# Proof-concurrency v6.1.1 runtime assertion identity primitive

Date: 2026-07-24

## Scope

This cut isolates the fixed-width identity and runtime-only assertion mode
vocabulary required before the final Proof-concurrency v6.1.1 runtime
assertion authority switch. Its accepted design source is:

```text
docs/reviews/packages/zips/arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip
SHA-256: 1b7de5f2c10a5b29d67c72011e4272df9a76af8907fd21fe162de54809fc69ef
```

The repository-retained archive matches that outer SHA-256; all 19 declared
content-member hashes match `MANIFEST.txt`, `OPEN_QUESTIONS.md` is `none`, and
`FINAL_STATUS.md` is ready for implementation.

The implementation revision is Jujutsu change `mlmymkkkxpsl`. This cut is
based directly on `main` revision `af68c31f3e25` and contains no other active
goal slice.

## Implemented boundary

- `RuntimeAssertionGuardId` owns the exact 16-byte persisted guard and rejects
  the reserved all-zero representation during ordinary construction and
  deserialization.
- `RuntimeArtifactFingerprint` owns the exact 32-byte runtime-plan artifact
  fingerprint and applies the same checked decode rule.
- `RuntimeAssertionMode` represents only runtime-capable `Check` and `Debug`
  modes. Typed conversion rejects `Prove`; no string parsing or runtime
  compatibility spelling exists.
- `AssertionConditionIndex` validates the one-to-64 authored condition limit
  and bounds-checks the selected zero-based position before narrowing to
  `u8`.
- The session-local mode and condition index deliberately implement no Serde.

Core remains Sans I/O and has no syntax or HIR dependency. Runtime-plan owns
the conversion from the existing typed HIR assertion mode.

## Explicit non-goals

This primitive does not add a guard to `RuntimeAssertion`, derive canonical
guard seeds, publish an assertion site/inventory, or change AWBC, bundle,
cache, save, checkpoint, replay, CLI, LSP, Agent, or MCP payloads. Those changes
require the final typed HIR/project identity and must land as an authority
switch without a guard-less dual reader. This cut therefore does not claim the
Proof-concurrency package complete and does not require Tier 2 by itself.

## Validation

The isolated cut produced these results on Jujutsu change
`mlmymkkkxpsl`:

- `cargo test -p arcweft-core effect::assertion_identity::tests` — passed,
  2 passed and 0 failed;
- `cargo test -p arcweft-runtime-plan assertion_identity::tests` — passed,
  2 passed and 0 failed;
- `cargo check --workspace --all-targets --all-features` — passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` —
  passed;
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` — passed its
  error gate after scanning 3,605 files, 1,899 Rust files, 885,912 Rust
  physical LOC, and 94 package manifests; it reported 0 errors and 142
  existing warnings; and
- `just test-workspace` — ran the broad normal suite, including the new core
  and runtime-plan tests, but the recipe remains non-green because the two
  already documented `arcweft-cli --test arcw_fixtures_check_run` functions
  `spec_should_pass_check_fixtures_pass_after_refactor` and
  `spec_should_pass_run_fixtures_pass_after_refactor` fail on
  `010_capability_fs_read.arcw` and `002_file_read_task.arcw`.

Those two fixtures declare `type FsError` inside `extern capability fs`. The
public detached capability AST/HIR path still publishes functions but not the
owned type member, so sema reports `sema.nominal.unknown_type` for `FsError`.
This pre-existing authority gap is recorded in
`2026-07-23-lang-01-1-1-2-2-adapter-callable-nominal-publication.md` and must
be closed by the Proof-concurrency atomic public syntax/HIR switch. A global
`FsError`, named-type fallback, compatibility alias, or fixture-only bypass
was deliberately not added.

Tier 2 was not run for this primitive-only cut. It changes neither a runtime,
render, Agent, MCP, nor capture execution path; the later assertion
site/inventory plus AWBC/save/replay authority switch is the qualifying broad
cut and must run Tier 2.

## Structural measurements

Exact current-checkout measurements are:

| Path | Owner/class | Bytes | Physical LOC | Embedded test LOC | Responsibility |
| --- | --- | ---: | ---: | ---: | --- |
| `crates/arcweft-core/src/effect/assertion_identity.rs` | `arcweft-core`, production with unit tests | 4,166 | 114 | 40 | Persistable fixed-width guard/fingerprint identity and checked decode |
| `crates/arcweft-core/src/effect.rs` | `arcweft-core`, production boundary | 12,096 | 373 | 0 | Effect model and deliberate assertion-identity re-export |
| `crates/arcweft-runtime-plan/src/assertion_identity.rs` | `arcweft-runtime-plan`, production with unit tests | 4,622 | 126 | 45 | Runtime-only typed mode conversion and bounded authored condition index |
| `crates/arcweft-runtime-plan/src/lib.rs` | `arcweft-runtime-plan`, facade | 369 | 20 | 0 | Responsibility-module publication |

No Cargo edge, feature, schema, root-level compatibility re-export, or manual
cross-crate field projection was added. The two new responsibility modules are
well below warning thresholds, and every previously existing changed file
remains below its applicable structural warning threshold.
