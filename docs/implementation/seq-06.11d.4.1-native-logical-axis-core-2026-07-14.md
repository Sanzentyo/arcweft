# seq-06.11d.4.1 native logical-axis core

- Date: 2026-07-14
- Source package: `arcweft-seq-06.11d.4.1-native-logical-axis-final-contract.zip`
- Status: core implementation complete; design-gated extensions remain explicit

## Outcome

Native Style now has one typed, closed logical-axis context and one canonical
physical computed boundary:

```text
authored declaration
  -> typed token resolution
  -> box-axes winner
  -> shorthand expansion
  -> logical alias and sign resolution
  -> canonical physical-slot cascade
  -> ViewPhysicalBoxStyle
  -> bundle/player/renderer consumers
```

`ViewBoxAxisMode` owns the four supported snapshots and the exact mapping.
`ComputedViewStyle` cannot key `box-axes`, a shorthand, or a logical alias;
winning properties retain authored, expanded, resolved, priority, and source
provenance. Positive logical translation means logical end, and `i32::MIN` is
rejected because it cannot be sign-reversed.

The resolver derives the root default from `HorizontalLtr` and child axes from
the existing parent computed snapshot. It resolves a local axis winner before
ordinary contributions, expands shorthands under the existing work budget,
maps aliases once, canonicalizes transition targets, and keys revisions/cache
entries with effective mode and provider revision.

Bundle projection retains a shared `ViewPhysicalBoxStyle`. The player-scene and
wgpu renderer consume canonical physical translation/size/overflow values;
the former physical/logical fallback chains are removed.

## Implemented acceptance surface

- Closed enum/source/product spelling, canonical tags, validated component
  snapshots, mapping for all 22 logical aliases, and checked translation signs.
- Typed `box-axes` property/value metadata: inherited, non-appendable,
  non-transitionable, and `AXIS_CONTEXT` invalidation.
- Sema expected-type parsing, mode inventory diagnostics, authored lowering,
  and non-reversible logical-translation diagnostics.
- Existing Style-section codec and merge round trips without a sidecar or
  format version shim.
- Canonical computed keys, property provenance, axes/source/revision,
  transitions, usage set, physical packet, cache identity, and canonical diff.
- Two-phase resolution, shorthand expansion, physical/logical winner
  competition under the unchanged priority tuple, token overflow protection,
  and parent inheritance.
- Canonical bundle, player, and renderer consumption.

## Intentionally excluded, design-gated work

The supplied package names broader outcomes but does not give enough concrete
API/lifecycle/geometry direction to implement these safely. They are not part
of this core completion claim:

- [seq-06.11d.4.1.1 host seed and provider invalidation](../reviews/requests/2026-07-14-seq-06.11d.4.1.1-native-logical-axis-host-seed-provider-invalidation-contract.md):
  public per-mount seed injection, nested runtime propagation, provider/barrier
  dependency indexing, and cache eviction;
- [seq-06.11d.4.1.2 physical box geometry](../reviews/requests/2026-07-14-seq-06.11d.4.1.2-native-physical-box-geometry-contract.md):
  full min/max, padding, margin, inset, containing-block, and downstream
  geometry semantics;
- [seq-06.11d.4.1.3 logical scroll anchoring](../reviews/requests/2026-07-14-seq-06.11d.4.1.3-native-logical-scroll-anchoring-contract.md):
  logical start/end state and negative-axis scroll ranges;
- [seq-06.11d.4.1.4 provider-revision motion](../reviews/requests/2026-07-14-seq-06.11d.4.1.4-native-axis-provider-revision-motion-contract.md):
  cancel/snap/continue behavior across mode or provider changes.

Style tooling remains separately gated by
[seq-06.11d.5.3 native Style LSP and formatter](../reviews/requests/2026-07-14-seq-06.11d.5.3-native-style-lsp-formatter-contract.md).
The d.4.1 implementation exposes typed inventories and provenance for that
future work; it does not edit the concurrently owned tooling/LSP surface.

## Design deviations

- The package's mandatory public `inherited_axes` resolve-context field and
  `HostExplicit` injection are deferred to d.4.1.1. The implemented core uses
  the existing parent seam plus the fixed root default, avoiding a guessed host
  API while retaining the final axis/provider result types.
- `ViewStyleResolveError::AxisValueOverflow` and model diagnostics name their
  source field `style_source`, because `thiserror` reserves a field literally
  named `source` for nested error chaining.
- Full physical edge/inset semantics, logical scroll-start state, and
  provider-revision motion behavior are represented but not guessed.

## Validation

Focused evidence completed during implementation:

