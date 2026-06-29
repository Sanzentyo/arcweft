# Seq04.8.1 bytecode/link actual reuse boundary implementation

Date: 2026-06-30

This implementation graduates the ready subset of seq04.8 bytecode/link cache gates from conservative evidence to actual reusable facts.

## Implemented boundary

- `BytecodeUnitObject` persists canonical AWBC bytes and typed identity facts.
- `LinkPlanObject` persists a typed, order-preserving link descriptor.
- Conservative seq04.8 payloads remain valid and still force rebuild/relink.
- Actual `VerifiedReusable` payloads become `CacheRecordStatus::Hit` only after validation.
- Bytecode read-through performs AWBC decode, semantic verify, and exact re-encode round trip.
- Link-plan read-through validates ordered unit/resource identity through the descriptor digest and optional current descriptor expectation.
- Cache explain distinguishes actual reusable hits from conservative gates.

## Validation

Run:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-project persistent_object --all-features
cargo test -p arcweft-project-loader persistent_query --all-features
cargo test -p arcweft-compiler persistent_query --all-features
cargo test -p arcweft-bundle --all-features
cargo test -p arcweft-cli cache --all-features
cargo check -p arcweft-project -p arcweft-project-loader -p arcweft-compiler -p arcweft-bundle -p arcweft-cli --all-targets --all-features
cargo clippy -p arcweft-project -p arcweft-project-loader -p arcweft-compiler -p arcweft-bundle -p arcweft-cli --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

Applied validation on the implementation working copy:

- `cargo fmt --all -- --check`
- `cargo test -p arcweft-project persistent_object --all-features`
- `cargo test -p arcweft-project-loader persistent_query --all-features`
- `cargo test -p arcweft-compiler persistent_query --all-features`
- `cargo test -p arcweft-bundle --all-features`
- `cargo test -p arcweft-cli cache --all-features`
- `cargo check -p arcweft-project -p arcweft-project-loader -p arcweft-compiler -p arcweft-bundle -p arcweft-cli --all-targets --all-features`
- `cargo clippy -p arcweft-project -p arcweft-project-loader -p arcweft-compiler -p arcweft-bundle -p arcweft-cli --all-targets --all-features -- -D warnings`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`
- package `verification/check-fixture-equivalence.rs` against `fixtures/persistent-cache-bytecode-link/seq04-8-1`
- `git diff --check`

Structural audit final measurement:

- Files scanned: 2092
- Rust files: 1034
- Rust physical LOC: 486271
- Package manifests: 91
- Violations: 0 errors, 124 warnings

Changed Rust file size measurement:

| Path | Bytes | Physical LOC | Role |
| --- | ---: | ---: | --- |
| `crates/arcweft-project/src/persistent_object.rs` | 1299 | 25 | facade |
| `crates/arcweft-project/src/persistent_object/payload.rs` | 38536 | 973 | production data contract |
| `crates/arcweft-project/src/persistent_object/codec.rs` | 62428 | 1442 | production codec plus unit tests |
| `crates/arcweft-project-loader/src/cache/persistent_query.rs` | 64517 | 1655 | production adapter validation |
| `crates/arcweft-project-loader/src/cache/persistent_query/tests.rs` | 43171 | 1078 | unit tests |
| `crates/arcweft-compiler/src/persistent.rs` | 51558 | 1270 | production builders plus unit tests |

## Remaining conservative boundary

`TypecheckGateObject` remains conservative. Bytecode/link producers without seq04.7 runtime-plan identity, canonical AWBC bytes, or stable link descriptor continue to write `ConservativeRebuild` payloads.

## Follow-up candidates

- Full build orchestration actual builder integration:
  `docs/reviews/requests/2026-06-30-seq-04.8.2-full-build-orchestration-actual-builder-integration.md`
- Bytecode/link producer identity closure and conservative continuation:
  `docs/reviews/requests/2026-06-30-seq-04.8.3-bytecode-link-producer-identity-closure.md`
- Normal build CLI goldens and cache evidence:
  `docs/reviews/requests/2026-06-30-seq-04.8.4-normal-build-cli-goldens-cache-evidence.md`
