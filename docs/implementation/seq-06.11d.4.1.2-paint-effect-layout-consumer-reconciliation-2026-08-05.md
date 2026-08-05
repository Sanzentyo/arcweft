# Paint-effect layout consumer reconciliation

Date: 2026-08-05
Inspected Git revision: `f9f40e9630f6f85b614aa79ccc244c037717c280`
Working-tree state at inspection: dirty; 462 paths were modified, including
this working cut and unrelated parallel edits, all of which were preserved.

## Outcome

The native `modern-feedback-view` failure was caused by a consumer-boundary
bug, not by an unsupported `BoxShadow` value in the DSL. The transient
physical projection sent every property classified as `RepresentedOnly` to the
layout validator. That made layout reject paint/compositing properties even
though the runtime already retained them in the paint/composite partitions and
projected their visual values.

The canonical geometry metadata now answers applicability per typed geometry
consumer. `PaintEffectBounds` remains represented-only and fail-closed for the
`Paint` consumer, but it does not block `Measure`, `Layout`, `Clip`, input,
focus, avoidance, scroll, or capture consumers. Flex distribution, wrapping,
alignment, rotation, masking, and the other represented-only features remain
rejected until their owning consumer can execute them.

No property-specific bypass was added to bundle or player code. The rule is
owned by `ViewRepresentedGeometryFeature` and is consumed by
`ViewGeometryPropertySupport`.

## Performed and passed

- Added the typed consumer applicability rule in
  `crates/arcweft-view/src/geometry/error.rs`.
- Added a geometry contract test covering all non-paint consumers, paint
  fail-closed behavior, and the existing FlexDistribution rejection.
- Added a bundle projection regression test proving that non-empty shadow,
  filter, and backdrop-filter values pass physical projection and remain in
  their typed runtime/visual owners.
- Ran targeted `rustfmt` on all changed Rust files.
- `cargo test -p arcweft-view --test geometry_contract`: 25 passed.
- `cargo test -p arcweft-view --test logical_axis_cascade`: 6 passed.
- `cargo test -p arcweft-bundle --test runtime_control_style_resolution`: 5
  passed.
- `cargo test -p arcweft-player-scene --lib frame::view_style::tests`: 18
  passed.
- `cargo test -p arcweft-player-scene --lib frame::view_geometry::tests`: 8
  passed.
- `cargo clippy -p arcweft-view --all-targets --all-features -- -D warnings`:
  passed.
- `cargo clippy -p arcweft-bundle --all-targets --all-features -- -D
  warnings`: passed.
- `cargo clippy -p arcweft-player-scene --all-targets --all-features -- -D
  warnings`: passed.
- `cargo check --workspace --all-targets --all-features`: passed. Existing
  warnings in unrelated language-HIR code remain visible but do not fail this
  check.
- Rebuilt the CLI with `cargo build -p arcweft-cli --all-features`.
- Re-ran the native sample with:

  ```text
  target\debug\arcw.exe run --runner native --manifest-path samples\modern-feedback-view\arcw.toml --profile main --text-input-trace-out target\modern-feedback-view\codex-native-validation-e556b25254f24c2da55e1923e3bf8b37.trace.json
  ```

  It reached `Running native player` without the prior geometry projection
  failure. The interactive process was then stopped deliberately; no orphan
  process remained.

## Failed, not run, and intentionally limited

- `cargo fmt --all -- --check` still fails on unrelated dirty files, including
  language-HIR, language-syntax, CLI, and identity files. The three changed Rust
  files pass targeted formatting; the workspace was not reformatted to avoid
  overwriting parallel work.
- The native sample was not kept open for an interactive/manual capture, so the
  validation trace was not produced. The relevant regression gate was the
  absence of the former frame-time failure after reaching the native player.
- `cargo test --workspace`, `just test-workspace`, and the structural audit were
  not run in this cut.

## Explicit non-goals

This cut does not implement paint-effect outsets in `ViewPaintOutsets` or make
`ViewGeometryConsumer::Paint` executable for shadow/filter bounds. That is a
separate renderer/geometry closure and remains an explicit typed capability
gap. It would require one shared paint-outset planner and corresponding final
geometry tests; it must not be approximated by making layout consume visual
effects or by silently dropping them.

Unrelated dirty working-tree changes were not modified, staged, or included in
this implementation cut.