- `cargo test -p arcweft-view --test logical_axes`
- `cargo test -p arcweft-view --test logical_axis_cascade`
- `cargo test -p arcweft-view --test computed_style`
- `cargo test -p arcweft-lang-sema --test style box_axes`
- `cargo test -p arcweft-lang-sema --test style box_axis`
- `cargo test -p arcweft-bundle --test runtime_control_style_resolution`
- `cargo test -p arcweft-bundle --test view_style_program`
- `cargo test -p arcweft-player-scene box_style_consumes_only_canonical_physical_geometry`
- `cargo check -p arcweft-view -p arcweft-bundle -p arcweft-player-scene --all-targets --all-features`
- `cargo check -p arcweft-view -p arcweft-render-wgpu --all-targets --all-features`
- `cargo clippy -p arcweft-view --all-targets --all-features -- -D warnings`
- `cargo clippy -p arcweft-lang-sema --all-targets --all-features -- -D warnings`
- `cargo clippy -p arcweft-view -p arcweft-bundle -p arcweft-player-scene --all-targets --all-features -- -D warnings`
- `cargo test -p arcweft-view` (all unit, integration, and doc-test targets)
- `cargo test -p arcweft-lang-sema --test style` (13 passed)
- `cargo check -p arcweft-view -p arcweft-lang-sema -p arcweft-compiler -p arcweft-bundle -p arcweft-runtime-driver -p arcweft-player-scene -p arcweft-render-wgpu --all-targets --all-features`
- `cargo clippy -p arcweft-view -p arcweft-lang-sema -p arcweft-compiler -p arcweft-bundle -p arcweft-runtime-driver -p arcweft-player-scene -p arcweft-render-wgpu --all-targets --all-features -- -D warnings`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

The stabilized multi-package integration cut also passed:

- `cargo fmt --all -- --check`;
- `cargo check --workspace --all-targets --all-features`;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- `just test-workspace`;
- `just test-rich-text`; and
- `just test-doc`.

Its final dry-run structural audit scanned 2,731 files, including 1,299 Rust
files and 635,093 physical Rust LOC, with zero errors and 127 existing warnings.

The d.4.1 scoped structural-audit write scanned 2,722 repository files,
including 1,298 Rust files and 634,614 physical Rust LOC. It reported
zero errors and 127 warnings. The warnings are pre-existing workspace hotspots;
the d.4.1 resolver was split at its logical-axis boundary and no changed d.4.1
production file newly crosses a warning threshold. Machine-readable results are
checked in under
[`structure-audits/seq-06.11d.4.1-native-logical-axis`](structure-audits/seq-06.11d.4.1-native-logical-axis/).

## Structural audit

Measurements are from stable Jujutsu change `tuqmpsrwrnnp`. Bytes and physical
LOC are exact values from the current checkout, not diff additions. No Cargo
dependency was added or changed by this slice. `arcweft-view` has 7 direct
dependency edges and 11 workspace consumer edges in the generated dependency
inventory. Its fan-out is `arcweft-id`,
`arcweft-image`, `arcweft-presentation`, `serde`, development-only
`serde_json`, `thiserror`, and `unicode-segmentation`; its fan-in is
`arcweft-bundle`, `arcweft-character-view`, `arcweft-cli`, `arcweft-compiler`,
`arcweft-lang-sema`, `arcweft-player-scene`, development-only
`arcweft-player-text-input`, target-specific `arcweft-player-web`,
`arcweft-render-wgpu`, `arcweft-runtime-driver`, and `arcweft-runtime-host`.

### Changed production Rust files

| Owning crate | Path | Bytes | LOC | Classification and major responsibility |
| --- | --- | ---: | ---: | --- |
| `arcweft-view` | `src/lib.rs` | 7,293 | 158 | facade; deliberate Style exports; its `#[cfg(test)]` is only a two-line external test-module declaration |
| `arcweft-view` | `src/style.rs` | 6,062 | 182 | facade; responsibility-module exports |
| `arcweft-view` | `src/style/axis.rs` | 11,445 | 396 | production; closed modes, physical snapshots, signs, revisions, and usage sets |
| `arcweft-view` | `src/style/cascade.rs` | 10,214 | 320 | production; canonical contribution identity and builder boundary |
| `arcweft-view` | `src/style/computed.rs` | 10,717 | 340 | production; canonical computed map, transitions, axes, and physical packet |
| `arcweft-view` | `src/style/property.rs` | 36,671 | 1,090 | production; typed property inventory, metadata, expansion, and axis mapping |
| `arcweft-view` | `src/style/resolver.rs` | 38,342 | 1,080 | production orchestration; collection, cascade, cache, matching, and budgets |
| `arcweft-view` | `src/style/resolver/axis.rs` | 6,846 | 201 | production; axis winner, expansion, sign mapping, and transition lowering |
| `arcweft-view` | `src/style/sheet.rs` | 30,127 | 964 | Sans-I/O data model and checked declaration construction |
| `arcweft-view` | `src/style/value.rs` | 23,709 | 862 | production; typed specified values and checked fixed-milli sign operations |
| `arcweft-lang-sema` | `src/style/check.rs` | 18,354 | 513 | production; Style semantic validation and sign-reversibility rule |
| `arcweft-lang-sema` | `src/style/diagnostic.rs` | 5,658 | 191 | production; structured Style diagnostics |
| `arcweft-lang-sema` | `src/style/value.rs` | 25,471 | 720 | production; expected-type Style value checking and lowering |
| `arcweft-bundle` | `src/resource_codec/view/codec.rs` | 86,165 | 2,361 | production codec; exhaustive canonical View Style encoding/decoding |
| `arcweft-bundle` | `src/resource_codec/view/runtime_control_style.rs` | 9,236 | 271 | Sans-I/O runtime Style packet and physical-box ownership |
| `arcweft-bundle` | `src/resource_codec/view/runtime_control_style/projection.rs` | 19,860 | 532 | production boundary projection from computed Style to runtime data |
| `arcweft-player-scene` | `src/frame/view_style/consumer.rs` | 20,759 | 543 | production consumer; canonical physical layout/overflow packet use |
| `arcweft-render-wgpu` | `src/view.rs` | 5,658 | 192 | production renderer adapter; canonical physical translation use |

