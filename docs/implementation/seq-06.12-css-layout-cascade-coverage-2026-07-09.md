# Seq06.12 CSS layout/cascade coverage package application

Date: 2026-07-09

Maintenance update (2026-07-10): the fixture source scanner and its unused
expected-output files were removed. The maintained contract is the typed
`CssCoverageReport` behavior exercised from `coverage.css`; no claim is made
for computed-style or visual artifacts that are not generated and decoded by a
test.

## Source

Applied package:

```text
D:/sanze/Downloads/arcweft-seq06.12-css-layout-cascade-coverage-2026-07-07.zip
```

The package defines the deterministic first production CSS layout/cascade
coverage cut for the retained View/Takumi path. The checkout already contained
the main `arcweft-takumi-adapter::coverage` implementation, diagnostics, style
integration and coverage tests, so this application was
an idempotent comparison plus missing acceptance-test and evidence updates.

## Applied updates

- Added the package's extra coverage tests for:
  - product-data CSS custom property declarations;
  - fixture-backed flex/gap/padding support and grid diagnostics;
  - matrix rows that keep grid/container-query future work explicit.
- Updated `coverage.css` from `CssComponent` / `@layer ... component` to the
  current `CssView` / `@layer ... view` naming and consume it through the typed
  coverage analyzer test.
- Updated the seq06.12 design/future-work notes to use current View terminology.
- Fixed pre-existing `arcweft-render-wgpu::font_system` clippy warnings that
  blocked the package's `arcweft-takumi-adapter -D warnings` validation.

## Non-goals

- No browser DOM/CSSOM fallback was introduced.
- No PNG visual baselines were changed.
- Grid, container queries, media-query branching, and full computed-style
  snapshot extraction remain future work.
- Interactive `overflow:auto|scroll` remains diagnostic/probe coverage here;
  actual scrolling belongs to the retained `Scroll` contract.

## Validation

Executed:

```text
cargo fmt --all -- --check
cargo test -p arcweft-takumi-adapter --all-features css_layout_cascade --quiet
cargo test -p arcweft-takumi-adapter --all-features --quiet
cargo clippy -p arcweft-takumi-adapter --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/structure-audit-seq06-12-2026-07-09
```

Results:

- Focused coverage tests passed with 11 matching tests.
- The fixture-backed typed coverage test passed.
- `arcweft-takumi-adapter` all-features tests passed.
- `arcweft-takumi-adapter` all-targets/all-features clippy passed with
  `-D warnings`.
- Structure audit reported 2 existing error-level size violations:
  - `crates/arcweft-cli/src/app/bundle_view.rs`
  - `crates/arcweft-player-scene/src/input.rs`

The structure audit was written under `target/` rather than committed because
the checkout already had unrelated dirty changes in `input.rs`.
