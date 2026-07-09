# Maintainability audit — 2026-07-10

## Scope

This audit tracks removal of ad hoc implementation, redundant wrappers and
low-value tests, clearer responsibility boundaries, and implementation/spec
gaps found while reviewing the current Rust workspace. Stable design chapters
are changed only when a design contract changes; this file records checkout
measurements, implementation decisions, validation, and remaining work.

The audit started from Git revision `a59be17c2455` / Jujutsu change
`qrkrvnuumznk`. The working copy was clean.

## Baseline

The canonical structural audit reported:

```text
files scanned: 2520
Rust files: 1180
Rust physical LOC: 588316
package manifests: 91
violations: 1 error(s), 152 warning(s)
```

The only error-level finding was
`crates/arcweft-cli/src/app/bundle_view.rs`: 91,581 bytes and 2,590 physical
LOC. It mixed stateful component View lowering with deterministic schema
identity serialization. It is production code, contains no embedded test
module, and belongs to `arcweft-cli`. This slice does not change Cargo
dependencies, so workspace dependency fan-in and fan-out are unchanged.

## Slice 1: isolate View schema identity generation

`bundle_view_schema.rs` now owns the deterministic conversion from View
expressions and patterns to digest-backed schema references. The module-level
documentation states the invariant: schema identity generation must not depend
on mutable layout or input-resource lowering state. `bundle_view.rs` retains
orchestration, layout, style, focus, input, and resource emission.

Current checkout measurements after the split:

| Path | Kind | Bytes | Physical LOC | Embedded tests | Major responsibilities |
| --- | --- | ---: | ---: | --- | --- |
| `crates/arcweft-cli/src/app.rs` | production facade | 5,352 | 128 | no | CLI application module declarations and command dispatch |
| `crates/arcweft-cli/src/app/bundle_view.rs` | production | 87,720 | 2,468 | no | View lowering orchestration, layout, style, focus, input, and sidecar emission |
| `crates/arcweft-cli/src/app/bundle_view_schema.rs` | production responsibility module | 4,525 | 138 | no | Canonical pattern/expression schema source and deterministic digest references |

The canonical audit now reports zero error-level violations. The remaining
size warning on `bundle_view.rs` is real; later decomposition should separate
scroll/style resolution and text-control authoring as cohesive modules rather
than creating pass-through wrappers.

No new test was added for the file move. Existing bundle behavior tests already
exercise let, await, match, and repeat schema reference emission, so a test that
only asserted the new private module boundary would be implementation-coupled
and low value.

## Confirmed follow-up findings

The audit has confirmed these higher-value implementation issues for separate,
reviewable slices:

- Patch artifacts do not yet recompute and compare every declared section
  fingerprint field against the materialized target, while runtime hot swap
  locally reclassifies a declared compatibility result. The source-inspection
  test for this contract gives false confidence and should be replaced by
  tampered-artifact behavior tests plus typed validation errors.
- `arcweft-lang-sema` manually repeats every `DataFormat` variant even though
  `arcweft-data::DataFormat` owns the enum and its author-facing names. The
  owning enum needs an iterable authoritative inventory, and semantic
  registration must consume it.
- Dialogue speaker and generated text-key normalization is duplicated between
  HIR lowering and ID-context tooling. The implementations have diverged for
  some author spellings and need one shared domain rule with parity tests.
- Pure and engine evaluators duplicate sequence length/index conversion logic.
  Their behavior must be compared before moving shared integer/index behavior
  onto the owning runtime value types.

## Slice 1 validation

```text
cargo fmt --all
cargo check -p arcweft-cli --lib --all-features
cargo test -p arcweft-cli --all-features --lib app::bundle::tests -- --nocapture
  36 passed; 0 failed; 0 ignored
cargo check --workspace
cargo clippy --workspace --all-targets --all-features
  passed with pre-existing warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write target/structure-audit/slice-01
  0 error(s); 153 warning(s)
```

`just test-workspace` reached `arcweft-core` and exposed two pre-existing stale
tests: `engine_steps_flow_ops_and_applies_goto` and
`game_mode_stops_on_visible_output_but_server_mode_drains` send the authored
source ID `say.opening.001` as an input payload, while `RuntimeLineId` now
normalizes that source family to the canonical runtime label `opening.001`.
Both tests also fail when run alone, so this is not test-order interference and
is unrelated to the CLI-only module split. A separate test-contract slice must
construct the advance payload from the typed runtime line ID and then rerun the
workspace gate.

