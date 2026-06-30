# Seq04.8.2 full-build orchestration actual builder integration

Date: 2026-06-30

This cut wires the ordinary `arcw build` write-through route to the seq04.8.1
actual bytecode/link persistent object builders for the producer family that can
prove every required identity from normal build output: a single-module,
single-compile-unit project whose product AWBC is exactly the build unit.

## Implemented boundary

- `ProjectBuildBundleOutput` carries generated or cached AWFB bytes together
  with a `FullBuildPersistentArtifactContext` before persistent query
  write-through.
- The context extracts canonical AWBC bytes from the AWFB `ProgramBytecode`
  section instead of deriving bytecode identity from paths or fixtures.
- The context uses the emitted runtime-plan artifact bytes as the
  runtime-plan-unit digest only for the covered single-unit shape.
- `BytecodeUnit` write-through calls `actual_bytecode_unit_payload` when the
  context has runtime-plan, AWBC schema, verifier policy, codegen policy,
  target/query option, feature-set, relocation/import, and canonical AWBC
  identities.
- `LinkPlan` write-through calls `actual_link_plan_payload` when the context has
  stable ordered unit identities plus entrypoint, resource, adapter requirement,
  patch compatibility, and product-option identities.
- Producers outside the covered shape remain conservative and surface typed
  `reuse_evidence` in the build cache report.
- Repeated builds read persistent objects before overwriting them, but this cut
  keeps source rebuild explicit by reporting actual reusable read-through as
  `HitThenRebuilt` with policy
  `full_build_shadow_validation_rebuilt_after_persistent_read_through`.
- The existing `project_commands.rs` embedded test module moved to
  `project_commands/tests.rs` so this package does not push the project command
  facade over the structure-audit error threshold.

## Identity ownership

The CLI/build adapter owns filesystem reads and AWFB inspection. Compiler and
data-format crates stay Sans I/O. The persistent builders remain in
`arcweft-compiler::persistent`, and read-through validation remains in
`arcweft-project-loader::cache::persistent_query`.

`FullBuildPersistentArtifactContext` owns the actual identities after project
compilation and AWFB creation, but before persistent cache write-through:

- runtime-plan unit digest: digest of the emitted runtime-plan artifact bytes;
- canonical AWBC bytes: decoded AWFB `ProgramBytecode` section bytes;
- AWBC schema identity: AWFB program-bytecode section kind and schema version;
- verifier policy identity: AWBC read-through verifier policy used by the
  persistent adapter;
- codegen policy identity: product AWBC lowerer plus emitted AWBC section schema;
- target profile identity: current `CompilerObjectKey::query_options_digest`;
- feature-set identity: sorted/deduped compiler enabled features;
- relocation/import identity: AWFB adapter-requirements section identity;
- link descriptor identity: ordered unit identities plus entrypoint, resource,
  adapter, patch-compatibility, product-option, and dependency-body-root facts.

## Conservative continuation

Multi-module and multi-compile-unit projects are intentionally not promoted.
The current product AWBC is linked-product-wide, so treating it as a reusable
per-module or per-SCC bytecode unit would invent a unit AWBC identity. Those
producers remain `ConservativeRebuild` and are the seq04.8.3 input.

## Validation run

Executed on this checkout:

```bash
cargo fmt --all
cargo test -p arcweft-cli cache_build_writes_persistent_query_evidence_and_preserves_awfb_root --all-features -- --nocapture
cargo test -p arcweft-project-loader persistent_query --all-features -- --nocapture
cargo test -p arcweft-compiler persistent --all-features -- --nocapture
cargo check -p arcweft-cli -p arcweft-project-loader -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-cli -p arcweft-project-loader -p arcweft-compiler --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/structure-audit-seq04-8-2-after-split
cargo fmt --all -- --check
git diff --check
```

Results:

- `cargo test -p arcweft-cli
  cache_build_writes_persistent_query_evidence_and_preserves_awfb_root
  --all-features -- --nocapture`: passed. The test now asserts second-build
  `BytecodeUnit` and `LinkPlan` cache reports carry `actual_reusable` evidence
  with `hit_then_rebuilt` status.
- `cargo test -p arcweft-project-loader persistent_query --all-features --
  --nocapture`: 28 passed.
- `cargo test -p arcweft-compiler persistent --all-features -- --nocapture`: 8
  passed.
- `cargo check` and focused `cargo clippy ... -D warnings`: passed.
- Structural audit after moving the embedded tests: 2,116 files scanned, 1,042
  Rust files, 492,884 Rust physical LOC, 0 error(s), 124 warning(s).
- Final `cargo fmt --all -- --check` and `git diff --check`: passed.

Structure measurements for the files touched by this package:

- `crates/arcweft-cli/src/app/project_commands.rs`: 81,062 bytes, 2,196
  physical LOC, production, no embedded tests. It remains above the 1,200 LOC
  warning threshold and should be split further by project build/check/watch
  responsibilities in a future structure cut.
- `crates/arcweft-cli/src/app/project_commands/tests.rs`: 11,568 bytes, 348
  physical LOC, test module.

Fixture snippets live under `fixtures/persistent-cache-build/seq04-8-2/`. They
document the expected second-build cache report and cache-explain read-through
shape without pinning build-dependent digests.

## Remaining conservative producers

- Multi-module and multi-compile-unit actual bytecode/link identities remain
  conservative.
- Ordinary full-build orchestration still rebuilds after read-through; this cut
  records actual hits as shadow validation, not source-skipping execution.
- The normal build-path CLI golden for the complete user-facing JSON output is
  still a follow-up once the full build output shape stabilizes.
