# seq-01.5 product AWFB bytecode migration implementation note

Status: `IMPLEMENTED_AND_VERIFIED`.

This note records the Rust workspace state after applying and adjusting the
seq-01.5 package from
`D:/sanze/Downloads/arcweft-seq-01.5-product-awfb-bytecode-migration-2026-06-25.zip`.

## Implemented cuts in this package

- Product AWFB executable payload is canonical AWBC (`AwbcProgram::encode_canonical`).
- Structured product `BytecodeProgram` and compact sidecar tags are rejected, not executed.
- Product runtime load paths are routed through the shared runtime executor facade using an AWBC product tier.
- Source gates cover product bundle, runtime-driver, runtime-host, and native-player paths.
- Documentation and schema examples describe `executable_payload = awbc_v1`.
- Bundle schema version is bumped to 4 for new product manifests carrying the
  AWBC executable payload marker.
- `ProgramGeneration::from_bundle` uses canonical AWBC bytes for product code
  identity when `product_awbc` is present, so patch/generation classification
  changes when the AWBC executable changes.

## Verification state

The package author did not run Rust commands. The implementation agent ran the
package command matrix in this checkout on 2026-06-25:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-core -p arcweft-bundle -p arcweft-runtime-driver -p arcweft-runtime-host -p arcweft-player-native --all-targets
cargo test -p arcweft-core awbc_product_step -- --nocapture
cargo test -p arcweft-bundle product_awbc -- --nocapture
cargo test -p arcweft-runtime-driver awbc_product -- --nocapture
cargo test -p arcweft-runtime-host awbc_product -- --nocapture
cargo test -p arcweft-player-native awbc_product -- --nocapture
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Results:

- `cargo fmt --all -- --check`: passed.
- Focused target `cargo check`: passed.
- `cargo test -p arcweft-core awbc_product_step`: passed, 1 test.
- `cargo test -p arcweft-bundle product_awbc`: passed, 6 integration tests
  across AWBC-only encode/decode and source gates.
- `cargo test -p arcweft-runtime-driver awbc_product`: passed, 1 integration
  test.
- `cargo test -p arcweft-runtime-host awbc_product`: passed, 1 integration
  test.
- `cargo test -p arcweft-player-native awbc_product`: passed, 1 integration
  test.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed.
- Structural audit passed with `1506` files scanned, `831` Rust files,
  `403992` Rust physical LOC, `89` package manifests, `0` errors, and `99`
  warnings.

Additional source-gate check:

```bash
rg -n "bundle\.bytecode\.program\.clone\(\)|from_bytecode_parts\(|BytecodeVmExecutor::new|BYTECODE_SECTION_STRUCTURED|CompactBytecodeProgram" crates/arcweft-bundle/src/product.rs crates/arcweft-runtime-driver/src/session.rs crates/arcweft-runtime-host/src/bundle_runner.rs crates/arcweft-player-native/src/lib.rs
```

This returned no matches in the product codec/runtime files.

## Structural audit measurements

Revision inspected: Jujutsu working copy on `main` after applying seq-01.5.

Changed Rust files:

| Path | Kind | Bytes | LOC | Embedded test LOC | Responsibilities |
|---|---:|---:|---:|---:|---|
| `crates/arcweft-bundle/src/lib.rs` | production/facade | 64175 | 1820 | 574 | bundle data model, schema version, product AWBC public API |
| `crates/arcweft-bundle/src/product.rs` | production | 24055 | 647 | 262 | AWFB section encode/decode, AWBC-only product payload rejection gates |
| `crates/arcweft-bundle/src/product_awbc.rs` | production | 1942 | 54 | 0 | typed product AWBC section codec |
| `crates/arcweft-bundle/tests/product_awbc_only.rs` | integration test | 5783 | 164 | 0 | AWBC-only product encode/decode diagnostics |
| `crates/arcweft-bundle/tests/product_awbc_source_gates.rs` | integration test | 1415 | 36 | 0 | product runtime/source forbidden-term gates |
| `crates/arcweft-core/src/awbc.rs` | facade | 526 | 18 | 2 | AWBC module namespace |
| `crates/arcweft-core/src/awbc/product_step.rs` | production | 24977 | 622 | 18 | AWBC VM/fiber to `RuntimeStepResult` parity adapter |
| `crates/arcweft-core/src/awbc/schema_impls.rs` | production | 1006 | 30 | 0 | AWBC conversion impls |
| `crates/arcweft-core/src/executor.rs` | production | 14569 | 464 | 0 | shared runtime executor facade, AWBC product tier |
| `crates/arcweft-runtime-driver/src/session.rs` | production | 27352 | 755 | 0 | portable bundle session, product AWBC executor selection |
| `crates/arcweft-runtime-driver/src/swap.rs` | production | 23199 | 722 | 235 | generation identity and swap classification, AWBC code digest |
| `crates/arcweft-runtime-driver/tests/awbc_product_session.rs` | integration test | 3472 | 99 | 0 | AWBC product session smoke |
| `crates/arcweft-runtime-host/src/bundle_runner.rs` | production | 39152 | 1138 | 259 | native bundle runner, default AWBC product executor |
| `crates/arcweft-runtime-host/tests/awbc_product_runner.rs` | integration test | 255 | 7 | 0 | runner default executor gate |
| `crates/arcweft-player-native/src/lib.rs` | production | 12083 | 334 | 168 | native/headless player runtime metadata |
| `crates/arcweft-player-native/tests/awbc_product_input.rs` | integration test | 227 | 9 | 0 | player metadata executor gate |
| `crates/arcweft-cli/src/app/runtime/options.rs` | production | 13702 | 377 | 0 | CLI/runtime executor enum mapping |
| `crates/arcweft-cli/src/app/runtime/executor.rs` | production | 3165 | 98 | 0 | CLI source-runtime executor tier mapping |
| `crates/arcweft-cli/src/output.rs` | production | 48456 | 1382 | 0 | CLI JSON output contract |

Largest Rust files in this checkout remain existing hotspots unrelated to this
cut: `arcweft-text-layout/src/vertical_orientation.rs` (`12399` LOC), CLI
exact-check fixtures such as `cli_runtime_bench.rs` (`7945` LOC), and native
observe/check fixtures above `4000` LOC. No new error-level ownership threshold
was introduced by seq-01.5. Changed production files remain below the 2500 LOC
error threshold; changed `lib.rs`/output facade hotspots are pre-existing and
this cut added only narrow product AWBC API/metadata variants.

## Remaining boundary after merge

After all source gates and runtime smoke tests pass, remove the old `arcweft-core::compact_bytecode` sidecar module if no non-test path imports it.

No filesystem, network, signing-key, wall-clock, or platform I/O was added to
`arcweft-core` or `arcweft-bundle`. No structured `BytecodeProgram` product
fallback is retained for decoded product `.awfb` execution. The old
`arcweft-core::compact_bytecode` module remains a non-goal for this cut and is
left for the documented deletion gate.

Follow-up design/review request:
`docs/reviews/requests/2026-06-25-seq-01.6-product-awbc-build-parity-and-legacy-audit.md`
records the post-merge audit items for ordinary AWFB builder wiring, compact
AWBC runtime-step parity review, and remaining legacy/deletion-gate
classification.
