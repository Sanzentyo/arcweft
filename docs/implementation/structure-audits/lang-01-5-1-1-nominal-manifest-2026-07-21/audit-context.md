# Lang-01.5.1.1 nominal identity and manifest audit context

## Snapshot

- Base Git revision: `f08a5074e7e7df7a477b176c199e08cb0fd69b38`
- Jujutsu change: `kyrxwztpntrqpkttuqlyoqtlwzwyzour`
- Pre-description working commit at measurement: `72de2c367db57ad4af5552214ee404e47f797227`
- Files scanned: 3,468
- Rust files: 1,804
- Rust physical LOC: 830,297
- Cargo manifests: 94
- Result: 0 errors, 131 pre-existing warnings

The generated [file metrics](file_metrics.csv), [dependency edges](dependency_edges.csv),
[public-type duplicate inventory](public_type_duplicates.csv), and
[violation report](violations.md) are the exact audit output for this snapshot.

## Changed Rust files

| Path | Bytes | Physical LOC | Class | Embedded tests |
|---|---:|---:|---|---|
| `crates/arcweft-bundle/src/lib.rs` | 82,536 | 2,264 | production | yes |
| `crates/arcweft-bundle/src/resource_codec/view/codec.rs` | 54,342 | 1,501 | production | no |
| `crates/arcweft-bundle/src/resource_codec/view/codec/part.rs` | 5,335 | 138 | production | no |
| `crates/arcweft-bundle/src/resource_codec/view/model/part.rs` | 2,781 | 100 | production | no |
| `crates/arcweft-bundle/src/standard_view.rs` | 15,469 | 409 | production | no |
| `crates/arcweft-bundle/tests/standard_dialogue_view.rs` | 17,331 | 497 | test | no |
| `crates/arcweft-bundle/tests/view_product_validation.rs` | 14,180 | 408 | test | no |
| `crates/arcweft-bundle/tests/view_resource_codecs.rs` | 60,493 | 1,645 | test | no |
| `crates/arcweft-bundle/tests/view_style_program.rs` | 11,234 | 280 | test | no |
| `crates/arcweft-cli/src/app/bundle_view/lowering.rs` | 44,792 | 1,161 | production | no |
| `crates/arcweft-cli/src/app/bundle/tests.rs` | 86,261 | 2,704 | test | no |
| `crates/arcweft-cli/src/app/bundle/tests/view_sidecars.rs` | 12,179 | 348 | test | no |
| `crates/arcweft-compiler/src/style.rs` | 30,713 | 835 | production | no |
| `crates/arcweft-compiler/src/view_part.rs` | 5,816 | 168 | production | no |
| `crates/arcweft-dialogue/src/character_dialogue/schema.rs` | 29,440 | 841 | production | no |
| `crates/arcweft-id/src/lib.rs` | 10,655 | 377 | production | yes |
| `crates/arcweft-launch/src/accepted.rs` | 15,253 | 441 | production | yes |
| `crates/arcweft-launch/src/decode/tests.rs` | 60,730 | 1,675 | test | no |
| `crates/arcweft-launch/src/decode/values/profile/policy.rs` | 24,240 | 774 | production | no |
| `crates/arcweft-launch/src/diagnostic.rs` | 7,603 | 226 | production | no |
| `crates/arcweft-launch/src/manifest.rs` | 8,647 | 260 | production | no |
| `crates/arcweft-launch/src/source_map.rs` | 16,475 | 514 | production | no |
| `crates/arcweft-player-web/src/inset_shadow_exact_capture.rs` | 28,372 | 794 | production | no |
| `crates/arcweft-player-web/tests/parity.rs` | 38,926 | 1,141 | test | no |
| `crates/arcweft-runtime-driver/src/view_runtime.rs` | 51,419 | 1,343 | production | no |
| `crates/arcweft-runtime-driver/src/view_runtime/axis_seed_tests.rs` | 31,662 | 870 | test | no |
| `crates/arcweft-runtime-driver/src/view_runtime/catalog.rs` | 21,787 | 573 | production | no |
| `crates/arcweft-runtime-driver/src/view_runtime/catalog/fingerprint.rs` | 9,013 | 250 | production | no |
| `crates/arcweft-runtime-driver/src/view_runtime/dialogue_acceptance_tests.rs` | 12,839 | 341 | test | no |
| `crates/arcweft-runtime-driver/src/view_runtime/evaluator.rs` | 66,474 | 1,623 | production | no |
| `crates/arcweft-runtime-driver/src/view_runtime/replacement/reconcile.rs` | 6,628 | 188 | production | no |
| `crates/arcweft-runtime-driver/tests/dialogue_restore/mod.rs` | 7,824 | 226 | test | no |
| `crates/arcweft-runtime-driver/tests/session.rs` | 112,435 | 3,075 | test | no |
| `crates/arcweft-runtime-driver/tests/view_runtime.rs` | 94,867 | 2,628 | test | no |
| `crates/arcweft-runtime-plan/src/render_text/speaker_preset.rs` | 6,214 | 192 | production | no |
| `crates/arcweft-view/src/lib.rs` | 8,064 | 171 | production | no |
| `crates/arcweft-view/src/program.rs` | 34,929 | 1,037 | production | yes |
| `crates/arcweft-view/src/style.rs` | 6,612 | 193 | production | no |
| `crates/arcweft-view/src/style/axis.rs` | 22,922 | 764 | production | yes |
| `crates/arcweft-view/src/style/sheet.rs` | 32,012 | 1,014 | production | no |
| `crates/arcweft-view/src/view.rs` | 3,136 | 98 | production | no |
| `crates/arcweft-view/src/view/identity.rs` | 11,431 | 362 | production | yes |
| `crates/arcweft-view/tests/logical_axis_cascade.rs` | 17,041 | 496 | test | no |
| `crates/arcweft-view/tests/logical_axis_provider.rs` | 62,091 | 2,118 | test | no |
| `crates/arcweft-view/tests/style_sheet.rs` | 31,606 | 942 | test | no |

The identity files own the new family invariants. The bundle model and codecs
carry `ViewId` without an unchecked projection. Launch owns the sole manifest
decoder, structured diagnostics, and revision-bound token paths. Compiler,
CLI, runtime, player, and test changes are direct typed call-site migrations;
they do not add a second registry, decoder, or compatibility surface.

## Largest workspace Rust files

The largest current non-generated Rust files are test matrices rather than new
production owners: 7,062, 6,717, 6,109, 5,977, 5,257, 4,285, 4,218, and
3,838 LOC for the leading eight rows in `file_metrics.csv`. The largest changed
production files are existing `arcweft-bundle/src/lib.rs` (2,264 LOC),
`view_runtime/evaluator.rs` (1,623 LOC), `view/codec.rs` (1,501 LOC), and
`view_runtime.rs` (1,343 LOC). This cut changes only nominal projections in
those hotspots; no orchestration responsibility was added.

## Dependency boundary

`arcweft-runtime-driver` now has a direct normal dependency on `arcweft-id`
because semantic attachment targets remain generic `PublicId` values while
View definition owners are nominal `ViewId` values. The measured crate has
fan-out 15 and fan-in 6. The edge points downward to the identity crate and
does not create a cycle or a higher-layer dependency.
