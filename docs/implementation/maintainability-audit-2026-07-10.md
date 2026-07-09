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
