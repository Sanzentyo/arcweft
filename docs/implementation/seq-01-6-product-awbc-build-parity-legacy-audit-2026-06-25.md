# seq-01.6 product AWBC build wiring, parity gate, and legacy audit

Status: `IMPLEMENTED_AND_VERIFIED`.

Base revision: `04660f5348bfbc6e7c1fe156a008fb7df6cf5cef`.

## Implemented cut

- Normal direct/project/profile compilation lowers canonical product AWBC with
  `arcweft-runtime-plan::awbc_lower::AwbcLowerer`.
- The shared CLI bundle builder attaches that program through
  `ArcweftBundle::with_product_awbc` before any ordinary AWFB writer runs.
- Project build/watch and native/web run/watch therefore share one product
  executable producer boundary.
- AWBC lowering failures are surfaced in the `product_awbc_lower` build phase
  with lowering-path or verifier diagnostics, before the codec boundary.
- `AwbcProgram` inventories unsupported product-step families through an owned,
  typed API. Product execution rejects those families before a compact fiber is
  created.
- Runtime-driver and runtime-host preserve the typed AWBC product-step
  construction error. Runtime-driver keeps canonical-AWBC verification failures
  as a separate typed category.
- `arcweft-core::compact_bytecode` is removed because the current-revision code
  import gate is empty.
- Canonical AWBC generation identity receives a focused regression test.
- Source gates cover the shared product builder and compact residue deletion.

## Implementation adjustments after package apply

The package applicator matched the requested base revision and applied cleanly.
Local Rust verification then exposed two producer-side canonicalization issues
that were fixed in this checkout:

- `AwbcProgram::canonicalize_string_table` now sorts and deduplicates the owned
  AWBC string table while remapping every `AwbcStringId` reference before
  verification or encoding.
- Flow entry blocks lowered by `arcweft-runtime-plan::awbc_lower` now use
  `AwbcSafePointKind::FlowEntry`, matching the AWBC verifier's function-entry
  safe-point contract.

These are not product-format deviations. They make the existing seq-01.6
producer wiring emit verifier-clean canonical AWBC from ordinary source builds.

## Product-default completion boundary

Builder correctness is complete for the reviewed producer paths: a source-built
product AWFB carries canonical AWBC and no structured fallback is added.

Runtime-step parity is **not** complete. The following executable families are
blocked before product execution until seq-01.6.1 closes their differential
contract:

- entry arguments/root bindings;
- pure helpers and intrinsics;
- potentially trapping expression/pattern operations until source-map reporting
  is complete;
- content, effects, tasks and spawned fibers;
- streams and sources;
- dialogue and choice;
- await and await-many;
- host calls;
- explicit budget yield and trap terminators.

The accepted runtime subset is deliberately small and deterministic. This is a
hard product safety boundary, not a compatibility fallback.

## Build-path result

All source-produced AWFB paths converge on:

```text
compile_profile_runtime_plan
  -> AwbcLowerer
  -> ProfileCompiledRuntimePlan.product_awbc
  -> compile_bundle_for_selection
  -> ArcweftBundle::with_product_awbc
  -> AWFB encoder
```

Headless source execution that does not write AWFB remains dev/source-only and
may use structured VM/AOT tiers.

## Legacy result

- `compact_bytecode`: deleted; zero code import gate was satisfied.
- `BytecodeVmExecutor`, `StructuredVm`, `StructuredAot`: retained for explicit
  dev/source/test use.
- structured constructors accepting `AwbcProduct`: deletion-gated residue; not
  a valid product constructor.
- `arcweft-agent-runner::run_controller_bundle`: classified as an Agent-product
  blocker because it executes `bundle.bytecode.program`.

## Identity and consistency

Ordinary product builds now place canonical AWBC bytes in the encoded
`ProgramBytecode` section. Container/patch identity therefore observes those
bytes. `ProgramGeneration::from_bundle` already hashes canonical AWBC as the
`__awbc_program` code slot; the package adds a regression test showing an AWBC
byte change changes generation code identity while content remains unchanged.

`AwbcLowerer` receives the same validated plan and display catalog used to
assemble the bundle. No filesystem, signing key, clock, network, or platform
operation is introduced into core/bundle/lowering data layers.

## Verification record

The package assembler did not have a complete private checkout. The
implementation agent applied the package in this checkout, made the adjustments
above, and ran the post-apply verification matrix on 2026-06-26.

Package/applicator checks:

```bash
uv run python apply_seq_01_6.py --self-test
uv run python verification/package_static_check.py
uv run python apply_seq_01_6.py --repo . --check-only
uv run python apply_seq_01_6.py --repo .
```

All four checks passed. The `--check-only` pass confirmed the base revision and
all exact edit contexts for `04660f5348bfbc6e7c1fe156a008fb7df6cf5cef`.

Rust verification:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-core -p arcweft-runtime-plan -p arcweft-compiler \
  -p arcweft-bundle -p arcweft-cli -p arcweft-runtime-driver \
  -p arcweft-runtime-host -p arcweft-player-native --all-targets
cargo test -p arcweft-core awbc_product_step -- --nocapture
cargo test -p arcweft-bundle product_awbc -- --nocapture
cargo test -p arcweft-cli compile_bundle_for_selection_attaches_product_awbc -- --nocapture
cargo test -p arcweft-cli project_bundle_for_selection_attaches_product_awbc -- --nocapture
cargo test -p arcweft-bundle project_and_run_awfb_writers_share_product_bundle_builder -- --nocapture
cargo test -p arcweft-runtime-driver awbc_product -- --nocapture
cargo test -p arcweft-runtime-driver generation_from_bundle_uses_canonical_product_awbc_identity -- --nocapture
cargo test -p arcweft-runtime-host awbc_product -- --nocapture
cargo test -p arcweft-player-native awbc_product -- --nocapture
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Results:

- `cargo fmt --all -- --check`: passed.
- Focused multi-package `cargo check`: passed.
- `cargo test -p arcweft-core awbc_product_step`: passed, 3 tests.
- `cargo test -p arcweft-bundle product_awbc`: passed, 6 tests across product
  codec and source gates.
- `cargo test -p arcweft-bundle project_and_run_awfb_writers_share_product_bundle_builder`:
  passed, 1 source-gate test.
- `cargo test -p arcweft-cli compile_bundle_for_selection_attaches_product_awbc`:
  passed, 1 test.
- `cargo test -p arcweft-cli project_bundle_for_selection_attaches_product_awbc`:
  passed, 1 test.
- `cargo test -p arcweft-cli patch_bundle_artifact_helper_diffs_base_and_next_awfb_bytes`:
  passed, 1 test.
- `cargo test -p arcweft-runtime-driver awbc_product`: passed, 1 integration
  test.
- `cargo test -p arcweft-runtime-driver generation_from_bundle_uses_canonical_product_awbc_identity`:
  passed, 1 test.
- `cargo test -p arcweft-runtime-host awbc_product`: passed, 1 integration
  test.
- `cargo test -p arcweft-player-native awbc_product`: passed, 1 integration
  test.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed.
- Structural audit passed with `1508` files scanned, `830` Rust files,
  `404241` Rust physical LOC, `89` package manifests, `0` errors, and `99`
  warnings.

## Non-goals retained

- no redesign of AWBC canonical encoding or seq-01.5 product markers;
- no structured product fallback;
- no optimized native/AOT compiled regions;
- no one-shot external migration tool for old products;
- no premature deletion of structured dev/source tiers;
- no claim that seq-01.4.1 parity is complete.