`resource_codec/view/codec.rs` was already a warning-level size hotspot. This
slice adds only the required exhaustive `BoxAxes` codec arm; the file remains a
cohesive generated-format boundary, contains no embedded tests, and remains
below the 2,500 LOC error threshold. The resolver initially crossed 1,200 LOC
during implementation, so logical-axis lowering was moved to
`resolver/axis.rs`; the orchestration file is now 1,080 LOC.

### Changed Rust test files

| Owning crate | Path | Bytes | LOC | Classification and major responsibility |
| --- | --- | ---: | ---: | --- |
| `arcweft-view` | `tests/logical_axes.rs` | 7,602 | 249 | integration test; closed inventory, exhaustive 22-by-4 mapping, metadata, and sign overflow |
| `arcweft-view` | `tests/logical_axis_cascade.rs` | 9,230 | 272 | integration test; two-phase cascade, shorthand, transition, usage, inheritance, and typed errors |
| `arcweft-lang-sema` | `tests/style.rs` | 13,629 | 443 | integration test; authored BoxAxes lowering and diagnostics |
| `arcweft-bundle` | `tests/runtime_control_style_resolution.rs` | 9,689 | 258 | integration test; canonical runtime projection |
| `arcweft-bundle` | `tests/view_style_program.rs` | 9,218 | 239 | integration test; codec and merge round trips |
| `arcweft-player-scene` | `src/frame/view_style/tests.rs` | 22,638 | 666 | unit test module; canonical physical consumer behavior |

### Largest workspace Rust files at the audit revision

| Path | Bytes | LOC | Classification and major responsibility |
| --- | ---: | ---: | --- |
| `crates/arcweft-text-layout/src/vertical_orientation.rs` | 357,456 | 12,399 | generated Unicode Vertical Orientation lookup table |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 256,594 | 7,974 | CLI integration and runtime benchmark checks |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 238,805 | 6,620 | native vertical observation integration tests |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 220,473 | 6,109 | published JLREQ class-mix integration corpus |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 214,731 | 5,850 | native sample/effect integration tests |
| `crates/arcweft-compiler/src/tests.rs` | 179,339 | 5,350 | compiler unit-test module |
| `crates/arcweft-cli/tests/check/agent_script_debug.rs` | 195,821 | 5,249 | Agent script/debug integration tests |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_units.rs` | 143,206 | 4,177 | published JLREQ unit integration corpus |
| `crates/arcweft-lang-sema/src/tests/typecheck.rs` | 125,373 | 4,120 | semantic type-check unit tests |
| `crates/arcweft-cli/src/app/agent/native/tests.rs` | 110,752 | 3,181 | native Agent application unit tests |
| `crates/arcweft-core/src/tests/flow.rs` | 88,953 | 2,553 | core runtime-flow unit tests |
| `crates/arcweft-runtime-driver/tests/session.rs` | 90,907 | 2,504 | runtime-driver session integration tests |
| `crates/arcweft-core/src/value.rs` | 84,017 | 2,500 | production runtime value model and operations |
| `crates/arcweft-cli/src/app/bundle/tests.rs` | 79,322 | 2,486 | CLI bundle unit tests |
| `crates/arcweft-core/src/engine/eval/calls.rs` | 89,488 | 2,481 | production runtime call evaluation |

The generated Unicode file is explicitly marked generated in its module
header and in `file_metrics.csv`; it is not mixed into ordinary production
hotspot assessment. None of the listed pre-existing workspace hotspots was
expanded by this package.