## Slice 2: restore typed test contracts and remove false source gates

The normal workspace route exposed a chain of stale tests that had survived
earlier API and grammar migrations. This slice now derives dialogue advance
payloads and native status labels from `RuntimeLineId`, tests fixed-arity
spread rejection with a genuinely dynamic sequence, pins presentation handle
IDs to their canonical `handle.main.*` form, and updates the persistent-cache
fixtures from removed `start(...)` syntax to `goto`. The multi-module cache
golden now uses the language-owned dot-separated canonical module path
`crate.support`.

Five tests that inspected implementation source or documentation text were
removed instead of being retargeted to new symbol names or file locations:

- one text-control writeback source-spelling test;
- two inset-shadow implementation/collector source-spelling tests;
- the repository-wide removed-word scan;
- the repository-wide host-path text scan.

The corresponding `just` source-scan entrypoints were removed. Inset-shadow
exact PNG packet validation remains, as do the behavior tests for text-control
commands. This avoids converting the same brittle source scan into a differently
named structural rule.

Current changed-Rust-file measurements at Jujutsu change `vxqyvwvwkoqy`:

| Path | Kind | Bytes | Physical LOC | Embedded tests | Major responsibility touched |
| --- | --- | ---: | ---: | --- | --- |
| `crates/arcweft-core/src/tests.rs` | unit-test support | 1,725 | 59 | no | typed dialogue input construction |
| `crates/arcweft-core/src/tests/flow.rs` | unit tests | 88,953 | 2,553 | no | engine flow behavior |
| `crates/arcweft-core/src/tests/step.rs` | unit tests | 6,360 | 198 | no | game/server stepping behavior |
| `crates/arcweft-lang-sema/src/tests/declarations.rs` | unit tests | 42,193 | 1,394 | no | declaration and spread semantics |
| `crates/arcweft-player-native/src/lib.rs` | production facade | 13,734 | 367 | yes | native player orchestration and status reporting |
| `crates/arcweft-player-scene/tests/text_control_writeback_source_gates.rs` | integration tests | 1,400 | 30 | no | text-control command behavior evidence |
| `crates/arcweft-player-web/tests/parity.rs` | integration tests | 29,159 | 845 | no | web/native presentation parity |
| `crates/arcweft-render-wgpu/tests/view_box_shadow_exact_png_golden.rs` | integration tests | 6,275 | 201 | no | exact PNG artifact packets |
| `crates/arcweft-runtime-plan/src/flow/tests.rs` | unit tests | 22,292 | 722 | no | flow-to-runtime-plan lowering |
| `crates/arcweft-cli/tests/regression_harness.rs` | integration tests | 5,061 | 173 | no | checkout hygiene and audited unsafe boundaries |

No Cargo dependency, public production contract, or crate boundary changed, so
dependency fan-in and fan-out are unchanged. The largest non-generated
production files remain `arcweft-core/src/awbc/product_step.rs` (95,176 bytes,
2,499 LOC), `arcweft-core/src/value.rs` (83,955 bytes, 2,498 LOC),
`arcweft-core/src/engine/eval/calls.rs` (89,488 bytes, 2,481 LOC), and
`arcweft-cli/src/app/bundle_view.rs` plus `arcweft-runtime-plan/src/flow.rs`
(both 2,468 LOC). They remain warning-level decomposition candidates; this
test-contract slice does not mix in unrelated production splits.

## Slice 2 validation

```text
cargo fmt --all -- --check
cargo test -p arcweft-core --lib
  172 passed; 0 failed
cargo test -p arcweft-runtime-plan --lib flow::tests
  12 passed; 0 failed
cargo test -p arcweft-cli --test regression_harness
  2 passed; 0 failed
cargo test -p arcweft-cli --test seq04_8_4_persistent_cache_build_cli_goldens
  2 passed; 0 failed
cargo test -p arcweft-render-wgpu --test view_box_shadow_exact_png_golden --all-features
  0 passed; 0 failed; 2 ignored Tier 2 packets
just test-workspace
  passed
cargo clippy --workspace --all-targets --all-features
  passed with pre-existing warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write target/structure-audit/slice-02
  0 error(s); 153 warning(s)
```
